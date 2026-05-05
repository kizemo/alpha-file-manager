// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import type { DirEntry } from '@/types/dir-entry';

const mockConvertFileSrc = vi.hoisted(() => vi.fn((path: string) => `asset://${path}`));
const mockInvoke = vi.hoisted(() => vi.fn());
const mockFetch = vi.fn();
const mockCreateImageBitmap = vi.fn();
const mockDrawImage = vi.fn();
const mockCloseImageBitmap = vi.fn();
const PLACEHOLDER_DATA_URL = 'data:image/jpeg;base64,placeholder';

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
  for (let flushIndex = 0; flushIndex < 10; flushIndex += 1) {
    await Promise.resolve();
  }
}

function createFetchResponse(blob: Blob): Response {
  return {
    ok: true,
    blob: () => Promise.resolve(blob),
  } as Response;
}

function setupImagePlaceholderMocks(): void {
  mockFetch.mockResolvedValue(createFetchResponse(new Blob(['image'])));
  mockCreateImageBitmap.mockResolvedValue({
    close: mockCloseImageBitmap,
  } as unknown as ImageBitmap);
  vi.stubGlobal('fetch', mockFetch);
  vi.stubGlobal('createImageBitmap', mockCreateImageBitmap);
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => {
    return {
      drawImage: mockDrawImage,
    } as unknown as CanvasRenderingContext2D;
  });
  vi.spyOn(HTMLCanvasElement.prototype, 'toDataURL').mockReturnValue(PLACEHOLDER_DATA_URL);
}

beforeEach(() => {
  mockConvertFileSrc.mockClear();
  mockInvoke.mockReset();
  mockFetch.mockReset();
  mockCreateImageBitmap.mockReset();
  mockDrawImage.mockReset();
  mockCloseImageBitmap.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
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

  it('creates a 20x20 placeholder with createImageBitmap when supported', async () => {
    setupImagePlaceholderMocks();

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    expect(thumbnails.getImageThumbnailPlaceholder(entry)).toBeUndefined();
    await flushThumbnailWork();

    expect(thumbnails.getImageThumbnailPlaceholder(entry)).toBe(PLACEHOLDER_DATA_URL);
    expect(mockFetch).toHaveBeenCalledWith(`asset://${entry.path}`, expect.any(Object));
    expect(mockCreateImageBitmap).toHaveBeenCalledWith(expect.any(Blob), {
      resizeWidth: 20,
      resizeHeight: 20,
      resizeQuality: 'low',
    });
    expect(mockDrawImage).toHaveBeenCalled();
    expect(mockCloseImageBitmap).toHaveBeenCalled();
  });

  it('keeps the icon fallback when createImageBitmap is unavailable', async () => {
    vi.stubGlobal('fetch', mockFetch);
    vi.stubGlobal('createImageBitmap', undefined);

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    expect(thumbnails.getImageThumbnailPlaceholder(entry)).toBeUndefined();
    await flushThumbnailWork();

    expect(mockFetch).not.toHaveBeenCalled();
    expect(thumbnails.imageThumbnailPlaceholders.value).toEqual({});
  });

  it('does not generate placeholders for large image files', async () => {
    setupImagePlaceholderMocks();

    const thumbnails = useImageThumbnails();
    const entry = {
      ...createImageEntry('large-image.jpg'),
      size: 17 * 1024 * 1024,
    };

    expect(thumbnails.getImageThumbnailPlaceholder(entry)).toBeUndefined();
    await flushThumbnailWork();

    expect(mockFetch).not.toHaveBeenCalled();
    expect(thumbnails.imageThumbnailPlaceholders.value).toEqual({});
  });

  it('removes queued placeholder requests when they are cancelled', async () => {
    setupImagePlaceholderMocks();
    const pendingResponse = createDeferred<Response>();
    mockFetch.mockReturnValue(pendingResponse.promise);

    const thumbnails = useImageThumbnails();
    const firstEntry = createImageEntry('image-1.jpg');
    const secondEntry = createImageEntry('image-2.jpg');

    thumbnails.getImageThumbnailPlaceholder(firstEntry);
    thumbnails.getImageThumbnailPlaceholder(secondEntry);
    thumbnails.cancelImageThumbnail(secondEntry);
    pendingResponse.resolve(createFetchResponse(new Blob(['image'])));
    await flushThumbnailWork();

    expect(mockFetch).toHaveBeenCalledTimes(1);
    expect(Object.keys(thumbnails.imageThumbnailPlaceholders.value)).toHaveLength(1);
  });
});
