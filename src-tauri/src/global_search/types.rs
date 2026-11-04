// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GlobalSearchScanReason {
    Manual,
    Startup,
    Idle,
    DriveChange,
    SettingsChange,
}

fn default_scan_reason() -> GlobalSearchScanReason {
    GlobalSearchScanReason::Manual
}

/// 这一轮扫描走的是哪条路。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GlobalSearchScanMode {
    /// 全量重扫 —— 重建整棵索引。
    Full,
    /// 增量更新 —— 只把 watcher 报出来的变化应用到活索引。
    Incremental,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GlobalSearchScanPhase {
    Idle,
    Scanning,
    Canceling,
    Committing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GlobalSearchScanOutcome {
    Completed,
    Canceled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchSettings {
    pub scan_depth: usize,
    pub ignored_paths: Vec<String>,
    pub drive_roots: Vec<String>,
    pub parallel_scan: bool,
    #[serde(default = "default_scan_reason")]
    pub scan_reason: GlobalSearchScanReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchDriveScanError {
    pub drive_root: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchQueryOptions {
    pub limit: usize,
    pub include_files: bool,
    pub include_directories: bool,
    pub exact_match: bool,
    pub typo_tolerance: bool,
    pub min_score_threshold: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchResultEntry {
    pub name: String,
    pub ext: Option<String>,
    pub path: String,
    pub size: u64,
    pub item_count: Option<u32>,
    pub modified_time: u64,
    pub accessed_time: u64,
    pub created_time: u64,
    pub mime: Option<String>,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_hidden: bool,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchStatus {
    pub is_scan_in_progress: bool,
    pub is_committing: bool,
    pub is_parallel_scan: bool,
    pub scan_phase: GlobalSearchScanPhase,
    pub scan_reason: Option<GlobalSearchScanReason>,
    pub last_scan_time: Option<u64>,
    pub last_scan_outcome: Option<GlobalSearchScanOutcome>,
    pub last_scan_reason: Option<GlobalSearchScanReason>,
    pub last_scan_started_time: Option<u64>,
    pub last_scan_finished_time: Option<u64>,
    pub last_scan_duration_ms: Option<u64>,
    pub last_scan_indexed_item_count: Option<u64>,
    pub last_scan_error: Option<String>,
    /// 上一轮扫描走的是全量还是增量。
    pub last_scan_mode: Option<GlobalSearchScanMode>,
    /// 增量通道是否就绪（活索引有效 + watcher 在跑 + 代次对得上）。
    pub incremental_ready: bool,
    /// 增量 watcher 的代次 —— 换过一次就说明中间有缺口，不能只做增量。
    pub incremental_watcher_generation: u64,
    /// 上一轮增量应用是否失败。
    pub last_incremental_failed: bool,
    pub indexed_item_count: u64,
    pub scan_indexed_item_count: u64,
    pub indexed_drive_roots: Vec<String>,
    pub index_size_bytes: u64,
    pub current_drive_root: Option<String>,
    pub current_scan_path: Option<String>,
    pub drive_scan_errors: Vec<GlobalSearchDriveScanError>,
    pub is_index_valid: bool,
    pub scanned_drives_count: u32,
    pub total_drives_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexPathsSettings {
    pub paths: Vec<String>,
    pub scan_depth: usize,
    pub ignored_paths: Vec<String>,
}
