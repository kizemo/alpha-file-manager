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
import {
  effectScope,
  nextTick,
  ref,
} from 'vue';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn<(command: string, args: unknown) => Promise<unknown>>(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

// FORK-MODIFICATION 2026-11-19（批次 24）：shell 祖先链的响应式外壳。
//
// 夹具是一加 Ace 3 的真机三层路径。`\\\\\\?\\` 是**值**里的三个反斜杠 + `?` + 一个反斜杠
// （shell 解析名的真实形态），TS 字面量必须双写；长度 129 由 address-bar-parts 的用例钉住。
const phoneRoot = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\\\\\?\\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33}';
const storageCrumb = `${phoneRoot}\\SID-{10001,,1001925963776}`;
const folderCrumb = `${storageCrumb}\\{66A9CB84-2262-AF6E-0000-000000000000}`;

const phoneChain = [
  {
    path: phoneRoot,
    name: '一加 Ace 3',
  },
  {
    path: storageCrumb,
    name: '内部共享存储空间',
  },
  {
    path: folderCrumb,
    name: '.7934039a',
  },
];

/** 让「已 resolve 的 invoke」跑完它后面的赋值，并让 watcher 落一次。 */
async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await nextTick();
  await new Promise<void>(resolve => setTimeout(resolve, 0));
  await nextTick();
}

async function loadUseShellPathChain() {
  const module = await import('@/composables/use-shell-path-chain');

  return module.useShellPathChain;
}

describe('useShellPathChain', () => {
  beforeEach(() => {
    vi.resetModules();
    invokeMock.mockReset();
    invokeMock.mockResolvedValue([]);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shell 路径按后端契约拉取祖先链', async () => {
    const useShellPathChain = await loadUseShellPathChain();

    invokeMock.mockImplementation((command: string) => (command === 'resolve_shell_path_chain'
      ? Promise.resolve(phoneChain)
      : Promise.resolve([])));

    const scope = effectScope();
    const api = scope.run(() => useShellPathChain(() => folderCrumb));

    await settle();

    expect(invokeMock).toHaveBeenCalledWith('resolve_shell_path_chain', { path: folderCrumb });
    expect(api?.shellPathChain.value).toEqual(phoneChain);

    scope.stop();
  });

  it('同一个 path 再次进入时不再重复请求（按 path 缓存）', async () => {
    const useShellPathChain = await loadUseShellPathChain();

    invokeMock.mockImplementation((command: string) => (command === 'resolve_shell_path_chain'
      ? Promise.resolve(phoneChain)
      : Promise.resolve([])));

    const firstScope = effectScope();
    const firstApi = firstScope.run(() => useShellPathChain(() => folderCrumb));

    await settle();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(firstApi?.shellPathChain.value).toEqual(phoneChain);

    firstScope.stop();

    const secondScope = effectScope();
    const secondApi = secondScope.run(() => useShellPathChain(() => folderCrumb));

    await settle();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(secondApi?.shellPathChain.value).toEqual(phoneChain);

    secondScope.stop();
  });

  it('旧路径的迟到响应不得覆盖当前路径的链（竞态守卫）', async () => {
    const useShellPathChain = await loadUseShellPathChain();

    const storageChain = phoneChain.slice(0, 2);
    // 用 Map 存 resolver：`let` 变量在闭包里赋值会被 TS 窄化成 `null`，
    // 后面 `resolveStale?.()` 就过不了 ts 门禁。
    const pendingResolvers = new Map<string, (value: unknown) => void>();

    invokeMock.mockImplementation((_command: string, args: unknown) => {
      const path = (args as { path: string }).path;

      if (path === folderCrumb) {
        return new Promise<unknown>((resolve) => {
          pendingResolvers.set(path, resolve);
        });
      }

      return Promise.resolve(storageChain);
    });

    const currentPath = effectScope();
    const pathRef = ref(folderCrumb);
    const api = currentPath.run(() => useShellPathChain(() => pathRef.value));

    await settle();

    pathRef.value = storageCrumb;
    await settle();

    expect(api?.shellPathChain.value).toEqual(storageChain);

    // 第一个请求现在才回来：它属于已经不是当前路径的 folderCrumb，必须被丢弃。
    pendingResolvers.get(folderCrumb)?.(phoneChain);
    await settle();

    expect(api?.shellPathChain.value).toEqual(storageChain);
    expect(invokeMock).toHaveBeenCalledTimes(2);

    currentPath.stop();
  });

  it('命令报错时链保持为空且不抛异常，失败的链不写缓存', async () => {
    const useShellPathChain = await loadUseShellPathChain();
    const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    invokeMock.mockImplementation((command: string) => (command === 'resolve_shell_path_chain'
      ? Promise.reject(new Error('shell namespace unavailable'))
      : Promise.resolve([])));

    const firstScope = effectScope();
    const api = firstScope.run(() => useShellPathChain(() => folderCrumb));

    await settle();

    expect(api?.shellPathChain.value).toEqual([]);

    firstScope.stop();

    const secondScope = effectScope();
    const secondApi = secondScope.run(() => useShellPathChain(() => folderCrumb));

    await settle();

    // 设备可能只是稍后才出现 ⇒ 失败不能缓存成「永久空链」。
    expect(secondApi?.shellPathChain.value).toEqual([]);
    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(consoleErrorSpy).toHaveBeenCalled();

    secondScope.stop();
    consoleErrorSpy.mockRestore();
  });

  it('非 shell 路径不问后端', async () => {
    const useShellPathChain = await loadUseShellPathChain();

    const scope = effectScope();
    const api = scope.run(() => useShellPathChain(() => 'C:/Users'));

    await settle();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(api?.shellPathChain.value).toEqual([]);

    scope.stop();
  });

  it('后端返回非数组时按空链处理', async () => {
    const useShellPathChain = await loadUseShellPathChain();

    invokeMock.mockImplementation((command: string) => (command === 'resolve_shell_path_chain'
      ? Promise.resolve({ unexpected: true })
      : Promise.resolve([])));

    const scope = effectScope();
    const api = scope.run(() => useShellPathChain(() => storageCrumb));

    await settle();

    expect(api?.shellPathChain.value).toEqual([]);

    scope.stop();
  });
});
