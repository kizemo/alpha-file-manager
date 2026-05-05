// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { ref } from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import type { DirEntry } from '@/types/dir-entry';

const IMAGE_THUMBNAIL_MAX_DIMENSION = 384;
const MAX_CONCURRENT_IMAGE_THUMBNAILS = 3;
const UNSUPPORTED_IMAGE_THUMBNAIL_EXTENSIONS = new Set(['svg']);

interface ImageThumbnailRequest {
  entry: DirEntry;
  generation: number;
  maxDimension: number;
  thumbnailKey: string;
}

export function normalizeImageThumbnailMaxDimension(maxDimension?: number): number {
  if (!maxDimension || !Number.isFinite(maxDimension)) {
    return IMAGE_THUMBNAIL_MAX_DIMENSION;
  }

  return IMAGE_THUMBNAIL_MAX_DIMENSION;
}

function getImageThumbnailKey(entry: DirEntry, maxDimension: number): string {
  return `${entry.path}|${entry.modified_time}|${entry.size}|${maxDimension}`;
}

function canGenerateImageThumbnail(entry: DirEntry): boolean {
  const extension = entry.ext?.toLowerCase();

  return extension ? !UNSUPPORTED_IMAGE_THUMBNAIL_EXTENSIONS.has(extension) : false;
}

export function useImageThumbnails() {
  const imageThumbnails = ref<Record<string, string>>({});
  const thumbnailQueue: ImageThumbnailRequest[] = [];
  const processingThumbnails = new Set<string>();
  const cancelledThumbnails = new Set<string>();
  const failedThumbnails = new Set<string>();
  let thumbnailGeneration = 0;

  function getProcessingThumbnailKey(request: ImageThumbnailRequest): string {
    return `${request.generation}|${request.thumbnailKey}`;
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
      imageThumbnails.value[nextRequest.thumbnailKey]
      || processingThumbnails.has(getProcessingThumbnailKey(nextRequest))
      || failedThumbnails.has(nextRequest.thumbnailKey)
    ) {
      processNextThumbnail();
      return;
    }

    processingThumbnails.add(getProcessingThumbnailKey(nextRequest));
    processImageThumbnail(nextRequest);
  }

  async function processImageThumbnail(request: ImageThumbnailRequest): Promise<void> {
    const processingKey = getProcessingThumbnailKey(request);

    try {
      const thumbnailPath = await invoke<string>('generate_image_thumbnail', {
        path: request.entry.path,
        modifiedTime: request.entry.modified_time,
        size: request.entry.size,
        maxDimension: request.maxDimension,
      });

      if (request.generation === thumbnailGeneration && !cancelledThumbnails.has(processingKey)) {
        imageThumbnails.value = {
          ...imageThumbnails.value,
          [request.thumbnailKey]: convertFileSrc(thumbnailPath),
        };
      }
    }
    catch {
      if (request.generation === thumbnailGeneration && !cancelledThumbnails.has(processingKey)) {
        failedThumbnails.add(request.thumbnailKey);
      }
    }
    finally {
      processingThumbnails.delete(processingKey);
      cancelledThumbnails.delete(processingKey);
      processNextThumbnail();
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

  function getImageThumbnail(entry: DirEntry, maxDimension?: number): string | undefined {
    if (!canGenerateImageThumbnail(entry)) {
      return undefined;
    }

    const normalizedMaxDimension = normalizeImageThumbnailMaxDimension(maxDimension);
    const thumbnailKey = getImageThumbnailKey(entry, normalizedMaxDimension);
    const cachedThumbnail = imageThumbnails.value[thumbnailKey];
    const processingKey = `${thumbnailGeneration}|${thumbnailKey}`;

    if (cachedThumbnail || failedThumbnails.has(thumbnailKey)) {
      return cachedThumbnail;
    }

    if (processingThumbnails.has(processingKey)) {
      cancelledThumbnails.delete(processingKey);
      return undefined;
    }

    enqueueImageThumbnail(entry, normalizedMaxDimension, thumbnailKey);

    return undefined;
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
  }

  function clearThumbnails() {
    thumbnailGeneration += 1;
    imageThumbnails.value = {};
    thumbnailQueue.length = 0;
    cancelledThumbnails.clear();
    failedThumbnails.clear();
  }

  return {
    imageThumbnails,
    getImageThumbnail,
    cancelImageThumbnail,
    clearThumbnails,
  };
}
