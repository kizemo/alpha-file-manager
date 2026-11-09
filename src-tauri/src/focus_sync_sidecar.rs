// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! Focus Sync 侧车进程的生命周期管理。
//!
//! 背景（2026-11-09）：Focus Sync 早期靠 Windows 计划任务
//! `KizemoFocusSync`（AtLogOn + `--service`）常驻，与 Alpha File Manager
//! 的开关状态完全脱钩 —— 用户装了扩展就有一个脱离应用的生命周期。
//!
//! 现改为：**应用启动时拉起，应用退出时杀掉**。
//!
//! 选主进程而不是扩展侧拉起的理由（实测）：扩展的 `activate()` 在本项目的
//! 沙箱 Web Worker 里存在不可观测的断点（`__focus_sync_trace` 里连
//! `N01.activate.start` 都没有），而 `setup_handler` 是确定性入口。
//!
//! 三层兜底：
//! 1. 应用退出（`RunEvent::Exit`）→ `stop()` 主动 kill；
//! 2. 应用被强杀 / 崩溃 → 侧车自带父进程看门狗（`spawn_parent_watchdog`，
//!    仅在不带 `--service` 时启用）在父进程句柄 signaled 后自杀；
//! 3. 侧车二进制不存在（未安装 Focus Sync 扩展）→ 只记日志，不报错。
//!
//! 注意：**不要**给侧车传 `--service` —— 该 flag 会关掉看门狗。
//! 保留它只是为了兼容历史部署（legacy 计划任务）。

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::Manager;

const SIDECAR_FILE_NAME: &str = "focus-sync-sidecar.exe";
/// 与扩展侧 `dist/index.js`、侧车默认值保持一致的固定端口。
const SIDECAR_PORT: u16 = 37421;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 由 `setup_handler` 注册进 Tauri 状态表，供退出分支取用。
#[derive(Default)]
pub struct FocusSyncSidecarState {
    child: Mutex<Option<Child>>,
}

/// 解析侧车可执行文件路径。
///
/// 优先级：
/// 1. `<安装目录>\tools\focus-sync-sidecar.exe`
///    —— 安装器从 `extensions/kizemo.focus-sync/bin/` 拷入，`bundle.resources` 映射；
/// 2. `%APPDATA%\<identifier>\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\`
///    —— 开发态 / 扩展自行部署时的位置。
fn resolve_sidecar_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("tools").join(SIDECAR_FILE_NAME));
        }
    }

    if let Ok(data_dir) = app.path().app_data_dir() {
        candidates.push(
            data_dir
                .join("extensions")
                .join("kizemo.focus-sync")
                .join("bin")
                .join("focus-sync-sidecar")
                .join(SIDECAR_FILE_NAME),
        );
    }

    candidates.into_iter().find(|path| path.is_file())
}

/// 在 `setup_handler` 里调用：拉起侧车并把 `Child` 存进应用状态。
pub fn start(app: &tauri::AppHandle) {
    let state = FocusSyncSidecarState::default();

    let Some(path) = resolve_sidecar_path(app) else {
        log::info!("focus-sync sidecar: binary not found, skip launch");
        app.manage(state);
        return;
    };

    let mut command = Command::new(&path);
    command
        .arg("--port")
        .arg(SIDECAR_PORT.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    match command.spawn() {
        Ok(child) => {
            log::info!(
                "focus-sync sidecar: launched pid={} port={} exe={}",
                child.id(),
                SIDECAR_PORT,
                path.display()
            );
            if let Ok(mut guard) = state.child.lock() {
                *guard = Some(child);
            }
        }
        Err(error) => {
            log::warn!(
                "focus-sync sidecar: failed to launch {}: {error}",
                path.display()
            );
        }
    }

    app.manage(state);
}

/// 在 `RunEvent::Exit` 里调用：杀掉侧车。
///
/// 只在真正的 `Exit` 上调用 —— `ExitRequested` 可能被托盘逻辑取消。
/// 应用被强杀时走侧车自带的父进程看门狗，不依赖这里。
pub fn stop(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<FocusSyncSidecarState>() else {
        return;
    };
    let Ok(mut guard) = state.child.lock() else {
        return;
    };
    let Some(mut child) = guard.take() else {
        return;
    };

    let pid = child.id();
    match child.kill() {
        Ok(()) => log::info!("focus-sync sidecar: killed pid={pid}"),
        Err(error) => log::warn!("focus-sync sidecar: kill pid={pid} failed: {error}"),
    }
    let _ = child.wait();
}