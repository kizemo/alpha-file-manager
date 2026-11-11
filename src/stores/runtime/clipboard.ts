// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { DirEntry } from '@/types/dir-entry';
import { i18n } from '@/localization';
import {
  arePathsEquivalent,
  getParentPath,
  getSharedSourceDirectory,
  isDestinationInsideAnySourceDirectory,
} from '@/utils/file-operation-paths';
import { getPathLeafName } from '@/utils/normalize-path';
import { isVirtualLocationPath } from '@/utils/virtual-locations';
import { isTransientClipboardAccessError } from '@/utils/system-clipboard-errors';
// FORK-MODIFICATION 2026-11-11 (batch 9, R1/R2): 系统剪贴板只承载本地路径。
import { shouldSyncPathsToSystemClipboard, clipboardHoldsRemoteItems } from '@/utils/clipboard-routing';
import { useUserSettingsStore } from '@/stores/storage/user-settings';
import {
  hasSameFileClipboardContent,
  shouldShowClipboardUi,
  type ClipboardOrigin,
} from '@/stores/runtime/clipboard-visibility';

export type { ClipboardOrigin } from '@/stores/runtime/clipboard-visibility';
export type ClipboardOperationType = 'copy' | 'move' | '';
export type ConflictResolution = 'replace' | 'skip' | 'auto-rename';

export interface PathResolutionEntry {
  destination_path: string;
  resolution: ConflictResolution;
}

export type ConflictResolutionPayload = {
  perPathResolutions: PathResolutionEntry[];
};

export interface ClipboardState {
  type: ClipboardOperationType;
  items: DirEntry[];
}

export type MovedPathPair = {
  from: string;
  to: string;
};

export interface FileOperationResult {
  success: boolean;
  cancelled?: boolean;
  error?: string;
  copied_count?: number;
  failed_count?: number;
  skipped_count?: number;
  moved_paths?: MovedPathPair[];
  fromStatusCenterJob?: boolean;
}

export interface SystemClipboardImagePasteResult extends FileOperationResult {
  path?: string | null;
}

export interface SystemClipboardImageInfo {
  width: number;
  height: number;
  sizeBytes: number;
  clipboardSequence?: number | null;
  tempPath?: string | null;
  tempVersion?: number | null;
  savedSizeBytes?: number | null;
}

export interface SystemClipboardSavedImage {
  path: string;
  sizeBytes: number;
}

export interface ConflictItem {
  source_path: string;
  source_name: string;
  source_is_dir: boolean;
  source_size: number | null;
  source_modified_ms: number | null;
  destination_path: string;
  destination_is_dir: boolean;
  destination_size: number | null;
  destination_modified_ms: number | null;
  relative_path: string;
}

export const useClipboardStore = defineStore('clipboard', () => {
  const userSettingsStore = useUserSettingsStore();
  const clipboardType = ref<ClipboardOperationType>('');
  const clipboardItems = ref<DirEntry[]>([]);
  const clipboardImage = ref<SystemClipboardImageInfo | null>(null);
  const clipboardOrigin = ref<ClipboardOrigin>('');
  const isOperationInProgress = ref(false);
  let pendingSystemClipboardMutation: Promise<void> | null = null;
  let pendingSystemClipboardImageSave: Promise<SystemClipboardImageInfo | null> | null = null;

  const hasFileItems = computed(() => clipboardItems.value.length > 0);
  const hasImageContent = computed(() => clipboardImage.value !== null);
  const hasItems = computed(() => hasFileItems.value || hasImageContent.value);
  const showClipboardUi = computed(() => shouldShowClipboardUi({
    hasItems: hasItems.value,
    origin: clipboardOrigin.value,
    hasImageContent: hasImageContent.value,
    hasFileItems: hasFileItems.value,
    settings: userSettingsStore.userSettings.clipboard,
  }));
  const itemCount = computed(() => hasImageContent.value ? 1 : clipboardItems.value.length);
  const isCopyOperation = computed(() => clipboardType.value === 'copy');
  const isMoveOperation = computed(() => clipboardType.value === 'move');

  /**
   * Gets the source directory (parent directory of clipboard items)
   * Returns null if items are from different directories
   */
  const sourceDirectory = computed(() =>
    getSharedSourceDirectory(clipboardItems.value.map(item => item.path)),
  );

  function setClipboard(
    type: ClipboardOperationType,
    items: DirEntry[],
    options?: {
      syncToSystemClipboard?: boolean;
    },
  ) {
    clipboardType.value = type;
    clipboardItems.value = items.map(item => ({ ...item }));
    clipboardImage.value = null;
    clipboardOrigin.value = 'internal';

    if (options?.syncToSystemClipboard !== false) {
      void syncToSystemClipboard();
    }
  }

  function addToClipboard(
    type: ClipboardOperationType,
    items: DirEntry[],
    options?: {
      syncToSystemClipboard?: boolean;
    },
  ) {
    clipboardType.value = type;
    clipboardImage.value = null;
    clipboardOrigin.value = 'internal';

    for (const item of items) {
      const itemAlreadyAdded = clipboardItems.value.some(
        clipboardItem => clipboardItem.path === item.path,
      );

      if (!itemAlreadyAdded) {
        clipboardItems.value.push({ ...item });
      }
    }

    if (options?.syncToSystemClipboard !== false) {
      void syncToSystemClipboard();
    }
  }

  function removeFromClipboard(item: DirEntry) {
    clipboardItems.value = clipboardItems.value.filter(
      clipboardItem => clipboardItem.path !== item.path,
    );

    if (clipboardItems.value.length === 0) {
      discardClipboard();
      return;
    }

    void syncToSystemClipboard();
  }

  function clearClipboard() {
    clipboardType.value = '';
    clipboardItems.value = [];
    clipboardImage.value = null;
    clipboardOrigin.value = '';
  }

  function trackSystemClipboardMutation(mutation: Promise<void>): Promise<void> {
    const trackedMutation = mutation.finally(() => {
      if (pendingSystemClipboardMutation === trackedMutation) {
        pendingSystemClipboardMutation = null;
      }
    });

    pendingSystemClipboardMutation = trackedMutation;
    return trackedMutation;
  }

  async function waitForPendingSystemClipboardMutation(): Promise<void> {
    const pendingMutation = pendingSystemClipboardMutation;

    if (pendingMutation) {
      await pendingMutation;
    }
  }

  async function clearSystemClipboardFiles(): Promise<void> {
    await trackSystemClipboardMutation((async () => {
      try {
        await invoke('clear_system_clipboard_files');
      }
      catch (error) {
        console.error('Failed to clear system clipboard files:', error);
      }
    })());
  }

  function discardClipboard() {
    clearClipboard();
    void clearSystemClipboardFiles();
  }

  function snapshotClipboard(): {
    type: ClipboardOperationType;
    items: DirEntry[];
    image: SystemClipboardImageInfo | null;
    origin: ClipboardOrigin;
  } {
    return {
      type: clipboardType.value,
      items: clipboardItems.value.map(item => ({ ...item })),
      image: clipboardImage.value ? { ...clipboardImage.value } : null,
      origin: clipboardOrigin.value,
    };
  }

  function restoreClipboardFromSnapshot(snapshot: ReturnType<typeof snapshotClipboard>) {
    clipboardType.value = snapshot.type;
    clipboardItems.value = snapshot.items;
    clipboardImage.value = snapshot.image ? { ...snapshot.image } : null;
    clipboardOrigin.value = snapshot.origin;
  }

  async function readSystemClipboardFiles(): Promise<{
    paths: string[];
    operation: 'copy' | 'move';
  } | null> {
    try {
      const result = await invoke<{
        paths: string[];
        operation: string;
      }>('read_system_clipboard_files');

      return {
        paths: result.paths,
        operation: result.operation === 'move' ? 'move' : 'copy',
      };
    }
    catch (error) {
      if (!isTransientClipboardAccessError(error)) {
        console.error('Failed to read system clipboard files:', error);
      }

      return null;
    }
  }

  async function readSystemClipboardImageInfo(): Promise<SystemClipboardImageInfo | null> {
    try {
      return await invoke<SystemClipboardImageInfo | null>('read_system_clipboard_image_info');
    }
    catch (error) {
      console.error('Failed to read system clipboard image:', error);
      return null;
    }
  }

  async function saveSystemClipboardImageToTemp(): Promise<SystemClipboardSavedImage | null> {
    try {
      return await invoke<SystemClipboardSavedImage | null>('save_system_clipboard_image_to_temp');
    }
    catch (error) {
      console.error('Failed to save system clipboard image:', error);
      return null;
    }
  }

  function setClipboardImage(
    imageInfo: SystemClipboardImageInfo,
    origin: Exclude<ClipboardOrigin, ''> = 'external',
  ) {
    clipboardType.value = 'copy';
    clipboardItems.value = [];
    clipboardImage.value = { ...imageInfo };
    clipboardOrigin.value = origin;
  }

  async function syncToSystemClipboard(): Promise<void> {
    if (!clipboardItems.value.length || !clipboardType.value) {
      return;
    }

    const paths = clipboardItems.value.map(item => item.path);

    // FORK-MODIFICATION 2026-11-11 (batch 9, R1/R2): 远程条目不得写进系统文件剪贴板。
    // 根因（真机 2026-11-11 13:35 复现）：`quark://…` 被当成「本地文件路径」交给
    // Windows 剪贴板，于是 ① 系统剪贴板从此一直带着一个本机不存在的路径，
    // ② `pasteItems` 读到「系统剪贴板非空」就 entirely 忽略应用内剪贴板 ——
    // 网盘里 Ctrl+C、再在本地窗格 Ctrl+V 永远没有反应（用户报的第 2 条缺陷）。
    // 远程内容的下载落地由 `handleRemotePaste → downloadItemsFromRemote` 负责，
    // 走的是应用内剪贴板，压根不需要系统剪贴板参与。
    if (!shouldSyncPathsToSystemClipboard(paths)) {
      return;
    }

    const operation = clipboardType.value === 'move' ? 'move' : 'copy';

    await trackSystemClipboardMutation((async () => {
      try {
        await invoke('set_system_clipboard_files', {
          paths,
          operation,
        });
      }
      catch (error) {
        console.error('Failed to sync system clipboard:', error);
      }
    })());
  }

  async function pasteSystemClipboardImage(destinationPath: string): Promise<SystemClipboardImagePasteResult> {
    const savedImage = await ensureSystemClipboardImageSaved();

    if (!savedImage?.tempPath) {
      return {
        success: false,
        error: i18n.global.t('fileBrowser.noItemsInClipboard'),
        copied_count: 0,
        failed_count: 1,
        skipped_count: 0,
        path: null,
      };
    }

    return await pasteSavedClipboardImage(savedImage.tempPath, destinationPath);
  }

  async function pasteSavedClipboardImage(
    sourcePath: string,
    destinationPath: string,
  ): Promise<SystemClipboardImagePasteResult> {
    try {
      return await invoke<SystemClipboardImagePasteResult>('paste_saved_clipboard_image', {
        sourcePath,
        destinationPath,
      });
    }
    catch (error) {
      return {
        success: false,
        error: String(error),
        copied_count: 0,
        failed_count: 1,
        skipped_count: 0,
        path: null,
      };
    }
  }

  function createFallbackClipboardEntry(path: string, isDirectory: boolean): DirEntry {
    return {
      name: getPathLeafName(path) || path,
      ext: null,
      path,
      size: 0,
      item_count: null,
      modified_time: 0,
      accessed_time: 0,
      created_time: 0,
      mime: null,
      is_file: !isDirectory,
      is_dir: isDirectory,
      is_symlink: false,
      is_hidden: false,
      link_type: null,
      link_target: null,
      link_status: null,
      hard_link_count: null,
    };
  }

  async function createClipboardEntriesFromPaths(paths: string[]): Promise<DirEntry[]> {
    let pathIsDirectory: boolean[];

    try {
      pathIsDirectory = await invoke<boolean[]>('paths_are_directories', {
        paths,
      });
    }
    catch {
      pathIsDirectory = paths.map(() => false);
    }

    return await Promise.all(paths.map(async (path, pathIndex) => {
      try {
        return await invoke<DirEntry>('get_dir_entry_with_timeout', {
          path,
          timeoutMs: 1000,
        });
      }
      catch {
        return createFallbackClipboardEntry(path, pathIsDirectory[pathIndex] ?? false);
      }
    }));
  }

  async function refreshAfterExternalClipboardConsumption(sourcePaths: string[]): Promise<void> {
    const parentDirectories = [...new Set(sourcePaths.map(path => getParentPath(path)))];

    const { useWorkspacesStore } = await import('@/stores/storage/workspaces');
    const { useDirSizesStore } = await import('@/stores/runtime/dir-sizes');
    const workspacesStore = useWorkspacesStore();
    const dirSizesStore = useDirSizesStore();

    workspacesStore.handleDirectoryContentsChanged(parentDirectories);
    dirSizesStore.invalidate(sourcePaths);
  }

  async function areClipboardSourcePathsMissing(sourcePaths: string[]): Promise<boolean> {
    if (sourcePaths.length === 0) {
      return false;
    }

    const pathExistsResults = await Promise.all(sourcePaths.map(async (sourcePath) => {
      try {
        return await invoke<boolean>('path_exists', {
          path: sourcePath,
        });
      }
      catch {
        return true;
      }
    }));

    return pathExistsResults.every(pathExists => !pathExists);
  }

  async function checkExternalClipboardConsumption(): Promise<boolean> {
    if (!hasFileItems.value || isOperationInProgress.value) {
      return false;
    }

    const sourcePaths = clipboardItems.value.map(item => item.path);

    if (isMoveOperation.value && await areClipboardSourcePathsMissing(sourcePaths)) {
      discardClipboard();
      await refreshAfterExternalClipboardConsumption(sourcePaths);
      return true;
    }

    const systemClipboard = await readSystemClipboardFiles();

    if (!systemClipboard) {
      return false;
    }

    if (systemClipboard.paths.length === 0) {
      const systemClipboardImage = await readSystemClipboardImageInfo();

      if (systemClipboardImage) {
        setClipboardImage(getClipboardImageWithSavedFile(systemClipboardImage));
        return true;
      }

      // FORK-MODIFICATION 2026-11-11（T9 批次 9，判据 ②）：
      // 应用内剪贴板装的是**远程条目**时，「系统剪贴板为空」**不是**「外部粘贴把内容吃掉了」。
      // 远程条目按设计从来不写系统剪贴板（`shouldSyncPathsToSystemClipboard`），
      // 所以系统剪贴板为空是**常态**；旧逻辑在这里 `discardClipboard()` 把远程条目清掉，
      // 用户刚 Ctrl+C 再 Ctrl+V 就只剩「剪贴板中没有项目」。
      if (clipboardHoldsRemoteItems(clipboardItems.value)) {
        return false;
      }

      discardClipboard();
      return true;
    }

    return false;
  }

  async function syncFromSystemClipboard(): Promise<void> {
    await waitForPendingSystemClipboardMutation();

    if (await checkExternalClipboardConsumption()) {
      return;
    }

    const systemClipboard = await readSystemClipboardFiles();

    if (!systemClipboard) {
      return;
    }

    if (systemClipboard.paths.length === 0) {
      const systemClipboardImage = await readSystemClipboardImageInfo();

      if (systemClipboardImage) {
        setClipboardImage(getClipboardImageWithSavedFile(systemClipboardImage));
        return;
      }

      // FORK-MODIFICATION 2026-11-11（T9 批次 9，判据 ②）：同 `checkExternalClipboardConsumption` ——
      // 远程条目从一开始就没进过系统剪贴板，「系统剪贴板为空」不能拿来清掉它。
      if (clipboardHoldsRemoteItems(clipboardItems.value)) {
        return;
      }

      clearClipboard();
      return;
    }

    // FORK-MODIFICATION 2026-11-11（T9 批次 9，判据 ②）：
    // 应用内剪贴板装的是**远程条目**时，不许被系统剪贴板里的残留本地文件覆盖 ——
    // 远程条目根本没进过系统剪贴板，覆盖它等于把用户刚复制的东西弄丢。
    // （旧逻辑：系统剪贴板非空 ⇒ 直接把 `clipboardItems` 换成它，
    //   用户在网盘里 Ctrl+C 之后，系统剪贴板里那次**旧的**本地复制会顶掉它。）
    if (clipboardHoldsRemoteItems(clipboardItems.value)) {
      return;
    }

    if (systemClipboard.operation !== 'copy' && systemClipboard.operation !== 'move') {
      clearClipboard();
      return;
    }

    const systemClipboardItems = await createClipboardEntriesFromPaths(systemClipboard.paths);

    if (systemClipboardItems.length === 0) {
      clearClipboard();
      return;
    }

    const preserveInternalOrigin = clipboardOrigin.value === 'internal'
      && hasSameFileClipboardContent(
        clipboardItems.value,
        clipboardType.value,
        systemClipboard.paths,
        systemClipboard.operation,
      );

    clipboardType.value = systemClipboard.operation;
    clipboardItems.value = systemClipboardItems;
    clipboardImage.value = null;
    clipboardOrigin.value = preserveInternalOrigin ? 'internal' : 'external';
  }

  function canReuseSavedClipboardImage(imageInfo: SystemClipboardImageInfo): boolean {
    const currentImage = clipboardImage.value;

    if (!currentImage?.tempPath) {
      return false;
    }

    return isSameClipboardImage(currentImage, imageInfo);
  }

  function isSameClipboardImage(
    currentImage: SystemClipboardImageInfo,
    imageInfo: SystemClipboardImageInfo,
  ): boolean {
    if (
      currentImage.clipboardSequence !== undefined
      && currentImage.clipboardSequence !== null
      && imageInfo.clipboardSequence !== undefined
      && imageInfo.clipboardSequence !== null
    ) {
      return currentImage.clipboardSequence === imageInfo.clipboardSequence;
    }

    return currentImage.width === imageInfo.width
      && currentImage.height === imageInfo.height
      && currentImage.sizeBytes === imageInfo.sizeBytes;
  }

  function getClipboardImageWithSavedFile(imageInfo: SystemClipboardImageInfo): SystemClipboardImageInfo {
    if (!canReuseSavedClipboardImage(imageInfo)) {
      return imageInfo;
    }

    return {
      ...imageInfo,
      tempPath: clipboardImage.value?.tempPath,
      tempVersion: clipboardImage.value?.tempVersion,
      savedSizeBytes: clipboardImage.value?.savedSizeBytes,
    };
  }

  async function saveSystemClipboardImage(
    imageInfo: SystemClipboardImageInfo,
  ): Promise<SystemClipboardImageInfo> {
    if (canReuseSavedClipboardImage(imageInfo)) {
      return {
        ...imageInfo,
        tempPath: clipboardImage.value?.tempPath,
        tempVersion: clipboardImage.value?.tempVersion,
        savedSizeBytes: clipboardImage.value?.savedSizeBytes,
      };
    }

    const savedImage = await saveSystemClipboardImageToTemp();

    if (!clipboardImage.value || !isSameClipboardImage(clipboardImage.value, imageInfo)) {
      return imageInfo;
    }

    const savedClipboardImage = {
      ...imageInfo,
      tempPath: savedImage?.path ?? null,
      tempVersion: savedImage ? Date.now() : null,
      savedSizeBytes: savedImage?.sizeBytes ?? null,
    };
    clipboardImage.value = savedClipboardImage;

    return savedClipboardImage;
  }

  async function ensureSystemClipboardImageSaved(): Promise<SystemClipboardImageInfo | null> {
    if (!clipboardImage.value) {
      return null;
    }

    if (clipboardImage.value.tempPath) {
      return clipboardImage.value;
    }

    if (pendingSystemClipboardImageSave) {
      await pendingSystemClipboardImageSave;

      if (!clipboardImage.value) {
        return null;
      }

      if (clipboardImage.value.tempPath) {
        return clipboardImage.value;
      }
    }

    const imageInfo = { ...clipboardImage.value };
    pendingSystemClipboardImageSave = saveSystemClipboardImage(imageInfo).finally(() => {
      pendingSystemClipboardImageSave = null;
    });

    return pendingSystemClipboardImageSave;
  }

  function isItemInClipboard(item: DirEntry): boolean {
    return clipboardItems.value.some(
      clipboardItem => clipboardItem.path === item.path,
    );
  }

  /**
   * Checks if the destination is the same as the source directory
   * (only relevant for move operations - can't move items to same location)
   */
  function isSameAsSourceDirectory(destinationPath: string): boolean {
    if (!sourceDirectory.value) {
      return false;
    }

    return arePathsEquivalent(destinationPath, sourceDirectory.value);
  }

  /**
   * Checks if destination is inside one of the clipboard items
   * (can't move/copy a folder into itself)
   */
  function isDestinationInsideClipboardItem(destinationPath: string): boolean {
    return isDestinationInsideAnySourceDirectory(
      destinationPath,
      clipboardItems.value.map(item => item.path),
      clipboardItems.value.map(item => item.is_dir),
    );
  }

  /**
   * Checks if paste operation is allowed for the given destination
   */
  function canPasteTo(destinationPath: string): boolean {
    if (isVirtualLocationPath(destinationPath)) {
      return false;
    }

    if (hasImageContent.value) {
      return Boolean(destinationPath);
    }

    if (!hasItems.value || (!isCopyOperation.value && !isMoveOperation.value)) {
      return false;
    }

    // Can't paste into a folder that's in the clipboard
    if (isDestinationInsideClipboardItem(destinationPath)) {
      return false;
    }

    // For move operations, can't move to the same directory
    if (isMoveOperation.value && isSameAsSourceDirectory(destinationPath)) {
      return false;
    }

    return true;
  }

  async function pasteItems(
    destinationPath: string,
    perPathResolutions?: PathResolutionEntry[],
  ): Promise<FileOperationResult> {
    if (hasImageContent.value) {
      const result = await pasteSystemClipboardImage(destinationPath);

      if (result.success) {
        discardClipboard();
      }

      return result;
    }

    if (!hasItems.value) {
      return {
        success: false,
        error: i18n.global.t('fileBrowser.noItemsInClipboard'),
      };
    }

    if (isDestinationInsideClipboardItem(destinationPath)) {
      return {
        success: false,
        error: i18n.global.t('fileBrowser.cannotPasteIntoItself'),
      };
    }

    if (isMoveOperation.value && isSameAsSourceDirectory(destinationPath)) {
      return {
        success: false,
        error: i18n.global.t('fileBrowser.cannotMoveToSameDirectory'),
      };
    }

    const sourcePaths = clipboardItems.value.map(item => item.path);
    const operationType = clipboardType.value;
    const clipboardSnapshot = snapshotClipboard();
    isOperationInProgress.value = true;

    const displayPath = destinationPath.split(/[/\\]/).pop() ?? destinationPath;

    if (operationType !== 'copy' && operationType !== 'move') {
      isOperationInProgress.value = false;
      return {
        success: false,
        error: i18n.global.t('fileBrowser.invalidClipboardOperation'),
      };
    }

    clearClipboard();

    try {
      const { useCopyMoveJobsStore } = await import('@/stores/runtime/copy-move-jobs');
      const copyMoveJobsStore = useCopyMoveJobsStore();

      const result = await copyMoveJobsStore.startJob(
        operationType,
        sourcePaths,
        destinationPath,
        null,
        perPathResolutions,
        {
          label: operationType === 'copy'
            ? i18n.global.t('notifications.copyingItems')
            : i18n.global.t('notifications.movingItems'),
          displayPath,
        },
      );

      if (!result.success && !clipboardItems.value.length) {
        restoreClipboardFromSnapshot(clipboardSnapshot);
      }

      if (result.success) {
        discardClipboard();
      }

      return result;
    }
    catch (error) {
      if (!clipboardItems.value.length) {
        restoreClipboardFromSnapshot(clipboardSnapshot);
      }

      return {
        success: false,
        error: String(error),
      };
    }
    finally {
      isOperationInProgress.value = false;
    }
  }

  return {
    clipboardType,
    clipboardItems,
    clipboardImage,
    isOperationInProgress,
    hasFileItems,
    hasImageContent,
    hasItems,
    showClipboardUi,
    itemCount,
    isCopyOperation,
    isMoveOperation,
    sourceDirectory,
    setClipboard,
    addToClipboard,
    removeFromClipboard,
    clearClipboard,
    discardClipboard,
    isItemInClipboard,
    isSameAsSourceDirectory,
    isDestinationInsideClipboardItem,
    canPasteTo,
    pasteItems,
    readSystemClipboardFiles,
    readSystemClipboardImageInfo,
    saveSystemClipboardImageToTemp,
    ensureSystemClipboardImageSaved,
    setClipboardImage,
    pasteSystemClipboardImage,
    pasteSavedClipboardImage,
    syncToSystemClipboard,
    syncFromSystemClipboard,
    checkExternalClipboardConsumption,
    clearSystemClipboardFiles,
  };
});
