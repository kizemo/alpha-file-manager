// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-02：**删除阶段进度条「假 0%」**的判别性单测。
//
// 2026-11-02 真机验收的实测日志：
//
//   [02:16:00] phase=deleting items=0 total_items=Some(50051) percent=Some(0)
//   [02:16:12] phase=deleting items=0 total_items=Some(50051) percent=Some(0)
//   [02:16:32] phase=deleting items=0 total_items=Some(50051) percent=Some(0)
//   [02:16:38] finished in 53.17s: deleted_items=50051
//
// 回收站路径把**整棵树一次性交给 shell** 搬运，树内逐条目进度拿不到 ——
// 条目计数只在每个顶层路径搬完之后才推进。界面却照常报百分比，于是
// 50,051 项的目录删除时，进度条 0% 停了 38 秒，然后瞬间跳到 100%。
//
// 进度条的语义是「离完成还有多远」。给不出这个答案时就不该假装能给出
// —— 与 2026-10-22「进度条卡在 4%」事故是同一个教训。

import { describe, expect, it } from 'vitest';
import {
  isDeleteCancellable,
  isDeleteScanningPhase,
  isIndeterminateDeletePhase,
  isTrashMovePhase,
  shouldReportDeletePercent,
} from '@/utils/progress-presentation';

describe('删除作业的进度呈现', () => {
  it('统计阶段还没有分母，走不确定态', () => {
    const subject = {
      type: 'deleteTrash',
      phase: 'scanning' as const,
    };

    expect(isDeleteScanningPhase(subject)).toBe(true);
    expect(isIndeterminateDeletePhase(subject)).toBe(true);
    expect(shouldReportDeletePercent(subject)).toBe(false);
  });

  it('回收站搬运期不报百分比（假 0% 的回归点）', () => {
    const subject = {
      type: 'deleteTrash',
      phase: 'deleting' as const,
    };

    expect(isTrashMovePhase(subject)).toBe(true);
    expect(isIndeterminateDeletePhase(subject)).toBe(true);
    expect(shouldReportDeletePercent(subject)).toBe(false);
  });

  it('搬运期即使已经有条目搬完，也不假装知道进度', () => {
    const subject = {
      type: 'deleteTrash',
      phase: 'deleting' as const,
    };

    expect(isTrashMovePhase(subject)).toBe(true);
    expect(shouldReportDeletePercent(subject)).toBe(false);
  });

  it('永久删除是逐条目推进的，照常报真实百分比', () => {
    const subject = {
      type: 'deletePermanent',
      phase: 'deleting' as const,
    };

    expect(isTrashMovePhase(subject)).toBe(false);
    expect(isIndeterminateDeletePhase(subject)).toBe(false);
    expect(shouldReportDeletePercent(subject)).toBe(true);
  });

  it('阶段字段缺失的回收站作业按搬运处理，不报假百分比', () => {
    const subject = { type: 'deleteTrash' };

    expect(isTrashMovePhase(subject)).toBe(true);
    expect(shouldReportDeletePercent(subject)).toBe(false);
  });

  it('复制 / 移动等非删除作业不受影响', () => {
    const subject = {
      type: 'copy',
      phase: 'deleting' as const,
    };

    expect(isDeleteScanningPhase(subject)).toBe(false);
    expect(isTrashMovePhase(subject)).toBe(false);
    expect(isIndeterminateDeletePhase(subject)).toBe(false);
    expect(shouldReportDeletePercent(subject)).toBe(false);
  });
});

// FORK-MODIFICATION 2026-11-02：**取消按钮能不能摆出来**的判别性单测。
//
// 2026-11-02 真机验收实测：用户在回收站搬运期点了「取消」，界面显示
// 「正在取消…」，作业却把 50,051 项一项不少地删完了（后端 `cancelled=false`）。
//
// 根因：回收站搬运是**一次性 shell 调用**（`delete_tree.rs` 的
// `delete_paths_to_trash`），搬运一旦开始就没有中断点 —— 取消标志位要到
// 下一个顶层路径才会被检查。所以搬运期那个「取消」按钮是个点了没用的按钮。

describe('删除作业能不能取消', () => {
  it('回收站搬运期取消不了', () => {
    const subject = {
      type: 'deleteTrash',
      phase: 'deleting' as const,
    };

    expect(isDeleteCancellable(subject)).toBe(false);
  });

  it('统计阶段随时能取消', () => {
    expect(isDeleteCancellable({
      type: 'deleteTrash',
      phase: 'scanning',
    })).toBe(true);
    expect(isDeleteCancellable({
      type: 'deletePermanent',
      phase: 'scanning',
    })).toBe(true);
  });

  it('永久删除是逐条目推进的，删除阶段也能取消', () => {
    const subject = {
      type: 'deletePermanent',
      phase: 'deleting' as const,
    };

    expect(isDeleteCancellable(subject)).toBe(true);
  });

  it('阶段字段缺失时按可取消处理（宁可多给一次机会）', () => {
    expect(isDeleteCancellable({ type: 'deleteTrash' })).toBe(true);
  });

  it('非删除作业不适用这个判定', () => {
    const subject = {
      type: 'copy',
      phase: 'deleting' as const,
    };

    expect(isDeleteCancellable(subject)).toBe(false);
  });
});
