// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { markRaw } from 'vue';
import { useI18n } from 'vue-i18n';
import { toast, ToastStatic } from '@/components/ui/toaster';
import { useStatusCenterStore, type Operation } from '@/stores/runtime/status-center';
import { useArchiveJobsStore } from '@/stores/runtime/archive-jobs';
import { useCopyMoveJobsStore } from '@/stores/runtime/copy-move-jobs';
import { useDeleteJobsStore } from '@/stores/runtime/delete-jobs';
import { useDirSizesStore } from '@/stores/runtime/dir-sizes';
import { isDeleteCancellable } from '@/utils/progress-presentation';

export function useCancelOperation() {
  const { t } = useI18n();
  const statusCenterStore = useStatusCenterStore();
  const archiveJobsStore = useArchiveJobsStore();
  const copyMoveJobsStore = useCopyMoveJobsStore();
  const deleteJobsStore = useDeleteJobsStore();
  const dirSizesStore = useDirSizesStore();

  async function cancelOperation(operation: Operation) {
    if (operation.type === 'dir-size') {
      await dirSizesStore.cancelSize(operation.path);
      return;
    }

    if (operation.status === 'cancelling') {
      return;
    }

    statusCenterStore.updateOperation(operation.id, {
      status: 'cancelling',
      message: t('statusCenter.cancelling'),
    });

    if (operation.type === 'archive') {
      await archiveJobsStore.cancelJob(operation.id);
    }
    else if (operation.type === 'deleteTrash' || operation.type === 'deletePermanent') {
      const outcome = await deleteJobsStore.cancelJob(operation.id);

      // FORK-MODIFICATION 2026-11-02：取消没被接受时，把状态改回来。
      //
      // 回收站搬运期没有中断点，那时的取消**不生效**。旧实现照样显示
      // 「正在取消…」，作业却把整棵树删完（2026-11-02 真机实测：点了取消，
      // 50,051 项一项没少，后端 `cancelled=false`）。后端现在据实回报，
      // 界面据实回退并说明原因 —— 宁可说「取消不了」，也不能骗人。
      if (!outcome.accepted) {
        statusCenterStore.updateOperation(operation.id, {
          status: 'in-progress',
          message: '',
        });

        toast.custom(markRaw(ToastStatic), {
          componentProps: {
            data: {
              title: t('statusCenter.cancelNotAccepted'),
              description: '',
            },
          },
          duration: 4000,
        });
      }
    }
    else if (operation.type === 'copy' || operation.type === 'move') {
      await copyMoveJobsStore.cancelJob(operation.id);
    }
  }

  async function cancelOperations(operations: Operation[]) {
    const cancellableOperations = operations
      .filter(
        operation => operation.status === 'in-progress' || operation.status === 'pending',
      )
      // FORK-MODIFICATION 2026-11-02：别去取消那些取消不了的作业。
      //
      // 回收站搬运期的删除作业按定义就取消不了，把它塞进批量取消只会得到一串
      // 「取消不生效」的提示，白白吵用户。
      .filter((operation) => {
        if (operation.type === 'deleteTrash' || operation.type === 'deletePermanent') {
          return isDeleteCancellable(operation);
        }

        return true;
      });

    await Promise.all(cancellableOperations.map(operation => cancelOperation(operation)));
  }

  return {
    cancelOperation,
    cancelOperations,
  };
}
