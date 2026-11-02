// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-01：**删除作业扫描阶段 → 规模提示**的判别性单测。
//
// 以前「正在删除 N 个项目」这条提示，靠删除**之前**的一次独立预扫描拿到规模。
// 那次预扫描是同步命令 ⇒ Tauri v2 把它跑在主线程上 ⇒ 扫大树时整个窗口冻住，
// 用户看到的就是「删除后长时间无响应」。
//
// 现在规模由删除作业自己的扫描阶段产出。于是「扫描阶段什么时候算结束」
// 必须有确定判据：**阶段仍是 `scanning` 且分母已经出现**（扫描途中没有分母，
// 数完才有）。

import { describe, expect, it } from 'vitest';
import {
  LARGE_BATCH_ITEM_THRESHOLD,
  isDeleteScanCompleted,
  shouldWarnLargeBatchDelete,
  toDeleteScanSummary,
} from '@/utils/delete-scan-summary';

describe('删除作业的扫描阶段', () => {
  it('扫描途中没有分母，不算完成', () => {
    expect(
      isDeleteScanCompleted({
        phase: 'scanning',
        totalCount: null,
        files: 0,
        directories: 0,
        totalBytes: null,
      }),
    ).toBe(false);
  });

  it('统计完的那一条（阶段仍是 scanning 且分母出现）算完成', () => {
    expect(
      isDeleteScanCompleted({
        phase: 'scanning',
        totalCount: 1200,
        files: 1000,
        directories: 200,
        totalBytes: 4096,
      }),
    ).toBe(true);
  });

  it('删除阶段的进度不算扫描完成', () => {
    expect(
      isDeleteScanCompleted({
        phase: 'deleting',
        totalCount: 1200,
        files: 1000,
        directories: 200,
        totalBytes: 4096,
      }),
    ).toBe(false);
  });

  it('规模摘要从进度事件里取齐四个数', () => {
    expect(
      toDeleteScanSummary({
        phase: 'scanning',
        totalCount: 1200,
        files: 1000,
        directories: 200,
        totalBytes: 4096,
      }),
    ).toEqual({
      totalItems: 1200,
      files: 1000,
      directories: 200,
      totalBytes: 4096,
    });
  });

  it('缺字段时按 0 兜底，不给 NaN', () => {
    expect(
      toDeleteScanSummary({
        phase: 'scanning',
        totalCount: 5,
      }),
    ).toEqual({
      totalItems: 5,
      files: 0,
      directories: 0,
      totalBytes: 0,
    });
  });

  it('达到阈值才提示，未达到不打扰', () => {
    expect(
      shouldWarnLargeBatchDelete({
        totalItems: LARGE_BATCH_ITEM_THRESHOLD - 1,
        files: 0,
        directories: 0,
        totalBytes: 0,
      }),
    ).toBe(false);

    expect(
      shouldWarnLargeBatchDelete({
        totalItems: LARGE_BATCH_ITEM_THRESHOLD,
        files: 0,
        directories: 0,
        totalBytes: 0,
      }),
    ).toBe(true);
  });
});
