// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

use crate::utils::{
    is_hidden_path, metadata_times_unix_ms, normalize_path, path_extension_lowercase,
};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{
    AllQuery, BooleanQuery, FuzzyTermQuery, Occur, Query, RegexQuery, TermQuery,
};
use tantivy::schema::{IndexRecordOption, Value};
use tantivy::Term;
use walkdir::WalkDir;

use super::ignore::{builtin_ignored_paths, get_drive_root, normalize_case, IgnoredPathMatcher};
use super::incremental::escape_regex_literal;
use super::index::{index_dir, open_or_create_index, remote_index_dir};
use super::remote_index::open_remote_reader;
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

    // FORK-MODIFICATION: 2026-11-25 —— W-12「中文子串检索失效」的查询侧兜底。
    //
    // 根因不在打分，而在**取不到候选**：`name` 字段用 `default` 分词器，而
    // SimpleTokenizer 按「非字母数字」切分，CJK 字符的 `is_alphanumeric()` 为真
    // ⇒ 一整段连续中文只会产出**一个** token：
    //
    //   "2024年用户调研技巧汇总.docx" -> ["2024年用户调研技巧汇总", "docx"]
    //
    // 而 FuzzyTermQuery 是**整词**比对：查询 "技巧" 要命中上面那个 term 需要
    // 编辑距离 ≤ 1~2，实际相距 6 ⇒ 一条候选都取不到。于是「中文只要不独立成词
    // 就永远搜不到」，实测 MCP：`技巧` 只返 7 条（全是「技巧」独立成词的），
    // `收集` 返 0 条。
    //
    // 修法：查询侧补一条 `RegexQuery(".*<词>.*")`。tantivy 0.26 的 RegexQuery
    // 走 term 词典的自动机枚举，`.*技巧.*` 能命中 term "2024年用户调研技巧汇总"。
    // 因此**不需要改 schema、不需要重建索引** —— 现有 23,334 条云盘索引照旧可用
    // （改分词器则必须整库重建，代价与风险都不成比例）。
    //
    // 只对**含非 ASCII 字符**的词加这一条，理由：
    //   - CJK 没有分隔符，子串命中是常态；ASCII 词由 `.`/`_`/`-`/空白天然切开，
    //     fuzzy 已能覆盖绝大多数，且 ASCII 单字符的正则展开会命中海量 term，
    //     成本远大于收益（ASCII 中缀子串失效属另一类问题，不在 W-12 范围）。
    //   - `exact_match` 分支刻意不加：「精确匹配」的语义是整词相等，
    //     `matches_exact_name` 事后也会把子串命中再筛掉一遍。
    //
    // ⚠ RegexQuery 的命中都是同一个常量分，所以候选条数超过 limit 时
    // 「具体取哪几条」由 tantivy 的文档顺序决定；最终排序仍由
    // `materialize_candidates` / `finalize_results` 按名字相似度重排。
    for word in &words {
        if let Some(substring_query) = build_substring_query(fields, word) {
            subqueries.push((Occur::Should, substring_query));
        }
    }

    Box::new(BooleanQuery::from(subqueries))
}

/// 「这个词作为子串出现」的查询；对纯 ASCII 词返回 `None`（见 `build_query` 注释）。
///
/// tantivy 0.26 的 `RegexQuery` 底层是 `tantivy-fst::Regex` —— DFA 引擎，
/// **全匹配**语义且不支持 `^` / `$` 这类零宽断言（见 `incremental.rs:192-195`），
/// 所以两侧的 `.*` 是必需的，不能靠锚点。
fn build_substring_query(
    fields: &GlobalSearchIndexFields,
    word: &str,
) -> Option<Box<dyn Query>> {
    if word.is_ascii() {
        return None;
    }

    let pattern = format!(".*{}.*", escape_regex_literal(word));

    match RegexQuery::from_pattern(&pattern, fields.name) {
        Ok(query) => Some(Box::new(query)),
        Err(error) => {
            // 转义之后理论上不会失败；真失败就退化成「只有 fuzzy」，
            // 也就是改动前的行为，不阻断查询。
            log::warn!("[global-search] 子串查询正则构造失败（pattern={pattern}）：{error}");
            None
        }
    }
}

// FORK-MODIFICATION: 2026-11-25 —— W-13「CJK 子串命中在候选池饱和时被挤出」的召回层修复。
//
// ## 症状
//
// 台账 `docs/open-items-2026-11-24.md` §4 末行：查「分析」这类**高频 2 字词**时，
// 候选池直接打满服务端硬上限 500 条，靶子 `Midjourney 使用心得+关键词分析.mp4`
// 一条都搜不到（改用 5 字的「关键词分析」就稳定命中，score=0.92）。
//
// ## 根因（不是排序错，是**候选阶段就被扔掉了**）
//
// ① `build_query` 里的子串正则是**众多 Should 分支之一**。tantivy 的
//    `RegexQuery` 走 `AutomatonWeight`，命中只拿一个**常量分**；
//    而 fuzzy 分支拿的是 BM25 分。
// ② 候选池 = `TopDocs::with_limit(limit)`，即**按 BM25 分取前 N 条**。
//    ⇒ fuzzy 命中的文档（BM25 分高）把池子填满，只靠子串命中的文档
//    （常量分，排在后面）在**候选阶段**就被切掉了。
// ③ 后面再怎么按相似度排序也救不回来 —— 池子里已经没有它了。
//
// ⇒ 关键推论：**给正则加 `BoostQuery` 提权救不了**。同分时 tantivy 按文档序排，
// 后写入索引的连接（实测里阿里云盘排在百度后面）会整批落在后面，
// 池子一填满还是被挤掉。必须在**召回层**解决。
//
// ## 修法
//
// 单独跑一条「**只有子串正则**」的查询，把「名字里含这个词」的文档**整批**捞出来，
// 与主池合并后交给**同一套**相似度排序去挑。
//
// ⇒ **最终排序口径一个字没改**（仍是名称相似度降序），只是候选更全 ——
// 这是「召回修复」，不是「排序改写」。
//
// ## 为什么子串池要「整批取」而不是取前 N 条
//
// 正则命中同分 ⇒ tantivy 按文档序排 ⇒ 取前 N 条等于「只留最早写入索引的那批」。
// 所以先用 `Count` 收集器把命中总数数一遍：命中数 ≤ 上限时**全部取回**（无偏），
// 超过上限才退化成取前 N 条并记一条 WARN（结果可能不完整，但不会拖垮查询）。

/// CJK 子串候选池的条数硬顶。
///
/// 取值依据：子串候选是**唯一**能把「名字里含这个词」的文档捞出来的通道，
/// 一旦被截断，截掉的很可能就是用户最想要的那几条（且正则命中同分，
/// 截断时优先丢的是**后写入索引**的连接 —— 实测里就是阿里云盘）。
/// 20,000 与云盘索引「每连接 2 万条」的配额同量级：再大也不会多召回什么，
/// 只会让单次查询的物化开销变高。
const SUBSTRING_CANDIDATE_LIMIT: usize = 20_000;

/// CJK 子串候选池的条数下限（保底）。
///
/// 4,000 条 ≈ 1–2 MB 量级的瞬时物化开销（对比批次 37 之前的 10 万条 / 30–80 MB），
/// 但足以让「分析」这种一次命中成百上千条的高频 2 字词**整批取回**，
/// 不被主池的 BM25 顺序挤掉。
const SUBSTRING_CANDIDATE_FLOOR: usize = 4_000;

/// 子串候选池的条数：主池的数倍，但**不低于 `SUBSTRING_CANDIDATE_FLOOR`**、
/// 不高于 `SUBSTRING_CANDIDATE_LIMIT`。
fn substring_candidate_limit_for(options: &GlobalSearchQueryOptions) -> usize {
    options
        .limit
        .max(1)
        .saturating_mul(4)
        .clamp(SUBSTRING_CANDIDATE_FLOOR, SUBSTRING_CANDIDATE_LIMIT)
}

/// 只含「子串正则」的独立查询（不含 fuzzy 分支）。
///
/// # 为什么要这条独立通道
///
/// 见文件头 W-13 说明：子串正则在主查询里只是众多 Should 分支之一，
/// 命中只拿常量分 ⇒ 必然排在 BM25 分更高的 fuzzy 命中后面 ⇒ 池子一填满就被切掉。
///
/// # 返回 `None` 的两种情况（都与改动前逐字一致）
///
/// - 精确匹配模式：语义是「整词相等」，子串命中会被 `matches_exact_name` 筛掉；
/// - 全 ASCII 查询：没有子串通道 ⇒ **不多跑一次检索**，行为与改动前完全一致。
fn build_substring_only_query(
    fields: &GlobalSearchIndexFields,
    query: &str,
    options: &GlobalSearchQueryOptions,
) -> Option<Box<dyn Query>> {
    if options.exact_match {
        return None;
    }

    let normalized = normalize_case(query);
    let words = split_query_tokens(&normalized);

    let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
    for word in &words {
        if let Some(substring_query) = build_substring_query(fields, word) {
            subqueries.push((Occur::Should, substring_query));
        }
    }

    if subqueries.is_empty() {
        return None;
    }

    Some(Box::new(BooleanQuery::from(subqueries)))
}

/// 收集「名字里含查询词」的候选文档地址（子串候选池）。
///
/// 返回 `(候选地址, 命中总数)`。
///
/// `connection_filter` 传 `Some` 时（云盘侧）按当前勾选的连接集合过滤 ——
/// 与主查询的口径一致，避免把「用户已取消勾选的网盘」捞进来。
fn collect_substring_candidate_addresses(
    searcher: &tantivy::Searcher,
    fields: &GlobalSearchIndexFields,
    query: &str,
    options: &GlobalSearchQueryOptions,
    limit: usize,
    connection_filter: Option<Box<dyn Query>>,
) -> Result<(Vec<tantivy::DocAddress>, usize), String> {
    let Some(substring_query) = build_substring_only_query(fields, query, options) else {
        return Ok((Vec::new(), 0));
    };

    let query_boxed: Box<dyn Query> = match connection_filter {
        Some(filter) => Box::new(BooleanQuery::new(vec![
            (Occur::Must, substring_query),
            (Occur::Must, filter),
        ])),
        None => substring_query,
    };

    let matched = searcher
        .search(&query_boxed, &Count)
        .map_err(|error| error.to_string())?;

    let top_docs = searcher
        .search(&query_boxed, &TopDocs::with_limit(limit).order_by_score())
        .map_err(|error| error.to_string())?;

    let candidates: Vec<tantivy::DocAddress> = top_docs
        .into_iter()
        .map(|(_score, address)| address)
        .take(limit)
        .collect();

    if matched > candidates.len() {
        log::warn!(
            "[global-search] 子串候选命中 {matched} 条，超过候选上限 {limit}，本次只取前 {} 条（结果可能不完整）",
            candidates.len()
        );
    }

    Ok((candidates, matched))
}

/// 合并主池与子串池（按文档地址去重，主池在前）。
///
/// 去重是必须的：主查询里也含子串正则，两边会取到同一批文档；
/// 不去重会让同一条结果出现两次（排序键相同 ⇒ 相邻，看起来像重复项）。
fn merge_candidate_pools(
    primary: Vec<tantivy::DocAddress>,
    extra: Vec<tantivy::DocAddress>,
) -> Vec<tantivy::DocAddress> {
    if extra.is_empty() {
        return primary;
    }

    let mut seen: std::collections::HashSet<(u32, u32)> = primary
        .iter()
        .map(|address| (address.segment_ord, address.doc_id))
        .collect();

    let mut merged = primary;
    for address in extra {
        if seen.insert((address.segment_ord, address.doc_id)) {
            merged.push(address);
        }
    }

    merged
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
    // FORK-MODIFICATION 2026-11-02：查询路径计时埋点。
    //
    // 用户报「全盘搜索非常缓慢，甚至等不到结果」，但此前 query.rs 里
    // 零计时 —— 拿不到任何耗时数字，定位只能靠猜。这里把一次查询拆成
    // 「索引打开 / 候选收集 / 物化 / 收尾」四段，连同索引总文档数一起
    // 打进日志，让「慢」变成可比较的数字。
    let query_started = std::time::Instant::now();
    let mut index_open_ms: u128 = 0;

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
        let open_started = std::time::Instant::now();
        let (index, reader, fields) = open_or_create_index(&index_path)?;
        index_open_ms = open_started.elapsed().as_millis();
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
    let collect_started = std::time::Instant::now();
    let primary_candidates =
        collect_candidate_addresses(&searcher, &fields, &query, &options, candidate_limit)?;

    // FORK-MODIFICATION 2026-11-25（W-13）：追加「子串候选池」。
    //
    // 主池按 **BM25 分**取前 N 条 ⇒ 只靠子串正则命中的文档（常量分，排在
    // fuzzy 命中之后）会在**候选阶段**被切掉，后面再怎么按相似度排序也救不回来。
    // 子串池把这些「名字里含查询词」的文档整批捞进来 ⇒ 召回变全，
    // 而**最终排序口径一个字没改**（仍是名称相似度降序）。
    let (substring_candidates, substring_matched) = collect_substring_candidate_addresses(
        &searcher,
        &fields,
        &query,
        &options,
        substring_candidate_limit_for(&options),
        None,
    )?;
    let substring_candidate_count = substring_candidates.len();
    let candidates = merge_candidate_pools(primary_candidates, substring_candidates);
    let collect_ms = collect_started.elapsed().as_millis();

    let materialize_started = std::time::Instant::now();
    let (results, _materialized_count) =
        materialize_candidates(&searcher, &fields, &options, &normalized_query, &candidates);
    let materialize_ms = materialize_started.elapsed().as_millis();

    let finalize_started = std::time::Instant::now();
    let finalized = finalize_results(results, options.limit);
    let finalize_ms = finalize_started.elapsed().as_millis();

    // FORK-MODIFICATION 2026-11-22（批次 37）：合并云盘（远程连接）结果。
    //
    // 云盘索引是**独立**的一份 tantivy 索引（理由见 `remote_index.rs` 的模块头注释），
    // 这里用**同一个查询、同一个选项**去查它，再与本地结果合并。
    //
    // 没有云盘索引时（`None`）**原样返回本地结果** —— 行为与加这个功能之前
    // 逐字一致：不报错、结果集不变、条数不变。
    let merged = match search_remote_index(&base_dir, &query, &options) {
        None => finalized,
        Some(remote_results) => merge_search_results(finalized, remote_results, options.limit),
    };

    log::info!(
        "[global-search] query timings: total={}ms index_open={}ms collect={}ms materialize={}ms finalize={}ms candidates={} results={} substring_candidates={} substring_matched={} index_docs={}",
        query_started.elapsed().as_millis(),
        index_open_ms,
        collect_ms,
        materialize_ms,
        finalize_ms,
        candidates.len(),
        merged.len(),
        substring_candidate_count,
        substring_matched,
        searcher.num_docs()
    );

    Ok(merged)
}

/// 查询云盘索引。
///
/// **返回 `None` 表示「没有云盘索引」** —— 调用方据此保持改动前的行为
/// （不报错、不改结果集）。索引存在但一条都没匹配上时返回 `Some(空 vec)`，
/// 两者必须分得清。
///
/// 过滤口径与本地查询**完全一致**（同一个 `build_query` + 同一个
/// `materialize_candidates`）：相似度门槛、精确匹配、文件/目录类型都按同一套规则。
///
/// # 云盘结果的可见性由「当前勾选集合」决定
///
/// 批次 43 登记的遗留项 O-1：**索引里有什么**（重建时传进去的集合）与
/// **能不能被搜到**（用户当前勾了哪些）是两件事。
/// 只有前者时，「重建中途失败 ⇒ 旧索引还在」就会让已经被取消勾选的网盘
/// 继续被搜到。
///
/// ⇒ 这里在**索引层**按当前勾选集合过滤（`connection_id` 上的 `TermQuery`），
/// 不是物化后再筛：
/// 后筛会让被取消勾选的网盘先把候选预算吃光，勾选中的网盘反而一条都搜不到。
fn search_remote_index(
    base_dir: &Path,
    query: &str,
    options: &GlobalSearchQueryOptions,
) -> Option<Vec<GlobalSearchResultEntry>> {
    let index_path = remote_index_dir(base_dir);
    let (reader, fields) = open_remote_reader(&index_path)?;

    let searcher = reader.searcher();
    let normalized_query = normalize_case(query);
    let candidate_limit = remote_candidate_limit_for(options);

    // 当前勾选了「加入全局检索」的连接 —— 查询侧的过滤依据。
    let user_data_dir = super::remote_index::user_data_dir_in(base_dir);
    let allowed_connection_ids = super::remote_index::enabled_connection_ids(&user_data_dir);

    if allowed_connection_ids.is_empty() {
        // 一个网盘都没勾 ⇒ 云盘一条都不该出现。
        // 注意这**不是**「没有云盘索引」（`None`），而是「有索引但当前没有可见的连接」。
        //
        // W-8（批次 45）：这条路径以前无声，日志上和「索引是空的」完全分不开。
        // 每次输入都会命中，所以只在**首次**说一遍；重新勾上后再取消，
        // 进程内不会重复刷，但日志里已经留下了这条判据。
        static LOGGED_EMPTY_SELECTION: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !LOGGED_EMPTY_SELECTION.swap(true, std::sync::atomic::Ordering::Relaxed) {
            log::info!(
                "[global-search] 当前没有任何勾选「加入全局检索」的云盘连接，本次查询云盘结果按空处理"
            );
        }
        return Some(Vec::new());
    }

    let primary_candidates = match collect_remote_candidate_addresses(
        &searcher,
        &fields,
        query,
        options,
        candidate_limit,
        &allowed_connection_ids,
    ) {
        Ok(candidates) => candidates,
        Err(error) => {
            log::warn!("[global-search] 云盘索引查询失败：{}", error);
            return Some(Vec::new());
        }
    };

    // FORK-MODIFICATION 2026-11-25（W-13）：云盘侧同样追加「子串候选池」。
    //
    // 云盘的实测症状最典型（W-13 台账行）：查「分析」这类高频 2 字词时，
    // 候选池被百度那批 fuzzy 命中填满，**后写入索引的阿里云盘**里
    // 「只能靠子串命中」的文档一条都进不来 ⇒ 用户的云盘中文文件「搜不到」。
    //
    // 过滤口径与主查询一致：同样按当前勾选的连接集合过滤，
    // 不会把「用户已取消勾选的网盘」捞进来。
    let (substring_candidates, substring_matched) = match collect_substring_candidate_addresses(
        &searcher,
        &fields,
        query,
        options,
        substring_candidate_limit_for(options),
        Some(build_connection_filter(&fields, &allowed_connection_ids)),
    ) {
        Ok(result) => result,
        Err(error) => {
            log::warn!("[global-search] 云盘子串候选查询失败：{}", error);
            (Vec::new(), 0)
        }
    };
    let substring_candidate_count = substring_candidates.len();
    let candidates = merge_candidate_pools(primary_candidates, substring_candidates);

    if substring_matched > 0 {
        log::info!(
            "[global-search] 云盘子串候选：命中 {} 条，入池 {} 条",
            substring_matched,
            substring_candidate_count
        );
    }

    let (mut results, _materialized_count) =
        materialize_candidates(&searcher, &fields, options, &normalized_query, &candidates);

    // W-14 (B) / 批次 53：云盘池内部按「分数降序 + 同分带内连接公平」重排。物化返回的是
    // 候选池顺序 = 主池 BM25 序 + 子串池文档序 = **索引写入序**（真机是 7z → 百度网盘 →
    // 阿里云盘）。不重排的话，合并层留出的保底席位会被最早写入的那个连接整批吃掉，用户刚
    // 上传的靶子照样进不了窗口。排序只发生在**同一个云盘索引内部**，不做跨来源新近。
    order_remote_hits(&mut results);

    Some(results)
}

/// 批次 52 W-14 (B) + 批次 53：云盘命中池的排列口径 = **分数降序，同分带内按连接轮转席位**。
///
/// 批次 52 的口径是「同分带内一律 `modified_time` 降序」，真机立刻暴露另一个极端：
/// `分析` 的 0.92 同分带 101 席全部属于连接 `0f0696ba`，靶子所属连接 `646dacdb` 在 200
/// 与 500 窗口内各拿 0 席 —— 「谁的文件最新谁独占整带」和「谁先写入谁独占整带」是同一个
/// 缺陷的两个极端。取证见 `docs/open-items-2026-11-24.md` §7（批次 53）。
///
/// 新口径：分数不同仍按分数优先；**同分带内**每个连接轮流拿席位，组内按 mtime 新近优先，
/// 组序按该组最新 mtime 降序（mtime 全为 0 时退化为连接 id 升序，仍然确定）。
/// 只重排，不删条目、不丢命中。
fn order_remote_hits(results: &mut Vec<GlobalSearchResultEntry>) {
    if results.len() < 2 {
        return;
    }

    // 第一步铺平：分数降序 + 同分内 mtime 降序 ⇒ 每个连接组的第一条就是该组最新的一条。
    // 用 total_cmp 而不是 partial_cmp：NaN 分数也要有确定次序，不能退化成不稳定比较。
    results.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| b.modified_time.cmp(&a.modified_time))
    });

    // 第二步：逐个同分带做连接轮转。带与带之间的次序由第一步保证，这里绝不改动。
    let flat = std::mem::take(results);
    let mut ordered = Vec::with_capacity(flat.len());
    let mut rest = flat;
    while !rest.is_empty() {
        let band_score = rest[0].score;
        let band_end = rest
            .iter()
            .position(|entry| entry.score.total_cmp(&band_score) != std::cmp::Ordering::Equal)
            .unwrap_or(rest.len());
        let band = rest.drain(..band_end).collect::<Vec<_>>();
        ordered.extend(rotate_band_by_connection(band));
    }
    *results = ordered;
}

/// 同分带内按 `connection_id` 公平轮转发席位：每轮每个连接出一条，直到全部发完。
///
/// 组序 = 该组最新 `modified_time` 降序，并列时按连接 id 升序；组内维持传进来的 mtime
/// 降序。带里只有一个连接（或全是解析不出连接的条目）时原样返回，行为与批次 52 逐条一致。
fn rotate_band_by_connection(band: Vec<GlobalSearchResultEntry>) -> Vec<GlobalSearchResultEntry> {
    if band.len() < 2 {
        return band;
    }

    let mut groups: std::collections::BTreeMap<
        String,
        std::collections::VecDeque<GlobalSearchResultEntry>,
    > = std::collections::BTreeMap::new();
    for entry in band {
        // 解析不出连接的条目并成一类：它们没有「哪个连接」可言，不构成公平性问题。
        let key = cloud_connection_id(&entry).unwrap_or_default();
        groups.entry(key).or_default().push_back(entry);
    }
    if groups.len() < 2 {
        return groups.into_values().collect::<Vec<_>>().into_iter().flatten().collect();
    }

    let mut queues = groups.into_iter().collect::<Vec<_>>();
    queues.sort_by(|a, b| {
        let newest = |group: &(String, std::collections::VecDeque<GlobalSearchResultEntry>)| {
            group.1.front().map(|entry| entry.modified_time)
        };
        newest(b)
            .cmp(&newest(a))
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut out = Vec::new();
    loop {
        let mut handed_out = false;
        for (_, queue) in queues.iter_mut() {
            if let Some(entry) = queue.pop_front() {
                out.push(entry);
                handed_out = true;
            }
        }
        if !handed_out {
            return out;
        }
    }
}

/// 条目所属的云盘连接身份。
///
/// 复用应用的 `sfm://` URI 权威解析器 `remote::path::parse_remote_path`（
/// `remote_index.rs` 的 `remote_path_of` 用的也是它），**不是**嗅探字面量前缀。批次 52
/// 注释里「不嗅探 path 前缀」针对的是「本地 vs 云盘」的来源标记 —— 那个仍然由
/// `merge_search_results` 的元组携带；这里只在**已经全是云盘条目的池内部**分连接。
fn cloud_connection_id(entry: &GlobalSearchResultEntry) -> Option<String> {
    crate::remote::parse_remote_path(&entry.path).map(|remote| remote.connection_id)
}

/// 取云盘索引的候选文档地址，**按当前勾选的连接集合过滤**。
///
/// 过滤打在 `connection_id` 字段上（`TermQuery` + `BooleanQuery` 的 `Should`），
/// 与文本查询用 `Must` 组合 —— 两条都得满足才算命中。
fn collect_remote_candidate_addresses(
    searcher: &tantivy::Searcher,
    fields: &GlobalSearchIndexFields,
    query: &str,
    options: &GlobalSearchQueryOptions,
    limit: usize,
    allowed_connection_ids: &std::collections::BTreeSet<String>,
) -> Result<Vec<tantivy::DocAddress>, String> {
    let text_query = build_query(fields, query, options);
    let connection_filter = build_connection_filter(fields, allowed_connection_ids);

    let query_boxed: Box<dyn Query> = Box::new(BooleanQuery::new(vec![
        (Occur::Must, text_query),
        (Occur::Must, connection_filter),
    ]));

    let top_docs = searcher
        .search(&query_boxed, &TopDocs::with_limit(limit).order_by_score())
        .map_err(|error| error.to_string())?;

    Ok(top_docs
        .into_iter()
        .map(|(_tantivy_score, doc_address)| doc_address)
        .take(limit)
        .collect())
}

/// 构造「条目属于当前勾选连接之一」这个过滤器。
///
/// 空 id 直接跳过：`Term::from_field_text` 拿空串构造出的 term 匹配不到任何东西，
/// 留着只会让过滤器平白多一个分支。
fn build_connection_filter(
    fields: &GlobalSearchIndexFields,
    allowed_connection_ids: &std::collections::BTreeSet<String>,
) -> Box<dyn Query> {
    let connection_id_field = fields
        .connection_id
        .expect("云盘索引 schema 必须带 connection_id 字段");

    let subqueries: Vec<(Occur, Box<dyn Query>)> = allowed_connection_ids
        .iter()
        .filter(|connection_id| !connection_id.is_empty())
        .map(|connection_id| {
            let term = Term::from_field_text(connection_id_field, connection_id);

            (
                Occur::Should,
                Box::new(TermQuery::new(term, IndexRecordOption::Basic)) as Box<dyn Query>,
            )
        })
        .collect();

    Box::new(BooleanQuery::new(subqueries))
}

/// 合并本地结果与云盘结果。
///
/// 规则（顺序即优先级）：
///
/// 1. **按 path 去重**：同一个 path 出现两次时保留**分数高**的一条；
///    分数相同保留本地那条（本地先入队）。
/// 2. **按 score 降序**排全局序。`sort_by` 是稳定排序 ⇒ 分数相同时
///    本地条目排在云盘条目前面 —— 本地结果优先，是这个产品一贯的取向。
/// 3. **同分带内给云盘保底席位**（FORK-MODIFICATION 2026-11-25，W-14 候选②）：
///    见 `reserve_cloud_seats_in_cut_band`。只重排「正好卡在窗口边界那个分数」的
///    同分带，分数高于它的本地条目一条都不会被挤掉。
/// 4. **总量截断到 `limit`**：`limit` 就是「结果条数上限」这个旋钮本身
///    （见 `candidate_limit_for` 的注释），合并后仍然只给用户这么多条。
pub(super) fn merge_search_results(
    local: Vec<GlobalSearchResultEntry>,
    remote: Vec<GlobalSearchResultEntry>,
    limit: usize,
) -> Vec<GlobalSearchResultEntry> {
    // 元组第二位 = 「这条来自云盘」。`GlobalSearchResultEntry` 自身没有来源字段，
    // 而保底配额要按来源分席 ⇒ 在合并过程中就把来源带着走，
    // 不去嗅探 path 前缀（`sfm://remote/` 是 `location.rs` 的实现细节，不是这里的契约）。
    let mut merged: Vec<(GlobalSearchResultEntry, bool)> =
        Vec::with_capacity(local.len() + remote.len());
    let mut position_by_path: std::collections::HashMap<String, usize> =
        std::collections::HashMap::with_capacity(local.len() + remote.len());

    for (entry, from_cloud) in local
        .into_iter()
        .map(|entry| (entry, false))
        .chain(remote.into_iter().map(|entry| (entry, true)))
    {
        match position_by_path.get(&entry.path) {
            Some(position) => {
                if entry.score > merged[*position].0.score {
                    merged[*position] = (entry, from_cloud);
                }
            }
            None => {
                position_by_path.insert(entry.path.clone(), merged.len());
                merged.push((entry, from_cloud));
            }
        }
    }

    merged.sort_by(|entry_a, entry_b| {
        entry_b
            .0
            .score
            .partial_cmp(&entry_a.0.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    reserve_cloud_seats_in_cut_band(&mut merged, limit);
    merged.truncate(limit);

    merged
        .into_iter()
        .map(|(entry, _from_cloud)| entry)
        .collect()
}

/// W-14 候选②：**同分命中按来源保底配额**（FORK-MODIFICATION 2026-11-25）。
///
/// 治的症状（真机实测）：查「分析」时本地 5,200 条、云盘 307 条**全部同分 0.92**，
/// 规则 2 的稳定排序让本地恒在云盘之前，规则 4 的 `truncate(limit)` 随即把整批云盘
/// 切在窗口之外 ⇒ 用户的云盘中文文件「搜不到」。批次 50 已让 307 条全部入池，
/// 丢是丢在这一层。
///
/// 做法：只在 **cut 分那一条同分带**内重排 —— `cut = merged[limit - 1].score`，
/// 即「正好卡在窗口边界上」的那个分数。把窗口内该带尾部的若干**本地**席位，
/// 让给窗口外该带的**云盘**条目，目标是云盘占到窗口的一半（台账候选②「各占一半」）。
///
/// 四条硬约束：
///
/// - **不碰更高分的条目**：只重排 `[band_start, band_end)`，分数高于 cut 的原样不动；
/// - **不丢数据**：让出席位的本地条目挪到窗口之后（由调用方 `truncate` 处理），
///   本函数只重排、总长度不变；
/// - **本地优先不破**：同分带在窗口内只剩 1 席时不给云盘（`giveable == 0`）；
/// - **云盘没货不硬凑**：席位数受「窗口外同分云盘条数」上限约束。
///
/// ⚠ 不用「跨索引 doc id 新近」做 tie-break：两个索引的 doc id 各自独立编号、
/// 云盘索引恒小，那等价于「云盘优先」，不是这里拍板的策略。
/// 云盘组内的新近优先与连接轮转发生在 `order_remote_hits`（同一个索引内部）。
fn reserve_cloud_seats_in_cut_band(
    merged: &mut Vec<(GlobalSearchResultEntry, bool)>,
    limit: usize,
) {
    if limit == 0 || merged.len() <= limit {
        // 不会发生截断 ⇒ 不存在「被切掉的云盘」，不需要配额。
        return;
    }

    let cut = merged[limit - 1].0.score;
    let Some(band_start) = merged
        .iter()
        .position(|(entry, _from_cloud)| entry.score == cut)
    else {
        return;
    };
    let mut band_end = band_start;
    while band_end < merged.len() && merged[band_end].0.score == cut {
        band_end += 1;
    }

    // 稳定排序后同分带内本地恒在云盘之前 ⇒ 这两个下标列表天然分段。
    let band_local: Vec<usize> = (band_start..band_end)
        .filter(|index| !merged[*index].1)
        .collect();
    let band_cloud: Vec<usize> = (band_start..band_end)
        .filter(|index| merged[*index].1)
        .collect();

    let window_band = limit - band_start;
    let locals_in_window = band_local.len().min(window_band);
    let clouds_in_window = window_band - locals_in_window;
    let clouds_outside = band_cloud.len() - clouds_in_window;
    if clouds_outside == 0 {
        return;
    }

    // 至少给本地留 1 席（本地优先）；云盘的目标是窗口的一半。
    let giveable = locals_in_window.saturating_sub(1);
    let desired = (limit / 2).saturating_sub(clouds_in_window);
    let seats = desired.min(giveable).min(clouds_outside);
    if seats == 0 {
        return;
    }

    let keep_local = locals_in_window - seats;
    let keep_cloud = window_band - keep_local;
    let mut band: Vec<(GlobalSearchResultEntry, bool)> =
        Vec::with_capacity(band_end - band_start);
    // 窗口内：本地前 keep_local 条 + 云盘前 keep_cloud 条。
    // 云盘池已按新近 + 连接轮转排好（`order_remote_hits`）⇒ 这里取到的是最新的那批。
    band.extend(
        band_local
            .iter()
            .take(keep_local)
            .map(|index| merged[*index].clone()),
    );
    band.extend(
        band_cloud
            .iter()
            .take(keep_cloud)
            .map(|index| merged[*index].clone()),
    );
    // 窗口外：让出席位的本地 + 未入选的云盘。同分，顺序不改变分数语义。
    band.extend(
        band_local
            .iter()
            .skip(keep_local)
            .map(|index| merged[*index].clone()),
    );
    band.extend(
        band_cloud
            .iter()
            .skip(keep_cloud)
            .map(|index| merged[*index].clone()),
    );

    merged.splice(band_start..band_end, band);
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

/// 云盘侧的候选预算。
///
/// **独立于本地**：本地的候选预算按「盘符数 × limit」算（见 `candidate_limit_for`），
/// 云盘没有盘符概念，给它自己的 `limit` 就够 —— 两个预算互不影响，
/// 本地的候选条数不会因为多了一份云盘索引而变大。
fn remote_candidate_limit_for(options: &GlobalSearchQueryOptions) -> usize {
    options.limit.max(1).min(MAX_SEARCH_CANDIDATE_MATERIALIZATION)
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
    // reader 的「常驻体积预算」：tantivy 没有内存旋钮，
    // 只能用「索引越大越早释放」来近似（见 state.rs 的 READER_RESIDENT_BUDGET_BYTES）。
    let index_size_bytes = {
        let state = GLOBAL_SEARCH_STATE
            .read()
            .map_err(|error| error.to_string())?;
        state.status.index_size_bytes
    };

    if !is_reader_idle(now_millis(), index_size_bytes) {
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

    // FORK-MODIFICATION 批次 55：云盘路径不是本地文件系统路径。
    //
    // 旧行为：`root.is_dir()` 对 `sfm://remote/<conn>/...` 恒为 false，
    // 于是直接 `Ok(Vec::new())` —— 调用方（MCP 的 search_files）会把
    // 「这个通道不支持云盘」误读成「这个目录里没有匹配的文件」，
    // 拿到一个**看起来正常、实际是假**的空结果。
    //
    // 现在改成明确报错，把「不支持」与「没有匹配」区分开。
    // 修的是「静默给错答案」，不是「给云盘加递归检索」——
    // 后者是新功能（走索引还是走实时遍历、每次按键烧多少 API 配额），
    // 取舍待定，见台账 W-18。
    if crate::remote::is_remote_path(&root_path) {
        return Err(format!(
            "云盘目录暂不支持递归检索（{root_path}）：该通道只遍历本地磁盘。\
             云盘文件请省略目录参数走全局索引检索，或用 list_directory 逐层列举。"
        ));
    }

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
    // FORK-MODIFICATION: 2026-11-25 —— W-12「中文子串检索失效」的回归测试。
    //
    // 现象（docs/open-items-2026-11-24.md）：`name` 字段走 SimpleTokenizer，一整段
    // 连续中文只产出一个 term，而 FuzzyTermQuery 是**整词**比对 ⇒ 查询「技巧」
    // 取不到 term「2024年用户调研技巧汇总」，实测 MCP 只返 7 条、`收集` 返 0 条。
    //
    // 钉两件事，缺一不可：
    //   ① `build_query` 真能取到子串候选（RegexQuery 兜底生效）；
    //   ② 候选的相似度仍过得了长度门槛 —— 否则取到了也会被
    //      `materialize_candidates` 二次过滤，等于没修。
    mod cjk_substring {
        use super::*;

        /// 与用户实测同形的夹具：CJK 连续串、中间没有分隔符。
        const SAMPLE_NAMES: [&str; 4] = [
            "2024年用户调研技巧汇总.docx",
            "收集整理项目进度.xlsx",
            "项目计划表.xlsx",
            "meeting-notes.md",
        ];

        fn build_fixture_index() -> (tantivy::Index, GlobalSearchIndexFields) {
            let (schema, fields) = build_schema();
            let index = tantivy::Index::create_in_ram(schema);
            let mut writer = index.writer(50_000_000).unwrap();

            for name in SAMPLE_NAMES {
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
            (index, fields)
        }

        /// 走真实的 `build_query` 取候选；不叠 `materialize_candidates` 的过滤，
        /// 这样断言的就是「取不取得到」这一层。
        fn candidate_names(query_text: &str) -> Vec<String> {
            let (index, fields) = build_fixture_index();
            let reader = index.reader().unwrap();
            let searcher = reader.searcher();
            let query = build_query(&fields, query_text, &create_options(false));

            searcher
                .search(&query, &TopDocs::with_limit(50).order_by_score())
                .unwrap()
                .into_iter()
                .filter_map(|(_score, address)| {
                    let document: tantivy::TantivyDocument = searcher.doc(address).ok()?;
                    document
                        .get_first(fields.name)
                        .and_then(|value| value.as_str())
                        .map(|value| value.to_string())
                })
                .collect()
        }

        #[test]
        fn finds_substring_in_the_middle_of_a_cjk_run() {
            let names = candidate_names("技巧");

            assert!(
                names.contains(&"2024年用户调研技巧汇总.docx".to_string()),
                "CJK 中段子串取不到候选，names={names:?}"
            );
            assert!(
                !names.iter().any(|name| name == "项目计划表.xlsx"),
                "无关名字不应被取到，names={names:?}"
            );
        }

        #[test]
        fn finds_substring_at_the_start_of_a_cjk_run() {
            // 台账里 `收集` 实测返 0 条：fuzzy 是整词比对，隔着 6 个字取不到。
            let names = candidate_names("收集");

            assert!(
                names.contains(&"收集整理项目进度.xlsx".to_string()),
                "CJK 开头子串取不到候选，names={names:?}"
            );
        }

        #[test]
        fn finds_single_han_character() {
            let names = candidate_names("研");

            assert!(
                names.contains(&"2024年用户调研技巧汇总.docx".to_string()),
                "单字中文取不到候选，names={names:?}"
            );
        }

        #[test]
        fn substring_hits_clear_the_length_threshold() {
            // 取到候选还不够 —— 相似度必须过门槛。门槛按**字节**长度取，
            // 与 `materialize_candidates` 里 `normalized_query.len()` 一致。
            for (query_text, name) in [
                ("技巧", "2024年用户调研技巧汇总.docx"),
                ("收集", "收集整理项目进度.xlsx"),
                ("研", "2024年用户调研技巧汇总.docx"),
            ] {
                let score = calculate_similarity_score(query_text, name);
                let threshold = get_min_score_for_query_length(query_text.len());

                assert!(
                    score >= threshold,
                    "查询「{query_text}」对「{name}」相似度 {score} 低于门槛 {threshold}"
                );
            }
        }

        #[test]
        fn substring_query_is_built_only_for_non_ascii_words() {
            let (_schema, fields) = build_schema();

            assert!(build_substring_query(&fields, "report").is_none());
            assert!(build_substring_query(&fields, "meeting-notes").is_none());
            assert!(build_substring_query(&fields, "技巧").is_some());
            assert!(build_substring_query(&fields, "研").is_some());
        }

        #[test]
        fn regex_metacharacters_in_the_word_are_escaped() {
            // 名字里带 `.` `*` 时不能把正则语义改掉，否则命中范围会跑偏。
            assert_eq!(escape_regex_literal("a.b*c"), r"a\.b\*c");
        }
    }

    // FORK-MODIFICATION: 2026-11-25 —— W-13「CJK 子串命中在候选池饱和时被挤出」的回归测试。
    //
    // 症状（台账 docs/open-items-2026-11-24.md §4 末行）：查「分析」这类高频 2 字词时，
    // 候选池打满，**只靠子串命中**的文档一条都搜不到（改用 5 字的「关键词分析」就稳定命中）。
    //
    // 根因：主池按 BM25 取前 N 条 ⇒ 只命中子串正则（常量分）的文档排在后面，
    // 池子一填满就在**候选阶段**被切掉，后面再怎么按相似度排序也救不回来。
    //
    // 修复：另开一条「子串候选池」，把「名字里含查询词」的文档整批捞进来再合并。
    //
    // 夹具的关键设计（否则测试会假绿）：
    //   · 12 个填充文档名含**独立成词**的「分析」⇒ fuzzy 分支命中，分数更高；
    //   · 靶子名为「深度学习模型分析报告」⇒ 与「分析」的编辑距离为 6，
    //     **fuzzy 分支匹配不上**，只有子串正则能命中；
    //   · 填充文档**先写**、靶子**后写** ⇒ 同分时 tantivy 按文档序排，靶子天然在末尾。
    // ⇒ 主池（limit=5）必然挤掉靶子；子串池必然捞得到靶子。
    mod w13_substring_pool {
        use super::*;

        const FILLER_COUNT: usize = 12;
        const TARGET_NAME: &str = "深度学习模型分析报告.mp4";
        const TARGET_TOKEN: &str = "深度学习模型分析报告";

        fn build_saturated_fixture() -> (tantivy::Index, GlobalSearchIndexFields) {
            let (schema, fields) = build_schema();
            let index = tantivy::Index::create_in_ram(schema);
            let mut writer = index.writer(50_000_000).unwrap();

            for filler_index in 0..FILLER_COUNT {
                let name = format!("分析 {filler_index:02}.docx");
                writer
                    .add_document(doc!(
                        fields.path => format!("C:/test/{name}"),
                        fields.name => name.clone(),
                        fields.name_lower => name.to_lowercase(),
                        fields.is_file => 1u64,
                        fields.is_dir => 0u64,
                        fields.modified_time => 0u64,
                        fields.size => 0u64,
                    ))
                    .unwrap();
            }

            // 靶子**最后**写入：同分时按文档序 ⇒ 它天然排在池子末尾。
            writer
                .add_document(doc!(
                    fields.path => format!("C:/test/{TARGET_NAME}"),
                    fields.name => TARGET_NAME.to_string(),
                    fields.name_lower => TARGET_NAME.to_lowercase(),
                    fields.is_file => 1u64,
                    fields.is_dir => 0u64,
                    fields.modified_time => 0u64,
                    fields.size => 0u64,
                ))
                .unwrap();

            writer.commit().unwrap();

            (index, fields)
        }

        /// 靶子在索引里的文档地址（按整词 term 精确取，不依赖夹具里的写入顺序）。
        fn target_address(
            searcher: &tantivy::Searcher,
            fields: &GlobalSearchIndexFields,
        ) -> tantivy::DocAddress {
            address_of_token(searcher, fields, TARGET_TOKEN)
        }

        /// 任一填充文档（token「分析」）的文档地址。
        fn filler_address(
            searcher: &tantivy::Searcher,
            fields: &GlobalSearchIndexFields,
        ) -> tantivy::DocAddress {
            address_of_token(searcher, fields, "分析")
        }

        fn address_of_token(
            searcher: &tantivy::Searcher,
            fields: &GlobalSearchIndexFields,
            token: &str,
        ) -> tantivy::DocAddress {
            let term = Term::from_field_text(fields.name, token);
            let query = TermQuery::new(term, IndexRecordOption::Basic);
            let hits = searcher
                .search(&query, &TopDocs::with_limit(1).order_by_score())
                .unwrap();

            hits.first()
                .map(|(_score, address)| *address)
                .unwrap_or_else(|| panic!("夹具里找不到 token 对应的文档：{token}"))
        }

        fn pool_contains(pool: &[tantivy::DocAddress], address: tantivy::DocAddress) -> bool {
            pool.iter().any(|candidate| {
                candidate.segment_ord == address.segment_ord && candidate.doc_id == address.doc_id
            })
        }

        #[test]
        fn saturated_primary_pool_drops_substring_only_target() {
            let (index, fields) = build_saturated_fixture();
            let reader = index.reader().unwrap();
            let searcher = reader.searcher();
            let options = create_options(false);
            let target = target_address(&searcher, &fields);

            let primary = collect_candidate_addresses(&searcher, &fields, "分析", &options, 5).unwrap();

            assert_eq!(primary.len(), 5, "夹具失效：主池没被填满");
            assert!(
                !pool_contains(&primary, target),
                "夹具失效：靶子不该靠主池进来（它与「分析」编辑距离 6，fuzzy 分支匹配不上）"
            );
        }

        #[test]
        fn substring_pool_recovers_target_and_merges_without_duplicates() {
            let (index, fields) = build_saturated_fixture();
            let reader = index.reader().unwrap();
            let searcher = reader.searcher();
            let options = create_options(false);
            let target = target_address(&searcher, &fields);

            let primary = collect_candidate_addresses(&searcher, &fields, "分析", &options, 5).unwrap();
            let (substring, matched) =
                collect_substring_candidate_addresses(&searcher, &fields, "分析", &options, 512, None)
                    .unwrap();

            assert_eq!(matched, FILLER_COUNT + 1, "子串命中总数应当是全部 13 条");
            assert!(
                pool_contains(&substring, target),
                "子串池必须捞到只靠子串命中的靶子"
            );

            let merged = merge_candidate_pools(primary, substring);
            assert!(pool_contains(&merged, target), "合并后靶子必须进池");
            assert_eq!(
                merged.len(),
                FILLER_COUNT + 1,
                "合并必须去重：主池 5 + 子串池 13 - 重叠 5 = 13"
            );
        }

        #[test]
        fn substring_only_document_reaches_final_results() {
            let (index, fields) = build_saturated_fixture();
            let reader = index.reader().unwrap();
            let searcher = reader.searcher();
            // limit 放大到 100：让「最终截断」不成为干扰项 ——
            // W-13 的病灶在**候选阶段**，不在这一层的截断。
            let mut options = create_options(false);
            options.limit = 100;

            let primary = collect_candidate_addresses(&searcher, &fields, "分析", &options, 5).unwrap();
            let (substring, _matched) =
                collect_substring_candidate_addresses(&searcher, &fields, "分析", &options, 512, None)
                    .unwrap();
            let candidates = merge_candidate_pools(primary, substring);

            let (results, _materialized) =
                materialize_candidates(&searcher, &fields, &options, "分析", &candidates);
            let finalized = finalize_results(results, options.limit);

            assert!(
                finalized.iter().any(|entry| entry.name == TARGET_NAME),
                "只靠子串命中的文档必须出现在最终结果里"
            );
        }

        #[test]
        fn ascii_query_produces_no_substring_channel() {
            let (_schema, fields) = build_schema();
            let options = create_options(false);

            assert!(
                build_substring_only_query(&fields, "midjourney", &options).is_none(),
                "纯 ASCII 查询不该多跑一次子串检索（行为必须与改动前一致）"
            );
            assert!(build_substring_only_query(&fields, "分析", &options).is_some());
        }

        #[test]
        fn exact_match_mode_produces_no_substring_channel() {
            let (_schema, fields) = build_schema();
            let options = create_options(true);

            assert!(
                build_substring_only_query(&fields, "分析", &options).is_none(),
                "精确匹配模式下子串命中会被 matches_exact_name 筛掉，不该多跑一次检索"
            );
        }

        #[test]
        fn merge_candidate_pools_dedupes_and_keeps_primary_order() {
            let (index, fields) = build_saturated_fixture();
            let reader = index.reader().unwrap();
            let searcher = reader.searcher();

            let filler = filler_address(&searcher, &fields);
            let target = target_address(&searcher, &fields);

            let merged = merge_candidate_pools(vec![filler], vec![filler, target]);
            let keys: Vec<(u32, u32)> = merged
                .iter()
                .map(|address| (address.segment_ord, address.doc_id))
                .collect();
            let filler_key = (filler.segment_ord, filler.doc_id);
            let target_key = (target.segment_ord, target.doc_id);

            assert_eq!(keys, vec![filler_key, target_key], "重复项必须去掉，主池顺序必须保留");
        }

        #[test]
        fn substring_candidate_limit_scales_with_limit_and_is_bounded() {
            let mut options = create_options(false);

            options.limit = 10;
            assert_eq!(substring_candidate_limit_for(&options), 4000, "少于下限时抬到 4000");

            options.limit = 1000;
            assert_eq!(substring_candidate_limit_for(&options), 4000, "limit x 4 = 4000");

            options.limit = 2000;
            assert_eq!(substring_candidate_limit_for(&options), 8000, "正常值是 limit 的 4 倍");

            options.limit = 10000;
            assert_eq!(substring_candidate_limit_for(&options), 20000, "超量时压到硬顶 20000");
        }
    }

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

    // ---- 云盘（远程连接）结果合并（批次 37） ---------------------------------

    fn remote_entry(path: &str, name: &str, score: f32) -> GlobalSearchResultEntry {
        GlobalSearchResultEntry {
            name: name.to_string(),
            ext: None,
            path: path.to_string(),
            size: 0,
            item_count: None,
            modified_time: 0,
            accessed_time: 0,
            created_time: 0,
            mime: None,
            is_file: true,
            is_dir: false,
            is_symlink: false,
            is_hidden: false,
            score,
        }
    }

    fn cloud_options(limit: usize) -> GlobalSearchQueryOptions {
        GlobalSearchQueryOptions {
            limit,
            include_files: true,
            include_directories: true,
            exact_match: false,
            typo_tolerance: true,
            min_score_threshold: Some(0.0),
        }
    }

    /// 合并的三条规则：按 path 去重、按 score 降序、总量受 limit 约束。
    #[test]
    fn merging_dedupes_by_path_and_orders_by_score() {
        let local = vec![
            remote_entry("C:/local/a.txt", "a.txt", 0.4),
            remote_entry("C:/local/b.txt", "b.txt", 0.9),
        ];
        let remote = vec![
            remote_entry("sfm://remote/conn-a/c.txt", "c.txt", 0.7),
            // 同一条目在两边都出现 ⇒ 只保留一条
            remote_entry("C:/local/a.txt", "a.txt", 0.1),
        ];

        let merged = merge_search_results(local, remote, 10);

        assert_eq!(merged.len(), 3, "同一个 path 必须只留一条");
        assert_eq!(merged[0].path, "C:/local/b.txt");
        assert_eq!(merged[1].path, "sfm://remote/conn-a/c.txt");
        assert_eq!(merged[2].path, "C:/local/a.txt");
    }

    /// 同一条目在两边都出现、且**分数相同**时，保留本地那条
    /// （本地先入队 + 严格大于才替换）。
    #[test]
    fn merging_keeps_the_local_entry_when_the_scores_tie() {
        let local = vec![remote_entry("C:/local/a.txt", "本地名字.txt", 0.5)];
        let remote = vec![remote_entry("C:/local/a.txt", "云盘名字.txt", 0.5)];

        let merged = merge_search_results(local, remote, 10);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "本地名字.txt");
    }

    /// 合并后的总量仍然受 `limit` 约束。
    #[test]
    fn merging_respects_the_limit() {
        let local = vec![
            remote_entry("C:/local/a.txt", "a.txt", 0.9),
            remote_entry("C:/local/b.txt", "b.txt", 0.8),
        ];
        let remote = vec![remote_entry("sfm://remote/conn-a/c.txt", "c.txt", 0.7)];

        let merged = merge_search_results(local, remote, 2);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].path, "C:/local/a.txt");
        assert_eq!(merged[1].path, "C:/local/b.txt");
    }

    /// **没有云盘索引时行为与改动前完全一致** —— 返回 `None`，
    /// 调用方据此原样返回本地结果（不报错、不改结果集）。
    #[test]
    fn a_missing_remote_index_yields_no_results_at_all() {
        let temp = tempfile::TempDir::new().expect("temp dir");

        assert!(search_remote_index(temp.path(), "readme", &cloud_options(10)).is_none());
    }

    /// 云盘索引的写入 → 查询往返。
    #[test]
    fn the_remote_index_round_trips_through_a_write_and_a_query() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let base_dir = temp.path().to_path_buf();

        // 勾选集合里必须有 conn-a —— 查询侧按**当前**勾选集合过滤。
        super::super::remote_index::write_test_connections(
            &super::super::remote_index::user_data_dir_in(&base_dir),
            &[("conn-a", true)],
        );

        let written = super::super::remote_index::write_remote_index(
            &remote_index_dir(&base_dir),
            &[super::super::remote_index::RemoteIndexDoc {
                path: "sfm://remote/conn-a/readme.txt".to_string(),
                name: "readme.txt".to_string(),
                is_dir: false,
                size: 12,
                modified_time: 0,
                connection_id: "conn-a".to_string(),
            }],
        )
        .expect("write");

        assert_eq!(written, 1);

        let results = search_remote_index(&base_dir, "readme", &cloud_options(10))
            .expect("云盘索引必须存在");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "sfm://remote/conn-a/readme.txt");
        assert_eq!(results[0].name, "readme.txt");
    }

    /// **取消勾选 = 立刻搜不到**（台账 O-1 的验收标准）。
    ///
    /// 索引里同时有两条连接的条目，但只有 `conn-a` 还勾着 ⇒
    /// `conn-b` 的那条**一条都搜不到**。
    ///
    /// 这条正是「重建中途失败 ⇒ 旧索引还在」那个场景的正解：
    /// 过滤发生在**索引层**，不依赖「下次重建顺手清掉」。
    #[test]
    fn an_unchecked_connection_is_filtered_out_at_the_index_level() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let base_dir = temp.path().to_path_buf();

        super::super::remote_index::write_test_connections(
            &super::super::remote_index::user_data_dir_in(&base_dir),
            &[("conn-a", true), ("conn-b", false)],
        );

        super::super::remote_index::write_remote_index(
            &remote_index_dir(&base_dir),
            &[
                super::super::remote_index::RemoteIndexDoc {
                    path: "sfm://remote/conn-a/report.pdf".to_string(),
                    name: "report.pdf".to_string(),
                    is_dir: false,
                    size: 12,
                    modified_time: 0,
                    connection_id: "conn-a".to_string(),
                },
                super::super::remote_index::RemoteIndexDoc {
                    path: "sfm://remote/conn-b/report.pdf".to_string(),
                    name: "report.pdf".to_string(),
                    is_dir: false,
                    size: 12,
                    modified_time: 0,
                    connection_id: "conn-b".to_string(),
                },
            ],
        )
        .expect("write");

        let results = search_remote_index(&base_dir, "report", &cloud_options(10))
            .expect("云盘索引必须存在");

        assert_eq!(results.len(), 1, "没勾选的连接必须一条都搜不到");
        assert_eq!(results[0].path, "sfm://remote/conn-a/report.pdf");
    }

    /// **一个网盘都没勾** ⇒ 云盘一条都不出现。
    ///
    /// 注意这返回的是 `Some(空)` 而不是 `None`：
    /// `None` 的含义是「没有云盘索引」，`Some(空)` 是「有索引，但当前没有可见的连接」。
    #[test]
    fn no_checked_connection_means_no_remote_results() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let base_dir = temp.path().to_path_buf();

        super::super::remote_index::write_test_connections(
            &super::super::remote_index::user_data_dir_in(&base_dir),
            &[("conn-a", false)],
        );

        super::super::remote_index::write_remote_index(
            &remote_index_dir(&base_dir),
            &[super::super::remote_index::RemoteIndexDoc {
                path: "sfm://remote/conn-a/report.pdf".to_string(),
                name: "report.pdf".to_string(),
                is_dir: false,
                size: 12,
                modified_time: 0,
                connection_id: "conn-a".to_string(),
            }],
        )
        .expect("write");

        let results = search_remote_index(&base_dir, "report", &cloud_options(10))
            .expect("云盘索引存在");

        assert!(results.is_empty());
    }

    /// **两条连接都勾着** ⇒ 两条都在结果里（过滤不能误伤）。
    #[test]
    fn every_checked_connection_stays_searchable() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let base_dir = temp.path().to_path_buf();

        super::super::remote_index::write_test_connections(
            &super::super::remote_index::user_data_dir_in(&base_dir),
            &[("conn-a", true), ("conn-b", true)],
        );

        super::super::remote_index::write_remote_index(
            &remote_index_dir(&base_dir),
            &[
                super::super::remote_index::RemoteIndexDoc {
                    path: "sfm://remote/conn-a/report.pdf".to_string(),
                    name: "report.pdf".to_string(),
                    is_dir: false,
                    size: 12,
                    modified_time: 0,
                    connection_id: "conn-a".to_string(),
                },
                super::super::remote_index::RemoteIndexDoc {
                    path: "sfm://remote/conn-b/report.pdf".to_string(),
                    name: "report.pdf".to_string(),
                    is_dir: false,
                    size: 12,
                    modified_time: 0,
                    connection_id: "conn-b".to_string(),
                },
            ],
        )
        .expect("write");

        let results = search_remote_index(&base_dir, "report", &cloud_options(10))
            .expect("云盘索引必须存在");

        assert_eq!(results.len(), 2);
    }

    // ---- W-14（批次 52）：同分命中按来源保底配额 ------------------------------
    //
    // 靶子（真机实测）：阿里云盘 `Midjourney 使用心得+关键词分析.mp4`，查「分析」时
    // 307 条云盘命中一条都进不了结果窗口。
    //
    // 机制（读码坐实，**不是**上一轮记的「子串池互斥门」）：
    // `query.rs:724` 的稳定排序让同分时本地恒在云盘之前，`:730` 才 `truncate(limit)`；
    // 而本地侧 `finalize_results` 单就能产出 `limit × 盘符数` 条候选 ⇒ 0.92 这条长同分带
    // 被本地填满，云盘整批落在窗口之外。批次 50 已让 307 条全部入池，丢在合并层。
    //
    // 用户拍板的修法 = 候选②「同分命中按来源保底配额」，且**必须同时覆盖两处**：
    //   (A) `merge_search_results`：同分带内给云盘保底席位；
    //   (B) `search_remote_index`：云盘组内同分按 mtime 新近优先 ——
    //       否则配额席位会被「最早写入索引的连接」（百度）吃光，靶子（阿里，最后写入）依旧不出现。
    /// 批次 53 B：**云盘同分带内必须按连接轮转席位**，否则一个连接霸榜会把其余连接挤光。
    ///
    /// 批次 52 把云盘池的并列序改成「一律按 `modified_time` 降序」，装机版当场证伪其充分性：
    /// 用户真实索引里搜『分析』，云盘命中 101 条**全部来自同一个连接** `0f0696ba`，
    /// 靶子所在的 `646dacdb` 在 200 与 500 两个窗口内**都是 0 条**。
    /// 根因是「按新近」在单连接海量新文件面前等于「按连接独占」。取证见
    /// `docs/open-items-2026-11-24.md` W-14 与 `.superpowers/sdd/b52/probe.out.md`。
    ///
    /// 这里的夹具按真机形状搭：A 连接一屋子名字相同的新文件，B 连接只有靶子这一条旧文件。
    mod w15_connection_fairness {
        use super::*;

        /// 只用 `remote_entry` 造条目：`connection_id` **不是索引字段**，连接身份由
        /// `sfm://remote/<id>/...` 的 path 携带，批次 53 不改数据模型。
        fn cloud(path: &str, name: &str, score: f32, modified_time: u64) -> GlobalSearchResultEntry {
            GlobalSearchResultEntry {
                modified_time,
                ..remote_entry(path, name, score)
            }
        }

        fn conn_of(entry: &GlobalSearchResultEntry) -> Option<&str> {
            entry.path.split('/').nth(3)
        }

        fn band(count: usize, base: u64, score: f32) -> Vec<GlobalSearchResultEntry> {
            (0..count)
                .map(|i| {
                    cloud(
                        &format!("sfm://remote/conn-flat/dir{i:03}/报告{i}.mp4"),
                        &format!("报告{i}.mp4"),
                        score,
                        base - i as u64,
                    )
                })
                .collect()
        }

        /// 判别性测试：真机那条靶子在修复前落在 21 条云盘命中的**最后一条**。
        #[test]
        fn a_stale_connection_must_get_seats_in_a_saturated_same_score_band() {
            let mut hits: Vec<GlobalSearchResultEntry> = (0..20)
                .map(|i| {
                    cloud(
                        &format!("sfm://remote/conn-new/f{i:02}/分析报告.mp4"),
                        "分析报告.mp4",
                        0.92,
                        9_000 - i as u64,
                    )
                })
                .collect();
            hits.push(cloud(
                "sfm://remote/conn-old/08艺术/Midjourney 使用心得+关键词分析.mp4",
                "Midjourney 使用心得+关键词分析.mp4",
                0.92,
                10,
            ));
            order_remote_hits(&mut hits);

            assert_eq!(hits.len(), 21, "一个条目都不许丢");
            let target = hits
                .iter()
                .position(|e| e.name.starts_with("Midjourney"))
                .expect("靶子必须还在窗口里");
            // 修复前靶子排最后（下标 20）；按连接轮转后它必须拿到第二个席位。
            assert!(
                target <= 1,
                "旧连接的靶子必须进前两名，实际下标 = {target}，窗口 = {:?}",
                hits.iter().map(|e| conn_of(e).unwrap_or("?")).collect::<Vec<_>>()
            );
            assert_eq!(conn_of(&hits[1]).unwrap(), "conn-old", "第二个席位必须给旧连接");
        }

        /// 轮转必须是**逐轮交替**，而不是「每个连接先放一条再各自排完」。
        #[test]
        fn seats_rotate_round_robin_across_connections() {
            let mk = |conn: &str, latest: u64| {
                (0..3)
                    .map(|i| {
                        cloud(
                            &format!("sfm://remote/{conn}/d{i}/报告{i}.mp4"),
                            &format!("{conn}-报告{i}.mp4"),
                            0.92,
                            latest - i as u64,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let mut hits = Vec::new();
            hits.extend(mk("conn-old", 1_000));
            hits.extend(mk("conn-mid", 5_000));
            hits.extend(mk("conn-new", 9_000));
            order_remote_hits(&mut hits);

            let conns: Vec<&str> = hits.iter().map(|e| conn_of(e).unwrap()).collect();
            assert_eq!(
                conns,
                vec![
                    "conn-new",
                    "conn-mid",
                    "conn-old",
                    "conn-new",
                    "conn-mid",
                    "conn-old",
                    "conn-new",
                    "conn-mid",
                    "conn-old",
                ],
                "三个连接必须各占每轮的同一位置：{conns:?}"
            );
            // 组序按该组最新修改时间降序 ⇒ 新连接仍打头，「更近的先看」这条不回退。
            assert_eq!(hits[0].modified_time, 9_000, "新连接的更新一条必须仍排第一");
        }

        /// 铁律 1：轮转不得碰比卡窗口那批更高分的结果。
        #[test]
        fn rotation_never_touches_a_higher_score_band() {
            let mut hits = vec![
                cloud("sfm://remote/conn-a/a/x1.mp4", "x1.mp4", 0.92, 9),
                cloud("sfm://remote/conn-b/b/x2.mp4", "x2.mp4", 0.92, 8),
                cloud("sfm://remote/conn-a/a/x3.mp4", "x3.mp4", 0.92, 7),
                cloud("sfm://remote/conn-b/b/x4.mp4", "x4.mp4", 0.92, 6),
                cloud("sfm://remote/conn-b/b/hot.mp4", "hot.mp4", 0.99, 1),
                cloud("sfm://remote/conn-a/a/top.mp4", "top.mp4", 0.99, 2),
            ];
            order_remote_hits(&mut hits);

            assert_eq!(hits.len(), 6);
            assert_eq!(hits[0].name, "top.mp4", "高分带内部仍按新近，0.99 的两条不许散到后面");
            assert_eq!(hits[1].name, "hot.mp4");
            let conns: Vec<&str> = hits[2..].iter().map(|e| conn_of(e).unwrap()).collect();
            assert_eq!(conns, vec!["conn-a", "conn-b", "conn-a", "conn-b"]);
        }

        /// 没有连接身份（非 `sfm://remote/` 形态）的条目必须退回批次 52 的纯新近序，
        /// 且绝不能被误塞进「未知连接」这一组去参与轮转。
        #[test]
        fn entries_without_a_connection_keep_the_plain_recency_order() {
            let mut hits: Vec<GlobalSearchResultEntry> = (0..5)
                .map(|i| cloud(&format!("C:/local/f{i}.txt"), &format!("f{i}.txt"), 0.92, 500 - i as u64))
                .collect();
            hits.reverse();
            order_remote_hits(&mut hits);
            let times: Vec<u64> = hits.iter().map(|e| e.modified_time).collect();
            assert_eq!(times, vec![500, 499, 498, 497, 496]);
        }

        /// 分数各不相同 ⇒ 没有同分带 ⇒ 只按分数降序，轮转不参与。
        #[test]
        fn distinct_scores_stay_in_their_own_bands() {
            let mut hits = vec![
                cloud("sfm://remote/conn-c/c.txt", "c.txt", 0.5, 999),
                cloud("sfm://remote/conn-a/a.txt", "a.txt", 0.9, 1),
                cloud("sfm://remote/conn-b/b.txt", "b.txt", 0.7, 500),
            ];
            order_remote_hits(&mut hits);
            let names: Vec<&str> = hits.iter().map(|e| e.name.as_str()).collect();
            assert_eq!(names, vec!["a.txt", "b.txt", "c.txt"]);
        }

        /// 单连接 + 同分 = 批次 52 的行为，必须**逐条不变**（新规则不能改成洗牌）。
        #[test]
        fn a_single_connection_band_is_left_in_recency_order() {
            let mut hits = band(6, 600, 0.92);
            hits.reverse();
            order_remote_hits(&mut hits);
            let times: Vec<u64> = hits.iter().map(|e| e.modified_time).collect();
            assert_eq!(times, vec![600, 599, 598, 597, 596, 595]);
        }

        /// 铁律 2 的规模版：轮转只重排，条数与集合都必须守恒。
        #[test]
        fn rotation_conservess_every_entry() {
            let mut hits: Vec<GlobalSearchResultEntry> = (0..50)
                .map(|i| {
                    cloud(
                        &format!("sfm://remote/conn-{}/f{}", i % 4, i),
                        &format!("f{i}.txt"),
                        0.92,
                        1_000 - i as u64,
                    )
                })
                .collect();
            let before: Vec<String> = hits.iter().map(|e| e.path.clone()).collect();
            order_remote_hits(&mut hits);
            let after: Vec<String> = hits.iter().map(|e| e.path.clone()).collect();
            assert_eq!(after.len(), before.len());
            let mut sorted_before = before.clone();
            let mut sorted_after = after.clone();
            sorted_before.sort();
            sorted_after.sort();
            assert_eq!(sorted_before, sorted_after, "轮转不得增删条目");
            assert_eq!(after.len(), 50);
        }
    }

    mod w14_cloud_band_quota {
        use super::*;

        /// 云盘路径前缀（`location.rs` 的 `sfm://remote/{connection_id}{path}`）。
        const CLOUD_PREFIX: &str = "sfm://remote/";

        /// 带 mtime 的云盘条目 —— `remote_entry` 的 `modified_time` 恒为 0，测「同分按新近」不够用。
        fn cloud_entry(
            path: &str,
            name: &str,
            score: f32,
            modified_time: u64,
        ) -> GlobalSearchResultEntry {
            GlobalSearchResultEntry {
                modified_time,
                ..remote_entry(path, name, score)
            }
        }

        /// 长同分本地带：模拟真机「5,200 条本地命中全是 0.92」。
        fn local_band(count: usize, score: f32) -> Vec<GlobalSearchResultEntry> {
            (0..count)
                .map(|i| {
                    remote_entry(
                        &format!("C:/local/f{i:04}.txt"),
                        &format!("f{i:04}.txt"),
                        score,
                    )
                })
                .collect()
        }

        fn cloud_count(entries: &[GlobalSearchResultEntry]) -> usize {
            entries
                .iter()
                .filter(|entry| entry.path.starts_with(CLOUD_PREFIX))
                .count()
        }

        /// (A) 根因钉死：长同分本地带 ⇒ 云盘一条都进不了窗口。
        ///
        /// 修复前 `cloud_count == 0`（本条 RED）；修复后云盘必须拿到保底席位，
        /// 且拿到的是**最新**的那条（(B) 生效的证据）。
        #[test]
        fn a_long_same_score_local_band_must_still_leave_room_for_the_cloud() {
            let remote = vec![
                cloud_entry("sfm://remote/conn-baidu/old.docx", "old.docx", 0.92, 100),
                cloud_entry("sfm://remote/conn-ali/mid.docx", "mid.docx", 0.92, 200),
                cloud_entry(
                    "sfm://remote/conn-ali/Midjourney 使用心得+关键词分析.mp4",
                    "Midjourney 使用心得+关键词分析.mp4",
                    0.92,
                    300,
                ),
            ];

            let merged = merge_search_results(local_band(10, 0.92), remote, 6);

            assert_eq!(merged.len(), 6, "总量仍受 limit 约束");
            assert!(
                cloud_count(&merged) >= 1,
                "同分带内云盘必须有保底席位，实际云盘 {} 条：{:?}",
                cloud_count(&merged),
                merged.iter().map(|e| e.name.as_str()).collect::<Vec<_>>()
            );
            assert!(
                merged
                    .iter()
                    .any(|entry| entry.name == "Midjourney 使用心得+关键词分析.mp4"),
                "保底席位必须给到 mtime 最新的云盘条目（靶子），实际 {:?}",
                merged.iter().map(|e| e.name.as_str()).collect::<Vec<_>>()
            );
        }

        /// (A) 配额只吃**同分**的本地席位：分数高于 cut 的本地条目一条都不能被挤掉。
        #[test]
        fn the_quota_never_evicts_a_local_hit_that_scores_above_the_cut() {
            let mut local = vec![
                remote_entry("C:/local/top.txt", "top.txt", 1.0),
                remote_entry("C:/local/second.txt", "second.txt", 0.95),
            ];
            local.extend(local_band(8, 0.92));
            let remote = vec![
                cloud_entry("sfm://remote/conn-ali/c1.txt", "c1.txt", 0.92, 100),
                cloud_entry("sfm://remote/conn-ali/c2.txt", "c2.txt", 0.92, 200),
            ];

            let merged = merge_search_results(local, remote, 10);

            assert_eq!(merged.len(), 10);
            assert_eq!(merged[0].path, "C:/local/top.txt", "1.0 的本地条目必须留在首位");
            assert_eq!(merged[1].path, "C:/local/second.txt", "0.95 的本地条目不得被挤掉");
            assert_eq!(cloud_count(&merged), 2, "同分带内云盘应拿到配额席位");
        }

        /// (A) 同分带只剩 1 个席位时留给本地 —— 保住「本地结果优先」这个一贯取向。
        #[test]
        fn a_single_seat_cut_band_stays_local() {
            let local = vec![remote_entry("C:/local/a.txt", "a.txt", 0.92)];
            let remote = vec![cloud_entry(
                "sfm://remote/conn-ali/b.txt",
                "b.txt",
                0.92,
                999,
            )];

            let merged = merge_search_results(local, remote, 1);

            assert_eq!(merged.len(), 1);
            assert_eq!(merged[0].path, "C:/local/a.txt");
        }

        /// (A) 没有云盘命中时，合并结果与改动前逐条一致（配额不得凭空造条目）。
        #[test]
        fn an_empty_cloud_pool_changes_nothing() {
            let merged = merge_search_results(local_band(5, 0.92), Vec::new(), 3);

            assert_eq!(merged.len(), 3);
            assert_eq!(cloud_count(&merged), 0);
            assert_eq!(merged[0].path, "C:/local/f0000.txt");
        }

        /// (B) 云盘候选池内部：同分必须按 mtime 新近优先。
        ///
        /// 真机写入顺序是 7z → 百度 → 阿里，靶子在**最后写入**的连接里；
        /// 若池内仍按文档序，配额席位会被百度吃光。
        #[test]
        fn the_cloud_pool_orders_same_score_hits_newest_first() {
            use super::super::super::remote_index::{self, RemoteIndexDoc};

            let temp = tempfile::TempDir::new().expect("temp dir");
            let base_dir = temp.path().to_path_buf();
            remote_index::write_test_connections(
                &remote_index::user_data_dir_in(&base_dir),
                &[("conn-baidu", true), ("conn-ali", true)],
            );
            remote_index::write_remote_index(
                &remote_index_dir(&base_dir),
                &[
                    RemoteIndexDoc {
                        path: "sfm://remote/conn-baidu/旧的分析报告.mp4".to_string(),
                        name: "旧的分析报告.mp4".to_string(),
                        is_dir: false,
                        size: 1,
                        modified_time: 1_000,
                        connection_id: "conn-baidu".to_string(),
                    },
                    RemoteIndexDoc {
                        path: "sfm://remote/conn-ali/新的分析报告.mp4".to_string(),
                        name: "新的分析报告.mp4".to_string(),
                        is_dir: false,
                        size: 1,
                        modified_time: 2_000,
                        connection_id: "conn-ali".to_string(),
                    },
                ],
            )
            .expect("write");

            let results = search_remote_index(&base_dir, "分析", &cloud_options(10))
                .expect("云盘索引必须存在");

            assert_eq!(results.len(), 2, "两条都该命中（子串通道）");
            assert_eq!(
                results[0].name, "新的分析报告.mp4",
                "云盘组内同分必须 mtime 新近优先，实际 {:?}",
                results
                    .iter()
                    .map(|entry| (entry.name.clone(), entry.score, entry.modified_time))
                    .collect::<Vec<_>>()
            );
        }

        /// 批次 53 B 的**真机同构**验收：两个连接、同名的云盘文件，一个连接 8 条新的、
        /// 另一个连接只有靶子这一条旧的。名字完全一致 ⇒ `name` 字段同分，正是卡窗口时
        /// 会被「一律按新近」整带让给单个连接的那种形状（真机 101 条同分命中全来自一个连接）。
        ///
        /// 修这条夹具时实测到一个重要事实：把新文件改名成 `分析报告.mp4` 后它拿 **0.96875**
        /// 而靶子名拿 **0.92** —— `分析` 在 `分析报告` 里是**词头**、在 `使用心得+关键词分析`
        /// 里是**词尾**，两者走的是不同打分通道，**不构成同分带**，因而轮转不适用。
        /// 所以夹具必须用同名文件才能复现真机的同分挤压。
        ///
        /// 修复前实测：靶子排在第 8 位（8 条新文件全压在前面）；按连接轮转后必须进前 2。
        #[test]
        fn a_stale_connection_keeping_only_the_target_still_gets_seats() {
            use super::super::super::remote_index::{self, RemoteIndexDoc};

            const TARGET_NAME: &str = "Midjourney 使用心得+关键词分析.mp4";

            let temp = tempfile::TempDir::new().expect("temp dir");
            let base_dir = temp.path().to_path_buf();
            let index_dir = remote_index_dir(&base_dir);
            let conns = remote_index::user_data_dir_in(&base_dir);
            remote_index::write_test_connections(&conns, &[("conn-fresh", true), ("conn-stale", true)]);

            let mut docs: Vec<RemoteIndexDoc> = (0..8)
                .map(|i| RemoteIndexDoc {
                    path: format!("sfm://remote/conn-fresh/f{i:02}/{TARGET_NAME}"),
                    name: TARGET_NAME.to_string(),
                    is_dir: false,
                    size: 10,
                    modified_time: 5_000 - i as u64,
                    connection_id: "conn-fresh".to_string(),
                })
                .collect();
            docs.push(RemoteIndexDoc {
                path: format!("sfm://remote/conn-stale/08艺术/{TARGET_NAME}"),
                name: TARGET_NAME.to_string(),
                is_dir: false,
                size: 11,
                modified_time: 10,
                connection_id: "conn-stale".to_string(),
            });
            remote_index::write_remote_index(&index_dir, &docs).expect("write");

            let results = search_remote_index(&base_dir, "分析", &cloud_options(10)).expect("云盘索引必须存在");
            assert_eq!(results.len(), 9, "一个都不许丢");
            let target = results
                .iter()
                .position(|e| e.path.contains("conn-stale"))
                .expect("靶子必须出现在窗口里");
            // 同分前提先自证：否则这条测试测不到轮转，只是测了分数带。
            let scores: Vec<f32> = results.iter().map(|e| e.score).collect();
            assert!(
                scores.windows(2).all(|w| (w[0] - w[1]).abs() < f32::EPSILON),
                "夹具必须落在同一个分数带上，实际 {scores:?}"
            );
            assert!(
                target <= 1,
                "只剩一条命中的旧连接必须拿到前排席位，实际下标 = {target}，窗口 = {:?}",
                results
                    .iter()
                    .map(|e| (e.path.split('/').nth(3).unwrap_or("?").to_string(), e.score, e.modified_time))
                    .collect::<Vec<_>>()
            );
        }

        /// (A)+(B) 合起来的验收：真实云盘索引 + 长同分本地带 ⇒ 靶子必须落在窗口内。
        #[test]
        fn the_target_from_the_last_written_connection_survives_a_long_local_band() {
            use super::super::super::remote_index::{self, RemoteIndexDoc};

            let temp = tempfile::TempDir::new().expect("temp dir");
            let base_dir = temp.path().to_path_buf();
            remote_index::write_test_connections(
                &remote_index::user_data_dir_in(&base_dir),
                &[("conn-baidu", true), ("conn-ali", true)],
            );
            remote_index::write_remote_index(
                &remote_index_dir(&base_dir),
                &[
                    RemoteIndexDoc {
                        path: "sfm://remote/conn-baidu/百度盘的分析素材.mp4".to_string(),
                        name: "百度盘的分析素材.mp4".to_string(),
                        is_dir: false,
                        size: 1,
                        modified_time: 1_000,
                        connection_id: "conn-baidu".to_string(),
                    },
                    RemoteIndexDoc {
                        path: "sfm://remote/conn-ali/Midjourney 使用心得+关键词分析.mp4".to_string(),
                        name: "Midjourney 使用心得+关键词分析.mp4".to_string(),
                        is_dir: false,
                        size: 1,
                        modified_time: 2_000,
                        connection_id: "conn-ali".to_string(),
                    },
                ],
            )
            .expect("write");

            let remote = search_remote_index(&base_dir, "分析", &cloud_options(50))
                .expect("云盘索引必须存在");
            assert_eq!(remote.len(), 2, "两条云盘命中都该入池（批次 50 已闭合召回）");

            let merged = merge_search_results(local_band(200, 0.92), remote, 6);

            assert!(
                merged
                    .iter()
                    .any(|entry| entry.name == "Midjourney 使用心得+关键词分析.mp4"),
                "靶子必须出现在结果窗口内，实际 {:?}",
                merged.iter().map(|e| e.name.as_str()).collect::<Vec<_>>()
            );
        }
    }
}

// FORK-MODIFICATION 批次 55：云盘目录作用域检索的「静默 0 条」守卫。
//
// `search_directory_recursive_blocking` 走的是本地 WalkDir，
// 对 `sfm://remote/<conn>/...` 这类云盘路径 `root.is_dir()` 恒为 false，
// 旧行为直接 `Ok(Vec::new())` —— 调用方（MCP 的 search_files）会把
// 「这个通道不支持云盘」误读成「这个目录里没有匹配的文件」。
// 批次 55 改成明确报错，并锁住「本地目录的行为一个字都没变」。
#[cfg(test)]
mod w18_remote_scope_guard {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn remote_directory_returns_explicit_error_instead_of_empty_result() {
        let error = search_directory_recursive_blocking(
            "sfm://remote/conn-baidu/my-drive".to_string(),
            "分析".to_string(),
            3,
            50,
            false,
        )
        .expect_err("云盘目录必须明确报错，而不是静默返回 0 条");

        assert!(
            error.contains("云盘") && error.contains("不支持"),
            "错误信息要说清「不支持」，而不是让人误读成「没有匹配」，实际：{error}"
        );
        assert!(
            error.contains("sfm://remote/conn-baidu/my-drive"),
            "错误信息必须带上原始路径，实际：{error}"
        );
    }

    #[test]
    fn remote_connection_root_also_returns_explicit_error() {
        let error = search_directory_recursive_blocking(
            "sfm://remote".to_string(),
            "分析".to_string(),
            3,
            50,
            false,
        )
        .expect_err("连接根目录也必须明确报错");

        assert!(error.contains("不支持"), "实际：{error}");
    }

    #[test]
    fn local_missing_directory_still_returns_empty_ok() {
        let result = search_directory_recursive_blocking(
            "Z:/__afm_b55_missing__".to_string(),
            "分析".to_string(),
            3,
            50,
            false,
        )
        .expect("本地不存在的目录仍然是「没有匹配」，不是错误");

        assert!(result.is_empty(), "本地缺失目录必须保持空结果");
    }

    /// 夹具根目录。
    ///
    /// 刻意**不用 `std::env::temp_dir()`**：它是 `%USERPROFILE%\AppData\Local\Temp`，
    /// 而 `builtin_ignored_paths()` 覆盖 `/AppData/Local/Temp`
    /// ⇒ `filter_entry` 会把整棵夹具剪掉，测试会假成「递归搜索完全失效」。
    /// 同一个坑本仓库已经栽过两次（见同文件 `recursive_scan_cap` 的夹具注释）。
    fn fixture_root(label: &str) -> std::path::PathBuf {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:/".to_string());
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();

        std::path::PathBuf::from(home)
            .join("afm-rust-search-fixtures")
            .join(format!("{label}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn local_directory_hit_is_unchanged() {
        let root = fixture_root("w18-local");
        let sub = root.join("sub");

        std::fs::create_dir_all(&sub).expect("create fixture dir");
        std::fs::write(sub.join("分析报告.txt"), b"x").expect("write fixture file");

        let result = search_directory_recursive_blocking(
            root.to_string_lossy().replace('\\', "/"),
            "分析".to_string(),
            3,
            50,
            false,
        )
        .expect("本地目录检索必须照常成功");

        assert!(
            result
                .iter()
                .any(|entry| entry.name == "分析报告.txt"),
            "本地命中不能被云盘守卫误伤，实际 {:?}",
            result.iter().map(|e| e.name.as_str()).collect::<Vec<_>>()
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
