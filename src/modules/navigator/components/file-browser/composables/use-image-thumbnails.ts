// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { shallowRef, triggerRef } from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import type { DirEntry } from '@/types/dir-entry';
import { isRemotePath } from '@/utils/remote-paths';
import { getFileBrowserEntryKey } from '@/modules/navigator/components/file-browser/utils/file-browser-entry-key';

export const IMAGE_THUMBNAIL_MAX_DIMENSION = 384;
export const IMAGE_THUMBNAIL_DIMENSION_BUCKET = 64;
const IMAGE_THUMBNAIL_PLACEHOLDER_SIZE = 20;
const MAX_CONCURRENT_IMAGE_THUMBNAILS = 3;
const MAX_CONCURRENT_IMAGE_PLACEHOLDERS = 1;
const MAX_IMAGE_THUMBNAIL_FAILURE_ATTEMPTS = 3;
const IMAGE_THUMBNAIL_RETRY_BASE_DELAY_MS = 1500;
const IMAGE_THUMBNAIL_RETRY_MAX_DELAY_MS = 15_000;
const MAX_IMAGE_PLACEHOLDER_SOURCE_FILE_SIZE_BYTES = 16 * 1024 * 1024;
const UNSUPPORTED_IMAGE_THUMBNAIL_EXTENSIONS = new Set(['svg']);
const BROWSER_RENDERABLE_IMAGE_THUMBNAIL_EXTENSIONS = new Set([
  'bmp',
  'gif',
  'ico',
  'jpeg',
  'jpg',
  'png',
  'webp',
]);

interface ImageThumbnailRequest {
  entry: DirEntry;
  generation: number;
  maxDimension: number;
  thumbnailKey: string;
}

interface ImageThumbnailPlaceholderRequest {
  entry: DirEntry;
  generation: number;
  thumbnailKey: string;
}

export function normalizeImageThumbnailMaxDimension(maxDimension?: number): number {
  if (!maxDimension || !Number.isFinite(maxDimension)) {
    return IMAGE_THUMBNAIL_MAX_DIMENSION;
  }

  const roundedDimension = Math.round(maxDimension);

  if (roundedDimension <= 0) {
    return IMAGE_THUMBNAIL_MAX_DIMENSION;
  }

  return Math.min(roundedDimension, IMAGE_THUMBNAIL_MAX_DIMENSION);
}

/**
 * FORK-MODIFICATION 2026-11-21（批次 34 缺陷 A）：把缩略图请求维度量化到 64 的整数倍。
 *
 * 网格卡片的预览尺寸来自 `clientWidth * devicePixelRatio`，会被滚动条出现/消失、圆角与
 * 亚像素布局抖动改写 —— 真机实测同一屏卡片在一分钟内取到 340 / 331 / 255 / 248 四档。
 * 维度是缩略图缓存键的一部分（见 `getImageThumbnailKey`）：每抖一次就换一个 key，
 * 同一张图重下一次，且卡片侧 `watch(imagePreviewSrc)` 无条件清 `loadedImagePreviewSrc`，
 * 而主图样式是 `--final { opacity: 0 }` + `--final-loaded { opacity: 1 }` ⇒ 收尾永远追不上，
 * 网格视图的图片就停在 `opacity: 0` 的透明空洞里（列表视图用固定 48px，不受此影响）。
 *
 * 量化后 340 与 331 落在同一个桶（384），首次请求即终态；磁盘上的 4 档收敛为 2 档。
 */
export function quantizeImageThumbnailMaxDimension(maxDimension: number): number {
  if (!Number.isFinite(maxDimension) || maxDimension <= 0) {
    return IMAGE_THUMBNAIL_DIMENSION_BUCKET;
  }

  const bucketedDimension = Math.ceil(maxDimension / IMAGE_THUMBNAIL_DIMENSION_BUCKET)
    * IMAGE_THUMBNAIL_DIMENSION_BUCKET;

  return Math.min(bucketedDimension, IMAGE_THUMBNAIL_MAX_DIMENSION);
}

function getImageThumbnailKey(entry: DirEntry, maxDimension: number): string {
  return `${getFileBrowserEntryKey(entry)}|${entry.modified_time}|${entry.size}|${maxDimension}`;
}

/**
 * 远程网盘（`sfm://remote/...`）的图片**不能**用服务端回的签名直链当 `<img src>`：
 * 直链要走公网 CDN 且带时效签名，WebView2 里跨源取图实测拿不到像素（批次 32 判定）。
 * 所以远程条目改走 Rust 代理命令 `remote_generate_thumbnail`：
 * 后端取字节 → 解码 → 落本地缓存目录 `remote-image-thumbnails` → 回一个本地文件路径，
 * 前端再 `convertFileSrc` 成 asset URI，与本地缩略图完全同一条渲染路径。
 *
 * `DirEntry.thumbnail_url` 字段保留（后端仍会回），但前端不再把它当图片用。
 */
function canGenerateImageThumbnail(entry: DirEntry): boolean {
  // 目录条目没有可解码的图片，挡在门口省一次必然失败的 IPC。
  if (entry.is_dir) {
    return false;
  }

  const extension = entry.ext?.toLowerCase();

  return extension ? !UNSUPPORTED_IMAGE_THUMBNAIL_EXTENSIONS.has(extension) : false;
}

/**
 * shell namespace 路径（手机 MTP/WPD 等）的判据，必须与 Rust 侧
 * `dir_reader::shell_namespace::is_shell_namespace_path` 同源：`::{...}` 或 `shell:` 前缀。
 */
export function isShellNamespacePath(path: string): boolean {
  const trimmed = path?.trim() ?? '';

  return trimmed.startsWith('::{') || trimmed.toLowerCase().startsWith('shell:');
}

function canGenerateImageThumbnailPlaceholder(entry: DirEntry): boolean {
  // 远程网盘条目的原图字节读不到（`sfm://remote/...` 不是文件系统路径），
  // 拿它做占位图必然失败 ⇒ 远程条目的占位图一律不做。
  if (isRemotePath(entry.path)) {
    return false;
  }

  const extension = entry.ext?.toLowerCase();

  // 前端读不到 shell 条目的原图字节（convertFileSrc 的 asset URI 只服务文件系统），
  // 拿它做占位图必然失败 ⇒ 交给后端的 shell 缩略图通道出图。
  if (isShellNamespacePath(entry.path)) {
    return false;
  }

  if (!extension || entry.size > MAX_IMAGE_PLACEHOLDER_SOURCE_FILE_SIZE_BYTES) {
    return false;
  }

  if (extension === 'gif') {
    return false;
  }

  return BROWSER_RENDERABLE_IMAGE_THUMBNAIL_EXTENSIONS.has(extension);
}

function supportsImageThumbnailPlaceholder(): boolean {
  return typeof document !== 'undefined'
    && typeof fetch === 'function'
    && typeof createImageBitmap === 'function';
}

export function useImageThumbnails() {
  // FORK-MODIFICATION 2026-10-28（内存优化 A2）：缩略图缓存从深响应式 `ref<Record>`
  // 改成 `shallowRef<Map>`。深响应式会让每个 base64 条目都进代理与依赖表，
  // 而原先「`{ ...cache, [key]: value }` 整体重建」在单调增长时是 O(n²) 分配。
  // 现在写入是原地 `Map.set`（O(1)），响应式由写入后的 `triggerRef` 主动触发 ——
  // 依赖 `.value` 的模板与 computed 会照常重新求值。
  const imageThumbnails = shallowRef<Map<string, string>>(new Map());
  const imageThumbnailPlaceholders = shallowRef<Map<string, string>>(new Map());
  const thumbnailQueue: ImageThumbnailRequest[] = [];
  const placeholderQueue: ImageThumbnailPlaceholderRequest[] = [];
  const processingThumbnails = new Set<string>();
  const processingPlaceholders = new Set<string>();
  const cancelledThumbnails = new Set<string>();
  const cancelledPlaceholders = new Set<string>();
  const failedThumbnails = shallowRef<Set<string>>(new Set());
  const failedPlaceholders = shallowRef<Set<string>>(new Set());
  const thumbnailFailureAttempts = new Map<string, number>();
  const thumbnailRetryTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const activePlaceholderAbortControllers = new Map<string, AbortController>();
  let thumbnailGeneration = 0;

  function getProcessingThumbnailKey(request: ImageThumbnailRequest): string {
    return `${request.generation}|${request.thumbnailKey}`;
  }

  function getProcessingPlaceholderKey(request: ImageThumbnailPlaceholderRequest): string {
    return `${request.generation}|${request.thumbnailKey}`;
  }

  function setImageThumbnail(thumbnailKey: string, thumbnailSrc: string): void {
    if (imageThumbnails.value.get(thumbnailKey) === thumbnailSrc) {
      return;
    }

    imageThumbnails.value.set(thumbnailKey, thumbnailSrc);
    clearThumbnailFailureState(thumbnailKey);
    triggerRef(imageThumbnails);
  }

  function setImageThumbnailPlaceholder(thumbnailKey: string, placeholderDataUrl: string): void {
    if (imageThumbnailPlaceholders.value.get(thumbnailKey) === placeholderDataUrl) {
      return;
    }

    imageThumbnailPlaceholders.value.set(thumbnailKey, placeholderDataUrl);
    triggerRef(imageThumbnailPlaceholders);
  }

  /**
   * FORK-MODIFICATION 2026-11-21（批次 34 缺陷 B）：失败不再是终态。
   *
   * 旧实现把 key 永久塞进 `failedThumbnails`，而 `getImageThumbnail` 对失败 key 恒返
   * `undefined` 且不再入队，`clearThumbnails()` 又只在换目录/卸载时跑 ⇒ 远程出现一次
   * `null`（限流 / 超时 / 直链失效都是合法常态）就让这张图在本目录内永久空白。
   * 现在改成有限次退避重试，达到次数上限才真判死，避免对上万张的超大目录无限打请求。
   */
  function markThumbnailFailed(thumbnailKey: string): void {
    const attempts = (thumbnailFailureAttempts.get(thumbnailKey) ?? 0) + 1;
    thumbnailFailureAttempts.set(thumbnailKey, attempts);

    if (!failedThumbnails.value.has(thumbnailKey)) {
      failedThumbnails.value.add(thumbnailKey);
      triggerRef(failedThumbnails);
    }

    if (attempts >= MAX_IMAGE_THUMBNAIL_FAILURE_ATTEMPTS) {
      return;
    }

    scheduleThumbnailRetry(thumbnailKey, attempts);
  }

  function scheduleThumbnailRetry(thumbnailKey: string, attempts: number): void {
    if (thumbnailRetryTimers.has(thumbnailKey)) {
      return;
    }

    const retryDelay = Math.min(
      IMAGE_THUMBNAIL_RETRY_BASE_DELAY_MS * 2 ** (attempts - 1),
      IMAGE_THUMBNAIL_RETRY_MAX_DELAY_MS,
    );
    const retryTimer = setTimeout(() => {
      thumbnailRetryTimers.delete(thumbnailKey);

      if (!failedThumbnails.value.delete(thumbnailKey)) {
        return;
      }

      // 卡片侧在 `getImageThumbnail` 里读这个集合 ⇒ 触发后重新求值即重新入队。
      triggerRef(failedThumbnails);
    }, retryDelay);

    thumbnailRetryTimers.set(thumbnailKey, retryTimer);
  }

  function clearThumbnailFailureState(thumbnailKey: string): void {
    thumbnailFailureAttempts.delete(thumbnailKey);

    const retryTimer = thumbnailRetryTimers.get(thumbnailKey);

    if (retryTimer !== undefined) {
      clearTimeout(retryTimer);
      thumbnailRetryTimers.delete(thumbnailKey);
    }

    if (failedThumbnails.value.delete(thumbnailKey)) {
      triggerRef(failedThumbnails);
    }
  }

  function markPlaceholderFailed(thumbnailKey: string): void {
    if (failedPlaceholders.value.has(thumbnailKey)) {
      return;
    }

    failedPlaceholders.value.add(thumbnailKey);
    triggerRef(failedPlaceholders);
  }

  function processNextThumbnail() {
    if (processingThumbnails.size >= MAX_CONCURRENT_IMAGE_THUMBNAILS) {
      return;
    }

    const nextRequest = thumbnailQueue.shift();

    if (!nextRequest) {
      return;
    }

    if (
      imageThumbnails.value.get(nextRequest.thumbnailKey)
      || processingThumbnails.has(getProcessingThumbnailKey(nextRequest))
      || failedThumbnails.value.has(nextRequest.thumbnailKey)
    ) {
      processNextThumbnail();
      return;
    }

    processingThumbnails.add(getProcessingThumbnailKey(nextRequest));
    processImageThumbnail(nextRequest);
  }

  function processNextPlaceholder() {
    if (processingPlaceholders.size >= MAX_CONCURRENT_IMAGE_PLACEHOLDERS) {
      return;
    }

    const nextRequest = placeholderQueue.shift();

    if (!nextRequest) {
      return;
    }

    if (
      imageThumbnails.value.get(nextRequest.thumbnailKey)
      || imageThumbnailPlaceholders.value.get(nextRequest.thumbnailKey)
      || processingPlaceholders.has(getProcessingPlaceholderKey(nextRequest))
      || failedPlaceholders.value.has(nextRequest.thumbnailKey)
    ) {
      processNextPlaceholder();
      return;
    }

    processingPlaceholders.add(getProcessingPlaceholderKey(nextRequest));
    processImageThumbnailPlaceholder(nextRequest);
  }

  /**
   * 本地与远程分派到两个不同的命令，两边都只回「缓存里的本地图路径」：
   * - 本地：`generate_image_thumbnail`，读文件系统；
   * - 远程：`remote_generate_thumbnail`，由 provider 代取字节后写进独立缓存目录。
   *
   * 远程命令**不传 `size`**：云端列表给的 size 是原图大小，与代取的缩略图字节无关，
   * 缓存键改用 `modified_time` 承担失效（见 `remote/thumbnails.rs`）。
   * 拿不到图时远程返回 `null`（不是报错）：没直链、没权限、超时都是合法常见结果。
   */
  async function requestThumbnailPath(request: ImageThumbnailRequest): Promise<string | null> {
    if (isRemotePath(request.entry.path)) {
      return invoke<string | null>('remote_generate_thumbnail', {
        path: request.entry.path,
        modifiedTime: request.entry.modified_time,
        maxDimension: request.maxDimension,
      });
    }

    return invoke<string>('generate_image_thumbnail', {
      path: request.entry.path,
      modifiedTime: request.entry.modified_time,
      size: request.entry.size,
      maxDimension: request.maxDimension,
    });
  }

  async function processImageThumbnail(request: ImageThumbnailRequest): Promise<void> {
    const processingKey = getProcessingThumbnailKey(request);

    try {
      const thumbnailPath = await requestThumbnailPath(request);

      if (request.generation !== thumbnailGeneration || cancelledThumbnails.has(processingKey)) {
        return;
      }

      if (thumbnailPath) {
        setImageThumbnail(request.thumbnailKey, convertFileSrc(thumbnailPath));
      }
      else {
        // `null` 也要标失败：否则队列与 fallback 都会以为「还在排队」，
        // 界面上就是一直不出图的透明空洞。
        markThumbnailFailed(request.thumbnailKey);
      }
    }
    catch {
      if (request.generation === thumbnailGeneration && !cancelledThumbnails.has(processingKey)) {
        markThumbnailFailed(request.thumbnailKey);
      }
    }
    finally {
      processingThumbnails.delete(processingKey);
      cancelledThumbnails.delete(processingKey);
      processNextThumbnail();
    }
  }

  async function createImageThumbnailPlaceholderDataUrl(
    entry: DirEntry,
    abortSignal: AbortSignal | undefined,
  ): Promise<string> {
    const response = await fetch(convertFileSrc(entry.path), { signal: abortSignal });

    if (!response.ok) {
      return '';
    }

    const imageBlob = await response.blob();
    const imageBitmap = await createImageBitmap(imageBlob, {
      resizeWidth: IMAGE_THUMBNAIL_PLACEHOLDER_SIZE,
      resizeHeight: IMAGE_THUMBNAIL_PLACEHOLDER_SIZE,
      resizeQuality: 'low',
    });

    try {
      const canvas = document.createElement('canvas');
      canvas.width = IMAGE_THUMBNAIL_PLACEHOLDER_SIZE;
      canvas.height = IMAGE_THUMBNAIL_PLACEHOLDER_SIZE;
      const canvasContext = canvas.getContext('2d');

      if (!canvasContext) {
        return '';
      }

      canvasContext.drawImage(imageBitmap, 0, 0, canvas.width, canvas.height);

      return canvas.toDataURL('image/jpeg', 0.35);
    }
    finally {
      imageBitmap.close();
    }
  }

  async function processImageThumbnailPlaceholder(request: ImageThumbnailPlaceholderRequest): Promise<void> {
    const processingKey = getProcessingPlaceholderKey(request);
    const abortController = typeof AbortController === 'undefined' ? undefined : new AbortController();

    if (abortController) {
      activePlaceholderAbortControllers.set(processingKey, abortController);
    }

    try {
      const placeholderDataUrl = await createImageThumbnailPlaceholderDataUrl(
        request.entry,
        abortController?.signal,
      );

      if (
        placeholderDataUrl
        && request.generation === thumbnailGeneration
        && !cancelledPlaceholders.has(processingKey)
        && !imageThumbnails.value.get(request.thumbnailKey)
      ) {
        setImageThumbnailPlaceholder(request.thumbnailKey, placeholderDataUrl);
      }
      else if (
        !placeholderDataUrl
        && request.generation === thumbnailGeneration
        && !cancelledPlaceholders.has(processingKey)
      ) {
        markPlaceholderFailed(request.thumbnailKey);
      }
    }
    catch {
      if (request.generation === thumbnailGeneration && !cancelledPlaceholders.has(processingKey)) {
        markPlaceholderFailed(request.thumbnailKey);
      }
    }
    finally {
      processingPlaceholders.delete(processingKey);
      cancelledPlaceholders.delete(processingKey);
      activePlaceholderAbortControllers.delete(processingKey);
      processNextPlaceholder();
    }
  }

  function enqueueImageThumbnail(entry: DirEntry, maxDimension: number, thumbnailKey: string) {
    if (thumbnailQueue.some(request => request.thumbnailKey === thumbnailKey)) {
      return;
    }

    thumbnailQueue.push({
      entry,
      generation: thumbnailGeneration,
      maxDimension,
      thumbnailKey,
    });
    processNextThumbnail();
  }

  function enqueueImageThumbnailPlaceholder(entry: DirEntry, thumbnailKey: string) {
    if (placeholderQueue.some(request => request.thumbnailKey === thumbnailKey)) {
      return;
    }

    placeholderQueue.push({
      entry,
      generation: thumbnailGeneration,
      thumbnailKey,
    });
    processNextPlaceholder();
  }

  function getImageThumbnail(entry: DirEntry, maxDimension?: number): string | undefined {
    if (!canGenerateImageThumbnail(entry)) {
      return undefined;
    }

    const normalizedMaxDimension = normalizeImageThumbnailMaxDimension(maxDimension);
    const thumbnailKey = getImageThumbnailKey(entry, normalizedMaxDimension);
    const cachedThumbnail = imageThumbnails.value.get(thumbnailKey);
    const processingKey = `${thumbnailGeneration}|${thumbnailKey}`;

    if (cachedThumbnail || failedThumbnails.value.has(thumbnailKey)) {
      return cachedThumbnail;
    }

    if (processingThumbnails.has(processingKey)) {
      cancelledThumbnails.delete(processingKey);
      return undefined;
    }

    enqueueImageThumbnail(entry, normalizedMaxDimension, thumbnailKey);

    return undefined;
  }

  function getImageThumbnailPlaceholder(entry: DirEntry, maxDimension?: number): string | undefined {
    if (!canGenerateImageThumbnailPlaceholder(entry) || !supportsImageThumbnailPlaceholder()) {
      return undefined;
    }

    const normalizedMaxDimension = normalizeImageThumbnailMaxDimension(maxDimension);
    const thumbnailKey = getImageThumbnailKey(entry, normalizedMaxDimension);
    const cachedThumbnail = imageThumbnails.value.get(thumbnailKey);
    const cachedPlaceholder = imageThumbnailPlaceholders.value.get(thumbnailKey);
    const processingKey = `${thumbnailGeneration}|${thumbnailKey}`;

    if (cachedThumbnail || cachedPlaceholder || failedPlaceholders.value.has(thumbnailKey)) {
      return cachedPlaceholder;
    }

    if (processingPlaceholders.has(processingKey)) {
      if (cancelledPlaceholders.has(processingKey)) {
        enqueueImageThumbnailPlaceholder(entry, thumbnailKey);
      }

      return undefined;
    }

    enqueueImageThumbnailPlaceholder(entry, thumbnailKey);

    return undefined;
  }

  function shouldShowImageThumbnailFallback(entry: DirEntry, maxDimension?: number): boolean {
    if (!canGenerateImageThumbnail(entry)) {
      return true;
    }

    const normalizedMaxDimension = normalizeImageThumbnailMaxDimension(maxDimension);
    const thumbnailKey = getImageThumbnailKey(entry, normalizedMaxDimension);

    // 远程条目在出图之前一律显示兜底图标。
    // 远程没有占位图通道（原图字节读不到），等待期若返回 false 就是一片透明空洞；
    // 真机像素判据（批次 32 的 J1）量的是「卡片上有内容」，空洞会被直接判成没图。
    if (isRemotePath(entry.path)) {
      return !imageThumbnails.value.get(thumbnailKey);
    }

    return failedThumbnails.value.has(thumbnailKey) || failedPlaceholders.value.has(thumbnailKey);
  }

  function cancelImageThumbnail(entry: DirEntry, maxDimension?: number): void {
    if (!canGenerateImageThumbnail(entry)) {
      return;
    }

    const normalizedMaxDimension = normalizeImageThumbnailMaxDimension(maxDimension);
    const thumbnailKey = getImageThumbnailKey(entry, normalizedMaxDimension);
    const processingKey = `${thumbnailGeneration}|${thumbnailKey}`;

    for (let requestIndex = thumbnailQueue.length - 1; requestIndex >= 0; requestIndex -= 1) {
      const request = thumbnailQueue[requestIndex];

      if (request.thumbnailKey === thumbnailKey && request.generation === thumbnailGeneration) {
        thumbnailQueue.splice(requestIndex, 1);
      }
    }

    if (processingThumbnails.has(processingKey)) {
      cancelledThumbnails.add(processingKey);
    }

    for (let requestIndex = placeholderQueue.length - 1; requestIndex >= 0; requestIndex -= 1) {
      const request = placeholderQueue[requestIndex];

      if (request.thumbnailKey === thumbnailKey && request.generation === thumbnailGeneration) {
        placeholderQueue.splice(requestIndex, 1);
      }
    }

    if (processingPlaceholders.has(processingKey)) {
      cancelledPlaceholders.add(processingKey);
      activePlaceholderAbortControllers.get(processingKey)?.abort();
    }
  }

  function clearThumbnails() {
    thumbnailGeneration += 1;
    imageThumbnails.value.clear();
    imageThumbnailPlaceholders.value.clear();
    thumbnailQueue.length = 0;
    placeholderQueue.length = 0;
    cancelledThumbnails.clear();
    cancelledPlaceholders.clear();
    failedThumbnails.value.clear();
    failedPlaceholders.value.clear();
    // FORK-MODIFICATION 2026-11-21（批次 34 缺陷 C）：在途请求的 processing 标记必须一起清。
    // 旧实现只清队列与集合，`processingThumbnails` / `processingPlaceholders` 要等在途
    // promise 的 `finally` 才释放；只要有一个请求不 settle（IPC 无响应、WebView 挂起），
    // 并发预算就被永久吃掉，`MAX_CONCURRENT_IMAGE_THUMBNAILS` 用满后整条队列停摆 ——
    // 真机现象就是「前 26 张出图后长时间零新增」。旧键里带旧 generation，
    // 后继 `finally` 再 delete 同一个旧键是无害的（新 generation 的键与之不同）。
    processingThumbnails.clear();
    processingPlaceholders.clear();

    for (const retryTimer of thumbnailRetryTimers.values()) {
      clearTimeout(retryTimer);
    }

    thumbnailRetryTimers.clear();
    thumbnailFailureAttempts.clear();
    triggerRef(imageThumbnails);
    triggerRef(imageThumbnailPlaceholders);
    triggerRef(failedThumbnails);
    triggerRef(failedPlaceholders);

    for (const abortController of activePlaceholderAbortControllers.values()) {
      abortController.abort();
    }

    activePlaceholderAbortControllers.clear();
  }

  return {
    imageThumbnails,
    imageThumbnailPlaceholders,
    getImageThumbnail,
    getImageThumbnailPlaceholder,
    shouldShowImageThumbnailFallback,
    cancelImageThumbnail,
    clearThumbnails,
  };
}
