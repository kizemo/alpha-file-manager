// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! Focus Sync 侧车 `/health` 握手里的**纯逻辑**：解析响应体、拼日志正文、
//! 比对两侧版本、把响应体裁成一行。没有 tauri、没有网络、没有文件系统。
//!
//! 为什么单独一个模块（2026-11-26，批次 63）：`focus_sync_sidecar` 引用
//! `tauri::AppHandle`，在里面写单测会把 tao 的窗口链拖进 `cargo test --lib`
//! 的测试二进制，加载阶段就崩（同 `focus_sync_supervisor` 顶部那段说明）。
//!
//! 背景（批次 62 排查「扩展显示未连接」）：主日志里只有
//! `focus-sync sidecar: launched pid=...`，看不出侧车是哪一版、处于哪种模式、
//! 扩展到底有没有在轮询它 —— 只能另起探针手工 `curl /health` 才知道。
//! 这些纯函数就是那次手工探针的固化，**只服务于日志**，不参与启动决策。

use serde_json::Value;

/// `/health` 里对排查有用、且批次 62 实测确实存在的字段。
///
/// 实测响应体（2026-11-25）：
/// `{"version":"0.6.1","mode":"spawn","extension_alive":true,
///   "first_push_received":false,"last_health_poll_age_ms":1200}`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSnapshot {
    pub version: String,
    pub mode: String,
    /// 侧车认为扩展还活着吗（扩展每轮健康检查会刷新）。
    pub extension_alive: Option<bool>,
    /// 扩展是否已经成功 push 过一次路径。
    pub first_push_received: Option<bool>,
    /// 距上次扩展轮询过去了多久；`None` 表示侧车还没被轮询过。
    pub last_health_poll_age_ms: Option<u64>,
}

/// 解析失败的原因。刻意分开「根本不是 JSON」与「缺字段」——
/// 前者通常意味着端口被别的服务（或代理错误页）占了，后者是侧车版本太旧。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthParseError {
    /// 响应体不是 JSON 对象。
    NotJsonObject(String),
    /// 没有非空的 `version` 字符串。
    MissingVersion,
    /// 没有非空的 `mode` 字符串。
    MissingMode,
}

impl std::fmt::Display for HealthParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotJsonObject(reason) => write!(formatter, "not a JSON object: {reason}"),
            Self::MissingVersion => write!(formatter, "missing \"version\""),
            Self::MissingMode => write!(formatter, "missing \"mode\""),
        }
    }
}

fn non_empty_str(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}

/// 解析 `/health` 响应体。
pub fn parse_health(body: &str) -> Result<HealthSnapshot, HealthParseError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| HealthParseError::NotJsonObject(error.to_string()))?;
    if !value.is_object() {
        return Err(HealthParseError::NotJsonObject(
            "top-level value is not an object".to_string(),
        ));
    }

    let version = non_empty_str(value.get("version")).ok_or(HealthParseError::MissingVersion)?;
    let mode = non_empty_str(value.get("mode")).ok_or(HealthParseError::MissingMode)?;

    Ok(HealthSnapshot {
        version,
        mode,
        extension_alive: value.get("extension_alive").and_then(Value::as_bool),
        first_push_received: value.get("first_push_received").and_then(Value::as_bool),
        last_health_poll_age_ms: value.get("last_health_poll_age_ms").and_then(Value::as_u64),
    })
}

/// 拼一行固定字段顺序的日志正文，便于 grep 与两台机器之间肉眼比对。
pub fn describe(snapshot: &HealthSnapshot) -> String {
    format!(
        "version={} mode={} extension_alive={} first_push={} last_health_poll={}",
        snapshot.version,
        snapshot.mode,
        optional_bool(snapshot.extension_alive),
        optional_bool(snapshot.first_push_received),
        match snapshot.last_health_poll_age_ms {
            Some(age) => format!("{age}ms"),
            None => "none".to_string(),
        },
    )
}

fn optional_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

/// 裁成一行、限长，避免把整页 HTML 或二进制塞进日志。
pub fn snippet(body: &str) -> String {
    const MAX_CHARS: usize = 160;
    let flattened: String = body
        .chars()
        .map(|character| if character.is_control() { ' ' } else { character })
        .collect();
    let trimmed = flattened.trim();
    if trimmed.chars().count() <= MAX_CHARS {
        return trimmed.to_string();
    }
    let head: String = trimmed.chars().take(MAX_CHARS).collect();
    format!("{head}…")
}

/// 去掉 `v` 前缀与首尾空白：扩展写 `0.6.2`、侧车写 `v0.6.2` 时不该报不一致。
fn normalize_version(raw: &str) -> String {
    raw.trim()
        .trim_start_matches(&['v', 'V'][..])
        .trim()
        .to_string()
}

/// 只从 `package.json` 里取 `version`，不做任何期望管理。
///
/// 读不到 / 不是 JSON / 没有 `version` 一律返回 `None`：老装机的扩展可能没有这个
/// 文件，那种情况下静默跳过比对，不制造假告警。
pub fn version_from_package_json(raw: &str) -> Option<String> {
    let value: Value = serde_json::from_str(raw).ok()?;
    non_empty_str(value.get("version"))
}

/// 版本握手：返回 `Some(告警正文)` 表示侧车与扩展版本不一致，
/// `None` 表示「一致」或「无法比较」（缺扩展版本 / 空串）。
///
/// ⚠️ 结论只写日志：**不因为不一致拒绝启动**。拒绝启动属行为契约变更，
/// 需要先定阈值（批次 62 §三十八·5 的裁决）。
pub fn version_skew(sidecar_version: &str, extension_version: Option<&str>) -> Option<String> {
    let extension_version = extension_version?;
    let sidecar = normalize_version(sidecar_version);
    let extension = normalize_version(extension_version);
    if sidecar.is_empty() || extension.is_empty() || sidecar == extension {
        return None;
    }
    Some(format!(
        "version skew: sidecar {sidecar} != extension {extension}; \
         the extension's bundled sidecar is probably stale \
         (reinstall or update the Focus Sync extension)"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 批次 62 在真机上抓到的原始响应体，字段原样保留。
    const LIVE_BODY: &str = r#"{"version":"0.6.1","mode":"spawn","extension_alive":true,"first_push_received":false,"last_health_poll_age_ms":1200}"#;

    #[test]
    fn parses_the_live_health_payload() {
        let snapshot = parse_health(LIVE_BODY).expect("live payload must parse");
        assert_eq!(
            snapshot,
            HealthSnapshot {
                version: "0.6.1".to_string(),
                mode: "spawn".to_string(),
                extension_alive: Some(true),
                first_push_received: Some(false),
                last_health_poll_age_ms: Some(1200),
            }
        );
    }

    #[test]
    fn rejects_a_non_json_body() {
        // 端口被别的服务占住时的典型响应（批次 62 见过 `not found` 纯文本）。
        let error = parse_health("not found").expect_err("plain text must not parse");
        assert!(matches!(error, HealthParseError::NotJsonObject(_)), "{error}");
    }

    #[test]
    fn rejects_a_json_value_that_is_not_an_object() {
        let error = parse_health("[1, 2, 3]").expect_err("array must not parse");
        assert!(matches!(error, HealthParseError::NotJsonObject(_)), "{error}");
    }

    #[test]
    fn reports_which_field_is_missing() {
        assert_eq!(
            parse_health(r#"{"mode":"spawn"}"#).expect_err("no version"),
            HealthParseError::MissingVersion
        );
        assert_eq!(
            parse_health(r#"{"version":"0.6.1"}"#).expect_err("no mode"),
            HealthParseError::MissingMode
        );
        // 空串按缺失处理：侧车若把 version 填成 "" 也要报出来。
        assert_eq!(
            parse_health(r#"{"version":"   ","mode":"spawn"}"#).expect_err("blank version"),
            HealthParseError::MissingVersion
        );
    }

    #[test]
    fn tolerates_absent_optional_fields() {
        let snapshot = parse_health(r#"{"version":"0.6.1","mode":"service"}"#)
            .expect("only version+mode are required");
        assert_eq!(snapshot.extension_alive, None);
        assert_eq!(snapshot.first_push_received, None);
        assert_eq!(snapshot.last_health_poll_age_ms, None);
    }

    #[test]
    fn describe_keeps_a_fixed_field_order() {
        let snapshot = parse_health(LIVE_BODY).unwrap();
        assert_eq!(
            describe(&snapshot),
            "version=0.6.1 mode=spawn extension_alive=true first_push=false \
             last_health_poll=1200ms"
        );
    }

    #[test]
    fn describe_marks_absent_fields_instead_of_dropping_them() {
        let snapshot = parse_health(r#"{"version":"0.6.1","mode":"spawn"}"#).unwrap();
        assert_eq!(
            describe(&snapshot),
            "version=0.6.1 mode=spawn extension_alive=unknown first_push=unknown \
             last_health_poll=none"
        );
    }

    #[test]
    fn snippet_flattens_control_characters_and_truncates() {
        assert_eq!(snippet("  hello\r\nworld\t  "), "hello  world");
        let long = "a".repeat(200);
        let cut = snippet(&long);
        assert_eq!(cut.chars().count(), 161, "160 chars plus the ellipsis");
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn version_skew_is_silent_when_versions_agree() {
        assert_eq!(version_skew("0.6.1", Some("0.6.1")), None);
        // `v` 前缀与空白不算差异。
        assert_eq!(version_skew("v0.6.1", Some(" 0.6.1 ")), None);
        assert_eq!(version_skew("V0.6.1", Some("0.6.1")), None);
        // 任一为空即无法比较。
        assert_eq!(version_skew("0.6.1", Some("")), None);
        assert_eq!(version_skew("", Some("0.6.1")), None);
    }

    #[test]
    fn version_skew_is_silent_without_an_extension_version() {
        assert_eq!(version_skew("0.6.1", None), None);
    }

    #[test]
    fn version_skew_reports_both_sides() {
        let warning = version_skew("0.6.1", Some("0.6.2")).expect("0.6.1 vs 0.6.2 must warn");
        assert!(warning.contains("0.6.1"), "{warning}");
        assert!(warning.contains("0.6.2"), "{warning}");
        assert!(warning.contains("sidecar"), "{warning}");
    }

    #[test]
    fn reads_the_version_out_of_an_extension_package_json() {
        let raw = r#"{"name":"focus-sync","version":"0.6.2","main":"./dist/index.js"}"#;
        assert_eq!(version_from_package_json(raw), Some("0.6.2".to_string()));
        assert_eq!(version_from_package_json("{}"), None);
        assert_eq!(version_from_package_json("not json"), None);
        assert_eq!(
            version_from_package_json(r#"{"version":""}"#),
            None,
            "empty version must be treated as absent"
        );
    }
}