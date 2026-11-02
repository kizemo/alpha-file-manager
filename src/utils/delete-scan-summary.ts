// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-01：删除作业扫描阶段的规模摘要。
//
// 「正在删除 N 个项目」这条提示，以前靠删除**之前**另起一趟预扫描拿到规模。
// 那趟预扫描是同步命令 ⇒ Tauri v2 把它跑在主线程上 ⇒ 扫一棵大树时整个
// 窗口冻住，用户看到的就是「删除后长时间无响应」。
//
// 现在规模由删除作业**自己的扫描阶段**产出 —— 同一趟遍历，既当进度分母，
// 又当规模提示的来源，而且跑在后台线程上、可取消。
//
// 代价是「扫描阶段什么时候算结束」必须有确定判据，不能靠猜。

/**
 * FORK-MODIFICATION 2026-10-22：超过这个条目数就算「大批量」。
 *
 * 阈值取自 2026-10-22 那份批量删除分析报告的建议值（10,000）。低于它的时候
 * 删除是「秒级」的事，弹个规模提示纯属噪声。
 */
export const LARGE_BATCH_ITEM_THRESHOLD = 10_000;

/** 一批待删除目标的规模概览。 */
export interface DeleteScanSummary {
  /** 条目总数（文件 + 目录）。 */
  totalItems: number;
  /** 文件数。 */
  files: number;
  /** 目录数。 */
  directories: number;
  /** 总字节。 */
  totalBytes: number;
}

/** 进度事件里能拿到的规模字段。 */
export interface DeleteScanSnapshot {
  /** 当前阶段：`scanning` / `deleting`。 */
  phase?: string | null;
  /** 条目总数；扫描途中还没有分母。 */
  totalCount?: number | null;
  files?: number | null;
  directories?: number | null;
  totalBytes?: number | null;
  /**
   * 后端显式给的「扫描已数完」标志。
   *
   * 旧实现只能靠「阶段还是 scanning 且分母出现了」来猜，但扫描途中的进度
   * 事件也可能带分母 —— 那会让规模提示提前弹出，而且文件数 / 目录数还是 0。
   */
  scanCompleted?: boolean | null;
}

/**
 * 扫描阶段是否已经数完。
 *
 * 判据：阶段仍是 `scanning`、分母已经出现，并且要么后端明确标了
 * `scanCompleted`，要么文件数 / 目录数已经不是 0（扫描途中的进度事件
 * 这两个数恒为 0）。
 */
export function isDeleteScanCompleted(snapshot: DeleteScanSnapshot): boolean {
  if (snapshot.phase !== 'scanning') {
    return false;
  }

  if (!Number.isFinite(snapshot.totalCount)) {
    return false;
  }

  if (snapshot.scanCompleted === true) {
    return true;
  }

  return (snapshot.files ?? 0) > 0 || (snapshot.directories ?? 0) > 0;
}

/** 从进度事件里取齐规模摘要；缺字段按 0 兜底，不给 NaN。 */
export function toDeleteScanSummary(snapshot: DeleteScanSnapshot): DeleteScanSummary {
  return {
    totalItems: snapshot.totalCount ?? 0,
    files: snapshot.files ?? 0,
    directories: snapshot.directories ?? 0,
    totalBytes: snapshot.totalBytes ?? 0,
  };
}

/** 规模够大才提示，未达到不打扰用户。 */
export function shouldWarnLargeBatchDelete(summary: DeleteScanSummary): boolean {
  return summary.totalItems >= LARGE_BATCH_ITEM_THRESHOLD;
}
