// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use super::types::{
    GlobalSearchQueryOptions, GlobalSearchResultEntry, GlobalSearchSettings, GlobalSearchStatus,
    IndexPathsSettings,
};

#[tauri::command]
pub async fn global_search_init(app: tauri::AppHandle) -> Result<GlobalSearchStatus, String> {
    tauri::async_runtime::spawn_blocking(move || super::scan::global_search_init(app))
        .await
        .map_err(|join_error| format!("Global search init task failed: {join_error}"))?
}

// FORK-MODIFICATION 2026-11-08：状态查询 / 取消扫描改成 async（挪到工作线程）。
//
// Tauri v2 的**同步 command 跑在主线程**，而主线程就是窗口的消息泵 ——
// 在这里等锁（全局搜索的状态锁）或等磁盘，主线程一停，窗口就不再响应，
// Windows 直接判「未响应」。用户 2026-11-08 反馈「点左侧导航切换时出现无响应」，
// 应用日志里能看到 91 次心跳迟到 0.8~4.7 秒（心跳走 `invoke`，迟到即主线程被堵）。
//
// 状态查询是纯读、而且前端在扫描期间一直轮询它，最适合放到工作线程；
// 取消扫描只是置一个原子标志，放到工作线程同样没有语义变化。
#[tauri::command]
pub async fn global_search_get_status() -> Result<GlobalSearchStatus, String> {
    tauri::async_runtime::spawn_blocking(super::scan::global_search_get_status)
        .await
        .map_err(|join_error| format!("Global search status task failed: {join_error}"))?
}

#[tauri::command]
pub async fn global_search_cancel_scan() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(super::scan::global_search_cancel_scan)
        .await
        .map_err(|join_error| format!("Global search cancel task failed: {join_error}"))?
}

#[tauri::command]
pub async fn global_search_start_scan(
    app: tauri::AppHandle,
    settings: GlobalSearchSettings,
) -> Result<(), String> {
    super::scan::global_search_start_scan(app, settings).await
}

#[tauri::command]
pub async fn global_search_index_paths(
    app: tauri::AppHandle,
    settings: IndexPathsSettings,
) -> Result<u64, String> {
    super::scan::global_search_index_paths(app, settings).await
}

#[tauri::command]
pub async fn global_search_query(
    app: tauri::AppHandle,
    query: String,
    options: GlobalSearchQueryOptions,
) -> Result<Vec<GlobalSearchResultEntry>, String> {
    super::query::global_search_query(app, query, options).await
}

#[tauri::command]
pub async fn global_search_query_paths(
    paths: Vec<String>,
    query: String,
    options: GlobalSearchQueryOptions,
) -> Result<Vec<GlobalSearchResultEntry>, String> {
    super::query::global_search_query_paths(paths, query, options).await
}

/// 云盘索引重建的**全局**互斥锁（W-9，批次 47）。
///
/// 为什么必须是全局一把、而不是每个连接一把：一轮重建对索引目录是
/// **先删（`clear_remote_index`）后建（`write_remote_index`）**，
/// 两轮重建并发跑就会互相删掉对方正在写的段文件。批次 45 收尾实测：
/// 12:41:28 与 12:49:29 两次重建并发抢同一目录，中途采样看到 `segments: []`
/// 的「假空索引」，用户表现为「云盘明明勾着却搜不到东西」。
///
/// 前端（`use-remote-global-search-index.ts`）只有一把**模块级**串行锁，
/// CDP 直调 / 任何后端调用都会绕过它 ⇒ 闸门必须落在后端。
static REBUILD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 拿重建锁；已有重建在跑时先记一条 WARN 再排队。
///
/// 排队而不是直接失败：调用方拿到的返回值契约是「本次重建写入了多少条」，
/// 让它排队等前一轮跑完，语义仍然是「从零重建一次」。日志里那句 WARN 是为了
/// 事后能区分「重建本身慢」和「重建在排队等前一轮」（W-7 那套取证日志的延续）。
async fn acquire_rebuild_lock() -> tokio::sync::MutexGuard<'static, ()> {
    match REBUILD_LOCK.try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            log::warn!("[global-search] 已有云盘索引重建在进行，本次重建排队等待（W-9）");
            REBUILD_LOCK.lock().await
        }
    }
}

/// 按给定的连接 id **从零重建**云盘（远程连接）索引，返回写入的条目总数。
///
/// 语义（与前端约定的契约）：
///
/// - **先删后建** ⇒ 取消勾选的连接不会在索引里留下残条目；
/// - **按连接隔离失败**：一个云盘 400 / 超时 / 鉴权过期只记日志，
///   其它连接照常建索引，最后仍然返回 `Ok(条目数)`；
/// - `connection_ids` 为空 ⇒ 清空云盘索引并返回 `0`。
#[tauri::command]
pub async fn global_search_index_remote_connections(
    app: tauri::AppHandle,
    connection_ids: Vec<String>,
) -> Result<u64, String> {
    // W-9（批次 47）：整轮重建串行化。
    // 拿不到锁就排队 —— 排队必须发生在**任何**文件操作之前，
    // 否则两轮重建还是会同时去删同一个索引目录。
    let _rebuild_guard = acquire_rebuild_lock().await;

    let base_dir =
        crate::app_dirs::app_data_dir(&app).map_err(|error: tauri::Error| error.to_string())?;

    // 与 `remote/commands.rs` 的 `user_data_dir` **同源**：
    // `appDataDir + userStorageFiles.userDataDirName`。
    // 两处必须一致 —— 算错的表现是「Rust 说没有这条连接」，而界面上明明有。
    let user_data_dir = super::remote_index::user_data_dir_in(&base_dir);

    let index_path = super::index::remote_index_dir(&base_dir);

    // W-7（批次 45）：重建的入口/出口/耗时此前全程无声，
    // 日志里只有逐连接的「收集到 N 条」，看不出这场重建到底开没开始、写完没写完、花了多久。
    let rebuild_started = std::time::Instant::now();
    log::info!(
        "[global-search] 云盘索引重建开始：{} 个连接 {}",
        connection_ids.len(),
        connection_ids.join(",")
    );

    if connection_ids.is_empty() {
        super::remote_index::clear_remote_index(&index_path)?;
        log::info!(
            "[global-search] 云盘索引重建：连接列表为空，已清空索引目录（{} 连接未勾「加入全局检索」）",
            index_path.display()
        );
        return Ok(0);
    }

    // 遍历要走网络 ⇒ 在 async 上下文里直接 `.await` provider 调用。
    // **不要**在 `spawn_blocking` 里再套一层 tokio runtime
    // （`Runtime::block_on` 在运行时内部会 panic：
    // "Cannot start a runtime from within a runtime"）。
    let collect_started = std::time::Instant::now();
    let docs =
        super::remote_index::collect_remote_entries(&user_data_dir, &base_dir, &connection_ids)
            .await;
    let collected = docs.len();
    log::info!(
        "[global-search] 云盘索引收集完成：{} 条，collect={}ms（每连接上限 {} 条）",
        collected,
        collect_started.elapsed().as_millis(),
        super::remote_index::REMOTE_INDEX_MAX_ENTRIES_PER_CONNECTION
    );

    // 写盘（tantivy）挪到工作线程，别占着 async 运行时的工作线程。
    let indexed = tauri::async_runtime::spawn_blocking(move || {
        super::remote_index::write_remote_index(&index_path, &docs)
    })
    .await
    .map_err(|join_error| {
        log::warn!("[global-search] 云盘索引写盘线程崩溃：{}", join_error);
        format!("Remote search index task failed: {join_error}")
    })??;

    log::info!(
        "[global-search] 云盘索引重建完成：写入 {} 条，总耗时 {}ms",
        indexed,
        rebuild_started.elapsed().as_millis()
    );
    Ok(indexed)
}

/// 云盘索引的当前状态（`missing` / `stale` / `ready`）。
///
/// 前端进「设置 → 搜索」页时用它决定「要不要补跑一次重建」：
///
/// - `stale` ⇒ 盘上的索引是**老格式**（升级换了索引 schema，
///   老索引里没有 `connection_id` 字段）⇒ 必须重建一次，
///   否则查询侧会一直忽略这份索引，用户表现为「升级之后云盘搜不到东西」；
/// - `missing` ⇒ 还没建过索引 ⇒ **不自动重建**（那要花网盘 API 配额，
///   用户没主动要求就不该替他花）。
#[tauri::command]
pub async fn global_search_remote_index_state(app: tauri::AppHandle) -> Result<String, String> {
    let base_dir =
        crate::app_dirs::app_data_dir(&app).map_err(|error: tauri::Error| error.to_string())?;
    let index_path = super::index::remote_index_dir(&base_dir);

    let state = super::remote_index::remote_index_state(&index_path);

    Ok(match state {
        super::types::RemoteIndexState::Missing => "missing",
        super::types::RemoteIndexState::Stale => "stale",
        super::types::RemoteIndexState::Ready => "ready",
    }
    .to_string())
}

// FORK-MODIFICATION: 需求反馈 3 —— 目录内搜索递归化。
// 与 global_search_query_paths 分开而不是复用：后者对每个路径只读一层，
// 且被全局搜索的「优先路径」流程依赖，改它的语义会波及那条链路。
#[tauri::command]
pub async fn search_directory_recursive(
    root_path: String,
    query: String,
    max_depth: u32,
    limit: u32,
    typo_tolerance: bool,
) -> Result<Vec<GlobalSearchResultEntry>, String> {
    super::query::search_directory_recursive(root_path, query, max_depth, limit, typo_tolerance).await
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::*;

    /// W-9（批次 47）：两轮云盘索引重建**不得**同时进行。
    ///
    /// 判定量是「在飞行中」的峰值：四个重建任务同时抢锁，峰值必须是 1。
    /// 不加锁时这里是 4 —— 那正是批次 45 实测的成因
    /// （12:41:28 与 12:49:29 两轮重建同时去删同一个索引目录，
    /// 中途采样到 `segments: []` 的假空索引）。
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_rebuild_lock_never_lets_two_rebuilds_run_at_once() {
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..4 {
            let in_flight = Arc::clone(&in_flight);
            let peak = Arc::clone(&peak);
            handles.push(tokio::spawn(async move {
                let _guard = acquire_rebuild_lock().await;
                let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                in_flight.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for handle in handles {
            handle.await.expect("重建任务不应 panic");
        }

        assert_eq!(
            peak.load(Ordering::SeqCst),
            1,
            "两轮重建不得同时进行：峰值并发必须是 1"
        );
    }
}
