// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION (2026-11-03)：默认资源管理器改造 L3 —— 解析 `/select` 参数。
//
// Windows 的「打开文件所在目录」最终落到 `explorer.exe /select,"C:\dir\file.txt"`。
// 命令行解析之后这是**一个** argv 元素：`/select,C:\dir\file.txt`。
//
// 本启动器是注册进 shell 的那层壳，必须把 select 意图解析出来：
//   1. 转发给主程序时改写成规范形式 `--sigma-select=<路径>`，
//      主程序据此「打开父目录 + 选中该条目」，而不是把它当成普通路径参数。
//   2. 兜底回 Explorer 时用 `/select,<路径>` —— 参数形式才对得上，
//      直接把 `/select,C:\x` 当文件夹路径丢给 explorer 是打不开的。

/// Explorer 用的 select 标记（`/select`）。
pub const LAUNCH_SELECT_FLAG: &str = "/select";

/// 转发给主程序时使用的规范形式前缀。
pub const LAUNCH_SELECT_ARG_PREFIX: &str = "--sigma-select=";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParsedLaunchArguments {
    /// 「打开文件所在目录」要选中的条目路径。没有 select 请求时为 `None`。
    pub select_path: Option<String>,
    /// 要转发给主程序的参数（select 标记已改写成规范形式）。
    pub passthrough: Vec<String>,
}

fn trim_argument_quotes(value: &str) -> &str {
    let trimmed = value.trim();

    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

/// 解析 `/select,<路径>` 与 `/select=<路径>` 两种内联写法。
///
/// 返回 `None` 表示这个参数不是内联 select 写法。
fn parse_inline_select_argument(argument: &str) -> Option<String> {
    // 全部用 `get` 切片：参数里可能有非 ASCII 字符，直接按字节下标切会 panic。
    let prefix = argument.get(..LAUNCH_SELECT_FLAG.len())?;

    if !prefix.eq_ignore_ascii_case(LAUNCH_SELECT_FLAG) {
        return None;
    }

    let separator = argument
        .get(LAUNCH_SELECT_FLAG.len()..)
        .and_then(|rest| rest.chars().next())?;

    if separator != ',' && separator != '=' {
        return None;
    }

    let value = trim_argument_quotes(
        argument
            .get(LAUNCH_SELECT_FLAG.len() + separator.len_utf8()..)
            .unwrap_or_default(),
    );

    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn is_select_flag_argument(argument: &str) -> bool {
    argument.trim().eq_ignore_ascii_case(LAUNCH_SELECT_FLAG)
}

/// 把启动参数拆成「select 目标」与「要转发给主程序的参数」。
///
/// 认得三种写法：
///   - `/select,<路径>` —— Explorer 的实际形式（命令行解析后是一个 argv 元素）
///   - `/select=<路径>`
///   - `/select <路径>` —— 两个 argv 元素
///
/// 多个 select 请求时**先出现的赢**，与动作注册表「先注册者赢」的口径一致。
pub fn parse_launch_arguments(arguments: &[String]) -> ParsedLaunchArguments {
    let mut select_path: Option<String> = None;
    let mut passthrough: Vec<String> = Vec::with_capacity(arguments.len());
    let mut index = 0usize;

    while index < arguments.len() {
        let argument = &arguments[index];

        if let Some(inline_path) = parse_inline_select_argument(argument) {
            if select_path.is_none() {
                select_path = Some(inline_path);
            }

            index += 1;
        }
        else if is_select_flag_argument(argument) {
            if let Some(next_argument) = arguments.get(index + 1) {
                let next_path = trim_argument_quotes(next_argument);

                if !next_path.is_empty() && select_path.is_none() {
                    select_path = Some(next_path.to_string());
                }

                index += 2;
            }
            else {
                // 孤立的 `/select`（后面没有路径）：丢掉，别转发给主程序。
                index += 1;
            }
        }
        else {
            passthrough.push(argument.clone());
            index += 1;
            continue;
        }

        if let Some(path) = select_path.as_deref() {
            passthrough.push(format!("{LAUNCH_SELECT_ARG_PREFIX}{path}"));
        }
    }

    ParsedLaunchArguments {
        select_path,
        passthrough,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_explorer_inline_select_argument() {
        let parsed = parse_launch_arguments(&arguments(&["/select,C:\\Users\\aleks\\report.pdf"]));

        assert_eq!(
            parsed.select_path.as_deref(),
            Some("C:\\Users\\aleks\\report.pdf")
        );
        assert_eq!(
            parsed.passthrough,
            vec!["--sigma-select=C:\\Users\\aleks\\report.pdf".to_string()]
        );
    }

    #[test]
    fn parses_space_separated_select_arguments() {
        let parsed = parse_launch_arguments(&arguments(&["/select", "C:\\Users\\aleks\\report.pdf"]));

        assert_eq!(
            parsed.select_path.as_deref(),
            Some("C:\\Users\\aleks\\report.pdf")
        );
        assert_eq!(
            parsed.passthrough,
            vec!["--sigma-select=C:\\Users\\aleks\\report.pdf".to_string()]
        );
    }

    #[test]
    fn keeps_plain_path_arguments_untouched() {
        let parsed = parse_launch_arguments(&arguments(&["C:\\Users\\aleks\\Documents"]));

        assert_eq!(parsed.select_path, None);
        assert_eq!(
            parsed.passthrough,
            vec!["C:\\Users\\aleks\\Documents".to_string()]
        );
    }

    #[test]
    fn keeps_other_switches_in_passthrough() {
        let parsed = parse_launch_arguments(&arguments(&["--sigma-autostart", "/select,C:\\x"]));

        assert_eq!(parsed.select_path.as_deref(), Some("C:\\x"));
        assert_eq!(
            parsed.passthrough,
            vec![
                "--sigma-autostart".to_string(),
                "--sigma-select=C:\\x".to_string(),
            ]
        );
    }

    #[test]
    fn drops_a_lone_select_flag() {
        let parsed = parse_launch_arguments(&arguments(&["/select"]));

        assert_eq!(parsed.select_path, None);
        assert!(parsed.passthrough.is_empty());
    }

    #[test]
    fn ignores_arguments_that_only_look_like_the_select_flag() {
        let parsed = parse_launch_arguments(&arguments(&["/selected", "C:\\x"]));

        assert_eq!(parsed.select_path, None);
        assert_eq!(
            parsed.passthrough,
            vec!["/selected".to_string(), "C:\\x".to_string()]
        );
    }
}
