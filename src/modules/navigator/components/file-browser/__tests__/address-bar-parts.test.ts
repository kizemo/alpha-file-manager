// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import { LOCATIONS_VIRTUAL_PATH } from '@/utils/virtual-path-constants';
import { buildAddressBarParts } from '../address-bar-parts';

describe('buildAddressBarParts', () => {
  it('builds Locations > / for the unix filesystem root', () => {
    const parts = buildAddressBarParts('/', 'linux', 'Locations');

    expect(parts.map(part => ({
      path: part.path,
      name: part.name,
    }))).toEqual([
      {
        path: LOCATIONS_VIRTUAL_PATH,
        name: 'Locations',
      },
      {
        path: '/',
        name: '/',
      },
    ]);
  });

  it('builds Locations > C: for a windows drive root', () => {
    const parts = buildAddressBarParts('C:/', 'windows', 'Locations');

    expect(parts.map(part => ({
      path: part.path,
      name: part.name,
    }))).toEqual([
      {
        path: LOCATIONS_VIRTUAL_PATH,
        name: 'Locations',
      },
      {
        path: 'C:/',
        name: 'C:',
      },
    ]);
  });

  it('builds Locations > / for trailing-slash-only unix roots', () => {
    const parts = buildAddressBarParts('///', 'linux', 'Locations');

    expect(parts.map(part => ({
      path: part.path,
      name: part.name,
    }))).toEqual([
      {
        path: LOCATIONS_VIRTUAL_PATH,
        name: 'Locations',
      },
      {
        path: '/',
        name: '/',
      },
    ]);
  });

  it('does not prepend Locations for nested unix home paths', () => {
    const parts = buildAddressBarParts('/home/user', 'linux', 'Locations');

    expect(parts.map(part => ({
      path: part.path,
      name: part.name,
    }))).toEqual([
      {
        path: '/home',
        name: 'home',
      },
      {
        path: '/home/user',
        name: 'user',
      },
    ]);
  });

  // FORK-MODIFICATION 2026-10-18：远程面包屑（阶段 2 需求 1）。
  describe('远程路径族', () => {
    const context = {
      rootLabel: 'Remote',
      connectionNames: {
        'conn-a': '速豹堂',
      },
    };

    it('逐级拼出可导航的远程路径，而不是 sfm:/ 开头的假盘符', () => {
      // 改动前：getAddressBarSegments 切出 ['sfm:','remote','c','pub']，
      // 'sfm:' 带冒号被当成 Windows 盘符，拼成 'sfm:/remote/c/pub' —— 一个用户
      // 从没去过的路径。下面每一条 path 都是能真正 navigateTo 的。
      const parts = buildAddressBarParts('sfm://remote/conn-a/pub/docs', 'windows', 'Locations', context);

      expect(parts.map(part => ({
        path: part.path,
        name: part.name,
        isLast: part.isLast,
      }))).toEqual([
        {
          path: 'sfm://remote',
          name: 'Remote',
          isLast: false,
        },
        {
          path: 'sfm://remote/conn-a',
          name: '速豹堂',
          isLast: false,
        },
        {
          path: 'sfm://remote/conn-a/pub',
          name: 'pub',
          isLast: false,
        },
        {
          path: 'sfm://remote/conn-a/pub/docs',
          name: 'docs',
          isLast: true,
        },
      ]);

      // 判别点：没有任何一条落进本地盘符拼法（单斜杠的 sfm:/remote/...）。
      // 注意不能用 startsWith('sfm:/') 判断 —— 正确的 sfm://remote 本身也满足它。
      expect(parts.every(part => !part.path.startsWith('sfm:/remote'))).toBe(true);
      expect(parts.every(part => part.path.startsWith('sfm://remote'))).toBe(true);
    });

    it('连接根那一级是末级，名字是连接名', () => {
      const parts = buildAddressBarParts('sfm://remote/conn-a', 'windows', 'Locations', context);

      expect(parts.map(part => part.path)).toEqual([
        'sfm://remote',
        'sfm://remote/conn-a',
      ]);
      expect(parts[1]?.name).toBe('速豹堂');
      expect(parts[1]?.isLast).toBe(true);
    });

    it('查不到连接名时退回连接 id，而不是留空', () => {
      // 连接被删掉后 workspace 里可能还留着标签页。空名字会让面包屑出现一个
      // 看得见、点不动的空按钮；uuid 难看但至少能认出是哪一条。
      const parts = buildAddressBarParts('sfm://remote/conn-gone/pub', 'windows', 'Locations', context);

      expect(parts.map(part => part.name)).toEqual([
        'Remote',
        'conn-gone',
        'pub',
      ]);
    });

    it('sfm://remote 本身只有一级，且不能当拖放目标', () => {
      // getNavigableParentPath 允许从连接根上到 sfm://remote，
      // 所以这一级必须存在，否则「返回上一级」会走进一个没有面包屑的页面。
      const parts = buildAddressBarParts('sfm://remote', 'windows', 'Locations', context);

      expect(parts).toHaveLength(1);
      expect(parts[0]?.path).toBe('sfm://remote');
      expect(parts[0]?.isLast).toBe(true);
      expect(parts[0]?.disableDropTarget).toBe(true);
    });

    it('非法远程路径不产出面包屑', () => {
      // `..` 段让 parseRemotePath 返回 null。`getNavigableParentPath` 对同一批路径
      // 也返回 null —— 一个能点、一个不能点的面包屑比完全没有更糟。
      expect(buildAddressBarParts('sfm://remote/conn-a/pub/../etc', 'windows', 'Locations', context))
        .toEqual([]);
    });

    it('不传远程上下文时退回方案名与连接 id', () => {
      // 组件外的调用方（以及老测试）可以继续用三参数签名，不该在这里崩掉。
      const parts = buildAddressBarParts('sfm://remote/conn-a/pub', 'windows', 'Locations');

      expect(parts.map(part => part.name)).toEqual([
        'sfm://remote',
        'conn-a',
        'pub',
      ]);
    });

    it('本地路径不受远程分支影响（阳性对照）', () => {
      const parts = buildAddressBarParts('C:/Users', 'windows', 'Locations');

      expect(parts.map(part => part.path)).toEqual([
        'sfm://locations',
        'C:/',
        'C:/Users',
      ]);
    });
  });

  // FORK-MODIFICATION 2026-11-19（批次 23）：shell 设备面包屑。
  //
  // 手机（MTP）的路径是 `::{20D04FE0-...}\?\usb#...` —— 一个不透明标识符。
  // 通用分支会把它切成一堆看不懂的段（还会把 `::{` 当成盘符），
  // 所以整条路径只当一级，显示「位置 > 设备名」。
  //
  // FORK-MODIFICATION 2026-11-19（批次 24）：多级层级改由后端祖先链
  // （`resolve_shell_path_chain`）提供，见下面「shell 祖先链多级面包屑」。
  // 本 describe 锁的是**还没拿到链**（加载中 / 命令报错 / 非 shell 路径）时的
  // 降级形态 —— 必须仍然等于批次 23 的两段式，用户看到的东西不能比改动前更差。
  describe('shell 设备面包屑（未拿到祖先链时的降级形态）', () => {
    const devicePath = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\\usb#vid_22d9&pid_2765';

    it('没有链时产出「位置 > 设备名」两级面包屑', () => {
      const parts = buildAddressBarParts(devicePath, 'windows', 'Locations', undefined, {
        deviceNames: { [devicePath]: 'Galaxy S22' },
        fallbackLabel: 'Device',
        pathChain: [],
      });

      expect(parts.map(part => ({
        path: part.path,
        name: part.name,
      }))).toEqual([
        {
          path: LOCATIONS_VIRTUAL_PATH,
          name: 'Locations',
        },
        {
          path: devicePath,
          name: 'Galaxy S22',
        },
      ]);
      // shell 路径不是文件系统目录，拖放目标没有意义。
      expect(parts.every(part => part.disableDropTarget)).toBe(true);
    });

    it('查不到设备名时用短标签，绝不把 ::{...} 整串糊在界面上', () => {
      const parts = buildAddressBarParts(devicePath, 'windows', 'Locations', undefined, {
        deviceNames: {},
        fallbackLabel: 'Device',
      });

      expect(parts).toHaveLength(2);
      expect(parts[1]?.name).toBe('Device');
      expect(parts[1]?.name).not.toContain('::{');
    });

    it('本地路径不受 shell 分支影响（阳性对照）', () => {
      const parts = buildAddressBarParts('C:/Users', 'windows', 'Locations');

      expect(parts.map(part => part.path)).toEqual([
        'sfm://locations',
        'C:/',
        'C:/Users',
      ]);
    });
  });

  // FORK-MODIFICATION 2026-11-19（批次 24）：多级层级由后端 `resolve_shell_path_chain`
  // 的祖先链提供，前端**不再**自己切分 shell 路径。
  //
  // 夹具是一加 Ace 3 的真机路径（三层：设备根 → 存储容器 → 文件夹）。
  // 长度 129 / 156 / 195 / 82 是实测字节数，写错一个反斜杠就会对不上，
  // 所以这里把长度也当断言钉住 —— 它同时证明这些字符串没有被任何「规范化」动过。
  describe('shell 祖先链多级面包屑', () => {
    const phoneRoot = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\\\\\?\\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33}';
    const storageCrumb = `${phoneRoot}\\SID-{10001,,1001925963776}`;
    const folderCrumb = `${storageCrumb}\\{66A9CB84-2262-AF6E-0000-000000000000}`;
    const fnasRoot = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\uuid:4d696e69-444c-164e-9d41-7085c27f4afe';

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

    it('夹具就是真机字节长度（反斜杠没被转义吃掉）', () => {
      expect(phoneRoot).toHaveLength(129);
      expect(storageCrumb).toHaveLength(156);
      expect(folderCrumb).toHaveLength(195);
      expect(fnasRoot).toHaveLength(82);
    });

    it('三层链产出「位置 > 设备 > 存储 > 文件夹」四段，每级 path 用链项自己的 path', () => {
      const parts = buildAddressBarParts(folderCrumb, 'windows', 'Locations', undefined, {
        deviceNames: {},
        fallbackLabel: 'Device',
        pathChain: phoneChain,
      });

      expect(parts.map(part => part.path)).toEqual([
        LOCATIONS_VIRTUAL_PATH,
        phoneRoot,
        storageCrumb,
        folderCrumb,
      ]);
      expect(parts.map(part => part.name)).toEqual([
        'Locations',
        '一加 Ace 3',
        '内部共享存储空间',
        '.7934039a',
      ]);
      expect(parts).toHaveLength(4);
      expect(parts.every(part => part.disableDropTarget)).toBe(true);
      expect(parts[0]?.useLocationsDropdown).toBe(true);
      expect(parts[0]?.displayRootIcon).toBe(true);
      expect(parts.slice(1).every(part => !part.displayRootIcon && !part.useLocationsDropdown)).toBe(true);
      expect(parts.slice(0, 3).every(part => part.isLast === false)).toBe(true);
      expect(parts[3]?.isLast).toBe(true);
    });

    // 缺陷 D1：设备根下一级的存储容器（「内部共享存储空间」）曾被前端降级成一个笼统的
    // 「设备」标签。链一旦可用，每一级的名字都必须来自链，不能出现兜底标签。
    it('存储容器显示自己的名字，不再被降级成兜底标签', () => {
      const parts = buildAddressBarParts(storageCrumb, 'windows', 'Locations', undefined, {
        deviceNames: {},
        fallbackLabel: 'Device',
        pathChain: phoneChain.slice(0, 2),
      });

      expect(parts.map(part => part.name)).toEqual([
        'Locations',
        '一加 Ace 3',
        '内部共享存储空间',
      ]);
      expect(parts.some(part => part.name === 'Device')).toBe(false);
      expect(parts[2]?.path).toBe(storageCrumb);
    });

    it('链项名字为空时该级退回兜底标签，整条面包屑不崩', () => {
      const parts = buildAddressBarParts(storageCrumb, 'windows', 'Locations', undefined, {
        deviceNames: {},
        fallbackLabel: 'Device',
        pathChain: [
          {
            path: phoneRoot,
            name: '一加 Ace 3',
          },
          {
            path: storageCrumb,
            name: '',
          },
        ],
      });

      expect(parts.map(part => part.name)).toEqual([
        'Locations',
        '一加 Ace 3',
        'Device',
      ]);
    });

    it('链与当前路径对不上（陈旧链）时退回两段式降级形态', () => {
      const parts = buildAddressBarParts(storageCrumb, 'windows', 'Locations', undefined, {
        deviceNames: {},
        fallbackLabel: 'Device',
        pathChain: phoneChain,
      });

      expect(parts).toHaveLength(2);
      expect(parts[1]?.path).toBe(storageCrumb);
      expect(parts[1]?.name).toBe('Device');
    });

    it('设备根只有一层时链与降级形态同为两段，但名字来自链', () => {
      const parts = buildAddressBarParts(fnasRoot, 'windows', 'Locations', undefined, {
        deviceNames: {},
        fallbackLabel: 'Device',
        pathChain: [{
          path: fnasRoot,
          name: 'fnNAS',
        }],
      });

      expect(parts.map(part => part.name)).toEqual([
        'Locations',
        'fnNAS',
      ]);
      expect(parts[1]?.path).toBe(fnasRoot);
    });
  });
});
