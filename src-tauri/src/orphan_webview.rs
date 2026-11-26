// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 清理「上一次异常退出」留下的 WebView2 孤儿进程。
//!
//! 背景
//!
//! 应用被强杀（任务管理器结束进程 / SIGKILL）时，WebView2 的子进程不会跟着退出。
//! 它们仍然持有 WebView2 用户数据目录（%LOCALAPPDATA%\<identifier>\EBWebView）的句柄，
//! 于是下一次启动时 WebView2 创建环境失败：
//!
//!     failed to setup app: failed to create webview:
//!     WebView2 error: WindowsError(HRESULT(0x800700AA), "请求的资源在使用中。")
//!
//! 应用会在这里 panic 退出（exit code 101），用户的表现是「应用突然打不开了」。
//!
//! 孤儿进程本身也是资源泄漏：每个孤儿 msedgewebview2.exe 常驻几十 MB 内存，
//! 攒多了会持续吃 CPU 与内存。
//!
//! 安全边界
//!
//! 只处理同时满足三个条件的进程：
//!
//! 1. 进程名是 msedgewebview2.exe；
//! 2. 命令行里引用了我们自己的 WebView2 用户数据目录；
//! 3. 它的祖先链里没有活着的应用实例 —— 正在运行的另一个实例不会被误杀。
//!
//! 这个模块刻意不引用 tauri：这样它可以进 cargo test --lib 的测试二进制。
//! 测试二进制一旦链上 tauri::AppHandle / tauri::async_runtime 会在加载阶段
//! 以 STATUS_ENTRYPOINT_NOT_FOUND 崩掉（见 AGENTS.md 里的记录）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Tauri identifier，受保护常量，与 tauri.conf.json 保持一致。
/// 下面的单测会对着 tauri.conf.json 逐字校验，防止两边漂移。
pub const APP_IDENTIFIER: &str = "com.sigma-file-manager.app";

/// WebView2 子进程的文件名。
const WEBVIEW_PROCESS_NAME: &str = "msedgewebview2.exe";

/// WebView2 用户数据目录在 <LocalAppData>\<identifier> 下的文件夹名。
const WEBVIEW_DATA_DIR_NAME: &str = "EBWebView";

/// 沿祖先链向上查找的最大步数。
const MAX_ANCESTOR_DEPTH: usize = 32;

/// 进程表里的一条记录。
#[derive(Clone, Debug)]
pub struct ProcessRecord {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub cmdline: String,
}

impl ProcessRecord {
    fn new(pid: u32, parent: Option<u32>, name: &str, cmdline: &str) -> Self {
        Self {
            pid,
            parent,
            name: name.to_string(),
            cmdline: cmdline.to_string(),
        }
    }
}

/// 祖先链里是否有活着的应用实例。
///
/// records 是当前活着的进程表快照；祖先链走不动的地方（pid 不在快照里）
/// 就是已经死掉的祖先 —— 那说明这个 WebView2 是上一轮留下的残留。
fn belongs_to_live_app(
    record: &ProcessRecord,
    by_pid: &HashMap<u32, &ProcessRecord>,
    app_names: &[String],
) -> bool {
    let mut current = record.parent;
    for _ in 0..MAX_ANCESTOR_DEPTH {
        let Some(pid) = current else {
            return false;
        };
        let Some(ancestor) = by_pid.get(&pid) else {
            // 祖先已经不在进程表里了 => 它死了 => 这个 WebView2 是残留。
            return false;
        };
        if app_names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&ancestor.name))
        {
            return true;
        }
        current = ancestor.parent;
    }
    false
}

/// 选出「我们自己的孤儿 WebView2 进程」。
///
/// - records：当前活着的进程表快照（pid / 父 pid / 进程名 / 命令行）。
/// - app_names：应用自身的进程名（通常是 alpha-file-manager.exe）。
/// - markers：命令行里出现即视为「引用了我们自己的 WebView2 目录」的片段。
pub fn collect_orphan_webview_pids(
    records: &[ProcessRecord],
    app_names: &[String],
    markers: &[String],
) -> Vec<u32> {
    let lowered_markers: Vec<String> = markers
        .iter()
        .filter(|marker| !marker.is_empty())
        .map(|marker| marker.to_lowercase())
        .collect();
    if lowered_markers.is_empty() {
        return Vec::new();
    }

    let mut by_pid: HashMap<u32, &ProcessRecord> = HashMap::with_capacity(records.len());
    for record in records {
        by_pid.insert(record.pid, record);
    }

    let mut orphans = Vec::new();
    for record in records {
        if !record.name.eq_ignore_ascii_case(WEBVIEW_PROCESS_NAME) {
            continue;
        }
        let cmdline = record.cmdline.to_lowercase();
        if !lowered_markers.iter().any(|marker| cmdline.contains(marker)) {
            continue;
        }
        if belongs_to_live_app(record, &by_pid, app_names) {
            continue;
        }
        orphans.push(record.pid);
    }
    orphans.sort_unstable();
    orphans
}

/// 命令行匹配片段：显式指定的 WebView2 数据目录优先。
fn webview_markers() -> Vec<String> {
    let mut markers = Vec::new();
    if let Some(value) = std::env::var_os("WEBVIEW2_USER_DATA_FOLDER") {
        let text = value.to_string_lossy().to_string();
        if !text.is_empty() {
            markers.push(text);
        }
    }
    if let Some(path) = default_webview_data_dir() {
        let text = path.to_string_lossy().to_string();
        if !text.is_empty() {
            markers.push(text);
        }
    }
    // 兜底：只依赖 identifier + 目录名，不受用户目录写法影响。
    markers.push(format!("{APP_IDENTIFIER}\\{WEBVIEW_DATA_DIR_NAME}"));
    markers
}

/// WebView2 用户数据目录（release 构建下的默认位置）。
fn default_webview_data_dir() -> Option<PathBuf> {
    crate::app_dirs::local_data_dir_without_app(APP_IDENTIFIER)
        .map(|dir| dir.join(WEBVIEW_DATA_DIR_NAME))
}

/// 应用自身的进程名（alpha-file-manager.exe）。
fn app_process_names() -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(file_name) = exe.file_name() {
            let name = file_name.to_string_lossy().to_string();
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    if names.is_empty() {
        names.push("alpha-file-manager.exe".to_string());
    }
    names
}

/// 启动前清理孤儿 WebView2 进程，返回被清理掉的 pid 列表。
///
/// 必须在任何 WebView2 进程被创建之前调用。
pub fn cleanup_orphan_webview_processes() -> Vec<u32> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

    let app_names = app_process_names();
    let markers = webview_markers();

    // 第一遍：只读进程表（不含命令行），开销小。
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing(),
    );

    // 只有真的存在 WebView2 进程时，才去读它们的命令行。
    let candidates: Vec<Pid> = system
        .processes()
        .iter()
        .filter(|(_, process)| process.name().eq_ignore_ascii_case(WEBVIEW_PROCESS_NAME))
        .map(|(pid, _)| *pid)
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }

    // 第二遍：只给这些进程补上命令行。
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&candidates),
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );

    let records: Vec<ProcessRecord> = system
        .processes()
        .iter()
        .map(|(pid, process)| {
            let cmdline = process
                .cmd()
                .iter()
                .map(|part| part.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            ProcessRecord::new(
                pid.as_u32(),
                process.parent().map(|parent| parent.as_u32()),
                &process.name().to_string_lossy(),
                &cmdline,
            )
        })
        .collect();

    let orphans = collect_orphan_webview_pids(&records, &app_names, &markers);
    if orphans.is_empty() {
        return Vec::new();
    }

    log::warn!(
        "orphan webview cleanup: {} stale WebView2 process(es) found: {:?}",
        orphans.len(),
        orphans
    );
    eprintln!(
        "[afm] orphan webview cleanup: killing {} stale process(es): {:?}",
        orphans.len(),
        orphans
    );

    let mut killed = Vec::new();
    for pid in &orphans {
        let pid = Pid::from(*pid as usize);
        let Some(process) = system.process(pid) else {
            continue;
        };
        if process.kill() {
            killed.push(pid.as_u32());
        } else {
            log::warn!(
                "orphan webview cleanup: failed to kill pid {}",
                pid.as_u32()
            );
        }
    }

    if !killed.is_empty() {
        // 句柄是在进程对象真正销毁时才释放的，给 WebView2 一点时间。
        std::thread::sleep(Duration::from_millis(300));
    }

    eprintln!(
        "[afm] orphan webview cleanup: killed {} stale process(es)",
        killed.len()
    );
    killed
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "alpha-file-manager.exe";

    fn marker() -> String {
        format!("{APP_IDENTIFIER}\\{WEBVIEW_DATA_DIR_NAME}")
    }

    fn webview_cmd() -> String {
        format!(
            "msedgewebview2.exe --type=renderer --user-data-dir=C:\\Users\\tester\\AppData\\Local\\{}\\{}",
            APP_IDENTIFIER, WEBVIEW_DATA_DIR_NAME
        )
    }

    #[test]
    fn identifier_matches_tauri_conf() {
        let conf = include_str!("../tauri.conf.json");
        assert!(
            conf.contains(&format!("\"identifier\": \"{APP_IDENTIFIER}\"")),
            "APP_IDENTIFIER 与 tauri.conf.json 不一致"
        );
    }

    #[test]
    fn orphan_when_the_creator_is_gone() {
        // 应用（pid 100）已经死了，不在进程表里。
        let records = vec![ProcessRecord::new(
            200,
            Some(100),
            WEBVIEW_PROCESS_NAME,
            &webview_cmd(),
        )];

        let orphans = collect_orphan_webview_pids(&records, &[APP.to_string()], &[marker()]);
        assert_eq!(orphans, vec![200]);
    }

    #[test]
    fn live_instance_protects_its_webview_children() {
        let records = vec![
            ProcessRecord::new(100, None, APP, "C:\\app\\alpha-file-manager.exe"),
            ProcessRecord::new(200, Some(100), WEBVIEW_PROCESS_NAME, &webview_cmd()),
            ProcessRecord::new(201, Some(200), WEBVIEW_PROCESS_NAME, &webview_cmd()),
        ];

        let orphans = collect_orphan_webview_pids(&records, &[APP.to_string()], &[marker()]);
        assert!(orphans.is_empty(), "活着的实例不该被当成孤儿: {orphans:?}");
    }

    #[test]
    fn renderer_children_of_an_orphan_are_orphans_too() {
        let records = vec![
            ProcessRecord::new(200, Some(100), WEBVIEW_PROCESS_NAME, &webview_cmd()),
            ProcessRecord::new(201, Some(200), WEBVIEW_PROCESS_NAME, &webview_cmd()),
        ];

        let orphans = collect_orphan_webview_pids(&records, &[APP.to_string()], &[marker()]);
        assert_eq!(orphans, vec![200, 201]);
    }

    #[test]
    fn other_apps_webview_is_untouched() {
        let records = vec![ProcessRecord::new(
            200,
            Some(100),
            WEBVIEW_PROCESS_NAME,
            "msedgewebview2.exe --user-data-dir=C:\\Users\\tester\\AppData\\Local\\Other.App\\EBWebView",
        )];

        let orphans = collect_orphan_webview_pids(&records, &[APP.to_string()], &[marker()]);
        assert!(orphans.is_empty());
    }

    #[test]
    fn non_webview_process_is_untouched() {
        let records = vec![ProcessRecord::new(
            200,
            Some(100),
            "some-other.exe",
            &webview_cmd(),
        )];

        let orphans = collect_orphan_webview_pids(&records, &[APP.to_string()], &[marker()]);
        assert!(orphans.is_empty());
    }

    #[test]
    fn empty_markers_never_kill() {
        let records = vec![ProcessRecord::new(
            200,
            Some(100),
            WEBVIEW_PROCESS_NAME,
            &webview_cmd(),
        )];

        let orphans = collect_orphan_webview_pids(&records, &[APP.to_string()], &[]);
        assert!(orphans.is_empty());
    }
}
