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

    // ── 手机 / 设备节点：shell 命名空间解析路径必须逐字透传 ──────────────────
    //
    // FORK-MODIFICATION (2026-11-19)：批次 23 之后，Windows 资源管理器双击 MTP 手机时，
    // 会把「shell 命名空间解析路径」作为 argv 交给注册进 shell 的本启动器。
    // 下面三条常量是**真机取证的权威原文**（逐字符），不是随手编的样例：
    //   - `\\\?\` = 父节点分隔符 `\` 加上设备命名空间前缀 `\\?\`，不是笔误，
    //     不能折叠成一个 `\`，也不能当成 UNC 前缀改写；
    //   - `#`、`&`、`,`、`:` 都是这条路径自身的字符，不是命令行元字符，不能转义；
    //   - 整串 82 ~ 156 字符（带存储目录名的能到 180），任何截断都会让主程序收不到设备。
    //
    // 两层实现的分工：本启动器**只**改写 `/select`，其余 argv 原样 clone 进
    // `passthrough`（见 `parse_launch_arguments` 的 else 分支）；「shell 路径要保留、
    // 不删不改」由主程序 `filter_shell_namespace_args`（src-tauri/src/lib.rs:161）负责，
    // 那侧已有对应用例。历史上的严重缺陷正是这两层中有一层把 `::{…}` 参数删掉，
    // 用户看到的现象是「双击手机只跳到位置界面」。这里把启动器这一层钉死。

    /// 手机（MTP）设备根节点：真机原文，129 字符。
    const MTP_SHELL_PATH: &str = r#"::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\\?\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33}"#;

    /// 同一台手机的存储节点（带 `SID-{…}` 片段）：真机原文，156 字符。
    const MTP_SHELL_PATH_WITH_SID: &str = r#"::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\\?\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33}\SID-{10001,,1001925963776}"#;

    /// 按 WPD uuid 解析的设备节点：真机原文，82 字符。
    const DEVICE_SHELL_PATH_BY_UUID: &str =
        r#"::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\uuid:4d696e69-444c-164e-9d41-7085c27f4afe"#;

    /// 手机设备根节点必须整串原样出现在透传参数里。
    #[test]
    fn keeps_mtp_device_shell_namespace_path_verbatim() {
        let parsed = parse_launch_arguments(&arguments(&[MTP_SHELL_PATH]));

        assert_eq!(parsed.select_path, None);
        assert_eq!(parsed.passthrough, vec![MTP_SHELL_PATH.to_string()]);
    }

    /// 带 `SID-{…}` 的手机存储节点同样原样透传（`,` 与 `#` 都不该触发任何特殊处理）。
    #[test]
    fn keeps_mtp_storage_shell_namespace_path_verbatim() {
        let parsed = parse_launch_arguments(&arguments(&[MTP_SHELL_PATH_WITH_SID]));

        assert_eq!(parsed.select_path, None);
        assert_eq!(parsed.passthrough, vec![MTP_SHELL_PATH_WITH_SID.to_string()]);
    }

    /// `uuid:` 形态的设备节点（含冒号）不能被当成「带 scheme 的开关」丢弃。
    #[test]
    fn keeps_device_shell_namespace_path_by_uuid_verbatim() {
        let parsed = parse_launch_arguments(&arguments(&[DEVICE_SHELL_PATH_BY_UUID]));

        assert_eq!(parsed.select_path, None);
        assert_eq!(
            parsed.passthrough,
            vec![DEVICE_SHELL_PATH_BY_UUID.to_string()]
        );
    }

    /// 注册表 `Folder\shell\open\command` 里 `%1` 外面常包一层引号。
    ///
    /// 真机上命令行解析会把那层引号吃掉，所以上面三条用的就是裸串形态；
    /// 这条用例钉的是另一半契约：**即使引号留到了 argv 里**，
    /// 引号内的路径也必须逐字未动 —— 用现成的 `trim_argument_quotes` 剥掉引号后
    /// 必须与真机原文完全相等（既没被转义，也没被截断）。
    #[test]
    fn keeps_quoted_shell_namespace_path_intact() {
        let quoted = format!("\"{MTP_SHELL_PATH_WITH_SID}\"");
        let parsed = parse_launch_arguments(&arguments(&[quoted.as_str()]));

        assert_eq!(parsed.select_path, None);
        assert_eq!(parsed.passthrough, vec![quoted]);
        assert_eq!(
            trim_argument_quotes(parsed.passthrough.first().expect("quoted path dropped")),
            MTP_SHELL_PATH_WITH_SID
        );
    }

    /// 带空格的 shell 路径（按真机形态构造：设备节点下那层 `Internal shared storage`）。
    ///
    /// 命令行解析后它仍是**一个** argv 元素：不能按空格切开，也不能因为带空格
    /// 就补转义或加引号；带引号形态剥掉引号后必须逐字相同。
    #[test]
    fn keeps_shell_namespace_path_with_spaces_intact() {
        let path_with_space = format!(r#"{MTP_SHELL_PATH_WITH_SID}\Internal shared storage"#);
        let quoted = format!("\"{path_with_space}\"");

        let plain = parse_launch_arguments(&arguments(&[path_with_space.as_str()]));
        assert_eq!(plain.select_path, None);
        assert_eq!(plain.passthrough, vec![path_with_space.clone()]);

        let wrapped = parse_launch_arguments(&arguments(&[quoted.as_str()]));
        assert_eq!(wrapped.select_path, None);
        assert_eq!(wrapped.passthrough, vec![quoted]);
        assert_eq!(
            trim_argument_quotes(wrapped.passthrough.first().expect("quoted path dropped")),
            path_with_space
        );
    }

    /// 回归护栏：启动器**不得**因为参数长得像 shell 命名空间路径（`::{`、`shell:`、
    /// 内含 `#` / `&` / `,` / `:`）就丢弃或改写它 —— 历史上正是「看着不像正常路径就删掉」
    /// 导致双击手机只跳到位置界面。对照普通路径 `D:\tmp`：行为必须一致，
    /// 条数、顺序、内容三者都不许变。
    #[test]
    fn never_drops_arguments_because_of_shell_namespace_markers() {
        let ordinary_path = "D:\\tmp";
        let parsed = parse_launch_arguments(&arguments(&[
            "::{",
            MTP_SHELL_PATH,
            DEVICE_SHELL_PATH_BY_UUID,
            "shell:::{20D04FE0-3AEA-1069-A2D8-08002B30309D}",
            ordinary_path,
        ]));

        assert_eq!(parsed.select_path, None);
        assert_eq!(
            parsed.passthrough,
            vec![
                "::{".to_string(),
                MTP_SHELL_PATH.to_string(),
                DEVICE_SHELL_PATH_BY_UUID.to_string(),
                "shell:::{20D04FE0-3AEA-1069-A2D8-08002B30309D}".to_string(),
                ordinary_path.to_string(),
            ]
        );
    }

    /// 混合场景：手机路径与普通文件夹路径同时进来时，两条都原样保留、顺序不变。
    #[test]
    fn keeps_shell_namespace_path_beside_ordinary_folder_path() {
        let parsed = parse_launch_arguments(&arguments(&[MTP_SHELL_PATH_WITH_SID, "D:\\tmp"]));

        assert_eq!(parsed.select_path, None);
        assert_eq!(
            parsed.passthrough,
            vec![MTP_SHELL_PATH_WITH_SID.to_string(), "D:\\tmp".to_string()]
        );
    }

    /// select 改写是唯一允许动 argv 的分支：即便目标就是手机路径，
    /// 也必须只加 `--sigma-select=` 前缀、路径部分逐字不变。
    #[test]
    fn keeps_shell_namespace_path_intact_when_rewritten_as_select_argument() {
        let inline_select = format!("/select,{MTP_SHELL_PATH}");
        let expected = format!("--sigma-select={MTP_SHELL_PATH}");
        let parsed = parse_launch_arguments(&arguments(&[inline_select.as_str()]));

        assert_eq!(parsed.select_path.as_deref(), Some(MTP_SHELL_PATH));
        assert_eq!(parsed.passthrough, vec![expected]);
    }
}
