// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 目录树在焦点处于远程目录时切到远程目录树（用户 2026-10-19 反馈第 3 条，
// 以及同日第二次要求「根用设置里的远程目录、标签用连接名」）。

import { describe, expect, it } from 'vitest';
import { resolveRemoteTreeRoot, resolveTreeRoots } from '../remote-tree-roots';

const CONN = 'e2b24ab525544dc7adeb5cedb192179b';
const REMOTE_ROOT = `sfm://remote/${CONN}`;
/** 用户这条连接在设置里的初始目录（WordPress 站点根）。 */
const INITIAL = '/vol1/@team/study_platform/wp_root';
const REMOTE_INITIAL = `${REMOTE_ROOT}/vol1/@team/study_platform/wp_root`;

const DRIVE_PATHS = ['C:/', 'D:/'];
const DRIVE_LABELS = {
  'C:/': 'Win (C:)',
  'D/': 'Program (D:)',
};
const CONNECTIONS = {
  [CONN]: {
    name: '速豹堂',
    initialPath: INITIAL,
  },
};

function resolve(activePath: string | undefined) {
  return resolveTreeRoots({
    activePath,
    drivePaths: DRIVE_PATHS,
    driveLabels: DRIVE_LABELS,
    connectionsById: CONNECTIONS,
  });
}

describe('焦点在本地时：行为一个字没变', () => {
  it.each([
    ['本地盘符', 'C:/Users/Duanyi'],
    ['本地子目录', 'E:/website_sbt/wp-content'],
    ['没有焦点路径', undefined],
    ['空串', ''],
  ])('%s → 树根是盘符，标签原样透传', (_label, activePath) => {
    const result = resolve(activePath);

    expect(result.rootPaths).toEqual(['C:/', 'D:/']);
    // 卷标是 Rust 侧 DriveInfo.name 给的，必须原样传下去。
    expect(result.rootLabels).toEqual({
      'C:/': 'Win (C:)',
      'D/': 'Program (D:)',
    });
  });

  it('本地根的标签是独立副本，改它不影响调用方传进来的对象', () => {
    const result = resolve('C:/');
    result.rootLabels['C:/'] = 'tampered';

    expect(DRIVE_LABELS['C:/']).toBe('Win (C:)');
  });
});

describe('焦点在远程时：根是设置里的那个目录，标签是连接名', () => {
  it('根取 initialPath，不是连接根', () => {
    const result = resolve(`sfm://remote/${CONN}/vol1`);

    // 上一轮这里取的是连接根，理由是「根跟着焦点换会不停重建」。
    // 那个顾虑针对的是「当前目录」；initialPath 是连接配置里的固定值，
    // 只在换连接时才变，重建问题不存在。
    expect(result.rootPaths).toEqual([REMOTE_INITIAL]);
  });

  it('根不随焦点在远端深处移动', () => {
    // 焦点走到 wp_root 下面的第三层，根仍然停在配置目录 ——
    // 这正是「不重建」的保证。
    const deep = `${REMOTE_INITIAL}/wp-content/themes/twentytwentyone`;

    expect(resolve(deep).rootPaths).toEqual([REMOTE_INITIAL]);
  });

  it('标签用连接名，不是连接 id', () => {
    const result = resolve(`sfm://remote/${CONN}/vol1`);

    expect(result.rootLabels).toEqual({ [REMOTE_INITIAL]: '速豹堂' });
  });

  it('盘符此时不再作为根', () => {
    const result = resolve(`sfm://remote/${CONN}/vol1`);

    expect(result.rootPaths).not.toContain('C:/');
    expect(result.rootLabels['C:/']).toBeUndefined();
  });

  it('连接名缺失时退回连接 id，不显示空白', () => {
    const result = resolveTreeRoots({
      activePath: `sfm://remote/${CONN}/vol1`,
      drivePaths: DRIVE_PATHS,
      driveLabels: DRIVE_LABELS,
      connectionsById: {
        [CONN]: {
          name: '',
          initialPath: INITIAL,
        },
      },
    });

    expect(result.rootLabels).toEqual({ [REMOTE_INITIAL]: CONN });
  });

  it('连接名是空白串时也退回连接 id', () => {
    const result = resolveTreeRoots({
      activePath: `sfm://remote/${CONN}/vol1`,
      drivePaths: DRIVE_PATHS,
      driveLabels: DRIVE_LABELS,
      connectionsById: {
        [CONN]: {
          name: '   ',
          initialPath: INITIAL,
        },
      },
    });

    expect(result.rootLabels).toEqual({ [REMOTE_INITIAL]: CONN });
  });
});

describe('resolveRemoteTreeRoot · initialPath 各种拿不到的情况都退回连接根', () => {
  it('配置里没有 initialPath（连接表还没加载完）', () => {
    expect(resolveRemoteTreeRoot(CONN, undefined)).toBe(REMOTE_ROOT);
  });

  it.each([
    ['空串', ''],
    ['远端根', '/'],
    ['多余斜杠', '///'],
    ['词法逃逸 ..', '/vol1/../etc'],
    ['词法逃逸 .', '/vol1/./etc'],
  ])('initialPath = %s 退回连接根', (_label, initialPath) => {
    // 宁可根浅一点，也不要树变成空的。
    expect(resolveRemoteTreeRoot(CONN, initialPath)).toBe(REMOTE_ROOT);
  });

  it('没有尾斜杠的正常值原样接上去', () => {
    expect(resolveRemoteTreeRoot(CONN, '/vol1/@team/study_platform/wp_root'))
      .toBe(REMOTE_INITIAL);
  });

  it('有尾斜杠也归一成同一个根', () => {
    expect(resolveRemoteTreeRoot(CONN, `${INITIAL}/`)).toBe(REMOTE_INITIAL);
  });

  it('非法连接 id 返回 null，由调用方退回盘符', () => {
    expect(resolveRemoteTreeRoot('', '/vol1')).toBeNull();
  });
});

describe('非法连接 id 退回盘符，而不是让树变空', () => {
  it.each([
    ['空连接 id', 'sfm://remote/'],
    ['只有 scheme 没有 id', 'sfm://remote'],
    ['空串', ''],
  ])('%s → 退回盘符', (_label, activePath) => {
    const result = resolveTreeRoots({
      activePath,
      drivePaths: DRIVE_PATHS,
      driveLabels: DRIVE_LABELS,
      connectionsById: CONNECTIONS,
    });

    expect(result.rootPaths).toEqual(['C:/', 'D:/']);
    expect(result.rootLabels).toEqual({
      'C:/': 'Win (C:)',
      'D/': 'Program (D:)',
    });
  });

  it('连接 id 里带斜杠不是非法 —— 它是「另一个连接」', () => {
    // `sfm://remote/a/b` 解析成连接 `a` 的 `/b`，`parseRemotePath` 按首个斜杠切分。
    // 写成「非法」会逼着实现去拒绝一条合法路径，那是把解析器的职责抢过来。
    const result = resolveTreeRoots({
      activePath: 'sfm://remote/a/b',
      drivePaths: DRIVE_PATHS,
      driveLabels: DRIVE_LABELS,
      connectionsById: {
        a: {
          name: 'A 连接',
          initialPath: '/data',
        },
      },
    });

    expect(result.rootPaths).toEqual(['sfm://remote/a/data']);
    expect(result.rootLabels).toEqual({ 'sfm://remote/a/data': 'A 连接' });
  });

  it('连接表里根本没有这条连接时退回连接根，而不是空树', () => {
    const result = resolveTreeRoots({
      activePath: `sfm://remote/${CONN}/vol1`,
      drivePaths: DRIVE_PATHS,
      driveLabels: DRIVE_LABELS,
      connectionsById: {},
    });

    expect(result.rootPaths).toEqual([REMOTE_ROOT]);
    expect(result.rootLabels).toEqual({ [REMOTE_ROOT]: CONN });
  });
});

// FORK-MODIFICATION 2026-11-19（批次 24）：焦点在本地时，树根 = 盘符 + shell 设备。
//
// 用户 2026-11-19 报的缺陷原话：「在位置中，已经可以显示手机，但在目录树无法显示手机」。
// 手机（MTP）、无盘符读卡器这类对象在 Windows 里不是卷，而是 shell 命名空间里的节点，
// 路径形如 `::{GUID}\?\usb#vid_22d9&pid_2765&mi_00`：整体是**不透明标识符**，
// 里面的 `\` 不是路径分隔符，`?` 是 shell 解析名的分隔标记。
// 所以这里只要求一件事：根标签把设备清单**原样**接在盘符后面，一个字都不许改。
const PHONE_ROOT = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\\usb#vid_22d9&pid_2765&mi_00';
const PHONE_LABEL = 'Pixel 7';
const CARD_ROOT = '::{ED834ED7-4B85-436A-ACF0-9C4B3E62ABD9}\\?\\usb#vid_0bda&pid_0129';
const SHELL_FOLDER_ROOT = 'shell:MyFolder';

function resolveDevices(
  activePath: string | undefined,
  deviceRoots: Parameters<typeof resolveTreeRoots>[0]['deviceRoots'],
) {
  return resolveTreeRoots({
    activePath,
    drivePaths: DRIVE_PATHS,
    driveLabels: DRIVE_LABELS,
    connectionsById: CONNECTIONS,
    deviceRoots,
  });
}

describe('焦点在本地时：盘符之后追加 shell 设备根', () => {
  it('没有设备（清单为空）时，根与标签和接入前完全一致', () => {
    // 枚举失败 / 尚未连上手机 ⇒ 静默降级，不许影响盘符根渲染。
    expect(resolveDevices('C:/Users/Duanyi', []).rootPaths).toEqual(['C:/', 'D:/']);
    expect(resolveDevices('C:/Users/Duanyi', []).rootLabels).toEqual({
      'C:/': 'Win (C:)',
      'D/': 'Program (D:)',
    });
  });

  it('没传 deviceRoots 时同样只有盘符（入参可选，老调用方不受影响）', () => {
    const result = resolveDevices('C:/', undefined);

    expect(result.rootPaths).toEqual(['C:/', 'D:/']);
  });

  it('设备根接在盘符之后，顺序是「盘符 → 设备」', () => {
    const result = resolveDevices('C:/Users/Duanyi', [
      {
        path: PHONE_ROOT,
        label: PHONE_LABEL,
      },
      {
        path: CARD_ROOT,
        label: 'SD Reader',
      },
    ]);

    expect(result.rootPaths).toEqual(['C:/', 'D:/', PHONE_ROOT, CARD_ROOT]);
  });

  it('设备标签用设备显示名（用户看得见的名字，不是一串 GUID）', () => {
    const result = resolveDevices('C:/Users/Duanyi', [{
      path: PHONE_ROOT,
      label: PHONE_LABEL,
    }]);

    expect(result.rootLabels[PHONE_ROOT]).toBe(PHONE_LABEL);
  });

  it('shell 路径原样当根 key：不切段、不归一、不丢 ? 和 &', () => {
    // 断言的是**逐字符相等**。任何 normalize 都会让设备从树上消失（批次 23 的故障形态）。
    const result = resolveDevices('E:/x', [{
      path: PHONE_ROOT,
      label: PHONE_LABEL,
    }]);

    expect(result.rootPaths).toContain(PHONE_ROOT);
    expect(result.rootPaths[2]).not.toContain('\\\\');
    expect(result.rootPaths.some(p => p === PHONE_ROOT)).toBe(true);
    // 没有哪一段被单独立成根
    expect(result.rootPaths).not.toContain('::{20D04FE0-3AEA-1069-A2D8-08002B30309D}');
    expect(result.rootPaths).not.toContain('usb#vid_22d9&pid_2765&mi_00');
  });

  it('设备没有标签时兜底成可读的名字，而不是空白或裸 GUID', () => {
    const result = resolveDevices('C:/', [
      { path: PHONE_ROOT },
      { path: SHELL_FOLDER_ROOT },
      { path: '   ' },
    ]);

    // `::{GUID}\?...` 里没有任何一段是名字 ⇒ 兜底口径与「位置」页一致（virtual-locations）。
    expect(result.rootLabels[PHONE_ROOT]).toBe('Device');
    // `shell:MyFolder` 最后一段本身就是名字。
    expect(result.rootLabels[SHELL_FOLDER_ROOT]).toBe('MyFolder');
    // 空白路径直接丢掉，不占一个根位。
    expect(result.rootPaths).toEqual(['C:/', 'D:/', PHONE_ROOT, SHELL_FOLDER_ROOT]);
  });

  it('标签是空串或全空格时同样走兜底', () => {
    const result = resolveDevices('C:/', [
      {
        path: SHELL_FOLDER_ROOT,
        label: '   ',
      },
    ]);

    expect(result.rootLabels[SHELL_FOLDER_ROOT]).toBe('MyFolder');
  });

  it('重复的设备路径只留一个根，标签取第一个有值的', () => {
    const result = resolveDevices('C:/', [
      {
        path: PHONE_ROOT,
        label: PHONE_LABEL,
      },
      {
        path: PHONE_ROOT,
        label: '同一台手机第二次枚举',
      },
    ]);

    expect(result.rootPaths).toEqual(['C:/', 'D:/', PHONE_ROOT]);
    expect(result.rootLabels[PHONE_ROOT]).toBe(PHONE_LABEL);
  });

  it('设备路径与盘符重复时不产生并列的第二个根', () => {
    const result = resolveDevices('C:/', [{
      path: 'C:/',
      label: '不该出现两遍',
    }]);

    expect(result.rootPaths).toEqual(['C:/', 'D:/']);
    expect(result.rootLabels['C:/']).toBe('Win (C:)');
  });

  it('本地根的标签仍是独立副本：追加设备标签不污染调用方传进来的 driveLabels', () => {
    const result = resolveDevices('C:/', [{
      path: PHONE_ROOT,
      label: PHONE_LABEL,
    }]);

    expect((DRIVE_LABELS as Record<string, string | undefined>)[PHONE_ROOT]).toBeUndefined();
    expect(result.rootLabels).not.toBe(DRIVE_LABELS);
  });
});

describe('焦点在远程时：设备不进远程树（要求 ①）', () => {
  it('远程分支优先，deviceRoots 一个都不接', () => {
    const result = resolveDevices(`sfm://remote/${CONN}/vol1`, [
      {
        path: PHONE_ROOT,
        label: PHONE_LABEL,
      },
    ]);

    expect(result.rootPaths).toEqual([REMOTE_INITIAL]);
    expect(result.rootLabels).toEqual({ [REMOTE_INITIAL]: '速豹堂' });
    expect(result.rootLabels[PHONE_ROOT]).toBeUndefined();
  });

  it('非法连接 id 退回本地时，设备根照样接上（退回的是本地分支，不是空树）', () => {
    const result = resolveTreeRoots({
      activePath: 'sfm://remote/',
      drivePaths: DRIVE_PATHS,
      driveLabels: DRIVE_LABELS,
      connectionsById: CONNECTIONS,
      deviceRoots: [{
        path: PHONE_ROOT,
        label: PHONE_LABEL,
      }],
    });

    expect(result.rootPaths).toEqual(['C:/', 'D:/', PHONE_ROOT]);
  });
});
