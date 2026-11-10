// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// ---------------------------------------------------------------------------
// 夸克网盘 Web API 的纯函数层
//
// 红线（与 `remote/mod.rs:8-18`、`baidu_api.rs:8-14` 同一条）：本文件被
// `cargo test --lib` 的测试二进制链接，**不得出现 `tauri::AppHandle`、
// `tauri::async_runtime::*` 或 `tauri::` 的任何路径**。一旦碰到，Tauri 全局
// 运行时的默认类型参数 `Wry` 被单态化，把 tao 整条窗口链拖进测试二进制；
// 测试二进制没有 application manifest 声明 comctl32 v6，`TaskDialogIndirect`
// 解析不到，进程在加载阶段以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉 ——
// 一个测试都跑不了，且报错与本模块毫无关系。
//
// **本轮（T1）刻意不含任何网络请求代码**：没有 `reqwest`、没有 `tokio::net`、
// 没有 `send()` / `client.execute()`。原因是方案 §5 阶段 0 的 **T0 闸门**
// 尚未产出结论 —— 「HTTP 请求由 Rust 直接发」还是「在 WebView2 内 fetch、
// Rust 只解析」这两条路线还没定。T0 的结论会决定请求由谁发出，但**不会**
// 改变本文件里的任何东西：常量、错误码映射、响应解析、分页判据、参数构造
// 在两条路线下完全一致。传输层等 T0 结论后再加，届时只动 HTTP 那一段，
// 不动这里。
//
// 接口来源说明（**不是官方开放平台**，也没有官方开放平台）：夸克没有面向
// 第三方「浏览用户整个网盘」的公开 API，本文件走的是网页端 / PC 客户端自己
// 使用的内部 REST 接口，形态与社区通行实现一致。依据见
// `docs/plan-2026-11-11-quark-pan.md`：
//   - §3.2.2 列表调用：`GET https://drive-pc.quark.cn/1/clouddrive/file/sort`
//     → `{"code":0,"data":{"list":[...],"metadata":{...}}}`
//   - §3.2.4 错误码表（模块 887411 `ERROR_TIPS`，另有一份副本多 3 个码）
//   - §3.2.5 下载语义：`GET /1/clouddrive/file/download?fid={fid}`
//     → 带 `auth_key` / `Expires` 的直链
//   - §3.2.6 条目字段：`fid` / `pdir_fid` / `name` / `file_name` / `size` /
//     `format_type` / `file_type` / `download_url` / `thumbnail` / `updated_at` …
//   - §4.6 API 层契约、§4.7 错误映射表（穷尽）
//
// ⚠️ **本文件里有三处「未实证」的取值**，都做成了**单一常量 / 单一函数**，
// T0 实测后只改那一处即可，调用方不受影响：
//   1. `QUARK_DEFAULT_TIME_UNIT` —— `updated_at` 是秒还是毫秒（§7-U8 未决）。
//   2. `QUARK_DEFAULT_SIZE_UNIT` —— `size` 是字节还是 KB（§7-U8 未决）。
//   3. `QUARK_FILE_TYPE_DIRECTORY` —— `file_type` 的哪一值代表目录
//      （§3.2.6 只列了字段名，没给取值语义）。
//
// 未实证的部分在本文件里逐处标注「未实证」；调用方不得把它们当成已证结论。
// ---------------------------------------------------------------------------

use crate::remote::error::{RemoteError, RemoteErrorKind};
use crate::remote::path::{build_remote_path, join_remote_parent_and_name};
use crate::remote::provider::RemoteEntry;
use crate::remote::sftp_provider::is_hidden_remote_name;
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::time::Duration;

// ===========================================================================
// 常量
// ===========================================================================

/// 夸克网盘网页端的主机名。
///
/// 账号信息（昵称）在**这个**主机上取，列表 / 下载在 `drive-pc.quark.cn` 上。
pub const QUARK_PAN_HOST: &str = "pan.quark.cn";

/// 夸克网盘 PC 端接口的根地址（列表、下载直链都走这里）。
pub const QUARK_DRIVE_BASE_URL: &str = "https://drive-pc.quark.cn";

/// 客户端标识（本地实证的 query 参数）。
///
/// ⚠️ **它不是 OAuth `client_id`** —— 夸克没有面向第三方的开放平台，这个
/// `533` 只是网页端 / PC 客户端自己带的一个 query 参数。
pub const QUARK_CLIENT_ID: &str = "533";

/// 产品线标识（`ucpro`）。
pub const QUARK_PR: &str = "ucpro";

/// 客户端来源标识。Windows 客户端固定为 `win`（`ucparams.fr`：
/// `darwin ? "mac" : "win"`，模块 780721）。
///
/// ⚠️ 开源脚本用 `pc`，两边都被广泛使用；§7-U5 要求 T0 做 A/B 对照。
/// 本轮按**真实客户端**取 `win`。
pub const QUARK_FR: &str = "win";

/// 操作系统标识（客户端取 `process.platform`，Windows 上是 `win32`）。
pub const QUARK_SYS: &str = "win32";

/// 客户端的固定参数串（模块 860782 `UC_PARAMS_STR`）。
///
/// 一个字符都不能改：服务端按它区分「来自官方客户端的请求」。
pub const QUARK_UC_PARAM_STR: &str = "dsdnfrpfbivesscpgimibtbmnijblauputogpintnwktprchmt";

/// 列表的默认页大小，同时是本轮采用的**上限**（超过会被服务端截断）。
///
/// 客户端自己用的是 `_size = 100`（产物 @5312702）。⚠️ 服务端的真实上限
/// §7-U9 未定 ⇒ 这里**必须**用 [`clamp_page_limit`] 夹住 `_size`，否则
/// 服务端静默截断会让目录漏条目而且**不报错**（风险 R4）。
pub const QUARK_LIST_PAGE_SIZE: u32 = 100;

/// 列表的默认排序串。
///
/// ⚠️ §7-U13：排序字段的具体取值**以 T0 实测为准**，不得当成既定契约。
/// 默认取方案 §4.6 给的 `file_name:asc,updated_at:desc`。
pub const QUARK_LIST_SORT: &str = "file_name:asc,updated_at:desc";

/// 会话 cookie 名：账号登录态，等价于密码。
///
/// ⚠️ **仅开源项目侧证据**（§7-U1）：本地客户端的 renderer **从未按名字**
/// 引用 `__pus`，它是「整罐 cookie + `withCredentials`」模型（§3.2.7）。
/// 本常量只用于「从整罐里读出这一个值」的场合；发请求时仍发整罐。
pub const QUARK_SESSION_COOKIE_PUS: &str = "__pus";

/// 请求 `User-Agent`。
///
/// ⚠️ **未实证**：真实客户端的 UA 形如
/// `... QuarkPC/1.2.3.4 quark-cloud-drive/1.2.3.4` —— 客户端从 UA 里正则
/// 抠版本号（`QuarkPC/(\d+\.\d+\.\d+\.\d+)`，模块 407716）。这里的版本号是
/// **占位值**，T0 抓到真机 UA 后替换。`ve` 参数必须与本 UA 里的
/// `QuarkPC/x.y.z.w` 一致 —— 这一点由
/// [`quark_client_version_from_user_agent`] + 单测钉死。
pub const QUARK_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 \
QuarkPC/1.0.0.0 quark-cloud-drive/1.0.0.0";

/// `ve` 参数（客户端版本号）。
///
/// ⚠️ **未实证**（占位值），且**必须**与 [`QUARK_USER_AGENT`] 里的
/// `QuarkPC/x.y.z.w` 一致。
pub const QUARK_CLIENT_VERSION: &str = "1.0.0.0";

/// 单次 HTTP 请求的超时。
///
/// 客户端自己用 10 秒（axios `timeout: 1e4`），但它有长连接；Rust 侧保守放宽。
pub const QUARK_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// 建立下载连接的超时。
///
/// 与请求超时分开：连不上要**立刻**放弃，而下载慢是正常的。
pub const QUARK_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 下载一个文件的整体超时。
///
/// 比 [`QUARK_OPERATION_TIMEOUT`] 宽得多：整份文件要进内存，30 秒的请求
/// 超时对几百 MiB 的文件是必然超时的。
pub const QUARK_FILE_TIMEOUT: Duration = Duration::from_secs(900);

/// 列一个目录的整体超时。
///
/// 一个大目录要翻很多页，每页各是一次请求。这一层的意义是「别把界面卡死」。
pub const QUARK_DIRECTORY_TIMEOUT: Duration = Duration::from_secs(120);

/// 单个文件最大下载多少字节（500 MiB）。
///
/// 人为的闸：超过就明确报「不支持」，而不是把机器的内存吃光。
///
/// ⚠️ 夸克下载直链**自带 30 分钟提前过期**（§3.2.5 / 风险 R6），超大文件
/// 风险更高；阈值不得调高。
pub const QUARK_MAX_DOWNLOAD_BYTES: u64 = 500 * 1024 * 1024;

/// 下载直链的**提前刷新窗口**（30 分钟，单位：秒）。
///
/// 客户端的判定是 `Date.now() >= 1000 * (exp - 1800)` —— 提前 30 分钟就当
/// 过期，避免下载到一半 403（风险 R6）。
pub const QUARK_DOWNLOAD_REFRESH_WINDOW_SECONDS: u64 = 1800;

// ===========================================================================
// 单位换算
// ===========================================================================

/// 夸克时间戳字段（`updated_at`）的单位。
///
/// ⚠️ §7-U8 **未决**：本地 bundle 只做展示，没见权威定义；猜错的后果是
/// 「按修改时间排序」在远程目录下变成一片随机顺序，而且**不会报错**
/// （风险 R5）。所以这里**不硬编码成唯一一种解释**，而是把两条路径都做成
/// 纯函数，单测各自锁定输出。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarkTimeUnit {
    /// 按**秒**解释（×1000）。
    Seconds,
    /// 按**毫秒**解释（原样）。
    Milliseconds,
}

/// T0 未定前，`updated_at` 按哪种单位解释。
///
/// **默认按秒**（§7-U8 写的是「`updated_at` 暂按秒」）。T0 用已知文件核对后
/// 只改这一行，全仓行为跟着变。
pub const QUARK_DEFAULT_TIME_UNIT: QuarkTimeUnit = QuarkTimeUnit::Seconds;

/// 夸克体积字段（`size`）的单位。
///
/// ⚠️ §7-U8 **未决**：`size` 是字节还是 KB 未定（客户端另有 `file_size`
/// 字段，§3.2.6）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarkSizeUnit {
    /// 按**字节**解释（原样）。
    Bytes,
    /// 按 **KiB** 解释（×1024）。
    Kibibytes,
}

/// T0 未定前，`size` 按哪种单位解释。
///
/// **默认按字节**（§7-U8 写的是「`size` 暂按字节」）。T0 用已知大小的文件
/// 核对后只改这一行。
pub const QUARK_DEFAULT_SIZE_UNIT: QuarkSizeUnit = QuarkSizeUnit::Bytes;

/// 秒 → Unix 纪元毫秒。
///
/// `RemoteEntry.modified_time` 的约定是 **Unix 纪元毫秒**（见 `provider.rs`
/// 的字段注释）。忘了 ×1000 的后果是「按修改时间排序」在远程目录下变成一片
/// 随机顺序 —— 而且**不会报错**。
///
/// 与 `baidu_api::baidu_seconds_to_unix_millis` 同一件事，只是来源不同
/// （那边是 xpan 的 `server_mtime`，这边是夸克的 `updated_at`）。
pub fn quark_seconds_to_unix_millis(seconds: u64) -> u64 {
    seconds.saturating_mul(1_000)
}

/// 毫秒 → Unix 纪元毫秒（恒等）。
///
/// 存在的意义不是「做点什么」，而是让「按毫秒解释」这条路径**也是一条被单测
/// 锁定的显式路径** —— §7-U8 未决前不能把它悄悄折进同一个函数里。
pub fn quark_millis_to_unix_millis(millis: u64) -> u64 {
    millis
}

/// 按指定单位把夸克的时间戳字段折成 Unix 纪元毫秒。
///
/// 这是**唯一**的换算入口：调用方要么显式给单位，要么用
/// [`QUARK_DEFAULT_TIME_UNIT`]。
pub fn quark_timestamp_to_unix_millis(raw: u64, unit: QuarkTimeUnit) -> u64 {
    match unit {
        QuarkTimeUnit::Seconds => quark_seconds_to_unix_millis(raw),
        QuarkTimeUnit::Milliseconds => quark_millis_to_unix_millis(raw),
    }
}

/// 按指定单位把夸克的体积字段折成字节。
///
/// 存在的理由同 [`quark_timestamp_to_unix_millis`]：§7-U8 未决前，「字节」
/// 与「KB」两种解释都要是**显式且被测试锁定**的路径。
pub fn quark_size_to_bytes(size: u64, unit: QuarkSizeUnit) -> u64 {
    match unit {
        QuarkSizeUnit::Bytes => size,
        QuarkSizeUnit::Kibibytes => size.saturating_mul(1024),
    }
}

// ===========================================================================
// 会话
// ===========================================================================

/// 夸克网盘的会话凭据。
///
/// 装的是 **`.quark.cn` 域的整罐 cookie**（`name=value; name2=value2`），
/// 不是单个 `__pus` —— §3.2.7 实测本地客户端是「整罐 cookie +
/// `withCredentials`」模型，renderer 从不按名字挑 cookie；社区说的
/// 「只需要 `__pus`」只是 B 层单方证据（§7-U1）。
///
/// **安全等级等同于密码**：拿到它就能冒充用户。写盘（走 DPAPI 密文）、
/// 日志、错误信息里都不得出现明文 —— 因此这里手写了 `Debug`（不 derive），
/// 只打印长度不打印内容。
#[derive(Clone, PartialEq, Eq)]
pub struct QuarkSession {
    cookie_jar: String,
}

impl QuarkSession {
    /// 新建一个会话。`cookie_jar` 是 `.quark.cn` 域的**整罐** cookie。
    pub fn new(cookie_jar: impl Into<String>) -> Self {
        Self {
            cookie_jar: cookie_jar.into(),
        }
    }

    /// 整罐 cookie 的原值。**只在拼 Cookie 头时用**，不要拿去打日志。
    pub fn cookie_jar(&self) -> &str {
        &self.cookie_jar
    }

    /// 是否至少有一个可用的 cookie。
    pub fn has_credentials(&self) -> bool {
        !self.cookie_jar.trim().is_empty()
    }

    /// 从整罐里读出某个名字的 cookie 值。名字不存在或值非法时返回 `None`。
    ///
    /// ⚠️ 这是「**读**某个 cookie」的便利函数，**不是**发请求时的 cookie
    /// 集合 —— 发请求发整罐（见 [`QuarkSession::cookie_header`]）。
    pub fn cookie_value(&self, name: &str) -> Option<String> {
        for part in self.cookie_jar.split(';') {
            let (key, value) = part.split_once('=')?;
            if key.trim() != name {
                continue;
            }
            return sanitize_cookie_value(value);
        }
        None
    }

    /// 拼出 `Cookie:` 头的值。
    ///
    /// 整罐缺失或**任一**条目格式非法 ⇒ `None`（调用方据此报「会话失效 /
    /// 未授权」）。
    ///
    /// 为什么不是「挑掉坏的、留好的」：坏条目只有两种来源 —— **值被截断**
    /// 或**有人想注入头部**。两种都说明这罐 cookie 不可信，发一个「少了几条」
    /// 的头过去，只会把「会话失效」变成一个更难查的错。
    pub fn cookie_header(&self) -> Option<String> {
        sanitize_cookie_jar(&self.cookie_jar)
    }

    /// 给日志用的**脱敏**描述：只有长度，没有内容。
    pub fn describe(&self) -> String {
        format!("cookie-jar(len={})", self.cookie_jar.len())
    }
}

/// 手写的 `Debug`：**只打印长度，绝不打印明文**。
impl fmt::Debug for QuarkSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "QuarkSession({})", self.describe())
    }
}

/// 校验并清洗**一个** cookie 的值。
///
/// RFC 6265 的 cookie-value 不允许 CTLs、空白、`"`、`,`、`;`、`\`。这几个字符
/// 出现在会话 cookie 里只有两种可能：**值被截断了**或者**有人想注入头部**
/// —— 两种都不能原样发出去。所以这里不是「过滤」而是**判非法**：含这些字符
/// 就返回 `None`，宁可让调用方报「会话失效」，也不发一个语义不明的头。
///
/// （与 `baidu_api::sanitize_cookie_value` 同款 —— 照抄。）
pub fn sanitize_cookie_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let forbidden = trimmed.chars().any(|character| {
        character.is_control()
            || character.is_whitespace()
            || matches!(character, '"' | ',' | ';' | '\\')
    });
    if forbidden {
        return None;
    }

    Some(trimmed.to_string())
}

/// 校验并清洗**一整罐** cookie（`name=value; name2=value2`）。
///
/// 返回规范化后的罐（用 `; ` 分隔）；空罐或任一条目非法 ⇒ `None`。
///
/// 条目名只允许 cookie-token 的字符集（RFC 6265 §4.1.1），值走
/// [`sanitize_cookie_value`]。
pub fn sanitize_cookie_jar(raw: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    for segment in raw.split(';') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        let (name, value) = segment.split_once('=')?;
        let name = name.trim();
        if !is_cookie_token(name) {
            return None;
        }

        let value = sanitize_cookie_value(value)?;
        parts.push(format!("{name}={value}"));
    }

    if parts.is_empty() {
        return None;
    }

    Some(parts.join("; "))
}

/// 是不是合法的 cookie 名（cookie-token）。
fn is_cookie_token(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '!' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | '-' | '.' | '^' | '_' | '`' | '|' | '~')
        })
}

// ===========================================================================
// URL 拼装
// ===========================================================================

/// 百分号编码一个 query 组件。
///
/// 只保留 `A-Z` / `a-z` / `0-9` / `-` / `_` / `.` / `~`；**斜杠也会被编码**
/// （`/` → `%2F`），因为网盘里的 id / 主机名是 query 值的一部分而不是 URL 路径。
///
/// （与 `baidu_api::encode_query_component` 同款 —— 照抄。）
pub fn encode_query_component(raw: &str) -> String {
    urlencoding::encode(raw).into_owned()
}

/// 列目录：`/1/clouddrive/file/sort`。
///
/// 参数形态照方案 §4.6（**已实证**），顺序与那一条逐字一致：
///
/// ```text
/// GET https://drive-pc.quark.cn/1/clouddrive/file/sort
///     ?pdir_fid=0&_page=1&_size=100&_fetch_total=1&_fetch_sub_dirs=0
///     &_sort=file_name:asc,updated_at:desc
///     &pr=ucpro&fr=win&sys=win32&ve={UA 版本}&uc_param_str={固定串}
///     &client_id=533
///     &device_model={本机主机名}&ut={QUARK_WSGUID}
/// ```
///
/// ⚠️ `device_model` 与 `ut` **不能省**（评审 M1）：真实客户端的
/// `driveRequest` 确有 request 拦截器，注入 `device_model`（来自
/// `getDeviceNameSync()`，即**本机主机名**）与 `ut`（来自
/// `window.QUARK_WSGUID`）。少了这两个参数，请求被拒会被误读成「路线不成立」。
///
/// ⚠️ `device_model` 会把**本机主机名**带出 ⇒ 属敏感项，不落盘、不进日志
/// （§6.4）。
///
/// `pdir_fid` 是**父目录的 fid**，根目录为 `0`。
pub fn quark_list_dir_url(
    pdir_fid: &str,
    page: u32,
    size: u32,
    device_model: &str,
    ut: &str,
) -> String {
    format!(
        "{QUARK_DRIVE_BASE_URL}/1/clouddrive/file/sort\
         ?pdir_fid={pdir_fid}\
         &_page={page}\
         &_size={size}\
         &_fetch_total=1\
         &_fetch_sub_dirs=0\
         &_sort={sort}\
         &pr={pr}\
         &fr={fr}\
         &sys={sys}\
         &ve={ve}\
         &uc_param_str={uc}\
         &client_id={client_id}\
         &device_model={device_model}\
         &ut={ut}",
        pdir_fid = encode_query_component(pdir_fid),
        page = page.max(1),
        size = clamp_page_limit(size),
        sort = QUARK_LIST_SORT,
        pr = QUARK_PR,
        fr = QUARK_FR,
        sys = QUARK_SYS,
        ve = QUARK_CLIENT_VERSION,
        uc = QUARK_UC_PARAM_STR,
        client_id = QUARK_CLIENT_ID,
        device_model = encode_query_component(device_model),
        ut = encode_query_component(ut),
    )
}

/// 取账号信息：`https://pan.quark.cn/account/info?fr=pc&platform=pc`。
///
/// 取 `data.nickname`，用来在授权卡上回填「已授权：<昵称>」。
///
/// ⚠️ 这个端点的 `code` 是**字符串** `"OK"`，不是数字 `0`（**已实测**）。
/// 解析器对两种都接受，见 [`parse_account_info_response`]。
pub fn quark_account_info_url() -> String {
    format!("https://{QUARK_PAN_HOST}/account/info?fr=pc&platform=pc")
}

/// 取下载直链：`/1/clouddrive/file/download?fid={fid}`。
///
/// 返回体里带 `auth_key` / `Expires` 的 `download_url`；直链本身要**显式带
/// Cookie 头**去下载（客户端就是这么干的，§3.2.5）。
///
/// ⚠️ **未实证**：§4.6 只给了这条路径的形态。直链是否也需要 `pr` / `fr` 等
/// 基础参数、T0 补测后在本函数里补齐即可 —— 传输层还没写，现在改成本最低。
pub fn quark_download_url(fid: &str) -> String {
    format!(
        "{QUARK_DRIVE_BASE_URL}/1/clouddrive/file/download?fid={}",
        encode_query_component(fid)
    )
}

/// 从 `User-Agent` 里抠出客户端版本号（`QuarkPC/x.y.z.w`）。
///
/// 客户端自己就是这么干的（模块 407716 `getQuarkPCVersion()`）。抠不出来时
/// 返回 `None` —— 调用方据此决定 `ve` 填什么，**不猜**。
pub fn quark_client_version_from_user_agent(user_agent: &str) -> Option<String> {
    let tail = user_agent.split("QuarkPC/").nth(1)?;
    let version: String = tail
        .chars()
        .take_while(|character| {
            character.is_ascii_digit() || *character == '.'
        })
        .collect();

    // 必须是四段 `x.y.z.w`，否则视为抠不到。
    let segments = version.split('.').count();
    if segments != 4 || version.ends_with('.') {
        return None;
    }

    Some(version)
}

// ===========================================================================
// 分页
// ===========================================================================

/// 一页列表的取数范围。
///
/// 夸克用的是**页码**（`_page`，从 1 开始），不是百度那种偏移量（`_start`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuarkListPage {
    /// 页码，**从 1 开始**。
    pub page: u32,
    /// 本页最多取多少条（`_size`）。
    pub size: u32,
}

impl QuarkListPage {
    /// 第一页。
    pub fn first() -> Self {
        Self {
            page: 1,
            size: QUARK_LIST_PAGE_SIZE,
        }
    }
}

/// 把请求的页大小夹到服务端允许的范围。
///
/// 0 ⇒ 用默认页大小（同时也是本轮采用的上限 100）；超过 100 ⇒ 截到 100。
///
/// 为什么要夹：服务端会**静默截断**超限的 `_size`，返回 100 条却不报错。
/// 不夹的话分页会漏条目 —— 目录里第 101 条之后的内容永远看不到，而且
/// **不会报错**（风险 R4）。
pub fn clamp_page_limit(limit: u32) -> u32 {
    if limit == 0 {
        return QUARK_LIST_PAGE_SIZE;
    }
    limit.min(QUARK_LIST_PAGE_SIZE)
}

/// 算下一页。返回 `None` 表示**已经取完**。
///
/// 判据是「本页收到的条数 < size」⇒ 服务端没有更多了。
///
/// ⚠️ 边界：恰好收满 `size` 条时会**多发一次**请求（那一页会是空的）。
/// 这是刻意的 —— 接口没有可靠的 `has_more` 字段（`_fetch_total` 给的总数
/// 未经实测），少一条的目录与正好整页的目录无法从返回值上区分，硬猜会漏条目。
/// **宁可多一次请求，不可漏条目。**
pub fn next_page(page: &QuarkListPage, received: usize) -> Option<QuarkListPage> {
    if received < page.size as usize {
        return None;
    }
    if received == 0 {
        return None;
    }

    let next = page.page.checked_add(1)?;
    Some(QuarkListPage {
        page: next,
        size: page.size,
    })
}

// ===========================================================================
// 响应结构
// ===========================================================================

/// 列表里的一个条目。
///
/// ⚠️ 字段名与类型按 §3.2.6 / §4.6 写；`size` / `updated_at` 的**单位**未定
/// （§7-U8），用 [`quark_size_to_bytes`] / [`quark_timestamp_to_unix_millis`]
/// 显式折算。
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct QuarkFileEntry {
    /// 文件 id。取下载直链要它。
    #[serde(default)]
    pub fid: String,
    /// 父目录的 fid。根目录是 `0`。
    #[serde(default)]
    pub pdir_fid: String,
    /// 文件名（不含目录）。
    #[serde(default)]
    pub file_name: String,
    /// 文件名（另一个字段名；`file_name` 缺失时用它）。
    #[serde(default)]
    pub name: String,
    /// 体积。⚠️ **单位未定**（§7-U8）。
    #[serde(default, deserialize_with = "deserialize_lenient_u64")]
    pub size: u64,
    /// 条目类型。⚠️ **哪一值代表目录未定**（见 [`QUARK_FILE_TYPE_DIRECTORY`]）。
    #[serde(default, deserialize_with = "deserialize_lenient_i64")]
    pub file_type: i64,
    /// 文件格式类型。
    #[serde(default, deserialize_with = "deserialize_lenient_i64")]
    pub format_type: i64,
    /// 服务器修改时间。⚠️ **秒还是毫秒未定**（§7-U8）。
    #[serde(default, deserialize_with = "deserialize_lenient_u64")]
    pub updated_at: u64,
    /// 缩略图地址。
    #[serde(default)]
    pub thumbnail: Option<String>,
    /// 下载直链（列表响应里一般没有，取直链要走下载接口）。
    #[serde(default)]
    pub download_url: Option<String>,
    /// 分享用的文件令牌。
    #[serde(default)]
    pub share_fid_token: Option<String>,
    /// 命名空间。
    #[serde(default)]
    pub name_space: Option<String>,
}

/// 账号信息。
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct QuarkUserInfo {
    /// 昵称。⚠️ 字段形状仍待验（§7-U12），取不到时为空串（UI 降级成
    /// 「昵称未知」），**不报错**。
    #[serde(default)]
    pub nickname: String,
}

/// 一页列表的解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarkListPayload {
    /// 本页条目。
    pub entries: Vec<QuarkFileEntry>,
    /// 服务端给的条目总数（`metadata` 里）。
    ///
    /// ⚠️ `metadata` 的字段名**未实证**，本实现依次试
    /// `_total` / `total` / `total_count` / `count`，都取不到就是 `None`。
    /// 拿它做「翻页数对不对」的交叉校验（风险 R4），拿不到就退回「按条数判
    /// 断」这一条路。
    pub total: Option<u64>,
}

/// 下载直链。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarkDownloadLink {
    /// 直链本身。
    pub download_url: String,
    /// 签名参数（部分响应里单独给）。
    pub auth_key: Option<String>,
    /// 过期时间（Unix 秒）。
    pub expires: Option<u64>,
}

/// 响应里的 `code` 字段的形态。
#[derive(Debug, Clone, PartialEq, Eq)]
enum QuarkCode {
    /// 成功：数字 `0` 或字符串 `"OK"`。
    Success,
    /// 失败：非 0 的数字错误码。
    Number(i64),
    /// 失败：一个**不是** `"OK"` 的字符串 —— 视为协议改版。
    Text(String),
}

// ===========================================================================
// 响应解析（纯函数）
// ===========================================================================

/// 解析列表响应。
///
/// `code != 0` ⇒ 走 [`failure_from_code`]；**绝不**把失败响应解析成一个空
/// 列表冒充成功。响应不是合法 JSON 或缺 `code` ⇒ [`QuarkFailure::Malformed`]
/// （分类到 `RemoteErrorKind::Protocol`）。
pub fn parse_list_response(body: &str) -> Result<Vec<QuarkFileEntry>, QuarkFailure> {
    parse_list_payload(body).map(|payload| payload.entries)
}

/// 解析列表响应，并带回 `metadata` 里的总数。
///
/// 形态：`{"code":0,"data":{"list":[...],"metadata":{...}}}`。
pub fn parse_list_payload(body: &str) -> Result<QuarkListPayload, QuarkFailure> {
    let root: serde_json::Value =
        serde_json::from_str(body).map_err(|_| QuarkFailure::Malformed)?;

    check_code(root.get("code"))?;

    let data = root.get("data").ok_or(QuarkFailure::Malformed)?;
    // 正常形态是 `data.list`；`data` 本身就是数组时也接受（容错，不报错）。
    let list = data.get("list").unwrap_or(data);
    let entries: Vec<QuarkFileEntry> =
        serde_json::from_value(list.clone()).map_err(|_| QuarkFailure::Malformed)?;

    Ok(QuarkListPayload {
        entries,
        total: metadata_total(data.get("metadata")),
    })
}

/// 解析账号信息响应。
///
/// ⚠️ 这个端点的 `code` 是**字符串** `"OK"`（**已实测**），不是数字 `0`。
/// 两种都接受。
pub fn parse_account_info_response(body: &str) -> Result<QuarkUserInfo, QuarkFailure> {
    let root: serde_json::Value =
        serde_json::from_str(body).map_err(|_| QuarkFailure::Malformed)?;

    check_code(root.get("code"))?;

    let data = root.get("data").ok_or(QuarkFailure::Malformed)?;
    let info: QuarkUserInfo =
        serde_json::from_value(data.clone()).map_err(|_| QuarkFailure::Malformed)?;

    Ok(info)
}

/// 解析下载直链响应。
///
/// ⚠️ **未实证**：§4.6 只说「返回带 `auth_key` / `Expires` 的
/// `download_url`」。本实现容许 `data` 是对象或对象数组，取第一个带
/// `download_url` 的。
pub fn parse_download_response(body: &str) -> Result<QuarkDownloadLink, QuarkFailure> {
    let root: serde_json::Value =
        serde_json::from_str(body).map_err(|_| QuarkFailure::Malformed)?;

    check_code(root.get("code"))?;

    let data = root.get("data").ok_or(QuarkFailure::Malformed)?;
    let item = match data {
        serde_json::Value::Array(items) => items.first().ok_or(QuarkFailure::Malformed)?,
        _ => data,
    };

    let download_url = item
        .get("download_url")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(QuarkFailure::Malformed)?
        .to_string();

    let auth_key = item
        .get("auth_key")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);

    let expires = item
        .get("expires")
        .and_then(value_as_u64)
        .or_else(|| expiry_from_url(&download_url));

    Ok(QuarkDownloadLink {
        download_url,
        auth_key,
        expires,
    })
}

/// 判定下载直链是否已过期。
///
/// 客户端的判定是 `Date.now() >= 1000 * (exp - 1800)` —— **提前 30 分钟**
/// 就当过期，避免下载到一半 403（风险 R6）。
pub fn quark_download_is_expired(expires_unix_seconds: u64, now_unix_millis: u64) -> bool {
    let refresh_at_millis = expires_unix_seconds
        .saturating_sub(QUARK_DOWNLOAD_REFRESH_WINDOW_SECONDS)
        .saturating_mul(1_000);
    now_unix_millis >= refresh_at_millis
}

/// 校验响应里的 `code`。
///
/// 规则：
/// - 缺 `code` ⇒ [`QuarkFailure::Malformed`]（⇒ `Protocol`）
/// - 数字 `0` ⇒ 成功
/// - 数字非 `0` ⇒ [`failure_from_code`]
/// - 字符串 `"OK"` ⇒ 成功（`/account/info` **已实测**就是它）
/// - 其它字符串 ⇒ [`QuarkFailure::Protocol`]（协议改版 / 风控，风险 R2）
/// - 其它类型 ⇒ [`QuarkFailure::Malformed`]
fn check_code(code: Option<&serde_json::Value>) -> Result<(), QuarkFailure> {
    let Some(value) = code else {
        return Err(QuarkFailure::Malformed);
    };

    match value {
        serde_json::Value::Number(number) => {
            let code = number.as_i64().ok_or(QuarkFailure::Malformed)?;
            if code == 0 {
                return Ok(());
            }
            Err(failure_from_code(code))
        }
        serde_json::Value::String(text) => {
            if text.trim() == "OK" {
                return Ok(());
            }
            Err(QuarkFailure::Protocol)
        }
        _ => Err(QuarkFailure::Malformed),
    }
}

/// 从 `metadata` 里取条目总数。字段名未实证 ⇒ 依次试几个候选。
fn metadata_total(metadata: Option<&serde_json::Value>) -> Option<u64> {
    let metadata = metadata?;

    for key in ["_total", "total", "total_count", "count"] {
        if let Some(total) = metadata.get(key).and_then(value_as_u64) {
            return Some(total);
        }
    }

    None
}

/// 从下载直链的 query 里取过期时间（`Expires=` 或 `auth_key=`）。
fn expiry_from_url(url: &str) -> Option<u64> {
    let query = url.split_once('?')?.1;

    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if key.eq_ignore_ascii_case("Expires") || key.eq_ignore_ascii_case("auth_key") {
            if let Some(seconds) = value_as_u64(&serde_json::Value::String(value.to_string())) {
                return Some(seconds);
            }
        }
    }

    None
}

/// 把一个 JSON 值读成 `u64`（接受数字与数字字符串）。
fn value_as_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

/// 容错地反序列化一个 `u64`（接受数字与数字字符串）。
fn deserialize_lenient_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    value_as_u64(&value).ok_or_else(|| {
        <D::Error as serde::de::Error>::custom("夸克返回的数值字段既不是数字也不是数字字符串")
    })
}

/// 容错地反序列化一个 `i64`（接受数字与数字字符串）。
fn deserialize_lenient_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match &value {
        serde_json::Value::Number(number) => number.as_i64().ok_or_else(|| {
            <D::Error as serde::de::Error>::custom("夸克返回的数值字段超出 i64 范围")
        }),
        serde_json::Value::String(text) => text.trim().parse::<i64>().map_err(|_| {
            <D::Error as serde::de::Error>::custom("夸克返回的数值字段不是合法整数")
        }),
        _ => Err(<D::Error as serde::de::Error>::custom(
            "夸克返回的数值字段类型不对",
        )),
    }
}

// ===========================================================================
// 条目 → RemoteEntry
// ===========================================================================

/// `file_type` 里代表**目录**的值。
///
/// ⚠️ **未实证**：§3.2.6 只列了字段名，没给取值语义；社区实现普遍按
/// `file_type == 0` 判目录。**只改这一个常量**就能翻转全仓行为，T0 真机
/// 核对后修正即可。
pub const QUARK_FILE_TYPE_DIRECTORY: i64 = 0;

/// 这个条目是不是目录。
pub fn quark_entry_is_directory(entry: &QuarkFileEntry) -> bool {
    entry.file_type == QUARK_FILE_TYPE_DIRECTORY
}

/// 条目的名字。`file_name` 为空时退回 `name`。
///
/// ⚠️ 夸克的条目**没有路径**（只有 `fid`），所以路径得由调用方拿「当前目录
/// 的路径」拼 —— 见 [`to_remote_entry`]。
pub fn quark_entry_name(entry: &QuarkFileEntry) -> Option<String> {
    if !entry.file_name.trim().is_empty() {
        return Some(entry.file_name.clone());
    }
    if !entry.name.trim().is_empty() {
        return Some(entry.name.clone());
    }

    None
}

/// 把一个夸克条目映射成本项目的 `RemoteEntry`。
///
/// `dir_path` 是**当前正在列举的目录**的远端路径（根目录传 `"/"`）。
///
/// ⚠️ **为什么路径要由调用方给**：百度侧条目自带完整 `path`（`/01A_软件/…`），
/// 而夸克侧只有 `fid` / `pdir_fid` —— 服务端**不返回路径**。所以路径只能由
/// 「父目录路径 + 条目名」拼出来，父目录路径只有调用方知道。
///
/// 返回 `None` 表示这条数据没法用（名字取不到、或路径构造不出来）——
/// **跳过它，而不是造一条半截数据**。
pub fn to_remote_entry(
    connection_id: &str,
    dir_path: &str,
    entry: &QuarkFileEntry,
) -> Option<RemoteEntry> {
    let name = quark_entry_name(entry)?;

    // 条目名里带分隔符 ⇒ 它其实是个路径片段，直接拼会造出越界的路径。
    if name.contains('/') || name.contains('\\') {
        return None;
    }

    let remote_path = join_remote_parent_and_name(dir_path, &name);
    let path = build_remote_path(connection_id, &remote_path)?;

    Some(RemoteEntry {
        is_hidden: is_hidden_remote_name(&name),
        is_symlink: false,
        is_dir: quark_entry_is_directory(entry),
        size: quark_size_to_bytes(entry.size, QUARK_DEFAULT_SIZE_UNIT),
        modified_time: quark_timestamp_to_unix_millis(entry.updated_at, QUARK_DEFAULT_TIME_UNIT),
        name,
        path,
    })
}

/// 映射一整页。映射不出来的条目被跳过。
pub fn to_remote_entries(
    connection_id: &str,
    dir_path: &str,
    entries: &[QuarkFileEntry],
) -> Vec<RemoteEntry> {
    entries
        .iter()
        .filter_map(|entry| to_remote_entry(connection_id, dir_path, entry))
        .collect()
}

// ===========================================================================
// 错误分类
// ===========================================================================

/// 夸克网盘侧的失败原因。
///
/// 这是**协议无关的语义分类**，不是 `RemoteErrorKind` 的副本 ——
/// 两者之间由 [`classify_quark_failure`] 一张穷尽表连接。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarkFailure {
    /// 会话失效 / 未登录（`31001` / `31004`）。
    Auth,
    /// 已登录但没有权限，或内容违规（`14014` / `22001`）。
    Permission,
    /// 目标不存在（`14013` / `21001` / `25008`）。
    NotFound,
    /// 参数或协议层被拒（`14012` / `23008` / `24005` / `43004` / `25005`），
    /// 以及「响应里的 `code` 是个不是 `"OK"` 的字符串」这一类**协议改版**。
    Protocol,
    /// 网络不可达：DNS 失败、连接被拒、超时、`14001`-`14010`。
    Unreachable,
    /// 连上了但中途断开。
    Disconnected,
    /// 响应不是合法 JSON，或结构与预期不符（含缺 `code`）。
    Malformed,
    /// 响应体不是合法 UTF-8（文件名编码解不开）。
    Encoding,
    /// 服务端说这个操作不支持，或触到了人为闸
    /// （`23018` / `38001` / `25004` / `32004` / `32003` / `32005`）。
    Unsupported,
    /// 未在映射表里列出的错误码。**必须带上原始 code** —— 否则用户只看到
    /// 「未知错误」，开发侧也无从定位是哪个码。
    Unknown { code: i64 },
    /// 未预期的 HTTP 状态码（非 2xx）。
    UnexpectedStatus { status: u16 },
}

/// 把夸克侧的失败原因翻译成本项目的错误分类。
///
/// 这是一张**穷尽**映射：每个 [`QuarkFailure`] 变体都必须落到一个
/// `RemoteErrorKind`，漏一个就会让某类失败退化成「笼统错误」。
pub fn classify_quark_failure(failure: &QuarkFailure) -> RemoteErrorKind {
    match failure {
        QuarkFailure::Auth => RemoteErrorKind::Auth,
        QuarkFailure::Permission => RemoteErrorKind::Permission,
        QuarkFailure::NotFound => RemoteErrorKind::NotFound,
        QuarkFailure::Protocol => RemoteErrorKind::Protocol,
        QuarkFailure::Unreachable => RemoteErrorKind::Unreachable,
        QuarkFailure::Disconnected => RemoteErrorKind::Disconnected,
        // 「响应无法解析」在 `RemoteErrorKind::Protocol` 的定义里就是协议层错误。
        QuarkFailure::Malformed => RemoteErrorKind::Protocol,
        QuarkFailure::Encoding => RemoteErrorKind::Encoding,
        QuarkFailure::Unsupported => RemoteErrorKind::Unsupported,
        QuarkFailure::Unknown { .. } => RemoteErrorKind::Unknown,
        QuarkFailure::UnexpectedStatus { .. } => RemoteErrorKind::Unknown,
    }
}

/// 把夸克的 `code` 翻成失败原因。
///
/// 逐条对应方案 §4.7 的穷尽表（该表的依据是本地客户端的 `ERROR_TIPS`
/// 错误码表，模块 887411，另有一份副本多出 `23018` / `25008` / `38001`
/// 三个码 —— 评审 M7 要求补齐，本表已补齐）：
///
/// | code | 含义 | 分类 |
/// |---|---|---|
/// | `31001` / `31004` | 请重新登录 / 会话失效 | `Auth` |
/// | `14013` / `21001` / `25008` | 文件不存在 / 已失效 | `NotFound` |
/// | `14014` / `22001` | 违规 / 含违规信息 | `Permission` |
/// | `14012` | 文件名非法 | `Protocol` |
/// | `23008` | 重命名同名冲突 | `Protocol` |
/// | `24005` | 文件/文件夹状态已更新，请返回重试 | `Protocol` |
/// | `43004` | 本地时间差异大 | `Protocol` |
/// | `25005` | 解析失败 | `Protocol` |
/// | `23018` / `38001` | 文件不支持下载 | `Unsupported` |
/// | `25004` / `32004` / `32003` / `32005` | 体积/空间/次数超限 | `Unsupported` |
/// | `14001`-`14010` | 网络 / 数据异常 | `Unreachable` |
/// | 其余 | 未列出 | `Unknown`（**带原始 code**）|
///
/// ⚠️ `31004` 与 `31001` **同等对待**（客户端里是
/// `if ([31001, 31004].includes(r.code) && ...user.logout())`）—— 会话已失效
/// 却提示「未知错误」是最糟的一种退化，所以两条都映射到 `Auth`。
pub fn failure_from_code(code: i64) -> QuarkFailure {
    match code {
        31001 | 31004 => QuarkFailure::Auth,
        14013 | 21001 | 25008 => QuarkFailure::NotFound,
        14014 | 22001 => QuarkFailure::Permission,
        14012 | 23008 | 24005 | 43004 | 25005 => QuarkFailure::Protocol,
        23018 | 38001 | 25004 | 32004 | 32003 | 32005 => QuarkFailure::Unsupported,
        14001..=14010 => QuarkFailure::Unreachable,
        other => QuarkFailure::Unknown { code: other },
    }
}

/// 把 HTTP 状态码翻成失败原因。
///
/// 3xx 归到 `Auth`：会话失效时夸克会把请求**重定向到登录页**，而我们
/// **刻意不跟随重定向**，就是为了把这件事变成一条可分类的「请重新授权」，
/// 而不是拿到一个登录页 HTML 再报「响应无法解析」。
pub fn failure_from_http_status(status: u16) -> QuarkFailure {
    match status {
        200..=299 => QuarkFailure::UnexpectedStatus { status },
        301 | 302 | 303 | 307 | 308 => QuarkFailure::Auth,
        401 | 403 => QuarkFailure::Auth,
        404 => QuarkFailure::NotFound,
        408 | 429 => QuarkFailure::Unreachable,
        500..=599 => QuarkFailure::Unreachable,
        _ => QuarkFailure::Protocol,
    }
}

/// 传输层失败的形态（纯数据，不依赖任何 HTTP 库，便于离线单测）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QuarkTransportShape {
    /// 连接阶段失败（DNS、握手、拒绝连接）。
    pub connect: bool,
    /// 超时。
    pub timeout: bool,
    /// 响应体读了一半断了。
    pub body: bool,
    /// 请求构造/发送阶段失败。
    pub request: bool,
}

/// 把传输层失败分类。
///
/// 纯函数形态：单测不碰网络。连接失败与超时都归 `Unreachable`（§4.7 末行：
/// 「连接超时 / DNS 失败 → 网络不可达」）。
pub fn classify_transport(shape: QuarkTransportShape) -> RemoteErrorKind {
    if shape.connect || shape.timeout {
        return RemoteErrorKind::Unreachable;
    }
    if shape.body {
        return RemoteErrorKind::Disconnected;
    }
    if shape.request {
        return RemoteErrorKind::Protocol;
    }
    RemoteErrorKind::Unknown
}

/// 一行「这个错误分类在夸克网盘上能不能出现」的说明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarkErrorKindCoverage {
    pub kind: RemoteErrorKind,
    /// 本协议**能否**产出这个分类。
    pub producible: bool,
    /// 不能的话，为什么。**必须非空** —— 「不适用」也要说清楚为什么不适用。
    pub note: &'static str,
}

/// `RemoteErrorKind` 全部 11 个变体逐个表态的**穷尽表**。
///
/// 为什么要显式写出来：`RemoteErrorKind` 有 11 个变体，而夸克网盘只涉及其中的
/// 9 个。剩下两个（`HostKeyUnknown` / `HostKeyChanged`）是 **SSH 主机密钥**
/// 概念 —— 夸克网盘走 HTTPS + 系统信任库，没有「这台主机你信不信」这回事，
/// 所以**永远不会被本协议产出**。
///
/// 把这件事写成一张表而不是靠注释，是为了让「漏了哪个分类」变成一条会红的
/// 断言 —— 将来 `RemoteErrorKind` 加了新变体，这张表会立刻不完整。
pub fn quark_error_kind_coverage() -> Vec<QuarkErrorKindCoverage> {
    vec![
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::Auth,
            producible: true,
            note: "会话失效：code 31001 / 31004，或 3xx 重定向到登录页，或 HTTP 401/403。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::HostKeyUnknown,
            producible: false,
            note: "不适用：这是 SSH 主机密钥概念；夸克网盘走 HTTPS + 系统信任库，没有主机指纹可确认。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::HostKeyChanged,
            producible: false,
            note: "不适用：同上；TLS 证书由系统信任库校验，证书链异常表现为连接失败（Unreachable）。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::Unreachable,
            producible: true,
            note: "网络不可达：连接失败、超时、被限流（429）、服务端 5xx，或 code 14001-14010。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::Permission,
            producible: true,
            note: "已登录但没有权限：code 14014 / 22001（违规 / 含违规信息）。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::NotFound,
            producible: true,
            note: "目标不存在：code 14013 / 21001 / 25008，或 HTTP 404。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::Protocol,
            producible: true,
            note: "协议层错误：code 14012 / 23008 / 24005 / 43004 / 25005，响应无法解析，或 code 是个非 \"OK\" 的字符串（协议改版）。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::Encoding,
            producible: true,
            note: "响应体不是合法 UTF-8（文件名编码解不开）。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::Disconnected,
            producible: true,
            note: "连上了但响应体读了一半断了。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::Unsupported,
            producible: true,
            note: "服务端不支持或触到人为闸：code 23018 / 38001（不支持下载）、25004 / 32004 / 32003 / 32005（体积/空间/次数超限）。",
        },
        QuarkErrorKindCoverage {
            kind: RemoteErrorKind::Unknown,
            producible: true,
            note: "兜底：未在错误码表里列出的 code，或未预期的 HTTP 状态码。",
        },
    ]
}

/// 把失败原因翻成给用户看的错误。
///
/// 文案一律简体中文，且**不含任何凭据内容**。
///
/// ⚠️ 落到 `Unknown` 的那条**必须带上原始 code** —— 否则用户只看到「未知
/// 错误」，开发侧也无从定位是哪个码。
pub fn to_remote_error(failure: &QuarkFailure) -> RemoteError {
    let message = match failure {
        QuarkFailure::Auth => "夸克网盘会话已失效，请重新授权。".to_string(),
        QuarkFailure::Permission => "夸克网盘拒绝了这次操作（权限不足或内容违规）。".to_string(),
        QuarkFailure::NotFound => "夸克网盘上没有这个文件或目录。".to_string(),
        QuarkFailure::Protocol => "夸克网盘接口拒绝了这次请求（参数或协议层错误）。".to_string(),
        QuarkFailure::Unreachable => "连不上夸克网盘服务器。".to_string(),
        QuarkFailure::Disconnected => "与夸克网盘的连接中途断开了。".to_string(),
        QuarkFailure::Malformed => "夸克网盘返回了无法解析的响应。".to_string(),
        QuarkFailure::Encoding => "夸克网盘返回的内容编码无法解析。".to_string(),
        QuarkFailure::Unsupported => "这个操作在夸克网盘上暂不支持。".to_string(),
        QuarkFailure::Unknown { code } => {
            format!("夸克网盘操作失败（未识别的错误码 {code}）。")
        }
        QuarkFailure::UnexpectedStatus { status } => {
            format!("夸克网盘返回了未预期的响应状态（HTTP {status}）。")
        }
    };

    RemoteError::new(classify_quark_failure(failure), message)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §4.6 里那份列表响应的形态（照抄方案，不是构造出来的）。
    const OBSERVED_LIST_BODY: &str = r#"{"code":0,"data":{"list":[
        {"fid":"1001","pdir_fid":"0","file_name":"01A_软件","name":"01A_软件","size":0,"file_type":0,"format_type":0,"updated_at":1700000000,"thumbnail":"https://t/thumb.jpg","download_url":"","share_fid_token":"tok","name_space":"/"},
        {"fid":"1002","pdir_fid":"0","file_name":"设计素材.zip","name":"设计素材.zip","size":2048,"file_type":1,"format_type":0,"updated_at":1700000060,"thumbnail":"","download_url":"https://d/dl.zip","share_fid_token":"","name_space":"/"}
    ],"metadata":{"_total":2}}}"#;

    // ---- 常量 ----------------------------------------------------------

    #[test]
    fn the_fixed_client_parameters_match_the_observed_ones() {
        assert_eq!(QUARK_PAN_HOST, "pan.quark.cn");
        assert_eq!(QUARK_DRIVE_BASE_URL, "https://drive-pc.quark.cn");
        assert_eq!(QUARK_CLIENT_ID, "533");
        assert_eq!(QUARK_PR, "ucpro");
        assert_eq!(QUARK_FR, "win");
        assert_eq!(QUARK_SYS, "win32");
        assert_eq!(QUARK_SESSION_COOKIE_PUS, "__pus");
    }

    #[test]
    fn the_uc_param_string_is_the_observed_fixed_one() {
        assert_eq!(
            QUARK_UC_PARAM_STR,
            "dsdnfrpfbivesscpgimibtbmnijblauputogpintnwktprchmt"
        );
    }

    #[test]
    fn the_timeouts_are_the_ones_the_plan_lists() {
        assert_eq!(QUARK_OPERATION_TIMEOUT, Duration::from_secs(30));
        assert_eq!(QUARK_CONNECT_TIMEOUT, Duration::from_secs(30));
        assert_eq!(QUARK_FILE_TIMEOUT, Duration::from_secs(900));
        assert_eq!(QUARK_DIRECTORY_TIMEOUT, Duration::from_secs(120));
        assert_eq!(QUARK_MAX_DOWNLOAD_BYTES, 500 * 1024 * 1024);
    }

    #[test]
    fn the_download_refresh_window_is_thirty_minutes() {
        assert_eq!(QUARK_DOWNLOAD_REFRESH_WINDOW_SECONDS, 1800);
    }

    // ---- URL 拼装 ------------------------------------------------------

    #[test]
    fn the_list_url_matches_the_shape_in_the_plan() {
        let url = quark_list_dir_url("0", 1, 100, "DESKTOP-ABC", "1234567890");

        assert_eq!(
            url,
            "https://drive-pc.quark.cn/1/clouddrive/file/sort\
             ?pdir_fid=0\
             &_page=1\
             &_size=100\
             &_fetch_total=1\
             &_fetch_sub_dirs=0\
             &_sort=file_name:asc,updated_at:desc\
             &pr=ucpro\
             &fr=win\
             &sys=win32\
             &ve=1.0.0.0\
             &uc_param_str=dsdnfrpfbivesscpgimibtbmnijblauputogpintnwktprchmt\
             &client_id=533\
             &device_model=DESKTOP-ABC\
             &ut=1234567890"
                .replace("             ", "")
        );
    }

    #[test]
    fn the_list_url_carries_the_parent_fid() {
        let url = quark_list_dir_url("9876543210", 1, 100, "PC", "ut");

        assert!(url.contains("pdir_fid=9876543210"), "{url}");
    }

    #[test]
    fn the_list_url_encodes_the_parent_fid() {
        let url = quark_list_dir_url("a/b", 1, 100, "PC", "ut");

        assert!(url.contains("pdir_fid=a%2Fb"), "{url}");
    }

    #[test]
    fn the_list_url_carries_the_device_model_and_the_ut_parameter() {
        // 评审 M1：这两个参数不能省，少了会被服务端拒。
        let url = quark_list_dir_url("0", 1, 100, "DESKTOP-ABC", "WSGUID-1");

        assert!(url.contains("device_model=DESKTOP-ABC"), "{url}");
        assert!(url.contains("ut=WSGUID-1"), "{url}");
    }

    #[test]
    fn the_list_url_encodes_the_device_model() {
        let url = quark_list_dir_url("0", 1, 100, "MY PC", "ut");

        assert!(url.contains("device_model=MY%20PC"), "{url}");
    }

    #[test]
    fn the_list_url_clamps_an_oversized_page_size() {
        let url = quark_list_dir_url("0", 1, 5_000, "PC", "ut");

        assert!(url.contains("_size=100"), "{url}");
        assert!(!url.contains("_size=5000"), "{url}");
    }

    #[test]
    fn the_list_url_never_asks_for_page_zero() {
        let url = quark_list_dir_url("0", 0, 100, "PC", "ut");

        assert!(url.contains("_page=1"), "{url}");
    }

    #[test]
    fn the_list_url_keeps_the_sort_expression_verbatim() {
        // 排序串里的 `:` 与 `,` 是协议的一部分，不编码。
        let url = quark_list_dir_url("0", 1, 100, "PC", "ut");

        assert!(url.contains("_sort=file_name:asc,updated_at:desc"), "{url}");
    }

    #[test]
    fn the_account_info_url_matches_the_observed_one() {
        assert_eq!(
            quark_account_info_url(),
            "https://pan.quark.cn/account/info?fr=pc&platform=pc"
        );
    }

    #[test]
    fn the_download_url_encodes_the_fid() {
        let url = quark_download_url("1002");

        assert_eq!(
            url,
            "https://drive-pc.quark.cn/1/clouddrive/file/download?fid=1002"
        );
    }

    #[test]
    fn the_download_url_encodes_an_unusual_fid() {
        let url = quark_download_url("a b/c");

        assert!(url.contains("fid=a%20b%2Fc"), "{url}");
    }

    #[test]
    fn query_components_keep_only_unreserved_characters() {
        assert_eq!(encode_query_component("aZ0-_.~"), "aZ0-_.~");
        assert_eq!(encode_query_component("/"), "%2F");
        assert_eq!(encode_query_component("a b"), "a%20b");
    }

    #[test]
    fn the_client_version_is_read_out_of_the_user_agent() {
        assert_eq!(
            quark_client_version_from_user_agent(QUARK_USER_AGENT),
            Some("1.0.0.0".to_string())
        );
    }

    #[test]
    fn the_user_agent_and_the_ve_parameter_agree() {
        // `ve` 必须与 UA 里的 `QuarkPC/x.y.z.w` 一致，否则服务端看到的像是
        // 一个自相矛盾的客户端。
        assert_eq!(
            quark_client_version_from_user_agent(QUARK_USER_AGENT),
            Some(QUARK_CLIENT_VERSION.to_string())
        );
    }

    #[test]
    fn a_user_agent_without_the_quark_family_has_no_version() {
        assert_eq!(
            quark_client_version_from_user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"),
            None
        );
    }

    // ---- 分页 ----------------------------------------------------------

    #[test]
    fn the_page_size_is_clamped_to_the_server_maximum() {
        assert_eq!(clamp_page_limit(10), 10);
        assert_eq!(clamp_page_limit(100), 100);
        assert_eq!(clamp_page_limit(100_000), 100);
    }

    #[test]
    fn a_zero_page_size_falls_back_to_the_default() {
        assert_eq!(clamp_page_limit(0), QUARK_LIST_PAGE_SIZE);
    }

    #[test]
    fn a_short_page_means_the_listing_is_complete() {
        let page = QuarkListPage::first();

        assert_eq!(next_page(&page, 3), None);
    }

    #[test]
    fn a_full_page_advances_the_page_number() {
        let page = QuarkListPage::first();

        assert_eq!(
            next_page(&page, 100),
            Some(QuarkListPage {
                page: 2,
                size: 100
            })
        );
    }

    #[test]
    fn a_full_page_deliberately_asks_for_one_more_empty_request() {
        // 刻意行为：宁可多发一次请求，不可漏条目。
        let page = QuarkListPage::first();

        assert!(next_page(&page, 100).is_some());
    }

    #[test]
    fn the_next_page_keeps_the_page_size() {
        let page = QuarkListPage {
            page: 3,
            size: 50,
        };

        assert_eq!(
            next_page(&page, 50),
            Some(QuarkListPage {
                page: 4,
                size: 50
            })
        );
    }

    #[test]
    fn an_empty_page_ends_the_listing() {
        let page = QuarkListPage::first();

        assert_eq!(next_page(&page, 0), None);
    }

    #[test]
    fn a_page_number_overflow_ends_the_listing() {
        let page = QuarkListPage {
            page: u32::MAX,
            size: 100,
        };

        assert_eq!(next_page(&page, 100), None);
    }

    #[test]
    fn the_first_page_starts_at_one_with_the_default_size() {
        let page = QuarkListPage::first();

        assert_eq!(page.page, 1);
        assert_eq!(page.size, QUARK_LIST_PAGE_SIZE);
    }

    // ---- 响应解析 ------------------------------------------------------

    #[test]
    fn a_successful_list_response_yields_the_entries() {
        let entries = parse_list_response(OBSERVED_LIST_BODY).expect("parse");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].fid, "1001");
        assert_eq!(entries[0].file_name, "01A_软件");
        assert_eq!(entries[1].size, 2048);
        assert_eq!(entries[1].updated_at, 1700000060);
    }

    #[test]
    fn a_list_response_reads_every_documented_field() {
        let entries = parse_list_response(OBSERVED_LIST_BODY).expect("parse");
        let entry = &entries[1];

        assert_eq!(entry.pdir_fid, "0");
        assert_eq!(entry.name, "设计素材.zip");
        assert_eq!(entry.file_type, 1);
        assert_eq!(entry.format_type, 0);
        assert_eq!(entry.thumbnail.as_deref(), Some(""));
        assert_eq!(
            entry.download_url.as_deref(),
            Some("https://d/dl.zip")
        );
        assert_eq!(entry.share_fid_token.as_deref(), Some(""));
        assert_eq!(entry.name_space.as_deref(), Some("/"));
    }

    #[test]
    fn a_list_response_with_code_ok_string_is_also_successful() {
        // `/account/info` 实测就是字符串 "OK"；列表端点若哪天也这样，
        // 解析器不能因此崩掉。
        let body = r#"{"code":"OK","data":{"list":[],"metadata":{}}}"#;

        assert!(parse_list_response(body).is_ok());
    }

    #[test]
    fn a_list_response_with_a_nonzero_code_is_a_failure() {
        let body = r#"{"code":31001,"data":{"list":[],"metadata":{}}}"#;

        assert!(parse_list_response(body).is_err());
    }

    #[test]
    fn a_failed_list_response_never_comes_back_as_an_empty_list() {
        // 反例：失败响应**不得**被解析成空列表冒充成功。
        let body = r#"{"code":31001,"data":{"list":[],"metadata":{}}}"#;

        let parsed = parse_list_response(body);

        assert!(parsed.is_err());
    }

    #[test]
    fn a_list_response_that_is_not_json_is_a_protocol_failure() {
        let error = parse_list_response("<html>登录</html>").expect_err("must fail");

        assert_eq!(classify_quark_failure(&error), RemoteErrorKind::Protocol);
    }

    #[test]
    fn a_list_response_without_code_is_a_protocol_failure() {
        let body = r#"{"data":{"list":[],"metadata":{}}}"#;

        let error = parse_list_response(body).expect_err("must fail");

        assert_eq!(classify_quark_failure(&error), RemoteErrorKind::Protocol);
    }

    #[test]
    fn a_list_response_with_a_text_code_is_a_protocol_failure() {
        // 不是 "OK" 的字符串 ⇒ 协议改版（风险 R2）。
        let body = r#"{"code":"RISK_CONTROL","data":{"list":[]}}"#;

        let error = parse_list_response(body).expect_err("must fail");

        assert_eq!(classify_quark_failure(&error), RemoteErrorKind::Protocol);
    }

    #[test]
    fn a_list_response_with_a_null_code_is_a_protocol_failure() {
        let body = r#"{"code":null,"data":{"list":[]}}"#;

        let error = parse_list_response(body).expect_err("must fail");

        assert_eq!(classify_quark_failure(&error), RemoteErrorKind::Protocol);
    }

    #[test]
    fn a_list_response_without_data_is_a_protocol_failure() {
        let body = r#"{"code":0}"#;

        let error = parse_list_response(body).expect_err("must fail");

        assert_eq!(classify_quark_failure(&error), RemoteErrorKind::Protocol);
    }

    #[test]
    fn a_list_response_with_an_unparsable_size_is_a_protocol_failure() {
        let body = r#"{"code":0,"data":{"list":[{"fid":"1","size":"很大"}]}}"#;

        let error = parse_list_response(body).expect_err("must fail");

        assert_eq!(classify_quark_failure(&error), RemoteErrorKind::Protocol);
    }

    #[test]
    fn a_list_entry_accepts_a_numeric_string_size() {
        let body = r#"{"code":0,"data":{"list":[{"fid":"1","size":"2048"}]}}"#;

        let entries = parse_list_response(body).expect("parse");

        assert_eq!(entries[0].size, 2048);
    }

    #[test]
    fn a_list_entry_tolerates_missing_fields() {
        let body = r#"{"code":0,"data":{"list":[{"fid":"1"}]}}"#;

        let entries = parse_list_response(body).expect("parse");

        assert_eq!(entries[0].fid, "1");
        assert_eq!(entries[0].size, 0);
        assert_eq!(entries[0].file_name, "");
        assert_eq!(entries[0].thumbnail, None);
    }

    #[test]
    fn the_list_payload_reports_the_total_from_metadata() {
        let payload = parse_list_payload(OBSERVED_LIST_BODY).expect("parse");

        assert_eq!(payload.total, Some(2));
    }

    #[test]
    fn the_list_payload_has_no_total_when_metadata_is_absent() {
        let body = r#"{"code":0,"data":{"list":[]}}"#;

        let payload = parse_list_payload(body).expect("parse");

        assert_eq!(payload.total, None);
    }

    #[test]
    fn an_empty_but_successful_list_is_ok() {
        let body = r#"{"code":0,"data":{"list":[],"metadata":{"_total":0}}}"#;

        let entries = parse_list_response(body).expect("parse");

        assert!(entries.is_empty());
    }

    #[test]
    fn the_account_info_response_yields_the_nickname() {
        let body = r#"{"code":"OK","data":{"nickname":"山人"}}"#;

        let info = parse_account_info_response(body).expect("parse");

        assert_eq!(info.nickname, "山人");
    }

    #[test]
    fn the_account_info_response_accepts_a_numeric_zero_code() {
        let body = r#"{"code":0,"data":{"nickname":"山人"}}"#;

        assert!(parse_account_info_response(body).is_ok());
    }

    #[test]
    fn the_account_info_response_degrades_to_an_empty_nickname() {
        // 昵称取不到时**不报错**：UI 有「昵称未知」的降级路径（§7-U12）。
        let body = r#"{"code":"OK","data":{}}"#;

        let info = parse_account_info_response(body).expect("parse");

        assert_eq!(info.nickname, "");
    }

    #[test]
    fn the_account_info_response_without_data_is_a_protocol_failure() {
        let body = r#"{"code":"OK"}"#;

        let error = parse_account_info_response(body).expect_err("must fail");

        assert_eq!(classify_quark_failure(&error), RemoteErrorKind::Protocol);
    }

    #[test]
    fn the_account_info_response_with_an_auth_code_is_an_auth_failure() {
        let body = r#"{"code":31001,"data":{"nickname":"山人"}}"#;

        let error = parse_account_info_response(body).expect_err("must fail");

        assert_eq!(classify_quark_failure(&error), RemoteErrorKind::Auth);
    }

    #[test]
    fn the_download_response_yields_the_download_url() {
        let body = r#"{"code":0,"data":[{"download_url":"https://dl-c-sz-u.drive.quark.cn/x","auth_key":"k","expires":1700000000}]}"#;

        let link = parse_download_response(body).expect("parse");

        assert_eq!(link.download_url, "https://dl-c-sz-u.drive.quark.cn/x");
        assert_eq!(link.auth_key.as_deref(), Some("k"));
        assert_eq!(link.expires, Some(1700000000));
    }

    #[test]
    fn the_download_response_accepts_an_object_payload() {
        let body = r#"{"code":0,"data":{"download_url":"https://dl/x"}}"#;

        let link = parse_download_response(body).expect("parse");

        assert_eq!(link.download_url, "https://dl/x");
    }

    #[test]
    fn the_download_response_without_a_url_is_a_protocol_failure() {
        let body = r#"{"code":0,"data":[{"auth_key":"k"}]}"#;

        let error = parse_download_response(body).expect_err("must fail");

        assert_eq!(classify_quark_failure(&error), RemoteErrorKind::Protocol);
    }

    // ---- 错误码映射 ----------------------------------------------------

    #[test]
    fn every_documented_code_maps_to_its_documented_kind() {
        // 方案 §4.7 的穷尽表，一字不差地锁在这里。
        let cases: &[(i64, RemoteErrorKind)] = &[
            (31001, RemoteErrorKind::Auth),
            (31004, RemoteErrorKind::Auth),
            (14013, RemoteErrorKind::NotFound),
            (21001, RemoteErrorKind::NotFound),
            (25008, RemoteErrorKind::NotFound),
            (14014, RemoteErrorKind::Permission),
            (22001, RemoteErrorKind::Permission),
            (14012, RemoteErrorKind::Protocol),
            (23008, RemoteErrorKind::Protocol),
            (24005, RemoteErrorKind::Protocol),
            (43004, RemoteErrorKind::Protocol),
            (25005, RemoteErrorKind::Protocol),
            (23018, RemoteErrorKind::Unsupported),
            (38001, RemoteErrorKind::Unsupported),
            (25004, RemoteErrorKind::Unsupported),
            (32004, RemoteErrorKind::Unsupported),
            (32003, RemoteErrorKind::Unsupported),
            (32005, RemoteErrorKind::Unsupported),
            (14001, RemoteErrorKind::Unreachable),
            (14005, RemoteErrorKind::Unreachable),
            (14010, RemoteErrorKind::Unreachable),
        ];

        for (code, expected) in cases {
            let failure = failure_from_code(*code);
            assert_eq!(
                classify_quark_failure(&failure),
                *expected,
                "code {code} mapped wrong"
            );
        }
    }

    #[test]
    fn the_relogin_code_is_an_auth_failure() {
        let failure = failure_from_code(31001);

        assert_eq!(classify_quark_failure(&failure), RemoteErrorKind::Auth);
    }

    #[test]
    fn the_expired_session_code_is_also_an_auth_failure() {
        // 客户端里 31001 与 31004 同等对待（都触发登出）。
        let failure = failure_from_code(31004);

        assert_eq!(classify_quark_failure(&failure), RemoteErrorKind::Auth);
    }

    #[test]
    fn every_code_in_the_network_range_is_unreachable() {
        for code in 14001..=14010 {
            let failure = failure_from_code(code);
            assert_eq!(
                classify_quark_failure(&failure),
                RemoteErrorKind::Unreachable,
                "code {code} mapped wrong"
            );
        }
    }

    #[test]
    fn the_codes_outside_the_network_range_keep_their_own_meaning() {
        // 14001-14010 之外的两个相邻码不能被范围误吞。
        assert_eq!(
            classify_quark_failure(&failure_from_code(14012)),
            RemoteErrorKind::Protocol
        );
        assert_eq!(
            classify_quark_failure(&failure_from_code(14013)),
            RemoteErrorKind::NotFound
        );
        assert_eq!(
            classify_quark_failure(&failure_from_code(14014)),
            RemoteErrorKind::Permission
        );
    }

    #[test]
    fn an_unlisted_code_falls_back_to_unknown() {
        let failure = failure_from_code(99999);

        assert_eq!(classify_quark_failure(&failure), RemoteErrorKind::Unknown);
    }

    #[test]
    fn an_unlisted_code_keeps_its_original_number() {
        let failure = failure_from_code(99999);

        assert_eq!(failure, QuarkFailure::Unknown { code: 99999 });
    }

    #[test]
    fn the_unknown_error_message_carries_the_original_code() {
        // 不带原始 code 就没法排障 —— 用户只看到「未知错误」也定位不到是哪个码。
        let error = to_remote_error(&QuarkFailure::Unknown { code: 99999 });

        assert!(
            error.message.contains("99999"),
            "message was: {}",
            error.message
        );
    }

    #[test]
    fn http_401_and_403_are_auth_failures() {
        assert_eq!(
            classify_quark_failure(&failure_from_http_status(401)),
            RemoteErrorKind::Auth
        );
        assert_eq!(
            classify_quark_failure(&failure_from_http_status(403)),
            RemoteErrorKind::Auth
        );
    }

    #[test]
    fn http_redirects_are_auth_failures() {
        for status in [301, 302, 303, 307, 308] {
            assert_eq!(
                classify_quark_failure(&failure_from_http_status(status)),
                RemoteErrorKind::Auth,
                "status {status} mapped wrong"
            );
        }
    }

    #[test]
    fn http_404_is_a_not_found_failure() {
        assert_eq!(
            classify_quark_failure(&failure_from_http_status(404)),
            RemoteErrorKind::NotFound
        );
    }

    #[test]
    fn http_429_and_5xx_are_unreachable() {
        for status in [408, 429, 500, 503] {
            assert_eq!(
                classify_quark_failure(&failure_from_http_status(status)),
                RemoteErrorKind::Unreachable,
                "status {status} mapped wrong"
            );
        }
    }

    #[test]
    fn an_unexpected_http_status_is_unknown() {
        let failure = failure_from_http_status(200);

        assert_eq!(classify_quark_failure(&failure), RemoteErrorKind::Unknown);
    }

    #[test]
    fn the_unexpected_status_message_carries_the_status() {
        let error = to_remote_error(&QuarkFailure::UnexpectedStatus { status: 418 });

        assert!(error.message.contains("418"), "{}", error.message);
    }

    #[test]
    fn a_transport_timeout_is_unreachable() {
        let shape = QuarkTransportShape {
            timeout: true,
            ..QuarkTransportShape::default()
        };

        assert_eq!(classify_transport(shape), RemoteErrorKind::Unreachable);
    }

    #[test]
    fn a_transport_connect_failure_is_unreachable() {
        // §4.7 末行：连接超时 / DNS 失败 ⇒ 网络不可达。
        let shape = QuarkTransportShape {
            connect: true,
            ..QuarkTransportShape::default()
        };

        assert_eq!(classify_transport(shape), RemoteErrorKind::Unreachable);
    }

    #[test]
    fn a_transport_body_failure_is_a_disconnect() {
        let shape = QuarkTransportShape {
            body: true,
            ..QuarkTransportShape::default()
        };

        assert_eq!(classify_transport(shape), RemoteErrorKind::Disconnected);
    }

    #[test]
    fn a_transport_request_failure_is_a_protocol_error() {
        let shape = QuarkTransportShape {
            request: true,
            ..QuarkTransportShape::default()
        };

        assert_eq!(classify_transport(shape), RemoteErrorKind::Protocol);
    }

    #[test]
    fn the_coverage_table_covers_every_kind() {
        let coverage = quark_error_kind_coverage();

        assert_eq!(coverage.len(), 11);
    }

    #[test]
    fn the_coverage_table_lists_every_kind_exactly_once() {
        let coverage = quark_error_kind_coverage();

        let kinds = [
            RemoteErrorKind::Auth,
            RemoteErrorKind::HostKeyUnknown,
            RemoteErrorKind::HostKeyChanged,
            RemoteErrorKind::Unreachable,
            RemoteErrorKind::Permission,
            RemoteErrorKind::NotFound,
            RemoteErrorKind::Protocol,
            RemoteErrorKind::Encoding,
            RemoteErrorKind::Disconnected,
            RemoteErrorKind::Unsupported,
            RemoteErrorKind::Unknown,
        ];

        for kind in kinds {
            let hits = coverage.iter().filter(|entry| entry.kind == kind).count();
            assert_eq!(hits, 1, "kind {kind:?} listed {hits} times");
        }
    }

    #[test]
    fn the_coverage_table_marks_the_host_key_kinds_as_impossible() {
        let coverage = quark_error_kind_coverage();

        for kind in [
            RemoteErrorKind::HostKeyUnknown,
            RemoteErrorKind::HostKeyChanged,
        ] {
            let entry = coverage
                .iter()
                .find(|entry| entry.kind == kind)
                .expect("entry");
            assert!(!entry.producible, "kind {kind:?} should not be producible");
        }
    }

    #[test]
    fn every_coverage_note_explains_itself() {
        for entry in quark_error_kind_coverage() {
            assert!(
                !entry.note.trim().is_empty(),
                "kind {:?} has an empty note",
                entry.kind
            );
        }
    }

    // ---- 单位换算 ------------------------------------------------------

    #[test]
    fn seconds_are_converted_to_unix_millis() {
        assert_eq!(quark_seconds_to_unix_millis(1_700_000_000), 1_700_000_000_000);
    }

    #[test]
    fn millis_are_passed_through() {
        assert_eq!(
            quark_millis_to_unix_millis(1_700_000_000_123),
            1_700_000_000_123
        );
    }

    #[test]
    fn the_timestamp_converter_honours_the_seconds_unit() {
        assert_eq!(
            quark_timestamp_to_unix_millis(1_700_000_000, QuarkTimeUnit::Seconds),
            1_700_000_000_000
        );
    }

    #[test]
    fn the_timestamp_converter_honours_the_millis_unit() {
        assert_eq!(
            quark_timestamp_to_unix_millis(1_700_000_000_123, QuarkTimeUnit::Milliseconds),
            1_700_000_000_123
        );
    }

    #[test]
    fn the_default_time_unit_is_seconds() {
        // §7-U8 未决前按秒；T0 核对后只改这一个常量。
        assert_eq!(QUARK_DEFAULT_TIME_UNIT, QuarkTimeUnit::Seconds);
    }

    #[test]
    fn bytes_are_passed_through() {
        assert_eq!(quark_size_to_bytes(2048, QuarkSizeUnit::Bytes), 2048);
    }

    #[test]
    fn kibibytes_are_scaled_to_bytes() {
        assert_eq!(quark_size_to_bytes(2, QuarkSizeUnit::Kibibytes), 2048);
    }

    #[test]
    fn the_default_size_unit_is_bytes() {
        assert_eq!(QUARK_DEFAULT_SIZE_UNIT, QuarkSizeUnit::Bytes);
    }

    // ---- 条目映射 ------------------------------------------------------

    #[test]
    fn the_entry_name_prefers_file_name() {
        let entry = QuarkFileEntry {
            file_name: "a.txt".to_string(),
            name: "b.txt".to_string(),
            ..QuarkFileEntry::default()
        };

        assert_eq!(quark_entry_name(&entry), Some("a.txt".to_string()));
    }

    #[test]
    fn the_entry_name_falls_back_to_name() {
        let entry = QuarkFileEntry {
            name: "b.txt".to_string(),
            ..QuarkFileEntry::default()
        };

        assert_eq!(quark_entry_name(&entry), Some("b.txt".to_string()));
    }

    #[test]
    fn an_entry_without_a_name_is_skipped() {
        let entry = QuarkFileEntry::default();

        assert_eq!(quark_entry_name(&entry), None);
    }

    #[test]
    fn the_remote_entry_is_a_file_when_its_type_is_not_the_directory_one() {
        let entry = QuarkFileEntry {
            file_name: "a.txt".to_string(),
            file_type: 1,
            ..QuarkFileEntry::default()
        };

        let remote = to_remote_entry("conn-a", "/", &entry).expect("entry");

        assert!(!remote.is_dir);
    }

    #[test]
    fn the_remote_entry_is_a_directory_when_its_type_says_so() {
        let entry = QuarkFileEntry {
            file_name: "docs".to_string(),
            file_type: QUARK_FILE_TYPE_DIRECTORY,
            ..QuarkFileEntry::default()
        };

        let remote = to_remote_entry("conn-a", "/", &entry).expect("entry");

        assert!(remote.is_dir);
    }

    #[test]
    fn the_remote_entry_is_built_under_the_current_directory() {
        let entry = QuarkFileEntry {
            file_name: "a.txt".to_string(),
            ..QuarkFileEntry::default()
        };

        let remote = to_remote_entry("conn-a", "/docs", &entry).expect("entry");

        assert_eq!(remote.path, "sfm://remote/conn-a/docs/a.txt");
    }

    #[test]
    fn the_remote_entry_at_the_root_has_no_leading_slash_in_the_path() {
        let entry = QuarkFileEntry {
            file_name: "a.txt".to_string(),
            ..QuarkFileEntry::default()
        };

        let remote = to_remote_entry("conn-a", "/", &entry).expect("entry");

        assert_eq!(remote.path, "sfm://remote/conn-a/a.txt");
    }

    #[test]
    fn the_remote_entry_carries_the_size_in_bytes() {
        let entry = QuarkFileEntry {
            file_name: "a.zip".to_string(),
            size: 2048,
            ..QuarkFileEntry::default()
        };

        let remote = to_remote_entry("conn-a", "/", &entry).expect("entry");

        assert_eq!(remote.size, 2048);
    }

    #[test]
    fn the_remote_entry_carries_the_modification_time_in_millis() {
        let entry = QuarkFileEntry {
            file_name: "a.zip".to_string(),
            updated_at: 1_700_000_000,
            ..QuarkFileEntry::default()
        };

        let remote = to_remote_entry("conn-a", "/", &entry).expect("entry");

        assert_eq!(remote.modified_time, 1_700_000_000_000);
    }

    #[test]
    fn a_dot_prefixed_name_is_marked_hidden() {
        let entry = QuarkFileEntry {
            file_name: ".hidden".to_string(),
            ..QuarkFileEntry::default()
        };

        let remote = to_remote_entry("conn-a", "/", &entry).expect("entry");

        assert!(remote.is_hidden);
    }

    #[test]
    fn an_entry_whose_name_contains_a_separator_is_skipped() {
        let entry = QuarkFileEntry {
            file_name: "a/b".to_string(),
            ..QuarkFileEntry::default()
        };

        assert!(to_remote_entry("conn-a", "/", &entry).is_none());
    }

    #[test]
    fn the_remote_entries_skip_the_unusable_ones() {
        let entries = vec![
            QuarkFileEntry {
                file_name: "good.txt".to_string(),
                ..QuarkFileEntry::default()
            },
            QuarkFileEntry::default(),
        ];

        let remote = to_remote_entries("conn-a", "/", &entries);

        assert_eq!(remote.len(), 1);
        assert_eq!(remote[0].name, "good.txt");
    }

    // ---- 会话 ----------------------------------------------------------

    #[test]
    fn the_session_debug_output_never_prints_the_cookie() {
        let session = QuarkSession::new("__pus=SECRET_VALUE; other=1");

        let rendered = format!("{session:?}");

        assert!(!rendered.contains("SECRET_VALUE"), "{rendered}");
    }

    #[test]
    fn the_session_description_only_prints_the_length() {
        let session = QuarkSession::new("__pus=SECRET_VALUE");

        let described = session.describe();

        assert!(!described.contains("SECRET_VALUE"), "{described}");
        assert!(described.contains("len="), "{described}");
    }

    #[test]
    fn a_session_without_cookies_has_no_cookie_header() {
        assert_eq!(QuarkSession::new("").cookie_header(), None);
    }

    #[test]
    fn the_cookie_header_keeps_the_whole_jar() {
        let session = QuarkSession::new("__pus=abc; other=def");

        assert_eq!(
            session.cookie_header(),
            Some("__pus=abc; other=def".to_string())
        );
    }

    #[test]
    fn the_cookie_header_rejects_a_jar_with_a_broken_pair() {
        let session = QuarkSession::new("__pus=abc; broken");

        assert_eq!(session.cookie_header(), None);
    }

    #[test]
    fn the_cookie_header_rejects_a_jar_with_an_illegal_value() {
        let session = QuarkSession::new("__pus=a\"b");

        assert_eq!(session.cookie_header(), None);
    }

    #[test]
    fn a_cookie_value_can_be_read_by_name() {
        let session = QuarkSession::new("__pus=abc; other=def");

        assert_eq!(session.cookie_value("other"), Some("def".to_string()));
    }

    #[test]
    fn the_pus_cookie_can_be_read_by_name() {
        let session = QuarkSession::new("__pus=abc; other=def");

        assert_eq!(
            session.cookie_value(QUARK_SESSION_COOKIE_PUS),
            Some("abc".to_string())
        );
    }

    #[test]
    fn a_missing_cookie_value_is_none() {
        let session = QuarkSession::new("__pus=abc");

        assert_eq!(session.cookie_value("missing"), None);
    }

    #[test]
    fn a_session_without_cookies_has_no_credentials() {
        assert!(!QuarkSession::new("   ").has_credentials());
    }

    #[test]
    fn a_session_with_cookies_has_credentials() {
        assert!(QuarkSession::new("__pus=abc").has_credentials());
    }

    #[test]
    fn cookie_values_are_trimmed_and_accepted_when_ordinary() {
        assert_eq!(
            sanitize_cookie_value("  abc123  "),
            Some("abc123".to_string())
        );
        assert_eq!(sanitize_cookie_value("a-b_c.d~e"), Some("a-b_c.d~e".into()));
    }

    #[test]
    fn cookie_values_with_separators_or_controls_are_rejected() {
        assert_eq!(sanitize_cookie_value("a;b"), None);
        assert_eq!(sanitize_cookie_value("a,b"), None);
        assert_eq!(sanitize_cookie_value("a\"b"), None);
        assert_eq!(sanitize_cookie_value("a\\b"), None);
        assert_eq!(sanitize_cookie_value("a\r\nb"), None);
        assert_eq!(sanitize_cookie_value("   "), None);
    }

    #[test]
    fn a_cookie_value_with_inner_whitespace_is_rejected() {
        assert_eq!(sanitize_cookie_value("a b"), None);
    }

    // ---- 下载过期判定 --------------------------------------------------

    #[test]
    fn a_download_url_is_not_expired_before_the_refresh_window() {
        // exp = 1700000000，提前 30 分钟 ⇒ 1699998200 之后才算过期。
        assert!(!quark_download_is_expired(1_700_000_000, 1_699_998_199_000));
    }

    #[test]
    fn a_download_url_is_expired_thirty_minutes_early() {
        assert!(quark_download_is_expired(1_700_000_000, 1_699_998_200_000));
    }

    #[test]
    fn a_download_url_is_expired_after_its_expiry() {
        assert!(quark_download_is_expired(1_700_000_000, 1_700_000_000_000));
    }

    #[test]
    fn the_expiry_falls_back_to_the_url_query() {
        let body = r#"{"code":0,"data":[{"download_url":"https://dl/x?Expires=1700000000&k=v"}]}"#;

        let link = parse_download_response(body).expect("parse");

        assert_eq!(link.expires, Some(1_700_000_000));
    }
}
