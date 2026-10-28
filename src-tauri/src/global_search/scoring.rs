// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

pub(super) fn calculate_similarity_score(query: &str, name: &str) -> f32 {
    let query_lower = query.to_lowercase();
    let name_lower = name.to_lowercase();

    if name_lower == query_lower {
        return 1.0;
    }

    if name_lower.starts_with(&query_lower) {
        return 0.95 + (query_lower.len() as f32 / name_lower.len() as f32) * 0.05;
    }

    // FORK-MODIFICATION: 2026-10-28 —— 「子串命中」不再早退，改为与 token 分支取最大值。
    //
    // 原来这里直接 `return`，于是下面整段 token 分支在「名字包含查询」时**永远执行不到**。
    // 而子串分支的得分区间只有 [0.8, 0.95)，短查询的门槛却是 0.9
    // （`get_min_score_for_query_length`：1-3 字符 0.9 / 4-6 字符 0.65 / 7-9 字符 0.55 /
    // 更长 0.5）。两边一撞就是：
    //
    //   查询 "doc"（3 字符，门槛 0.9）命中 "report.docx"
    //   早退得分 = 0.8 + (3/10)*0.15 = 0.845 < 0.9  ->  被丢弃
    //
    // 而 token 分支本来能给出 0.6 + 1.0*0.2 + 0.9*0.15 = 0.935
    // （"docx".starts_with("doc")），足以过门槛 —— 只是永远轮不到它跑。
    //
    // 后果：按扩展名搜（pdf / doc / jpg）、按中间片段搜（095 -> 20251009_0095_27.jpg）
    // 这类**非前缀的子串命中**几乎全灭，用户看到的就是「短查询搜不到东西」。
    // 这也是 §18 里记的「多盘符搜索返回条数少于 limit」的成因之一：
    // 候选池在过滤前就被截断，被门槛挡掉的候选又没有替补。
    //
    // 现在的语义：子串命中本身就是「用户敲的字面量出现在名字里」的强信号，
    // 不该仅仅因为名字长就被压到门槛以下；token 分支负责把跨分隔符的命中救回来。
    // 两边取最大值即可，门槛与打分口径重新对齐。
    //
    // 刻意**不动** `get_min_score_for_query_length` 的门槛数值：门槛降下来会把短查询的
    // 模糊（typo）匹配一起放开，噪音远大于收益；先修打分口径这个真正的病根。
    let substring_score = if name_lower.contains(&query_lower) {
        0.8 + (query_lower.len() as f32 / name_lower.len() as f32) * 0.15
    } else {
        0.0
    };

    let query_tokens: Vec<&str> = query_lower
        .split(|c: char| c.is_whitespace() || c == '.' || c == '_' || c == '-')
        .filter(|segment| !segment.is_empty())
        .collect();

    let name_tokens: Vec<&str> = name_lower
        .split(|c: char| c.is_whitespace() || c == '.' || c == '_' || c == '-')
        .filter(|segment| !segment.is_empty())
        .collect();

    let mut token_score = 0.0f32;

    if !query_tokens.is_empty() && !name_tokens.is_empty() {
        let mut matched_count = 0;
        let mut partial_match_score = 0.0f32;

        for query_token in &query_tokens {
            let mut best_token_score = 0.0f32;

            for name_token in &name_tokens {
                if *name_token == *query_token {
                    best_token_score = 1.0;
                    break;
                } else if name_token.starts_with(query_token) {
                    best_token_score = best_token_score.max(0.9);
                } else if name_token.contains(query_token) {
                    best_token_score = best_token_score.max(0.8);
                } else {
                    let query_chars: Vec<char> = query_token.chars().collect();
                    let name_chars: Vec<char> = name_token.chars().collect();
                    if !query_chars.is_empty() && !name_chars.is_empty() {
                        let distance = levenshtein_distance(&query_chars, &name_chars);
                        let max_len = query_chars.len().max(name_chars.len());
                        if distance <= 2 && max_len > 0 {
                            let similarity = 1.0 - (distance as f32 / max_len as f32);
                            best_token_score = best_token_score.max(similarity * 0.7);
                        }
                    }
                }
            }

            if best_token_score > 0.5 {
                matched_count += 1;
            }
            partial_match_score += best_token_score;
        }

        let match_ratio = matched_count as f32 / query_tokens.len() as f32;
        let avg_token_score = partial_match_score / query_tokens.len() as f32;

        if match_ratio >= 0.5 {
            token_score = 0.6 + (match_ratio * 0.2) + (avg_token_score * 0.15);
        }
    }

    // 子串命中与 token 命中取较高者（FORK-MODIFICATION: 2026-10-28，见上）。
    let best_score = substring_score.max(token_score);
    if best_score > 0.0 {
        return best_score;
    }

    let query_chars: Vec<char> = query_lower.chars().collect();
    let name_chars: Vec<char> = name_lower.chars().collect();

    if query_chars.is_empty() || name_chars.is_empty() {
        return 0.0;
    }

    let distance = levenshtein_distance(&query_chars, &name_chars);
    let max_len = query_chars.len().max(name_chars.len()) as f32;
    let similarity = 1.0 - (distance as f32 / max_len);

    similarity.max(0.0)
}

fn levenshtein_distance(first: &[char], second: &[char]) -> usize {
    let len_first = first.len();
    let len_second = second.len();

    if len_first == 0 {
        return len_second;
    }

    if len_second == 0 {
        return len_first;
    }

    let mut previous_row: Vec<usize> = (0..=len_second).collect();
    let mut current_row: Vec<usize> = vec![0; len_second + 1];

    for row_idx in 1..=len_first {
        current_row[0] = row_idx;

        for col_idx in 1..=len_second {
            let cost = if first[row_idx - 1] == second[col_idx - 1] {
                0
            } else {
                1
            };

            current_row[col_idx] = (previous_row[col_idx] + 1)
                .min(current_row[col_idx - 1] + 1)
                .min(previous_row[col_idx - 1] + cost);
        }

        std::mem::swap(&mut previous_row, &mut current_row);
    }

    previous_row[len_second]
}

pub(super) fn get_min_score_for_query_length(query_len: usize) -> f32 {
    match query_len {
        1..=3 => 0.9,
        4..=6 => 0.65,
        7..=9 => 0.55,
        _ => 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FORK-MODIFICATION: 2026-10-28 —— 短查询门槛缺陷的回归用例。
    ///
    /// 缺陷：`calculate_similarity_score` 的「子串命中」分支早退，导致 token 分支
    /// 永远跑不到；子串分支得分上限 0.95 而短查询门槛是 0.9，于是**非前缀**的
    /// 子串命中（按扩展名搜、按中间片段搜）几乎全被 0.9 的门槛挡掉。
    ///
    /// 判据统一写成「得分 >= 该查询长度对应的门槛」，这样门槛将来若调整，
    /// 用例会自动跟随，不会变成钉死的魔数。

    #[test]
    fn short_query_matches_extension_of_longer_name() {
        let score = calculate_similarity_score("doc", "report.docx");
        assert!(
            score >= get_min_score_for_query_length("doc".len()),
            "score {score} should pass the 3-char threshold"
        );
    }

    #[test]
    fn short_query_matches_numeric_fragment_in_long_name() {
        let score = calculate_similarity_score("095", "20251009_0095_27.jpg");
        assert!(
            score >= get_min_score_for_query_length("095".len()),
            "score {score} should pass the 3-char threshold"
        );
    }

    #[test]
    fn short_query_matches_extension_after_cjk_prefix() {
        let score = calculate_similarity_score("pdf", "2024年报表.pdf");
        assert!(
            score >= get_min_score_for_query_length("pdf".len()),
            "score {score} should pass the 3-char threshold"
        );
    }

    #[test]
    fn short_cjk_query_matches_longer_name() {
        let score = calculate_similarity_score("图", "图片.png");
        assert!(
            score >= get_min_score_for_query_length("图".len()),
            "score {score} should pass the 3-char threshold"
        );
    }

    #[test]
    fn unrelated_name_stays_below_short_query_threshold() {
        let score = calculate_similarity_score("zzz", "hello.txt");
        assert!(
            score < get_min_score_for_query_length("zzz".len()),
            "score {score} should stay below the 3-char threshold"
        );
    }

    #[test]
    fn prefix_hit_still_scores_above_short_query_threshold() {
        let score = calculate_similarity_score("report", "report.docx");
        assert!(score >= 0.95, "score {score} should keep the prefix bonus");
    }

    #[test]
    fn typo_tolerance_still_works_for_medium_queries() {
        // token 分支的容错匹配（约 0.89）必须仍然能过 4-6 字符的门槛 0.65。
        let score = calculate_similarity_score("repot", "report.docx");
        assert!(
            score >= get_min_score_for_query_length("repot".len()),
            "score {score} should pass the 5-char threshold"
        );
    }

    #[test]
    fn identical_names_score_one() {
        assert_eq!(calculate_similarity_score("report", "report"), 1.0);
    }
}
