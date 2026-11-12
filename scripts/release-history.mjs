// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 站点版本清单 history[] 的去重规则。
//
// 根因：归档旧版本进 history[] 时无条件 unshift，而「重发一个已经进过历史的旧版本」
// 不会先把同版本的旧条目去掉 ⇒ 站点版本历史里出现两条一模一样的记录（3.12.0 就这么重过一条）。
//
// 这里把「去掉同版本旧条目」抽成纯函数：一来 release-publish.mjs 只留调用，
// 二来它可以被单测直接覆盖，不必去跑整条发版链路。

/**
 * 去掉 history 里所有 version 与给定版本相同的条目。
 *
 * @param {Array<{version?: string|null}> | null | undefined} history
 * @param {string | null | undefined} version 要去掉的版本号
 * @returns {{ history: Array<{version?: string|null}>, removed: number }} 去重后的数组与被去掉的条数
 */
export function dropHistoryEntriesForVersion(history, version) {
  const source = Array.isArray(history) ? history : [];

  // 版本号缺失时不许动历史：一个 entry?.version === undefined 的空壳条目不是「同版本」。
  if (version === null || version === undefined || version === '') {
    return { history: source.slice(), removed: 0 };
  }

  const kept = source.filter(entry => entry?.version !== version);

  return { history: kept, removed: source.length - kept.length };
}
