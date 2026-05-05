// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import type { DirEntry } from '@/types/dir-entry';

const mockConvertFileSrc = vi.hoisted(() => vi.fn((path: string) => `asset://${path}`));
const mockInvoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => mockConvertFileSrc(path),
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { normalizeImageThumbnailMaxDimension, useImageThumbnails } from '../use-image-thumbnails';

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
}

function createDeferred<T>(): Deferred<T> {
  let resolveDeferred!: (value: T) => void;
  const promise = new Promise<T>((resolve) => {
    resolveDeferred = resolve;
  });

  return {
    promise,
    resolve: resolveDeferred,
  };
}

function createImageEntry(fileName: string): DirEntry {
  return {
    name: fileName,
    ext: 'jpg',
    path: `C:/media/${fileName}`,
    size: 1024,
    item_count: null,
    modified_time: 123,
    accessed_time: 123,
    created_time: 123,
    mime: 'image/jpeg',
    is_file: true,
    is_dir: false,
    is_symlink: false,
    is_hidden: false,
  };
}

async function flushThumbnailWork(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

beforeEach(() => {
  mockConvertFileSrc.mockClear();
  mockInvoke.mockReset();
});

describe('normalizeImageThumbnailMaxDimension', () => {
  it('uses the default size when no valid size is provided', () => {
    expect(normalizeImageThumbnailMaxDimension()).toBe(384);
    expect(normalizeImageThumbnailMaxDimension(Number.NaN)).toBe(384);
  });

  it('uses one cache size for every requested thumbnail size', () => {
    expect(normalizeImageThumbnailMaxDimension(1)).toBe(384);
    expect(normalizeImageThumbnailMaxDimension(340)).toBe(384);
    expect(normalizeImageThumbnailMaxDimension(10_000)).toBe(384);
  });
});

describe('useImageThumbnails', () => {
  it('removes queued thumbnail requests when they are cancelled', async () => {
    const pendingRequests: Deferred<string>[] = [];
    mockInvoke.mockImplementation(() => {
      const pendingRequest = createDeferred<string>();
      pendingRequests.push(pendingRequest);
      return pendingRequest.promise;
    });

    const thumbnails = useImageThumbnails();
    const entries = [
      createImageEntry('image-1.jpg'),
      createImageEntry('image-2.jpg'),
      createImageEntry('image-3.jpg'),
      createImageEntry('image-4.jpg'),
    ];

    for (const entry of entries) {
      thumbnails.getImageThumbnail(entry);
    }

    expect(mockInvoke).toHaveBeenCalledTimes(3);

    thumbnails.cancelImageThumbnail(entries[3]);

    pendingRequests.forEach((pendingRequest, requestIndex) => {
      pendingRequest.resolve(`C:/thumb-${requestIndex}.jpg`);
    });
    await flushThumbnailWork();

    const invokedPaths = mockInvoke.mock.calls.map((call) => {
      return (call[1] as { path: string }).path;
    });

    expect(mockInvoke).toHaveBeenCalledTimes(3);
    expect(invokedPaths).not.toContain(entries[3].path);
  });

  it('ignores an active thumbnail result after cancellation', async () => {
    const pendingRequest = createDeferred<string>();
    mockInvoke.mockReturnValue(pendingRequest.promise);

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    thumbnails.getImageThumbnail(entry);
    thumbnails.cancelImageThumbnail(entry);
    pendingRequest.resolve('C:/thumb.jpg');
    await flushThumbnailWork();

    expect(thumbnails.imageThumbnails.value).toEqual({});
    expect(mockConvertFileSrc).not.toHaveBeenCalled();
  });
});
