// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// ---------------------------------------------------------------------------
// 阿里云盘「扫码登录」的纯函数层
//
// 红线（与 `remote/mod.rs:8-14` 同一条）：本文件被 `cargo test --lib` 的测试
// 二进制链接，**不得出现 `tauri::AppHandle`、`tauri::async_runtime::*` 或
// `tauri::` 的任何路径**。一旦碰到，Tauri 全局运行时的默认类型参数 `Wry`
// 被单态化，测试二进制在加载阶段以 `STATUS_ENTRYPOINT_NOT_FOUND`
// (0xC0000139) 崩掉 —— 一个测试都跑不了，且报错与本模块毫无关系。
//
// # 为什么是扫码而不是 OAuth 授权码
//
// 证据：`.superpowers/sdd/b28-api-probe.md` §「code → token 的两条路径」。
// - **QR 直出**（`query.do` → `CONFIRMED` → `bizExt` → `pds_login_result.accessToken`）
//   拿到的 token 实测能直接当 Bearer 用（`/v2/user/get` 200、`file/list` 200）。
// - **OAuth code 换 token**（`POST /token/get`）拿到的 41 字符 token
//   **不能**当 Bearer（HTTP 400 `InvalidHeader`）⇒ 这条路**不可用**。
//
// ⇒ 本轮只走 QR。OAuth 那条路的常量与函数保留在 `aliyun_api.rs` 里，
// 但**不接线**，并在注释里写明「实测不可用」。
//
// # 凭据纪律
//
// `accessToken` / `refreshToken` 的**明文**只在进程内存里流转。
// 本模块的 `Debug` 只打印长度与 sha16 前 16 位；错误文案里也绝不含它们。
// ---------------------------------------------------------------------------

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::remote::error::{RemoteError, RemoteErrorKind};

/// 取字符串字段。数字也接受（转成字符串）。
fn lenient_str(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

/// 取整数字段。字符串数字也接受。
fn lenient_i64(value: &Value, key: &str) -> Option<i64> {
    match value.get(key) {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// 在一层 JSON 里找一个字段；找不到就往 `data` / `content.data` 里再找一层。
///
/// 服务端的响应外壳不统一（有的直接是对象，有的裹了一层 `content`），
/// 而我们**不想**因为外壳变了就整个功能挂掉 ⇒ 逐层找。
fn find_field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    if let Some(found) = value.get(key) {
        return Some(found);
    }

    for path in ["data", "content", "result"] {
        let Some(inner) = value.get(path) else {
            continue;
        };
        if let Some(found) = inner.get(key) {
            return Some(found);
        }
        if let Some(inner) = inner.get("data") {
            if let Some(found) = inner.get(key) {
                return Some(found);
            }
        }
    }

    None
}

/// 在一层 JSON 里找一个字符串字段（含上面的逐层兜底）。
fn find_str(value: &Value, key: &str) -> Option<String> {
    find_field(value, key).and_then(|found| match found {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

/// 在一层 JSON 里找一个整数字段（含上面的逐层兜底）。
fn find_i64(value: &Value, key: &str) -> Option<i64> {
    find_field(value, key).and_then(|found| match found {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    })
}

// ===========================================================================
// 扫码登录：取二维码
// ===========================================================================

/// 一次扫码登录的「二维码票据」。
///
/// `code_content` 是要画成二维码图的那串东西（前端用 `qrcode` 库渲染）；
/// `ck` 与 `t` 是轮询时要带回去的东西。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliyunQrTicket {
    /// 二维码内容。
    pub code_content: String,
    /// 轮询用的会话标识。
    pub ck: String,
    /// 轮询用的时间戳。
    pub t: Option<i64>,
}

/// 解析 `GET /newlogin/qrcode/generate.do` 的响应。
///
/// 响应形状（实测）：`content.data.{ codeContent, ck, t, resultCode, processFinished }`。
pub fn aliyun_parse_qr_ticket(body: &str) -> Result<AliyunQrTicket, RemoteError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| RemoteError::protocol("阿里云盘返回的二维码内容无法解析。"))?;

    if !value.is_object() {
        return Err(RemoteError::protocol("阿里云盘返回的二维码内容无法解析。"));
    }

    let code_content = find_str(&value, "codeContent")
        .or_else(|| find_str(&value, "code_content"))
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| RemoteError::protocol("阿里云盘没有返回二维码内容，请稍后重试。"))?;

    let ck = find_str(&value, "ck")
        .map(|text| text.trim().to_string())
        .unwrap_or_default();

    Ok(AliyunQrTicket {
        code_content,
        ck,
        t: find_i64(&value, "t"),
    })
}

// ===========================================================================
// 扫码登录：轮询状态
// ===========================================================================

/// 扫码状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliyunQrStatus {
    /// 还没扫。
    New,
    /// 已扫，等确认。
    Scanned,
    /// 已确认 ⇒ `bizExt` 里有凭据。
    Confirmed,
    /// 出错了。
    Error,
    /// 二维码过期。
    Expired,
    /// 用户取消了。
    Canceled,
}

impl AliyunQrStatus {
    /// 机器名（前端用）。与后端状态卡一样，全小写。
    pub fn name(&self) -> &'static str {
        match self {
            AliyunQrStatus::New => "new",
            AliyunQrStatus::Scanned => "scanned",
            AliyunQrStatus::Confirmed => "confirmed",
            AliyunQrStatus::Error => "error",
            AliyunQrStatus::Expired => "expired",
            AliyunQrStatus::Canceled => "canceled",
        }
    }

    /// 是不是终态（不用再轮询了）。
    pub fn is_final(&self) -> bool {
        !matches!(self, AliyunQrStatus::New | AliyunQrStatus::Scanned)
    }

    /// 从服务端的 `qrCodeStatus` 解析。
    ///
    /// **不认识的值按 `Error` 处理** —— 宁可让用户重扫一次，
    /// 也不能让前端在一个看不懂的状态上永远转圈。
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "NEW" => AliyunQrStatus::New,
            "SCANED" | "SCANNED" => AliyunQrStatus::Scanned,
            "CONFIRMED" => AliyunQrStatus::Confirmed,
            "EXPIRED" => AliyunQrStatus::Expired,
            "CANCELED" | "CANCELLED" => AliyunQrStatus::Canceled,
            _ => AliyunQrStatus::Error,
        }
    }
}

/// 一次轮询的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliyunQrPoll {
    /// 状态。
    pub status: AliyunQrStatus,
    /// 确认后服务端给的 base64 载荷（`bizExt`）。只有 `Confirmed` 时才有。
    pub biz_ext: Option<String>,
}

impl Default for AliyunQrPoll {
    fn default() -> Self {
        Self {
            status: AliyunQrStatus::New,
            biz_ext: None,
        }
    }
}

/// 解析 `POST /newlogin/qrcode/query.do` 的响应。
///
/// 响应形状（实测）：`content.data.{ qrCodeStatus, resultCode, bizExt }`。
pub fn aliyun_parse_qr_poll(body: &str) -> Result<AliyunQrPoll, RemoteError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| RemoteError::protocol("阿里云盘返回的扫码状态无法解析。"))?;

    if !value.is_object() {
        return Err(RemoteError::protocol("阿里云盘返回的扫码状态无法解析。"));
    }

    let raw_status = find_str(&value, "qrCodeStatus")
        .or_else(|| find_str(&value, "qr_code_status"))
        .unwrap_or_default();

    let status = if raw_status.is_empty() {
        // 没有状态字段 ⇒ 服务端在报错（`resultCode` 之类）。
        AliyunQrStatus::Error
    } else {
        AliyunQrStatus::parse(&raw_status)
    };

    let biz_ext = find_str(&value, "bizExt")
        .or_else(|| find_str(&value, "biz_ext"))
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());

    Ok(AliyunQrPoll { status, biz_ext })
}

// ===========================================================================
// 扫码登录：解码凭据
// ===========================================================================

/// 从 `bizExt` 里解出来的凭据。
#[derive(Clone, PartialEq, Eq)]
pub struct AliyunQrCredentials {
    /// 访问凭证。**绝不进日志、绝不进前端载荷。**
    pub access_token: String,
    /// 刷新凭证。
    pub refresh_token: Option<String>,
    /// 「我的文件」盘 id。
    pub default_drive_id: Option<String>,
    /// 有效秒数。
    pub expires_in: Option<i64>,
    /// 绝对过期时刻（UTC ISO8601 原文）。
    pub expire_time: Option<String>,
    /// 用户 id。
    pub user_id: Option<String>,
    /// 昵称。
    pub nick_name: Option<String>,
}

impl std::fmt::Debug for AliyunQrCredentials {
    /// **只打印长度与 sha16，绝不打印明文。**
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "AliyunQrCredentials(access_token(len={}), sha16={})",
            self.access_token.len(),
            short_sha16(&self.access_token)
        )
    }
}

/// 一段文本的 sha256 前 16 位（十六进制）。给日志做「同一个值是不是同一个」的对照。
pub fn short_sha16(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    hex[..16].to_string()
}

/// 把一段 base64 文本归一化成 `STANDARD` 字母表能解的形状。
///
/// 为什么要这么啰嗦：淘系 `bizExt` 里的 base64 **是 URL-safe 变体**（`-` / `_`），
/// 而当初 b28 取证时用的是 **Node 的宽松解码**（`Buffer.from(s, 'base64')` 会自动把
/// `-_` 映射回 `+/`、忽略非法字符、不要求补 `=`）⇒ 同一份真实数据，
/// Node 能解出 777 长度的 token，**Rust 的严格 `STANDARD.decode` 必失败**。
/// 2026-11-20 真机实测就是死在这一步（`biz_ext_len=2852` → 「扫码结果无法解码」）。
///
/// 这里复刻 Node 的宽松语义：只保留 base64 字母表内的字符，映射 URL-safe 别名，
/// 再按 4 字节一组补齐 `=`。
fn aliyun_normalize_base64(text: &str) -> String {
    let mut body = String::with_capacity(text.len() + 4);
    for ch in text.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '+' | '/' => body.push(ch),
            '-' => body.push('+'),
            '_' => body.push('/'),
            _ => {}
        }
    }
    let remainder = body.len() % 4;
    if remainder != 0 && remainder != 1 {
        body.push_str(&"=".repeat(4 - remainder));
    }
    body
}

/// 把 `bizExt` 解出来的字节变成文本。
///
/// **实测（2026-11-20）**：阿里云盘那坨 JSON 里的**中文是 GBK/GB18030 编码的，
/// 不是 UTF-8** —— 2138 字节里有 76 个非法 UTF-8 字节（`userData.back_up_config`
/// 的键「手机备份」就是 GBK 双字节）。
///
/// 于是 `String::from_utf8` 整条失败 ⇒ 用户看到「扫码结果无法解码，请重新扫码」，
/// 而页面那边其实已经确认登录、凭据也到手了（扫码白扫一次）。
///
/// 换编码**不会动到凭据**：`accessToken` / `refreshToken` / `defaultDriveId` /
/// `userId` 全是纯 ASCII（实测 777 字符 token 在 UTF-8 与 GB18030 下 sha16 相同），
/// 受影响的只有昵称、`userData` 这类展示字段。
///
/// 所以：先试严格 UTF-8，失败再按 GB18030（GBK 的超集）解；
/// 真解不出字符就交给后面的 JSON 解析去报错（那里的文案更贴切）。
fn decode_biz_ext_text(decoded: &[u8]) -> Result<String, RemoteError> {
    if let Ok(text) = std::str::from_utf8(decoded) {
        return Ok(text.to_string());
    }

    let (text, _had_errors) = encoding_rs::GB18030.decode_without_bom_handling(decoded);
    Ok(text.into_owned())
}

/// 解 `bizExt`：base64 → JSON → `pds_login_result`。
///
/// 形状（实测）：`bizExt` = base64(JSON)，JSON 里 `pds_login_result` 是凭据本体。
///
/// 兼容三种实测到的形态：
/// ① 严格标准 base64；
/// ② URL-safe base64（真机登录窗回传的就是这一种 ⇒ 走宽松归一化）；
/// ③ 直接就是 JSON（页面把 `bizExt` 当对象回传时被 `JSON.stringify` 过）。
pub fn aliyun_decode_biz_ext(biz_ext: &str) -> Result<AliyunQrCredentials, RemoteError> {
    let trimmed = biz_ext.trim();
    if trimmed.is_empty() {
        return Err(RemoteError::auth("扫码结果里没有凭据，请重新扫码。"));
    }

    let value: Value = if trimmed.starts_with('{') {
        serde_json::from_str(trimmed)
            .map_err(|_| RemoteError::auth("扫码结果无法解析，请重新扫码。"))?
    } else {
        let decoded = STANDARD
            .decode(trimmed)
            .or_else(|_| STANDARD.decode(aliyun_normalize_base64(trimmed)))
            .map_err(|_| {
                // 形状诊断：只报长度与字符类别，**绝不**打印 bizExt 正文。
                eprintln!(
                    "[remote-diag] aliyun biz_ext undecodable: len={} starts_with_brace={} has_dash={} has_underscore={} has_plus={} has_slash={}",
                    trimmed.len(),
                    trimmed.starts_with('{'),
                    trimmed.contains('-'),
                    trimmed.contains('_'),
                    trimmed.contains('+'),
                    trimmed.contains('/'),
                );
                RemoteError::auth("扫码结果无法解码，请重新扫码。")
            })?;

        let text = decode_biz_ext_text(&decoded)?;

        serde_json::from_str(&text)
            .map_err(|_| RemoteError::auth("扫码结果无法解析，请重新扫码。"))?
    };

    let result = value
        .get("pds_login_result")
        .filter(|inner| inner.is_object())
        .unwrap_or(&value);

    let access_token = lenient_str(result, "accessToken")
        .or_else(|| lenient_str(result, "access_token"))
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| RemoteError::auth("扫码结果里没有访问凭证，请重新扫码。"))?;

    Ok(AliyunQrCredentials {
        access_token,
        refresh_token: lenient_str(result, "refreshToken")
            .or_else(|| lenient_str(result, "refresh_token"))
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
        default_drive_id: lenient_str(result, "defaultDriveId")
            .or_else(|| lenient_str(result, "default_drive_id"))
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
        expires_in: lenient_i64(result, "expiresIn").or_else(|| lenient_i64(result, "expires_in")),
        expire_time: lenient_str(result, "expireTime")
            .or_else(|| lenient_str(result, "expire_time"))
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
        user_id: lenient_str(result, "userId").or_else(|| lenient_str(result, "user_id")),
        nick_name: lenient_str(result, "nickName")
            .or_else(|| lenient_str(result, "nick_name"))
            .or_else(|| lenient_str(result, "userName"))
            .or_else(|| lenient_str(result, "user_name")),
    })
}

// ===========================================================================
// 用户信息 / 网盘信息
// ===========================================================================

/// `POST /v2/user/get` 的响应里我们关心的部分。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliyunUserProfile {
    /// 「我的文件」盘 id。**这才是主盘**（实测根目录 13 项）。
    pub default_drive_id: Option<String>,
    /// 保险箱盘 id。**不可读**（`ForbiddenDriveLocked`）⇒ 本轮不用。
    pub default_sbox_drive_id: Option<String>,
    /// 用户 id。
    pub user_id: Option<String>,
    /// 昵称。
    pub nick_name: Option<String>,
}

/// 解析 `POST /v2/user/get` 的响应。
///
/// 字段位置**未完全坐实**（有的在外层、有的裹一层 `data`）⇒ 逐层找。
pub fn aliyun_parse_user_profile(body: &str) -> Result<AliyunUserProfile, RemoteError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| RemoteError::protocol("阿里云盘返回的用户信息无法解析。"))?;

    if !value.is_object() {
        return Err(RemoteError::protocol("阿里云盘返回的用户信息无法解析。"));
    }

    Ok(AliyunUserProfile {
        default_drive_id: find_str(&value, "default_drive_id")
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
        default_sbox_drive_id: find_str(&value, "default_sbox_drive_id")
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
        user_id: find_str(&value, "user_id").or_else(|| find_str(&value, "uid")),
        nick_name: find_str(&value, "nick_name").or_else(|| find_str(&value, "user_name")),
    })
}

/// `POST /v2/drive/get` 的响应里我们关心的部分。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliyunDriveInfo {
    /// 盘 id。
    pub drive_id: Option<String>,
    /// 盘名（实测主盘叫 `Default`）。
    pub drive_name: Option<String>,
    /// 盘类型。
    pub drive_type: Option<String>,
    /// 状态。
    pub status: Option<String>,
}

/// 解析 `POST /v2/drive/get` 的响应。
pub fn aliyun_parse_drive_info(body: &str) -> Result<AliyunDriveInfo, RemoteError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| RemoteError::protocol("阿里云盘返回的网盘信息无法解析。"))?;

    if !value.is_object() {
        return Err(RemoteError::protocol("阿里云盘返回的网盘信息无法解析。"));
    }

    Ok(AliyunDriveInfo {
        drive_id: find_str(&value, "drive_id"),
        drive_name: find_str(&value, "drive_name"),
        drive_type: find_str(&value, "drive_type"),
        status: find_str(&value, "status"),
    })
}

/// UTC ISO8601 时间戳 → Unix 秒。
///
/// `expire_time` 是绝对过期时刻（实测形如 `2026-11-20T02:05:14Z`）。
/// 解析不出来返回 `None` —— 宁可不知道过期时间，也不要显示一个错的时间。
pub fn aliyun_expire_time_to_unix(text: &str) -> Option<i64> {
    crate::remote::aliyun_api::aliyun_iso8601_to_unix_millis(text).map(|millis| (millis / 1000) as i64)
}

/// 一个凭据是否已经过期。
pub fn aliyun_credentials_expired(expires_at_unix: Option<i64>, now_unix: i64) -> bool {
    match expires_at_unix {
        Some(expires_at) => now_unix >= expires_at,
        None => false,
    }
}

/// 错误文案里绝不能出现的东西 —— 用来给错误文案做一次自检。
///
/// 单元测试拿它守住「错误文案不含凭证」这条红线。
pub fn contains_secret(text: &str, secret: &str) -> bool {
    !secret.is_empty() && text.contains(secret)
}

// ===========================================================================
// 扫码登录：请求的地址与请求体
// ===========================================================================

/// 一次扫码登录的「客户端上下文」。
///
/// 2026-11-20 实测（证据 `.superpowers/sdd/b29-accept-run/key-probe-01.json`）：
/// 服务端**只按 `ck` 查会话** —— 假 `ck` + 真 `hsiz` ⇒ `EXPIRED`，
/// 真 `ck` + 假 `hsiz` ⇒ `NEW` ⇒ `hsiz` 不是查找键。
///
/// 但真实浏览器（证据 `b29-accept-run/real-qr-capture.json`）的 generate 与 query
/// 带的是**同一组**客户端标识（`_csrf_token` / `umidToken` / `hsiz`），
/// 且 query 里 `hsiz == ck` ⇒ 我们也照这个形状来：
/// **一次扫码会话只有一份上下文，generate 与 query 共用同一份**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliyunQrClientContext {
    /// 32 位 hex（会话标识）。
    pub hsiz: String,
    /// 40 位 hex（设备标识）。
    pub umid_token: String,
    /// 24 位 CSRF 令牌。
    pub csrf_token: String,
    /// 32 位 hex（链路追踪 id）。
    pub page_trace_id: String,
}

impl AliyunQrClientContext {
    /// 生成一份新的上下文。
    ///
    /// 全部是随机值，**不来自任何账号、不含任何凭据**。
    /// 随机源用 `uuid` 的 v4（已在 `Cargo.toml` 的 `[dependencies]` 里）⇒ 不新增依赖。
    pub fn random() -> Self {
        let a = Uuid::new_v4().simple().to_string();
        let b = Uuid::new_v4().simple().to_string();
        let c = Uuid::new_v4().simple().to_string();
        let d = Uuid::new_v4().simple().to_string();

        Self {
            hsiz: a.clone(),
            umid_token: format!("{a}{b}").chars().take(40).collect(),
            csrf_token: c.chars().take(24).collect(),
            page_trace_id: d,
        }
    }
}

/// 生成二维码时带的查询串。
///
/// 逐字照抄真实浏览器的请求（字段与顺序都照抄）：
/// `appName` / `fromSite` / `appEntrance` / `_csrf_token` / `umidToken` /
/// `hsiz` / `bizParams` / `mainPage` / `isMobile` / `lang` / `returnUrl` /
/// `umidTag`。
pub fn aliyun_qr_generate_query(context: &AliyunQrClientContext) -> String {
    format!(
        "appName=aliyun_drive&fromSite=52&appEntrance=web_default&_csrf_token={}&umidToken={}&hsiz={}&bizParams=taobaoBizLoginFrom%3Dweb_default&mainPage=false&isMobile=false&lang=zh_CN&returnUrl=&umidTag=SERVER",
        urlencoding::encode(&context.csrf_token),
        urlencoding::encode(&context.umid_token),
        urlencoding::encode(&context.hsiz),
    )
}

/// 轮询扫码状态时带的查询串。
pub fn aliyun_qr_query_query() -> String {
    "appName=aliyun_drive&fromSite=52".to_string()
}

/// 轮询扫码状态时带的表单体。
///
/// 逐字照抄真实浏览器的请求体（23 个字段，顺序也照抄）。
/// 真实浏览器里 `hsiz == ck` ⇒ 这里也这么填。
///
/// `nav_user_agent` 由调用方给（`remote::ALIYUN_USER_AGENT`）——
/// 本模块是纯函数层，不去依赖带网络栈的 `aliyun_provider`。
pub fn aliyun_qr_query_form(
    ck: &str,
    t: Option<i64>,
    context: &AliyunQrClientContext,
    nav_user_agent: &str,
) -> String {
    let mut pairs: Vec<(&str, String)> = Vec::new();

    if let Some(t) = t {
        pairs.push(("t", t.to_string()));
    }

    pairs.push(("ck", ck.to_string()));
    pairs.push(("ua", String::new()));
    pairs.push(("appName", "aliyun_drive".to_string()));
    pairs.push(("appEntrance", "web_default".to_string()));
    pairs.push(("_csrf_token", context.csrf_token.clone()));
    pairs.push(("umidToken", context.umid_token.clone()));
    pairs.push(("hsiz", ck.to_string()));
    pairs.push(("bizParams", "taobaoBizLoginFrom=web_default".to_string()));
    pairs.push(("mainPage", "false".to_string()));
    pairs.push(("isMobile", "false".to_string()));
    pairs.push(("lang", "zh_CN".to_string()));
    pairs.push(("returnUrl", String::new()));
    pairs.push(("fromSite", "52".to_string()));
    pairs.push(("umidTag", "SERVER".to_string()));
    pairs.push(("navlanguage", "zh-CN".to_string()));
    pairs.push(("navUserAgent", nav_user_agent.to_string()));
    pairs.push(("navPlatform", "Win32".to_string()));
    pairs.push(("isIframe", "false".to_string()));
    pairs.push(("documentReferer", String::new()));
    pairs.push(("defaultView", "qrcode".to_string()));
    pairs.push(("deviceId", String::new()));
    pairs.push(("pageTraceId", context.page_trace_id.clone()));

    pairs
        .into_iter()
        .map(|(key, value)| format!("{}={}", key, urlencoding::encode(&value)))
        .collect::<Vec<String>>()
        .join("&")
}

/// 供上层复用的错误类型别名（避免每个文件都写 `RemoteErrorKind`）。
pub type AliyunQrErrorKind = RemoteErrorKind;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_qr_requests_carry_the_verified_query_strings() {
        let context = AliyunQrClientContext::random();

        // 生成二维码的查询串逐字照抄真实浏览器。
        let generate = aliyun_qr_generate_query(&context);
        assert!(generate.contains("appName=aliyun_drive"));
        assert!(generate.contains("fromSite=52"));
        assert!(generate.contains("umidTag=SERVER"));
        assert!(generate.contains("_csrf_token="));
        assert!(generate.contains("umidToken="));
        assert!(generate.contains("hsiz="));
        assert!(generate.contains("bizParams=taobaoBizLoginFrom%3Dweb_default"));

        // 轮询的查询串 + 表单体。
        assert!(aliyun_qr_query_query().contains("appName=aliyun_drive"));
        let form = aliyun_qr_query_form("the-ck", Some(1_700_000_000_000), &context, "the-ua");
        assert!(form.contains("ck=the-ck"));
        assert!(form.contains("t=1700000000000"));
        // 真实浏览器里 hsiz == ck。
        assert!(form.contains("hsiz=the-ck"));
        assert!(form.contains("umidToken="));
        assert!(form.contains("navUserAgent=the-ua"));
        // 没给时间戳时就不带这个字段。
        // 用 "&t=" 判，别被 "navUserAgent=" 里的 "t=" 误伤。
        let without_t = aliyun_qr_query_form("the-ck", None, &context, "the-ua");
        assert!(!without_t.contains("&t="));
    }

    #[test]
    fn the_client_context_is_random_and_shaped_like_the_real_one() {
        let one = AliyunQrClientContext::random();
        let two = AliyunQrClientContext::random();

        // 长度必须与真实浏览器一致（抓包实测）。
        assert_eq!(one.hsiz.len(), 32);
        assert_eq!(one.umid_token.len(), 40);
        assert_eq!(one.csrf_token.len(), 24);
        assert_eq!(one.page_trace_id.len(), 32);

        // 两次生成必须不同 —— 否则两次扫码会共用同一份上下文。
        assert_ne!(one.hsiz, two.hsiz);
        assert_ne!(one.umid_token, two.umid_token);
        assert_ne!(one.csrf_token, two.csrf_token);
        assert_ne!(one.page_trace_id, two.page_trace_id);
    }

    #[test]
    fn the_qr_ticket_is_read_from_the_generate_response() {
        let body = r#"{"content":{"status":"S","success":true,
            "data":{"codeContent":"the-code-content","ck":"the-ck","t":1700000000000,
                    "resultCode":100,"processFinished":true}}}"#;

        let ticket = aliyun_parse_qr_ticket(body).expect("解析");

        assert_eq!(ticket.code_content, "the-code-content");
        assert_eq!(ticket.ck, "the-ck");
        assert_eq!(ticket.t, Some(1_700_000_000_000));
    }

    #[test]
    fn a_generate_response_without_a_code_content_is_rejected() {
        let error = aliyun_parse_qr_ticket(r#"{"content":{"data":{"ck":"x"}}}"#).expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Protocol);
    }

    #[test]
    fn the_poll_response_carries_the_status_and_the_biz_ext() {
        let body = r#"{"content":{"data":{"qrCodeStatus":"CONFIRMED","resultCode":100,
            "bizExt":"the-biz-ext"}}}"#;

        let poll = aliyun_parse_qr_poll(body).expect("解析");

        assert_eq!(poll.status, AliyunQrStatus::Confirmed);
        assert_eq!(poll.biz_ext.as_deref(), Some("the-biz-ext"));
    }

    #[test]
    fn the_status_machine_knows_its_own_states() {
        assert_eq!(AliyunQrStatus::parse("NEW"), AliyunQrStatus::New);
        assert_eq!(AliyunQrStatus::parse("scanned"), AliyunQrStatus::Scanned);
        assert_eq!(AliyunQrStatus::parse("CONFIRMED"), AliyunQrStatus::Confirmed);
        assert_eq!(AliyunQrStatus::parse("EXPIRED"), AliyunQrStatus::Expired);
        assert_eq!(AliyunQrStatus::parse("CANCELED"), AliyunQrStatus::Canceled);
        // 不认识的值 ⇒ 终态（别让界面永远转圈）。
        assert_eq!(AliyunQrStatus::parse("WHAT"), AliyunQrStatus::Error);
        assert!(AliyunQrStatus::Error.is_final());
        assert!(!AliyunQrStatus::New.is_final());
        assert!(!AliyunQrStatus::Scanned.is_final());
    }

    #[test]
    fn the_biz_ext_is_base64_json_with_the_credentials_inside() {
        // {"pds_login_result":{"accessToken":"at","refreshToken":"rt",
        //   "defaultDriveId":"15998870","expiresIn":7200,"expireTime":"2026-11-20T02:05:14Z",
        //   "userId":"u-1","nickName":"小明"}}
        let json = r#"{"pds_login_result":{"accessToken":"at","refreshToken":"rt","defaultDriveId":"15998870","expiresIn":7200,"expireTime":"2026-11-20T02:05:14Z","userId":"u-1","nickName":"小明"}}"#;
        let encoded = STANDARD.encode(json);

        let credentials = aliyun_decode_biz_ext(&encoded).expect("解码");

        assert_eq!(credentials.access_token, "at");
        assert_eq!(credentials.refresh_token.as_deref(), Some("rt"));
        assert_eq!(credentials.default_drive_id.as_deref(), Some("15998870"));
        assert_eq!(credentials.expires_in, Some(7200));
        assert_eq!(credentials.user_id.as_deref(), Some("u-1"));
        assert_eq!(credentials.nick_name.as_deref(), Some("小明"));
    }

    #[test]
    fn a_gbk_encoded_biz_ext_still_decodes_the_credentials() {
        // 实测（2026-11-20）：阿里云盘 `bizExt` 里那坨 JSON 的**中文是 GBK/GB18030**，
        // 不是 UTF-8 —— 严格 UTF-8 解码会整条失败（「扫码结果无法解码」）。
        // 凭据字段全是纯 ASCII，所以换编码只影响昵称这类展示字段。
        let json = r#"{"pds_login_result":{"accessToken":"at-gbk","defaultDriveId":"15998870","nickName":"小明"}}"#;
        let (bytes, _, _) = encoding_rs::GB18030.encode(json);
        let encoded = STANDARD.encode(bytes.as_ref());

        let credentials = aliyun_decode_biz_ext(&encoded).expect("GBK 也要能解");

        assert_eq!(credentials.access_token, "at-gbk");
        assert_eq!(credentials.default_drive_id.as_deref(), Some("15998870"));
        assert_eq!(credentials.nick_name.as_deref(), Some("小明"));
    }

    #[test]
    fn a_biz_ext_without_an_access_token_is_an_auth_problem() {
        let encoded = STANDARD.encode(r#"{"pds_login_result":{"userId":"u-1"}}"#);

        let error = aliyun_decode_biz_ext(&encoded).expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    #[test]
    fn a_garbage_biz_ext_is_an_auth_problem() {
        let error = aliyun_decode_biz_ext("这不是 base64").expect_err("必须失败");

        assert_eq!(error.kind, RemoteErrorKind::Auth);
    }

    /// 淘系回传的 `bizExt` 是 **URL-safe base64**（`-` / `_`）。
    /// 2026-11-20 真机实测：Rust 的严格 `STANDARD.decode` 直接失败（2852 字符的凭据块
    /// 解不出来），而当初 b28 取证用的是 Node 的宽松解码 ⇒ 同一份数据两边结论相反。
    /// 这条用例把「严格口径必然失败、宽松口径必须成功」钉死。
    #[test]
    fn a_url_safe_biz_ext_decodes_even_though_the_strict_decoder_cannot() {
        // 先造一段标准 base64 里**一定**含 `+` / `/` 的载荷，
        // 再把字母表换成 URL-safe —— 只有这样「严格解码必然失败」才成立。
        //
        // 注意（2026-11-20 修复）：base64 的 3 字节组里，只有第 3 个字节
        // （`byte2 & 0x3F`）可能落进 62 / 63 ⇒ 只有载荷里出现 `>` (0x3E)、
        // `?` (0x3F) 这类字符才可能吐出 `+` / `/`；纯 ASCII 且以 `}}` 结尾的
        // JSON **永远**吐不出来（`}` = 0x7D → 61）。
        // 原来这里是 `while` + 追加 `x` ⇒ **无限循环**，把整个 `cargo test --lib`
        // 挂死（该用例跑 >60 秒不退出）。改成**有界**循环 + 追加 `?`
        // （实测第 1 步就命中 `/`）。
        let mut token = "at".to_string();
        let mut encoded = STANDARD
            .encode(format!(r#"{{"pds_login_result":{{"accessToken":"{token}"}}}}"#));
        let mut attempts = 0usize;
        while !(encoded.contains('+') || encoded.contains('/')) {
            token.push('?');
            encoded = STANDARD
                .encode(format!(r#"{{"pds_login_result":{{"accessToken":"{token}"}}}}"#));
            attempts += 1;
            assert!(attempts < 16, "这条用例的前提（base64 必须含 + 或 /）不再成立");
        }
        let url_safe = encoded.replace('+', "-").replace('/', "_");

        assert!(
            STANDARD.decode(&url_safe).is_err(),
            "这条用例必须真的落在「严格解码失败」的口径上，否则它证明不了任何东西"
        );

        let credentials = aliyun_decode_biz_ext(&url_safe).expect("URL-safe base64 必须能解出来");
        assert_eq!(credentials.access_token, token);
    }

    /// 页面把 `bizExt` 当对象回传时，钩子会 `JSON.stringify` ⇒ 收到的是原始 JSON。
    #[test]
    fn a_json_biz_ext_decodes_without_base64() {
        let credentials = aliyun_decode_biz_ext(
            r#"{"pds_login_result":{"accessToken":"at-json","refreshToken":"rt-json"}}"#,
        )
        .expect("原始 JSON 必须能解出来");

        assert_eq!(credentials.access_token, "at-json");
        assert_eq!(credentials.refresh_token.as_deref(), Some("rt-json"));
    }

    #[test]
    fn the_base64_normalizer_maps_the_url_safe_alphabet_and_pads() {
        // `ab-_cd` 归一化成 `ab+/cd` 之后是 6 个字符，不是合法 base64 长度
        // （必须是 4 的倍数），所以归一化器要补成 `ab+/cd==`，
        // `STANDARD.decode` 才收得下 —— 这正是它存在的意义。
        assert_eq!(aliyun_normalize_base64("ab-_cd"), "ab+/cd==");
        assert_eq!(aliyun_normalize_base64("QQ"), "QQ==");
        assert_eq!(aliyun_normalize_base64("QUJD"), "QUJD");
    }

    #[test]
    fn the_credentials_debug_never_leaks_the_token() {
        let credentials = AliyunQrCredentials {
            access_token: "the-access-token-value".to_string(),
            refresh_token: Some("the-refresh-token-value".to_string()),
            default_drive_id: Some("15998870".to_string()),
            expires_in: Some(7200),
            expire_time: None,
            user_id: None,
            nick_name: None,
        };

        let text = format!("{credentials:?}");

        assert!(!text.contains("the-access-token-value"));
        assert!(!text.contains("the-refresh-token-value"));
        assert!(text.contains("access_token(len="));
        assert!(text.contains("sha16="));
    }

    #[test]
    fn the_user_profile_is_read_leniently() {
        let body = r#"{"default_drive_id":"15998870","default_sbox_drive_id":"25998870",
                        "user_id":"u-1","nick_name":"小明"}"#;

        let profile = aliyun_parse_user_profile(body).expect("解析");

        assert_eq!(profile.default_drive_id.as_deref(), Some("15998870"));
        assert_eq!(profile.default_sbox_drive_id.as_deref(), Some("25998870"));
        assert_eq!(profile.user_id.as_deref(), Some("u-1"));
        assert_eq!(profile.nick_name.as_deref(), Some("小明"));
    }

    #[test]
    fn the_user_profile_is_also_found_behind_a_data_wrapper() {
        let body = r#"{"code":"Success","data":{"default_drive_id":"15998870"}}"#;

        let profile = aliyun_parse_user_profile(body).expect("解析");

        assert_eq!(profile.default_drive_id.as_deref(), Some("15998870"));
    }

    #[test]
    fn the_drive_info_is_read_leniently() {
        let body = r#"{"drive_id":"15998870","drive_name":"Default","drive_type":"user",
                        "status":"available"}"#;

        let info = aliyun_parse_drive_info(body).expect("解析");

        assert_eq!(info.drive_id.as_deref(), Some("15998870"));
        assert_eq!(info.drive_name.as_deref(), Some("Default"));
    }

    #[test]
    fn the_expiry_moment_becomes_unix_seconds() {
        let unix = aliyun_expire_time_to_unix("2026-11-20T02:05:14Z").expect("解析");

        assert!(unix > 0);
        assert_eq!(aliyun_expire_time_to_unix("不是时间"), None);
    }

    #[test]
    fn an_expiry_in_the_past_means_expired() {
        assert!(aliyun_credentials_expired(Some(100), 100));
        assert!(aliyun_credentials_expired(Some(100), 101));
        assert!(!aliyun_credentials_expired(Some(100), 99));
        // 不知道过期时间 ⇒ 不认为过期。
        assert!(!aliyun_credentials_expired(None, 999));
    }
}
