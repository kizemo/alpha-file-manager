// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-03：Shift+Del 永久删除「假失败」的前端闸门契约。
//
// ## 这道门禁为什么存在
//
// 真机日志（2026-11-03 15:06 本地）实测：第一个删除作业跑了 11.8 秒且期间
// 永久删除一条进度都没有，用户以为没反应连按三次，四个作业打到同一个文件 ——
// 第一个真的删掉了，后三个亚毫秒内拿到 os error 5 / os error 2，
// 前端各弹一条「删除失败」，文件其实早没了。这就是「假失败」。
//
// 修复分两层：后端（delete_tree.rs 的已不存在=成功、瞬态重试、心跳）与
// 前端（本 store 的按路径 in-flight 去重）。后端三层都有 Rust 单测守着，
// 这里守前端这一层：`normalizeDeletePathKey` 的行为 + 守卫在源码里的形态。
// 形态断言沿用本仓 `recycle-bin-delete-contract.test.ts` 的源码文本契约风格 ——
// 不依赖 Tauri 运行时 mock，改坏接线立刻红。

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { normalizeDeletePathKey } from '../delete-jobs';

const STORE_SOURCE = readFileSync(join(__dirname, '..', 'delete-jobs.ts'), 'utf8');

describe('normalizeDeletePathKey：in-flight 表的键', () => {
  it('Windows 路径统一成分隔符与大小写', () => {
    expect(normalizeDeletePathKey('E:\\办公文件\\a.xlsx')).toBe('e:/办公文件/a.xlsx');
    expect(normalizeDeletePathKey('e:/办公文件/a.xlsx')).toBe('e:/办公文件/a.xlsx');
    expect(normalizeDeletePathKey('C:\\tmp\\')).toBe('c:/tmp');
    expect(normalizeDeletePathKey('C:/tmp/')).toBe('c:/tmp');
  });

  it('POSIX 路径保持大小写敏感', () => {
    expect(normalizeDeletePathKey('/data/A')).toBe('/data/A');
    expect(normalizeDeletePathKey('/data/a')).toBe('/data/a');
    // 大小写敏感的两者不能被归一成同一个键。
    expect(normalizeDeletePathKey('/data/A')).not.toBe(normalizeDeletePathKey('/data/a'));
  });

  it('反斜杠与正斜杠的同一 Windows 路径得到同一个键', () => {
    expect(normalizeDeletePathKey('E:\\a\\b.dat'))
      .toBe(normalizeDeletePathKey('E:/a/b.dat'));
  });
});

describe('删除作业 in-flight 守卫的源码契约', () => {
  it('startJob 在任何 await 之前完成「查表 → 生成 jobId → 登记」', () => {
    const startJobIndex = STORE_SOURCE.indexOf('async function startJob(');
    expect(startJobIndex).toBeGreaterThan(-1);
    const body = STORE_SOURCE.slice(startJobIndex);
    const endIndex = body.indexOf('\n  async function cancelJob');
    expect(endIndex).toBeGreaterThan(-1);
    const startJobBody = body.slice(0, endIndex);

    // startJob 体内不允许出现代码级 await —— 登记与查表必须全程同步。
    // （只匹配 `await <标识符>` 的形态，注释里的「await」字样不误伤。）
    expect(startJobBody).not.toMatch(/\bawait\s+[\w$]/);
    // 唯一Id 生成与登记都在 runJob 委托之前。
    expect(startJobBody.indexOf('const jobId = uniqueId();'))
      .toBeLessThan(startJobBody.indexOf('inFlightPaths.set(key, jobId);'));
    expect(startJobBody.indexOf('inFlightPaths.set(key, jobId);'))
      .toBeLessThan(startJobBody.indexOf('runJob(jobId, paths, useTrash, options)'));
  });

  it('命中在飞作业时复用其结果，不再起第二个作业', () => {
    const startJobIndex = STORE_SOURCE.indexOf('async function startJob(');
    const startJobBody = STORE_SOURCE.slice(startJobIndex);
    expect(startJobBody).toContain('const existingResult = inFlightResults.get(attachedJobId);');
    expect(startJobBody).toContain('return existingResult;');
  });

  it('整批路径必须命中同一个 jobId 才复用', () => {
    // 命中不同作业时必须放弃 attach —— 否则会静默漏删没被覆盖的路径。
    expect(STORE_SOURCE).toContain('// 命中了两个不同的在飞作业 —— 没有单一作业覆盖整批，另起。');
    const runJobIndex = STORE_SOURCE.indexOf('async function runJob(');
    const startJobIndex = STORE_SOURCE.indexOf('async function startJob(');
    expect(runJobIndex).toBeGreaterThan(-1);
    expect(startJobIndex).toBeGreaterThan(runJobIndex);
  });

  it('作业结束后在 finally 里清理，且只删仍指向自己的键', () => {
    expect(STORE_SOURCE).toContain('.finally(cleanup);');
    expect(STORE_SOURCE).toContain('if (inFlightPaths.get(key) === jobId) {');
    expect(STORE_SOURCE).toContain('inFlightResults.delete(jobId);');
  });

  it('真正的 invoke 只存在于 runJob，路径必须带进来', () => {
    expect(STORE_SOURCE).toContain('invoke(\'start_delete_job\', {');
    // jobId 由 startJob 生成后传入，runJob 自己不再造 id —— 否则登记表会失配。
    const runJobBody = STORE_SOURCE.slice(
      STORE_SOURCE.indexOf('async function runJob('),
      STORE_SOURCE.indexOf('async function startJob('),
    );
    expect(runJobBody).not.toContain('uniqueId()');
  });
});
