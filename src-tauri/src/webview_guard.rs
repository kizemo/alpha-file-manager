// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! WebView2 子进程崩溃兜底 —— **纯逻辑部分**。
//!
//! ## 为什么必须单独拆一个文件
//!
//! `cargo test --lib` 的测试二进制有一条隐形红线：**任何被测试链接到的代码都不能碰到
//! `tauri::AppHandle` 或 `tauri::async_runtime::*`**。
//!
//! 根因是三步连锁：① `lib.rs` 的 `run()` 持有全部 command，但测试不调 `run()`，
//! 所以基线上 **tao 一条导入都没有**，GUI 代码整段被链接器丢掉；
//! ② 一旦某段代码被测试链接到并出现 `tauri::AppHandle`（默认 `R = Wry`），
//! 它就会单态化出 `tauri_runtime_wry`，把 tao 整条窗口链拖进来；
//! ③ tao 里有 `comctl32.dll!TaskDialogIndirect`，它属于 **comctl32 v6**，
//! 只有 application manifest 声明了才解析得到，测试二进制没有 manifest ⇒
//! 加载器找不到导出，整个测试二进制以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉，
//! 一个用例都跑不了，而且报错里完全看不出跟本模块有关。
//!
//! 所以：**判定逻辑放在这里（只依赖 sysinfo）**，依赖 tauri 的接线放在
//! `webview_guard_runtime.rs`，那份代码只从 `lib.rs::run()` 可达 ⇒ 测试二进制里会被链接器丢掉。
//!
//! ## 判定口径
//!
//! - **心跳**：前端窗口每 5 秒调一次 `webview_guard_heartbeat`。
//!   渲染进程死掉 ⇒ 该窗口的 JS 不再执行 ⇒ 心跳停。
//! - **进程旁证**：`msedgewebview2.exe` 的 pid 集合里少了东西 ⇒ 确实有子进程死过。
//! - 两者结合才能区分「渲染进程崩了」（有进程旁证）与「前端只是忙」（没有旁证）。

use std::collections::{HashMap, HashSet};

/// 前端心跳间隔（毫秒）。
pub const HEARTBEAT_INTERVAL_MS: u64 = 5_000;

/// 心跳超时（毫秒）：超过这个时间没收到心跳，就认为该窗口的渲染进程失联。
pub const HEARTBEAT_TIMEOUT_MS: u64 = 20_000;

/// 心跳超时的几倍。超过这个倍数仍然没有心跳，**即使没有任何进程消失的旁证**
/// 也判定为「卡死」—— 因为「进程没死但页面不动了」同样需要兜底。
pub const HUNG_TIMEOUT_MULTIPLIER: u64 = 3;

/// 看门狗轮询间隔（毫秒）。
pub const WATCHDOG_POLL_INTERVAL_MS: u64 = 2_000;

/// 判定「WebView2 浏览器进程消失」需要连续命中的次数。
///
/// 一次采样就报致命错误太敏感：应用退出时 webview 进程本来就会先死，
/// 再加上 WebView2 自己重启浏览器进程时也有一个窗口期。
pub const BROWSER_GONE_CONFIRM_TICKS: u32 = 2;

/// WebView2 在 Windows 上的宿主进程名（小写比较）。
pub const WEBVIEW_PROCESS_NAME: &str = "msedgewebview2.exe";

/// 祖先回溯的最大深度（防止进程树里有环时死循环）。
const MAX_ANCESTOR_DEPTH: usize = 32;

/// 一次进程扫描的结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebviewProcessSnapshot {
    /// 直接挂在宿主进程下的 `msedgewebview2.exe`（即 WebView2 **浏览器进程**）。
    pub browser_pids: Vec<u32>,
    /// 宿主进程树里所有 `msedgewebview2.exe`（含浏览器 / 渲染 / GPU / utility）。
    pub all_pids: Vec<u32>,
}

impl WebviewProcessSnapshot {
    /// 浏览器进程是否还在。
    pub fn has_browser(&self) -> bool {
        !self.browser_pids.is_empty()
    }
}

/// 一个窗口的心跳记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowHeartbeat {
    /// 窗口标签（`main` / `quick-view` / ...）。
    pub label: String,
    /// 最近一次心跳的时刻（毫秒，看门狗启动以来的单调时钟）。
    pub last_ping_ms: u64,
    /// 窗口当前是不是「看得见的」（可见且没有最小化）。
    ///
    /// 后台标签页的定时器会被 Chromium 限流（隐藏 5 分钟之后甚至 1 分钟才放行一次），
    /// 那种心跳间隔是限流的产物，不是界面卡住 ⇒ 不可见的窗口不参与「卡死」判定。
    pub visible: bool,
}

/// 看门狗的判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebviewGuardVerdict {
    /// 一切正常。
    Healthy,
    /// 某个窗口的渲染进程崩了（有「进程消失」的旁证）。
    RendererCrashed {
        label: String,
        missed_ms: u64,
    },
    /// 某个窗口长时间没有心跳，但没有任何进程消失的旁证 ⇒ 大概率是卡死。
    WindowHung {
        label: String,
        missed_ms: u64,
    },
    /// WebView2 浏览器进程没了 ⇒ 所有 webview 一起失效，界面无法恢复。
    BrowserProcessGone,
}

/// 看门狗的判定。
///
/// 参数：
/// - `now_ms`：当前时刻（毫秒，与心跳同一时间基准）。
/// - `heartbeats`：所有注册过心跳的窗口。
/// - `snapshot`：本次进程扫描结果。
/// - `expect_browser`：此前是否见过浏览器进程（没见过就说明是启动初期，不算异常）。
/// - `webview_process_vanished`：本次扫描是否发现有 WebView2 子进程消失。
pub fn decide_health(
    now_ms: u64,
    heartbeats: &[WindowHeartbeat],
    snapshot: &WebviewProcessSnapshot,
    expect_browser: bool,
    webview_process_vanished: bool,
) -> WebviewGuardVerdict {
    if expect_browser && !snapshot.has_browser() {
        return WebviewGuardVerdict::BrowserProcessGone;
    }

    let mut hung: Option<&WindowHeartbeat> = None;

    for heartbeat in heartbeats {
        let missed_ms = now_ms.saturating_sub(heartbeat.last_ping_ms);

        if missed_ms <= HEARTBEAT_TIMEOUT_MS {
            continue;
        }

        if webview_process_vanished {
            return WebviewGuardVerdict::RendererCrashed {
                label: heartbeat.label.clone(),
                missed_ms,
            };
        }

        // 只有「看得见的窗口」才判卡死：隐藏 / 最小化的窗口，Chromium 会限流
        // 它的定时器，心跳间隔变长是限流的产物，不是界面卡住。
        if missed_ms >= HEARTBEAT_TIMEOUT_MS.saturating_mul(HUNG_TIMEOUT_MULTIPLIER)
            && heartbeat.visible
            && hung.is_none()
        {
            hung = Some(heartbeat);
        }
    }

    match hung {
        Some(heartbeat) => WebviewGuardVerdict::WindowHung {
            label: heartbeat.label.clone(),
            missed_ms: now_ms.saturating_sub(heartbeat.last_ping_ms),
        },
        None => WebviewGuardVerdict::Healthy,
    }
}

/// 判断「WebView2 浏览器进程被换掉了」。
///
/// 杀浏览器进程时，WebView2 会自己把浏览器进程拉起来（新的 pid），
/// 而被杀掉的那个进程在进程表里可能还挂着（句柄没释放之前不会真正从表里消失）
/// ⇒ 「pid 集合少了东西」这条旁证不成立，判定会退化成「心跳超时且没有旁证」。
/// 所以这一类故障改用「浏览器进程集合变了」来认：
/// 之前见过的浏览器 pid 一个都不在了，同时出现了新的浏览器 pid。
///
/// 浏览器进程整个消失（`current` 为空）不算「被换掉」——
/// 那是 `WebviewGuardVerdict::BrowserProcessGone` 那条路，由判定逻辑优先处理。
pub fn browser_process_replaced(previous: &[u32], current: &[u32]) -> bool {
    if previous.is_empty() || current.is_empty() {
        return false;
    }

    let current_set: HashSet<u32> = current.iter().copied().collect();
    !previous.iter().any(|pid| current_set.contains(pid))
}

/// 从进程表里挑出属于宿主进程树的 WebView2 子进程。
///
/// `entries` 的每一项是 `(pid, parent_pid, 是否 webview 进程)`。
/// 拆成纯函数是为了能单测：sysinfo 只负责把进程表读出来。
pub fn collect_webview_pids(
    entries: &[(u32, Option<u32>, bool)],
    host_pid: u32,
) -> WebviewProcessSnapshot {
    let mut parent_map: HashMap<u32, Option<u32>> = HashMap::with_capacity(entries.len());
    for (pid, parent, _) in entries {
        parent_map.insert(*pid, *parent);
    }

    let is_descendant = |pid: u32| -> bool {
        let mut current = pid;
        for _ in 0..MAX_ANCESTOR_DEPTH {
            if current == host_pid {
                return true;
            }
            match parent_map.get(&current) {
                Some(Some(parent)) => current = *parent,
                _ => return false,
            }
        }
        false
    };

    let mut snapshot = WebviewProcessSnapshot::default();

    for (pid, parent, is_webview) in entries {
        // 宿主进程自己不算「webview 子进程」。
        // 生产环境宿主是 alpha-file-manager.exe，名字对不上本来也进不来，
        // 但这里显式排除，契约才不依赖「宿主不可能同名」这个前提。
        if *pid == host_pid {
            continue;
        }
        if !is_webview || !is_descendant(*pid) {
            continue;
        }

        snapshot.all_pids.push(*pid);

        // 直接挂在宿主进程下的那个就是 WebView2 浏览器进程。
        if *parent == Some(host_pid) {
            snapshot.browser_pids.push(*pid);
        }
    }

    snapshot
}

/// 扫描宿主进程树里的 WebView2 子进程。
pub fn scan_webview_processes() -> WebviewProcessSnapshot {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

    let host_pid = std::process::id();
    let mut system = System::new();
    // `nothing()` 只表示不刷新额外信息（CPU / 内存 / 命令行），
    // 进程表本身与 name / parent 仍然会刷新。
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());

    let mut entries: Vec<(u32, Option<u32>, bool)> = Vec::new();
    for (pid, process) in system.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        let is_webview = name == WEBVIEW_PROCESS_NAME;
        entries.push((pid.as_u32(), process.parent().map(|p| p.as_u32()), is_webview));
    }

    collect_webview_pids(&entries, host_pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heartbeat(label: &str, last_ping_ms: u64) -> WindowHeartbeat {
        WindowHeartbeat {
            label: label.to_string(),
            last_ping_ms,
            visible: true,
        }
    }

    fn hidden_heartbeat(label: &str, last_ping_ms: u64) -> WindowHeartbeat {
        WindowHeartbeat {
            label: label.to_string(),
            last_ping_ms,
            visible: false,
        }
    }

    fn snapshot_with_browser() -> WebviewProcessSnapshot {
        WebviewProcessSnapshot {
            browser_pids: vec![100],
            all_pids: vec![100, 101],
        }
    }

    #[test]
    fn healthy_when_heartbeat_is_fresh() {
        let verdict = decide_health(
            1_000,
            &[heartbeat("main", 900)],
            &snapshot_with_browser(),
            true,
            false,
        );

        assert_eq!(verdict, WebviewGuardVerdict::Healthy);
    }

    #[test]
    fn browser_gone_is_fatal_only_after_browser_was_seen() {
        let empty = WebviewProcessSnapshot::default();

        // 启动初期还没建出浏览器进程 ⇒ 不算异常。
        assert_eq!(
            decide_health(1_000, &[], &empty, false, false),
            WebviewGuardVerdict::Healthy
        );

        // 见过之后又没了 ⇒ 致命。
        assert_eq!(
            decide_health(1_000, &[], &empty, true, false),
            WebviewGuardVerdict::BrowserProcessGone
        );
    }

    #[test]
    fn stale_heartbeat_with_process_evidence_is_renderer_crash() {
        let verdict = decide_health(
            60_000,
            &[heartbeat("main", 1_000)],
            &snapshot_with_browser(),
            true,
            true,
        );

        assert_eq!(
            verdict,
            WebviewGuardVerdict::RendererCrashed {
                label: "main".to_string(),
                missed_ms: 59_000,
            }
        );
    }

    #[test]
    fn stale_heartbeat_without_process_evidence_stays_healthy_until_hung_threshold() {
        // 超时之内（20s）不算事。
        assert_eq!(
            decide_health(
                20_000,
                &[heartbeat("main", 1_000)],
                &snapshot_with_browser(),
                true,
                false
            ),
            WebviewGuardVerdict::Healthy
        );

        // 超过 3 倍超时（60s）⇒ 判定为卡死。
        assert_eq!(
            decide_health(
                61_000,
                &[heartbeat("main", 1_000)],
                &snapshot_with_browser(),
                true,
                false
            ),
            WebviewGuardVerdict::WindowHung {
                label: "main".to_string(),
                missed_ms: 60_000,
            }
        );
    }

    #[test]
    fn hidden_window_is_never_judged_hung() {
        // 隐藏 / 最小化的窗口：定时器被 Chromium 限流，心跳间隔变长是
        // 限流的产物，不该被判成「界面卡住」。
        let verdict = decide_health(
            120_000,
            &[hidden_heartbeat("main", 1_000)],
            &snapshot_with_browser(),
            true,
            false,
        );

        assert_eq!(verdict, WebviewGuardVerdict::Healthy);
    }

    #[test]
    fn crashed_window_wins_over_hung_window() {
        let verdict = decide_health(
            120_000,
            &[heartbeat("quick-view", 1_000), heartbeat("main", 2_000)],
            &snapshot_with_browser(),
            true,
            true,
        );

        match verdict {
            WebviewGuardVerdict::RendererCrashed { label, .. } => assert_eq!(label, "quick-view"),
            other => panic!("unexpected verdict: {other:?}"),
        }
    }

    #[test]
    fn collect_webview_pids_only_keeps_descendants_of_host() {
        let entries = vec![
            (10, None, true),        // 宿主自己
            (11, Some(10), true),    // 浏览器进程
            (12, Some(11), true),    // 渲染进程
            (13, Some(11), true),    // GPU 进程
            (14, Some(99), true),    // 别的应用的 webview
            (15, Some(10), false),   // 宿主直接拉起的别的进程
        ];

        let snapshot = collect_webview_pids(&entries, 10);

        assert_eq!(snapshot.browser_pids, vec![11]);
        assert_eq!(snapshot.all_pids, vec![11, 12, 13]);
    }

    #[test]
    fn collect_webview_pids_survives_parent_cycle() {
        let entries = vec![(10, None, true), (11, Some(12), true), (12, Some(11), true)];

        let snapshot = collect_webview_pids(&entries, 10);

        assert!(snapshot.all_pids.is_empty());
    }

    #[test]
    fn snapshot_reports_browser_presence() {
        assert!(snapshot_with_browser().has_browser());
        assert!(!WebviewProcessSnapshot::default().has_browser());
    }

    #[test]
    fn browser_process_replaced_is_false_before_any_browser_was_seen() {
        assert!(!browser_process_replaced(&[], &[11]));
    }

    #[test]
    fn browser_process_replaced_is_true_when_the_browser_pid_changed() {
        assert!(browser_process_replaced(&[10], &[11]));
    }

    #[test]
    fn browser_process_replaced_is_false_when_the_browser_stayed() {
        assert!(!browser_process_replaced(&[10], &[10]));
    }

    #[test]
    fn browser_process_replaced_is_false_when_the_browser_disappeared_entirely() {
        // 浏览器进程整个没了是 BrowserProcessGone 那条路，不算「被换掉」。
        assert!(!browser_process_replaced(&[10], &[]));
    }
}
