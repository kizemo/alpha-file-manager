// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use super::types::{GlobalSearchScanPhase, GlobalSearchStatus};
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tantivy::schema::Field;
use tantivy::{Index, IndexReader};

#[derive(Debug, Clone, Copy)]
pub(super) struct GlobalSearchIndexFields {
    pub(super) path: Field,
    pub(super) name: Field,
    pub(super) name_lower: Field,
    pub(super) is_file: Field,
    pub(super) is_dir: Field,
    pub(super) modified_time: Field,
    pub(super) size: Field,
}

pub(super) struct GlobalSearchState {
    pub(super) status: GlobalSearchStatus,
    pub(super) index: Option<Index>,
    pub(super) reader: Option<IndexReader>,
    pub(super) fields: Option<GlobalSearchIndexFields>,
    pub(super) cancel_flag: Arc<AtomicBool>,
}

pub(super) static GLOBAL_SEARCH_STATE: Lazy<Arc<RwLock<GlobalSearchState>>> = Lazy::new(|| {
    // 把「上一次查询时间」在状态初始化时就钉住（通常≈进程启动时刻），
    // 这样「启动后一直没人搜索」也能在超时后释放常驻 reader。
    let _ = &*LAST_QUERY_AT_MS;

    Arc::new(RwLock::new(GlobalSearchState {
        status: GlobalSearchStatus {
            is_scan_in_progress: false,
            is_committing: false,
            is_parallel_scan: false,
            scan_phase: GlobalSearchScanPhase::Idle,
            scan_reason: None,
            last_scan_time: None,
            last_scan_outcome: None,
            last_scan_reason: None,
            last_scan_started_time: None,
            last_scan_finished_time: None,
            last_scan_duration_ms: None,
            last_scan_indexed_item_count: None,
            last_scan_error: None,
            indexed_item_count: 0,
            scan_indexed_item_count: 0,
            indexed_drive_roots: vec![],
            index_size_bytes: 0,
            current_drive_root: None,
            current_scan_path: None,
            drive_scan_errors: vec![],
            is_index_valid: false,
            scanned_drives_count: 0,
            total_drives_count: 0,
        },
        index: None,
        reader: None,
        fields: None,
        cancel_flag: Arc::new(AtomicBool::new(false)),
    }))
});

pub(super) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

// FORK-MODIFICATION: 2026-10-28 —— 搜索常驻 reader 的空闲释放。
//
// tantivy 的 `IndexReader` **没有内存预算旋钮**（`IndexReaderBuilder` 只有
// `reload_policy` 与 `try_into`），能做的只有「不用就放掉」：常驻的 reader 会
// 一直攥着各个 segment 的 store / 倒排 mmap，索引越大常驻占用越可观。
//
// 这里采取的策略是「空闲即释放」：
//   * 超过阈值没有查询 ⇒ 把 `state.reader` 置空；
//   * 下次查询走 `query.rs` 里**已经存在**的 `needs_open` 重新打开路径
//     （`open_or_create_index`），索引本身不受影响，搜索结果不变。
//
// 释放动作只在写锁内做，因此不可能与正在跑的查询抢同一份 reader
// （查询全程持有读锁）。
//
// 阈值取 5 分钟：空闲 5 分钟后就把常驻 reader 归还（重开一次索引是百毫秒级，
// 用户无感），同时不至于让「连续搜索」的用户频繁付重开的代价。
pub(super) const READER_IDLE_RELEASE_AFTER_MS: u64 = 5 * 60 * 1000;

/// 上一次查询的时间戳（毫秒）。
///
/// 首次访问时初始化（进程启动阶段被 `GLOBAL_SEARCH_STATE` 的初始化顺带钉住，
// 若状态更晚才被碰到，则退化成「第一次查询的时刻」）。
pub(super) static LAST_QUERY_AT_MS: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(now_millis()));

/// 记一次查询活动（空闲释放的判断以「上一次查询时间」为准）。
pub(super) fn mark_query_activity() {
    LAST_QUERY_AT_MS.store(now_millis(), Ordering::Relaxed);
}

/// 常驻 reader 是否已经空闲超过阈值。
pub(super) fn is_reader_idle(now_ms: u64) -> bool {
    let last_query_at = LAST_QUERY_AT_MS.load(Ordering::Relaxed);
    now_ms.saturating_sub(last_query_at) >= READER_IDLE_RELEASE_AFTER_MS
}
