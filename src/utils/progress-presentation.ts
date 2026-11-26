// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-02：删除作业的进度**该怎么呈现**。
//
// 背景（2026-11-02 真机验收实测）：
//
//   [02:16:00] phase=deleting items=0 total_items=Some(50051) percent=Some(0)
//   [02:16:32] phase=deleting items=0 total_items=Some(50051) percent=Some(0)
//   [02:16:38] finished in 53.17s: deleted_items=50051
//
// 回收站路径把**整棵树一次性交给 shell** 搬运，树内逐条目进度拿不到 ——
// 条目计数只在每个顶层路径搬完之后才推进。界面却照常报百分比，于是
// 50,051 项的目录删除时进度条 0% 停了 38 秒，然后瞬间跳到 100%。
//
// 进度条的语义是「离完成还有多远」。给不出这个答案时就不该假装能给出
// —— 与 2026-10-22「进度条卡在 4%」事故是同一个教训。

/** 判定呈现方式只需要这几个字段。 */
export interface DeleteProgressSubject {
  /** 作业类型：`deleteTrash` / `deletePermanent` / 其它。 */
  type: string;
  /** 当前阶段：统计 / 删除。 */
  phase?: 'scanning' | 'deleting' | null;
}

/** 是否是删除类作业。 */
export function isDeleteOperation(subject: DeleteProgressSubject): boolean {
  return subject.type === 'deleteTrash' || subject.type === 'deletePermanent';
}

/**
 * 统计阶段：总数还没数完，没有分母。
 *
 * 只有删除类作业才有这个阶段 —— 复制 / 移动的进度口径是顶层路径数，不是条目数。
 */
export function isDeleteScanningPhase(subject: DeleteProgressSubject): boolean {
  return subject.phase === 'scanning' && isDeleteOperation(subject);
}

/**
 * 回收站搬运阶段：整棵树一次性交给 shell，树内进度拿不到。
 *
 * 判据用「阶段不是统计」而不是「阶段是删除」—— 阶段字段缺失时也按搬运处理，
 * 因为报一个假 0% 比不报更糟。
 *
 * 注意：这个判据是**结构性的**（类型 + 阶段），不看作业是否还在跑；
 * 调用方要显示进度条时自己再按 `status` 过滤。
 */
export function isTrashMovePhase(subject: DeleteProgressSubject): boolean {
  return subject.type === 'deleteTrash' && subject.phase !== 'scanning';
}

/** 进度条是否应该走不确定态。 */
export function isIndeterminateDeletePhase(subject: DeleteProgressSubject): boolean {
  return isDeleteScanningPhase(subject) || isTrashMovePhase(subject);
}

/**
 * 删除阶段是否还应该报百分比。
 *
 * 回收站搬运期拿不到树内进度 ⇒ 不报（报出来的 0% 是假的）；
 * 永久删除是逐条目推进的 ⇒ 照常报真实百分比。
 */
export function shouldReportDeletePercent(subject: DeleteProgressSubject): boolean {
  if (!isDeleteOperation(subject)) {
    return false;
  }

  if (subject.phase === 'scanning') {
    return false;
  }

  return !isTrashMovePhase(subject);
}

/**
 * 这个删除作业**现在**还能不能取消。
 *
 * 事实边界（2026-11-02 真机实测 + `delete_tree.rs` 的中断点分布）：
 *
 * - 统计阶段：随时能取消 —— 扫描本身带中断点。
 * - 永久删除阶段：能取消 —— 逐条目推进，每个条目都是中断点。
 * - 回收站搬运阶段：**不能取消** —— 整棵树一次性交给 shell，搬运一旦开始
 *   就没有中断点。这时候点「取消」只会让界面显示「正在取消…」，而作业照常
 *   把整棵树删完（实测 50,051 项删了 45 秒，后端 `cancelled=false`）。
 *
 * 所以界面不该在搬运期摆出一个「取消」按钮 —— 那是一个点了没用的按钮。
 *
 * 阶段未知（`phase` 缺失）时按「可取消」处理：作业刚建起来时阶段字段可能
 * 还没到，宁可多给一次机会，也别把一个真能取消的作业说成不能取消。
 */
export function isDeleteCancellable(subject: DeleteProgressSubject): boolean {
  if (!isDeleteOperation(subject)) {
    return false;
  }

  if (subject.phase === 'deleting') {
    return subject.type === 'deletePermanent';
  }

  return true;
}
