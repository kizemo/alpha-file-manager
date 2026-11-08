// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 百度网盘协议的**前端镜像**护栏。
//
// 后端 `RemoteProtocol` 加了 `BaiduPan` 变体，前端这边必须同步三件事：
// 类型联合、默认端口、以及「是不是百度」的判定函数。
// 漏掉任何一件，后果都不报错，只是界面上少一个选项或端口填错。

import { describe, expect, it } from 'vitest';
import {
  getRemoteProtocolDefaultPort,
  isBaiduPanProtocol,
} from '@/types/remote';

describe('百度网盘协议的前端镜像', () => {
  it('协议名与后端逐字一致', () => {
    // 后端写进 `user-remote-connections.json` 的就是这个字符串。
    // 改了这里而不改后端，老连接就读不出来了。
    expect(isBaiduPanProtocol('baidu_pan')).toBe(true);
    expect(isBaiduPanProtocol('sftp')).toBe(false);
    expect(isBaiduPanProtocol('ftp')).toBe(false);
  });

  it('默认端口是 HTTPS 的 443', () => {
    expect(getRemoteProtocolDefaultPort('baidu_pan')).toBe(443);
  });

  it('其余协议的默认端口没被带偏', () => {
    expect(getRemoteProtocolDefaultPort('ftp')).toBe(21);
    expect(getRemoteProtocolDefaultPort('ftps_explicit')).toBe(21);
    expect(getRemoteProtocolDefaultPort('ftps_implicit')).toBe(990);
    expect(getRemoteProtocolDefaultPort('sftp')).toBe(22);
  });
});
