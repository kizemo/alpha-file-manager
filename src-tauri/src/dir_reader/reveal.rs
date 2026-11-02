// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 「在资源管理器中显示」的纯内核（仅 Windows）。
//!
//! 这个模块**刻意不碰** `tauri::AppHandle` / `tauri::async_runtime::*`：
//! `cargo test --lib` 的测试二进制一旦链进 tao/Wry，就会在加载阶段以
//! `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉（详见 `dir_reader.rs` 顶部注释）。
//! 所以这里只有纯逻辑 + `std::process`，由 `commands.rs` 薄封装。
//!
//! 原实现有三处隐患（2026-11-01 加固）：
//! 1. `explorer.exe` 裸名依赖 PATH 解析 —— 解析不到就报 "program not found"，
//!    而且**错误信息里没有 path**，用户完全看不出是哪个条目出的问题。
//! 2. `/select,<path>` 没加引号 —— path 里带空格时 Explorer 会把后半截当成
//!    另一个参数，于是弹出「Windows 找不到 '…'」这类框。
//! 3. 前端交来的 path 是正斜杠（`utils::normalize_path`），Explorer 认的是反斜杠。
//!
//! 另外补了一条（2026-11-02）：**快捷方式（`.lnk`）的目标要先校验**。
//! Explorer 拿到 `.lnk` 会自己去解它；目标已经失效时它会弹自己的模态框
//! （「Windows 找不到 '…'」/「该项目已更改或移动」），用户看到的正是这种框。
//! 所以这里先把目标解析出来：目标还在就定位目标，目标没了就**在本地拦下来**
//! 返回错误，让前端弹 toast —— 不再把坏快捷方式交给 Explorer。

use std::path::{Path, PathBuf};

/// 解析出来的 reveal 目标。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealTarget {
    /// 要在资源管理器里选中的路径。
    pub selected_path: PathBuf,
    /// 该路径是不是从快捷方式解析出来的（只用于日志）。
    pub resolved_from_shortcut: bool,
}

/// 一次 reveal 操作准备好的参数（尚未启动进程）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedReveal {
    /// 系统自带 explorer.exe 的绝对路径。
    pub program: PathBuf,
    /// 传给 explorer.exe 的原始参数串（`/select,"C:\path\to\file"`）。
    pub argument: String,
    /// 实际被选中的路径。
    pub selected_path: PathBuf,
    /// 该路径是不是从快捷方式解析出来的（只用于日志）。
    pub resolved_from_shortcut: bool,
}

/// 找到系统自带的 explorer.exe 绝对路径。
///
/// 不用裸名 —— 裸名要靠 PATH 解析，解析不到就报 "program not found"，
/// 而且那条错误里根本没有 path，用户无从判断是哪个条目出的问题。
pub fn system_explorer_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(system_root) = std::env::var_os("SystemRoot") {
        candidates.push(PathBuf::from(system_root).join("explorer.exe"));
    }

    candidates.push(PathBuf::from(r"C:\Windows\explorer.exe"));

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
}

/// 把路径转成 Explorer 认的反斜杠形式。
pub fn to_native_windows_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

/// 组装 `/select,"<path>"`。
///
/// 引号是必须的：Explorer 的 `/select,` 后面跟的是**一整条路径**，
/// 路径里带空格时不给引号，Explorer 会把后半截当成另一个参数。
pub fn build_select_argument(selected_path: &Path) -> String {
    format!("/select,\"{}\"", to_native_windows_path(selected_path))
}

/// 解析 reveal 目标：普通路径要求它存在；快捷方式要求**它指向的目标**存在。
///
/// 快捷方式这条是关键：Explorer 收到 `.lnk` 会自己去解，目标失效时它会弹
/// 自己的模态框（「Windows 找不到 '…'」），那种框不是本应用能关掉的。
/// 所以在本地先把目标解析出来：
/// - 目标存在 ⇒ 定位目标（不选快捷方式本身）。
/// - 目标没了 ⇒ 直接返回 Err，错误串里带上快捷方式路径 ⇒ 前端弹 toast。
/// - 连目标都解析不出来（COM 读不了这个 `.lnk`）⇒ 退回快捷方式本身，
///   保持原有行为，不因为解析失败而挡住用户。
pub fn resolve_reveal_target(path: &Path) -> Result<RevealTarget, String> {
    resolve_reveal_target_with(path, resolve_shortcut_target)
}

/// [`resolve_reveal_target`] 的可注入版本（单测用假解析器，不碰 COM）。
fn resolve_reveal_target_with<F>(path: &Path, resolve_shortcut: F) -> Result<RevealTarget, String>
where
    F: Fn(&Path) -> Option<String>,
{
    if is_shortcut(path) {
        if let Some(target) = resolve_shortcut(path) {
            let target_path = PathBuf::from(target);

            if target_path.exists() {
                return Ok(RevealTarget {
                    selected_path: target_path,
                    resolved_from_shortcut: true,
                });
            }

            return Err(format!(
                "Shortcut target does not exist: {} (shortcut: {})",
                target_path.display(),
                path.to_string_lossy()
            ));
        }
    }

    if path.exists() {
        return Ok(RevealTarget {
            selected_path: path.to_path_buf(),
            resolved_from_shortcut: false,
        });
    }

    Err(format!("Path does not exist: {}", path.to_string_lossy()))
}

/// 是不是快捷方式（`.lnk`）。
fn is_shortcut(path: &Path) -> bool {
    path_extension_lowercase(path).as_deref() == Some("lnk")
}

/// 校验 + 组装（不启动进程）。
pub fn prepare_reveal(path: &str) -> Result<PreparedReveal, String> {
    if path.trim().is_empty() {
        return Err(format!("Reveal path is empty: '{path}'"));
    }

    let target = resolve_reveal_target(Path::new(path))?;

    let program = system_explorer_path().ok_or_else(|| {
        format!("Windows Explorer (explorer.exe) could not be located for path: {path}")
    })?;

    Ok(PreparedReveal {
        argument: build_select_argument(&target.selected_path),
        program,
        selected_path: target.selected_path,
        resolved_from_shortcut: target.resolved_from_shortcut,
    })
}

/// 真正的执行入口：校验 → 找 explorer.exe → 组参数 → 起进程。
pub fn reveal_in_explorer_blocking(path: &str) -> Result<(), String> {
    let prepared = prepare_reveal(path)?;

    log::info!(
        "[reveal] reveal_in_explorer: path={path} selected={} program={} argument={} from_shortcut={}",
        prepared.selected_path.display(),
        prepared.program.display(),
        prepared.argument,
        prepared.resolved_from_shortcut
    );

    let mut command = std::process::Command::new(&prepared.program);
    // `raw_arg`（`CommandExt`，Windows 专有）原样拼进命令行。
    // 走普通 `arg()` 会被 Rust 再加一层引号，
    // Explorer 会把带引号的整串当成路径，反而选不中目标。
    std::os::windows::process::CommandExt::raw_arg(&mut command, &prepared.argument);

    match command.spawn() {
        Ok(_) => {
            log::info!("[reveal] reveal_in_explorer spawned: path={path}");

            Ok(())
        }
        Err(error) => {
            log::error!(
                "[reveal] reveal_in_explorer failed: path={path} program={} error={error}",
                prepared.program.display()
            );

            Err(format!(
                "Failed to launch Windows Explorer for {path}: {error}"
            ))
        }
    }
}

/// 取小写扩展名。
fn path_extension_lowercase(path: &Path) -> Option<String> {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
}

/// 解析 `.lnk` 指向的目标。
fn resolve_shortcut_target(path: &Path) -> Option<String> {
    super::read::resolve_windows_shortcut_target(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn missing_path_is_rejected_with_the_path_in_the_error() {
        let directory = tempdir().expect("temp dir should be created");
        let missing = directory.path().join("missing-file.txt");
        let missing_text = missing.to_string_lossy().into_owned();

        let error = reveal_in_explorer_blocking(&missing_text)
            .expect_err("missing path should be rejected");

        assert!(
            error.contains(&missing_text),
            "error should mention the path, got: {error}"
        );
    }

    #[test]
    fn empty_path_is_rejected() {
        let error = reveal_in_explorer_blocking("").expect_err("empty path should be rejected");

        assert!(!error.is_empty(), "error should not be empty");
    }

    #[test]
    fn existing_file_produces_a_quoted_select_argument() {
        let directory = tempdir().expect("temp dir should be created");
        let file = directory.path().join("existing file.txt");
        fs::write(&file, "test").expect("file should be written");

        let prepared =
            prepare_reveal(&file.to_string_lossy()).expect("existing file should be accepted");

        assert_eq!(prepared.selected_path, file);
        assert!(
            prepared.argument.starts_with("/select,\""),
            "argument should be quoted, got: {}",
            prepared.argument
        );
        assert!(
            prepared.argument.ends_with('"'),
            "argument should end with a quote, got: {}",
            prepared.argument
        );
        assert!(
            prepared.argument.contains("existing file.txt"),
            "argument should contain the file name, got: {}",
            prepared.argument
        );
        assert!(
            prepared.program.is_file(),
            "explorer.exe should exist, got: {}",
            prepared.program.display()
        );
    }

    #[test]
    fn select_argument_uses_backslashes() {
        let argument = build_select_argument(Path::new("C:/Users/test/file.txt"));

        assert_eq!(argument, "/select,\"C:\\Users\\test\\file.txt\"");
    }

    #[test]
    fn explorer_program_is_an_absolute_path() {
        let program = system_explorer_path().expect("explorer.exe should be found");

        assert!(program.is_absolute(), "got: {}", program.display());
        assert_eq!(
            program.file_name().and_then(|name| name.to_str()),
            Some("explorer.exe")
        );
    }

    #[test]
    fn shortcut_with_a_live_target_selects_the_target() {
        let directory = tempdir().expect("temp dir should be created");
        let shortcut = directory.path().join("good.lnk");
        let target = directory.path().join("target.txt");
        fs::write(&shortcut, "not a real lnk, the resolver is faked").expect("shortcut stub written");
        fs::write(&target, "test").expect("target should be written");

        let resolved = resolve_reveal_target_with(&shortcut, |_| {
            Some(target.to_string_lossy().into_owned())
        })
        .expect("a live shortcut target should be accepted");

        assert_eq!(resolved.selected_path, target);
        assert!(resolved.resolved_from_shortcut);
    }

    #[test]
    fn shortcut_with_a_dead_target_is_rejected_instead_of_handing_it_to_explorer() {
        let directory = tempdir().expect("temp dir should be created");
        let shortcut = directory.path().join("broken.lnk");
        fs::write(&shortcut, "not a real lnk, the resolver is faked").expect("shortcut stub written");

        let missing = directory.path().join("gone.txt");
        let shortcut_text = shortcut.to_string_lossy().into_owned();

        let error = resolve_reveal_target_with(&shortcut, |_| {
            Some(missing.to_string_lossy().into_owned())
        })
        .expect_err("a dead shortcut target should be rejected");

        assert!(
            error.contains(&shortcut_text),
            "error should mention the shortcut path, got: {error}"
        );
        assert!(
            error.contains("Shortcut target does not exist"),
            "error should explain the shortcut target, got: {error}"
        );
    }

    #[test]
    fn shortcut_without_a_resolvable_target_falls_back_to_the_shortcut_itself() {
        let directory = tempdir().expect("temp dir should be created");
        let shortcut = directory.path().join("opaque.lnk");
        fs::write(&shortcut, "not a real lnk, the resolver is faked").expect("shortcut stub written");

        let resolved = resolve_reveal_target_with(&shortcut, |_| None)
            .expect("an opaque shortcut should still be revealable");

        assert_eq!(resolved.selected_path, shortcut);
        assert!(!resolved.resolved_from_shortcut);
    }
}
