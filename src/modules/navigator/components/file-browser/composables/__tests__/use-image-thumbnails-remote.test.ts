// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-21（批次 32 → 远程缩略图 Rust 代理）。
//
// 判据来源：docs/handoff-2026-11-21-batch32-remote-thumbnail-verdict.md §3（直链方案架构性不可行）
// 与 §4（改成 Rust 代理 remote_generate_thumbnail）。
//
// 这个文件钉住 4 件事，全都是「改回直链方案就会红」的形状：
// 1. 远程条目**绝不**把服务端签名直链当 `<img src>` 返回；
// 2. 远程条目走 `remote_generate_thumbnail`，**不是** `generate_image_thumbnail`，且不传 size；
// 3. 出图之前 fallback 必须是 true（透明空洞会让真机像素判据 J1 直接判没图）；
// 4. 同名远程条目各用自己的 path，不会串图。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import type { DirEntry } from '@/types/dir-entry';

const mockInvoke = vi.hoisted(() => vi.fn());
const mockConvertFileSrc = vi.hoisted(() => vi.fn((path: string) => `asset://${path}`));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => mockConvertFileSrc(path),
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  quantizeImageThumbnailMaxDimension,
  useImageThumbnails,
} from '../use-image-thumbnails';

const REMOTE_PATH = 'sfm://remote/conn-a/photos/IMG_0001.jpg';
const REMOTE_THUMBNAIL = 'https://example.invalid/thumb.jpg?sign=abc';

function createRemoteEntry(overrides: Partial<DirEntry> = {}): DirEntry {
  return {
    name: 'IMG_0001.jpg',
    ext: 'jpg',
    path: REMOTE_PATH,
    size: 1024,
    item_count: null,
    modified_time: 123,
    accessed_time: 0,
    created_time: 0,
    mime: 'image/jpeg',
    is_file: true,
    is_dir: false,
    is_symlink: false,
    is_hidden: false,
    thumbnail_url: REMOTE_THUMBNAIL,
    ...overrides,
  };
}

// 一次 macrotask 排空整条微任务链（缩略图生成是 await invoke 串起来的）。
async function flushThumbnailWork(): Promise<void> {
  await new Promise<void>((resolve) => {
    setTimeout(resolve, 0);
  });
}

describe('远程网盘图片缩略图（Rust 代理）', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockConvertFileSrc.mockClear();
  });

  it('不把服务端签名直链当缩略图返回，改走 remote_generate_thumbnail', async () => {
    const thumbnails = useImageThumbnails();
    const entry = createRemoteEntry();
    mockInvoke.mockResolvedValue('C:/cache/remote-image-thumbnails/a.png');

    expect(thumbnails.getImageThumbnail(entry)).toBeUndefined();

    await flushThumbnailWork();

    // 判据 1：任何一次调用都必须是远程代理命令，绝不能是本地命令。
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith('remote_generate_thumbnail', {
      path: REMOTE_PATH,
      modifiedTime: 123,
      maxDimension: 384,
    });
    expect(mockInvoke.mock.calls.map(([command]) => command)).not.toContain('generate_image_thumbnail');

    // 出图之后拿到的是 asset URI（本地产物），不是直链。
    const src = thumbnails.getImageThumbnail(entry);
    expect(src).toBe('asset://C:/cache/remote-image-thumbnails/a.png');
    expect(src).not.toContain('example.invalid');
    expect(src).not.toBe(REMOTE_THUMBNAIL);
  });

  it('远程条目在出图之前显示回退图标，不留透明空洞', async () => {
    const thumbnails = useImageThumbnails();
    const entry = createRemoteEntry();

    let resolveThumbnail: (value: string | null) => void = () => {};

    mockInvoke.mockReturnValue(new Promise<string | null>((resolve) => {
      resolveThumbnail = resolve;
    }));

    thumbnails.getImageThumbnail(entry);

    // pending 期：J1 像素判据要的就是这一刻卡片上有内容。
    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(true);

    resolveThumbnail('C:/cache/remote-image-thumbnails/a.png');
    await flushThumbnailWork();

    expect(thumbnails.getImageThumbnail(entry)).toBe('asset://C:/cache/remote-image-thumbnails/a.png');
    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(false);
  });

  it('远程代理返回 null 时标记失败并保留回退图标，且只请求一次', async () => {
    const thumbnails = useImageThumbnails();
    const entry = createRemoteEntry();
    mockInvoke.mockResolvedValue(null);

    expect(thumbnails.getImageThumbnail(entry)).toBeUndefined();
    await flushThumbnailWork();

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(thumbnails.getImageThumbnail(entry)).toBeUndefined();
    // 也不该在退避窗口内重新排队 —— 退避到期前 failedThumbnails 仍然挡着。
    // （失败不再是终态：退避到期后会被重新入队，见下方缺陷 B 的用例。）
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(true);

    // 收尾：取消这个用例亲手排下的退避定时器，别让它跨用例触发。
    thumbnails.clearThumbnails();
  });

  it('同名远程条目各用自己的 path，不会串图', async () => {
    const thumbnails = useImageThumbnails();
    const first = createRemoteEntry({
      entry_id: 'aliyun:file-a',
      path: 'sfm://remote/conn-a/photos/IMG_0001.jpg',
    });
    const second = createRemoteEntry({
      entry_id: 'aliyun:file-b',
      path: 'sfm://remote/conn-a/photos/2024/IMG_0001.jpg',
      modified_time: 456,
    });
    mockInvoke.mockImplementation((command: string, args: { path: string }) => {
      const cachedPath = args.path.endsWith('/photos/IMG_0001.jpg')
        ? 'C:/cache/remote-image-thumbnails/first.png'
        : 'C:/cache/remote-image-thumbnails/second.png';
      return Promise.resolve(cachedPath);
    });

    thumbnails.getImageThumbnail(first);
    thumbnails.getImageThumbnail(second);
    await flushThumbnailWork();

    const paths = mockInvoke.mock.calls.map(([, args]) => (args as { path: string }).path);
    expect(paths).toContain('sfm://remote/conn-a/photos/IMG_0001.jpg');
    expect(paths).toContain('sfm://remote/conn-a/photos/2024/IMG_0001.jpg');
    expect(thumbnails.getImageThumbnail(first)).toBe('asset://C:/cache/remote-image-thumbnails/first.png');
    expect(thumbnails.getImageThumbnail(second)).toBe('asset://C:/cache/remote-image-thumbnails/second.png');
  });

  it('远程条目仍然不做占位图（原图字节读不到）', async () => {
    const thumbnails = useImageThumbnails();
    const entry = createRemoteEntry();

    expect(thumbnails.getImageThumbnailPlaceholder(entry)).toBeUndefined();
    await flushThumbnailWork();

    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('远程目录条目不进缩略图管线', () => {
    const thumbnails = useImageThumbnails();
    const folder = createRemoteEntry({
      name: 'Photos',
      ext: '',
      path: 'sfm://remote/conn-a/photos',
      is_file: false,
      is_dir: true,
    });

    expect(thumbnails.getImageThumbnail(folder)).toBeUndefined();
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  // FORK-MODIFICATION 2026-11-21（批次 34 缺陷 A）：网格卡的 maxDimension 量化。
  //
  // 真机证据：同一屏卡片在 100 秒内取到 340 / 331 / 255 / 248 四档 maxDimension，
  // 磁盘上留下四组尺寸各异的缓存目录条目。维度是缓存键的一部分 ⇒ 每抖一次就换 key，
  // 卡片侧 `watch(imagePreviewSrc)` 无条件清 `loadedImagePreviewSrc`，而主图样式是
  // `--final { opacity: 0 }` / `--final-loaded { opacity: 1 }` ⇒ 收尾永远追不上，
  // 图片停在 opacity:0 的透明空洞里（列表视图固定 48px，所以列表正常）。
  it('网格卡预览尺寸量化到 64 的整数倍，四档抖动收敛为两档（缺陷 A）', () => {
    // 真机四档：340 / 331 同一桶，255 / 248 同一桶。
    expect(quantizeImageThumbnailMaxDimension(340)).toBe(384);
    expect(quantizeImageThumbnailMaxDimension(331)).toBe(384);
    expect(quantizeImageThumbnailMaxDimension(255)).toBe(256);
    expect(quantizeImageThumbnailMaxDimension(248)).toBe(256);

    // 量化不能把人顶出上限（后端 clamp 到 1024，但缓存键必须与 384 档一致）。
    expect(quantizeImageThumbnailMaxDimension(384)).toBe(384);
    expect(quantizeImageThumbnailMaxDimension(348)).toBe(384);
    expect(quantizeImageThumbnailMaxDimension(64)).toBe(64);

    // 非有限 / 非正数回落最小桶，绝不让 NaN 进缓存键（NaN 键永不命中）。
    expect(quantizeImageThumbnailMaxDimension(Number.NaN)).toBe(64);
    expect(quantizeImageThumbnailMaxDimension(0)).toBe(64);
    expect(quantizeImageThumbnailMaxDimension(-10)).toBe(64);
  });

  // FORK-MODIFICATION 2026-11-21（批次 34 缺陷 B）：失败不再是终态。
  //
  // 旧实现把 key 永久塞进 failedThumbnails，而 getImageThumbnail 对失败 key 恒返
  // undefined 且不再入队 ⇒ 远程任一张图吃到一个 null（限流/超时/直链失效都是常态）
  // 就在本目录内永久空白。现在改成有限次退避重试（1.5s 起、指数推进、上限 15s、最多 3 次）。
  it('远程代理失败后按退避重试，而不是永久判死（缺陷 B）', async () => {
    const thumbnails = useImageThumbnails();
    const entry = createRemoteEntry();
    mockInvoke.mockResolvedValue(null);

    thumbnails.getImageThumbnail(entry);
    await flushThumbnailWork();
    expect(mockInvoke).toHaveBeenCalledTimes(1);

    // 退避窗口内：既不立即重排，也不无限打请求。
    thumbnails.getImageThumbnail(entry);
    await flushThumbnailWork();
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(true);

    // 卡片侧读 failedThumbnails（computed 依赖），退避到期后重新求值即重新入队。
    await vi.waitFor(
      () => {
        thumbnails.getImageThumbnail(entry);
        expect(mockInvoke).toHaveBeenCalledTimes(2);
      },
      {
        timeout: 5000,
        interval: 50,
      },
    );

    // 收尾：取消第二次失败排下的退避定时器。
    thumbnails.clearThumbnails();
  });

  // FORK-MODIFICATION 2026-11-21（批次 34 缺陷 C）：clearThumbnails 必须一并清并发预算。
  //
  // 旧实现只清队列与集合，processingThumbnails / processingPlaceholders 要等在途 promise
  // 的 finally 才释放；真机上出现过「100 秒产出 26 张后 7.6 分钟零新增」的停摆 ——
  // 并发上限被不 settle 的请求永久占满后整条队列不再推进。
  it('clearThumbnails 清掉失败态与并发预算，重建后能立即重新入队（缺陷 C）', async () => {
    const thumbnails = useImageThumbnails();
    const entry = createRemoteEntry();
    mockInvoke.mockResolvedValue(null);

    thumbnails.getImageThumbnail(entry);
    await flushThumbnailWork();
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(true);

    // 换目录 / 卸载重建时的入口。
    thumbnails.clearThumbnails();

    mockInvoke.mockResolvedValue('C:/cache/remote-image-thumbnails/after-clear.png');
    thumbnails.getImageThumbnail(entry);
    await flushThumbnailWork();

    // 旧实现里失败 key 仍在集合中（或并发预算被占满）⇒ 这里会停在 1 次调用。
    expect(mockInvoke).toHaveBeenCalledTimes(2);
    expect(thumbnails.getImageThumbnail(entry)).toBe('asset://C:/cache/remote-image-thumbnails/after-clear.png');
  });
});
