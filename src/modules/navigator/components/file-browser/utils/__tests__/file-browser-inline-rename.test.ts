// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * FORK-MODIFICATION 2026-10-27：就地重命名的纯逻辑单测。
 *
 * 这一层只放纯函数（校验、选区、方向键意图、连续改名状态机），
 * 不碰 Vue、不碰 DOM —— 所以可以在这里把状态机的每条分支都钉死。
 *
 * 校验规则与 `file-browser-rename-dialog.vue` 的语义必须一致：
 * 两边各写一套的话，模态框能改的名字就地改名就改不了。
 */

import { describe, expect, it } from 'vitest';
import {
  getInlineRenameChainStateAfterCancel,
  getInlineRenameChainStateAfterCommit,
  getInlineRenameMoveIntent,
  getInlineRenameNameWithoutExtension,
  getInlineRenameSelectionRange,
  isValidInlineRenameName,
  shouldEnterInlineRenameOnNavigationTarget,
} from '../file-browser-inline-rename';

describe('就地重命名 · 文件名校验', () => {
  it('空串与空白串不合法', () => {
    expect(isValidInlineRenameName('')).toBe(false);
    expect(isValidInlineRenameName('   ')).toBe(false);
  });

  it('带非法字符的名字不合法', () => {
    expect(isValidInlineRenameName('a/b')).toBe(false);
    expect(isValidInlineRenameName('a\\b')).toBe(false);
    expect(isValidInlineRenameName('a:b')).toBe(false);
    expect(isValidInlineRenameName('a*b')).toBe(false);
    expect(isValidInlineRenameName('a?b')).toBe(false);
    expect(isValidInlineRenameName('a|b')).toBe(false);
    expect(isValidInlineRenameName('a<b')).toBe(false);
    expect(isValidInlineRenameName('a>b')).toBe(false);
    expect(isValidInlineRenameName('a"b')).toBe(false);
  });

  it('`.` 与 `..` 不合法', () => {
    expect(isValidInlineRenameName('.')).toBe(false);
    expect(isValidInlineRenameName('..')).toBe(false);
  });

  it('普通名字合法，两端的空白会被忽略', () => {
    expect(isValidInlineRenameName('report.txt')).toBe(true);
    expect(isValidInlineRenameName('  名字  ')).toBe(true);
    expect(isValidInlineRenameName('...')).toBe(true);
  });
});

describe('就地重命名 · 扩展名与选区', () => {
  it('只有 lastIndexOf(".") > 0 才算扩展名', () => {
    expect(getInlineRenameNameWithoutExtension('report.txt')).toBe('report');
    expect(getInlineRenameNameWithoutExtension('archive.tar.gz')).toBe('archive.tar');
    expect(getInlineRenameNameWithoutExtension('.gitignore')).toBe('.gitignore');
    expect(getInlineRenameNameWithoutExtension('README')).toBe('README');
  });

  it('文件选中不含扩展名的那一段', () => {
    expect(getInlineRenameSelectionRange('report.txt', false)).toEqual({
      start: 0,
      end: 6,
    });
    expect(getInlineRenameSelectionRange('.gitignore', false)).toEqual({
      start: 0,
      end: 10,
    });
  });

  it('目录全选', () => {
    expect(getInlineRenameSelectionRange('folder', true)).toEqual({
      start: 0,
      end: 6,
    });
  });
});

describe('就地重命名 · 方向键意图', () => {
  it('上下方向键映射成 up / down', () => {
    expect(getInlineRenameMoveIntent('ArrowUp')).toBe('up');
    expect(getInlineRenameMoveIntent('ArrowDown')).toBe('down');
  });

  it('其它按键没有移动意图', () => {
    expect(getInlineRenameMoveIntent('Enter')).toBeNull();
    expect(getInlineRenameMoveIntent('Escape')).toBeNull();
    expect(getInlineRenameMoveIntent('ArrowLeft')).toBeNull();
    expect(getInlineRenameMoveIntent('a')).toBeNull();
  });
});

describe('就地重命名 · 连续改名状态机', () => {
  it('回车提交成功 ⇒ 退出编辑态，但连续模式继续开着', () => {
    expect(getInlineRenameChainStateAfterCommit('enter', 'renamed')).toEqual({
      isEditing: false,
      isChainActive: true,
    });
  });

  it('方向键提交成功 ⇒ 退出编辑态，但连续模式继续开着', () => {
    expect(getInlineRenameChainStateAfterCommit('move', 'renamed')).toEqual({
      isEditing: false,
      isChainActive: true,
    });
  });

  it('名字没变也算提交成功（连续模式同样继续）', () => {
    expect(getInlineRenameChainStateAfterCommit('enter', 'unchanged')).toEqual({
      isEditing: false,
      isChainActive: true,
    });
  });

  it('失焦提交 ⇒ 编辑态与连续模式一起结束', () => {
    expect(getInlineRenameChainStateAfterCommit('blur', 'renamed')).toEqual({
      isEditing: false,
      isChainActive: false,
    });
  });

  it('提交失败 ⇒ 连续模式结束；回车与方向键留在编辑态，失焦退出编辑态', () => {
    expect(getInlineRenameChainStateAfterCommit('enter', 'failed')).toEqual({
      isEditing: true,
      isChainActive: false,
    });
    expect(getInlineRenameChainStateAfterCommit('move', 'failed')).toEqual({
      isEditing: true,
      isChainActive: false,
    });
    expect(getInlineRenameChainStateAfterCommit('blur', 'failed')).toEqual({
      isEditing: false,
      isChainActive: false,
    });
  });

  it('取消 ⇒ 编辑态与连续模式一起结束', () => {
    expect(getInlineRenameChainStateAfterCancel()).toEqual({
      isEditing: false,
      isChainActive: false,
    });
  });
});

describe('就地重命名 · 导航落点', () => {
  it('只有「连续模式开着」且「布局支持就地编辑」才落进编辑态', () => {
    expect(shouldEnterInlineRenameOnNavigationTarget(true, true)).toBe(true);
    expect(shouldEnterInlineRenameOnNavigationTarget(true, false)).toBe(false);
    expect(shouldEnterInlineRenameOnNavigationTarget(false, true)).toBe(false);
    expect(shouldEnterInlineRenameOnNavigationTarget(false, false)).toBe(false);
  });
});
