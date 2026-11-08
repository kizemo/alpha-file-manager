// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use crate::utils::{metadata_modified_time_unix_ms, normalize_path};
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tantivy::collector::Count;
use tantivy::query::{RegexQuery, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::{doc, Index, IndexReader, IndexWriter, Term};
use walkdir::WalkDir;

use super::ignore::{builtin_ignored_paths, normalize_case, IgnoredPathMatcher};
use super::incremental::{
    compile_changes, decide_incremental_plan, is_under, prefix_delete_pattern, same_drive_roots,
    ChangeSet, IncrementalGate, IncrementalPlan, IndexOp, PathKind,
};
use super::index::{
    calculate_dir_size, cleanup_orphan_index_dirs, clear_index, create_bulk_index_writer,
    create_fresh_index, index_dir, merge_index_segments, open_or_create_index, read_meta,
    remove_dir_force, replace_index_dir, staging_index_dir, validate_index, validate_staged_index,
    write_meta, GlobalSearchMeta, SCHEMA_VERSION,
};
use super::index_watcher::{self, WatcherSnapshot};
use super::state::{now_millis, GlobalSearchIndexFields, GlobalSearchState, GLOBAL_SEARCH_STATE};
use super::types::{
    GlobalSearchDriveScanError, GlobalSearchScanMode, GlobalSearchScanOutcome,
    GlobalSearchScanPhase, GlobalSearchScanReason, GlobalSearchSettings, GlobalSearchStatus,
    IndexPathsSettings,
};

const STATUS_UPDATE_INTERVAL: u64 = 500;

#[cfg(windows)]
fn is_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &Metadata) -> bool {
    false
}
struct CommittedIndexUpdate {
    doc_count: u64,
    index_size_bytes: u64,
    index: Index,
    reader: IndexReader,
    fields: GlobalSearchIndexFields,
    indexed_drive_roots: Option<Vec<String>>,
}

fn meta_from_status(state: &GlobalSearchState) -> GlobalSearchMeta {
    GlobalSearchMeta {
        last_scan_time: state.status.last_scan_time,
        indexed_item_count: state.status.indexed_item_count,
        schema_version: SCHEMA_VERSION,
        last_scan_outcome: state.status.last_scan_outcome.clone(),
        last_scan_reason: state.status.last_scan_reason.clone(),
        last_scan_started_time: state.status.last_scan_started_time,
        last_scan_finished_time: state.status.last_scan_finished_time,
        last_scan_duration_ms: state.status.last_scan_duration_ms,
        last_scan_indexed_item_count: state.status.last_scan_indexed_item_count,
        last_scan_error: state.status.last_scan_error.clone(),
        indexed_drive_roots: state.status.indexed_drive_roots.clone(),
        last_scan_mode: state.status.last_scan_mode,
        incremental_ready: state.status.incremental_ready,
        incremental_watcher_generation: state.status.incremental_watcher_generation,
        last_incremental_failed: state.status.last_incremental_failed,
    }
}

fn scan_outcome_for_result(result_succeeded: bool, was_cancelled: bool) -> GlobalSearchScanOutcome {
    if was_cancelled {
        GlobalSearchScanOutcome::Canceled
    } else if result_succeeded {
        GlobalSearchScanOutcome::Completed
    } else {
        GlobalSearchScanOutcome::Failed
    }
}

fn set_current_scan_path(path: Option<String>) {
    if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
        state.status.current_scan_path = path;
    }
}

fn should_skip_link_metadata(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink() || is_reparse_point(metadata)
}

fn maybe_set_current_scan_path(path: String, update_counter: &AtomicU64) {
    let update_count = update_counter.fetch_add(1, Ordering::Relaxed);
    if update_count % STATUS_UPDATE_INTERVAL == 0 {
        set_current_scan_path(Some(path));
    }
}

fn should_scan_walk_entry(
    path: &Path,
    ignored_matcher: &IgnoredPathMatcher,
    path_update_counter: &AtomicU64,
) -> bool {
    let path_string = path.to_string_lossy().to_string();
    let normalized = normalize_path(&path_string);

    maybe_set_current_scan_path(normalized.clone(), path_update_counter);

    if ignored_matcher.is_ignored(&normalized) {
        return false;
    }

    match std::fs::symlink_metadata(path) {
        Ok(metadata) => !should_skip_link_metadata(&metadata),
        Err(_) => false,
    }
}

fn apply_committed_index_status(
    state: &mut GlobalSearchState,
    base_dir: &Path,
    update: CommittedIndexUpdate,
    advance_last_scan_time: bool,
) {
    let CommittedIndexUpdate {
        doc_count,
        index_size_bytes,
        index,
        reader,
        fields,
        indexed_drive_roots,
    } = update;
    state.index = Some(index);
    state.reader = Some(reader);
    state.fields = Some(fields);
    state.status.indexed_item_count = doc_count;
    state.status.index_size_bytes = index_size_bytes;
    state.status.is_index_valid = doc_count > 0;
    if let Some(drive_roots) = indexed_drive_roots {
        state.status.indexed_drive_roots = drive_roots;
    }
    if advance_last_scan_time {
        state.status.last_scan_time = Some(now_millis());
    }
    let _ = write_meta(base_dir, &meta_from_status(state));
}

pub fn global_search_init(app: tauri::AppHandle) -> Result<GlobalSearchStatus, String> {
    // 应用启动时把上一个生命周期残留的 watcher 状态清干净。
    // 正常情况下进程重启就已经没有 watcher 了，但单元测试与热重载下
    // 可能是同一个进程；显式关一次，避免拿到一个「看起来在监听、
    // 实际上收不到事件」的僵尸 watcher。
    super::index_watcher::stop_watcher();

    let base_dir =
        crate::app_dirs::app_data_dir(&app).map_err(|error: tauri::Error| error.to_string())?;

    cleanup_orphan_index_dirs(&base_dir);

    let index_path = index_dir(&base_dir);
    let is_valid = validate_index(&index_path, &base_dir);

    if !is_valid {
        let _ = clear_index(&index_path);
    }

    let (index, reader, fields) = open_or_create_index(&index_path)?;

    let indexed_item_count = reader.searcher().num_docs();
    let index_size = calculate_dir_size(&index_path);
    let meta = read_meta(&base_dir);

    let mut state = GLOBAL_SEARCH_STATE
        .write()
        .map_err(|error| error.to_string())?;

    state.status.indexed_item_count = indexed_item_count;
    state.status.index_size_bytes = index_size;
    state.status.last_scan_time = meta
        .as_ref()
        .and_then(|meta_entry| meta_entry.last_scan_time);
    state.status.last_scan_outcome = meta
        .as_ref()
        .and_then(|meta_entry| meta_entry.last_scan_outcome.clone());
    state.status.last_scan_reason = meta
        .as_ref()
        .and_then(|meta_entry| meta_entry.last_scan_reason.clone());
    state.status.last_scan_started_time = meta
        .as_ref()
        .and_then(|meta_entry| meta_entry.last_scan_started_time);
    state.status.last_scan_finished_time = meta
        .as_ref()
        .and_then(|meta_entry| meta_entry.last_scan_finished_time);
    state.status.last_scan_duration_ms = meta
        .as_ref()
        .and_then(|meta_entry| meta_entry.last_scan_duration_ms);
    state.status.last_scan_indexed_item_count = meta
        .as_ref()
        .and_then(|meta_entry| meta_entry.last_scan_indexed_item_count);
    state.status.last_scan_error = meta
        .as_ref()
        .and_then(|meta_entry| meta_entry.last_scan_error.clone());
    state.status.indexed_drive_roots = if is_valid {
        meta.as_ref()
            .map(|meta_entry| meta_entry.indexed_drive_roots.clone())
            .unwrap_or_default()
    } else {
        vec![]
    };
    state.status.is_index_valid = is_valid && indexed_item_count > 0;
    state.status.scan_phase = GlobalSearchScanPhase::Idle;
    state.status.scan_reason = None;
    state.status.scan_indexed_item_count = 0;
    state.status.current_scan_path = None;

    // 停机期间的变化没法靠 watcher 补 —— 进程刚起来时增量通道一律不可用，
    // 第一轮扫描必然是全量。
    state.status.incremental_ready = false;
    state.status.incremental_watcher_generation = 0;
    state.status.last_incremental_failed = false;
    state.status.last_scan_mode = meta.as_ref().and_then(|entry| entry.last_scan_mode);

    // 应用自己的目录不进索引：索引目录会把索引自己写进索引，
    // WebView2 缓存是几十万个小文件的纯噪音。
    let mut app_owned_prefixes = vec![normalize_path(&base_dir.to_string_lossy())];
    if let Ok(local_data_dir) = crate::app_dirs::resolve(&app, crate::app_dirs::AppDir::LocalData) {
        app_owned_prefixes.push(normalize_path(&local_data_dir.to_string_lossy()));
    }
    state.app_owned_prefixes = app_owned_prefixes;
    state.index = Some(index);
    state.reader = Some(reader);
    state.fields = Some(fields);

    Ok(state.status.clone())
}

fn add_path_doc(
    writer: &IndexWriter,
    fields: &GlobalSearchIndexFields,
    path: &Path,
    path_string: &str,
) -> bool {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(_) => return false,
    };

    if should_skip_link_metadata(&metadata) {
        return false;
    }

    let is_dir = metadata.is_dir();
    let is_file = metadata.is_file();

    let name = match path.file_name().and_then(|segment| segment.to_str()) {
        Some(segment) => segment.to_string(),
        None => return false,
    };

    let name_lower = normalize_case(&name);
    let modified_time = metadata_modified_time_unix_ms(&metadata);

    let size = if is_file { metadata.len() } else { 0 };

    writer
        .add_document(doc!(
            fields.path => path_string.to_string(),
            fields.name => name,
            fields.name_lower => name_lower,
            fields.is_file => if is_file { 1u64 } else { 0u64 },
            fields.is_dir => if is_dir { 1u64 } else { 0u64 },
            fields.modified_time => modified_time,
            fields.size => size,
        ))
        .is_ok()
}

// ---------------------------------------------------------------------------
// 增量索引的执行层
//
// 计划层在 `incremental.rs`（纯逻辑，可单测）；这里是它落到 tantivy 上的地方。
// 这一层同样不碰 tauri —— 它必须能进 `cargo test --lib`。
// ---------------------------------------------------------------------------

/// 一轮增量应用的结果。
#[derive(Debug, Default, Clone)]
pub(super) struct IncrementalApplyStats {
    /// 实际执行的操作数。
    pub(super) applied_operations: u64,
    /// 被上层覆盖前缀吃掉、因而不必执行的操作数。
    pub(super) skipped_operations: u64,
    /// 新写入索引的文档数。
    pub(super) upserted_documents: u64,
}

/// 路径在索引里吗？
///
/// 只判「在不在」，不取文档内容 —— `Count` collector 是最省的一条路。
fn path_is_indexed(reader: &IndexReader, fields: &GlobalSearchIndexFields, path: &str) -> bool {
    let searcher = reader.searcher();
    let query = TermQuery::new(
        Term::from_field_text(fields.path, path),
        IndexRecordOption::Basic,
    );

    matches!(searcher.search(&query, &Count), Ok(count) if count > 0)
}

/// 删掉路径本身的那一条文档。
fn delete_exact_path(writer: &IndexWriter, fields: &GlobalSearchIndexFields, path: &str) {
    writer.delete_term(Term::from_field_text(fields.path, path));
}

/// 删掉路径以及它下面全部的文档。
///
/// 用正则查询而不是「删掉 `dir/` 这个 term」：后者只命中路径**恰好**等于
/// `C:/dir/` 的那一条，子树一条都删不掉 —— 那是既有实现里的一个缺陷。
fn delete_prefix(
    writer: &IndexWriter,
    fields: &GlobalSearchIndexFields,
    prefix: &str,
) -> Result<(), String> {
    let pattern = prefix_delete_pattern(prefix);
    let query =
        RegexQuery::from_pattern(&pattern, fields.path).map_err(|error| error.to_string())?;

    writer
        .delete_query(Box::new(query))
        .map_err(|error| error.to_string())?;

    Ok(())
}

/// 把一整棵目录写进索引。
fn index_tree(
    writer: &IndexWriter,
    fields: &GlobalSearchIndexFields,
    root: &Path,
    ignored_matcher: &IgnoredPathMatcher,
    scan_depth: usize,
) -> u64 {
    let mut indexed = 0u64;

    for entry_result in WalkDir::new(root)
        .follow_links(false)
        .max_depth(scan_depth.max(1))
        .into_iter()
        .filter_entry(|entry| {
            !ignored_matcher.is_ignored(&normalize_path(&entry.path().to_string_lossy()))
        })
    {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if entry.depth() == 0 {
            continue;
        }

        let path = entry.path();
        let path_string = match path.to_str() {
            Some(value) => normalize_path(value),
            None => continue,
        };

        if ignored_matcher.is_ignored(&path_string) {
            continue;
        }

        if add_path_doc(writer, fields, path, &path_string) {
            indexed += 1;
        }
    }

    indexed
}

/// 执行一批索引操作。
pub(super) fn apply_index_ops(
    writer: &IndexWriter,
    reader: &IndexReader,
    fields: &GlobalSearchIndexFields,
    ops: &[IndexOp],
    ignored_matcher: &IgnoredPathMatcher,
    scan_depth: usize,
) -> Result<IncrementalApplyStats, String> {
    let mut stats = IncrementalApplyStats::default();
    // 已经整棵重扫过的前缀 —— 它下面的路径不必再单独处理。
    let mut covered_prefixes: Vec<String> = Vec::new();

    for op in ops {
        match op {
            IndexOp::DeletePrefix { prefix } => {
                delete_prefix(writer, fields, prefix)?;
                stats.applied_operations += 1;
            }
            IndexOp::UpsertFile { path } => {
                if covered_prefixes
                    .iter()
                    .any(|covered| is_under(path, covered))
                {
                    stats.skipped_operations += 1;
                    continue;
                }

                // 先删旧的那一条，再写新的 —— 否则同一个路径会有两条文档。
                delete_exact_path(writer, fields, path);

                if add_path_doc(writer, fields, Path::new(path.as_str()), path) {
                    stats.upserted_documents += 1;
                }

                stats.applied_operations += 1;
            }
            IndexOp::UpsertDir { path } => {
                if covered_prefixes
                    .iter()
                    .any(|covered| is_under(path, covered))
                {
                    stats.skipped_operations += 1;
                    continue;
                }

                delete_exact_path(writer, fields, path);

                if add_path_doc(writer, fields, Path::new(path.as_str()), path) {
                    stats.upserted_documents += 1;
                }

                stats.applied_operations += 1;
            }
            IndexOp::UpsertTree { prefix } => {
                if covered_prefixes
                    .iter()
                    .any(|covered| is_under(prefix, covered))
                {
                    stats.skipped_operations += 1;
                    continue;
                }

                delete_prefix(writer, fields, prefix)?;
                covered_prefixes.push(prefix.clone());

                stats.upserted_documents += index_tree(
                    writer,
                    fields,
                    Path::new(prefix.as_str()),
                    ignored_matcher,
                    scan_depth,
                );

                stats.applied_operations += 1;
            }
        }
    }

    Ok(stats)
}

/// 回答「这个路径在文件系统里是什么」。
fn make_upsert_kind<'a>(
    reader: &'a IndexReader,
    fields: &'a GlobalSearchIndexFields,
) -> impl Fn(&str) -> PathKind + 'a {
    move |path: &str| match std::fs::metadata(Path::new(path)) {
        Ok(metadata) if metadata.is_dir() => PathKind::Directory {
            already_indexed: path_is_indexed(reader, fields, path),
        },
        Ok(_) => PathKind::File,
        Err(_) => PathKind::Missing,
    }
}

/// 把 watcher 积压的事件变成索引操作。
fn collect_index_ops(reader: &IndexReader, fields: &GlobalSearchIndexFields) -> Vec<IndexOp> {
    let events = index_watcher::drain_events();

    if events.is_empty() {
        return Vec::new();
    }

    let mut change_set = ChangeSet::new();
    change_set.apply_events(&events);
    let changes = change_set.compacted();

    if changes.is_empty() {
        return Vec::new();
    }

    compile_changes(&changes, &make_upsert_kind(reader, fields))
}

/// 把积压的变化应用到活索引上。
fn flush_pending_changes(
    index: &Index,
    reader: &IndexReader,
    fields: &GlobalSearchIndexFields,
    ignored_matcher: &IgnoredPathMatcher,
    scan_depth: usize,
) -> Result<IncrementalApplyStats, String> {
    let ops = collect_index_ops(reader, fields);

    if ops.is_empty() {
        return Ok(IncrementalApplyStats::default());
    }

    let mut writer = create_bulk_index_writer(index)?;
    let stats = apply_index_ops(&writer, reader, fields, &ops, ignored_matcher, scan_depth)?;

    writer.commit().map_err(|error| error.to_string())?;

    if let Err(error) = merge_index_segments(&mut writer) {
        log::warn!(
            "[global-search] Failed to merge index segments after incremental commit: {}",
            error
        );
    }

    writer
        .wait_merging_threads()
        .map_err(|error| error.to_string())?;
    reader.reload().map_err(|error| error.to_string())?;

    Ok(stats)
}

/// 决定这一轮走增量还是全量。
fn decide_scan_route(
    base_dir: &Path,
    requested_roots: &[String],
    watcher_snapshot: &WatcherSnapshot,
) -> IncrementalPlan {
    let meta = read_meta(base_dir);

    let (
        has_valid_live_index,
        indexed_drive_roots,
        incremental_ready,
        recorded_generation,
        last_scan_succeeded,
        last_incremental_failed,
    ) = match GLOBAL_SEARCH_STATE.read() {
        Ok(state) => (
            state.status.is_index_valid && state.reader.is_some(),
            state.status.indexed_drive_roots.clone(),
            state.status.incremental_ready,
            state.status.incremental_watcher_generation,
            state.status.last_scan_outcome == Some(GlobalSearchScanOutcome::Completed),
            state.status.last_incremental_failed,
        ),
        Err(_) => (false, Vec::new(), false, 0, false, false),
    };

    let gate = IncrementalGate {
        has_valid_live_index,
        meta_schema_version: meta.as_ref().map(|entry| entry.schema_version).unwrap_or(0),
        current_schema_version: SCHEMA_VERSION,
        indexed_drive_roots,
        requested_drive_roots: requested_roots.to_vec(),
        last_full_scan_succeeded: last_scan_succeeded,
        watcher_ready: watcher_snapshot.running
            && incremental_ready
            && watcher_snapshot.generation == recorded_generation
            && same_drive_roots(&watcher_snapshot.roots, requested_roots),
        watcher_overflowed: watcher_snapshot.overflowed,
        last_incremental_failed,
    };

    decide_incremental_plan(&gate)
}

/// 一轮扫描结束后的状态收尾。
fn finalize_scan_state(
    base_dir: &Path,
    index_path: &Path,
    scan_started_time: u64,
    scan_reason: GlobalSearchScanReason,
    scan_mode: GlobalSearchScanMode,
    watcher_generation: u64,
    result: Result<CommittedIndexUpdate, String>,
    advance_last_scan_time: bool,
) {
    let Ok(mut state) = GLOBAL_SEARCH_STATE.write() else {
        return;
    };

    let scan_finished_time = now_millis();
    let scan_duration_ms = scan_finished_time.saturating_sub(scan_started_time);
    let progress_doc_count = state.status.scan_indexed_item_count;
    let was_cancelled = state.cancel_flag.load(Ordering::SeqCst);
    let scan_error = result.as_ref().err().cloned();
    let scan_outcome = scan_outcome_for_result(result.is_ok(), was_cancelled);
    let round_outcome = scan_outcome.clone();

    state.status.is_scan_in_progress = false;
    state.status.is_committing = false;
    state.status.is_parallel_scan = false;
    state.status.scan_phase = GlobalSearchScanPhase::Idle;
    state.status.scan_reason = None;
    state.status.current_drive_root = None;
    state.status.current_scan_path = None;
    state.status.last_scan_outcome = Some(scan_outcome.clone());
    state.status.last_scan_reason = Some(scan_reason.clone());
    state.status.last_scan_started_time = Some(scan_started_time);
    state.status.last_scan_finished_time = Some(scan_finished_time);
    state.status.last_scan_duration_ms = Some(scan_duration_ms);
    state.status.last_scan_indexed_item_count = Some(progress_doc_count);
    state.status.last_scan_mode = Some(scan_mode);
    state.status.last_scan_error = match scan_outcome {
        GlobalSearchScanOutcome::Failed => scan_error,
        _ => None,
    };
    state.status.scan_indexed_item_count = 0;

    match result {
        Ok(update) => {
            state.status.last_scan_indexed_item_count = Some(update.doc_count);
            state.status.last_incremental_failed = false;

            if scan_mode == GlobalSearchScanMode::Full {
                // 全量重扫成功 ⇒ 索引与磁盘重新对齐 ⇒ 增量通道可以重新启用。
                state.status.incremental_ready = true;
                state.status.incremental_watcher_generation = watcher_generation;
                index_watcher::clear_overflowed();
            }

            apply_committed_index_status(&mut state, base_dir, update, advance_last_scan_time);
        }
        Err(_) => {
            state.status.last_incremental_failed = scan_mode == GlobalSearchScanMode::Incremental;
            state.status.incremental_ready = false;

            if state.reader.is_none() {
                if let Ok((index, reader, fields)) = open_or_create_index(index_path) {
                    state.index = Some(index);
                    state.reader = Some(reader);
                    state.fields = Some(fields);
                }
            }

            let committed_doc_count = state
                .reader
                .as_ref()
                .map_or(0, |reader| reader.searcher().num_docs());
            state.status.indexed_item_count = committed_doc_count;
            state.status.index_size_bytes = calculate_dir_size(index_path);
            state.status.is_index_valid = committed_doc_count > 0;
            let _ = write_meta(base_dir, &meta_from_status(&state));
        }
    }

    log::info!(
        "[global-search] scan round timings: mode={:?} outcome={:?} elapsed={}ms indexed={} reason={:?}",
        scan_mode.clone(),
        round_outcome,
        scan_duration_ms,
        state.status.indexed_item_count,
        scan_reason.clone()
    );
}

/// 一轮增量扫描。
fn run_incremental_pass(
    base_dir: &Path,
    index_path: &Path,
    settings: &GlobalSearchSettings,
    scan_started_time: u64,
    scan_reason: GlobalSearchScanReason,
    watcher_generation: u64,
    app_owned_prefixes: &[String],
) {
    let result = (|| -> Result<CommittedIndexUpdate, String> {
        let (index, reader, fields) = open_or_create_index(index_path)?;

        let ignored_paths: Vec<String> = settings
            .ignored_paths
            .iter()
            .map(|path| path.to_string())
            .chain(builtin_ignored_paths().iter().map(|path| path.to_string()))
            .chain(app_owned_prefixes.iter().map(|path| path.to_string()))
            .collect();
        let ignored_matcher = IgnoredPathMatcher::new(&ignored_paths);

        let stats = flush_pending_changes(
            &index,
            &reader,
            &fields,
            &ignored_matcher,
            settings.scan_depth,
        )?;

        log::info!(
            "[global-search] incremental pass: operations={} skipped={} upserted={}",
            stats.applied_operations,
            stats.skipped_operations,
            stats.upserted_documents
        );

        let doc_count = reader.searcher().num_docs();
        let index_size = calculate_dir_size(index_path);

        Ok(CommittedIndexUpdate {
            doc_count,
            index_size_bytes: index_size,
            index,
            reader,
            fields,
            indexed_drive_roots: None,
        })
    })();

    finalize_scan_state(
        base_dir,
        index_path,
        scan_started_time,
        scan_reason,
        GlobalSearchScanMode::Incremental,
        watcher_generation,
        result,
        true,
    );
}

pub fn global_search_get_status() -> Result<GlobalSearchStatus, String> {
    let state = GLOBAL_SEARCH_STATE
        .read()
        .map_err(|error| error.to_string())?;
    Ok(state.status.clone())
}

pub fn global_search_cancel_scan() -> Result<(), String> {
    let mut state = GLOBAL_SEARCH_STATE
        .write()
        .map_err(|error| error.to_string())?;
    if state.status.is_scan_in_progress && !state.status.is_committing {
        state.cancel_flag.store(true, Ordering::SeqCst);
        state.status.scan_phase = GlobalSearchScanPhase::Canceling;
    }
    Ok(())
}

fn scan_drive(
    root: &str,
    scan_depth: usize,
    ignored_matcher: &IgnoredPathMatcher,
    path_update_counter: &AtomicU64,
    fields: &GlobalSearchIndexFields,
    writer: &IndexWriter,
    indexed_count: &AtomicU64,
    cancel_flag: &AtomicBool,
) -> Result<(), GlobalSearchDriveScanError> {
    let root_path = PathBuf::from(root);
    let root_string = normalize_path(root);
    // FORK-MODIFICATION 2026-11-02：每个索引根的耗时与产出埋点。
    //
    // 现状 status.json 里只有「整轮扫描多久」这一个数（实测 22 分钟），
    // 看不出是哪个根拖的、扫出多少文档。逐根打点才能定位「索引规模过大」
    // 还是「某个根收集过深」。
    let drive_started = std::time::Instant::now();
    let mut drive_indexed_count: u64 = 0;
    // 枚举（走目录树）与写入（stat + 落文档）分开计时 ——
    // 全量扫描那 12 分钟到底花在哪，只有拆开才知道。
    let mut index_write_ms: u128 = 0;

    if let Err(error) = std::fs::read_dir(&root_path) {
        return Err(GlobalSearchDriveScanError {
            drive_root: root_string,
            message: error.to_string(),
        });
    }

    let mut items_since_last_update: u64 = 0;

    for entry_result in WalkDir::new(&root_path)
        .follow_links(false)
        .max_depth(scan_depth.max(1))
        .into_iter()
        .filter_entry(|entry| {
            should_scan_walk_entry(entry.path(), ignored_matcher, path_update_counter)
        })
    {
        if cancel_flag.load(Ordering::SeqCst) {
            break;
        }

        let entry = match entry_result {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let path = entry.path();
        let path_string = match path.to_str() {
            Some(path_str) => normalize_path(path_str),
            None => continue,
        };

        if ignored_matcher.is_ignored(&path_string) {
            continue;
        }

        if entry.depth() == 0 {
            continue;
        }

        let write_started = std::time::Instant::now();
        let did_add_doc = add_path_doc(writer, fields, path, &path_string);
        index_write_ms += write_started.elapsed().as_millis();

        if !did_add_doc {
            continue;
        }

        indexed_count.fetch_add(1, Ordering::Relaxed);
        drive_indexed_count += 1;
        items_since_last_update += 1;

        if items_since_last_update >= STATUS_UPDATE_INTERVAL {
            if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
                state.status.scan_indexed_item_count = indexed_count.load(Ordering::Relaxed);
            }
            items_since_last_update = 0;
        }
    }

    let drive_elapsed_ms = drive_started.elapsed().as_millis();
    let enumerate_ms = drive_elapsed_ms.saturating_sub(index_write_ms);
    log::info!(
        "[global-search] scan root timings: root={} elapsed={}ms indexed={} depth={} enumerate_ms={} index_write_ms={}",
        root_string,
        drive_elapsed_ms,
        drive_indexed_count,
        scan_depth,
        enumerate_ms,
        index_write_ms
    );

    Ok(())
}

fn should_skip_commit_after_failed_scan(
    valid_drive_count: usize,
    indexed_drive_roots: &[String],
    errors: &[GlobalSearchDriveScanError],
) -> bool {
    valid_drive_count > 0 && indexed_drive_roots.is_empty() && errors.len() == valid_drive_count
}

pub async fn global_search_start_scan(
    app: tauri::AppHandle,
    settings: GlobalSearchSettings,
) -> Result<(), String> {
    let scan_started_time = now_millis();
    let scan_reason = settings.scan_reason.clone();

    {
        let mut state = GLOBAL_SEARCH_STATE
            .write()
            .map_err(|error| error.to_string())?;
        if state.status.is_scan_in_progress {
            return Ok(());
        }
        state.status.is_scan_in_progress = true;
        state.status.is_parallel_scan = settings.parallel_scan && settings.drive_roots.len() > 1;
        state.status.scan_phase = GlobalSearchScanPhase::Scanning;
        state.status.scan_reason = Some(scan_reason.clone());
        state.status.current_drive_root = None;
        state.status.current_scan_path = None;
        state.status.scan_indexed_item_count = 0;
        state.status.drive_scan_errors = vec![];
        state.status.scanned_drives_count = 0;
        state.status.total_drives_count = settings.drive_roots.len() as u32;
        state.status.last_scan_error = None;
        state.cancel_flag.store(false, Ordering::SeqCst);
    }

    let base_dir =
        crate::app_dirs::app_data_dir(&app).map_err(|error: tauri::Error| error.to_string())?;
    let index_path = index_dir(&base_dir);
    let staging_path = staging_index_dir(&base_dir);

    let cancel_flag = {
        let state = GLOBAL_SEARCH_STATE
            .read()
            .map_err(|error| error.to_string())?;
        state.cancel_flag.clone()
    };

    // 只保留真实存在的盘根 —— watcher 与路由判定都基于它。
    let valid_drive_roots: Vec<String> = settings
        .drive_roots
        .iter()
        .filter(|root| {
            let path = Path::new(root.as_str());
            path.exists() && path.is_dir()
        })
        .cloned()
        .collect();

    if valid_drive_roots.is_empty() {
        if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
            state.status.is_scan_in_progress = false;
            state.status.total_drives_count = 0;
            state.status.current_drive_root = None;
        }
        return Err("No valid drives found to scan".to_string());
    }

    if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
        state.status.total_drives_count = valid_drive_roots.len() as u32;
    }

    let app_owned_prefixes = {
        let state = GLOBAL_SEARCH_STATE
            .read()
            .map_err(|error| error.to_string())?;
        if state.app_owned_prefixes.is_empty() {
            vec![normalize_path(&base_dir.to_string_lossy())]
        } else {
            state.app_owned_prefixes.clone()
        }
    };

    // watcher 必须**先**建好 —— 全量扫描期间发生的变化才接得住。
    // 重开 watcher 会让代次 +1，路由判定靠代次比对发现这个缺口。
    let watcher_generation =
        index_watcher::ensure_watcher(&valid_drive_roots, &app_owned_prefixes).unwrap_or(0);
    let watcher_snapshot = index_watcher::watcher_snapshot();

    match decide_scan_route(&base_dir, &valid_drive_roots, &watcher_snapshot) {
        IncrementalPlan::Incremental => {
            let task_base_dir = base_dir.clone();
            let task_index_path = index_path.clone();
            let task_settings = settings.clone();
            let task_reason = scan_reason.clone();

            tauri::async_runtime::spawn_blocking(move || {
                run_incremental_pass(
                    &task_base_dir,
                    &task_index_path,
                    &task_settings,
                    scan_started_time,
                    task_reason,
                    watcher_generation,
                    &app_owned_prefixes,
                );
            });

            return Ok(());
        }
        IncrementalPlan::FullRescan(blocker) => {
            log::info!(
                "[global-search] falling back to a full rescan, blocker={:?}",
                blocker
            );
        }
    }

    tauri::async_runtime::spawn_blocking(move || {
        let cleanup_staging_path = staging_path.clone();
        let result =
            (|| -> Result<(u64, Vec<String>, Index, IndexReader, GlobalSearchIndexFields, u64), String> {
                let (index, fields) = create_fresh_index(&staging_path)?;

                let mut writer = create_bulk_index_writer(&index)?;

                let ignored_paths: Vec<String> = settings
                    .ignored_paths
                    .iter()
                    .map(|path| path.to_string())
                    .chain(builtin_ignored_paths().iter().map(|path| path.to_string()))
                    .chain(app_owned_prefixes.iter().map(|path| path.to_string()))
                    .collect();
                let ignored_matcher = IgnoredPathMatcher::new(&ignored_paths);

                if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
                    state.status.total_drives_count = valid_drive_roots.len() as u32;
                }

                let indexed_count = AtomicU64::new(0);
                let path_update_counter = AtomicU64::new(0);
                let mut errors: Vec<GlobalSearchDriveScanError> = Vec::new();
                let mut scanned_count: u32 = 0;
                let mut indexed_drive_roots: Vec<String> = Vec::new();

                if settings.parallel_scan && valid_drive_roots.len() > 1 {
                    use std::thread;

                    thread::scope(|scope| {
                        let results: Vec<_> = valid_drive_roots
                            .iter()
                            .map(|root| {
                                let root = root.clone();
                                let ignored_matcher_ref = &ignored_matcher;
                                let writer_ref: &IndexWriter = &writer;
                                let indexed_count_ref = &indexed_count;
                                let path_update_counter_ref = &path_update_counter;
                                let cancel_flag_ref = &cancel_flag;
                                let scan_depth = settings.scan_depth;

                                scope.spawn(move || {
                                    if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
                                        state.status.current_drive_root =
                                            Some(normalize_path(&root));
                                    }

                                    let result = scan_drive(
                                        &root,
                                        scan_depth,
                                        ignored_matcher_ref,
                                        path_update_counter_ref,
                                        &fields,
                                        writer_ref,
                                        indexed_count_ref,
                                        cancel_flag_ref,
                                    );

                                    if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
                                        state.status.scanned_drives_count += 1;
                                        state.status.scan_indexed_item_count =
                                            indexed_count_ref.load(Ordering::Relaxed);
                                    }

                                    (root, result)
                                })
                            })
                            .collect();

                        for handle in results {
                            match handle.join() {
                                Ok((root, Ok(()))) => {
                                    indexed_drive_roots.push(normalize_path(&root));
                                }
                                Ok((_root, Err(error))) => {
                                    errors.push(error);
                                }
                                Err(_) => {}
                            }
                        }
                    });
                } else {
                    for root in valid_drive_roots.iter() {
                        if cancel_flag.load(Ordering::SeqCst) {
                            break;
                        }

                        let root_string = normalize_path(root);

                        if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
                            state.status.current_drive_root = Some(root_string.clone());
                        }

                        let result = scan_drive(
                            root,
                            settings.scan_depth,
                            &ignored_matcher,
                            &path_update_counter,
                            &fields,
                            &writer,
                            &indexed_count,
                            &cancel_flag,
                        );

                        scanned_count += 1;
                        if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
                            state.status.scanned_drives_count = scanned_count;
                            state.status.scan_indexed_item_count =
                                indexed_count.load(Ordering::Relaxed);
                        }

                        match result {
                            Ok(()) => {
                                indexed_drive_roots.push(root_string);
                            }
                            Err(error) => {
                                errors.push(error);
                            }
                        }
                    }
                }

                let should_skip_commit = should_skip_commit_after_failed_scan(
                    valid_drive_roots.len(),
                    &indexed_drive_roots,
                    &errors,
                );

                {
                    let mut state = GLOBAL_SEARCH_STATE
                        .write()
                        .map_err(|error| error.to_string())?;

                    if cancel_flag.load(Ordering::SeqCst) {
                        return Err("Scan cancelled".to_string());
                    }

                    state.status.drive_scan_errors = errors;
                    state.status.current_drive_root = None;

                    if should_skip_commit {
                        return Err("All selected drives failed to scan".to_string());
                    }

                    state.status.is_committing = true;
                    state.status.scan_phase = GlobalSearchScanPhase::Committing;
                }

                writer.commit().map_err(|error| error.to_string())?;

                // 提交后显式合并，把 segment 数量压回上限。
                // `NoMergePolicy` 只关掉后台自动合并，显式 `merge()` 不受它影响。
                // 合并失败**不**让这次扫描失败：索引已经提交成功且可用，
                // 只是 segment 会多一些（下次扫描还会再合并）。
                if let Err(error) = merge_index_segments(&mut writer) {
                    log::warn!(
                        "[global-search] Failed to merge index segments after commit: {}",
                        error
                    );
                }
                writer.wait_merging_threads().map_err(|error| error.to_string())?;

                drop(index);

                let final_count = indexed_count.load(Ordering::Relaxed);
                validate_staged_index(&staging_path, final_count)?;

                // FORK-MODIFICATION 2026-11-08：提交段的磁盘 IO 必须在状态锁**之外**。
                //
                // 根因（用户 2026-11-08 反馈「点左侧导航切换时出现无响应」）：
                // 这一段要做「替换索引目录 + 打开新索引 + 把扫描期间攒下的变化补写进
                // 索引 + 统计索引目录大小」，全是磁盘 IO；索引被上百万文件撑大、
                // 磁盘又正被扫描打满时，光替换目录与补写就能跑上好几秒。
                // 之前它整段在 `GLOBAL_SEARCH_STATE` 的**写锁**里，而 Tauri v2 的
                // **同步 command 跑在主线程** —— `global_search_get_status`
                // （前端在扫描期间一直在轮询）要抢同一把读锁，主线程被写锁挡住
                // ⇒ 窗口消息泵停摆 ⇒ Windows 判「未响应」。
                //
                // 为什么移出去是安全的：提交窗口由 `status.is_committing` 标记，
                // 查询侧（`query.rs`）在 `is_committing` 期间本来就直接返回空结果，
                // 读不到「旧 reader 已清空、新 reader 还没装上」的中间态。
                {
                    let mut state = GLOBAL_SEARCH_STATE
                        .write()
                        .map_err(|error| error.to_string())?;
                    state.index = None;
                    state.reader = None;
                    state.fields = None;
                }

                replace_index_dir(&staging_path, &index_path)?;
                let (live_index, live_reader, live_fields) = open_or_create_index(&index_path)?;

                // watcher 在全量扫描**之前**就建好了，扫描期间发生的变化都
                // 攒在事件缓冲里 —— 全量落地之后必须把它们补进去，否则
                // 「扫描时才被创建的文件」会永久缺失。
                let flush_stats = flush_pending_changes(
                    &live_index,
                    &live_reader,
                    &live_fields,
                    &ignored_matcher,
                    settings.scan_depth,
                )?;

                if flush_stats.applied_operations > 0 {
                    log::info!(
                        "[global-search] flushed pending changes after full scan: operations={} skipped={} upserted={}",
                        flush_stats.applied_operations,
                        flush_stats.skipped_operations,
                        flush_stats.upserted_documents
                    );
                }

                let index_size = calculate_dir_size(&index_path);
                let live_doc_count = live_reader.searcher().num_docs();

                Ok((
                    live_doc_count,
                    indexed_drive_roots,
                    live_index,
                    live_reader,
                    live_fields,
                    index_size,
                ))
            })();

        if result.is_err() && cleanup_staging_path.exists() {
            let _ = remove_dir_force(&cleanup_staging_path);
        }

        let result = result.map(
            |(doc_count, indexed_drive_roots, index, reader, fields, index_size)| {
                CommittedIndexUpdate {
                    doc_count,
                    index_size_bytes: index_size,
                    index,
                    reader,
                    fields,
                    indexed_drive_roots: Some(indexed_drive_roots),
                }
            },
        );

        // 全量与增量走**同一个**收尾函数。
        // 之前这里是一份复制粘贴的副本，复制品漏掉了 `scan round timings` 埋点，
        // 于是整轮耗时永远打不出来，验收脚本会一直等不到标记行。
        finalize_scan_state(
            &base_dir,
            &index_path,
            scan_started_time,
            scan_reason,
            GlobalSearchScanMode::Full,
            watcher_generation,
            result,
            !cancel_flag.load(Ordering::SeqCst),
        );
    });

    Ok(())
}

pub async fn global_search_index_paths(
    app: tauri::AppHandle,
    settings: IndexPathsSettings,
) -> Result<u64, String> {
    if settings.paths.is_empty() {
        return Ok(0);
    }

    {
        let state = GLOBAL_SEARCH_STATE
            .read()
            .map_err(|error| error.to_string())?;
        if state.status.is_scan_in_progress {
            return Err("A full scan is already in progress".to_string());
        }
    }

    let base_dir =
        crate::app_dirs::app_data_dir(&app).map_err(|error: tauri::Error| error.to_string())?;

    tauri::async_runtime::spawn_blocking(move || {
        global_search_index_paths_blocking(base_dir, settings)
    })
    .await
    .map_err(|join_error| format!("Global search path indexing task failed: {join_error}"))?
}

fn global_search_index_paths_blocking(
    base_dir: PathBuf,
    settings: IndexPathsSettings,
) -> Result<u64, String> {
    let index_path = index_dir(&base_dir);

    let (index, reader, fields) = open_or_create_index(&index_path)?;

    let mut writer = create_bulk_index_writer(&index)?;

    let app_owned_prefixes = {
        let state = GLOBAL_SEARCH_STATE
            .read()
            .map_err(|error| error.to_string())?;
        if state.app_owned_prefixes.is_empty() {
            vec![normalize_path(&base_dir.to_string_lossy())]
        } else {
            state.app_owned_prefixes.clone()
        }
    };

    let ignored_paths: Vec<String> = settings
        .ignored_paths
        .iter()
        .map(|path| path.to_string())
        .chain(builtin_ignored_paths().iter().map(|path| path.to_string()))
        .chain(app_owned_prefixes.iter().map(|path| path.to_string()))
        .collect();
    let ignored_matcher = IgnoredPathMatcher::new(&ignored_paths);
    let path_update_counter = AtomicU64::new(0);

    let mut indexed_count: u64 = 0;
    let mut did_mutate_index = false;
    let scan_depth = settings.scan_depth.max(1);

    for dir_path in &settings.paths {
        let path = Path::new(dir_path);

        if !path.exists() || !path.is_dir() {
            continue;
        }

        let normalized_dir = normalize_path(dir_path);

        if ignored_matcher.is_ignored(&normalized_dir) {
            continue;
        }

        // 旧实现只删了「路径恰好等于 dir/ 的那一条」与「dir 自己」——
        // 子树一条都删不掉，目录里的旧文档会一直留在索引里。
        // 改成前缀删除：路径自己 + 它下面全部，一起走。
        delete_prefix(&writer, &fields, &normalized_dir)?;
        did_mutate_index = true;

        for entry_result in WalkDir::new(path)
            .follow_links(false)
            .max_depth(scan_depth)
            .into_iter()
            .filter_entry(|entry| {
                should_scan_walk_entry(entry.path(), &ignored_matcher, &path_update_counter)
            })
        {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            if entry.depth() == 0 {
                continue;
            }

            let entry_path = entry.path();
            let path_string = match entry_path.to_str() {
                Some(path_str) => normalize_path(path_str),
                None => continue,
            };

            if ignored_matcher.is_ignored(&path_string) {
                continue;
            }

            let did_add_doc = add_path_doc(&writer, &fields, entry_path, &path_string);

            if did_add_doc {
                indexed_count += 1;
            }
        }
    }

    if did_mutate_index {
        writer.commit().map_err(|error| error.to_string())?;

        // 增量扫描直接往常驻索引里提交，每提交一次就多一个 segment。
        // 提交后显式合并，把 segment 数量压回上限。
        // 合并失败不影响本次增量扫描的结果（索引已提交且可用）。
        if let Err(error) = merge_index_segments(&mut writer) {
            log::warn!(
                "[global-search] Failed to merge index segments after commit: {}",
                error
            );
        }
        writer
            .wait_merging_threads()
            .map_err(|error| error.to_string())?;

        reader.reload().map_err(|error| error.to_string())?;

        let new_total = reader.searcher().num_docs();
        let index_size = calculate_dir_size(&index_path);

        if let Ok(mut state) = GLOBAL_SEARCH_STATE.write() {
            apply_committed_index_status(
                &mut state,
                &base_dir,
                CommittedIndexUpdate {
                    doc_count: new_total,
                    index_size_bytes: index_size,
                    index,
                    reader,
                    fields,
                    indexed_drive_roots: None,
                },
                true,
            );
        }
    }

    Ok(indexed_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex as StdMutex};
    use tempfile::TempDir;

    static GLOBAL_SEARCH_TEST_LOCK: Lazy<StdMutex<()>> = Lazy::new(|| StdMutex::new(()));

    #[test]
    fn committed_index_status_clears_valid_flag_and_persists_meta_for_empty_index() {
        let temp = TempDir::new().unwrap();
        let base_dir = temp.path();
        let index_path = index_dir(base_dir);
        let (index, reader, fields) = open_or_create_index(&index_path).unwrap();

        let mut state = GlobalSearchState {
            status: GlobalSearchStatus {
                is_scan_in_progress: false,
                is_committing: false,
                is_parallel_scan: false,
                scan_phase: GlobalSearchScanPhase::Idle,
                scan_reason: None,
                last_scan_time: Some(1),
                last_scan_outcome: None,
                last_scan_reason: None,
                last_scan_started_time: None,
                last_scan_finished_time: None,
                last_scan_duration_ms: None,
                last_scan_indexed_item_count: None,
                last_scan_error: None,
                last_scan_mode: None,
                incremental_ready: false,
                incremental_watcher_generation: 0,
                last_incremental_failed: false,
                indexed_item_count: 999,
                scan_indexed_item_count: 0,
                indexed_drive_roots: vec!["C:/".to_string()],
                index_size_bytes: 0,
                current_drive_root: None,
                current_scan_path: None,
                drive_scan_errors: vec![],
                is_index_valid: true,
                scanned_drives_count: 0,
                total_drives_count: 0,
            },
            index: None,
            reader: None,
            fields: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            app_owned_prefixes: vec![],
        };

        apply_committed_index_status(
            &mut state,
            base_dir,
            CommittedIndexUpdate {
                doc_count: 0,
                index_size_bytes: 0,
                index,
                reader,
                fields,
                indexed_drive_roots: Some(vec![]),
            },
            true,
        );

        assert!(!state.status.is_index_valid);
        assert_eq!(state.status.indexed_item_count, 0);
        assert!(state.status.indexed_drive_roots.is_empty());
        assert!(state.status.last_scan_time.unwrap() > 1);
        let meta = read_meta(base_dir).expect("meta written");
        assert_eq!(meta.indexed_item_count, 0);
        assert_eq!(meta.indexed_item_count, state.status.indexed_item_count);
        assert!(meta.indexed_drive_roots.is_empty());
        assert_eq!(meta.last_scan_time, state.status.last_scan_time);
    }

    #[test]
    fn cancel_scan_sets_cancel_flag_and_canceling_phase() {
        let _lock_guard = GLOBAL_SEARCH_TEST_LOCK.lock().unwrap();

        {
            let mut state = GLOBAL_SEARCH_STATE.write().unwrap();
            state.status.is_scan_in_progress = true;
            state.status.is_committing = false;
            state.status.scan_phase = GlobalSearchScanPhase::Scanning;
            state.cancel_flag.store(false, Ordering::SeqCst);
        }

        global_search_cancel_scan().unwrap();

        let state = GLOBAL_SEARCH_STATE.read().unwrap();
        assert!(state.cancel_flag.load(Ordering::SeqCst));
        assert_eq!(state.status.scan_phase, GlobalSearchScanPhase::Canceling);
        drop(state);

        let mut state = GLOBAL_SEARCH_STATE.write().unwrap();
        state.status.is_scan_in_progress = false;
        state.status.scan_phase = GlobalSearchScanPhase::Idle;
        state.cancel_flag.store(false, Ordering::SeqCst);
    }

    #[test]
    fn cancel_scan_does_not_cancel_committing_scan() {
        let _lock_guard = GLOBAL_SEARCH_TEST_LOCK.lock().unwrap();

        {
            let mut state = GLOBAL_SEARCH_STATE.write().unwrap();
            state.status.is_scan_in_progress = true;
            state.status.is_committing = true;
            state.status.scan_phase = GlobalSearchScanPhase::Committing;
            state.cancel_flag.store(false, Ordering::SeqCst);
        }

        global_search_cancel_scan().unwrap();

        let state = GLOBAL_SEARCH_STATE.read().unwrap();
        assert!(!state.cancel_flag.load(Ordering::SeqCst));
        assert_eq!(state.status.scan_phase, GlobalSearchScanPhase::Committing);
        drop(state);

        let mut state = GLOBAL_SEARCH_STATE.write().unwrap();
        state.status.is_scan_in_progress = false;
        state.status.is_committing = false;
        state.status.scan_phase = GlobalSearchScanPhase::Idle;
        state.cancel_flag.store(false, Ordering::SeqCst);
    }

    #[test]
    fn cancelled_outcome_wins_even_if_result_succeeded() {
        assert_eq!(
            scan_outcome_for_result(true, true),
            GlobalSearchScanOutcome::Canceled
        );
    }

    #[test]
    fn failed_full_scan_without_successful_drives_skips_commit() {
        let errors = vec![GlobalSearchDriveScanError {
            drive_root: "C:/".to_string(),
            message: "Access denied".to_string(),
        }];

        assert!(should_skip_commit_after_failed_scan(1, &[], &errors));
    }

    #[test]
    fn empty_successful_full_scan_can_commit() {
        let indexed_drive_roots = vec!["C:/".to_string()];

        assert!(!should_skip_commit_after_failed_scan(
            1,
            &indexed_drive_roots,
            &[]
        ));
    }

    fn write_test_doc(writer: &IndexWriter, fields: &GlobalSearchIndexFields, path: &str) {
        writer
            .add_document(doc!(
                fields.path => path.to_string(),
                fields.name => path.to_string(),
                fields.name_lower => path.to_lowercase(),
                fields.is_file => 1u64,
                fields.is_dir => 0u64,
                fields.modified_time => 0u64,
                fields.size => 0u64,
            ))
            .unwrap();
    }

    #[test]
    fn deleting_a_prefix_removes_the_whole_subtree() {
        let temp = TempDir::new().unwrap();
        let base_dir = temp.path();
        let index_path = index_dir(base_dir);
        let (index, reader, fields) = open_or_create_index(&index_path).unwrap();

        let mut writer = create_bulk_index_writer(&index).unwrap();
        write_test_doc(&writer, &fields, "C:/dir/a.txt");
        write_test_doc(&writer, &fields, "C:/dir/sub/b.txt");
        write_test_doc(&writer, &fields, "C:/directory/c.txt");
        writer.commit().unwrap();
        reader.reload().unwrap();

        assert_eq!(reader.searcher().num_docs(), 3);

        delete_prefix(&writer, &fields, "C:/dir").unwrap();
        writer.commit().unwrap();
        reader.reload().unwrap();

        assert_eq!(reader.searcher().num_docs(), 1);
        assert!(path_is_indexed(&reader, &fields, "C:/directory/c.txt"));
        assert!(!path_is_indexed(&reader, &fields, "C:/dir/a.txt"));
        assert!(!path_is_indexed(&reader, &fields, "C:/dir/sub/b.txt"));
    }

    #[test]
    fn applying_index_ops_upserts_a_new_file_and_drops_a_deleted_one() {
        let temp = TempDir::new().unwrap();
        let base_dir = temp.path();
        let index_path = index_dir(base_dir);
        let (index, reader, fields) = open_or_create_index(&index_path).unwrap();

        let mut writer = create_bulk_index_writer(&index).unwrap();

        let existing = temp.path().join("existing.txt");
        std::fs::write(&existing, b"x").unwrap();
        let fresh = temp.path().join("fresh.txt");
        std::fs::write(&fresh, b"y").unwrap();

        let existing_path = normalize_path(&existing.to_string_lossy());
        let fresh_path = normalize_path(&fresh.to_string_lossy());

        write_test_doc(&writer, &fields, &existing_path);
        writer.commit().unwrap();
        reader.reload().unwrap();

        // 这里刻意用空的忽略表：临时目录在 %TEMP% 下，
        // 用内置忽略规则会把整棵子树剪掉，测的就不是删除逻辑了。
        let matcher = IgnoredPathMatcher::new(&[]);

        let ops = vec![
            IndexOp::UpsertFile {
                path: fresh_path.clone(),
            },
            IndexOp::DeletePrefix {
                prefix: existing_path.clone(),
            },
        ];

        let stats = apply_index_ops(&writer, &reader, &fields, &ops, &matcher, 3).unwrap();
        writer.commit().unwrap();
        reader.reload().unwrap();

        assert_eq!(stats.applied_operations, 2);
        assert_eq!(stats.upserted_documents, 1);
        assert_eq!(reader.searcher().num_docs(), 1);
        assert!(path_is_indexed(&reader, &fields, &fresh_path));
        assert!(!path_is_indexed(&reader, &fields, &existing_path));
    }

    #[test]
    fn a_tree_rescan_covers_the_single_file_operations_below_it() {
        let temp = TempDir::new().unwrap();
        let base_dir = temp.path();
        let index_path = index_dir(base_dir);
        let (index, reader, fields) = open_or_create_index(&index_path).unwrap();

        let mut writer = create_bulk_index_writer(&index).unwrap();

        let dir = temp.path().join("tree");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, b"x").unwrap();

        let dir_path = normalize_path(&dir.to_string_lossy());
        let file_path = normalize_path(&file.to_string_lossy());

        let matcher = IgnoredPathMatcher::new(&[]);

        let ops = vec![
            IndexOp::UpsertTree {
                prefix: dir_path.clone(),
            },
            IndexOp::UpsertFile {
                path: file_path.clone(),
            },
        ];

        let stats = apply_index_ops(&writer, &reader, &fields, &ops, &matcher, 3).unwrap();
        writer.commit().unwrap();
        reader.reload().unwrap();

        assert_eq!(stats.applied_operations, 1);
        assert_eq!(stats.skipped_operations, 1);
        assert!(path_is_indexed(&reader, &fields, &file_path));
    }
}
