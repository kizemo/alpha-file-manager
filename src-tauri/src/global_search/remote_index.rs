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

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use tantivy::doc;
use tantivy::{Index, IndexReader, ReloadPolicy};

use super::index::{build_remote_schema, create_bulk_index_writer, create_fresh_remote_index};
use super::state::GlobalSearchIndexFields;
use super::types::RemoteIndexState;

/// 云盘索引的最大遍历深度。连接根算第 0 层，往下最多再钻 8 层。
///
/// 取值与本地扫描的 `scanDepth` 同量级：网盘目录普遍比本地浅，
/// 而每多一层就是一轮网络往返。
pub(super) const REMOTE_INDEX_MAX_DEPTH: usize = 8;

/// **单个连接**在一轮重建里最多写入的条目数。
///
/// 2026-11-24（批次 45）订正：此前这个上限是**所有连接合计**的全局配额（`20_000`），
/// 于是「谁排在第一个谁吃光全部配额」—— 实测百度一条连接独占 20 000 条，
/// 阿里云盘 / 夸克**一条都没进索引**（`meta.json` 只有 1 段 `max_doc = 20000`），
/// 用户表现为「勾了另两个网盘也搜不到它们任何文件」。
/// ⇒ 配额必须**按连接**算，否则「勾了哪个网盘就只搜哪个网盘」这条产品承诺
/// 对第二个网盘就是假的。
///
/// 触顶时必须 `log::warn!` 说出来 —— 静默截断等于骗用户「云盘内容全都在索引里」。
pub(super) const REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION: usize = 20_000;

/// 一轮重建里**所有连接合计**能写入的条目数上限。
///
/// = 每连接配额 × 本轮连接数（3 个连接 ⇒ 60 000 条 ⇒ 索引约 19 MB）。
/// 它**不是**另一个独立闸门，而是各连接配额的合计 ——
/// 否则「合计上限」本身又会变成新的饿死点（连接数一多，最后几个连接又进不去）。
pub(super) fn total_entry_limit(connection_count: usize) -> usize {
    REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION * connection_count.max(1)
}

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
    /// **这条条目属于哪个远程连接。**
    ///
    /// 这是「勾了哪个网盘，就只搜哪个网盘」这条规则能落地的关键：
    /// 查询侧拿它按**当前**勾选集合过滤，
    /// 取消勾选立刻生效，不必等下一次重建把旧条目清掉。
    pub(super) connection_id: String,
}

/// 遍历是否还应继续。
///
/// 抽成纯函数是为了能在**没有网络**的情况下单测上限判定：
/// 深度超了、条目数满了，都必须停，而且必须**说出来**。
/// 遍历是否还应继续。
///
/// `per_connection_entries` 是**本连接**已收集的条目数（配额按连接算），
/// `total_entries` / `total_limit` 是本轮合计的进度与上限。
pub(super) fn walk_should_continue(
    depth: usize,
    per_connection_entries: usize,
    total_entries: usize,
    total_limit: usize,
) -> bool {
    depth <= REMOTE_INDEX_MAX_DEPTH
        && per_connection_entries < REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION
        && total_entries < total_limit
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

    // 本轮合计上限 = 每连接配额 × 本轮连接数。
    // 配额按连接算是批次 45 的订正（见 `REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION`）：
    // 旧口径是全局配额，先到的连接吃光全部额度，后面的连接一条都进不了索引。
    let total_limit = total_entry_limit(connection_ids.len());
    log::info!(
        "[global-search] 本轮重建 {} 个连接，每连接上限 {} 条，合计上限 {} 条",
        connection_ids.len(),
        REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION,
        total_limit
    );

    for connection_id in connection_ids {
        if !walk_should_continue(0, 0, docs.len(), total_limit) {
            log::warn!(
                "[global-search] 云盘索引已达合计条目上限 {}，剩余连接不再遍历",
                total_limit
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
        walk_connection(provider.as_ref(), connection_id, &mut docs, total_limit).await;

        log::info!(
            "[global-search] 云盘索引连接 {} 收集到 {} 条（本连接上限 {} 条）",
            connection_id,
            docs.len() - before,
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION
        );
    }

    docs
}

/// 因达到上限而**未能继续下钻**的目录计数（W-10，批次 47）。
///
/// 旧实现对每一个被跳过的目录都打一条 WARN，配额被填满时日志会被刷成几万行
/// （实测百度一条连接就吃掉 20000 条），真正的告警全被埋掉。
/// 计数后只在本连接遍历结束时汇总一句，可观测性不降级、日志量从 O(目录数) 降到 O(连接数)。
#[derive(Debug, Default)]
struct SkippedDirs {
    count: usize,
}

impl SkippedDirs {
    fn record(&mut self) {
        self.count += 1;
    }

    /// 跳过数为 0 时返回 `None`（不产生任何日志）。
    fn describe(&self, connection_id: &str, total_limit: usize) -> Option<String> {
        if self.count == 0 {
            return None;
        }

        Some(format!(
            "[global-search] 云盘索引连接 {} 达到上限，另有 {} 个目录未继续下钻（合计上限 {} 条）",
            connection_id, self.count, total_limit
        ))
    }
}

/// 递归遍历一个连接的目录树。
///
/// 目录是**广度优先**：一层一层往下走，每层都过一遍上限判定。
/// 单个目录读不出来（鉴权过期 / 限流 / 超时）只记日志，其余目录照常走。
async fn walk_connection(
    provider: &dyn crate::remote::RemoteProvider,
    connection_id: &str,
    docs: &mut Vec<RemoteIndexDoc>,
    total_limit: usize,
) {
    // 待访问的目录队列：(远端路径, 深度)。连接根的远端路径是空串。
    let mut pending: Vec<(String, usize)> = vec![(String::new(), 0)];
    // **本连接**已收集的条目数 —— 配额按连接算（批次 45 订正，见常量说明）。
    let mut connection_entries: usize = 0;
    // W-10：因上限被跳过的子目录，遍历结束后汇总一句。
    let mut skipped_dirs = SkippedDirs::default();

    while let Some((remote_path, depth)) = pending.pop() {
        if !walk_should_continue(depth, connection_entries, docs.len(), total_limit) {
            log::warn!(
                "[global-search] 云盘索引连接 {} 达到上限，停止遍历（深度 {}，本连接已收集 {} 条，总计 {} 条 / 合计上限 {} 条）",
                connection_id,
                depth,
                connection_entries,
                docs.len(),
                total_limit
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
                        if walk_should_continue(depth + 1, connection_entries, docs.len(), total_limit)
                        {
                            pending.push((child_path, depth + 1));
                        } else {
                            // W-10（批次 47）：这里原本对**每一个**被跳过的目录都打一条 WARN。
                            // 配额被填满时（实测百度一条连接就吃掉 20000 条），
                            // 日志会被刷成几万行，真正的告警全被埋掉。改成只计数，
                            // 本连接遍历结束后由 `SkippedDirs::describe` 汇总一句。
                            skipped_dirs.record();
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
                connection_id: connection_id.to_string(),
            });
            connection_entries += 1;

            if !walk_should_continue(depth, connection_entries, docs.len(), total_limit) {
                log::warn!(
                    "[global-search] 云盘索引连接 {} 达到条目上限（本连接 {} 条 / 合计上限 {} 条），停止遍历",
                    connection_id,
                    REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION,
                    total_limit
                );
                break;
            }
        }
    }

    // W-10：整条连接遍历完只出一条汇总，替代原先逐目录的 WARN。
    if let Some(message) = skipped_dirs.describe(connection_id, total_limit) {
        log::warn!("{message}");
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
    let (index, fields) = create_fresh_remote_index(index_path)?;
    let mut writer = create_bulk_index_writer(&index)?;

    // 云盘索引**必须**带 `connection_id` 字段 ——
    // 查询侧要靠它按当前勾选集合过滤（见 `enabled_connection_ids`）。
    let connection_id_field = fields
        .connection_id
        .expect("云盘索引 schema 必须带 connection_id 字段");

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
                connection_id_field => entry.connection_id.as_str(),
            ))
            .map_err(|error| error.to_string())?;
    }

    // W-7（批次 45）：重建此前只在**遍历阶段**逐连接打了「收集到 N 条」，
    // 提交成功/失败与耗时全程无声 —— 出问题时只能靠 `meta.json` 的 mtime 反推。
    let commit_started = std::time::Instant::now();
    writer.commit().map_err(|error| {
        log::warn!(
            "[global-search] 云盘索引提交失败（待写 {} 条）：{}",
            docs.len(),
            error
        );
        error.to_string()
    })?;
    REMOTE_INDEX_UNAVAILABLE_LOGGED.store(false, std::sync::atomic::Ordering::Relaxed);
    log::info!(
        "[global-search] 云盘索引提交完成：{} 条，commit={}ms，目录 {}",
        docs.len(),
        commit_started.elapsed().as_millis(),
        index_path.display()
    );

    Ok(docs.len() as u64)
}

/// W-8（批次 45）：查询侧「为什么没有云盘结果」的**一次性**日志开关。
///
/// 检索每敲一个字符都会走 `open_remote_reader`，逐次打会刷屏；
/// 而取证要的不是频次，是用户报「云盘搜不到」时，日志里能不能把它和
/// 「索引不存在 / 打不开 / schema 不符 / 当前没有勾选」区分开
/// —— 在加这几行之前，查询侧对云盘路径**零日志**，`None` 静默降级成只搜本地，
/// 「索引是空的」与「勾选过滤生效了」在日志里长得一模一样。
///
/// 口径：每种不可用原因**首次**出现时打一行 INFO，之后静默；
/// 索引被重写（`write_remote_index` 提交成功）后开关复位，
/// 下一次再不可用仍然会说一遍。进程重启自然复位。
static REMOTE_INDEX_UNAVAILABLE_LOGGED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// 打一行「本次查询云盘结果被忽略」的一次性日志；已经说过就静默。
fn log_remote_index_unavailable_once(message: &str) {
    if !REMOTE_INDEX_UNAVAILABLE_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        log::info!("[global-search] {}", message);
    }
}

/// 打开云盘索引的 reader。
///
/// 索引目录不存在 / 打不开 / schema 对不上，一律返回 `None` ——
/// 查询路径据此判定「没有云盘索引」，行为与加这个功能之前**完全一致**
/// （**返回值的语义没动**，只是每种原因现在会在日志里说清楚）。
pub(super) fn open_remote_reader(
    index_path: &Path,
) -> Option<(IndexReader, GlobalSearchIndexFields)> {
    if !index_path.exists() {
        log_remote_index_unavailable_once(&format!(
            "云盘索引目录不存在（{}），本次及后续查询只返回本地结果，直到下一次重建",
            index_path.display()
        ));
        return None;
    }

    let index = match Index::open_in_dir(index_path) {
        Ok(index) => index,
        Err(error) => {
            log::warn!(
                "[global-search] 云盘索引打不开（{}）：{}；本次查询忽略云盘结果",
                index_path.display(),
                error
            );
            return None;
        }
    };
    let (schema, fields) = build_remote_schema();

    if index.schema() != schema {
        log::warn!("[global-search] 云盘索引的 schema 与当前不一致，本次查询忽略云盘结果");
        return None;
    }

    let reader = match index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
    {
        Ok(reader) => reader,
        Err(error) => {
            log::warn!("[global-search] 云盘索引 reader 建立失败：{}；本次查询忽略云盘结果", error);
            return None;
        }
    };

    Some((reader, fields))
}

/// 云盘索引的当前状态。
///
/// 用来判断「要不要补跑一次重建」：
///
/// - `Missing`：目录不存在（还没建过索引）—— **不自动重建**，那要花网盘配额；
/// - `Stale`：目录在，但 schema 与当前版本对不上（升级换了索引格式）—— **必须重建**，
///   否则查询侧会一直忽略这份索引，用户表现为「升级之后云盘搜不到东西了」；
/// - `Ready`：目录在且 schema 兼容。
pub(super) fn remote_index_state(index_path: &Path) -> RemoteIndexState {
    if !index_path.exists() {
        return RemoteIndexState::Missing;
    }

    let Ok(index) = Index::open_in_dir(index_path) else {
        // 目录在却打不开 ⇒ 当成「坏了」：重建会把它整个换掉。
        return RemoteIndexState::Stale;
    };

    let (schema, _) = build_remote_schema();

    if index.schema() == schema {
        RemoteIndexState::Ready
    } else {
        RemoteIndexState::Stale
    }
}

/// app-data 目录 → user-data 目录。
///
/// 与 `remote/commands.rs`、`global_search/commands.rs` 用的是**同一个**算法
/// （`appDataDir + userStorageFiles.userDataDirName`）。
/// 两处必须一致 —— 算错的表现是「Rust 说这条连接不存在」，
/// 而界面上明明有（凭据槽位认不出来）。
pub(super) fn user_data_dir_in(base_dir: &Path) -> PathBuf {
    base_dir.join(
        &crate::user_storage_files_config::user_storage_files_config().user_data_dir_name,
    )
}

/// **当前**勾选了「加入全局检索」的连接 id 集合。
///
/// 这是查询侧的过滤依据：「勾了哪个网盘，就只搜哪个网盘」。
///
/// 读的是连接表里的 `includeInGlobalSearch` 标记（前端勾选时写盘），
/// 与「重建索引时传进去的集合」是**两套独立的真相**：
///
/// - 重建集合决定**索引里有什么**；
/// - 勾选集合决定**能不能被搜到**。
///
/// 之前只有前者，于是「重建中途失败 ⇒ 旧索引还在」时，
/// 已经被取消勾选的网盘仍然搜得到 —— 这就是批次 43 登记的遗留项 O-1。
///
/// ⚠️ **读失败时返回空集合（= 一条都不给），而不是放行全部**：
/// 漏网的代价是「用户取消勾选了还能搜到」，比「暂时搜不到」严重得多。
pub(super) fn enabled_connection_ids(user_data_dir: &Path) -> BTreeSet<String> {
    match crate::remote::load_connections(user_data_dir) {
        Ok(connections) => connections
            .into_iter()
            .filter(|connection| connection.include_in_global_search)
            .map(|connection| connection.id)
            .collect(),
        Err(error) => {
            log::warn!(
                "[global-search] 读取远程连接列表失败，云盘检索结果按「无勾选」处理：{}",
                error
            );
            BTreeSet::new()
        }
    }
}

/// 清空云盘索引（取消勾选 / 传空列表时用）。
pub(super) fn clear_remote_index(index_path: &Path) -> Result<(), String> {
    super::index::clear_index(index_path)
}

/// **仅测试用**：往连接表里写一批连接，用来构造「当前勾选了哪些网盘」这个前提。
///
/// `entries` 是 `(连接 id, 是否勾选参与全局检索)`。
#[cfg(test)]
pub(super) fn write_test_connections(user_data_dir: &Path, entries: &[(&str, bool)]) {
    use crate::remote::{ConnectionsFile, ProxyConfig, RemoteConnection, RemoteProtocol};

    std::fs::create_dir_all(user_data_dir).expect("create user data dir");

    let connections = entries
        .iter()
        .map(|(id, include_in_global_search)| RemoteConnection {
            id: (*id).to_string(),
            name: (*id).to_string(),
            protocol: RemoteProtocol::BaiduPan,
            host: "pan.example.com".to_string(),
            port: 443,
            username: "tester".to_string(),
            initial_path: "/".to_string(),
            local_path: None,
            proxy: ProxyConfig::Direct,
            credential_key: None,
            include_in_global_search: *include_in_global_search,
        })
        .collect();

    let file = ConnectionsFile { connections };
    std::fs::write(
        crate::remote::connections_path_in(user_data_dir),
        serde_json::to_string_pretty(&file).expect("serialize connections"),
    )
    .expect("write connections file");
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
            connection_id: "conn-a".to_string(),
        }
    }

    /// W-10（批次 47）：被上限跳过的目录只在每条连接末尾汇总一句。
    #[test]
    fn skipped_dirs_summarize_into_one_line_per_connection() {
        let mut skipped = SkippedDirs::default();
        assert!(
            skipped.describe("conn-a", 20_000).is_none(),
            "没有目录被跳过时不应产生任何日志"
        );

        for _ in 0..3 {
            skipped.record();
        }

        let message = skipped
            .describe("conn-a", 20_000)
            .expect("跳过目录后必须有一条汇总日志");
        assert!(
            message.contains("conn-a"),
            "汇总日志要指明是哪个连接：{message}"
        );
        assert!(
            message.contains("另有 3 个目录"),
            "汇总日志要给出跳过目录数：{message}"
        );
        assert!(
            message.contains("20000"),
            "汇总日志要给出合计上限：{message}"
        );
        assert_eq!(message.lines().count(), 1, "汇总必须只有一行：{message}");
    }

    /// W-10：`describe` 是纯读 —— 一条连接只调用一次，但重复调用不得改变计数。
    #[test]
    fn skipped_dirs_describe_is_read_only() {
        let mut skipped = SkippedDirs::default();
        skipped.record();

        let first = skipped.describe("conn-a", 10).expect("应有一条汇总");
        let second = skipped.describe("conn-a", 10).expect("应仍有一条汇总");

        assert_eq!(first, second, "describe 不得修改内部计数");
        assert_eq!(skipped.count, 1, "describe 后计数仍应为 1");
    }

    /// 上限判定是纯函数 —— 没有网络也能把两条边界都钉住。
    #[test]
    fn the_walk_stops_at_the_depth_cap() {
        assert!(walk_should_continue(0, 0, 0, usize::MAX));
        assert!(walk_should_continue(REMOTE_INDEX_MAX_DEPTH, 0, 0, usize::MAX));
        assert!(!walk_should_continue(
            REMOTE_INDEX_MAX_DEPTH + 1,
            0,
            0,
            usize::MAX
        ));
    }

    #[test]
    fn the_walk_stops_at_the_entry_cap() {
        assert!(walk_should_continue(
            0,
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION - 1,
            0,
            usize::MAX
        ));
        assert!(!walk_should_continue(
            0,
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION,
            0,
            usize::MAX
        ));
        assert!(!walk_should_continue(
            0,
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION + 1,
            0,
            usize::MAX
        ));
    }

    /// **批次 45 订正的回归锁**：配额按连接算 ⇒
    /// 一个连接吃满自己的 20 000 条之后，**下一个连接仍然能开始遍历**。
    ///
    /// 旧口径（全局配额）下这条是假的：实测百度一条连接独占全部 20 000 条，
    /// 阿里云盘 / 夸克一条都没进索引 ⇒ 表现为「勾了另两个网盘也搜不到它们任何文件」。
    #[test]
    fn a_filled_connection_does_not_starve_the_next_one() {
        let total_limit = total_entry_limit(3);

        // 第一个连接已经吃满自己的配额（20 000 条）。
        assert!(!walk_should_continue(
            0,
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION,
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION,
            total_limit
        ));

        // 但**下一个连接**的配额是空的 ⇒ 它仍然可以开始遍历。
        assert!(walk_should_continue(
            0,
            0,
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION,
            total_limit
        ));
    }

    /// 合计上限 = 每连接配额 × 连接数（3 个连接 ⇒ 60 000 条）。
    #[test]
    fn the_total_limit_scales_with_the_connection_count() {
        assert_eq!(
            total_entry_limit(1),
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION
        );
        assert_eq!(
            total_entry_limit(3),
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION * 3
        );
        assert_eq!(
            total_entry_limit(0),
            REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION
        );
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

    // ---------------------------------------------------------------------
    // 「勾了哪个网盘，就只搜哪个网盘」—— 批次 44（台账 O-1）
    // ---------------------------------------------------------------------

    /// **当前勾选集合**只认 `includeInGlobalSearch = true` 的那些连接。
    ///
    /// 未勾选 / 老连接（根本没这个键）都**不**算。
    #[test]
    fn the_enabled_set_only_contains_the_checked_connections() {
        let temp = TempDir::new().expect("temp dir");
        let user_data_dir = temp.path().join("user-data");

        write_test_connections(
            &user_data_dir,
            &[("conn-a", true), ("conn-b", false), ("conn-c", true)],
        );

        let enabled = enabled_connection_ids(&user_data_dir);

        assert!(enabled.contains("conn-a"));
        assert!(!enabled.contains("conn-b"), "没勾选的不能进集合");
        assert!(enabled.contains("conn-c"));
        assert_eq!(enabled.len(), 2);
    }

    /// 连接表**不存在** ⇒ 一条都不算勾选（不是「全部放行」）。
    #[test]
    fn a_missing_connections_file_yields_an_empty_enabled_set() {
        let temp = TempDir::new().expect("temp dir");

        assert!(enabled_connection_ids(&temp.path().join("user-data")).is_empty());
    }

    /// 连接表**读不出来**（这里用「写了个不是 JSON 的文件」模拟）⇒
    /// 按「没有勾选任何网盘」处理，**不能**放行全部。
    ///
    /// 漏网的代价（用户取消勾选了还能搜到）比暂时搜不到严重得多。
    #[test]
    fn a_broken_connections_file_yields_an_empty_enabled_set() {
        let temp = TempDir::new().expect("temp dir");
        let user_data_dir = temp.path().join("user-data");
        std::fs::create_dir_all(&user_data_dir).expect("create user data dir");
        std::fs::write(
            crate::remote::connections_path_in(&user_data_dir),
            "{ this is not json",
        )
        .expect("write broken file");

        assert!(enabled_connection_ids(&user_data_dir).is_empty());
    }

    /// 云盘索引的状态三态：`missing` → 建过 → `ready`。
    #[test]
    fn the_remote_index_state_reports_missing_before_anything_is_built() {
        let temp = TempDir::new().expect("temp dir");
        let index_path = temp.path().join("remote-index");

        assert_eq!(remote_index_state(&index_path), RemoteIndexState::Missing);
    }

    #[test]
    fn the_remote_index_state_is_ready_once_it_is_built() {
        let temp = TempDir::new().expect("temp dir");
        let index_path = temp.path().join("remote-index");

        write_remote_index(&index_path, &[doc("sfm://remote/conn-a/a.txt", "a.txt", false)])
            .expect("write");

        assert_eq!(remote_index_state(&index_path), RemoteIndexState::Ready);
    }

    /// **老格式的索引必须被判成 `stale`**（而不是 `ready`）。
    ///
    /// 老索引里没有 `connection_id` 字段，查询侧打不出连接过滤 ⇒
    /// 只能当成「不可用」，让前端补跑一次重建。
    #[test]
    fn an_index_built_with_the_old_schema_is_stale() {
        let temp = TempDir::new().expect("temp dir");
        let index_path = temp.path().join("remote-index");

        // 用**本地**那份 schema 建一份索引 —— 它正是「老格式」的样子。
        let (index, fields) = super::super::index::create_fresh_index(&index_path).expect("create");
        let mut writer = super::super::index::create_bulk_index_writer(&index).expect("writer");
        writer
            .add_document(doc!(
                fields.path => "sfm://remote/conn-a/a.txt",
                fields.name => "a.txt",
                fields.name_lower => "a.txt",
                fields.is_file => 1u64,
                fields.is_dir => 0u64,
                fields.modified_time => 0u64,
                fields.size => 0u64,
            ))
            .expect("add doc");
        writer.commit().expect("commit");
        drop(writer);
        drop(index);

        assert_eq!(remote_index_state(&index_path), RemoteIndexState::Stale);
        // 老索引对查询侧也是「打不开」的 —— 查询结果里不会出现它的条目。
        assert!(open_remote_reader(&index_path).is_none());
    }

    /// 云盘索引的 schema 与**本地**索引的不同 ——
    /// 这正是「本地索引不用重建」的前提（本地索引是全盘扫描建出来的）。
    #[test]
    fn the_remote_schema_is_not_the_local_one() {
        let (local_schema, local_fields) = super::super::index::build_schema();
        let (remote_schema, remote_fields) = super::super::index::build_remote_schema();

        assert_ne!(local_schema, remote_schema);
        assert!(
            local_fields.connection_id.is_none(),
            "本地索引不许带 connection_id 字段"
        );
        assert!(
            remote_fields.connection_id.is_some(),
            "云盘索引必须带 connection_id 字段"
        );
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
