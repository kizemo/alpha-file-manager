// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! 云盘（远程连接）专用的全局检索索引。
//!
//! # 为什么必须是**独立**的一份索引
//!
//! 本地全局检索索引由 `scan.rs` 的 `WalkDir` 全盘扫描建出来，对 `sfm://remote/...`
//! 零感知。反过来，把云盘混进本地的 `drive_roots` 也不行：
//!
//! - `incremental.rs` 的 `same_drive_roots` 决定「全量重扫 vs 增量」，
//!   根集合里混进远程 id ⇒ 每轮判定不一致 ⇒ 每轮退化成全量重扫；
//! - `index_watcher.rs` 只 watch 本地盘根，而 `RemoteProvider` **没有 watch 能力**
//!   ⇒ 云盘只能全量遍历，每轮都遍历 = 每轮几千次网络往返 + 网盘接口配额消耗。
//!
//! 于是本模块另起一份 tantivy 索引（`<appData>/remote-index/`），
//! 只在用户显式触发重建时刷新；本地索引的增量管线**一行不动**。
//!
//! # 本模块刻意**不引用 `tauri::`**
//!
//! `cargo test --lib` 的测试二进制链接了整个 crate，而 `tauri::AppHandle` /
//! `tauri::async_runtime` 会把 Wry 运行时单态化，测试进程在加载阶段就以
//! `STATUS_ENTRYPOINT_NOT_FOUND` 崩掉（详见 `remote/mod.rs` 的模块头说明）。
//! 所以本模块只认**普通路径**：app-data 目录与 user-data 目录由调用方
//! （`global_search/commands.rs`）解析好再传进来。

use std::path::Path;

use tantivy::doc;
use tantivy::{Index, IndexReader, ReloadPolicy};

use super::index::{build_schema, create_bulk_index_writer, create_fresh_index};
use super::state::GlobalSearchIndexFields;

/// 云盘索引的最大遍历深度。连接根算第 0 层，往下最多再钻 8 层。
///
/// 取值与本地扫描的 `scanDepth` 同量级：网盘目录普遍比本地浅，
/// 而每多一层就是一轮网络往返。
pub(super) const REMOTE_INDEX_MAX_DEPTH: usize = 8;

/// 单次重建最多写入的条目数（**所有连接合计**）。
///
/// 这是「一次重建要花多少网络往返 / 多少接口配额」的总闸门。
/// 触顶时必须 `log::warn!` 说出来 —— 静默截断等于骗用户「云盘内容全都在索引里」。
pub(super) const REMOTE_INDEX_MAX_ENTRIES: usize = 20_000;

/// 一条待写入云盘索引的条目。
///
/// 只保留索引真正需要的字段：`RemoteEntry` 里的 `thumbnail_url` 是**签名直链**，
/// 既不进索引也不进日志。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteIndexDoc {
    /// 完整的 `sfm://remote/<连接id>/...` 路径 —— 前端可以直接拿它导航。
    pub(super) path: String,
    pub(super) name: String,
    pub(super) is_dir: bool,
    pub(super) size: u64,
    pub(super) modified_time: u64,
}

/// 遍历是否还应继续。
///
/// 抽成纯函数是为了能在**没有网络**的情况下单测上限判定：
/// 深度超了、条目数满了，都必须停，而且必须**说出来**。
pub(super) fn walk_should_continue(depth: usize, indexed_entries: usize) -> bool {
    depth <= REMOTE_INDEX_MAX_DEPTH && indexed_entries < REMOTE_INDEX_MAX_ENTRIES
}

/// 从完整的 `sfm://remote/<连接id>/...` 路径里取出**远端路径**部分。
///
/// 解析不出来就返回 `None` —— 那时这个目录**不能**继续下钻
/// （provider 只认远端路径），但条目本身照样进索引。
fn remote_path_of(full_path: &str) -> Option<String> {
    crate::remote::parse_remote_path(full_path).map(|parsed| parsed.remote_path)
}

/// 遍历给定的连接，把条目收集起来（这一步还**没有**写盘）。
///
/// **按连接隔离失败**：一个云盘 400 / 超时 / 鉴权过期都只记一条日志，
/// 其它连接照常遍历 —— 一个坏网盘不能拖垮整轮建索引。
///
/// 全部遍历都在 async 上下文里 `.await` provider 调用；
/// **不要**在这里套一层 tokio runtime（`Runtime::block_on` 在运行时里会 panic）。
pub(super) async fn collect_remote_entries(
    user_data_dir: &Path,
    app_data_dir: &Path,
    connection_ids: &[String],
) -> Vec<RemoteIndexDoc> {
    let mut docs: Vec<RemoteIndexDoc> = Vec::new();

    for connection_id in connection_ids {
        if !walk_should_continue(0, docs.len()) {
            log::warn!(
                "[global-search] 云盘索引已达条目上限 {}，剩余连接不再遍历",
                REMOTE_INDEX_MAX_ENTRIES
            );
            break;
        }

        let resolved =
            match crate::remote::resolve_connection(user_data_dir, app_data_dir, connection_id) {
                Ok(resolved) => resolved,
                Err(error) => {
                    log::warn!(
                        "[global-search] 云盘索引跳过连接 {}：解析失败：{}",
                        connection_id,
                        error
                    );
                    continue;
                }
            };

        let provider = crate::remote::provider_with_protocol(
            resolved.connection.protocol,
            user_data_dir,
            app_data_dir,
        )
        .await;

        let before = docs.len();
        walk_connection(provider.as_ref(), connection_id, &mut docs).await;

        log::info!(
            "[global-search] 云盘索引连接 {} 收集到 {} 条",
            connection_id,
            docs.len() - before
        );
    }

    docs
}

/// 递归遍历一个连接的目录树。
///
/// 目录是**广度优先**：一层一层往下走，每层都过一遍上限判定。
/// 单个目录读不出来（鉴权过期 / 限流 / 超时）只记日志，其余目录照常走。
async fn walk_connection(
    provider: &dyn crate::remote::RemoteProvider,
    connection_id: &str,
    docs: &mut Vec<RemoteIndexDoc>,
) {
    // 待访问的目录队列：(远端路径, 深度)。连接根的远端路径是空串。
    let mut pending: Vec<(String, usize)> = vec![(String::new(), 0)];

    while let Some((remote_path, depth)) = pending.pop() {
        if !walk_should_continue(depth, docs.len()) {
            log::warn!(
                "[global-search] 云盘索引连接 {} 达到上限，停止遍历（深度 {}，已收集 {} 条）",
                connection_id,
                depth,
                docs.len()
            );
            break;
        }

        let entries = match provider.read_dir(connection_id, &remote_path).await {
            Ok(entries) => entries,
            Err(error) => {
                log::warn!(
                    "[global-search] 云盘索引读取目录失败 conn={} path={} err={}",
                    connection_id,
                    remote_path,
                    error
                );
                continue;
            }
        };

        for entry in entries {
            if entry.is_dir {
                match remote_path_of(&entry.path) {
                    Some(child_path) => {
                        if walk_should_continue(depth + 1, docs.len()) {
                            pending.push((child_path, depth + 1));
                        } else {
                            log::warn!(
                                "[global-search] 云盘索引达到上限，目录 {} 未继续下钻",
                                entry.path
                            );
                        }
                    }
                    None => {
                        log::warn!(
                            "[global-search] 云盘索引无法解析目录路径 {}，未继续下钻",
                            entry.path
                        );
                    }
                }
            }

            docs.push(RemoteIndexDoc {
                path: entry.path,
                name: entry.name,
                is_dir: entry.is_dir,
                size: entry.size,
                modified_time: entry.modified_time,
            });

            if docs.len() >= REMOTE_INDEX_MAX_ENTRIES {
                log::warn!(
                    "[global-search] 云盘索引连接 {} 达到条目上限 {}，停止遍历",
                    connection_id,
                    REMOTE_INDEX_MAX_ENTRIES
                );
                break;
            }
        }
    }
}

/// 把收集到的条目写进云盘索引（**从零重建**）。
///
/// 先删目录再建 —— 取消勾选的连接因此不会在索引里留下残条目。
/// 返回实际写入的条目数。
pub(super) fn write_remote_index(
    index_path: &Path,
    docs: &[RemoteIndexDoc],
) -> Result<u64, String> {
    let (index, fields) = create_fresh_index(index_path)?;
    let mut writer = create_bulk_index_writer(&index)?;

    for entry in docs {
        let is_file = u64::from(!entry.is_dir);
        let is_dir = u64::from(entry.is_dir);
        let name_lower = entry.name.to_lowercase();

        writer
            .add_document(doc!(
                fields.path => entry.path.as_str(),
                fields.name => entry.name.as_str(),
                fields.name_lower => name_lower.as_str(),
                fields.is_file => is_file,
                fields.is_dir => is_dir,
                fields.modified_time => entry.modified_time,
                fields.size => entry.size,
            ))
            .map_err(|error| error.to_string())?;
    }

    writer.commit().map_err(|error| error.to_string())?;

    Ok(docs.len() as u64)
}

/// 打开云盘索引的 reader。
///
/// 索引目录不存在 / 打不开 / schema 对不上，一律返回 `None` ——
/// 查询路径据此判定「没有云盘索引」，行为与加这个功能之前**完全一致**。
pub(super) fn open_remote_reader(
    index_path: &Path,
) -> Option<(IndexReader, GlobalSearchIndexFields)> {
    if !index_path.exists() {
        return None;
    }

    let index = Index::open_in_dir(index_path).ok()?;
    let (schema, fields) = build_schema();

    if index.schema() != schema {
        log::warn!("[global-search] 云盘索引的 schema 与当前不一致，本次查询忽略云盘结果");
        return None;
    }

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .ok()?;

    Some((reader, fields))
}

/// 清空云盘索引（取消勾选 / 传空列表时用）。
pub(super) fn clear_remote_index(index_path: &Path) -> Result<(), String> {
    super::index::clear_index(index_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn doc(path: &str, name: &str, is_dir: bool) -> RemoteIndexDoc {
        RemoteIndexDoc {
            path: path.to_string(),
            name: name.to_string(),
            is_dir,
            size: 12,
            modified_time: 1_700_000_000_000,
        }
    }

    /// 上限判定是纯函数 —— 没有网络也能把两条边界都钉住。
    #[test]
    fn the_walk_stops_at_the_depth_cap() {
        assert!(walk_should_continue(0, 0));
        assert!(walk_should_continue(REMOTE_INDEX_MAX_DEPTH, 0));
        assert!(!walk_should_continue(REMOTE_INDEX_MAX_DEPTH + 1, 0));
    }

    #[test]
    fn the_walk_stops_at_the_entry_cap() {
        assert!(walk_should_continue(0, REMOTE_INDEX_MAX_ENTRIES - 1));
        assert!(!walk_should_continue(0, REMOTE_INDEX_MAX_ENTRIES));
        assert!(!walk_should_continue(0, REMOTE_INDEX_MAX_ENTRIES + 1));
    }

    /// 写入 → 打开 → 文档数对得上，且条目按原样落盘。
    #[test]
    fn writing_the_remote_index_produces_a_readable_index() {
        let temp = TempDir::new().expect("temp dir");
        let index_path = temp.path().join("remote-index");

        let written = write_remote_index(
            &index_path,
            &[
                doc("sfm://remote/conn-a/readme.txt", "readme.txt", false),
                doc("sfm://remote/conn-a/pub", "pub", true),
            ],
        )
        .expect("write");

        assert_eq!(written, 2);

        let (reader, _fields) = open_remote_reader(&index_path).expect("reader must open");
        assert_eq!(reader.searcher().num_docs(), 2);
    }

    /// **从零重建**：第二次写入必须覆盖第一次的内容，不能把旧条目留在索引里
    /// （取消勾选的连接就是这么消失的）。
    #[test]
    fn rewriting_the_remote_index_drops_the_previous_entries() {
        let temp = TempDir::new().expect("temp dir");
        let index_path = temp.path().join("remote-index");

        write_remote_index(
            &index_path,
            &[doc("sfm://remote/conn-a/old.txt", "old.txt", false)],
        )
        .expect("first write");

        write_remote_index(
            &index_path,
            &[doc("sfm://remote/conn-b/new.txt", "new.txt", false)],
        )
        .expect("second write");

        let (reader, _fields) = open_remote_reader(&index_path).expect("reader must open");
        assert_eq!(reader.searcher().num_docs(), 1);
    }

    /// 没有索引目录时返回 `None` —— 查询路径据此保持「加这个功能之前」的行为。
    #[test]
    fn a_missing_index_directory_yields_no_reader() {
        let temp = TempDir::new().expect("temp dir");

        assert!(open_remote_reader(&temp.path().join("remote-index")).is_none());
    }

    /// 清空索引后目录必须真的没了（不能留半截索引让查询读到残条目）。
    #[test]
    fn clearing_the_remote_index_removes_the_directory() {
        let temp = TempDir::new().expect("temp dir");
        let index_path = temp.path().join("remote-index");

        write_remote_index(&index_path, &[doc("sfm://remote/conn-a/a.txt", "a.txt", false)])
            .expect("write");
        assert!(index_path.exists());

        clear_remote_index(&index_path).expect("clear");

        assert!(!index_path.exists());
        assert!(open_remote_reader(&index_path).is_none());
    }

    /// `sfm://remote/<id>/...` → 远端路径；解析不出来就返回 `None`。
    #[test]
    fn extracts_the_remote_path_from_a_full_remote_entry_path() {
        assert_eq!(
            remote_path_of("sfm://remote/conn-a/pub/docs").as_deref(),
            Some("/pub/docs")
        );
        assert_eq!(
            remote_path_of("sfm://remote/conn-a/readme.txt").as_deref(),
            Some("/readme.txt")
        );
        assert_eq!(remote_path_of("C:/Users/duanyi/readme.txt"), None);
    }
}
