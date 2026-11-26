// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! T1 / U1 探针：验证打包产物能否读到百度网盘登录后的 `BDUSS` cookie。
//!
//! 只在 `AFM_BAIDU_PROBE=1` 时激活；未设置时 `install` 立即返回，零副作用，
//! 生产用户不会看到任何窗口。
//!
//! ## `cargo test --lib` 红线
//!
//! 本文件接触 `tauri::AppHandle` / `WebviewWindowBuilder`，**不得**进入单元测试的
//! 链接闭包，否则会把 tao 拖进来、测试二进制在加载阶段崩在
//! `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)`，一个测试都跑不了。
//! 本模块只由 `setup_handler` 调用，而测试从不执行 `setup_handler`，因此链接器会整段
//! 丢弃。红线完整说明见 `remote/mod.rs` 顶部。
//!
//! ## 为什么用「页面内 fetch + 本地回传」而不是在 Rust 侧发请求
//!
//! - 仓库的 `reqwest`（`src-tauri/Cargo.toml`）**没有启用 `blocking`**，只有 `rustls` + `stream`；
//! - `tauri::async_runtime::*` 是上面那条红线明确禁止的；
//! - 注入到 `pan.baidu.com` 页面里的 `fetch` 天然同源、自动带 cookie，
//!   只需要把结果回传到本地 `TcpListener`。
//!
//! 零新增依赖，而且**`BDUSS` 的值全程不落盘**（只报告长度与前 6 位），
//! 报告里不会出现可直接盗用账号的凭据。
//!
//! ## Windows 上的死锁坑
//!
//! `webview_window.rs:2731` 原文：*On Windows, this function deadlocks when used in a
//! synchronous command or event handlers ... use `async` commands and separate threads
//! when reading cookies.*
//! 所以 cookie 轮询必须跑在**独立线程**上，本模块正是这么做的。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// 开关。只有显式设为 `1` 才启动探针。
const GATE_ENV: &str = "AFM_BAIDU_PROBE";
/// 报告输出路径；缺省落在系统临时目录，文件名固定便于事后读取。
const OUT_ENV: &str = "AFM_BAIDU_PROBE_OUT";
const DEFAULT_OUT_NAME: &str = "afm-baidu-probe.txt";

/// 起始页。第二轮改成网盘主界面：登录态已存在时直接进主界面，同时触发
/// `pan.baidu.com` 域下发 `STOKEN` —— 用来验证「`STOKEN` 才是 `/api/*` 的前置条件」
/// 这一假设。若登录态缺失，百度会把这里重定向回登录页，不影响 `BDUSS` 轮询。
const START_URL: &str = "https://pan.baidu.com/disk/main";
/// 同时也是 `cookies_for_url` 的查询锚点。
const COOKIE_URL: &str = "https://pan.baidu.com/";

/// 轮询间隔。太密会在 WebView2 上刷屏，2 秒足够。
const POLL: Duration = Duration::from_secs(2);
/// 超时上限。到点还没登录就写一份超时报告并关窗，避免窗口永远开着。
const GIVE_UP: Duration = Duration::from_secs(900);

/// 页面回传端口。固定端口便于在报告里核对；被占用时本次探针失败并如实记录。
const CALLBACK_PORT: u16 = 47831;

/// 上下文验证要打的接口。都免费、无副作用、只读。
///
/// 第一轮（T1/U1 首跑）实测结果：
/// - `xpan/nas?method=uinfo` → 200 `errno:0` + 昵称 ✅（阳性对照）
/// - `api/quota`            → 400 `{"errmsg":"用户未登录","errno":-6}` ❌
/// - `api/list`             → 200 `{"errno":-6}` ❌
///
/// 同样是带真 cookie 的页面内同源 fetch，`xpan` 成功而 `/api/*` 全挂 ⇒ 两者读的不是
/// 同一份凭证。`/api/*` 的历史实现依赖 `pan.baidu.com` 域下的 `STOKEN`（首轮报告
/// `stoken_present: no`）。第二轮把起始页换成网盘主界面，看 `STOKEN` 是否下发、
/// `/api/*` 是否随之复活；同时补测 `xpan` 的 list / quota，验证「阶段 1 全部走 xpan REST」
/// 是否可行 —— 这条直接决定 T2 里 `baidu_api.rs` 的接口形态。
const PROBE_APIS: [(&str, &str); 4] = [
    (
        "uinfo",
        "/rest/2.0/xpan/nas?method=uinfo&app_id=250528&web=1",
    ),
    (
        "xpan_list",
        "/rest/2.0/xpan/file?method=list&dir=%2F&order=name&start=0&limit=10&web=1&app_id=250528",
    ),
    (
        "xpan_quota",
        "/rest/2.0/xpan/quota?checkfree=1&checkexpire=1&web=1&app_id=250528",
    ),
    (
        "api_list",
        "/api/list?dir=%2F&order=name&desc=0&start=0&limit=10&web=1&clienttype=0&app_id=250528&channel=chunlei",
    ),
];

/// 报告里每个 cookie 只写这些字段，**不写完整 value**。
const VALUE_HEAD_CHARS: usize = 6;

/// 入口。由 `setup_handler` 调用；gate 未开时什么都不做。
///
/// 永远返回 `()`：探针是诊断工具，任何失败都只写进报告，不能阻断应用启动。
pub fn install(app: &tauri::AppHandle) {
    if std::env::var(GATE_ENV).ok().as_deref() != Some("1") {
        return;
    }
    let report_path = out_path();
    let mut report = Report::new(report_path.clone());
    report.line(&format!("AFM baidu cookie probe (T1 / U1)"));
    report.line(&format!("started_utc: {}", utc_stamp()));
    report.line(&format!("gate_env: {GATE_ENV}=1"));
    report.line(&format!("cookie_url: {COOKIE_URL}"));
    report.line(&format!("callback: 127.0.0.1:{CALLBACK_PORT}"));
    report.line("");

    // 回调监听器先起来，免得页面 fetch 完成后端口还没准备好。
    if let Err(err) = CallbackSink::start(report_path.clone()) {
        report.line(&format!("FATAL: cannot bind callback listener: {err}"));
        report.flush();
        return;
    }
    report.line(&format!("callback_listener: bound on port {CALLBACK_PORT}"));

    let window = match WebviewWindowBuilder::new(
        app,
        "afm-baidu-probe",
        WebviewUrl::External(
            // 常量已核对过是合法 URL，解析失败只可能是编译期笔误。
            tauri::Url::parse(START_URL).expect("START_URL must be a valid absolute URL"),
        ),
    )
    .title("Alpha File Manager - Baidu cookie probe (T1/U1)")
    .inner_size(1100.0, 820.0)
    .build()
    {
        Ok(window) => window,
        Err(err) => {
            report.line(&format!("FATAL: cannot build probe window: {err}"));
            report.flush();
            return;
        }
    };
    report.line("probe_window: built");
    report.flush();

    // ---- 独立线程轮询 cookie（Windows 同步调用会死锁，见模块注释）----
    let poll_window = window.clone();
    let poll_report = report_path.clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut last_summary = String::new();
        loop {
            if started.elapsed() > GIVE_UP {
                append(
                    &poll_report,
                    &[
                        "",
                        &format!(
                            "RESULT: TIMEOUT after {}s - no BDUSS seen",
                            GIVE_UP.as_secs()
                        ),
                        "VERDICT_U1: FAIL",
                    ],
                );
                // 尽量把窗口收掉，免得留一个登录窗给用户。
                let _ = poll_window.close();
                return;
            }
            std::thread::sleep(POLL);

            // Url 解析失败属于编译期错误，这里直接跳过本轮更安全。
            let Ok(url) = tauri::Url::parse(COOKIE_URL) else {
                append(&poll_report, &["FATAL: COOKIE_URL no longer parses"]);
                return;
            };
            let cookies = match poll_window.cookies_for_url(url.clone()) {
                Ok(cookies) => cookies,
                Err(err) => {
                    append(&poll_report, &[&format!("cookies_for_url error: {err}")]);
                    continue;
                }
            };

            // 每轮把 cookie 全貌记一次，但只在「清单变化」时落盘，避免刷爆文件。
            let summary = describe_all(&cookies);
            if summary != last_summary {
                append(
                    &poll_report,
                    &[
                        "",
                        &format!(
                            "--- cookies at +{}s (count={}) ---",
                            started.elapsed().as_secs(),
                            cookies.len()
                        ),
                        &summary,
                    ],
                );
                last_summary = summary;
            }

            let Some(bduss) = find_cookie(&cookies, "BDUSS") else {
                continue;
            };
            let mut stoken = find_cookie(&cookies, "STOKEN").map(|c| c.value().to_string());

            // 第二轮新增：`/api/*` 走的是 `pan.baidu.com` 域下的 `STOKEN`，它在导航到
            // 网盘主界面之后才由服务端下发。profile 里已有登录态时，BDUSS 在 +2s 就可见，
            // 而页面尚未加载完 ⇒ 直接 eval 会测到「STOKEN 缺失」的中间态，把「时机问题」
            // 误读成「`/api/*` 不可用」。这里最多额外等 20 秒，拿到 STOKEN 或等满再继续。
            if stoken.is_none() {
                let stoken_deadline = Instant::now() + Duration::from_secs(20);
                while Instant::now() < stoken_deadline {
                    std::thread::sleep(POLL);
                    let Ok(later) = poll_window.cookies_for_url(url.clone()) else {
                        continue;
                    };
                    if let Some(cookie) = find_cookie(&later, "STOKEN") {
                        append(
                            &poll_report,
                            &[&format!(
                                "STOKEN_APPEARED_AFTER: +{}s",
                                started.elapsed().as_secs()
                            )],
                        );
                        stoken = Some(cookie.value().to_string());
                        break;
                    }
                }
            }

            append(
                &poll_report,
                &[
                    "",
                    "BDUSS_FOUND: yes",
                    &format!("bduss_len: {}", bduss.value().len()),
                    &format!("bduss_domain: {}", bduss.domain().unwrap_or("<none>")),
                    &format!("bduss_path: {}", bduss.path().unwrap_or("<none>")),
                    &format!("bduss_secure: {}", bduss.secure().unwrap_or(false)),
                    &format!("bduss_http_only: {}", bduss.http_only().unwrap_or(false)),
                    &format!(
                        "stoken_present: {}",
                        if stoken.is_some() { "yes" } else { "no" }
                    ),
                ],
            );
            // 读到了就注一次端到端校验，然后等回传。
            let js = probe_script(CALLBACK_PORT);
            if let Err(err) = poll_window.eval(&js) {
                append(&poll_report, &[&format!("eval error: {err}")]);
            }
            // 等回传；最多 30 秒。
            SinkState::wait(&poll_report, Duration::from_secs(30));
            let _ = poll_window.close();
            return;
        }
    });
}

fn out_path() -> PathBuf {
    match std::env::var(OUT_ENV) {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => std::env::temp_dir().join(DEFAULT_OUT_NAME),
    }
}

/// 页面内脚本：同源 `fetch` 三条只读接口，把结果用 `<img>` 回传到本地监听器。
///
/// 用 `new Image().src` 而不是 `fetch` 回传，是为了绕开跨域限制：
/// 图片请求不受 CORS 约束，也不需要服务端回 CORS 头。
fn probe_script(port: u16) -> String {
    let mut fetches = String::new();
    for (key, path) in PROBE_APIS {
        fetches.push_str(&format!(
            r#"
  try {{
    const r = await fetch('{path}', {{ credentials: 'include' }});
    send('{key}_status', String(r.status));
    const t = await r.text();
    send('{key}_body', t.length > 2400 ? t.slice(0, 2400) + '...[truncated]' : t);
  }} catch (e) {{ send('{key}_error', String(e)); }}
"#
        ));
    }
    format!(
        r#"(async () => {{
  const port = {port};
  const send = (k, v) => {{
    try {{
      new Image().src = 'http://127.0.0.1:' + port + '/r?k=' + encodeURIComponent(k) +
                        '&b=' + encodeURIComponent(String(v == null ? '' : v));
    }} catch (e) {{}}
  }};
  send('page_url', location.href);
  send('page_ua', navigator.userAgent);
{fetches}
  send('done', '1');
}})();"#
    )
}

/// 单个 cookie 的一行描述。**刻意不含完整 value。**
fn describe(cookie: &tauri::webview::Cookie<'static>) -> String {
    let value = cookie.value();
    let head: String = value.chars().take(VALUE_HEAD_CHARS).collect();
    format!(
        "  name={} domain={} path={} secure={} http_only={} expires={} value_len={} value_head={}...",
        cookie.name(),
        cookie.domain().unwrap_or("<none>"),
        cookie.path().unwrap_or("<none>"),
        cookie.secure().unwrap_or(false),
        cookie.http_only().unwrap_or(false),
        cookie
            .expires()
            // `cookie::Expiration` 只实现了 `Debug`（`Display` 只有开了 time 的
            // `formatting` feature 才可用），这里用 Debug 足够诊断。
            .map(|e| format!("{e:?}"))
            .unwrap_or_else(|| "<no-expiry>".to_string()),
        value.len(),
        head,
    )
}

fn describe_all(cookies: &[tauri::webview::Cookie<'static>]) -> String {
    let mut out = String::new();
    // 先把百度域下的挑前面，报告更好读。
    let mut sorted: Vec<_> = cookies.iter().collect();
    sorted.sort_by_key(|c| {
        let d = c.domain().unwrap_or("");
        (!d.contains("baidu"), c.name().to_string())
    });
    for cookie in sorted {
        out.push_str(&describe(cookie));
        out.push('\n');
    }
    out
}

fn find_cookie<'a>(
    cookies: &'a [tauri::webview::Cookie<'static>],
    name: &str,
) -> Option<&'a tauri::webview::Cookie<'static>> {
    cookies
        .iter()
        .find(|c| c.name() == name && !c.value().is_empty())
}

fn append(path: &PathBuf, lines: &[&str]) {
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    let _ = file.write_all(text.as_bytes());
    let _ = file.flush();
}

// ---------------------------------------------------------------------------
// 自检驱动器（2026-11-09）
// ---------------------------------------------------------------------------

/// 自检开关。设为 `1` 时跑一遍完整链路：
/// 「开登录窗 → 等到登录态 → 认领落盘」，走的是**真实命令**，
/// 不是另写一套旁路。
const SELFTEST_ENV: &str = "AFM_BAIDU_SELFTEST";
/// 自检目标连接 id。给了就用它；不给就从连接表里挑第一条 `baidu_pan`。
const SELFTEST_CONN_ENV: &str = "AFM_BAIDU_SELFTEST_CONN";
/// 自检报告路径。
const SELFTEST_OUT_ENV: &str = "AFM_BAIDU_SELFTEST_OUT";
/// 自检最多等多久（秒）。
const SELFTEST_WAIT: Duration = Duration::from_secs(180);

/// 入口。由 `setup_handler` 调用；gate 未开时什么都不做。
pub fn install_selftest(app: &tauri::AppHandle) {
    if std::env::var(SELFTEST_ENV).ok().as_deref() != Some("1") {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || run_selftest(app));
}

fn selftest_out_path() -> PathBuf {
    match std::env::var(SELFTEST_OUT_ENV) {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value.trim()),
        _ => std::env::temp_dir().join("afm-baidu-selftest.txt"),
    }
}

fn run_selftest(app: tauri::AppHandle) {
    let path = selftest_out_path();
    let _ = std::fs::write(&path, b"");
    append(&path, &[&format!("SELFTEST started_utc: {}", utc_stamp())]);

    let connection_id = match resolve_selftest_connection(&app) {
        Ok(id) => id,
        Err(err) => {
            append(
                &path,
                &[
                    &format!("SELFTEST FATAL: cannot resolve connection: {err}"),
                    "VERDICT_SELFTEST: FAIL",
                ],
            );
            return;
        }
    };
    append(&path, &[&format!("SELFTEST target connection: {connection_id}")]);

    // 走真实命令：开登录窗。
    match crate::baidu_login::remote_baidu_login_start(app.clone()) {
        Ok(payload) => append(
            &path,
            &[&format!("SELFTEST login_start: opened={}", payload.opened)],
        ),
        Err(err) => {
            append(
                &path,
                &[
                    &format!("SELFTEST FATAL: login_start failed: {err}"),
                    "VERDICT_SELFTEST: FAIL",
                ],
            );
            return;
        }
    }

    // 等认领槽位被填上。
    let started = Instant::now();
    let mut pending = None;
    while started.elapsed() < SELFTEST_WAIT {
        std::thread::sleep(Duration::from_secs(1));
        let state = crate::baidu_login::remote_baidu_login_pending();
        if state.pending {
            pending = Some(state);
            break;
        }
    }

    let Some(state) = pending else {
        append(
            &path,
            &[
                &format!("SELFTEST RESULT: FAIL (no pending session within {SELFTEST_WAIT:?})"),
                "VERDICT_SELFTEST: FAIL",
            ],
        );
        return;
    };
    append(
        &path,
        &[&format!(
            "SELFTEST pending: has_stoken={} summary={}",
            state.has_stoken, state.summary
        )],
    );

    // 认领落盘 —— 这一步会真的改写 `remote-credentials.json`。
    match crate::baidu_login::remote_baidu_login_claim(app.clone(), connection_id.clone()) {
        Ok(payload) => append(
            &path,
            &[
                &format!(
                    "SELFTEST claim: claimed={} summary={}",
                    payload.claimed, payload.summary
                ),
                "PROBE_SEQUENCE: complete",
                "VERDICT_SELFTEST: PASS",
            ],
        ),
        Err(err) => append(
            &path,
            &[
                &format!("SELFTEST FATAL: claim failed: {err}"),
                "VERDICT_SELFTEST: FAIL",
            ],
        ),
    }
}

/// 找自检要用的连接 id：环境变量优先，否则从连接表里挑第一条 `baidu_pan`。
fn resolve_selftest_connection(app: &tauri::AppHandle) -> Result<String, String> {
    if let Ok(value) = std::env::var(SELFTEST_CONN_ENV) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    let dir = crate::app_dirs::app_data_dir(app).map_err(|err| err.to_string())?;
    let path = dir.join("user-data").join("user-remote-connections.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("{}: {err}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    let connections = value
        .get("connections")
        .and_then(serde_json::Value::as_array)
        .ok_or("连接表里没有 connections 数组")?;

    for connection in connections {
        let is_baidu = connection
            .get("protocol")
            .and_then(serde_json::Value::as_str)
            == Some("baidu_pan");
        if is_baidu {
            if let Some(id) = connection.get("id").and_then(serde_json::Value::as_str) {
                return Ok(id.to_string());
            }
        }
    }

    Err("连接表里没有 baidu_pan 连接".to_string())
}

fn utc_stamp() -> String {
    // 不引入 chrono：探针只需要一个粗时间戳，用 SystemTime 的秒数即可。
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("epoch+{}s", d.as_secs()))
        .unwrap_or_else(|_| "<unknown>".to_string())
}

struct Report {
    path: PathBuf,
    text: String,
}

impl Report {
    fn new(path: PathBuf) -> Self {
        // 每次启动覆盖上一次的结果，避免新旧混在一起误判。
        let _ = std::fs::write(&path, b"");
        Self {
            path,
            text: String::new(),
        }
    }
    fn line(&mut self, text: &str) {
        self.text.push_str(text);
        self.text.push('\n');
    }
    fn flush(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut f| f.write_all(self.text.as_bytes()));
        self.text.clear();
    }
}

/// 本地回传监听器。收到页面 `<img>` 请求后把内容追加进报告。
struct CallbackSink;

impl CallbackSink {
    fn start(report_path: PathBuf) -> std::io::Result<()> {
        let listener = TcpListener::bind(("127.0.0.1", CALLBACK_PORT))?;
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 8192];
                let read = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..read]).to_string();
                let line = request.lines().next().unwrap_or("").to_string();
                let parsed = parse_callback(&line);
                if let Some((key, value)) = parsed {
                    append(&report_path, &[&format!("CB {key}: {value}")]);
                    if key == "done" {
                        append(
                            &report_path,
                            &[
                                "PROBE_SEQUENCE: complete",
                                "VERDICT_U1: PASS (BDUSS readable from packaged webview)",
                            ],
                        );
                    }
                }
                // 1x1 透明 GIF，让页面上的 <img> 正常结束。
                let body: [u8; 35] = [
                    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0x21, 0xf9, 0x04, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01,
                ];
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/gif\r\n");
                let _ = stream.write_all(b"Content-Length: 35\r\nConnection: close\r\n\r\n");
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            }
        });
        Ok(())
    }
}

/// 从请求行提取 `(k, b)`；b 做 percent-decode 后转 UTF-8。
fn parse_callback(request_line: &str) -> Option<(String, String)> {
    let mut parts = request_line.split_whitespace();
    let _method = parts.next()?;
    let target = parts.next()?;
    let query = target.split_once('?')?.1;
    let mut key = None;
    let mut value = None;
    for pair in query.split('&') {
        let (raw_k, raw_v) = pair.split_once('=').unwrap_or((pair, ""));
        match raw_k {
            "k" => key = Some(String::from_utf8_lossy(&percent_decode(raw_v)).to_string()),
            "b" => value = Some(String::from_utf8_lossy(&percent_decode(raw_v)).to_string()),
            _ => {}
        }
    }
    Some((key?, value.unwrap_or_default()))
}

fn percent_decode(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_nibble(bytes[index + 1]), hex_nibble(bytes[index + 2]))
            {
                out.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        out.push(if byte == b'+' { b' ' } else { byte });
        index += 1;
    }
    out
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

struct SinkState;

impl SinkState {
    /// 等到报告里出现 `PROBE_SEQUENCE: complete`，或超时。
    fn wait(report_path: &PathBuf, limit: Duration) {
        let started = Instant::now();
        while started.elapsed() < limit {
            std::thread::sleep(Duration::from_millis(500));
            let Ok(text) = std::fs::read_to_string(report_path) else {
                continue;
            };
            if text.contains("PROBE_SEQUENCE: complete") {
                return;
            }
        }
        append(
            report_path,
            &[
                "PROBE_SEQUENCE: TIMEOUT waiting for page callbacks",
                "VERDICT_U1: PARTIAL (BDUSS readable, but in-page API check did not report back)",
            ],
        );
    }
}
