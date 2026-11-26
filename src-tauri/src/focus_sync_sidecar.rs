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
//!
//! 另有一处**只读**的启动握探针（批次 63，见 `spawn_health_probe`）：
//! 侧车起来后探一次 `GET /health`，把侧车的 `version`/`mode`/扩展存活状态写进
//! 主日志，并与扩展自己 `package.json` 里的版本比对。它**只写日志**，
//! 既不改变监督器行为，也不因为版本不一致拒绝启动。

use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;

use crate::focus_sync_health;
use crate::focus_sync_supervisor::{Launcher, Supervisor};

const SIDECAR_FILE_NAME: &str = "focus-sync-sidecar.exe";
/// 与扩展侧 `dist/index.js`、侧车默认值保持一致的固定端口。
const SIDECAR_PORT: u16 = 37421;
/// 扩展自己的清单文件（VS Code 式扩展格式，`version` 就在顶层）。
const EXTENSION_PACKAGE_FILE_NAME: &str = "package.json";

/// 握手探测的总尝试次数 × 间隔 ≈ 6s：侧车从 spawn 到 listen 通常 < 1s，
/// 余量用来覆盖「端口刚被上一次进程占用、侧车正在重试」这类慢启动。
const HEALTH_PROBE_ATTEMPTS: u32 = 15;
const HEALTH_PROBE_INTERVAL: Duration = Duration::from_millis(400);
/// 单次请求超时。本地回环，2s 已经非常宽松。
const HEALTH_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

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

    // 侧车刚 spawn，端口通常还没 listen 完成 —— 放到后台等，不阻塞 setup。
    spawn_health_probe(resolve_extension_version(app));
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

/// 读扩展自己 `package.json` 里的版本，用来和侧车 `/health` 报的版本比对。
///
/// 读不到就返回 `None`（老装机可能没有这个文件）—— 调用方据此静默跳过比对，
/// 不制造假告警。
fn resolve_extension_version(app: &tauri::AppHandle) -> Option<String> {
    let data_dir = crate::app_dirs::app_data_dir(app).ok()?;
    let manifest = data_dir
        .join("extensions")
        .join("kizemo.focus-sync")
        .join(EXTENSION_PACKAGE_FILE_NAME);
    let raw = std::fs::read_to_string(manifest).ok()?;
    focus_sync_health::version_from_package_json(&raw)
}

/// 探一次 `GET /health`，把侧车版本/模式与扩展版本一起写进主日志。
///
/// 为什么要这个探针（批次 62 的教训）：扩展报「未连接」时，主日志里只有
/// `focus-sync sidecar: launched pid=...`，分不清是
/// ①侧车没起来 ②起来的是别的版本（装了扩展但侧车还是旧的）
/// ③侧车正常、扩展从没 push 过。三者的处置完全不同，而这三条信息
/// `/health` 里全都有 —— 以前只能另起探针手工 curl 才看得到。
///
/// ⚠️ 纯可观测性：不重试拉起、不 kill、不影响监督器的重启计数。
fn spawn_health_probe(extension_version: Option<String>) {
    tauri::async_runtime::spawn(async move {
        let Ok(client) = reqwest::Client::builder()
            .timeout(HEALTH_PROBE_TIMEOUT)
            // 127.0.0.1 绝不能走系统代理：开了代理的机器会把回环请求送出去，
            // 拿回一个代理错误页，比探针失败更容易误导排查。
            .no_proxy()
            .build()
        else {
            log::warn!("focus-sync sidecar: health probe skipped (cannot build http client)");
            return;
        };

        let url = format!("http://127.0.0.1:{SIDECAR_PORT}/health");
        let mut last_error = String::from("no attempt made");

        for attempt in 1..=HEALTH_PROBE_ATTEMPTS {
            match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    if !status.is_success() {
                        log::warn!(
                            "focus-sync sidecar: health probe got status={status} url={url} \
                             body={}",
                            focus_sync_health::snippet(&body)
                        );
                        return;
                    }
                    match focus_sync_health::parse_health(&body) {
                        Ok(snapshot) => {
                            log::info!(
                                "focus-sync sidecar: health {}",
                                focus_sync_health::describe(&snapshot)
                            );
                            if let Some(skew) = focus_sync_health::version_skew(
                                &snapshot.version,
                                extension_version.as_deref(),
                            ) {
                                log::warn!("focus-sync sidecar: {skew}");
                            }
                            return;
                        }
                        Err(error) => {
                            // 侧车答了但不是我们认得的格式：重试也没意义，直接记下来。
                            log::warn!(
                                "focus-sync sidecar: health body unreadable ({error}) url={url} \
                                 body={}",
                                focus_sync_health::snippet(&body)
                            );
                            return;
                        }
                    }
                }
                Err(error) => {
                    last_error = error.to_string();
                    if attempt < HEALTH_PROBE_ATTEMPTS {
                        tokio::time::sleep(HEALTH_PROBE_INTERVAL).await;
                    }
                }
            }
        }

        log::warn!(
            "focus-sync sidecar: health probe gave up after {HEALTH_PROBE_ATTEMPTS} attempts \
             url={url} last_error={last_error}"
        );
    });
}