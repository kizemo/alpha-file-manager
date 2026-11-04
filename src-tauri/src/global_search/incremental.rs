// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 全局搜索的增量索引：**变更计划**这一层。
//!
//! 这一层刻意是纯逻辑 —— 把文件系统 watcher 报过来的事件翻译成
//! 「索引该怎么改」，既不碰 tauri，也不碰 tantivy writer。
//!
//! 为什么不碰 tauri：`cargo test --lib` 的测试二进制里，任何被链接到的
//! `tauri::AppHandle` / `tauri::async_runtime::*` 都会把 tao 整条窗口链拖进来，
//! 测试二进制在**加载阶段**就以 `STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139)` 崩掉
//! （见 AGENTS.md）。这一层要能进单测，变更计划的正确性得有回归网兜着。
//!
//! 为此这里的路径归一化是**本模块自带**的，不复用 `crate::utils` ——
//! 这样本模块连 crate 级的依赖都没有，那条红线从根上够不着。
//!
//! 路径一律是**正斜杠**形式（`C:/Users/foo/bar.txt`），与索引里存的一致 ——
//! 索引侧写进去之前也是把 `\` 换成 `/`。

use std::collections::BTreeMap;

/// 把路径归一化成索引里的样子（反斜杠换正斜杠）。
pub fn normalize_index_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// watcher 报过来的一个文件系统事件。
///
/// 与 `notify` 解耦：这一层不关心事件来自哪个后端，只关心
/// 「哪个路径怎么了」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsEvent {
    /// 路径出现了（文件或目录）。
    Created { path: String },
    /// 路径内容变了。
    Modified { path: String },
    /// 路径没了。
    Removed { path: String },
    /// 路径改名了 —— 拆成「旧的没了 + 新的出现」。
    Renamed { from: String, to: String },
}

/// 一条变更的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// 路径出现了，或内容变了 —— 重新索引它。
    Upsert,
    /// 路径没了 —— 删掉索引里对应的那一条（以及它下面的全部）。
    Remove,
}

/// 一条变更。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// 归一化后的路径（正斜杠）。
    pub path: String,
    pub kind: ChangeKind,
}

/// 一批变更。
///
/// 同一条路径出现多次时，**后到的**覆盖先到的 —— 文件改完又被删，
/// 最终状态就是「没了」。
#[derive(Debug, Default, Clone)]
pub struct ChangeSet {
    changes: BTreeMap<String, ChangeKind>,
}

impl ChangeSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记一条变更。同一条路径以**后到的**为准。
    pub fn push(&mut self, path: &str, kind: ChangeKind) {
        self.changes
            .insert(normalize_index_path(path), kind);
    }

    /// 把一个文件系统事件翻译成变更。
    pub fn apply_event(&mut self, event: FsEvent) {
        match event {
            FsEvent::Created { path } | FsEvent::Modified { path } => {
                self.push(&path, ChangeKind::Upsert);
            }
            FsEvent::Removed { path } => {
                self.push(&path, ChangeKind::Remove);
            }
            FsEvent::Renamed { from, to } => {
                self.push(&from, ChangeKind::Remove);
                self.push(&to, ChangeKind::Upsert);
            }
        }
    }

    /// 批量应用事件。
    pub fn apply_events(&mut self, events: &[FsEvent]) {
        for event in events {
            self.apply_event(event.clone());
        }
    }

    /// 折叠后的变更列表。
    ///
    /// 折叠规则：**被删掉的祖先会吞掉它下面的全部待写入**。
    ///
    /// 目录都没了，里面那条「重新索引」写不写都无所谓 —— 写进去还会
    /// 立刻被父目录的删除操作清掉，白做一轮 IO。
    ///
    /// 注意方向：只有 `Remove` 会吞掉后代的 `Upsert`；
    /// `Upsert` 不会吞掉后代的 `Remove`（文件被删了就是被删了）。
    pub fn compacted(&self) -> Vec<Change> {
        let removed_prefixes: Vec<&str> = self
            .changes
            .iter()
            .filter(|(_, kind)| **kind == ChangeKind::Remove)
            .map(|(path, _)| path.as_str())
            .collect();

        let mut result = Vec::with_capacity(self.changes.len());

        for (path, kind) in &self.changes {
            if *kind == ChangeKind::Remove {
                result.push(Change {
                    path: path.clone(),
                    kind: *kind,
                });
                continue;
            }

            if removed_prefixes.iter().any(|ancestor| is_under(path, ancestor)) {
                continue;
            }

            result.push(Change {
                path: path.clone(),
                kind: *kind,
            });
        }

        result
    }
}

/// `path` 是不是在 `prefix` 之下。
///
/// `prefix` 是不带尾斜杠的目录路径。`C:/dir` 之下有 `C:/dir/a.txt`，
/// 但 `C:/directory/a.txt` **不在**它之下 —— 这是前缀匹配最容易错的地方。
pub fn is_under(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');

    if prefix.is_empty() {
        return false;
    }

    path.len() > prefix.len()
        && path.starts_with(prefix)
        && path.as_bytes()[prefix.len()] == b'/'
}

/// 把一个字面量路径转成正则里的字面量。
///
/// 用的是「所有 ASCII 标点都转义」的做法 —— 与 `regex` crate 自带的
/// `escape` 一致。路径里出现 `[` `.` `+` 这类字符时不转义就会把
/// 正则的语义改掉，导致删多或删少。
/// 正则里必须转义的元字符。
///
/// 只转义「真的会改变正则含义」的字符 —— `:` `/` `-` 这些不是元字符，
/// 转义它们只会把 pattern 拉长，而且不同正则引擎对「转义非字母」
/// 的处理并不一致。
const REGEX_METACHARACTERS: &str = r"\.+*?()[]{}^$|";

pub fn escape_regex_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() * 2);

    for character in value.chars() {
        if REGEX_METACHARACTERS.contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }

    escaped
}

/// 删除某个目录**整棵子树**用的正则。
///
/// 索引里的 `path` 字段是 raw（`STRING`）分词，整条路径是一个 term，
/// 所以匹配的是「term 本身」。
///
/// ⚠ tantivy 0.26 的 `RegexQuery` 底层是 `tantivy-fst::Regex` ——
/// 一个有限自动机引擎，**不支持零宽断言**（`^` / `$`），写了会直接
/// 报 "Empty match operators are not allowed"。它是**全匹配**语义
/// （DFA 必须消耗完整个 term 才算命中），所以不需要锚点：
/// `(prefix)(/.*)?` 只会命中这个目录自己和它下面以 `/` 开头的整棵子树，
/// `C:/directory` 不会被误伤。
///
/// `(?s)` 让 `.` 也匹配换行 —— Windows 上文件名可以带换行符，
/// 不加这个标志会漏掉一部分子孙。
pub fn prefix_delete_pattern(prefix: &str) -> String {
    let normalized = normalize_index_path(prefix);
    let trimmed = normalized.trim_end_matches('/');
    let escaped = escape_regex_literal(trimmed);

    format!("(?s){escaped}(/.*)?")
}

/// 一个路径在**文件系统**里的形态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// 普通文件。
    File,
    /// 目录。`already_indexed` 表示这个目录**已经**在索引里 ——
    /// 决定要不要整棵重扫：已经索引过的目录只需要刷新它自己那一条，
    /// 新出现的目录需要把整棵子树都写进去。
    Directory { already_indexed: bool },
    /// 路径已经不在了。
    Missing,
}

/// 一条要落到索引上的操作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexOp {
    /// 删掉路径**以及它下面全部**的文档。
    DeletePrefix { prefix: String },
    /// 重新索引这一个文件。
    UpsertFile { path: String },
    /// 重新索引这一个目录**自己**的那一条文档（不动它的子树）。
    UpsertDir { path: String },
    /// 重新索引这个目录**整棵子树**。
    UpsertTree { prefix: String },
}

/// 把变更列表翻译成索引操作。
///
/// 查询闭包把「事实来源」隔开，本函数保持纯逻辑、可单测：
/// `upsert_kind` 回答「这个路径在**文件系统**里是什么」。
///
/// 删除一律走**前缀删除**：正则 `^path(/.*)?$` 既能命中路径自己，
/// 也能命中它下面全部的子孙。目录被删时子孙必须一起走，否则索引里
/// 会留下一棵查不到根的孤儿树；而文件被删时这条正则只会命中它自己，
/// 代价只是一次「按字面量前缀走 FST」的查找，很便宜。
///
/// 之所以**不**去索引里查「它原来是文件还是目录」：文件被删的那一刻，
/// 磁盘上已经没有它了，能区分的只有索引 —— 而查索引要拿文档地址，
/// 那条路的 API 成本远高于「多删一次自己」。
pub fn compile_changes<G>(changes: &[Change], upsert_kind: &G) -> Vec<IndexOp>
where
    G: Fn(&str) -> PathKind,
{
    let mut ops = Vec::with_capacity(changes.len());

    for change in changes {
        match change.kind {
            ChangeKind::Remove => ops.push(IndexOp::DeletePrefix {
                prefix: change.path.clone(),
            }),
            ChangeKind::Upsert => match upsert_kind(&change.path) {
                PathKind::File => ops.push(IndexOp::UpsertFile {
                    path: change.path.clone(),
                }),
                PathKind::Directory { already_indexed } => {
                    if already_indexed {
                        ops.push(IndexOp::UpsertDir {
                            path: change.path.clone(),
                        })
                    } else {
                        ops.push(IndexOp::UpsertTree {
                            prefix: change.path.clone(),
                        })
                    }
                }
                // 路径又没了（创建后立刻被删）—— 当成删除处理。
                PathKind::Missing => ops.push(IndexOp::DeletePrefix {
                    prefix: change.path.clone(),
                }),
            },
        }
    }

    ops
}

// ---------------------------------------------------------------------------
// 一致性闸门
//
// 「能不能只做增量」这件事必须能被单独推理，因此判定逻辑放在纯函数里，
// 输入是一张快照，输出是「走增量」还是「必须全量重扫」+ 原因。
// ---------------------------------------------------------------------------

/// 不能走增量、必须全量重扫的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncrementalBlocker {
    /// 没有可用的活索引。
    NoValidIndex,
    /// 索引的 schema 版本对不上。
    SchemaVersionMismatch,
    /// 要索引的盘根集合和上次不一样。
    DriveRootsChanged,
    /// 上一次全量扫描没成功。
    LastFullScanFailed,
    /// watcher 没在跑（或不是上次全量之后启动的那个）。
    WatcherNotReady,
    /// watcher 的事件队列溢出过 —— 中间有变化没被记下来。
    WatcherOverflowed,
    /// 上一次增量应用失败。
    LastIncrementalFailed,
}

/// 增量计划的结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncrementalPlan {
    /// 只应用变更即可。
    Incremental,
    /// 必须退回全量重扫。
    FullRescan(IncrementalBlocker),
}

/// 闸门的输入快照。
#[derive(Debug, Clone, Default)]
pub struct IncrementalGate {
    /// 活索引能打开且文档数大于 0。
    pub has_valid_live_index: bool,
    /// 索引元信息里记的 schema 版本。
    pub meta_schema_version: u32,
    /// 当前代码期望的 schema 版本。
    pub current_schema_version: u32,
    /// 索引里已覆盖的盘根。
    pub indexed_drive_roots: Vec<String>,
    /// 本次要索引的盘根。
    pub requested_drive_roots: Vec<String>,
    /// 上一次全量扫描成功过。
    pub last_full_scan_succeeded: bool,
    /// watcher 正在跑、盘根对得上、且是上次全量之后启动的那一个。
    pub watcher_ready: bool,
    /// watcher 的事件队列溢出过。
    pub watcher_overflowed: bool,
    /// 上一次增量应用失败过。
    pub last_incremental_failed: bool,
}

/// 判定这一轮走增量还是全量。
///
/// 检查顺序是刻意的 —— 从「最基础的事实」排到「最近一次操作的状态」，
/// 这样报出来的原因始终是**最先失效的那一条**，排查时不用猜。
pub fn decide_incremental_plan(gate: &IncrementalGate) -> IncrementalPlan {
    if !gate.has_valid_live_index {
        return IncrementalPlan::FullRescan(IncrementalBlocker::NoValidIndex);
    }

    if gate.meta_schema_version != gate.current_schema_version {
        return IncrementalPlan::FullRescan(IncrementalBlocker::SchemaVersionMismatch);
    }

    if !same_drive_roots(&gate.indexed_drive_roots, &gate.requested_drive_roots) {
        return IncrementalPlan::FullRescan(IncrementalBlocker::DriveRootsChanged);
    }

    if !gate.last_full_scan_succeeded {
        return IncrementalPlan::FullRescan(IncrementalBlocker::LastFullScanFailed);
    }

    if !gate.watcher_ready {
        return IncrementalPlan::FullRescan(IncrementalBlocker::WatcherNotReady);
    }

    if gate.watcher_overflowed {
        return IncrementalPlan::FullRescan(IncrementalBlocker::WatcherOverflowed);
    }

    if gate.last_incremental_failed {
        return IncrementalPlan::FullRescan(IncrementalBlocker::LastIncrementalFailed);
    }

    IncrementalPlan::Incremental
}

/// 两组盘根是不是同一组（忽略顺序、大小写与斜杠风格差异）。
pub fn same_drive_roots(left: &[String], right: &[String]) -> bool {
    let mut left_normalized: Vec<String> = left
        .iter()
        .map(|root| normalize_index_path(root).to_lowercase())
        .collect();
    let mut right_normalized: Vec<String> = right
        .iter()
        .map(|root| normalize_index_path(root).to_lowercase())
        .collect();

    left_normalized.sort();
    right_normalized.sort();

    left_normalized == right_normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_event_wins_for_the_same_path() {
        let mut set = ChangeSet::new();
        set.push("C:/a.txt", ChangeKind::Upsert);
        set.push("C:/a.txt", ChangeKind::Remove);

        let compacted = set.compacted();
        assert_eq!(compacted.len(), 1);
        assert_eq!(compacted[0].kind, ChangeKind::Remove);
    }

    #[test]
    fn removing_a_tree_swallows_pending_writes_below_it() {
        let mut set = ChangeSet::new();
        set.push("C:/dir", ChangeKind::Remove);
        set.push("C:/dir/a.txt", ChangeKind::Upsert);
        set.push("C:/dir/sub/b.txt", ChangeKind::Upsert);
        set.push("C:/other.txt", ChangeKind::Upsert);

        let compacted = set.compacted();
        assert_eq!(compacted.len(), 2);
        assert!(compacted
            .iter()
            .any(|change| change.path == "C:/dir" && change.kind == ChangeKind::Remove));
        assert!(compacted.iter().any(|change| change.path == "C:/other.txt"));
    }

    #[test]
    fn a_sibling_with_a_shared_prefix_is_not_swallowed() {
        let mut set = ChangeSet::new();
        set.push("C:/dir", ChangeKind::Remove);
        set.push("C:/directory/a.txt", ChangeKind::Upsert);

        let compacted = set.compacted();
        assert_eq!(compacted.len(), 2);
    }

    #[test]
    fn a_removal_does_not_swallow_a_removal_below_it() {
        let mut set = ChangeSet::new();
        set.push("C:/dir", ChangeKind::Remove);
        set.push("C:/dir/a.txt", ChangeKind::Remove);

        let compacted = set.compacted();
        assert_eq!(compacted.len(), 2);
    }

    #[test]
    fn a_rename_becomes_a_remove_plus_an_upsert() {
        let mut set = ChangeSet::new();
        set.apply_event(FsEvent::Renamed {
            from: "C:/old.txt".into(),
            to: "C:/new.txt".into(),
        });

        let compacted = set.compacted();
        assert_eq!(compacted.len(), 2);
        assert!(compacted
            .iter()
            .any(|change| change.path == "C:/old.txt" && change.kind == ChangeKind::Remove));
        assert!(compacted
            .iter()
            .any(|change| change.path == "C:/new.txt" && change.kind == ChangeKind::Upsert));
    }

    #[test]
    fn paths_are_normalized_to_forward_slashes() {
        let mut set = ChangeSet::new();
        set.apply_event(FsEvent::Created {
            path: "C:\\dir\\a.txt".into(),
        });

        let compacted = set.compacted();
        assert_eq!(compacted[0].path, "C:/dir/a.txt");
    }

    #[test]
    fn a_path_under_a_prefix_is_recognised() {
        assert!(is_under("C:/dir/a.txt", "C:/dir"));
        assert!(!is_under("C:/dir", "C:/dir"));
        assert!(!is_under("C:/directory/a.txt", "C:/dir"));
        assert!(!is_under("C:/other/a.txt", "C:/dir"));
    }

    #[test]
    fn a_drive_root_prefix_still_matches() {
        assert!(is_under("C:/a.txt", "C:"));
        assert!(!is_under("C:", "C:"));
    }

    #[test]
    fn a_removal_is_always_a_prefix_delete() {
        let changes = vec![Change {
            path: "C:/a.txt".into(),
            kind: ChangeKind::Remove,
        }];

        let ops = compile_changes(&changes, &|_| PathKind::Missing);
        assert_eq!(ops, vec![IndexOp::DeletePrefix {
            prefix: "C:/a.txt".into()
        }]);
    }

    #[test]
    fn a_removed_directory_drops_the_whole_subtree() {
        let changes = vec![Change {
            path: "C:/dir".into(),
            kind: ChangeKind::Remove,
        }];

        let ops = compile_changes(&changes, &|_| PathKind::Missing);
        assert_eq!(ops, vec![IndexOp::DeletePrefix {
            prefix: "C:/dir".into()
        }]);
    }

    #[test]
    fn a_new_directory_needs_a_tree_scan_but_a_known_one_does_not() {
        let changes = vec![Change {
            path: "C:/dir".into(),
            kind: ChangeKind::Upsert,
        }];

        let fresh = compile_changes(&changes, &|_| PathKind::Directory {
            already_indexed: false,
        });
        assert_eq!(fresh, vec![IndexOp::UpsertTree {
            prefix: "C:/dir".into()
        }]);

        let known = compile_changes(&changes, &|_| PathKind::Directory {
            already_indexed: true,
        });
        assert_eq!(known, vec![IndexOp::UpsertDir {
            path: "C:/dir".into()
        }]);
    }

    #[test]
    fn a_vanished_upsert_becomes_a_delete() {
        let changes = vec![Change {
            path: "C:/a.txt".into(),
            kind: ChangeKind::Upsert,
        }];

        let ops = compile_changes(&changes, &|_| PathKind::Missing);
        assert_eq!(ops, vec![IndexOp::DeletePrefix {
            prefix: "C:/a.txt".into()
        }]);
    }

    #[test]
    fn the_prefix_delete_pattern_matches_the_directory_and_its_subtree() {
        let pattern = prefix_delete_pattern("C:/dir");

        assert_eq!(pattern, "(?s)C:/dir(/.*)?");
        assert!(!pattern.contains("directory"));
    }

    #[test]
    fn the_prefix_delete_pattern_escapes_metacharacters() {
        let pattern = prefix_delete_pattern("C:/a+b(c)");

        assert_eq!(pattern, "(?s)C:/a\\+b\\(c\\)(/.*)?");
    }

    #[test]
    fn the_prefix_delete_pattern_handles_a_drive_root() {
        assert_eq!(prefix_delete_pattern("C:/"), "(?s)C:(/.*)?");
    }

    #[test]
    fn the_gate_prefers_incremental_when_everything_is_in_order() {
        let gate = IncrementalGate {
            has_valid_live_index: true,
            meta_schema_version: 1,
            current_schema_version: 1,
            indexed_drive_roots: vec!["C:/".into(), "D:/".into()],
            requested_drive_roots: vec!["D:/".into(), "C:/".into()],
            last_full_scan_succeeded: true,
            watcher_ready: true,
            watcher_overflowed: false,
            last_incremental_failed: false,
        };

        assert_eq!(decide_incremental_plan(&gate), IncrementalPlan::Incremental);
    }

    #[test]
    fn the_gate_falls_back_when_the_drive_roots_changed() {
        let gate = IncrementalGate {
            has_valid_live_index: true,
            meta_schema_version: 1,
            current_schema_version: 1,
            indexed_drive_roots: vec!["C:/".into()],
            requested_drive_roots: vec!["C:/".into(), "E:/".into()],
            last_full_scan_succeeded: true,
            watcher_ready: true,
            watcher_overflowed: false,
            last_incremental_failed: false,
        };

        assert_eq!(
            decide_incremental_plan(&gate),
            IncrementalPlan::FullRescan(IncrementalBlocker::DriveRootsChanged)
        );
    }

    #[test]
    fn the_gate_falls_back_when_the_watcher_overflowed() {
        let gate = IncrementalGate {
            has_valid_live_index: true,
            meta_schema_version: 1,
            current_schema_version: 1,
            indexed_drive_roots: vec!["C:/".into()],
            requested_drive_roots: vec!["C:/".into()],
            last_full_scan_succeeded: true,
            watcher_ready: true,
            watcher_overflowed: true,
            last_incremental_failed: false,
        };

        assert_eq!(
            decide_incremental_plan(&gate),
            IncrementalPlan::FullRescan(IncrementalBlocker::WatcherOverflowed)
        );
    }

    #[test]
    fn the_gate_reports_the_first_failing_precondition() {
        let gate = IncrementalGate {
            has_valid_live_index: false,
            meta_schema_version: 0,
            current_schema_version: 1,
            indexed_drive_roots: vec![],
            requested_drive_roots: vec!["C:/".into()],
            last_full_scan_succeeded: false,
            watcher_ready: false,
            watcher_overflowed: true,
            last_incremental_failed: true,
        };

        assert_eq!(
            decide_incremental_plan(&gate),
            IncrementalPlan::FullRescan(IncrementalBlocker::NoValidIndex)
        );
    }

    #[test]
    fn drive_roots_compare_ignores_order_and_slash_style() {
        assert!(same_drive_roots(
            &["C:/".into(), "D:/".into()],
            &["d:\\".into(), "C:\\".into()]
        ));
        assert!(!same_drive_roots(&["C:/".into()], &["C:/".into(), "D:/".into()]));
    }
}
