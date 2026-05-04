// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { ref } from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import type { DirEntry } from '@/types/dir-entry';

const MAX_CONCURRENT_THUMBNAILS = 3;
const VIDEO_THUMBNAIL_SIZE = {
  width: 384,
  height: 271,
};

export interface VideoThumbnailSize {
  width: number;
  height: number;
}

interface VideoThumbnailRequest {
  entry: DirEntry;
  generation: number;
  thumbnailKey: string;
  targetSize: VideoThumbnailSize;
}

export function normalizeVideoThumbnailSize(): VideoThumbnailSize {
  return {
    width: VIDEO_THUMBNAIL_SIZE.width,
    height: VIDEO_THUMBNAIL_SIZE.height,
  };
}

function getVideoThumbnailKey(entry: DirEntry, targetSize: VideoThumbnailSize): string {
  return `${entry.path}|${entry.modified_time}|${entry.size}|${targetSize.width}x${targetSize.height}`;
}

function getProcessingVideoThumbnailKey(thumbnailKey: string, generation: number): string {
  return `${generation}|${thumbnailKey}`;
}

function getCoverDrawRect(
  sourceWidth: number,
  sourceHeight: number,
  targetWidth: number,
  targetHeight: number,
) {
  const sourceAspectRatio = sourceWidth / sourceHeight;
  const targetAspectRatio = targetWidth / targetHeight;

  if (sourceAspectRatio > targetAspectRatio) {
    const drawHeight = targetHeight;
    const drawWidth = drawHeight * sourceAspectRatio;

    return {
      drawX: (targetWidth - drawWidth) / 2,
      drawY: 0,
      drawWidth,
      drawHeight,
    };
  }

  const drawWidth = targetWidth;
  const drawHeight = drawWidth / sourceAspectRatio;

  return {
    drawX: 0,
    drawY: (targetHeight - drawHeight) / 2,
    drawWidth,
    drawHeight,
  };
}

export function useVideoThumbnails() {
  const videoThumbnails = ref<Record<string, string>>({});
  const thumbnailQueue: VideoThumbnailRequest[] = [];
  const processingThumbnails = new Set<string>();
  const failedThumbnails = new Set<string>();
  let thumbnailGeneration = 0;

  function createVideoThumbnailDataUrl(request: VideoThumbnailRequest): Promise<string> {
    return new Promise((resolve) => {
      const video = document.createElement('video');
      video.preload = 'metadata';
      video.muted = true;
      video.playsInline = true;
      video.crossOrigin = 'anonymous';

      function cleanup() {
        video.src = '';
        video.remove();
      }

      video.onloadeddata = () => {
        video.currentTime = Math.min(1, video.duration * 0.1);
      };

      video.onseeked = () => {
        const canvas = document.createElement('canvas');
        canvas.width = request.targetSize.width;
        canvas.height = request.targetSize.height;
        const canvasContext = canvas.getContext('2d');

        if (canvasContext && video.videoWidth > 0 && video.videoHeight > 0) {
          const drawRect = getCoverDrawRect(
            video.videoWidth,
            video.videoHeight,
            canvas.width,
            canvas.height,
          );
          canvasContext.drawImage(
            video,
            drawRect.drawX,
            drawRect.drawY,
            drawRect.drawWidth,
            drawRect.drawHeight,
          );
          resolve(canvas.toDataURL('image/jpeg', 0.8));
        }
        else {
          resolve('');
        }

        cleanup();
      };

      video.onerror = () => {
        resolve('');
        cleanup();
      };

      video.src = convertFileSrc(request.entry.path);
    });
  }

  async function getCachedVideoThumbnailPath(request: VideoThumbnailRequest): Promise<string | undefined> {
    const thumbnailPath = await invoke<string | null>('get_cached_video_thumbnail', {
      path: request.entry.path,
      modifiedTime: request.entry.modified_time,
      size: request.entry.size,
      width: request.targetSize.width,
      height: request.targetSize.height,
    });

    return thumbnailPath ? convertFileSrc(thumbnailPath) : undefined;
  }

  async function cacheVideoThumbnail(request: VideoThumbnailRequest, thumbnailDataUrl: string): Promise<string> {
    const thumbnailPath = await invoke<string>('cache_video_thumbnail', {
      path: request.entry.path,
      modifiedTime: request.entry.modified_time,
      size: request.entry.size,
      width: request.targetSize.width,
      height: request.targetSize.height,
      thumbnailDataUrl,
    });

    return convertFileSrc(thumbnailPath);
  }

  async function processVideoThumbnail(request: VideoThumbnailRequest): Promise<void> {
    try {
      let cachedThumbnail: string | undefined;

      try {
        cachedThumbnail = await getCachedVideoThumbnailPath(request);
      }
      catch {
        cachedThumbnail = undefined;
      }

      if (cachedThumbnail) {
        if (request.generation === thumbnailGeneration) {
          videoThumbnails.value = {
            ...videoThumbnails.value,
            [request.thumbnailKey]: cachedThumbnail,
          };
        }

        return;
      }

      const thumbnailDataUrl = await createVideoThumbnailDataUrl(request);

      if (!thumbnailDataUrl) {
        if (request.generation === thumbnailGeneration) {
          failedThumbnails.add(request.thumbnailKey);
        }

        return;
      }

      let thumbnailSrc = thumbnailDataUrl;

      try {
        thumbnailSrc = await cacheVideoThumbnail(request, thumbnailDataUrl);
      }
      catch {
        thumbnailSrc = thumbnailDataUrl;
      }

      if (request.generation === thumbnailGeneration) {
        videoThumbnails.value = {
          ...videoThumbnails.value,
          [request.thumbnailKey]: thumbnailSrc,
        };
      }
    }
    catch {
      if (request.generation === thumbnailGeneration) {
        failedThumbnails.add(request.thumbnailKey);
      }
    }
    finally {
      processingThumbnails.delete(getProcessingVideoThumbnailKey(request.thumbnailKey, request.generation));
      processNextThumbnail();
    }
  }

  function processNextThumbnail() {
    if (processingThumbnails.size >= MAX_CONCURRENT_THUMBNAILS) {
      return;
    }

    const nextRequest = thumbnailQueue.shift();

    if (!nextRequest) {
      return;
    }

    if (
      videoThumbnails.value[nextRequest.thumbnailKey]
      || processingThumbnails.has(getProcessingVideoThumbnailKey(nextRequest.thumbnailKey, nextRequest.generation))
      || failedThumbnails.has(nextRequest.thumbnailKey)
    ) {
      processNextThumbnail();
      return;
    }

    processingThumbnails.add(getProcessingVideoThumbnailKey(nextRequest.thumbnailKey, nextRequest.generation));
    processVideoThumbnail(nextRequest);
  }

  function enqueueVideoThumbnail(entry: DirEntry) {
    const normalizedTargetSize = normalizeVideoThumbnailSize();
    const thumbnailKey = getVideoThumbnailKey(entry, normalizedTargetSize);
    const requestGeneration = thumbnailGeneration;
    const processingKey = getProcessingVideoThumbnailKey(thumbnailKey, requestGeneration);

    if (
      videoThumbnails.value[thumbnailKey]
      || processingThumbnails.has(processingKey)
      || failedThumbnails.has(thumbnailKey)
      || thumbnailQueue.some(request => request.thumbnailKey === thumbnailKey)
    ) {
      return;
    }

    thumbnailQueue.push({
      entry,
      generation: requestGeneration,
      thumbnailKey,
      targetSize: normalizedTargetSize,
    });
    processNextThumbnail();
  }

  function getVideoThumbnail(entry: DirEntry): string | undefined {
    const normalizedTargetSize = normalizeVideoThumbnailSize();
    const thumbnailKey = getVideoThumbnailKey(entry, normalizedTargetSize);
    const cached = videoThumbnails.value[thumbnailKey];
    const processingKey = getProcessingVideoThumbnailKey(thumbnailKey, thumbnailGeneration);

    if (!cached && !processingThumbnails.has(processingKey) && !failedThumbnails.has(thumbnailKey)) {
      enqueueVideoThumbnail(entry);
    }

    return cached;
  }

  function clearThumbnails() {
    thumbnailGeneration += 1;
    videoThumbnails.value = {};
    thumbnailQueue.length = 0;
    failedThumbnails.clear();
  }

  return {
    videoThumbnails,
    getVideoThumbnail,
    clearThumbnails,
  };
}
