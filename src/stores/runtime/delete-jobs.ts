// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { defineStore } from 'pinia';
import { markRaw, shallowRef } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { toast, ToastStatic } from '@/components/ui/toaster';
import { i18n } from '@/localization';
import uniqueId from '@/utils/unique-id';
import { sourceDisplayNameFromPaths } from '@/utils/source-display-name';
// FORK-MODIFICATION 2026-11-01：删除作业扫描阶段的规模摘要。
import {
  isDeleteScanCompleted,
  toDeleteScanSummary,
  type DeleteScanSummary,
} from '@/utils/delete-scan-summary';
import { useStatusCenterStore } from './status-center';

interface DeleteJobProgressPayload {
  jobId: string;
  percent: number;
  detail: string;
  processedCount?: number | null;
  totalCount?: number | null;
  /**
   * FORK-MODIFICATION 2026-10-22：进度语义从「顶层路径数」换成「条目数」。
   *
   * 旧口径在删一棵含 25 万个文件的目录树时，进度条在整个删除期间几乎不动
   * （用户看到「4% 卡住 40 分钟」）。新口径按条目推进，字节数作辅助。
   */
  phase?: 'scanning' | 'deleting';
  /**
   * FORK-MODIFICATION 2026-11-01：扫描阶段是否已经数完。
   *
   * 规模提示（「正在删除 N 个项目」）以前靠删除**之前**另起的一趟同步预扫描，
   * 那趟扫描跑在 Tauri 主线程上，扫大树时把整个窗口冻住 —— 用户看到的就是
   * 「删除后长时间无响应」。现在规模由作业自己的扫描阶段产出。
   */
  scanCompleted?: boolean;
  deletedItems?: number;
  freedBytes?: number;
  totalBytes?: number | null;
  itemPercent?: number | null;
  bytePercent?: number | null;
  files?: number;
  directories?: number;
  currentPath?: string;
}

interface DeleteJobFinishedPayload {
  jobId: string;
  success: boolean;
  cancelled: boolean;
  useTrash: boolean;
  error: string | null;
  deletedPaths: string[];
  /** 真正删掉的条目数（文件 + 目录）。 */
  deletedItems?: number;
  /** 真正释放的字节数。 */
  freedBytes?: number;
  /** 预扫描得到的条目总数。 */
  totalItems?: number;
  /** 预扫描得到的文件数。 */
  files?: number;
  /** 预扫描得到的目录数。 */
  directories?: number;
  /** 预扫描得到的总字节。 */
  totalBytes?: number;
}

export interface DeleteJobResult {
  success: boolean;
  cancelled: boolean;
  error: string | null;
  deletedPaths: string[];
}

/**
 * 取消请求的结果。
 *
 * FORK-MODIFICATION 2026-11-02：后端不再无脑返回「已取消」。
 *
 * 回收站搬运期没有中断点，那时的取消**不生效**。后端据实回报，界面才不会
 * 显示一个假的「正在取消…」。
 */
export interface DeleteCancelOutcome {
  /** 这次取消有没有被接受。 */
  accepted: boolean;
  /** 作业当前阶段：`scanning` / `deleting`（作业不存在时为空串）。 */
  phase: string;
  /** 没被接受的原因代码（`trashMoveInProgress` / `unknownJob` / …）。 */
  reason: string | null;
}

export interface StartDeleteJobOptions {
  label: string;
  displayPath: string;
  /**
   * FORK-MODIFICATION 2026-11-01：扫描阶段数完时回调一次（整个作业至多一次）。
   *
   * 规模提示的语义不变（「正在删除 N 个项目」是进行中提示，不是删除前的
   * 确认框），只是来源换了：不再是删除前那趟会冻住主线程的同步预扫描，
   * 而是删除作业自己的扫描阶段。
   */
  onScanCompleted?: (summary: DeleteScanSummary) => void;
}

function toastDeleteSuccess(payload: DeleteJobFinishedPayload) {
  // FORK-MODIFICATION 2026-10-22：报**真实删掉的条目数**。
  //
  // 旧实现报的是顶层路径数 —— 删一棵含 25 万个文件的目录树，弹出来的却是
  // 「已删除 1 项」。数字小到让人以为没删掉。
  const deletedCount = payload.deletedItems ?? payload.deletedPaths.length;
  const titleKey = payload.useTrash
    ? 'notifications.trashedNItems'
    : 'notifications.deletedNItems';

  toast.custom(markRaw(ToastStatic), {
    componentProps: {
      data: {
        title: i18n.global.t(titleKey, { totalAmount: deletedCount }),
        description: '',
      },
    },
    duration: 2500,
  });
}

/**
 * FIX 2026-11-03「假失败」之三：把路径归一化成 in-flight 表的键。
 *
 * 带盘符的形态一定是 Windows 路径，Windows 大小写不敏感；其余（POSIX）保持原样，
 * 避免把 `/A` 与 `/a` 认成同一个靶子。分隔符统一成 `/`，并去掉结尾多余的分隔符。
 */
export function normalizeDeletePathKey(path: string): string {
  const unified = path.replace(/[\\/]+$/, '').replace(/\\/g, '/');
  return /^[a-z]:\//i.test(unified) ? unified.toLowerCase() : unified;
}

export const useDeleteJobsStore = defineStore('delete-jobs', () => {
  const eventUnlisteners = shallowRef<UnlistenFn[]>([]);

  /**
   * FIX 2026-11-03「删除失败：拒绝访问。 (os error 5)」的**前端闸门**。
   *
   * 真机日志（2026-11-03 15:06 本地）实测：用户按 Shift+Del 之后，第一个作业要跑
   * 十几秒（11.8s，期间永久删除一条进度都没有 —— 见 delete_tree.rs 同一批修复），
   * 用户以为没反应就连按三次，于是四个作业打到同一个文件上：
   * 第一个真的删掉了，第二、三个在亚毫秒内拿到 `os error 5`，第四个拿到 `os error 2`，
   * 前端各弹一条「删除失败」，而文件其实早就没了 —— 这就是「假失败」。
   *
   * 后端 `DELETE_JOBS` 只按 job_id 建索引，`use-dir-entry-actions.ts` 与本 store
   * 两层都没有任何**按路径**的互斥。补在这里：同一批路径已经在飞时复用它的结果，
   * 不再 invoke 第二个作业。整批路径都要命中同一个 jobId 才复用 —— 只要有一个
   * 路径不在那个作业里，就必须另起作业，否则那个路径会被静默漏掉。
   */
  const inFlightPaths = new Map<string, string>();
  const inFlightResults = new Map<string, Promise<DeleteJobResult>>();
  const completionHandlers = new Map<
    string,
    { resolve: (result: DeleteJobResult) => void }
  >();
  // FORK-MODIFICATION 2026-11-01：扫描阶段的规模回调（每个作业至多一次）。
  const scanHandlers = new Map<string, (summary: DeleteScanSummary) => void>();

  async function ensureEventListeners() {
    if (eventUnlisteners.value.length > 0) {
      return;
    }

    const statusCenterStore = useStatusCenterStore();

    const progressUnlisten = await listen<DeleteJobProgressPayload>(
      'delete-job-progress',
      (event) => {
        const payload = event.payload;
        const existing = statusCenterStore.getOperation(payload.jobId);

        if (existing?.status === 'cancelling') {
          return;
        }

        statusCenterStore.updateOperation(payload.jobId, {
          status: 'in-progress',
          progress: payload.percent,
          message: payload.detail,
          processedCount: payload.processedCount ?? undefined,
          totalCount: payload.totalCount ?? undefined,
          phase: payload.phase,
          deletedItems: payload.deletedItems,
          freedBytes: payload.freedBytes,
          totalBytes: payload.totalBytes ?? undefined,
          itemPercent: payload.itemPercent ?? undefined,
          bytePercent: payload.bytePercent ?? undefined,
          files: payload.files,
          directories: payload.directories,
          currentPath: payload.currentPath,
        });

        // FORK-MODIFICATION 2026-11-01：扫描数完的那一刻把规模递出去。
        //
        // 只在「统计完的那一条」触发一次；扫描途中的进度事件不触发。
        const scanHandler = scanHandlers.get(payload.jobId);

        if (scanHandler && isDeleteScanCompleted(payload)) {
          scanHandlers.delete(payload.jobId);
          scanHandler(toDeleteScanSummary(payload));
        }
      },
    );

    const finishedUnlisten = await listen<DeleteJobFinishedPayload>(
      'delete-job-finished',
      (event) => {
        const payload = event.payload;
        const handlers = completionHandlers.get(payload.jobId);
        completionHandlers.delete(payload.jobId);
        // 作业都结束了，不会再有规模回调 —— 别把闭包留在 map 里。
        scanHandlers.delete(payload.jobId);
        const existingOp = statusCenterStore.getOperation(payload.jobId);

        if (!handlers) {
          if (
            existingOp
            && (
              existingOp.status === 'in-progress'
              || existingOp.status === 'pending'
              || existingOp.status === 'cancelling'
            )
          ) {
            statusCenterStore.removeOperation(payload.jobId);
          }

          return;
        }

        if (payload.cancelled) {
          statusCenterStore.completeOperation(payload.jobId, 'cancelled');
          handlers.resolve({
            success: false,
            cancelled: true,
            error: null,
            deletedPaths: payload.deletedPaths,
          });
        }
        else if (payload.success) {
          statusCenterStore.completeOperation(payload.jobId, 'completed');
          toastDeleteSuccess(payload);
          handlers.resolve({
            success: true,
            cancelled: false,
            error: null,
            deletedPaths: payload.deletedPaths,
          });
        }
        else {
          const fallbackError = payload.useTrash
            ? i18n.global.t('notifications.errorTrashItems')
            : i18n.global.t('notifications.errorDeleteItems');
          const errorText = payload.error ?? fallbackError;
          statusCenterStore.completeOperation(
            payload.jobId,
            'error',
            errorText,
          );
          toast.custom(markRaw(ToastStatic), {
            componentProps: {
              data: {
                title: fallbackError,
                description: payload.error ?? '',
              },
            },
            duration: 5000,
          });
          handlers.resolve({
            success: false,
            cancelled: false,
            error: errorText,
            deletedPaths: payload.deletedPaths,
          });
        }
      },
    );

    eventUnlisteners.value = [progressUnlisten, finishedUnlisten];
  }

  /**
   * 真正起一个删除作业。jobId 由调用方生成并已在 in-flight 表里登记，
   * 这样「查表 → 登记」之间不存在任何 await，重复触发才会被挡住。
   */
  async function runJob(
    jobId: string,
    paths: string[],
    useTrash: boolean,
    options: StartDeleteJobOptions,
  ): Promise<DeleteJobResult> {
    await ensureEventListeners();
    const statusCenterStore = useStatusCenterStore();

    return new Promise((resolve, reject) => {
      completionHandlers.set(jobId, { resolve });

      if (options.onScanCompleted) {
        scanHandlers.set(jobId, options.onScanCompleted);
      }

      statusCenterStore.addOperation({
        id: jobId,
        type: useTrash ? 'deleteTrash' : 'deletePermanent',
        status: 'in-progress',
        label: options.label,
        path: options.displayPath,
        progress: 0,
        // FORK-MODIFICATION 2026-11-01：作业一建起来就标成统计阶段。
        //
        // 以前首个反馈要等后端第一次发进度事件（扫一棵大树时可能是好几秒后），
        // 这段时间界面上只有一条空白作业 —— 用户以为程序死了。现在不等事件,
        // 先把作业按扫描阶段摆出来，状态中心立刻显示「正在统计要删掉多少…」。
        phase: 'scanning',
        sourceDisplayName: sourceDisplayNameFromPaths(paths),
      });

      invoke('start_delete_job', {
        paths,
        useTrash,
        jobId,
      }).catch((error: unknown) => {
        completionHandlers.delete(jobId);
        scanHandlers.delete(jobId);
        statusCenterStore.removeOperation(jobId);
        reject(error);
      });
    });
  }

  async function startJob(
    paths: string[],
    useTrash: boolean,
    options: StartDeleteJobOptions,
  ): Promise<DeleteJobResult> {
    // FIX 2026-11-03「假失败」前端闸门：同一批路径已在飞时复用那个作业。
    //
    // 整批命中同一个 jobId 才复用；有一个路径不在就不 attach，另起作业 ——
    // attach 半个作业会把没覆盖到的路径静默漏掉。
    //
    // ⚠️ 已知副作用：attach 分支不调用第二个调用方的 options.onScanCompleted
    // （作业早于确认框建立，规模回调已属于第一个调用方）。判定可接受 ——
    // 走到 attach 时第二个确认框已经关掉了，没有 UI 在等这个回调。
    const keys = paths.map(normalizeDeletePathKey);
    let attachedJobId: string | null = null;

    for (const key of keys) {
      const jobIdForKey = inFlightPaths.get(key);

      if (!jobIdForKey || inFlightPaths.get(key) !== jobIdForKey) {
        attachedJobId = null;
        break;
      }

      if (!attachedJobId) {
        attachedJobId = jobIdForKey;
      }
      else if (attachedJobId !== jobIdForKey) {
        // 命中了两个不同的在飞作业 —— 没有单一作业覆盖整批，另起。
        attachedJobId = null;
        break;
      }
    }

    if (attachedJobId) {
      const existingResult = inFlightResults.get(attachedJobId);

      if (existingResult) {
        return existingResult;
      }
    }

    // uniqueId() 与 Map.set 都是同步的，全部在任何 await 之前完成。
    const jobId = uniqueId();

    for (const key of keys) {
      inFlightPaths.set(key, jobId);
    }

    function cleanup() {
      for (const key of keys) {
        // 只删仍指向自己的键 —— 极端情况下键可能已被后续作业接管。
        if (inFlightPaths.get(key) === jobId) {
          inFlightPaths.delete(key);
        }
      }

      inFlightResults.delete(jobId);
    }

    const result = runJob(jobId, paths, useTrash, options).finally(cleanup);
    inFlightResults.set(jobId, result);
    return result;
  }

  async function cancelJob(jobId: string): Promise<DeleteCancelOutcome> {
    return await invoke<DeleteCancelOutcome>('cancel_delete_job', { jobId });
  }

  return {
    ensureEventListeners,
    startJob,
    cancelJob,
  };
});
