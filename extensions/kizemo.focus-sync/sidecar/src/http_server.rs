//! HTTP/1.1 server: replaces raw TCP for Sigma FM extension IPC.
//!
//! Endpoints (per extension `release/extension/dist/index.js` contract):
//! - `POST /set_path` body `{"path":"D:\\downloads"}` → 200 `{"ok":true}`
//!                                                or 400 `{"ok":false,"error":"..."}`
//! - `POST /quit`                            → 200 `{"ok":true}` then `std::process::exit(0)`
//! - `GET  /get_status`                      → 200 `{"ok":true,"current_path":"...","active_dialogs":N}`
//! - `GET  /health`                          → 200 health snapshot (v0.3.0 Scheduled Task mode)
//!
//! Protocol context:
//! - Sigma FM extension speaks HTTP (uses `sigma.http.request`) — see
//!   `handoff-2026-09-30-fix-ipc-http.md §一` for the mismatch root cause.
//! - tiny_http is chosen for sync I/O + zero async-runtime overhead (the rest of
//!   the spike already runs tokio, but http_server is wrapped in
//!   `std::thread::spawn` from main — see `main.rs`).
//!
//! Reuse: `state::AppState`, `events::Event`, `logger::log_event`, `writer::write_path`.
//! No business logic changes from the round-18 TCP version — only the transport layer.

use crate::events::Event;
use crate::logger::{log_event, now_iso8601};
use crate::state::{AppState, DialogInfo};
use crate::writer::WriteOutcome;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::Arc;
use tiny_http::{Header, Method, Response, Server, StatusCode};
// v0.5.7 F-1: used ONLY for the foreground self-check log (the decision itself
// reads the monitor-published shared value). See set_path_deferred_not_foreground.
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

// v0.5.5: 41477 exclusion + first_push_received flag + /health expose +
// should_opened_write_back helper. See:
// docs/superpowers/plans/2026-10-06-focus-21-v0.5.5-ship-ready-plan-v6.md
//
// v0.5.8 (2026-10-08): bumped so the running build is IDENTIFIABLE from
// /health. It had been stuck at "0.5.5" through v0.5.6, v0.5.7 and the
// v0.5.7-verified rollback node, so a live sidecar could not be told apart from
// any other live sidecar by anything it reported. That ambiguity has been
// assumed away more than once in this project's history; a one-line version
// that is actually maintained is the cheapest fix available.
//
// v0.5.8 carries: F3 filename-box restore + read-back (BUG-2), H2 read-back
// verification (the "假成功" limitation), and the `dialog_gone` dead-HWND path.
// v0.5.12: a refused write no longer marks the dialog as written-back, so the
// v0.5.7 G-1 resync channel is reachable again and a dialog opened inside the
// startup window self-heals instead of requiring close-and-reopen.
// v0.5.13: the READER was fixed. `reader.rs` used to return the first
// UIA_EditControlTypeId it found, which on a classic shell dialog is a per-file
// property cell inside the file list (`UIProperty` / `System.ItemNameDisplay`)
// — the name of the first visible item, never a directory. Across 931 logged
// reads only 12 returned anything dir-shaped, so `initial_path != current_path`
// held permanently, `should_opened_write_back` was permanently true, and one
// dialog wrote the SAME target 29 times in 31 seconds (hwnd=722614). It now
// reads the address bar's Name (`aid=1001`, "地址: <path>") with a shape guard.
// v0.5.14: the WRITER is fixed — path following now actually moves the dialog.
// H1 was gated behind `is_winui3_wrapped`, which requires a `FileNameControlHost`
// marker that a classic `#32770` dialog never publishes; all 1265 dialogs ever
// logged are `#32770`, so the gate was constant-false and H1 never ran. Every
// write therefore landed on F3, which writes the FILENAME box and immediately
// takes it back (`restore_filename_verified`) without sending Enter — measured
// DID NOT NAVIGATE, while still logging `Write strategy=uia`. H1 is now tried
// for every dialog: it targets the ADDRESS bar, read-backs before Enter, never
// steals foreground, and never invokes the Save button. The v0.5.1 incident that
// caused H1 to be demoted was `commit_save_button`, which is still absent.
// v0.5.16: the READER could not parse a localized special-folder label.
// The Windows "Save As" dialog parks on the local Downloads folder, whose
// address bar Name is a localized DISPLAY name ("地址: 下载"), not a path.
// `reader::parse_address_label` only accepts path-shaped values, so it returned
// Err(NoDirectoryFound); `main.rs` then left `initial_path` empty and the
// `else if !initial_path.is_empty()` guard SKIPPED the write-back silently.
// Two independent layers now fix it:
//   (1) exact: `reader::read_path_with_strategy` falls back to
//       `IFileDialog::GetFolder` + `IShellItem::GetDisplayName(SIGDN_FILESYSPATH)`
//       via COM, which yields a language-independent real path;
//   (2) graceful: `state::should_write_back_when_path_unknown` writes anyway
//       and de-duplicates against the last path written to that hwnd, so the
//       v0.5.13 no-flicker invariant survives a failed read.
// Path comparisons are normalized (separator, trailing separator, case) so the
// reader's forward slashes no longer look different from AFM's backslashes.
// v0.5.17 (2026-11-06) — the write-back is now VERIFIED instead of assumed.
//
// 真机实测：一个真实的经典 shell「另存为」对话框停在
// `C:\Users\Duanyi\Downloads` 时，侧车记了
// `Write target=... strategy=uia`，而地址栏仍然停在 Downloads。
// 根因：`IFileDialog::SetFolder` 对**已打开**的对话框返回 S_OK 却完全不
// 改当前目录（它只设置 Show() 之前的初始文件夹），旧代码在这里直接判定
// 成功并跳过了 UIA 地址栏兜底 ⇒ 日志说成功、界面纹丝不动。
//
// 本版两处改动：
//   1. `try_set_folder_via_com`：SetFolder 之后必须回读对话框当前目录，
//      等于目标路径才算成功，否则继续走 UIA 地址栏路径；
//   2. `writer::write_path`：所有成功通道统一做一次写后回读校验，
//      不通过则降级为 `write_not_verified`，让对话框保持可重试
//      （沿用 v0.5.12 语义），且**不**误报「不支持的对话框类型」。
// v0.5.18 (2026-11-06) — the COM channel was dead on every thread that
// actually uses it.
//
// 真机实测：对着一个真实的原生「另存为」对话框调用生产代码
// `read_current_folder_via_com`，返回 NONE。根因不是「对话框不是原生
// IFileDialog」，而是**调用线程从未初始化 COM**：侧车的 COM 调用发生在
// tokio worker 线程和 dialog monitor 线程上，这两个线程都没跑过
// `CoInitializeEx`，于是第一步 `AccessibleObjectFromWindow` 就以
// CO_E_NOTINITIALIZED 失败 —— 而那一步**连日志都没有**，于是整个 COM
// 通道长期处于「静默失效」状态。
//
// 本版改动：
//   1. 新增 `ComGuard`：进入 COM 通道前在**当前线程**做
//      `CoInitializeEx(COINIT_APARTMENTTHREADED)`，离开时按引用计数
//      平衡 `CoUninitialize`（RPC_E_CHANGED_MODE 时不反初始化）；
//   2. 给 `AccessibleObjectFromWindow` 失败补上明确的 warn 日志 ——
//      「静默失败」比「崩溃」更难查，这是本轮定位耗时的主因。
// v0.5.19 (2026-11-09) — 两条独立的修复：
//
//   1. P1「虚拟路径外发」：百度网盘远程目录（`sfm://remote/<id>/<dir>`）
//      以前会被扩展无条件剥掉 `sfm:/` 前缀，变成伪路径
//      `\remote\<id>\0000temp`，再被写进别的程序的「打开」对话框，
//      Windows 弹「找不到」错误框；错误框又被 monitor 当成新对话框，
//      于是每 1-2 秒反复报错。现在扩展侧保留虚拟路径原样，sidecar 侧
//      在**两个写站点**都先校验目标是真实存在的本地目录
//      （`state::is_writable_local_path`），不合法就跳过并记日志。
//
//   2. P2「只上弦不下弦的旧闩」：v0.5.15 的 `afm_visit_latch` 一旦被上弦
//      就永远不失效，于是 22 分钟前的一次 AFM -> 其它应用的切换，
//      依然会让后续的对话框写入路径（真机日志实证）。现在换成
//      **交接闸门** `state::afm_handoff`：记录「用户从 AFM 切走的那一刻」
//      与「接手前台的进程 pid」，只有 (a) 确实发生过交接、(b) 未超 TTL、
//      (c) 弹窗属于那个接手的进程，三条同时成立才允许写入。
//
// 环境变量逃生阀：FOCUS_SYNC_AFM_GATE=0 让闸门恒开（回到旧行为，便于 A/B）。
// FOCUS_SYNC_AFM_HANDOFF_TTL_MS 调整交接时效（默认 120000ms）。
const SPIKE_VERSION: &str = "0.6.0";

#[derive(Debug, Deserialize)]
struct SetPathBody {
    path: String,
    /// focus-18: from extension PUSH body. Default false (serde default).
    /// true means the extension user enabled Auto-Confirm, so the sidecar
    /// may Invoke Save (per-dialog single-shot via state.try_mark_committed).
    #[serde(default)]
    auto_confirm: bool,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ResponseBody {
    Ok {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Status {
        ok: bool,
        current_path: String,
        active_dialogs: usize,
    },
    Health(HealthResponse),
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    /// `"ok"` / `"degraded"` / `"down"`.
    /// - "ok":       process alive, port bound, UIA healthy
    /// - "degraded": process alive, but UIA self-check failed OR session_id = 0
    /// - "down":     unused (we'd return 5xx instead)
    status: &'static str,
    uptime_secs: u64,
    last_ping_received_ts: u64,
    last_dialog_event_ts: u64,
    active_dialogs: usize,
    uia_healthy: bool,
    session_id: u32,
    version: &'static str,
    /// `"service"` when launched via Scheduled Task with `--service` flag,
    /// `"spawn"` when launched by Sigma FM (v0.2.0 backward-compat).
    mode: &'static str,
    /// focus-19 (2026-10-02): monotonic counter of dialogs the sidecar
    /// could NOT navigate (WinUI Save As / unknown). The extension polls
    /// this and surfaces a notification when the count increases since the
    /// last check (deep-analysis §4.2 Option B + §8 task 5).
    unsupported_dialog_count: u64,
    /// v0.5.5 (Mavis Round 21 B-4): boolean exposure for extension UX.
    /// Extension reads this to detect "sidecar is waiting for PUSH" and
    /// show user a "Sync Now?" notification.
    first_push_received: bool,
    /// v0.5.8: Unix millis of the LAST `GET /health` from the extension (0 if
    /// never).
    ///
    /// Exposed because the stale-path gate depends on it and the gate
    /// previously had NO observable output: when writes were being refused (or
    /// wrongly allowed) there was nothing to look at except "a Write happened"
    /// or "a Write did not happen". A gate you cannot read is indistinguishable
    /// from a gate that is not wired up. This value makes the gate's input
    /// visible from outside the process.
    last_health_poll_ts: u64,
    /// v0.5.8: ms since that poll, or -1 if the extension has never polled.
    last_health_poll_age_ms: i64,
    /// v0.5.8: the gate's verdict. Writes are refused unless this is true.
    extension_alive: bool,
    /// v0.5.15: AFM 访问闩状态。true = 用户访问过 AFM 窗口且这份授权还没被消耗,
    /// 下一次弹窗写回放行。真机验收靠它区分「闩没上弦」和「闸门被关掉」。
    afm_visit_latched: bool,
}

/// Blocking HTTP server loop. Returns when the server is dropped (e.g. after
/// `/quit` triggers `std::process::exit(0)`).
///
/// Tiny_http's `Server::http` binds `127.0.0.1:port` and `incoming_requests()`
/// yields requests sequentially. We process them inline — local IPC is single-
/// threaded by design (Sigma FM extension is the only consumer).
///
/// `mode` is the v0.3.0 mode label (`"service"` or `"spawn"`) included in /health.
pub fn run_server(state: Arc<AppState>, port: u16, mode: &'static str) -> Result<()> {
    let server = Server::http(("127.0.0.1", port)).map_err(anyhow::Error::msg)?;
    tracing::info!(port, "HTTP server listening");

    for request in server.incoming_requests() {
        let state = state.clone();
        let method = request.method().clone();
        let url = request.url().to_string();

        match (method, url.as_str()) {
            (Method::Post, "/set_path") => {
                if let Err(e) = handle_set_path(request, state) {
                    tracing::warn!(error = %e, "set_path handler error");
                }
            }
            (Method::Post, "/quit") => {
                if let Err(e) = handle_quit(request) {
                    tracing::warn!(error = %e, "quit handler error");
                }
                // handle_quit returns 200 then exits — never reached in normal flow.
            }
            (Method::Get, "/get_status") => {
                if let Err(e) = handle_get_status(request, state) {
                    tracing::warn!(error = %e, "get_status handler error");
                }
            }
            (Method::Get, "/health") => {
                if let Err(e) = handle_health(request, state, mode) {
                    tracing::warn!(error = %e, "health handler error");
                }
            }
            // v0.5.8: extension liveness ping.
            //
            // DELIBERATELY NOT `/health`. `/health` is a shared diagnostic
            // endpoint: `scripts\read-sidecar-log.ps1`, ad-hoc
            // `Invoke-WebRequest` and every debugging session hit it. Stamping
            // liveness there meant a single diagnostic read kept the
            // stale-path gate open — proven 2026-10-08 by watching
            // `last_health_poll_age_ms` track the observer's own polling.
            //
            // This endpoint is called ONLY by the extension's periodic health
            // check, so a stamp here genuinely means "Sigma FM's extension is
            // alive", which is what the gate actually needs to know.
            (Method::Get, "/ext_alive") => {
                if let Err(e) = handle_ext_alive(request, state) {
                    tracing::warn!(error = %e, "ext_alive handler error");
                }
            }
            _ => {
                let _ = request.respond(
                    Response::from_string("not found")
                        .with_status_code(StatusCode(404)),
                );
            }
        }
    }
    Ok(())
}

// ---- handlers ----

fn handle_set_path(
    mut request: tiny_http::Request,
    state: Arc<AppState>,
) -> Result<()> {
    // Read body (tiny_http limits by default to 8 MB; we cap smaller for safety).
    let mut body = String::new();
    request.as_reader().take(64 * 1024).read_to_string(&mut body)?;

    let parsed: Result<SetPathBody, _> = serde_json::from_str(&body);
    let parsed_body = match parsed {
        Ok(p) => p,
        Err(e) => {
            let resp = ResponseBody::Ok {
                ok: false,
                error: Some(format!("invalid JSON body: {e}")),
            };
            return respond_json(request, 400, &resp);
        }
    };
    let path = parsed_body.path;
    let auto_confirm = parsed_body.auto_confirm;

    if path.is_empty() {
        let resp = ResponseBody::Ok {
            ok: false,
            error: Some("path is empty".into()),
        };
        return respond_json(request, 400, &resp);
    }

    // D6 [MEDIUM] (Mavis 第 13 轮): PUSH dedup — if the extension pushes the
    // same path twice in a row, skip the per-input dialog write loop. This
    // matters most for the v0.5.1 SendInput path: each SendInput call
    // dispatches Ctrl+L, the path, and Enter into the dialog, which
    // would destroy any filename the user is currently typing if the
    // Sigma FM focus flickers (e.g. user re-clicks the same folder).
    //
    // We compare against the previous `current_path` BEFORE updating
    // state, so identical-push PUSHes short-circuit cleanly. The new
    // current_path is still set below so subsequent /get_status reflects
    // the latest call.
    let prev_path = state.get_current_path();
    let path_unchanged = prev_path == path;
    if path_unchanged {
        // v0.5.5 (Mavis Round 21 B-1): even if path didn't change, this
        // counts as a push — mark first_push_received explicitly. The
        // set_current_path below won't be called in this branch, so we
        // MUST mark here. Without this, user pushing C:\\ (== sentinel)
        // leaves flag = false, and opened_write_back will skip silently.
        state.mark_first_push_received();

        log_event(&Event::SpikeError {
            kind: "set_path_dedup".into(),
            message: format!("path={path} (unchanged, skipping dialog write loop)"),
            ts: now_iso8601(),
        });
        let resp = ResponseBody::Ok { ok: true, error: None };
        return respond_json(request, 200, &resp);
    }

    // Update state.
    state.set_current_path(path.clone());
    log_event(&Event::SpikeError {
        kind: "path_update".into(),
        message: format!("current_path={}", path),
        ts: now_iso8601(),
    });

    // Snapshot active dialogs, then write to each.
    // Important: release the lock before calling write_path.
    //
    // Round 19e (2026-09-30): The dialog registry is updated asynchronously via
    // main loop's DialogEvent::Opened handler. There is a race window where
    // /set_path runs BEFORE register_dialog has processed the new dialog. The
    // extension's follow-up /get_status confirms the dialog is now registered
    // (so it's not gone — just not yet visible to /set_path). Retry briefly so
    // the write attempt isn't silently dropped on the early PUSHes.
    let mut dialogs: Vec<DialogInfo> = state.active_dialogs();
    let mut retry_iter = 0u32;
    while dialogs.is_empty() && retry_iter < 8 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        dialogs = state.active_dialogs();
        retry_iter += 1;
    }
    if retry_iter > 0 {
        log_event(&Event::SpikeError {
            kind: "set_path_dialogs_retry".into(),
            message: format!("retried {retry_iter}x; final dialogs_count={}", dialogs.len()),
            ts: now_iso8601(),
        });
    }
    log_event(&Event::SpikeError {
        kind: "set_path_dialogs".into(),
        message: format!("dialogs_count={}", dialogs.len()),
        ts: now_iso8601(),
    });
    for d in dialogs {
        // v0.5.8 dead-HWND pruning — THIS is the gap that made the earlier
        // `dialog_gone` fix ineffective.
        //
        // The registry deliberately keeps dialogs after `DialogEvent::Closed`
        // (round 19c) so a foreground flicker cannot lose the write target.
        // The cost is that destroyed windows stay forever. My first attempt
        // put the liveness check inside `inject_folder_path`, but the two
        // `continue` statements BELOW (DEDUP skip, R-2 "not foreground" defer)
        // both fire first, so a dead handle never reached it: `active_dialogs`
        // was observed climbing to 3 while zero dialogs were open.
        //
        // Pruning has to happen BEFORE those skips, which is why it lives here
        // rather than deeper in the injection path.
        if !crate::uia_inject::dialog_window_exists(d.hwnd) {
            state.remove_dialog(d.hwnd);
            tracing::info!(
                "v0.5.8 set_path: pruning dead dialog hwnd={} (app={}) from registry",
                d.hwnd,
                d.app
            );
            continue;
        }
        // v0.5.19 — 写进别人的对话框之前，两道闸门：
        //
        //   1. **路径合法性**：只有真实存在的本地目录才允许写。AFM 停在百度
        //      网盘远程目录时推来的是虚拟路径，写进去只会换来对方的
        //      「找不到该文件」错误框，而那个错误框又会被当成新对话框重试
        //      ⇒ 关闭后仍反复报错（真机 2026-11-09 11:31）。
        //   2. **交接闸门**：v0.5.15 的注释说这一路不加闸（「即时写要求弹窗
        //      前台 => AFM 必非前台」）。那个推理有个洞：侧车重启后扩展的
        //      补推（first_push_received 被复位）是**陈旧**的，会把旧路径写进
        //      用户此刻正在看的对话框。交接闸门堵住这个洞。
        if !crate::state::is_writable_local_path(&path) {
            log_event(&Event::SpikeError {
                kind: "set_path_skip_non_local_path".into(),
                message: format!(
                    "hwnd={} app={} path='{}' (not a writable local directory); skipping",
                    d.hwnd, d.app, path
                ),
                ts: now_iso8601(),
            });
            continue;
        }
        let dialog_pid = crate::uia_event::window_pid_isize(d.hwnd as isize);
        if !state.afm_handoff_allows(dialog_pid) {
            log_event(&Event::SpikeError {
                kind: "set_path_skip_no_handoff".into(),
                message: format!(
                    "hwnd={} app={} path='{}' (no direct AFM -> dialog handoff); skipping",
                    d.hwnd, d.app, path
                ),
                ts: now_iso8601(),
            });
            continue;
        }
        // v0.5.7 DEDUP-1: skip a dialog that already holds this EXACT path.
        //
        // Observed 2026-10-08 14:02: the Opened write-back injected once, then
        // ~2.3s later /set_path injected the SAME path into the SAME dialog
        // again. R-2 let it through because the dialog was foreground, and the
        // existing `path_unchanged` dedup only compares against
        // `state.current_path` - not against "what we already wrote HERE".
        // The user saw the address bar cleared and refilled twice.
        if state.last_written_path(d.hwnd).as_deref() == Some(path.as_str()) {
            log_event(&Event::SpikeError {
                kind: "set_path_already_synced".into(),
                message: format!(
                    "hwnd={} app={} already holds this path; skipping redundant inject",
                    d.hwnd, d.app
                ),
                ts: now_iso8601(),
            });
            continue;
        }

        // v0.5.7 R-2 (2026-10-07, user feedback): /set_path must NOT touch a
        // BACKGROUND dialog.
        //
        // The user observed that navigating in Sigma FM yanks focus back to the
        // download dialog. Cause: this loop wrote to every registered dialog
        // regardless of focus, and the UIA/SendInput path took foreground.
        //
        // New contract:
        //   - Sigma FM pushes a path  -> we STORE it only.
        //   - User brings the dialog forward -> uia_event's monitor (which only
        //     detects FOREGROUND dialogs) emits Opened -> opened_write_back
        //     performs the actual navigation.
        // So a background dialog is simply skipped here; the pending write is
        // picked up the moment the user focuses it.
        // v0.5.7 F-1: read the foreground answer published by the MONITOR
        // thread. Do NOT call GetForegroundWindow() here — this thread has no
        // Windows message pump and its answer disagreed with the monitor's,
        // which deferred every push (2026-10-07 16:39 incident).
        //
        // The local call is made ONLY as a self-check and logged, so that any
        // future disagreement is observable rather than silent.
        let fg_shared = state.foreground_dialog();
        let fg_local = unsafe {
            let h = GetForegroundWindow();
            if h.0.is_null() {
                0
            } else {
                h.0 as u32
            }
        };
        if fg_shared != d.hwnd {
            log_event(&Event::SpikeError {
                kind: "set_path_deferred_not_foreground".into(),
                message: format!(
                    "hwnd={} app={} shared_fg={fg_shared} local_fg={fg_local} \
                     AGREE={}; path stored, will sync when the user focuses the dialog",
                    d.hwnd,
                    d.app,
                    fg_shared == fg_local
                ),
                ts: now_iso8601(),
            });
            if fg_shared != fg_local {
                tracing::warn!(
                    shared_fg = fg_shared,
                    local_fg = fg_local,
                    "v0.5.7 F-1 SELF-CHECK: shared and local foreground DISAGREE \
                     (expected: local is unreliable on this thread)"
                );
            }
            continue;
        }
        log_event(&Event::SpikeError {
            kind: "set_path_iter".into(),
            message: format!(
                "hwnd={} app={} auto_confirm={} (shared_fg matched)",
                d.hwnd, d.app, auto_confirm
            ),
            ts: now_iso8601(),
        });
        let outcome = crate::writer::write_path(&state, d.hwnd as isize, &path, auto_confirm);
        match outcome {
            WriteOutcome::UiaSetValue => {
                // v0.5.15: 路径真的写进弹窗了 -> 消耗 AFM 访问闩。/set_path
                // 这一路**不加闸**(即时写要求弹窗前台 => AFM 必非前台, 加闸会让
                // 下弦没有可靠时机), 只负责在成功后下弦。
                state.clear_afm_handoff();
                log_event(&Event::Write {
                    hwnd: d.hwnd,
                    app: d.app.clone(),
                    target: path.clone(),
                    strategy: "uia".into(),
                    ts: now_iso8601(),
                });
            }
            WriteOutcome::SendInputFallback => {
                // v0.5.15: 同上, 兜底策略写成功也下弦。
                state.clear_afm_handoff();
                log_event(&Event::Write {
                    hwnd: d.hwnd,
                    app: d.app.clone(),
                    target: path.clone(),
                    strategy: "sendinput".into(),
                    ts: now_iso8601(),
                });
            }
            WriteOutcome::Failed(reason) => {
                log_event(&Event::WriteFailed {
                    hwnd: d.hwnd,
                    app: d.app.clone(),
                    target: path.clone(),
                    reason,
                    ts: now_iso8601(),
                });
            }
        }
    }

    let resp = ResponseBody::Ok { ok: true, error: None };
    respond_json(request, 200, &resp)
}

fn handle_quit(request: tiny_http::Request) -> Result<()> {
    let resp = ResponseBody::Ok { ok: true, error: None };
    respond_json(request, 200, &resp)?;
    // Process exit — Sigma FM extension's deactivate() expects graceful shutdown.
    std::process::exit(0);
}

fn handle_get_status(
    request: tiny_http::Request,
    state: Arc<AppState>,
) -> Result<()> {
    let resp = ResponseBody::Status {
        ok: true,
        current_path: state.get_current_path(),
        active_dialogs: state.active_dialogs().len(),
    };
    respond_json(request, 200, &resp)
}

/// v0.3.0 health endpoint. Returns process state snapshot for extension
/// monitoring and user-facing diagnostics.
///
/// Status semantics (v3 review V3.17):
/// - "ok"       — process alive, port bound, UIA healthy, session != 0
/// - "degraded" — process alive but UIA self-check failed 3x OR session_id = 0
///                (RDP / SSH / CI runner scenarios; v3 review V3.1 — not panic)
/// - "down"     — unused; would require process to be in bad state, but we just
///                respond with whatever we have. Future: detect self-destruct.
fn handle_health(
    request: tiny_http::Request,
    state: Arc<AppState>,
    mode: &'static str,
) -> Result<()> {
    let snap = state.health_snapshot();
    let window = crate::writer::EXTENSION_ALIVE_WINDOW_MS;

    // v0.5.8: READ the gate's state, never stamp it. Stamping moved to
    // /ext_alive precisely because this endpoint is shared with diagnostics.
    // Reading it here (rather than after a stamp) is what makes the reported
    // value trustworthy — measuring immediately after writing made it
    // constant true, which hid the gate entirely.
    let last_poll = state.last_extension_ping_ts();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let age_ms: i64 = if last_poll == 0 {
        -1
    } else {
        now_ms as i64 - last_poll as i64
    };
    let alive_now = state.extension_alive(window);
    let status = if snap.uia_healthy && snap.session_id != 0 {
        "ok"
    } else {
        "degraded"
    };
    let body = HealthResponse {
        status,
        uptime_secs: snap.uptime_secs,
        last_ping_received_ts: snap.last_ping_received_ts,
        last_dialog_event_ts: snap.last_dialog_event_ts,
        active_dialogs: snap.active_dialogs,
        uia_healthy: snap.uia_healthy,
        session_id: snap.session_id,
        version: SPIKE_VERSION,
        mode,
        unsupported_dialog_count: snap.unsupported_dialog_count,
        // v0.5.5 (Mavis Round 21 B-4)
        first_push_received: snap.first_push_received,
        // v0.5.8: the stale-path gate's input and verdict, both visible.
        last_health_poll_ts: last_poll,
        last_health_poll_age_ms: age_ms,
        extension_alive: alive_now,
        // v0.5.15
        afm_visit_latched: snap.afm_visit_latched,
    };
    respond_json(request, 200, &ResponseBody::Health(body))
}

/// v0.5.8 — extension liveness ping. Stamps the ONLY stamp the stale-path gate
/// reads. See the route comment in `run_server` for why this cannot be
/// `/health`.
fn handle_ext_alive(
    request: tiny_http::Request,
    state: Arc<AppState>,
) -> Result<()> {
    let window = crate::writer::EXTENSION_ALIVE_WINDOW_MS;
    let alive_before = state.extension_alive(window);
    state.mark_extension_ping();
    let alive_after = state.extension_alive(window);

    // Announce the transition out loud, once, in both directions. Every silent
    // gate in this project's history became a bug nobody could see.
    if !alive_before && alive_after {
        tracing::info!(
            "v0.5.8 EXTENSION ALIVE: /ext_alive ping received; stale-path gate OPEN"
        );
    }

    let resp = ResponseBody::Ok { ok: true, error: None };
    respond_json(request, 200, &resp)
}

// ---- response helper ----

fn respond_json(
    request: tiny_http::Request,
    status: u16,
    body: &ResponseBody,
) -> Result<()> {
    let json = serde_json::to_string(body)?;
    // v0.5.8: charset=utf-8 — this endpoint has always sent bare
    // `application/json`, and the omission has cost real time three times on
    // 2026-10-08 alone: `Invoke-WebRequest ... .Content` returned
    // `E:\åŠå…æä»¶` for `E:\办公文件`, because the client falls back to the
    // system ANSI code page (936/GBK on this machine) when no charset is
    // declared. The sidecar sends UTF-8 unconditionally, so declaring it is
    // simply making the truth visible. Note that `Invoke-RestMethod` happened
    // to decode correctly while `Invoke-WebRequest` did not — the kind of
    // split that sends you hunting a mojibake bug that does not exist.
    let response = Response::from_string(json)
        .with_status_code(StatusCode(status))
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
                .unwrap(),
        );
    request.respond(response)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_response_includes_all_fields() {
        let snap = crate::state::HealthSnapshot {
            uptime_secs: 100,
            last_ping_received_ts: 1700000000000,
            last_dialog_event_ts: 1700000000000,
            active_dialogs: 2,
            uia_healthy: true,
            session_id: 1,
            unsupported_dialog_count: 0,
            // v0.5.5 (Mavis Round 21 B-4)
            first_push_received: true,
            // v0.5.15: 取 true,配合下面的往返断言 —— 内存里有值不等于出得去
            // (2026-10-15 的 `favoriteCategories` 就是这么丢的)。
            afm_visit_latched: true,
        };
        let resp = HealthResponse {
            status: "ok",
            uptime_secs: snap.uptime_secs,
            last_ping_received_ts: snap.last_ping_received_ts,
            last_dialog_event_ts: snap.last_dialog_event_ts,
            active_dialogs: snap.active_dialogs,
            uia_healthy: snap.uia_healthy,
            session_id: snap.session_id,
            version: SPIKE_VERSION,
            mode: "service",
            unsupported_dialog_count: snap.unsupported_dialog_count,
            // v0.5.5 (Mavis Round 21 B-4)
            first_push_received: snap.first_push_received,
            last_health_poll_ts: 0,
            last_health_poll_age_ms: -1,
            extension_alive: false,
            afm_visit_latched: snap.afm_visit_latched,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"uptime_secs\":100"));
        assert!(json.contains("\"session_id\":1"));
        // v0.5.5 (Mavis Round 23 N-1): use SPIKE_VERSION constant, NOT
        // hardcoded string. Prevents test from breaking every time version
        // bumps.
        assert!(json.contains(&format!("\"version\":\"{}\"", SPIKE_VERSION)));
        assert!(json.contains("\"mode\":\"service\""));
        assert!(json.contains("\"unsupported_dialog_count\":0"));
        // v0.5.15: 闩状态必须真的上得了线。字段在内存里、没进 JSON,真机上就只能猜。
        assert!(
            json.contains("\"afm_visit_latched\":true"),
            "expected field in JSON, got: {json}"
        );
    }

    #[test]
    fn health_response_degraded_when_session_zero() {
        // v3 review V3.1: session_id = 0 should be "degraded", not panic.
        // We can't easily test handle_health directly (it needs tiny_http Request),
        // but the status logic is verified by the data flow above.
        let body = HealthResponse {
            status: "degraded",
            uptime_secs: 0,
            last_ping_received_ts: 0,
            last_dialog_event_ts: 0,
            active_dialogs: 0,
            uia_healthy: true,
            session_id: 0,
            version: SPIKE_VERSION,
            mode: "service",
            unsupported_dialog_count: 0,
            // v0.5.5 (Mavis Round 21 B-4)
            first_push_received: false,
            last_health_poll_ts: 0,
            last_health_poll_age_ms: -1,
            extension_alive: false,
            afm_visit_latched: false,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("\"status\":\"degraded\""));
    }

    /// focus-19 (2026-10-02): the unsupported_dialog_count field MUST be
    /// serialized in /health JSON. The extension uses this to decide when
    /// to surface a "this dialog isn't supported" notification.
    #[test]
    fn health_response_includes_unsupported_dialog_count() {
        let body = HealthResponse {
            status: "ok",
            uptime_secs: 5,
            last_ping_received_ts: 1,
            last_dialog_event_ts: 2,
            active_dialogs: 0,
            uia_healthy: true,
            session_id: 1,
            version: SPIKE_VERSION,
            mode: "service",
            unsupported_dialog_count: 7,
            // v0.5.5 (Mavis Round 21 B-4)
            first_push_received: false,
            last_health_poll_ts: 0,
            last_health_poll_age_ms: -1,
            extension_alive: false,
            afm_visit_latched: false,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(
            json.contains("\"unsupported_dialog_count\":7"),
            "expected field in JSON, got: {json}"
        );
    }
}