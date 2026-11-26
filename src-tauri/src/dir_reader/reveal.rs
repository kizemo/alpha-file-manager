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

/// reveal 落地的方式。
///
/// 两种都要有（2026-11-07 修）：
/// - `Select` ⇒ `explorer.exe /select,"<path>"`，Explorer 打开该路径的父目录并选中它。
/// - `OpenDirectory` ⇒ `explorer.exe "<dir>"`，只打开目录、**不选中任何东西**。
///
/// 什么时候用后者：快捷方式的目标**解析不出来或已经失效**。
/// 这时若还把 `.lnk` 本身交给 Explorer，Explorer 会去解析并启动它，
/// 失败时弹出 Windows 自己的模态框「已中止操作」——
/// 那个框不是本应用能关掉的，也不是本应用想要的结果（2026-11-07 用户实测）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevealMode {
    Select,
    OpenDirectory,
}

/// 解析出来的 reveal 目标。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealTarget {
    /// 要在资源管理器里选中的路径（或要打开的目录）。
    pub selected_path: PathBuf,
    /// 该路径是不是从快捷方式解析出来的（只用于日志）。
    pub resolved_from_shortcut: bool,
    /// 落地方式。
    pub mode: RevealMode,
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
    /// 落地方式。
    pub mode: RevealMode,
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

/// 组装 `"<dir>"`（只打开目录，不选中任何条目）。
///
/// 用于「快捷方式目标解析不出来 / 已失效」的兜底：这时既不能把坏 `.lnk`
/// 交给 Explorer（会弹「已中止操作」），也没有目标可给 —— 只能打开 `.lnk`
/// 自己所在的目录。
pub fn build_open_argument(directory: &Path) -> String {
    format!("\"{}\"", to_native_windows_path(directory))
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
                    mode: RevealMode::Select,
                });
            }

            // 目标已被删掉：打开 `.lnk` 自己所在的目录，
            // **不**把坏快捷方式交给 Explorer（会弹「已中止操作」）。
            log::warn!(
                "[reveal] shortcut target is gone, falling back to the shortcut directory: shortcut={} target={}",
                path.to_string_lossy(),
                target_path.display()
            );

            return Ok(RevealTarget {
                selected_path: parent_or_self(path),
                resolved_from_shortcut: false,
                mode: RevealMode::OpenDirectory,
            });
        }

        // 连目标都解析不出来（COM 读不了这个 `.lnk`）：同样**不**把 `.lnk`
        // 交给 Explorer。
        //
        // 2026-11-07 用户实测：Windows 会为「解析不了的快捷方式」弹自己的模态框
        // ——「已中止操作」，框里写的正是 `.lnk` 自己的路径。
        // 那个框不是本应用能关掉的，所以只能在这里把 `.lnk` 拦下来。
        log::warn!(
            "[reveal] shortcut target could not be resolved, falling back to the shortcut directory: shortcut={}",
            path.to_string_lossy()
        );

        return Ok(RevealTarget {
            selected_path: parent_or_self(path),
            resolved_from_shortcut: false,
            mode: RevealMode::OpenDirectory,
        });
    }

    if path.exists() {
        let mode = if path.is_dir() {
            RevealMode::OpenDirectory
        } else {
            RevealMode::Select
        };

        return Ok(RevealTarget {
            selected_path: path.to_path_buf(),
            resolved_from_shortcut: false,
            mode,
        });
    }

    Err(format!("Path does not exist: {}", path.to_string_lossy()))
}

/// 是不是快捷方式（`.lnk`）。
fn is_shortcut(path: &Path) -> bool {
    path_extension_lowercase(path).as_deref() == Some("lnk")
}

/// 「打开文件所在目录」要落到的地方。
///
/// 与 [`RevealTarget`] 的区别：那个是给**原生 Explorer** 用的 ——
/// 只要把目标本身交给 `explorer /select,` 即可，Explorer 自己会显示它的父目录。
/// 这个是给**应用内定位**用的：应用要自己导航，所以必须同时给出
/// 「要打开的目录」和「要选中的条目」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealEntryLocation {
    /// 要打开的目录。
    pub directory_path: PathBuf,
    /// 要选中的条目。
    pub focus_path: PathBuf,
    /// 该落点是不是从快捷方式解析出来的（只用于日志）。
    pub resolved_from_shortcut: bool,
}

/// 解析「打开文件所在目录」时的默认超时（毫秒）。
///
/// 上限不是随便定的：COM 解析本身是毫秒级，但目标落在**网络盘**上时
/// `IShellLinkW::GetPath` 之后的 `exists()` 可能卡上好几秒 ——
/// 没有超时兜底，界面就会被拖死（2026-11-07 用户实测的「渲染进程崩溃」）。
pub const DEFAULT_REVEAL_TARGET_TIMEOUT_MS: u64 = 2500;

/// 超时下限：太短会误杀一次正常的解析。
pub const MIN_REVEAL_TARGET_TIMEOUT_MS: u64 = 200;

/// 超时上限：太长就失去「兜底」的意义了。
pub const MAX_REVEAL_TARGET_TIMEOUT_MS: u64 = 10_000;

/// 把外部传入的超时收敛到 `[MIN, MAX]` 区间。
pub fn clamp_reveal_timeout_ms(timeout_ms: u64) -> u64 {
    timeout_ms.clamp(MIN_REVEAL_TARGET_TIMEOUT_MS, MAX_REVEAL_TARGET_TIMEOUT_MS)
}

/// 解析「打开文件所在目录」的落点。
///
/// 口径（断链 / 解析失败都不报错，一律回落）：
/// - 不是快捷方式 → 打开它自己的父目录 + 选中它自己。
/// - 是快捷方式、目标能解析、目标还在 → 打开**目标的父目录** + 选中目标。
/// - 是快捷方式但目标解析不出来 / 目标已被删 → 打开 `.lnk` 自己的父目录 + 选中 `.lnk`。
///
/// ⚠️ 目标**不要求是目录**（2026-11-07 修）：指向文件的快捷方式才是绝大多数，
/// 早先实现在这里要求目标 `is_dir()`，于是「指向文件的快捷方式」一律回落成
/// 「打开 `.lnk` 自己所在的目录」—— 用户看到的现象就是「点了没反应，
/// 没跳到快捷方式指向的文件所在目录」。
pub fn resolve_reveal_entry_location(path: &Path) -> RevealEntryLocation {
    resolve_reveal_entry_location_with(path, resolve_shortcut_target)
}

/// [`resolve_reveal_entry_location`] 的可注入版本（单测用假解析器，不碰 COM）。
fn resolve_reveal_entry_location_with<F>(path: &Path, resolve_shortcut: F) -> RevealEntryLocation
where
    F: Fn(&Path) -> Option<String>,
{
    let fallback = RevealEntryLocation {
        directory_path: parent_or_self(path),
        focus_path: path.to_path_buf(),
        resolved_from_shortcut: false,
    };

    if !is_shortcut(path) {
        return fallback;
    }

    let Some(target) = resolve_shortcut(path) else {
        return fallback;
    };

    let target_path = PathBuf::from(target);

    // 目标可能刚好在解析之后被删掉 —— 同样回落，不报错。
    if !target_path.exists() {
        return fallback;
    }

    RevealEntryLocation {
        directory_path: parent_or_self(&target_path),
        focus_path: target_path,
        resolved_from_shortcut: true,
    }
}

/// 父目录；没有父目录（比如盘符根）时用它自己。
fn parent_or_self(path: &Path) -> PathBuf {
    path.parent()
        .map(Path::to_path_buf)
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| path.to_path_buf())
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

    let argument = match target.mode {
        RevealMode::Select => build_select_argument(&target.selected_path),
        RevealMode::OpenDirectory => build_open_argument(&target.selected_path),
    };

    Ok(PreparedReveal {
        argument,
        program,
        selected_path: target.selected_path,
        resolved_from_shortcut: target.resolved_from_shortcut,
        mode: target.mode,
    })
}

/// 真正的执行入口：校验 → 找 explorer.exe → 组参数 → 起进程。
pub fn reveal_in_explorer_blocking(path: &str) -> Result<(), String> {
    let prepared = match prepare_reveal(path) {
        Ok(prepared) => prepared,
        Err(error) => {
            // 2026-11-07：这条分支以前**一条日志都不打** ——
            // 快捷方式目标失效时 `prepare_reveal` 直接返回 Err，
            // 于是「用户点了没反应 + 日志里一条 [reveal] 都没有」。
            // 事后完全看不出到底走了哪条分支。补上。
            log::error!("[reveal] reveal_in_explorer rejected: path={path} error={error}");

            return Err(error);
        }
    };

    log::info!(
        "[reveal] reveal_in_explorer: path={path} selected={} program={} argument={} from_shortcut={} mode={:?}",
        prepared.selected_path.display(),
        prepared.program.display(),
        prepared.argument,
        prepared.resolved_from_shortcut,
        prepared.mode
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
        assert_eq!(resolved.mode, RevealMode::Select);
    }

    #[test]
    fn shortcut_with_a_dead_target_falls_back_to_the_shortcut_directory() {
        let directory = tempdir().expect("temp dir should be created");
        let shortcut = directory.path().join("broken.lnk");
        fs::write(&shortcut, "not a real lnk, the resolver is faked").expect("shortcut stub written");

        let missing = directory.path().join("gone.txt");

        let resolved = resolve_reveal_target_with(&shortcut, |_| {
            Some(missing.to_string_lossy().into_owned())
        })
        .expect("a dead shortcut target should fall back to the shortcut directory");

        // 关键：绝不能把坏掉的 `.lnk` 交给 Explorer —— Windows 会弹「已中止操作」。
        assert_eq!(resolved.selected_path, directory.path());
        assert_eq!(resolved.mode, RevealMode::OpenDirectory);
        assert!(!resolved.resolved_from_shortcut);
    }

    #[test]
    fn shortcut_without_a_resolvable_target_falls_back_to_the_shortcut_directory() {
        let directory = tempdir().expect("temp dir should be created");
        let shortcut = directory.path().join("opaque.lnk");
        fs::write(&shortcut, "not a real lnk, the resolver is faked").expect("shortcut stub written");

        let resolved = resolve_reveal_target_with(&shortcut, |_| None)
            .expect("an opaque shortcut should still be revealable");

        // 解析不出目标 ⇒ 打开 `.lnk` 自己所在的目录，
        // **不**把 `.lnk` 交给 Explorer（否则会弹「已中止操作」）。
        assert_eq!(resolved.selected_path, directory.path());
        assert_eq!(resolved.mode, RevealMode::OpenDirectory);
        assert!(!resolved.resolved_from_shortcut);
    }

    #[test]
    fn open_argument_quotes_the_directory_without_select() {
        let argument = build_open_argument(Path::new("C:/Users/test/Program Files"));

        assert_eq!(argument, "\"C:\\Users\\test\\Program Files\"");
        assert!(!argument.contains("/select,"));
    }

    #[test]
    fn an_existing_directory_is_opened_without_selecting() {
        let directory = tempdir().expect("temp dir should be created");

        let prepared = prepare_reveal(&directory.path().to_string_lossy())
            .expect("an existing directory should be accepted");

        assert_eq!(prepared.mode, RevealMode::OpenDirectory);
        assert!(!prepared.argument.contains("/select,"));
    }

    // ── 「打开文件所在目录」落点解析（2026-11-07 新增） ────────────────── ──────────────────

    #[test]
    fn reveal_entry_location_of_a_file_shortcut_points_at_the_files_own_directory() {
        let directory = tempdir().expect("temp dir should be created");
        let shortcut = directory.path().join("app.lnk");
        let target_directory = directory.path().join("Program Files");
        let target = target_directory.join("app.exe");

        fs::create_dir_all(&target_directory).expect("target directory should be created");
        fs::write(&shortcut, "not a real lnk, the resolver is faked").expect("shortcut stub written");
        fs::write(&target, "test").expect("target should be written");

        let location = resolve_reveal_entry_location_with(&shortcut, |_| {
            Some(target.to_string_lossy().into_owned())
        });

        assert_eq!(location.directory_path, target_directory);
        assert_eq!(location.focus_path, target);
        assert!(location.resolved_from_shortcut);
    }

    #[test]
    fn reveal_entry_location_of_a_directory_shortcut_points_at_the_directories_parent() {
        let directory = tempdir().expect("temp dir should be created");
        let shortcut = directory.path().join("projects.lnk");
        let target_directory = directory.path().join("Projects");

        fs::create_dir_all(&target_directory).expect("target directory should be created");
        fs::write(&shortcut, "not a real lnk, the resolver is faked").expect("shortcut stub written");

        let location = resolve_reveal_entry_location_with(&shortcut, |_| {
            Some(target_directory.to_string_lossy().into_owned())
        });

        assert_eq!(location.directory_path, directory.path());
        assert_eq!(location.focus_path, target_directory);
        assert!(location.resolved_from_shortcut);
    }

    #[test]
    fn reveal_entry_location_falls_back_when_the_shortcut_target_is_gone() {
        let directory = tempdir().expect("temp dir should be created");
        let shortcut = directory.path().join("broken.lnk");
        fs::write(&shortcut, "not a real lnk, the resolver is faked").expect("shortcut stub written");

        let missing = directory.path().join("gone.exe");

        let location = resolve_reveal_entry_location_with(&shortcut, |_| {
            Some(missing.to_string_lossy().into_owned())
        });

        assert_eq!(location.directory_path, directory.path());
        assert_eq!(location.focus_path, shortcut);
        assert!(!location.resolved_from_shortcut);
    }

    #[test]
    fn reveal_entry_location_of_a_plain_file_keeps_its_own_parent_directory() {
        let directory = tempdir().expect("temp dir should be created");
        let file = directory.path().join("report.pdf");
        fs::write(&file, "test").expect("file should be written");

        let location = resolve_reveal_entry_location_with(&file, |_| None);

        assert_eq!(location.directory_path, directory.path());
        assert_eq!(location.focus_path, file);
        assert!(!location.resolved_from_shortcut);
    }

    #[test]
    fn reveal_timeout_is_clamped_into_the_supported_range() {
        assert_eq!(clamp_reveal_timeout_ms(0), MIN_REVEAL_TARGET_TIMEOUT_MS);
        assert_eq!(clamp_reveal_timeout_ms(2500), 2500);
        assert_eq!(
            clamp_reveal_timeout_ms(60_000),
            MAX_REVEAL_TARGET_TIMEOUT_MS
        );
    }
}
