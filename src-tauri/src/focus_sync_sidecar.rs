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
//! 四层兜底：
//! 1. 应用退出（`RunEvent::Exit`）→ `stop()` 停止监督并主动 kill；
//! 2. 侧车自己退出（panic / 端口被占 / 被杀）→ 主进程的监督线程重新拉起
//!    （`focus_sync_supervisor`，上限对齐 legacy 计划任务的 `-RestartCount 3`，
//!    避免改造后丢掉计划任务原本带来的自愈能力）；
//! 3. 应用被强杀 / 崩溃 → 侧车自带父进程看门狗（`spawn_parent_watchdog`，
//!    仅在不带 `--service` 时启用）在父进程句柄 signaled 后自杀；
//! 4. 侧车二进制不存在（未安装 Focus Sync 扩展）→ 只记日志，不报错。
//!
//! 注意：**不要**给侧车传 `--service` —— 该 flag 会关掉看门狗。
//! 保留它只是为了兼容历史部署（legacy 计划任务）。

use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tauri::Manager;

use crate::focus_sync_supervisor::{Launcher, Supervisor};

const SIDECAR_FILE_NAME: &str = "focus-sync-sidecar.exe";
/// 与扩展侧 `dist/index.js`、侧车默认值保持一致的固定端口。
const SIDECAR_PORT: u16 = 37421;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 解析侧车可执行文件路径。
///
/// 优先级：
/// 1. `%APPDATA%\<identifier>\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\`
///    —— 扩展自己的部署目录（独立安装器 / 应用内安装扩展都落在这里）。
///    **这一份优先**：它与扩展本体（`dist/`）是同一次安装、同一版本，扩展升级时
///    也会一起更新；若让 AFM 自带的 `tools\` 副本优先，就会出现
///    「扩展已是新版、侧车还是旧版」的错配。
/// 2. `<安装目录>\tools\focus-sync-sidecar.exe`
///    —— AFM 主安装器随包带的那份（`bundle.resources` 映射），作为兜底。
fn resolve_sidecar_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 走 app_dirs 的统一解析点：直接调 Tauri 自带的 app 数据目录解析器会绕过
    // e2e 数据隔离（debug 构建下的 SFM_E2E_DATA_DIR 覆盖），
    // 让隔离实例读到用户真实的 app 数据目录。
    if let Ok(data_dir) = crate::app_dirs::app_data_dir(app) {
        candidates.push(
            data_dir
                .join("extensions")
                .join("kizemo.focus-sync")
                .join("bin")
                .join("focus-sync-sidecar")
                .join(SIDECAR_FILE_NAME),
        );
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("tools").join(SIDECAR_FILE_NAME));
        }
    }

    candidates.into_iter().find(|path| path.is_file())
}

/// 在 `setup_handler` 里调用：拉起侧车并把监督器存进应用状态。
pub fn start(app: &tauri::AppHandle) {
    let Some(path) = resolve_sidecar_path(app) else {
        log::info!("focus-sync sidecar: binary not found, skip launch");
        return;
    };

    log::info!(
        "focus-sync sidecar: resolving exe={} port={}",
        path.display(),
        SIDECAR_PORT
    );

    let port = SIDECAR_PORT;
    let launcher: Launcher = Arc::new(move || -> io::Result<Child> {
        let mut command = Command::new(&path);
        command
            .arg("--port")
            .arg(port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        command.spawn()
    });

    let supervisor = Supervisor::new(launcher);
    supervisor.start();
    app.manage(supervisor);
}

/// 在 `RunEvent::Exit` 里调用：停止监督并杀掉侧车。
///
/// 只在真正的 `Exit` 上调用 —— `ExitRequested` 可能被托盘逻辑取消。
/// 应用被强杀时走侧车自带的父进程看门狗，不依赖这里。
pub fn stop(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<Supervisor>() else {
        return;
    };
    state.stop();
}