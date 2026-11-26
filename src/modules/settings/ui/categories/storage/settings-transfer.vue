<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import {
  CircleAlertIcon, DownloadIcon, LoaderCircleIcon, SettingsIcon, UploadIcon,
} from '@lucide/vue';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { toast } from '@/components/ui/toaster';
import { SettingsItem } from '@/modules/settings';
import { useUserPathsStore } from '@/stores/storage/user-paths';
import { useUserSettingsStore } from '@/stores/storage/user-settings';
import { useUserStatsStore } from '@/stores/storage/user-stats';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import {
  buildSettingsTransferBundle,
  mergeImportedRemoteConnections,
  parseSettingsTransferBundle,
} from '@/modules/settings/utils/settings-transfer';
import type { SettingsTransferBundle } from '@/modules/settings/utils/settings-transfer';

const { t } = useI18n();

const userPathsStore = useUserPathsStore();
const userSettingsStore = useUserSettingsStore();
const userStatsStore = useUserStatsStore();
const remoteConnectionsStore = useRemoteConnectionsStore();

const isExporting = ref(false);
const isImporting = ref(false);
const isApplying = ref(false);
const isConfirmDialogOpen = ref(false);
const pendingBundle = ref<SettingsTransferBundle | null>(null);
const errorMessage = ref<string | null>(null);
const needsCredentialCount = ref(0);

const pendingSummary = computed(() => {
  const bundle = pendingBundle.value;

  if (!bundle) {
    return '';
  }

  return [
    `favorites: ${bundle.favorites.length}`,
    `connections: ${bundle.remoteConnections.length}`,
    `settings: ${Object.keys(bundle.settings).length}`,
  ].join(' · ');
});

/**
 * 读一份盘上 JSON。
 *
 * **读不到（文件不存在）返回 undefined，不算错** —— 首次运行时用户数据文件
 * 可能还没建出来，导出「当前已有的东西」即可。但**解析失败要抛出去**：
 * 那说明文件存在却被写坏了，静默当成空对象等于悄悄丢了一整份配置。
 */
async function readJsonFile(path: string): Promise<unknown> {
  if (!path) {
    return undefined;
  }

  let text: string;

  try {
    const bytes = await invoke<number[]>('read_file_binary', { path });
    text = new TextDecoder().decode(new Uint8Array(bytes));
  }
  catch {
    return undefined;
  }

  if (!text.trim()) {
    return undefined;
  }

  return JSON.parse(text);
}

async function handleExport() {
  isExporting.value = true;
  errorMessage.value = null;

  try {
    const targetPath = await saveDialog({
      defaultPath: 'alpha-file-manager-settings.json',
      filters: [{
        name: 'JSON',
        extensions: ['json'],
      }],
    });

    if (!targetPath) {
      return;
    }

    const customPaths = userPathsStore.customPaths;

    const [settings, stats, remoteConnections] = await Promise.all([
      readJsonFile(customPaths.appUserDataSettingsPath),
      readJsonFile(customPaths.appUserDataStatsPath),
      readJsonFile(customPaths.appUserDataRemoteConnectionsPath),
    ]);

    const bundle = buildSettingsTransferBundle({
      settings,
      stats,
      remoteConnections,
    });
    const text = JSON.stringify(bundle, null, 2);

    await invoke('write_file_binary', {
      path: targetPath,
      data: Array.from(new TextEncoder().encode(text)),
    });

    toast.success(t('settings.storage.settingsTransfer.exportSuccess'));
  }
  catch (error) {
    errorMessage.value = String(error);
    toast.error(t('settings.storage.settingsTransfer.exportFailed'));
  }
  finally {
    isExporting.value = false;
  }
}

async function handlePickImportFile() {
  isImporting.value = true;
  errorMessage.value = null;

  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{
        name: 'JSON',
        extensions: ['json'],
      }],
    });

    if (typeof selected !== 'string' || !selected) {
      return;
    }

    const bytes = await invoke<number[]>('read_file_binary', { path: selected });
    const text = new TextDecoder().decode(new Uint8Array(bytes));

    let parsed: unknown;

    try {
      parsed = JSON.parse(text);
    }
    catch {
      toast.error(t('settings.storage.settingsTransfer.invalidFile'));
      return;
    }

    const result = parseSettingsTransferBundle(parsed);

    if (!result.ok) {
      toast.error(t('settings.storage.settingsTransfer.invalidFile'));
      return;
    }

    pendingBundle.value = result.bundle;
    isConfirmDialogOpen.value = true;
  }
  catch (error) {
    errorMessage.value = String(error);
    toast.error(t('settings.storage.settingsTransfer.importFailed'));
  }
  finally {
    isImporting.value = false;
  }
}

async function applyImport() {
  const bundle = pendingBundle.value;

  if (!bundle) {
    return;
  }

  isApplying.value = true;
  errorMessage.value = null;

  try {
    await userSettingsStore.importUserSettingsEntries(bundle.settings);

    await userStatsStore.replaceFavoritesForImport({
      favorites: bundle.favorites,
      favoriteCategories: bundle.favoriteCategories,
      tags: bundle.tags,
      taggedItems: bundle.taggedItems,
    });

    // **元数据用导入的，凭据用本机的** —— 同 id 的连接沿用本机已存的那份密文，
    // 本机没有的连接凭据为空，导入后要提示用户重新登录。
    const merged = mergeImportedRemoteConnections(
      bundle.remoteConnections,
      remoteConnectionsStore.connections,
    );

    await remoteConnectionsStore.replaceConnectionsForImport(merged.connections);

    needsCredentialCount.value = merged.needsCredential.length;

    pendingBundle.value = null;
    isConfirmDialogOpen.value = false;

    toast.success(t('settings.storage.settingsTransfer.importSuccess'));
  }
  catch (error) {
    errorMessage.value = String(error);
    toast.error(t('settings.storage.settingsTransfer.importFailed'));
  }
  finally {
    isApplying.value = false;
  }
}

function cancelImport() {
  pendingBundle.value = null;
  isConfirmDialogOpen.value = false;
}
</script>

<template>
  <SettingsItem
    :title="t('settings.storage.settingsTransfer.title')"
    :description="t('settings.storage.settingsTransfer.description')"
    :icon="SettingsIcon"
  >
    <div class="settings-transfer__actions">
      <Button
        variant="outline"
        size="sm"
        :disabled="isExporting || isImporting || isApplying"
        @click="handleExport"
      >
        <LoaderCircleIcon
          v-if="isExporting"
          :size="14"
          class="settings-transfer__spinner"
        />
        <DownloadIcon
          v-else
          :size="14"
        />
        {{ isExporting ? t('settings.storage.settingsTransfer.exporting') : t('settings.storage.settingsTransfer.export') }}
      </Button>

      <Button
        variant="outline"
        size="sm"
        :disabled="isExporting || isImporting || isApplying"
        @click="handlePickImportFile"
      >
        <LoaderCircleIcon
          v-if="isImporting"
          :size="14"
          class="settings-transfer__spinner"
        />
        <UploadIcon
          v-else
          :size="14"
        />
        {{ isImporting ? t('settings.storage.settingsTransfer.importing') : t('settings.storage.settingsTransfer.import') }}
      </Button>
    </div>

    <div
      v-if="needsCredentialCount > 0"
      class="settings-transfer__hint"
    >
      <CircleAlertIcon
        :size="14"
        class="settings-transfer__hint-icon"
      />
      <span>{{ t('settings.storage.settingsTransfer.credentialsHint', { count: needsCredentialCount }) }}</span>
    </div>

    <div
      v-if="errorMessage"
      class="settings-transfer__error"
    >
      <CircleAlertIcon
        :size="14"
        class="settings-transfer__error-icon"
      />
      <span>{{ errorMessage }}</span>
    </div>

    <Dialog v-model:open="isConfirmDialogOpen">
      <DialogContent class="settings-transfer__confirm-dialog">
        <DialogHeader>
          <DialogTitle>
            {{ t('settings.storage.settingsTransfer.confirmTitle') }}
          </DialogTitle>
          <DialogDescription>
            {{ t('settings.storage.settingsTransfer.confirmDescription') }}
          </DialogDescription>
        </DialogHeader>

        <p
          v-if="pendingSummary"
          class="settings-transfer__summary"
        >
          {{ pendingSummary }}
        </p>

        <p class="settings-transfer__hint-text">
          {{ t('settings.storage.settingsTransfer.restartHint') }}
        </p>

        <DialogFooter>
          <Button
            variant="outline"
            :disabled="isApplying"
            @click="cancelImport"
          >
            {{ t('cancel') }}
          </Button>
          <Button
            :disabled="isApplying"
            @click="applyImport"
          >
            <LoaderCircleIcon
              v-if="isApplying"
              :size="14"
              class="settings-transfer__spinner"
            />
            {{ t('settings.storage.settingsTransfer.import') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </SettingsItem>
</template>

<style scoped>
.settings-transfer__actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
}

.settings-transfer__spinner {
  animation: settings-transfer-spin 1s linear infinite;
}

@keyframes settings-transfer-spin {
  to {
    transform: rotate(360deg);
  }
}

.settings-transfer__hint {
  display: flex;
  align-items: flex-start;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
  gap: 0.375rem;
}

.settings-transfer__hint-icon,
.settings-transfer__error-icon {
  flex-shrink: 0;
  margin-top: 0.125rem;
}

.settings-transfer__hint-text {
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
}

.settings-transfer__summary {
  color: hsl(var(--foreground));
  font-size: 0.8125rem;
}

.settings-transfer__error {
  display: flex;
  align-items: flex-start;
  color: hsl(var(--destructive));
  font-size: 0.75rem;
  gap: 0.375rem;
}
</style>
