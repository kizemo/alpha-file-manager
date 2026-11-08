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
