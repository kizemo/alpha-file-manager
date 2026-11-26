// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//! W-21：CJK 子串候选的「term 词典扫描」通道。
//!
//! ## 背景（缺陷 A 的根因）
//!
//! 旧实现用 `RegexQuery::from_pattern(".*<词>.*", name_lower)` 做 CJK 子串兜底。
//! `RegexQuery` 走 `tantivy-fst` 的 DFA，而 `.*` 前缀让自动机**接受任意前缀**
//! ⇒ 每次检索都要把整个 term 词典逐字节走一遍（**每个 segment 各一遍**），
//! 而一次用户查询里这条通道要跑**两遍**（主查询的 Should 分支 + W-13 的独立子串池）。
//!
//! ## 新实现
//!
//! 手工扫一遍 term 词典，用字节级子串匹配挑出命中的 term，
//! 再用 `TermSetQuery`（常量分 1.0、命中集合 = 命中 term 的 postings 并集）取回文档。
//!
//! ## 命中集合必须与旧路径**完全一致**
//!
//! 旧路径的 `.*needle.*` 是**全匹配**语义（tantivy-fst 的 `Regex` 把每个正则
//! 当作 `^(re)$` 处理），且 Rust 正则的 `.` **不匹配换行**
//! ⇒ 命中条件 = 「term 里存在一次 needle 出现，且它前后的部分都不含换行」。
//! 实现必须复刻这条规则（见 `term_contains_substring`），否则命中集合会变。
//!
//! ## 纯 ASCII term 为什么可以跳过
//!
//! 子串通道只对**含非 ASCII 字符**的查询词开放（见 `SubstringScan::query_for`），
//! 而非 ASCII 查询词必然含至少一个非 ASCII 字节 ⇒ 纯 ASCII 的 term 在字节级
//! 不可能包含它 ⇒ 跳过纯 ASCII term **不改变命中集合**（只是省掉一次子串判定）。

use std::collections::BTreeMap;

use tantivy::query::{Query, RegexQuery, TermSetQuery};
use tantivy::{SegmentReader, Searcher, Term};

use super::ignore::normalize_case;
use super::incremental::escape_regex_literal;
use super::query::split_query_tokens;
use super::state::GlobalSearchIndexFields;

/// 单个查询词允许命中的 term 条数上限。
///
/// 超过上限就**降级**（退回旧的 `RegexQuery` 路径），**不截断** ——
/// 截断会改变命中集合，那是本任务的红线。
pub(super) const MAX_SUBSTRING_MATCHED_TERMS: usize = 100_000;

/// 单个查询词的子串命中结果。
pub(super) enum SubstringMatch {
    /// 命中的 term 列表（可能为空 —— 空 = 该词在词典里没有任何 term 含它）。
    Terms(Vec<Term>),
    /// 降级：命中 term 数超上限 ⇒ 退回旧的 `RegexQuery` 路径。
    FallbackToRegex,
}

/// 一次查询里所有子串查询词的扫描结果。
pub(super) struct SubstringScan {
    matches: BTreeMap<String, SubstringMatch>,
}

impl SubstringScan {
    /// 空扫描结果：查不到的词一律走旧的 `RegexQuery` 路径。
    pub(super) fn empty() -> Self {
        SubstringScan {
            matches: BTreeMap::new(),
        }
    }

    /// 扫一遍 `name_lower` 的 term 词典，挑出每个子串查询词命中的 term。
    ///
    /// 只扫**非 ASCII** 的查询词：纯 ASCII 词本来就没有子串通道（与改动前一致）。
    pub(super) fn scan(
        searcher: &Searcher,
        fields: &GlobalSearchIndexFields,
        query: &str,
        max_terms: usize,
    ) -> SubstringScan {
        let needles = substring_needles(query);
        if needles.is_empty() {
            return SubstringScan::empty();
        }

        let mut matched: Vec<Vec<Term>> = vec![Vec::new(); needles.len()];
        let mut overflowed: Vec<bool> = vec![false; needles.len()];

        // 按 segment 切块并行：每个 segment 的 term 词典互相独立（各自的 `.term`
        // 文件、各自的 `TermStreamer`），扫描之间没有共享可变状态。
        let segments = searcher.segment_readers();
        let mut partials: Vec<(Vec<Vec<Term>>, Vec<bool>)> = Vec::new();
        let mut failed = false;

        if !segments.is_empty() {
            let parallel = scan_parallelism().min(segments.len()).max(1);
            let chunk_size = segments.len().div_ceil(parallel);

            std::thread::scope(|scope| {
                let mut handles = Vec::with_capacity(parallel);
                for chunk in segments.chunks(chunk_size) {
                    let needles = &needles;
                    handles.push(
                        scope.spawn(move || scan_segments(chunk, fields, needles, max_terms)),
                    );
                }
                for handle in handles {
                    match handle.join() {
                        Ok(Some(partial)) => partials.push(partial),
                        // 任一块读不到词典 ⇒ 整次扫描降级（与逐段串行时的行为一致）。
                        Ok(None) | Err(_) => failed = true,
                    }
                }
            });
        }

        if failed {
            return SubstringScan::all_fallback(&needles);
        }

        // 归并：各块按 segment 顺序串行拼接 ⇒ 命中集合与逐段串行扫描逐条相同。
        for (chunk_matched, chunk_overflowed) in &mut partials {
            for (index, bucket) in chunk_matched.iter_mut().enumerate() {
                overflowed[index] |= chunk_overflowed[index];
                if overflowed[index] {
                    // 该词已判定超上限 ⇒ 丢掉已收集的（不截断，走降级）。
                    bucket.clear();
                    matched[index].clear();
                    continue;
                }
                matched[index].append(bucket);
                if matched[index].len() > max_terms {
                    // 命中数超上限 ⇒ **降级**（不截断）：截断会改变命中集合。
                    overflowed[index] = true;
                    matched[index].clear();
                }
            }
        }

        for (index, needle) in needles.iter().enumerate() {
            if overflowed[index] {
                log::warn!(
                    "[global-search] 子串词「{needle}」命中的 term 超过上限 {max_terms}，降级为正则路径"
                );
            }
        }

        let mut matches = BTreeMap::new();
        for (index, needle) in needles.iter().enumerate() {
            let value = if overflowed[index] {
                SubstringMatch::FallbackToRegex
            } else {
                SubstringMatch::Terms(std::mem::take(&mut matched[index]))
            };
            matches.insert(needle.clone(), value);
        }

        SubstringScan { matches }
    }

    /// 全部降级为正则路径（扫描失败时用）。
    fn all_fallback(needles: &[String]) -> SubstringScan {
        let mut matches = BTreeMap::new();
        for needle in needles {
            matches.insert(needle.clone(), SubstringMatch::FallbackToRegex);
        }
        SubstringScan { matches }
    }

    /// 该词的扫描结果（`None` = 该词没被扫描过）。
    pub(super) fn match_for(&self, word: &str) -> Option<&SubstringMatch> {
        self.matches.get(word)
    }

    /// 该词的子串查询。
    ///
    /// `None` = 该词**没有**子串通道（纯 ASCII 词，与改动前逐字一致）。
    pub(super) fn query_for(
        &self,
        fields: &GlobalSearchIndexFields,
        word: &str,
    ) -> Option<Box<dyn Query>> {
        if word.is_ascii() {
            return None;
        }

        match self.match_for(word) {
            // `TermSetQuery` 与 `RegexQuery` 一样走 `AutomatonWeight`
            // ⇒ 常量分 1.0、命中集合 = 命中 term 的 postings 并集 ⇒ 口径一致。
            Some(SubstringMatch::Terms(terms)) => {
                Some(Box::new(TermSetQuery::new(terms.iter().cloned())))
            }
            // 降级 / 没扫过 ⇒ 走旧的正则路径，命中集合一定不变。
            Some(SubstringMatch::FallbackToRegex) | None => build_regex_substring_query(fields, word),
        }
    }
}

/// 扫描指定的一段 segment，返回该块的命中列表与「超过上限」标记。
///
/// `None` = 该块读不到词典 ⇒ 调用方整次降级为正则路径（与串行实现一致）。
fn scan_segments(
    segments: &[SegmentReader],
    fields: &GlobalSearchIndexFields,
    needles: &[String],
    max_terms: usize,
) -> Option<(Vec<Vec<Term>>, Vec<bool>)> {
    let mut matched: Vec<Vec<Term>> = vec![Vec::new(); needles.len()];
    let mut overflowed: Vec<bool> = vec![false; needles.len()];

    for segment in segments {
        let inverted_index = match segment.inverted_index(fields.name_lower) {
            Ok(inverted_index) => inverted_index,
            Err(error) => {
                log::warn!("[global-search] 子串词典扫描读不到倒排索引，降级为正则路径：{error}");
                return None;
            }
        };

        let mut stream = match inverted_index.terms().stream() {
            Ok(stream) => stream,
            Err(error) => {
                log::warn!("[global-search] 子串词典扫描读不到 term 词典，降级为正则路径：{error}");
                return None;
            }
        };

        while stream.advance() {
            let key = stream.key();

            // 纯 ASCII 的 term 不可能含非 ASCII 的 needle ⇒ 跳过（不改变命中集合）。
            if key.is_ascii() {
                continue;
            }

            let Ok(text) = std::str::from_utf8(key) else {
                continue;
            };

            for (index, needle) in needles.iter().enumerate() {
                if overflowed[index] {
                    continue;
                }
                if !term_contains_substring(text, needle) {
                    continue;
                }
                matched[index].push(Term::from_field_text(fields.name_lower, text));
                if matched[index].len() > max_terms {
                    // 本块已超上限 ⇒ 标记后丢弃（不截断；归并时该词整体降级）。
                    overflowed[index] = true;
                    matched[index].clear();
                }
            }
        }
    }

    Some((matched, overflowed))
}

/// 词典扫描的并行度：受可用核数限制并封顶 8（实际线程数还取段数的较小者）。
fn scan_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1)
        .clamp(1, 8)
}

/// 需要走子串通道的查询词：非 ASCII、去重、保序。
fn substring_needles(query: &str) -> Vec<String> {
    let normalized = normalize_case(query);
    let mut needles: Vec<String> = Vec::new();
    for word in split_query_tokens(&normalized) {
        if word.is_ascii() {
            continue;
        }
        if !needles.contains(&word) {
            needles.push(word);
        }
    }
    needles
}

/// 旧的 `RegexQuery` 路径（降级时使用，行为与改动前逐字一致）。
pub(super) fn build_regex_substring_query(
    fields: &GlobalSearchIndexFields,
    word: &str,
) -> Option<Box<dyn Query>> {
    let pattern = format!(".*{}.*", escape_regex_literal(word));

    match RegexQuery::from_pattern(&pattern, fields.name_lower) {
        Ok(query) => Some(Box::new(query)),
        Err(error) => {
            // 转义之后理论上不会失败；真失败就退化成「只有 fuzzy」，
            // 也就是改动前的行为，不阻断查询。
            log::warn!("[global-search] 子串查询正则构造失败（pattern={pattern}）：{error}");
            None
        }
    }
}

/// 该词的子串查询（带扫描结果的入口）。
///
/// - `scan = None` ⇒ **没有扫描结果**（精确匹配模式 / 未扫描）⇒ 非 ASCII 词
///   退回旧的 `RegexQuery` 路径，命中集合与改动前逐字一致；
/// - 纯 ASCII 词**任何情况下都没有**子串通道（与改动前一致）。
pub(super) fn substring_query_for(
    fields: &GlobalSearchIndexFields,
    word: &str,
    scan: Option<&SubstringScan>,
) -> Option<Box<dyn Query>> {
    if word.is_ascii() {
        return None;
    }

    match scan {
        Some(scan) => scan.query_for(fields, word),
        None => build_regex_substring_query(fields, word),
    }
}

/// `term` 是否满足旧正则 `.*needle.*` 的命中语义。
///
/// 旧路径的正则是**整串全匹配**（tantivy-fst 把每个正则当 `^(re)$` 处理），
/// 且 Rust 正则的 `.` **不匹配换行**
/// ⇒ 命中条件 = 「term 字节里存在一次 needle 出现，且它前面的部分与后面的部分都不含换行」。
///
/// 推论：needle 不含换行时，**term 只要含换行就永远不命中**
/// （needle 区间不可能同时盖住 term 里的全部换行 ⇒ 前后至少一侧含换行）。
/// 这条规则就是「纯 `contains` 是超集」的根因 —— 不复刻它就会**放大命中集合**。
pub(super) fn term_contains_substring(term: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }

    // 快路径：term 与 needle 都不含换行 ⇒ 纯子串判定即可。
    // （`str::contains` 走 Rust 的 memchr / 双字节快搜，比手写窗口比较快一个量级。）
    if !term.contains('\n') && !needle.contains('\n') {
        return term.contains(needle);
    }

    // 慢路径：needle 的出现区间必须**盖住 term 的全部换行**。
    let term_bytes = term.as_bytes();
    let needle_bytes = needle.as_bytes();
    let first_newline = term_bytes.iter().position(|byte| *byte == b'\n');
    let last_newline = term_bytes.iter().rposition(|byte| *byte == b'\n');

    let mut search_from = 0usize;
    while search_from + needle_bytes.len() <= term_bytes.len() {
        let Some(offset) = find_bytes(&term_bytes[search_from..], needle_bytes) else {
            break;
        };
        let hit_start = search_from + offset;
        let hit_end = hit_start + needle_bytes.len();

        // 前缀 [0, hit_start) 与后缀 [hit_end, len) 都不得含换行。
        let prefix_ok = first_newline.map_or(true, |position| hit_start <= position);
        let suffix_ok = last_newline.map_or(true, |position| hit_end > position);
        if prefix_ok && suffix_ok {
            return true;
        }

        // 继续找下一次出现（允许重叠，避免漏掉被 `match_indices` 跳过的位置）。
        search_from = hit_start + 1;
    }

    false
}

/// 朴素子串查找（只在慢路径用 —— 含换行的 term 极少）。
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::global_search::index::build_schema;
    use tantivy::collector::TopDocs;
    use tantivy::doc;
    use tantivy::schema::Value;

    fn build_index(names: &[&str]) -> (tantivy::Index, GlobalSearchIndexFields) {
        let (schema, fields) = build_schema();
        let index = tantivy::Index::create_in_ram(schema);
        let mut writer = index.writer(50_000_000).unwrap();

        for name in names {
            writer
                .add_document(doc!(
                    fields.path => format!("C:/test/{name}"),
                    fields.name => (*name).to_string(),
                    fields.name_lower => (*name).to_lowercase(),
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

    fn name_of(
        searcher: &tantivy::Searcher,
        fields: &GlobalSearchIndexFields,
        address: tantivy::DocAddress,
    ) -> String {
        searcher
            .doc(address)
            .ok()
            .and_then(|document: tantivy::TantivyDocument| {
                document
                    .get_first(fields.name)
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string())
            })
            .unwrap_or_default()
    }

    /// 旧路径（`RegexQuery`）的命中集合。
    fn old_regex_hits(
        searcher: &tantivy::Searcher,
        fields: &GlobalSearchIndexFields,
        needle: &str,
    ) -> Vec<String> {
        let pattern = format!(".*{}.*", escape_regex_literal(needle));
        let query = RegexQuery::from_pattern(&pattern, fields.name_lower).unwrap();

        searcher
            .search(&query, &TopDocs::with_limit(1_000).order_by_score())
            .unwrap()
            .into_iter()
            .map(|(_score, address)| name_of(searcher, fields, address))
            .collect()
    }

    /// 新路径（词典扫描 + `TermSetQuery`）的命中集合。
    fn new_scan_hits(
        searcher: &tantivy::Searcher,
        fields: &GlobalSearchIndexFields,
        needle: &str,
        scan: &SubstringScan,
    ) -> Vec<String> {
        let Some(query) = scan.query_for(fields, needle) else {
            return Vec::new();
        };

        searcher
            .search(&query, &TopDocs::with_limit(1_000).order_by_score())
            .unwrap()
            .into_iter()
            .map(|(_score, address)| name_of(searcher, fields, address))
            .collect()
    }

    fn sorted(mut names: Vec<String>) -> Vec<String> {
        names.sort();
        names
    }

    #[test]
    fn term_contains_plain_substring() {
        assert!(term_contains_substring(
            "2024年用户调研技巧汇总.docx",
            "技巧"
        ));
        assert!(!term_contains_substring("项目计划表.xlsx", "技巧"));
    }

    #[test]
    fn term_contains_respects_the_newline_rule_of_the_old_regex() {
        // 旧正则 `.*技巧.*` 是全匹配，且 `.` 不匹配换行：
        // 「技巧」前面只要有换行，匹配就不成立。
        assert!(
            !term_contains_substring("使用\n技巧.docx", "技巧"),
            "前缀含换行时旧正则不命中，纯 contains 是超集 ⇒ 会改变命中集合"
        );
        assert!(
            !term_contains_substring("技巧\n汇总.docx", "技巧"),
            "后缀含换行时旧正则不命中"
        );
    }

    #[test]
    fn scan_finds_cjk_substring_across_a_plus_sign() {
        let names = ["Midjourney 使用心得+关键词分析.mp4"];
        let (index, fields) = build_index(&names);
        let reader = index.reader().unwrap();
        let searcher = reader.searcher();

        let scan = SubstringScan::scan(&searcher, &fields, "使用心得+关键", MAX_SUBSTRING_MATCHED_TERMS);
        let hits = new_scan_hits(&searcher, &fields, "使用心得+关键", &scan);

        assert!(
            hits.iter().any(|name| name.starts_with("Midjourney")),
            "跨 `+` 的 CJK 子串取不到候选，hits={hits:?}"
        );
    }

    #[test]
    fn scan_does_not_admit_pure_ascii_names() {
        let names = ["report-2024.txt", "2024年用户调研技巧汇总.docx"];
        let (index, fields) = build_index(&names);
        let reader = index.reader().unwrap();
        let searcher = reader.searcher();

        let scan = SubstringScan::scan(&searcher, &fields, "技巧", MAX_SUBSTRING_MATCHED_TERMS);
        let hits = new_scan_hits(&searcher, &fields, "技巧", &scan);

        assert!(
            hits.contains(&"2024年用户调研技巧汇总.docx".to_string()),
            "CJK 靶子必须命中，hits={hits:?}"
        );
        assert!(
            !hits.contains(&"report-2024.txt".to_string()),
            "纯 ASCII 文件名不该被 CJK 子串通道纳入候选，hits={hits:?}"
        );
    }

    #[test]
    fn scan_falls_back_to_regex_when_the_term_cap_is_exceeded() {
        let names = ["技巧报告.pdf", "技巧总结.docx", "深度技巧.mp4"];
        let (index, fields) = build_index(&names);
        let reader = index.reader().unwrap();
        let searcher = reader.searcher();

        // 上限压到 1 ⇒ 命中 3 个 term 必然超限。
        let scan = SubstringScan::scan(&searcher, &fields, "技巧", 1);

        assert!(
            matches!(scan.match_for("技巧"), Some(SubstringMatch::FallbackToRegex)),
            "命中 term 数超过上限时必须降级为正则路径（截断会改变命中集合）"
        );

        let hits = new_scan_hits(&searcher, &fields, "技巧", &scan);
        assert_eq!(
            hits.len(),
            3,
            "降级后命中集合必须保持完整，hits={hits:?}"
        );
    }

    #[test]
    fn new_path_hit_set_matches_the_old_regex_hit_set() {
        // 故意掺入：换行、正则元字符、纯 ASCII、大小写。
        let names = [
            "2024年用户调研技巧汇总.docx",
            "Midjourney 使用心得+关键词分析.mp4",
            "report-2024.txt",
            "技巧.txt",
            "使用\n技巧.docx",
            "技巧\n汇总.docx",
            "a.b*c技巧报告.pdf",
            "会议记录2024.docx",
            "技巧报告(最终版).pdf",
        ];
        let (index, fields) = build_index(&names);
        let reader = index.reader().unwrap();
        let searcher = reader.searcher();

        let old_hits = sorted(old_regex_hits(&searcher, &fields, "技巧"));

        let scan = SubstringScan::scan(&searcher, &fields, "技巧", MAX_SUBSTRING_MATCHED_TERMS);
        let new_hits = sorted(new_scan_hits(&searcher, &fields, "技巧", &scan));

        assert_eq!(
            old_hits, new_hits,
            "新路径的命中集合必须与旧 RegexQuery 路径完全一致"
        );
    }
}
