// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 批次 65（W-18）—— 递归搜索失败必须**看得见**。
//
// 批次 59（W-18）把云盘目录作用域检索做成了真功能，后端在失败时返回明确报错
// （「该连接当前没有勾选『加入全局检索』，索引层不可见」）；
// 但前端的 `catch` 把错误整个吞掉，用户只看到「当前层」的结果 ——
// 「通道不支持」与「目录里没有匹配文件」被混成同一件事。
//
// 这组测试钉住四件事：
//   ① 后端报错原文被接住（字符串拒绝与 Error 拒绝都要接住）；
//   ② 后端成功时旧提示被清掉（不能留一条与当前状态无关的提示）；
//   ③ 查询清空后提示被清掉；
//   ④ 递归匹配拿不到时仍退回「只搜当前层」，不把当前层的结果也清空。

import {
  describe, expect, it, vi, beforeEach, afterEach,
} from 'vitest';
import { defineComponent, nextTick, ref } from 'vue';
import { mount } from '@vue/test-utils';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

import {
  toRecursiveSearchErrorMessage,
  useFileBrowserRecursiveSearch,
} from '../use-file-browser-recursive-search';

const BACKEND_ERROR = '云盘目录递归检索失败（sfm://remote/conn-a/docs）：该连接当前没有勾选「加入全局检索」，索引层不可见。请先在设置的云盘连接里勾选它。';

const RECURSIVE_ENTRY = {
  name: '分析报告.txt',
  ext: '.txt',
  path: 'C:/tmp/afm-w18/分析报告.txt',
  size: 12,
  item_count: null,
  modified_time: 1,
  accessed_time: 1,
  created_time: 1,
  mime: null,
  is_file: true,
  is_dir: false,
  is_symlink: false,
  is_hidden: false,
};

type RecursiveSearchState = ReturnType<typeof useFileBrowserRecursiveSearch>;

interface Harness {
  holder: { state?: RecursiveSearchState };
  currentPath: ReturnType<typeof ref<string>>;
  filterQuery: ReturnType<typeof ref<string>>;
}

/**
 * 把 composable 挂进一个真实组件里跑。
 *
 * 刻意不用「直接调用」：`onUnmounted` 在没有组件实例时会告警，
 * 而这条告警会淹没真正的断言失败信息。
 */
function mountComposable(currentPath: string): Harness {
  const path = ref(currentPath);
  const query = ref('');
  const holder: { state?: RecursiveSearchState } = {};

  mount(defineComponent({
    setup() {
      holder.state = useFileBrowserRecursiveSearch({
        currentPath: path,
        filterQuery: query,
        maxDepth: ref(3),
      });
      return {};
    },
    render: () => null,
  }));

  return {
    holder,
    currentPath: path,
    filterQuery: query,
  };
}

async function flushDebouncedSearch(): Promise<void> {
  await nextTick();
  await vi.advanceTimersByTimeAsync(300);
  await nextTick();
}

describe('useFileBrowserRecursiveSearch · 批次 65（W-18）', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('后端报错原文被接住，不再静默降级', async () => {
    invokeMock.mockRejectedValue(BACKEND_ERROR);
    const { holder, filterQuery } = mountComposable('sfm://remote/conn-a/docs');

    filterQuery.value = '分析';
    await flushDebouncedSearch();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock.mock.calls[0][0]).toBe('search_directory_recursive');
    expect(invokeMock.mock.calls[0][1]).toMatchObject({
      rootPath: 'sfm://remote/conn-a/docs',
      query: '分析',
    });
    expect(holder.state!.recursiveSearchError.value).toBe(BACKEND_ERROR);
  });

  it('后端成功时旧的错误提示被清掉', async () => {
    invokeMock.mockRejectedValueOnce(BACKEND_ERROR);
    const { holder, filterQuery } = mountComposable('sfm://remote/conn-a/docs');

    filterQuery.value = '分析';
    await flushDebouncedSearch();
    expect(holder.state!.recursiveSearchError.value).toBe(BACKEND_ERROR);

    invokeMock.mockResolvedValueOnce([RECURSIVE_ENTRY]);
    filterQuery.value = '报告';
    await flushDebouncedSearch();

    expect(holder.state!.recursiveSearchError.value).toBeNull();
    expect(holder.state!.recursiveMatches.value).toHaveLength(1);
    expect(holder.state!.recursiveMatches.value[0].name).toBe('分析报告.txt');
  });

  it('查询清空后错误提示不再成立，被清掉', async () => {
    invokeMock.mockRejectedValue(BACKEND_ERROR);
    const { holder, filterQuery } = mountComposable('sfm://remote/conn-a/docs');

    filterQuery.value = '分析';
    await flushDebouncedSearch();
    expect(holder.state!.recursiveSearchError.value).toBe(BACKEND_ERROR);

    filterQuery.value = '';
    await flushDebouncedSearch();

    expect(holder.state!.recursiveSearchError.value).toBeNull();
  });

  it('递归通道失败时当前层的过滤结果不受影响', async () => {
    invokeMock.mockRejectedValue(BACKEND_ERROR);
    const { holder, filterQuery } = mountComposable('sfm://remote/conn-a/docs');

    filterQuery.value = '分析';
    await flushDebouncedSearch();

    // 递归命中项为空（拿不到），但「递归搜索失败」这件事必须被记下来，
    // 否则界面上就只剩一个「没有匹配文件」的假象。
    expect(holder.state!.recursiveMatches.value).toEqual([]);
    expect(holder.state!.recursiveSearchError.value).toBe(BACKEND_ERROR);
  });

  it('错误值是 Error 实例时取 message', () => {
    expect(toRecursiveSearchErrorMessage(new Error(BACKEND_ERROR))).toBe(BACKEND_ERROR);
  });

  it('错误值是带 message 的普通对象时取 message', () => {
    expect(toRecursiveSearchErrorMessage({ message: BACKEND_ERROR })).toBe(BACKEND_ERROR);
  });

  it('错误值无法辨认时退回字符串化，不返回空串', () => {
    expect(toRecursiveSearchErrorMessage('纯字符串')).toBe('纯字符串');
    expect(toRecursiveSearchErrorMessage(undefined)).not.toBe('');
    expect(toRecursiveSearchErrorMessage({ code: 42 })).not.toBe('');
  });
});
