// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 夸克网盘协议的**前端镜像**护栏。
//
// 与 `remote-protocol-baidu.test.ts` 同构，但多锁两件事：
//
//  1. **「网盘」是一个类，不是一个协议** —— `isNetdiskProtocol` 必须同时认
//     百度与夸克。漏掉的话，侧边栏的分区归类会把夸克连接塞进「远程连接」
//     分区，用户在网盘分区里看不到自己刚加的夸克。
//  2. **主机与端口是固定值** —— 夸克没有「自选服务器」可言，表单里填的
//     主机名不参与请求；端口固定 443。
//
// 这些断言全都不会报错，只会「界面看着少一个选项」或「端口填错」。

import { describe, expect, it } from 'vitest';
import {
  getRemoteProtocolDefaultPort,
  isBaiduPanProtocol,
  isNetdiskProtocol,
  isQuarkPanProtocol,
} from '@/types/remote';

describe('夸克网盘协议的前端镜像', () => {
  it('协议名与后端逐字一致', () => {
    // 后端写进 `user-remote-connections.json` 的就是这个字符串。
    // 改了这里而不改后端，老连接就读不出来了。
    expect(isQuarkPanProtocol('quark_pan')).toBe(true);
    expect(isQuarkPanProtocol('baidu_pan')).toBe(false);
    expect(isQuarkPanProtocol('sftp')).toBe(false);
    expect(isQuarkPanProtocol('ftp')).toBe(false);
  });

  it('默认端口是 HTTPS 的 443', () => {
    expect(getRemoteProtocolDefaultPort('quark_pan')).toBe(443);
  });

  it('其余协议的默认端口没被带偏', () => {
    expect(getRemoteProtocolDefaultPort('ftp')).toBe(21);
    expect(getRemoteProtocolDefaultPort('ftps_explicit')).toBe(21);
    expect(getRemoteProtocolDefaultPort('ftps_implicit')).toBe(990);
    expect(getRemoteProtocolDefaultPort('sftp')).toBe(22);
    expect(getRemoteProtocolDefaultPort('baidu_pan')).toBe(443);
  });

  it('网盘是一个类：百度与夸克都算网盘，其余协议不算', () => {
    expect(isNetdiskProtocol('baidu_pan')).toBe(true);
    expect(isNetdiskProtocol('quark_pan')).toBe(true);
    expect(isNetdiskProtocol('ftp')).toBe(false);
    expect(isNetdiskProtocol('ftps_explicit')).toBe(false);
    expect(isNetdiskProtocol('ftps_implicit')).toBe(false);
    expect(isNetdiskProtocol('sftp')).toBe(false);
  });

  it('两个网盘协议互不串味', () => {
    expect(isBaiduPanProtocol('quark_pan')).toBe(false);
    expect(isQuarkPanProtocol('baidu_pan')).toBe(false);
  });
});
