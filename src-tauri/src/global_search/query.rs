// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use crate::utils::{
    is_hidden_path, metadata_times_unix_ms, normalize_path, path_extension_lowercase,
};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, BooleanQuery, FuzzyTermQuery, Occur, Query, TermQuery};
use tantivy::schema::{IndexRecordOption, Value};
use tantivy::Term;
use walkdir::WalkDir;

use super::ignore::{builtin_ignored_paths, get_drive_root, normalize_case, IgnoredPathMatcher};
use super::index::{index_dir, open_or_create_index};
use super::scoring::{calculate_similarity_score, get_min_score_for_query_length};
use super::state::{
    is_reader_idle, mark_query_activity, now_millis, GlobalSearchIndexFields, GLOBAL_SEARCH_STATE,
};
use super::types::{GlobalSearchQueryOptions, GlobalSearchResultEntry};

pub(super) fn build_query(
    fields: &GlobalSearchIndexFields,
    query: &str,
    options: &GlobalSearchQueryOptions,
) -> Box<dyn Query> {
    let normalized = normalize_case(query);

    if normalized.is_empty() {
        return Box::new(AllQuery);
    }

    if options.exact_match {
        return build_exact_match_query(fields, &normalized);
    }

    let words = split_query_tokens(&normalized);

    let max_distance = if options.typo_tolerance { 2 } else { 1 };

    let mut subqueries: Vec<(tantivy::query::Occur, Box<dyn Query>)> = Vec::new();

    for word in &words {
        let term = Term::from_field_text(fields.name, word);
        let fuzzy = FuzzyTermQuery::new(term, max_distance, true);
        subqueries.push((Occur::Should, Box::new(fuzzy)));
    }

    let whitespace_words: Vec<&str> = normalized.split_whitespace().collect();
    for word in whitespace_words {
        if word.contains('.') || word.contains('_') || word.contains('-') {
            let term = Term::from_field_text(fields.name, word);
            let fuzzy = FuzzyTermQuery::new(term, max_distance, true);
            subqueries.push((Occur::Should, Box::new(fuzzy)));
        }
    }

    Box::new(BooleanQuery::from(subqueries))
}

fn build_exact_match_query(
    fields: &GlobalSearchIndexFields,
    normalized_query: &str,
) -> Box<dyn Query> {
    let words = split_query_tokens(normalized_query);

    if words.is_empty() {
        return Box::new(AllQuery);
    }

    if words.len() == 1 {
        let term = Term::from_field_text(fields.name, &words[0]);
        return Box::new(TermQuery::new(term, IndexRecordOption::Basic));
    }

    let subqueries: Vec<(Occur, Box<dyn Query>)> = words
        .iter()
        .map(|word| {
            let term = Term::from_field_text(fields.name, word);
            (
                Occur::Must,
                Box::new(TermQuery::new(term, IndexRecordOption::Basic)) as Box<dyn Query>,
            )
        })
        .collect();

    Box::new(BooleanQuery::from(subqueries))
}

fn split_query_tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| {
            character.is_whitespace() || character == '.' || character == '_' || character == '-'
        })
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
        .collect()
}

fn matches_exact_name(query: &str, name: &str) -> bool {
    let query_tokens = split_query_tokens(&normalize_case(query));
    if query_tokens.is_empty() {
        return true;
    }

    let name_tokens = split_query_tokens(&normalize_case(name));
    query_tokens.iter().all(|query_token| {
        name_tokens
            .iter()
            .any(|name_token| name_token == query_token)
    })
}

pub(super) fn matches_type(
    doc_is_file: u64,
    doc_is_dir: u64,
    options: &GlobalSearchQueryOptions,
) -> bool {
    let is_file = doc_is_file == 1;
    let is_dir = doc_is_dir == 1;

    (options.include_files && is_file) || (options.include_directories && is_dir)
}

pub async fn global_search_query(
    app: tauri::AppHandle,
    query: String,
    options: GlobalSearchQueryOptions,
) -> Result<Vec<GlobalSearchResultEntry>, String> {
    let base_dir =
        crate::app_dirs::app_data_dir(&app).map_err(|error: tauri::Error| error.to_string())?;

    tauri::async_runtime::spawn_blocking(move || {
        global_search_query_blocking(base_dir, query, options)
    })
    .await
    .map_err(|join_error| format!("Global search query task failed: {join_error}"))?
}

pub(super) fn global_search_query_blocking(
    base_dir: PathBuf,
    query: String,
    options: GlobalSearchQueryOptions,
) -> Result<Vec<GlobalSearchResultEntry>, String> {
    let index_path = index_dir(&base_dir);

    // 空闲释放：常驻 reader 闲置超过阈值就先丢掉，紧接着的 needs_open 分支
    // 会重新打开它（索引与字段仍留在 state 里，状态保持自洽）。
    release_idle_reader_if_needed()?;

    let needs_open = {
        let state = GLOBAL_SEARCH_STATE
            .read()
            .map_err(|error| error.to_string())?;
        if state.status.is_committing {
            return Ok(Vec::new());
        }
        state.index.is_none() || state.reader.is_none() || state.fields.is_none()
    };

    if needs_open {
        let (index, reader, fields) = open_or_create_index(&index_path)?;
        let mut state = GLOBAL_SEARCH_STATE
            .write()
            .map_err(|error| error.to_string())?;

        if state.status.is_committing {
            return Ok(Vec::new());
        }

        if state.index.is_none() || state.reader.is_none() || state.fields.is_none() {
            state.index = Some(index);
            state.reader = Some(reader);
            state.fields = Some(fields);
        }
    }

    let state = GLOBAL_SEARCH_STATE
        .read()
        .map_err(|error| error.to_string())?;

    if state.status.is_committing {
        return Ok(Vec::new());
    }

    let reader = state
        .reader
        .as_ref()
        .ok_or_else(|| "Search index reader is not initialized".to_string())?;
    let fields = *state
        .fields
        .as_ref()
        .ok_or_else(|| "Search index fields are not initialized".to_string())?;

    // 本次查询算「活跃」：空闲释放的判断以「上一次查询时间」为准。
    mark_query_activity();

    let searcher = reader.searcher();
    let normalized_query = normalize_case(&query);

    // FORK-MODIFICATION: 2026-10-28 —— 关键顺序：**先按 limit 截断候选，再物化**。
    //
    // 旧实现是 `TopDocs::with_limit(100_000)` → 先把 10 万条 tantivy 文档全部
    // `searcher.doc()` 物化出来 → 最后才 `truncate(options.limit)`，于是单次查询的
    // 峰值内存被「本来用不上的那几万条文档」顶起来（30–80 MB 量级）。
    //
    // 现在：候选条数直接用 `options.limit`（取值依据见 `candidate_limit_for`），
    // 物化条数因此 ≤ limit。
    // 候选预算按「每个盘符各 limit」计算：保留旧的多盘符语义
    // （旧实现是在盘符分组内 truncate(limit)，总条数可达 limit x 盘符数），
    // 同时把物化量从 10 万级压到「limit x 盘符数」级。
    let drive_count = state.status.indexed_drive_roots.len();
    let candidate_limit = candidate_limit_for(&options, drive_count);
    let candidates =
        collect_candidate_addresses(&searcher, &fields, &query, &options, candidate_limit)?;
    let (results, _materialized_count) =
        materialize_candidates(&searcher, &fields, &options, &normalized_query, &candidates);

    Ok(finalize_results(results, options.limit))
}

/// 单次查询允许物化的文档条数上限。
///
/// 取值依据：`options.limit` **就是**「结果条数上限」旋钮本身
/// （前端 `globalSearch.resultLimit`，UI 夹取到 [10, 500]、默认 50；
/// MCP 侧 `clamp_result_limit` 夹取到 [1, 500]），所以候选条数直接用它，
/// 不再另写常量 —— 旧代码写死的 `100_000` 与实际需要没有任何关系。
///
/// **按盘符数放大**：旧实现是在「盘符分组内」`truncate(options.limit)`，
/// 多盘符时总条数可达 `limit × 盘符数`。若候选只给 limit，靠后的盘符会被挤成
/// 个位数（用户可见的结果变少），因此候选预算 = `limit × 盘符数`。
///
/// `TopDocs::with_limit(0)` 会 panic，这里兜底成 1。
fn candidate_limit_for(options: &GlobalSearchQueryOptions, drive_count: usize) -> usize {
    let per_drive = options.limit.max(1);
    let drives = drive_count.max(1);

    per_drive
        .saturating_mul(drives)
        .min(MAX_SEARCH_CANDIDATE_MATERIALIZATION)
}

/// 单次查询允许物化的文档条数绝对上限。
///
/// 兜底，防止「盘符数 × limit」在极端配置下失控
/// （limit ≤ 500、已索引盘符数通常为个位数）。
const MAX_SEARCH_CANDIDATE_MATERIALIZATION: usize = 5_000;

/// 取候选文档地址（只取地址，不物化文档）。
///
/// 候选条数按 `limit` 封顶 —— 这是「先截断、后物化」的关键：物化条数被限制在
/// `limit` 以内，而不是先把一堆文档物化出来再 truncate。
fn collect_candidate_addresses(
    searcher: &tantivy::Searcher,
    fields: &GlobalSearchIndexFields,
    query: &str,
    options: &GlobalSearchQueryOptions,
    limit: usize,
) -> Result<Vec<tantivy::DocAddress>, String> {
    let query_boxed = build_query(fields, query, options);
    let top_docs = searcher
        .search(&query_boxed, &TopDocs::with_limit(limit).order_by_score())
        .map_err(|error| error.to_string())?;

    // 收集器本身已经按 limit 封顶；这里再 take 一次，是为了让「物化条数 ≤ limit」
    // 这条不变量写死在代码里（将来换收集器也不会漏）。
    Ok(top_docs
        .into_iter()
        .map(|(_tantivy_score, doc_address)| doc_address)
        .take(limit)
        .collect())
}

/// 把候选文档物化成结果条目。
///
/// 返回 `(结果条目, 实际物化条数)`；物化条数 = 调用 `searcher.doc()` 的次数，
/// 恒 ≤ 候选条数 ≤ limit。
///
/// 过滤规则（忽略路径 / 精确匹配 / 相似度门槛 / 文件-目录类型）与改动前完全一致，
/// 只是**物化时机**从「先全量物化再截断」挪到了「先截断再物化」。
fn materialize_candidates(
    searcher: &tantivy::Searcher,
    fields: &GlobalSearchIndexFields,
    options: &GlobalSearchQueryOptions,
    normalized_query: &str,
    candidates: &[tantivy::DocAddress],
) -> (Vec<GlobalSearchResultEntry>, usize) {
    let internal_ignored: Vec<String> = builtin_ignored_paths()
        .iter()
        .map(|path| path.to_string())
        .collect();
    let internal_ignored_matcher = IgnoredPathMatcher::new(&internal_ignored);

    let min_score = options
        .min_score_threshold
        .unwrap_or_else(|| get_min_score_for_query_length(normalized_query.len()));

    let mut results: Vec<GlobalSearchResultEntry> = Vec::with_capacity(candidates.len());
    let mut materialized_count = 0usize;

    // 候选最多几百条（limit ≤ 500），顺序遍历即可：既省掉 rayon 的调度开销，
    // 又让结果顺序可复现（旧的 par_iter 顺序是不确定的）。
    for doc_address in candidates {
        materialized_count += 1;

        let retrieved: tantivy::TantivyDocument = match searcher.doc(*doc_address) {
            Ok(document) => document,
            Err(_) => continue,
        };

        let path_value = match retrieved.get_first(fields.path).and_then(|value| value.as_str()) {
            Some(value) => value.to_string(),
            None => continue,
        };

        if internal_ignored_matcher.is_ignored(&path_value) {
            continue;
        }

        let name_value = match retrieved.get_first(fields.name).and_then(|value| value.as_str()) {
            Some(value) => value.to_string(),
            None => continue,
        };

        if options.exact_match && !matches_exact_name(normalized_query, &name_value) {
            continue;
        }

        let name_score = calculate_similarity_score(normalized_query, &name_value);

        if name_score < min_score {
            continue;
        }

        let doc_is_file = retrieved
            .get_first(fields.is_file)
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let doc_is_dir = retrieved
            .get_first(fields.is_dir)
            .and_then(|value| value.as_u64())
            .unwrap_or(0);

        if !matches_type(doc_is_file, doc_is_dir, options) {
            continue;
        }

        let modified_time = retrieved
            .get_first(fields.modified_time)
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let size = retrieved
            .get_first(fields.size)
            .and_then(|value| value.as_u64())
            .unwrap_or(0);

        let ext = path_extension_lowercase(Path::new(&path_value));

        results.push(GlobalSearchResultEntry {
            name: name_value,
            ext,
            path: path_value,
            size,
            item_count: None,
            modified_time,
            accessed_time: 0,
            created_time: 0,
            mime: None,
            is_file: doc_is_file == 1,
            is_dir: doc_is_dir == 1,
            is_symlink: false,
            is_hidden: false,
            score: name_score,
        });
    }

    (results, materialized_count)
}

/// 按盘符分组 → 组内按相似度降序 → 每组截断到 limit → 合并后整体按相似度降序。
///
/// 分组 / 排序 / 截断的口径与改动前一致。
fn finalize_results(
    results: Vec<GlobalSearchResultEntry>,
    limit: usize,
) -> Vec<GlobalSearchResultEntry> {
    let mut drive_groups: std::collections::HashMap<String, Vec<GlobalSearchResultEntry>> =
        std::collections::HashMap::new();

    for entry in results {
        let drive_root = get_drive_root(&entry.path);
        drive_groups.entry(drive_root).or_default().push(entry);
    }

    let mut final_results: Vec<GlobalSearchResultEntry> = Vec::new();
    for (_drive, mut entries) in drive_groups {
        entries.par_sort_by(|entry_a, entry_b| {
            entry_b
                .score
                .partial_cmp(&entry_a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries.truncate(limit);
        final_results.extend(entries);
    }

    final_results.par_sort_by(|entry_a, entry_b| {
        entry_b
            .score
            .partial_cmp(&entry_a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    final_results
}

/// 空闲释放：常驻 reader 闲置超过阈值就丢掉，让下一次查询走 `needs_open` 重开。
///
/// 只在「确实有常驻 reader」且「没有正在扫描 / 提交索引」时执行；
/// 释放本身只是把 `state.reader` 置空，索引与字段仍留在 state 里，
/// 因此紧接着的 `needs_open` 分支会重新打开一套 (index, reader, fields) 覆盖回去，
/// 状态保持自洽。
fn release_idle_reader_if_needed() -> Result<(), String> {
    if !is_reader_idle(now_millis()) {
        return Ok(());
    }

    {
        let state = GLOBAL_SEARCH_STATE
            .read()
            .map_err(|error| error.to_string())?;

        if state.status.is_committing
            || state.status.is_scan_in_progress
            || state.reader.is_none()
        {
            return Ok(());
        }
    }

    let mut state = GLOBAL_SEARCH_STATE
        .write()
        .map_err(|error| error.to_string())?;

    // 拿到写锁后再确认一次：期间可能已经开始扫描了。
    if state.status.is_committing
        || state.status.is_scan_in_progress
        || state.reader.is_none()
    {
        return Ok(());
    }

    state.reader = None;

    Ok(())
}

pub async fn global_search_query_paths(
    paths: Vec<String>,
    query: String,
    options: GlobalSearchQueryOptions,
) -> Result<Vec<GlobalSearchResultEntry>, String> {
    if paths.is_empty() || query.trim().is_empty() {
        return Ok(Vec::new());
    }

    tauri::async_runtime::spawn_blocking(move || {
        global_search_query_paths_blocking(paths, query, options)
    })
    .await
    .map_err(|join_error| format!("Global search path query task failed: {join_error}"))?
}

// FORK-MODIFICATION: 需求反馈 3（2026-10-15）——目录内搜索递归化。
//
// 与 global_search_query_paths 的区别有两个，都不是「顺手加上就行」：
//
//  1. 前者对每个入参路径只 `read_dir` **一层**（这正是目录内搜索搜不到子目录的
//     根因），这里用 WalkDir 递归，并尊重 max_depth。
//  2. 过滤谓词不复用 scan.rs 的 should_scan_walk_entry：那个函数顺带调用
//     maybe_set_current_scan_path，会去改**全局索引的当前扫描路径**。目录内搜索
//     跟建索引毫无关系，借用它等于每敲一个字符就污染一次扫描状态。
//
// 只做**文件名子串**粗筛：目录内搜索支持 size: / items: / modified: 这类依赖
// 前端 store（目录大小、条目数、格式化日期）的谓词，把那些复制一份到 Rust
// 只会得到两份会漂移的语义。前端拿到结果后仍会用既有 matcher 精筛一遍。
// 代价是 `size:>1MB` 这种不含文件名关键词的查询只覆盖当前层。
fn should_include_recursive_entry(
    path: &Path,
    root_path: &Path,
    ignored_matcher: &IgnoredPathMatcher,
) -> bool {
    if path == root_path {
        return true;
    }

    !ignored_matcher.is_ignored(&normalize_path(&path.to_string_lossy()))
}

// FORK-MODIFICATION: 2026-10-16 —— 目录内搜索的容错粗筛阈值。
//
// 刻意**不复用** `get_min_score_for_query_length`：那个函数是为「模糊搜索排序」
// 设计的门槛，1-3 字符给到 0.9，恰恰会把「095 → 20251009_0095_27.jpg」
// 这类明显的容错命中（0.82）挡掉。粗筛阶段需要的是「宁可多收、交给前端精筛」，
// 所以这里取一个明显更松的值。
const TYPO_TOLERANCE_COEFFICIENT_THRESHOLD: f32 = 0.7;

pub(super) fn search_directory_recursive_blocking(
    root_path: String,
    query: String,
    max_depth: u32,
    limit: u32,
    typo_tolerance: bool,
) -> Result<Vec<GlobalSearchResultEntry>, String> {
    let root = PathBuf::from(&root_path);

    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let normalized_query = normalize_case(&query);
    let ignored_paths: Vec<String> = builtin_ignored_paths()
        .iter()
        .map(|path| path.to_string())
        .collect();
    let ignored_matcher = IgnoredPathMatcher::new(&ignored_paths);
    let effective_limit = limit.max(1) as usize;
    // 命中数远超 limit 时提前收工：磁盘遍历没有天然上界，一个十万文件的目录树
    // 走完再排序既慢又没必要。
    //
    // FORK-MODIFICATION: 2026-10-16 —— 这个「上限」原来是 effective_limit * 20，
    // 而下面计数的 `seen` 数的是**扫描过的条目**，不是命中数。两者一叠加的效果是：
    // 在一个 43,167 个条目的照片目录里，遍历到第 1000 个条目就收工，
    // 于是「递归搜索」在大目录里表现得**和没实现一模一样**（用户 2026-10-16 实测反馈）。
    //
    // 现在把两个概念拆开：
    //   collection_cap 数**扫描量**，是防超大目录卡死的安全阀，给足余量；
    //   match_cap 数**命中量**，是「结果已经足够挑了」的提前收工条件。
    // 这样小目录与中目录会一路扫完，大目录也不会被一条无关的文件名挡在门外。
    let collection_cap = effective_limit.saturating_mul(200).max(100_000);
    let match_cap = effective_limit.saturating_mul(4).max(200);
    let walk_depth = max_depth.max(1) as usize;

    let mut matched: Vec<(f32, GlobalSearchResultEntry)> = Vec::new();
    let mut seen = 0usize;

    let walk = WalkDir::new(&root)
        .follow_links(false)
        .min_depth(1)
        .max_depth(walk_depth)
        .into_iter()
        .filter_entry(|entry| should_include_recursive_entry(entry.path(), &root, &ignored_matcher));

    for entry_result in walk {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let path = entry.path();

        // filter_entry 已经保证不进忽略目录，这里再判一次只是为了把根目录
        // 自身挡在结果之外（min_depth(1) 已经做到了，这里是显式的双重保险）。
        if path == root {
            continue;
        }

        seen += 1;

        if seen > collection_cap {
            break;
        }

        let name = match path.file_name().and_then(|segment| segment.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // 粗筛：子串命中是零成本的快路径，一律收录。
        //
        // FORK-MODIFICATION: 2026-10-16 —— 容错（typoTolerance）接进来。
        // 之前只有子串，所以「拼写容错」这个开关在目录内搜索里**没有对应实现**，
        // 上一轮干脆没放这个复选框。现在有了：子串没命中时才去算相似度。
        //
        // 为什么不能每次都算分：`calculate_similarity_score` 内部是
        // Levenshtein 的 O(n·m) 双行 DP，43,167 个条目每个都算一遍会让
        // 每敲一个键都卡一下。先走 `contains` 把它挡在绝大多数之外。
        let is_substring_hit = normalized_query.is_empty()
            || normalize_case(&name).contains(&normalized_query);

        let is_typo_hit = !is_substring_hit && typo_tolerance && {
            let score = calculate_similarity_score(&normalized_query, &name);
            score >= TYPO_TOLERANCE_COEFFICIENT_THRESHOLD
        };

        if !is_substring_hit && !is_typo_hit {
            continue;
        }

        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        let is_file = metadata.is_file();
        let is_dir = metadata.is_dir();

        if !is_file && !is_dir {
            continue;
        }

        let name_score = if normalized_query.is_empty() {
            1.0_f32
        } else {
            // GlobalSearchResultEntry::score is f32, matching the scoring module.
            calculate_similarity_score(&normalized_query, &name)
        };

        // FORK-MODIFICATION: 2026-10-16 —— 这里**曾经**有一道
        // `if name_score < min_score { continue; }`，现已移除。
        //
        // 移除的理由是它与上面的粗筛自相矛盾。粗筛已经用 `contains` 确认过
        // 「查询是文件名的子串」，那是用户明确敲出来的字面量；而
        // `calculate_similarity_score` 对「名字比查询长」的子串命中返回的是
        // `0.8 + (query_len / name_len) * 0.15`。于是：
        //
        //   查询 "095"（3 字符，门槛 0.9）
        //   命中 "20251009_0095_27.jpg"
        //   得分 0.8 + (3/21)*0.15 = 0.821  <  0.9  →  被丢弃
        //
        // 三位数字的查询几乎必然落进这个区间（名字越长得分越低），
        // 于是用户搜 095 永远一条都搜不到 —— 表现为「递归搜索没实现」，
        // 而且**与目录大小无关**，小目录也一样搜不到。
        //
        // 现在分数只用于排序，不再当门槛：粗筛已经保证了结果集里每一条都是
        // 用户输入的真子串。真正的数量控制交给 effective_limit / match_cap。

        let (modified_time, accessed_time, created_time) = metadata_times_unix_ms(&metadata);
        let path_string = path.to_string_lossy().to_string();
        let normalized_path = normalize_path(&path_string);

        let result = GlobalSearchResultEntry {
            name,
            ext: path_extension_lowercase(path),
            path: normalized_path,
            size: if is_file { metadata.len() } else { 0 },
            item_count: None,
            modified_time,
            accessed_time,
            created_time,
            mime: None,
            is_file,
            is_dir,
            is_symlink: metadata.is_symlink(),
            is_hidden: is_hidden_path(path),
            score: name_score,
        };

        matched.push((name_score, result));

        // 命中数够挑了就收工。这是真正意义的「结果已经足够」，与上面的
        // collection_cap（安全阀）不是一回事。
        if matched.len() >= match_cap {
            break;
        }
    }

    matched.par_sort_by(|(score_a, entry_a), (score_b, entry_b)| {
        // 同分时按路径排，保证同样的输入给出同样的输出（列表顺序不能抖动）。
        score_b
            .partial_cmp(score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| entry_a.path.cmp(&entry_b.path))
    });

    let mut results: Vec<GlobalSearchResultEntry> = matched.into_iter().map(|(_, entry)| entry).collect();

    if results.len() > effective_limit {
        results.truncate(effective_limit);
    }

    Ok(results)
}

pub async fn search_directory_recursive(
    root_path: String,
    query: String,
    max_depth: u32,
    limit: u32,
    typo_tolerance: bool,
) -> Result<Vec<GlobalSearchResultEntry>, String> {
    if root_path.trim().is_empty() {
        return Ok(Vec::new());
    }

    tauri::async_runtime::spawn_blocking(move || {
        search_directory_recursive_blocking(root_path, query, max_depth, limit, typo_tolerance)
    })
    .await
    .map_err(|join_error| format!("Directory recursive search task failed: {join_error}"))?
}

fn global_search_query_paths_blocking(
    paths: Vec<String>,
    query: String,
    options: GlobalSearchQueryOptions,
) -> Result<Vec<GlobalSearchResultEntry>, String> {
    let normalized_query = normalize_case(&query);
    let min_score = get_min_score_for_query_length(normalized_query.len());

    let all_searchable_paths: Vec<PathBuf> = paths
        .par_iter()
        .flat_map(|path_string| {
            let path = Path::new(path_string);
            let mut collected_paths = Vec::new();

            if let Ok(metadata) = std::fs::metadata(path) {
                if metadata.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(path) {
                        for entry_result in entries.flatten() {
                            collected_paths.push(entry_result.path());
                        }
                    }
                } else {
                    collected_paths.push(path.to_path_buf());
                }
            }

            collected_paths
        })
        .collect();

    let results: Vec<GlobalSearchResultEntry> = all_searchable_paths
        .par_iter()
        .filter_map(|path| {
            let name = path
                .file_name()
                .and_then(|segment| segment.to_str())?
                .to_string();

            if options.exact_match && !matches_exact_name(&normalized_query, &name) {
                return None;
            }

            let name_score = calculate_similarity_score(&normalized_query, &name);

            if name_score < min_score {
                return None;
            }

            let metadata = std::fs::metadata(path).ok()?;

            let is_file = metadata.is_file();
            let is_dir = metadata.is_dir();

            if !matches_type(
                if is_file { 1 } else { 0 },
                if is_dir { 1 } else { 0 },
                &options,
            ) {
                return None;
            }

            let (modified_time, accessed_time, created_time) = metadata_times_unix_ms(&metadata);

            let size = if is_file { metadata.len() } else { 0 };

            let ext = path_extension_lowercase(path);

            let path_string = path.to_string_lossy().to_string();
            let normalized_path = normalize_path(&path_string);

            Some(GlobalSearchResultEntry {
                name,
                ext,
                path: normalized_path,
                size,
                item_count: None,
                modified_time,
                accessed_time,
                created_time,
                mime: None,
                is_file,
                is_dir,
                is_symlink: metadata.is_symlink(),
                is_hidden: is_hidden_path(path),
                score: name_score,
            })
        })
        .collect();

    let mut sorted_results = results;
    sorted_results.par_sort_by(|entry_a, entry_b| {
        entry_b
            .score
            .partial_cmp(&entry_a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if sorted_results.len() > options.limit {
        sorted_results.truncate(options.limit);
    }

    Ok(sorted_results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::global_search::index::build_schema;
    use tantivy::doc;

    fn create_options(exact_match: bool) -> GlobalSearchQueryOptions {
        GlobalSearchQueryOptions {
            limit: 10,
            include_files: true,
            include_directories: true,
            exact_match,
            typo_tolerance: true,
            min_score_threshold: Some(0.0),
        }
    }

    fn search_names(query_text: &str, exact_match: bool) -> Vec<String> {
        let (schema, fields) = build_schema();
        let index = tantivy::Index::create_in_ram(schema);
        let mut writer = index.writer(50_000_000).unwrap();

        for name in [
            "Exile by Aleksey Hoffman.jpg",
            "Exiled by Aleksey Hoffman.jpg",
            "Example.jpg",
        ] {
            writer
                .add_document(doc!(
                    fields.path => format!("C:/test/{name}"),
                    fields.name => name.to_string(),
                    fields.name_lower => name.to_lowercase(),
                    fields.is_file => 1u64,
                    fields.is_dir => 0u64,
                    fields.modified_time => 0u64,
                    fields.size => 0u64,
                ))
                .unwrap();
        }

        writer.commit().unwrap();

        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let query = build_query(&fields, query_text, &create_options(exact_match));
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(10).order_by_score())
            .unwrap();

        top_docs
            .into_iter()
            .filter_map(|(_score, address)| {
                let doc: tantivy::TantivyDocument = searcher.doc(address).ok()?;
                doc.get_first(fields.name)
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string())
            })
            .collect()
    }

    #[test]
    fn exact_match_query_matches_exact_filename_token() {
        let names = search_names("exile", true);

        assert!(names.contains(&"Exile by Aleksey Hoffman.jpg".to_string()));
    }

    #[test]
    fn exact_match_query_does_not_match_prefixed_token() {
        let names = search_names("exile", true);

        assert!(!names.contains(&"Exiled by Aleksey Hoffman.jpg".to_string()));
    }

    #[test]
    fn exact_name_filter_matches_all_query_tokens() {
        assert!(matches_exact_name(
            "exile hoffman",
            "Exile by Aleksey Hoffman.jpg"
        ));
        assert!(!matches_exact_name(
            "exile missing",
            "Exile by Aleksey Hoffman.jpg"
        ));
    }

    // FORK-MODIFICATION: 2026-10-16 —— 递归搜索在大目录里「形同没实现」的回归测试。
    //
    // 真实故障：collection_cap 原来是 limit * 20，而计数数的是**扫描过的条目**。
    // limit 默认 50 → 扫到第 1000 个条目就收工。用户在 43,167 个条目的照片目录里
    // 搜一个确实存在的文件名，永远得到「无匹配」，于是报告「递归搜索没实现」。
    //
    // 这个测试按同样的比例造夹具：造足够多的**无关**条目把目标文件挤到
    // collection_cap 之外，再断言它仍能被搜到。夹具只要改动 cap 的倍率就会红。
    mod recursive_scan_cap {
        use super::search_directory_recursive_blocking;
        use std::fs;
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        /// 夹具根目录。
        ///
        /// FORK-MODIFICATION: 2026-10-16 —— 刻意**不用 `tempfile::TempDir`**。
        /// `builtin_ignored_paths()` 覆盖 `/tmp` 与 `/AppData/Local/Temp`，而
        /// `TempDir` 默认就建在后者下面，于是 `filter_entry` 会把整棵夹具剪掉，
        /// 三个测试全部 0 命中 —— 看起来像「递归搜索完全失效」，实际是被忽略规则挡死了。
        /// 这是本项目第二次栽在同一个坑上（前端夹具那次记在 AGENTS.md 里），所以写进注释。
        fn fixture_root(label: &str) -> PathBuf {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:/".to_string());
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let seq = COUNTER.fetch_add(1, Ordering::Relaxed);

            PathBuf::from(home)
                .join("afm-rust-search-fixtures")
                .join(format!("{label}-{}-{nanos}-{seq}", std::process::id()))
        }

        struct Fixture {
            root: PathBuf,
        }

        impl Fixture {
            fn new(label: &str) -> Self {
                let root = fixture_root(label);
                let _ = fs::remove_dir_all(&root);
                fs::create_dir_all(&root).expect("create fixture root");

                Self { root }
            }

            fn path(&self) -> &Path {
                &self.root
            }
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.root);
            }
        }

        /// 造一个「噪音远多于目标」的目录树。
        ///
        /// 关键：目标文件放在**最后创建**的子目录里，而噪音条目数远超旧 cap。
        /// WalkDir 的遍历顺序受文件系统影响不可预测，但无论顺序如何，
        /// 旧实现在扫够 1000 条之前或之后都会丢掉目标 —— 两种顺序都能让测试转红。
        fn create_noisy_tree(noise_dirs: usize, noise_per_dir: usize) -> (Fixture, String) {
            let fixture = Fixture::new("noisy");

            for dir_index in 0..noise_dirs {
                let dir = fixture.path().join(format!("noise_{dir_index:04}"));
                fs::create_dir_all(&dir).expect("create noise dir");

                for file_index in 0..noise_per_dir {
                    fs::write(
                        dir.join(format!("noise_{dir_index:04}_{file_index:04}.bin")),
                        b"x",
                    )
                    .expect("write noise file");
                }
            }

            let target_dir = fixture.path().join("zzz_target");
            fs::create_dir_all(&target_dir).expect("create target dir");
            fs::write(target_dir.join("needle_095.txt"), b"hit").expect("write target");

            (fixture, "zzz_target/needle_095.txt".to_string())
        }

        fn to_slash(value: &str) -> String {
            value.replace('\\', "/")
        }

        #[test]
        fn finds_target_beyond_the_old_scan_cap() {
            // 30 目录 × 50 文件 = 1500 条噪音，远超旧实现的 1000 条扫描上限。
            let (fixture, target_relative) = create_noisy_tree(30, 50);

            let results = search_directory_recursive_blocking(
                to_slash(&fixture.path().to_string_lossy()),
                "needle".into(),
                7,
                50,
                false,
            )
            .expect("recursive search should succeed");

            assert!(
                results.iter().any(|entry| entry.path.ends_with(&target_relative)),
                "递归搜索必须能扫过 1500 条噪音后仍命中深层目标；\
                 实际命中 {} 条，路径示例 {:?}",
                results.len(),
                results.iter().map(|e| &e.path).take(3).collect::<Vec<_>>()
            );
        }

        #[test]
        fn matches_mid_name_substring_the_way_users_type_it() {
            // 用户搜 "095" 命中 "20251009_0095_27.jpg" —— 子串在文件名**中间**，
            // 不是前缀也不是整名。这正是真机截图里的那类查询。
            let fixture = Fixture::new("midname");
            let sub = fixture.path().join("1991_guangzhou");
            fs::create_dir_all(&sub).expect("create sub dir");
            fs::write(sub.join("20251009_0095_27.jpg"), b"x").expect("write target");

            let results = search_directory_recursive_blocking(
                to_slash(&fixture.path().to_string_lossy()),
                "095".into(),
                7,
                50,
                false,
            )
            .expect("recursive search should succeed");

            assert_eq!(
                results.len(),
                1,
                "三位数字查询必须命中文件名中段的条目；实际 {:?}",
                results.iter().map(|e| &e.path).collect::<Vec<_>>()
            );
        }

        #[test]
        fn respects_depth_limit() {
            let fixture = Fixture::new("depth");
            let deep = fixture.path().join("a").join("b").join("c");
            fs::create_dir_all(&deep).expect("create deep dir");
            fs::write(deep.join("needle_deep.txt"), b"hit").expect("write deep target");

            let root_path = to_slash(&fixture.path().to_string_lossy());

            // 深度 7 够得到 a/b/c。
            let found = search_directory_recursive_blocking(
                root_path.clone(),
                "needle".into(),
                7,
                50,
                false,
            )
            .expect("search should succeed");
            assert_eq!(found.len(), 1, "深度 7 时应命中 a/b/c/needle_deep.txt");

            // 深度 2 只够走到 a，够不到 a/b/c。
            let shallow = search_directory_recursive_blocking(root_path, "needle".into(), 2, 50, false)
                .expect("search should succeed");
            assert!(shallow.is_empty(), "深度 2 时不应命中 3 层深的文件");
        }

        #[test]
        fn truncates_to_limit_but_keeps_best_scoring_first() {
            let fixture = Fixture::new("truncate");

            for index in 0..10 {
                fs::write(
                    fixture.path().join(format!("needle_{index:02}.txt")),
                    b"x",
                )
                .expect("write file");
            }

            let results = search_directory_recursive_blocking(
                to_slash(&fixture.path().to_string_lossy()),
                "needle".into(),
                7,
                3,
                false,
            )
            .expect("search should succeed");

            assert_eq!(results.len(), 3, "limit=3 时只回 3 条");
        }

        // FORK-MODIFICATION: 2026-10-16 —— 容错开关真的有效果。
        //
        // 这一对是「假开关」与「真开关」的分界：如果 typo_tolerance=false 时
        // 也返回同样的结果，那界面上的复选框就只是个摆设。
        #[test]
        fn typo_tolerance_off_does_not_match_a_typo() {
            let fixture = Fixture::new("typo_off");
            fs::write(fixture.path().join("report_final.txt"), b"x").expect("write target");

            let results = search_directory_recursive_blocking(
                to_slash(&fixture.path().to_string_lossy()),
                "report_fianl".into(),
                7,
                50,
                false,
            )
            .expect("search should succeed");

            assert!(
                results.is_empty(),
                "关闭容错时拼错的查询不该命中；实际 {:?}",
                results.iter().map(|e| &e.path).collect::<Vec<_>>()
            );
        }

        #[test]
        fn typo_tolerance_on_matches_a_near_miss() {
            let fixture = Fixture::new("typo_on");
            fs::write(fixture.path().join("report_final.txt"), b"x").expect("write target");

            let results = search_directory_recursive_blocking(
                to_slash(&fixture.path().to_string_lossy()),
                "report_fianl".into(),
                7,
                50,
                true,
            )
            .expect("search should succeed");

            assert_eq!(
                results.len(),
                1,
                "开启容错后相邻字母颠倒应当命中；实际 {:?}",
                results.iter().map(|e| &e.path).collect::<Vec<_>>()
            );
        }

        #[test]
        fn typo_tolerance_does_not_swallow_unrelated_names() {
            let fixture = Fixture::new("typo_noise");
            fs::write(fixture.path().join("zzzzzzzzzzzz.txt"), b"x").expect("write noise");
            fs::write(fixture.path().join("report_final.txt"), b"x").expect("write target");

            let results = search_directory_recursive_blocking(
                to_slash(&fixture.path().to_string_lossy()),
                "report_fianl".into(),
                7,
                50,
                true,
            )
            .expect("search should succeed");

            assert_eq!(
                results.len(),
                1,
                "容错不该把毫不相关的文件名也收进来"
            );
        }
    }

    // FORK-MODIFICATION: 2026-10-28 —— 搜索内存优化（B1）的回归测试。
    //
    // 盯住三条不变量：
    //  1. 物化条数 ≤ limit（旧实现先物化 10 万条文档再 truncate，峰值 30–80 MB）；
    //  2. 候选条数先按 limit 截断；
    //  3. 排序 / 过滤语义与改动前一致（相似度高的排前面、类型过滤照旧生效）。
    //
    // 刻意**不**碰 `tauri::async_runtime` / `AppHandle`：测试二进制一旦链接到
    // 它们，Wry 运行时会被拖进来，`cargo test --lib` 会在加载阶段以
    // STATUS_ENTRYPOINT_NOT_FOUND 崩掉（见 dir_reader.rs 顶部的说明）。

    /// 建一个内存索引。entries 是 (文件名, is_file, is_dir)。
    fn build_budget_fixture(
        entries: &[(&str, u64, u64)],
    ) -> (tantivy::Index, tantivy::IndexReader, GlobalSearchIndexFields) {
        let (schema, fields) = build_schema();
        let index = tantivy::Index::create_in_ram(schema);
        let mut writer = index.writer(50_000_000).unwrap();

        for (name, is_file, is_dir) in entries {
            writer
                .add_document(doc!(
                    fields.path => format!("C:/afm-search-fixture/{name}"),
                    fields.name => name.to_string(),
                    fields.name_lower => name.to_lowercase(),
                    fields.is_file => *is_file,
                    fields.is_dir => *is_dir,
                    fields.modified_time => 0u64,
                    fields.size => 0u64,
                ))
                .unwrap();
        }

        writer.commit().unwrap();

        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::Manual)
            .try_into()
            .unwrap();

        (index, reader, fields)
    }

    fn budget_options(limit: usize, include_directories: bool) -> GlobalSearchQueryOptions {
        GlobalSearchQueryOptions {
            limit,
            include_files: true,
            include_directories,
            exact_match: false,
            typo_tolerance: true,
            min_score_threshold: None,
        }
    }

    #[test]
    fn materialized_documents_never_exceed_the_limit() {
        let entries: Vec<(String, u64, u64)> = (0..50)
            .map(|index| (format!("needle_{index:02}.txt"), 1u64, 0u64))
            .collect();
        let borrowed: Vec<(&str, u64, u64)> = entries
            .iter()
            .map(|(name, is_file, is_dir)| (name.as_str(), *is_file, *is_dir))
            .collect();

        let (_index, reader, fields) = build_budget_fixture(&borrowed);
        let query_options = budget_options(10, true);

        let searcher = reader.searcher();
        let candidates = collect_candidate_addresses(
            &searcher,
            &fields,
            "needle",
            &query_options,
            candidate_limit_for(&query_options, 1),
        )
        .unwrap();

        assert_eq!(
            candidates.len(),
            10,
            "候选文档必须先按 limit 截断，不能把 50 条都交给物化"
        );

        let (results, materialized_count) = materialize_candidates(
            &searcher,
            &fields,
            &query_options,
            &normalize_case("needle"),
            &candidates,
        );

        assert!(
            materialized_count <= query_options.limit,
            "物化条数必须 ≤ limit，实际 {materialized_count}"
        );
        assert_eq!(materialized_count, 10);
        assert!(
            results.len() <= query_options.limit,
            "结果条数必须 ≤ limit，实际 {}",
            results.len()
        );
    }

    #[test]
    fn candidate_budget_scales_with_indexed_drive_count() {
        let options = budget_options(50, true);

        assert_eq!(
            candidate_limit_for(&options, 1),
            50,
            "单盘符时候选预算应等于 limit"
        );
        assert_eq!(
            candidate_limit_for(&options, 3),
            150,
            "多盘符时候选预算应为 limit 乘盘符数（保留旧的每盘符 limit 语义）"
        );
        assert_eq!(
            candidate_limit_for(&options, 0),
            50,
            "盘符数未知时按单盘符兜底"
        );
        assert_eq!(
            candidate_limit_for(&options, usize::MAX),
            MAX_SEARCH_CANDIDATE_MATERIALIZATION,
            "极端盘符数必须被绝对上限截断"
        );
    }

    #[test]
    fn keeps_similarity_ordering_and_type_filtering() {
        let (_index, reader, fields) = build_budget_fixture(&[
            ("needle.txt", 1, 0),
            ("needle longer name.txt", 1, 0),
            ("needle folder", 0, 1),
        ]);

        let query_options = budget_options(10, false);
        let searcher = reader.searcher();
        let candidates = collect_candidate_addresses(
            &searcher,
            &fields,
            "needle",
            &query_options,
            candidate_limit_for(&query_options, 1),
        )
        .unwrap();
        let (materialized, _) = materialize_candidates(
            &searcher,
            &fields,
            &query_options,
            &normalize_case("needle"),
            &candidates,
        );

        let results = finalize_results(materialized, query_options.limit);

        assert_eq!(
            results.len(),
            2,
            "include_directories=false 时目录必须被过滤掉；实际 {:?}",
            results.iter().map(|entry| &entry.name).collect::<Vec<_>>()
        );
        assert!(results.iter().all(|entry| entry.is_file));
        assert_eq!(
            results[0].name, "needle.txt",
            "相似度高的（完全相等 1.0）必须排在前面"
        );
        assert!(
            results[0].score > results[1].score,
            "排序必须按相似度降序，实际 {} vs {}",
            results[0].score,
            results[1].score
        );
    }
}
