// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 阿里云盘协议的**前端镜像**护栏。
//
// 与夸克那份同构，但多锁两件事：
//
//  1. **「网盘」是三家，不是一家** —— `isNetdiskProtocol` 必须同时认百度、
//     夸克与阿里云盘。漏掉阿里云盘的表现是「加了阿里云盘，侧边栏却把它
//     归到『远程连接』分区」。
//  2. **授权状态只有四种分类** —— 后端只吐
//     `missing | ready | stale | stale_soon`。「正在读取」是前端的本地状态，
//     不是后端的返回值；把它塞进后端分类里，后端就会多出一个前端永远等不到的分支。
//
// 另加一条：**主机与端口是固定值**（`api.aliyundrive.com:443`），
// 用户填什么都不参与请求。

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  getRemoteProtocolDefaultPort,
  isAliyunPanProtocol,
  isBaiduPanProtocol,
  isNetdiskProtocol,
  isQuarkPanProtocol,
  NETDISK_PROTOCOLS,
} from '@/types/remote';

// 本文件在 `src/types/__tests__/` 下，往上三层才是仓库根。
const REPO_ROOT = join(__dirname, '..', '..', '..');
const TYPES_TS = join(REPO_ROOT, 'src', 'types', 'remote.ts');

/**
 * 从类型源文件里抽出某个**字符串字面量联合**的全部成员。
 *
 * 类型别名在运行期是不存在的，断言不了 ⇒ 只能读源码。
 * 这是唯一能在「有人往联合里多加一个成员」时立刻变红的办法。
 */
function extractUnionMembers(source: string, typeName: string): string[] {
  const pattern = new RegExp(`export type ${typeName}\\s*=\\s*((?:\\|\\s*'[a-z_]+'\\s*)+);`);
  const match = pattern.exec(source);

  if (!match) {
    return [];
  }

  return [...match[1].matchAll(/'([a-z_]+)'/g)].map(entry => entry[1]);
}

const typesSource = readFileSync(TYPES_TS, 'utf8');

describe('阿里云盘协议的前端镜像', () => {
  it('协议名与后端逐字一致', () => {
    // 后端写进 `user-remote-connections.json` 的就是这个字符串。
    // 改了这里而不改后端，老连接就读不出来了。
    expect(isAliyunPanProtocol('aliyun_pan')).toBe(true);
    expect(isAliyunPanProtocol('baidu_pan')).toBe(false);
    expect(isAliyunPanProtocol('quark_pan')).toBe(false);
    expect(isAliyunPanProtocol('sftp')).toBe(false);
    expect(isAliyunPanProtocol('ftp')).toBe(false);
  });

  it('默认端口是 HTTPS 的 443', () => {
    expect(getRemoteProtocolDefaultPort('aliyun_pan')).toBe(443);
  });

  it('其余协议的默认端口没被带偏', () => {
    expect(getRemoteProtocolDefaultPort('ftp')).toBe(21);
    expect(getRemoteProtocolDefaultPort('ftps_explicit')).toBe(21);
    expect(getRemoteProtocolDefaultPort('ftps_implicit')).toBe(990);
    expect(getRemoteProtocolDefaultPort('sftp')).toBe(22);
    expect(getRemoteProtocolDefaultPort('baidu_pan')).toBe(443);
    expect(getRemoteProtocolDefaultPort('quark_pan')).toBe(443);
  });

  it('网盘是一个类：三家都算网盘，其余协议不算', () => {
    expect(isNetdiskProtocol('baidu_pan')).toBe(true);
    expect(isNetdiskProtocol('quark_pan')).toBe(true);
    expect(isNetdiskProtocol('aliyun_pan')).toBe(true);
    expect(isNetdiskProtocol('ftp')).toBe(false);
    expect(isNetdiskProtocol('ftps_explicit')).toBe(false);
    expect(isNetdiskProtocol('ftps_implicit')).toBe(false);
    expect(isNetdiskProtocol('sftp')).toBe(false);
  });

  it('网盘协议清单里确实有阿里云盘', () => {
    expect(NETDISK_PROTOCOLS).toContain('aliyun_pan');
    // 批次 38 起网盘协议清单是四家：OneDrive 也走网盘这条链路。
    expect([...NETDISK_PROTOCOLS]).toEqual(['baidu_pan', 'quark_pan', 'aliyun_pan', 'onedrive_pan']);
  });

  it('三家网盘互不串味', () => {
    expect(isBaiduPanProtocol('aliyun_pan')).toBe(false);
    expect(isQuarkPanProtocol('aliyun_pan')).toBe(false);
    expect(isAliyunPanProtocol('baidu_pan')).toBe(false);
    expect(isAliyunPanProtocol('quark_pan')).toBe(false);
  });

  it('授权状态只有四种分类，不含 loading', () => {
    // 「正在读取」是前端的本地状态（等后端回答时自己显示），
    // 不是后端的返回值 —— 后端多吐一个分类，前端就会多等一个永远不会来的分支。
    expect(extractUnionMembers(typesSource, 'AliyunAuthStatusKind')).toEqual([
      'missing',
      'ready',
      'stale',
      'stale_soon',
    ]);
  });

  it('扫码状态有六种，后端多吐一种前端就会多等一个永远不会来的分支', () => {
    expect(extractUnionMembers(typesSource, 'AliyunQrStatusKind')).toEqual([
      'new',
      'scanned',
      'confirmed',
      'expired',
      'canceled',
      'error',
    ]);
  });

  it('扫码登录不再有事件频道 —— 进度由前端轮询拿', () => {
    expect(typesSource).not.toContain('AliyunLoginEventPayload');
  });
});
