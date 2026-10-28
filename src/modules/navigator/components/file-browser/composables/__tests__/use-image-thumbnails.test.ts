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
import { computed, nextTick } from 'vue';
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
  reject: (reason?: unknown) => void;
}

function createDeferred<T>(): Deferred<T> {
  let resolveDeferred!: (value: T) => void;
  let rejectDeferred!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolve, reject) => {
    resolveDeferred = resolve;
    rejectDeferred = reject;
  });

  return {
    promise,
    resolve: resolveDeferred,
    reject: rejectDeferred,
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

// 一次 macrotask 就能排空整条微任务链：每张缩略图的生成都是 `await invoke(...)`
// 串起来的微任务链，中途还会继续派发队列里的下一张。
async function flushAllThumbnailWork(): Promise<void> {
  await new Promise<void>((resolve) => {
    setTimeout(resolve, 0);
  });
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
    expect(normalizeImageThumbnailMaxDimension(-10)).toBe(384);
    expect(normalizeImageThumbnailMaxDimension(0.4)).toBe(384);
  });

  it('clamps requested thumbnail sizes to the backend cap', () => {
    expect(normalizeImageThumbnailMaxDimension(1)).toBe(1);
    expect(normalizeImageThumbnailMaxDimension(340)).toBe(340);
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

    expect(thumbnails.imageThumbnails.value.size).toBe(0);
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

  it('skips placeholder generation when createImageBitmap is unavailable', async () => {
    vi.stubGlobal('fetch', mockFetch);
    vi.stubGlobal('createImageBitmap', undefined);

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    expect(thumbnails.getImageThumbnailPlaceholder(entry)).toBeUndefined();
    await flushThumbnailWork();

    expect(mockFetch).not.toHaveBeenCalled();
    expect(thumbnails.imageThumbnailPlaceholders.value.size).toBe(0);
  });

  it('does not show the image fallback while placeholder generation is pending', async () => {
    setupImagePlaceholderMocks();
    const pendingResponse = createDeferred<Response>();
    mockFetch.mockReturnValue(pendingResponse.promise);

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    thumbnails.getImageThumbnailPlaceholder(entry);

    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(false);

    pendingResponse.resolve(createFetchResponse(new Blob(['image'])));
    await flushThumbnailWork();

    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(false);
  });

  it('shows the image fallback when thumbnail generation is unsupported', () => {
    const thumbnails = useImageThumbnails();
    const entry = {
      ...createImageEntry('image'),
      ext: '',
    };

    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(true);
  });

  it('shows the image fallback when thumbnail generation fails', async () => {
    mockInvoke.mockRejectedValue(new Error('thumbnail failed'));

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    thumbnails.getImageThumbnail(entry);

    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(false);

    await flushThumbnailWork();

    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(true);
  });

  it('shows the image fallback when placeholder generation fails', async () => {
    setupImagePlaceholderMocks();
    mockCreateImageBitmap.mockRejectedValue(new Error('placeholder failed'));

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    thumbnails.getImageThumbnailPlaceholder(entry);

    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(false);

    await flushThumbnailWork();

    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(true);
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
    expect(thumbnails.imageThumbnailPlaceholders.value.size).toBe(0);
  });

  it('does not generate placeholders for gif files', async () => {
    setupImagePlaceholderMocks();

    const thumbnails = useImageThumbnails();
    const entry = {
      ...createImageEntry('animation.gif'),
      ext: 'gif',
    };

    expect(thumbnails.getImageThumbnailPlaceholder(entry)).toBeUndefined();
    await flushThumbnailWork();

    expect(mockFetch).not.toHaveBeenCalled();
    expect(thumbnails.imageThumbnailPlaceholders.value.size).toBe(0);
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
    expect(thumbnails.imageThumbnailPlaceholders.value.size).toBe(1);
  });

  it('aborts an active placeholder request when it is cancelled', async () => {
    setupImagePlaceholderMocks();
    const pendingResponse = createDeferred<Response>();
    let placeholderAbortSignal: AbortSignal | undefined;
    mockFetch.mockImplementation((_source: RequestInfo | URL, options?: RequestInit) => {
      placeholderAbortSignal = options?.signal ?? undefined;

      return pendingResponse.promise;
    });

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    thumbnails.getImageThumbnailPlaceholder(entry);

    expect(placeholderAbortSignal).toBeDefined();
    expect(placeholderAbortSignal?.aborted).toBe(false);

    thumbnails.cancelImageThumbnail(entry);

    expect(placeholderAbortSignal?.aborted).toBe(true);
    pendingResponse.resolve(createFetchResponse(new Blob(['image'])));
    await flushThumbnailWork();
  });

  it('retries a placeholder after cancellation and immediate re-request', async () => {
    setupImagePlaceholderMocks();
    const firstResponse = createDeferred<Response>();
    const secondResponse = createDeferred<Response>();
    const pendingResponses = [firstResponse, secondResponse];
    mockFetch.mockImplementation((_source: RequestInfo | URL, options?: RequestInit) => {
      const pendingResponse = pendingResponses.shift();

      if (!pendingResponse) {
        return Promise.reject(new Error('Unexpected placeholder fetch'));
      }

      options?.signal?.addEventListener('abort', () => {
        pendingResponse.reject(new Error('Placeholder aborted'));
      });

      return pendingResponse.promise;
    });

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    thumbnails.getImageThumbnailPlaceholder(entry);
    thumbnails.cancelImageThumbnail(entry);
    thumbnails.getImageThumbnailPlaceholder(entry);
    await flushThumbnailWork();

    expect(mockFetch).toHaveBeenCalledTimes(2);
    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(false);

    secondResponse.resolve(createFetchResponse(new Blob(['image'])));
    await flushThumbnailWork();

    expect(thumbnails.getImageThumbnailPlaceholder(entry)).toBe(PLACEHOLDER_DATA_URL);
    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(false);
  });
});

describe('useImageThumbnails cache container', () => {
  function mockSuccessfulThumbnailGeneration(): void {
    mockInvoke.mockImplementation((_command: string, args: { path: string }) => {
      return Promise.resolve(`C:/thumb-${args.path}.jpg`);
    });
  }

  it('stores one entry per generated thumbnail', async () => {
    mockSuccessfulThumbnailGeneration();

    const thumbnails = useImageThumbnails();
    const entries: DirEntry[] = [];

    for (let entryIndex = 0; entryIndex < 12; entryIndex += 1) {
      entries.push(createImageEntry(`image-${entryIndex}.jpg`));
    }

    entries.forEach((entry) => {
      thumbnails.getImageThumbnail(entry);
    });

    await flushAllThumbnailWork();

    expect(thumbnails.imageThumbnails.value.size).toBe(12);
  });

  it('does not create duplicate entries when the same thumbnail key is written twice', async () => {
    mockSuccessfulThumbnailGeneration();

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    thumbnails.getImageThumbnail(entry);
    await flushAllThumbnailWork();

    const cachedThumbnailKeys = Array.from(thumbnails.imageThumbnails.value.keys());

    expect(cachedThumbnailKeys).toHaveLength(1);

    thumbnails.imageThumbnails.value.set(cachedThumbnailKeys[0], 'asset://C:/thumb-updated.jpg');
    thumbnails.imageThumbnails.value.set(cachedThumbnailKeys[0], 'asset://C:/thumb-newest.jpg');

    expect(thumbnails.imageThumbnails.value.size).toBe(1);
  });

  it('returns the newest value after a cached thumbnail key is overwritten', async () => {
    mockSuccessfulThumbnailGeneration();

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');

    thumbnails.getImageThumbnail(entry);
    await flushAllThumbnailWork();

    const cachedThumbnailKeys = Array.from(thumbnails.imageThumbnails.value.keys());

    thumbnails.imageThumbnails.value.set(cachedThumbnailKeys[0], 'asset://C:/thumb-newest.jpg');

    expect(thumbnails.imageThumbnails.value.get(cachedThumbnailKeys[0])).toBe('asset://C:/thumb-newest.jpg');
    expect(thumbnails.getImageThumbnail(entry)).toBe('asset://C:/thumb-newest.jpg');
  });

  it('updates computed values that depend on the thumbnail cache size', async () => {
    mockSuccessfulThumbnailGeneration();

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');
    const thumbnailCount = computed(() => thumbnails.imageThumbnails.value.size);

    expect(thumbnailCount.value).toBe(0);

    thumbnails.getImageThumbnail(entry);
    await flushAllThumbnailWork();
    await nextTick();

    expect(thumbnailCount.value).toBe(1);
  });

  it('updates computed values that depend on the placeholder cache size', async () => {
    setupImagePlaceholderMocks();

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');
    const placeholderCount = computed(() => thumbnails.imageThumbnailPlaceholders.value.size);

    expect(placeholderCount.value).toBe(0);

    thumbnails.getImageThumbnailPlaceholder(entry);
    await flushAllThumbnailWork();
    await nextTick();

    expect(placeholderCount.value).toBe(1);
  });

  it('updates computed values when thumbnail generation fails', async () => {
    mockInvoke.mockRejectedValue(new Error('thumbnail failed'));

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');
    const showsImageFallback = computed(() => thumbnails.shouldShowImageThumbnailFallback(entry));

    expect(showsImageFallback.value).toBe(false);

    thumbnails.getImageThumbnail(entry);
    await flushAllThumbnailWork();
    await nextTick();

    expect(showsImageFallback.value).toBe(true);
  });

  it('empties the cache and refreshes dependent computed values when cleared', async () => {
    mockSuccessfulThumbnailGeneration();

    const thumbnails = useImageThumbnails();
    const entry = createImageEntry('image.jpg');
    const thumbnailCount = computed(() => thumbnails.imageThumbnails.value.size);

    thumbnails.getImageThumbnail(entry);
    await flushAllThumbnailWork();

    expect(thumbnailCount.value).toBe(1);

    thumbnails.clearThumbnails();
    await nextTick();

    expect(thumbnails.imageThumbnails.value.size).toBe(0);
    expect(thumbnailCount.value).toBe(0);
  });
});
