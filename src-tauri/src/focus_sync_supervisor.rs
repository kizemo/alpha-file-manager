// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! Focus Sync 侧车的存活监督。
//!
//! 背景（2026-11-10 复核坐实）：legacy 计划任务 `KizemoFocusSync` 用
//! `New-ScheduledTaskSettingsSet -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)`
//! 给侧车带了一份自愈能力 —— 侧车自己退出（panic / 端口被占 / 被杀）会被系统
//! 重新拉起。改造后侧车改由 AFM 主进程持有，**这份自愈能力随计划任务一起消失**：
//! 侧车一死，扩展只会收到
//! "Sidecar not reachable on 127.0.0.1:37421"，直到用户重启 Alpha。
//!
//! ⇒ 这里由主进程补回等价能力：监督线程发现侧车已退出就重新拉起，
//! 次数上限对齐 legacy 的 `-RestartCount 3`。
//!
//! ⚠️ 本模块刻意**不引用任何 tauri 类型**：`cargo test --lib` 的测试二进制里一旦
//! 链接到 `tauri::AppHandle`，`<Wry as Runtime>` 会被单态化出来并把 tao 整条窗口链
//! 拖进来，而 tao 需要 comctl32 v6（只有 application manifest 声明才解析得到），
//! 测试二进制会在**加载阶段**以 STATUS_ENTRYPOINT_NOT_FOUND 崩掉。
//! 所以监督逻辑放在这里、做成不依赖 tauri 的纯 std 模块，才能直接写单测。

use std::io;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// 监督轮询间隔（生产值）。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(10);
/// 最多自动重启次数，对齐 legacy 计划任务的 `-RestartCount 3`。
pub const MAX_RESTARTS: u32 = 3;
/// 连续存活超过该时长即认为这一轮健康，重启计数归零。
pub const HEALTHY_AFTER: Duration = Duration::from_secs(60);
/// 退避轮询粒度：让 `stop()` 能尽快打断等待，而不是干等一整个间隔。
const POLL_SLICE: Duration = Duration::from_millis(50);

/// 拉起侧车的方式（可注入，便于单测）。
pub type Launcher = Arc<dyn Fn() -> io::Result<Child> + Send + Sync + 'static>;

/// 监督器共享的内部状态。
struct Inner {
    launcher: Launcher,
    /// 当前侧车进程句柄；`None` 表示「当前没有活着的侧车」。
    child: Mutex<Option<Child>>,
    /// 本次拉起的时间点，用来判断「这一轮是否健康」。
    started_at: Mutex<Option<Instant>>,
    /// 已用掉的重启次数。
    restarts: Mutex<u32>,
}

impl Inner {
    /// 调用注入的拉起器。
    fn launch(&self) -> Option<Child> {
        match (self.launcher)() {
            Ok(child) => Some(child),
            Err(error) => {
                log::warn!("focus-sync sidecar: launch failed: {error}");
                None
            }
        }
    }

    /// 记录一个新拉起的子进程。
    fn adopt(&self, child: Child) {
        let pid = child.id();
        if let Ok(mut guard) = self.child.lock() {
            *guard = Some(child);
        }
        if let Ok(mut guard) = self.started_at.lock() {
            *guard = Some(Instant::now());
        }
        log::info!("focus-sync sidecar: launched pid={pid}");
    }

    /// 侧车是否还活着。返回 false 时顺带清掉句柄（`try_wait` 已经回收了子进程）。
    fn is_alive(&self) -> bool {
        let Ok(mut guard) = self.child.lock() else {
            return false;
        };
        let Some(child) = guard.as_mut() else {
            return false;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                log::warn!("focus-sync sidecar: exited unexpectedly, status={status}");
                *guard = None;
                false
            }
            Ok(None) => true,
            Err(error) => {
                log::warn!("focus-sync sidecar: try_wait failed: {error}");
                true
            }
        }
    }

    /// 若侧车已连续存活超过 `HEALTHY_AFTER`，把重启计数归零（这一轮是健康的）。
    fn refresh_health(&self) {
        let started_at = match self.started_at.lock() {
            Ok(guard) => *guard,
            Err(_) => return,
        };
        let Some(started_at) = started_at else {
            return;
        };
        if started_at.elapsed() < HEALTHY_AFTER {
            return;
        }
        if let Ok(mut restarts) = self.restarts.lock() {
            *restarts = 0;
        }
    }

    /// 尝试重新拉起侧车；返回是否拉起成功。
    fn restart(&self) -> bool {
        let attempt = {
            let Ok(mut restarts) = self.restarts.lock() else {
                return false;
            };
            if *restarts >= MAX_RESTARTS {
                log::error!(
                    "focus-sync sidecar: giving up after {MAX_RESTARTS} restart attempts; \
                     the Focus Sync extension will report the sidecar as unreachable until Alpha restarts"
                );
                return false;
            }
            *restarts += 1;
            *restarts
        };

        match self.launch() {
            Some(child) => {
                self.adopt(child);
                log::info!(
                    "focus-sync sidecar: restarted (attempt {attempt}/{MAX_RESTARTS})"
                );
                true
            }
            None => false,
        }
    }

    /// 杀掉当前侧车（若有）。
    fn kill(&self) {
        let taken = match self.child.lock() {
            Ok(mut guard) => guard.take(),
            Err(_) => None,
        };
        if let Some(mut child) = taken {
            let pid = child.id();
            match child.kill() {
                Ok(()) => log::info!("focus-sync sidecar: killed pid={pid}"),
                Err(error) => log::warn!("focus-sync sidecar: kill pid={pid} failed: {error}"),
            }
            let _ = child.wait();
        }
    }
}

/// 侧车监督器：持有子进程句柄，并在子进程意外退出时重新拉起。
pub struct Supervisor {
    inner: Arc<Inner>,
    stop: Arc<AtomicBool>,
    interval: Duration,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Supervisor {
    /// 生产用构造：默认轮询间隔。
    pub fn new(launcher: Launcher) -> Self {
        Self::with_interval(launcher, DEFAULT_INTERVAL)
    }

    /// 指定轮询间隔（单测用）。
    pub fn with_interval(launcher: Launcher, interval: Duration) -> Self {
        let inner = Arc::new(Inner {
            launcher,
            child: Mutex::new(None),
            started_at: Mutex::new(None),
            restarts: Mutex::new(0),
        });
        Self {
            inner,
            stop: Arc::new(AtomicBool::new(false)),
            interval,
            handle: Mutex::new(None),
        }
    }

    /// 拉起侧车并启动监督线程。
    pub fn start(&self) {
        if let Some(child) = self.inner.launch() {
            self.inner.adopt(child);
        }

        let inner = Arc::clone(&self.inner);
        let stop = Arc::clone(&self.stop);
        let interval = self.interval;
        let spawned = std::thread::Builder::new()
            .name("focus-sync-supervisor".to_string())
            .spawn(move || supervise_loop(inner, stop, interval));
        match spawned {
            Ok(handle) => {
                if let Ok(mut guard) = self.handle.lock() {
                    *guard = Some(handle);
                }
            }
            Err(error) => {
                log::warn!("focus-sync sidecar: failed to spawn supervisor thread: {error}");
            }
        }
    }

    /// 停止监督并杀掉侧车（应用退出时调用）。
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.inner.kill();
        let handle = match self.handle.lock() {
            Ok(mut guard) => guard.take(),
            Err(_) => None,
        };
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        // 兜底：即使没走 stop() 也要让监督线程与侧车收掉。
        self.stop.store(true, Ordering::Relaxed);
        self.inner.kill();
    }
}

/// 监督线程主循环。
fn supervise_loop(inner: Arc<Inner>, stop: Arc<AtomicBool>, interval: Duration) {
    loop {
        if !sleep_until_stop(&stop, interval) {
            return;
        }
        if inner.is_alive() {
            inner.refresh_health();
        } else {
            inner.restart();
        }
    }
}

/// 分片睡眠；`stop` 被置位时立刻返回 false（表示「该退出了」）。
fn sleep_until_stop(stop: &AtomicBool, total: Duration) -> bool {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(POLL_SLICE);
    }
    !stop.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::sync::atomic::AtomicUsize;

    fn comspec() -> String {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }

    /// 拉起一个「立刻退出」的进程，模拟侧车崩溃。
    fn launcher_exiting(counter: Arc<AtomicUsize>) -> Launcher {
        Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Command::new(comspec())
                .args(["/c", "exit", "0"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        })
    }

    /// 拉起一个「一直活着」的进程，模拟侧车健康运行。
    fn launcher_alive(counter: Arc<AtomicUsize>) -> Launcher {
        Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Command::new(comspec())
                .args(["/c", "ping -n 60 127.0.0.1 > nul"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        })
    }

    /// 拉起总是失败的启动器。
    fn launcher_failing(counter: Arc<AtomicUsize>) -> Launcher {
        Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Err(io::Error::new(io::ErrorKind::Other, "boom"))
        })
    }

    fn wait_until(counter: &Arc<AtomicUsize>, target: usize, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if counter.load(Ordering::SeqCst) >= target {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn restarts_the_sidecar_after_it_exits() {
        let counter = Arc::new(AtomicUsize::new(0));
        let supervisor = Supervisor::with_interval(
            launcher_exiting(Arc::clone(&counter)),
            Duration::from_millis(50),
        );
        supervisor.start();

        assert!(
            wait_until(&counter, 2, Duration::from_secs(10)),
            "sidecar should be relaunched after an unexpected exit, launches={}",
            counter.load(Ordering::SeqCst)
        );
        supervisor.stop();
    }

    #[test]
    fn stop_prevents_further_restarts() {
        let counter = Arc::new(AtomicUsize::new(0));
        let supervisor = Supervisor::with_interval(
            launcher_exiting(Arc::clone(&counter)),
            Duration::from_millis(50),
        );
        supervisor.start();

        assert!(wait_until(&counter, 2, Duration::from_secs(10)));
        supervisor.stop();
        let after_stop = counter.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(
            counter.load(Ordering::SeqCst),
            after_stop,
            "no relaunch must happen after stop()"
        );
    }

    #[test]
    fn gives_up_after_max_restarts() {
        let counter = Arc::new(AtomicUsize::new(0));
        let supervisor =
            Supervisor::with_interval(launcher_failing(Arc::clone(&counter)), Duration::from_millis(50));
        supervisor.start();

        let expected = 1 + MAX_RESTARTS as usize;
        assert!(
            wait_until(&counter, expected, Duration::from_secs(10)),
            "expected {expected} launch attempts, got {}",
            counter.load(Ordering::SeqCst)
        );
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            counter.load(Ordering::SeqCst),
            expected,
            "supervisor must stop retrying after MAX_RESTARTS"
        );
        supervisor.stop();
    }

    #[test]
    fn healthy_sidecar_is_not_restarted() {
        let counter = Arc::new(AtomicUsize::new(0));
        let supervisor = Supervisor::with_interval(
            launcher_alive(Arc::clone(&counter)),
            Duration::from_millis(50),
        );
        supervisor.start();

        std::thread::sleep(Duration::from_millis(800));
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "a healthy sidecar must not be relaunched"
        );
        supervisor.stop();
    }
}
