// SPDX-License-Identifier: MIT
// Git 工具扩展：为 Alpha File Manager 提供仓库初始化、克隆、抓取、拉取、推送、
// 同步、分支与提交浏览，以及把仓库交给外部 IDE 打开的能力。
//
// 运行环境是宿主提供的 Web Worker 沙箱：只能通过 `sigma` 命名空间访问能力，
// 没有 DOM、没有网络 API、没有任务队列。所有 git 调用都走 `sigma.shell`。

import { FALLBACKS } from './generated-fallbacks.js';

const REPO_DIR_KEY = 'lastRepoDir';
const IDE_COMMAND_KEY = 'ideCommand';
const DEFAULT_IDE_COMMAND = 'code';

// `git status --porcelain=v1` 的两位状态码到文案 key 的映射。
const STATUS_CODE_LABELS = {
  'M': 'status.modified',
  'A': 'status.added',
  'D': 'status.deleted',
  'R': 'status.renamed',
  'C': 'status.copied',
  'U': 'status.conflicted',
  '?': 'status.untracked',
  '!': 'status.ignored',
};

const disposables = [];
let lastKnownPath = null;

// ---------------------------------------------------------------- 基础工具

function track(disposable) {
  if (disposable && typeof disposable.dispose === 'function') {
    disposables.push(disposable);
  }
}

function interpolate(template, params) {
  if (!params) {
    return template;
  }
  return template.replace(/\{(\w+)\}/g, (match, key) => {
    const value = params[key];
    return value === undefined || value === null ? match : String(value);
  });
}

/**
 * 取文案：优先用宿主合并进来的语言包，缺键时退回内置的中文表。
 * 两者都没有时返回 key 本身，便于在验收截图里直接看出缺键。
 */
function tr(key, params) {
  try {
    const value = sigma.i18n.extensionT(key, params);
    if (typeof value === 'string' && value && value !== key) {
      return value;
    }
  }
  catch {
    // 宿主语言包不可用时走内置文案。
  }
  const fallback = FALLBACKS[key];
  if (typeof fallback !== 'string') {
    return key;
  }
  return interpolate(fallback, params);
}

async function readStorage(key) {
  try {
    const value = await sigma.storage.get(key);
    return typeof value === 'string' ? value : null;
  }
  catch {
    return null;
  }
}

async function writeStorage(key, value) {
  try {
    await sigma.storage.set(key, value);
  }
  catch {
    // 写失败不影响本次操作。
  }
}

function notify(type, subtitle, description) {
  try {
    sigma.ui.showNotification({
      title: tr('extensionName'),
      subtitle,
      description,
      type,
    });
  }
  catch {
    // 通知失败不影响主流程。
  }
}

async function showInfo(title, message) {
  try {
    await sigma.ui.showDialog({ type: 'info', title, message });
  }
  catch {
    notify('info', title, message);
  }
}

async function askPrompt(title, message, defaultValue) {
  try {
    const result = await sigma.ui.showDialog({
      type: 'prompt',
      title,
      message,
      defaultValue: defaultValue || '',
    });
    if (!result || !result.confirmed) {
      return null;
    }
    const value = typeof result.value === 'string' ? result.value.trim() : '';
    return value || null;
  }
  catch {
    return null;
  }
}

// ---------------------------------------------------------------- 目录定位

async function currentPath() {
  try {
    const value = await sigma.context.getCurrentPath();
    if (value) {
      lastKnownPath = value;
      return value;
    }
  }
  catch {
    // 拿不到就退回已知的上一次路径。
  }
  return lastKnownPath;
}

/**
 * 解析本次操作要作用的目录。优先级：显式路径 > 当前导航目录 > 上次用过的仓库 > 询问用户。
 */
async function resolveDirectory(options = {}) {
  if (options.path) {
    return options.path;
  }
  const current = await currentPath();
  if (current) {
    return current;
  }
  const remembered = await readStorage(REPO_DIR_KEY);
  const answer = await askPrompt(
    tr('common.repoPathLabel'),
    tr('common.repoPathPrompt'),
    remembered || '',
  );
  return answer;
}

function directoryOfEntry(entry) {
  if (!entry || typeof entry.path !== 'string' || !entry.path) {
    return null;
  }
  if (entry.isDirectory) {
    return entry.path;
  }
  const normalized = entry.path.replace(/[\\/]+$/, '');
  const index = Math.max(normalized.lastIndexOf('\\'), normalized.lastIndexOf('/'));
  return index > 0 ? normalized.slice(0, index) : normalized;
}

// ---------------------------------------------------------------- git 调用

async function runGitPlain(directory, args) {
  const result = await sigma.shell.run('git', ['-C', directory, ...args]);
  return {
    code: typeof result.code === 'number' ? result.code : -1,
    stdout: typeof result.stdout === 'string' ? result.stdout : '',
    stderr: typeof result.stderr === 'string' ? result.stderr : '',
  };
}

/**
 * 带进度条地跑 git。
 * 三条分支各自只启动一次 git 进程：不会因为「降级」而把同一条命令跑两遍。
 */
async function runGitWithProgress(directory, args, subtitle) {
  const canStream = typeof sigma.shell.runWithProgress === 'function';
  const canWrap = typeof sigma.ui.withProgress === 'function';

  if (!canWrap) {
    return runGitPlain(directory, args);
  }

  return sigma.ui.withProgress(
    { subtitle, location: 'statusBar', cancellable: true },
    async (progress) => {
      if (!canStream) {
        return runGitPlain(directory, args);
      }
      const handle = await sigma.shell.runWithProgress(
        'git',
        ['-C', directory, ...args],
        (payload) => {
          const line = String((payload && payload.line) || '').trim();
          if (!line) {
            return;
          }
          try {
            progress.report({ subtitle: line.slice(0, 120) });
          }
          catch {
            // 进度条已经消失时忽略。
          }
        },
      );
      const result = await handle.result;
      return {
        code: typeof result.code === 'number' ? result.code : -1,
        stdout: typeof result.stdout === 'string' ? result.stdout : '',
        stderr: typeof result.stderr === 'string' ? result.stderr : '',
      };
    },
  );
}

function failureText(result, fallbackKey) {
  const stderr = (result.stderr || '').trim();
  if (stderr) {
    return stderr;
  }
  const stdout = (result.stdout || '').trim();
  if (stdout) {
    return stdout;
  }
  return tr(fallbackKey);
}

async function repoRoot(directory) {
  try {
    const result = await runGitPlain(directory, ['rev-parse', '--show-toplevel']);
    if (result.code !== 0) {
      return null;
    }
    const root = result.stdout.trim();
    return root || null;
  }
  catch {
    return null;
  }
}

/**
 * 定位一个仓库根目录。目录不是仓库时返回 { root: null, reason }。
 */
async function resolveRepo(directory) {
  if (!directory) {
    return { root: null, reason: tr('error.noDirectory') };
  }
  const root = await repoRoot(directory);
  if (!root) {
    return { root: null, reason: tr('error.notARepoDetail') };
  }
  return { root, reason: null };
}

// ---------------------------------------------------------------- 输出解析

function parseStatus(output) {
  const lines = String(output || '').split(/\r?\n/);
  let branchLine = '';
  const changes = [];
  for (const line of lines) {
    if (!line.trim()) {
      continue;
    }
    if (line.startsWith('##')) {
      branchLine = line.slice(2).trim();
      continue;
    }
    const code = line.slice(0, 2);
    const file = line.slice(3);
    changes.push({ code, file });
  }
  let branch = branchLine;
  let upstream = '';
  const dotIndex = branchLine.indexOf('...');
  if (dotIndex >= 0) {
    branch = branchLine.slice(0, dotIndex).trim();
    upstream = branchLine.slice(dotIndex + 3).trim();
  }
  return { branch, upstream, changes };
}

function parseLsTreeLine(line) {
  const tabIndex = line.indexOf('\t');
  if (tabIndex < 0) {
    return null;
  }
  const meta = line.slice(0, tabIndex).trim().split(/\s+/);
  if (meta.length < 3) {
    return null;
  }
  const rawSize = meta[3];
  return {
    mode: meta[0],
    type: meta[1],
    sha: meta[2],
    size: rawSize && rawSize !== '-' ? Number(rawSize) : 0,
    path: line.slice(tabIndex + 1),
  };
}

// ---------------------------------------------------------------- 命令实现

async function showStatus() {
  const directory = await resolveDirectory();
  if (!directory) {
    notify('warning', tr('error.noDirectory'));
    return;
  }
  const repo = await resolveRepo(directory);
  if (!repo.root) {
    notify('warning', tr('error.notARepo'), repo.reason);
    return;
  }
  const result = await runGitPlain(repo.root, ['status', '--porcelain=v1', '--branch']);
  if (result.code !== 0) {
    notify('error', tr('error.statusFailed'), failureText(result, 'error.unknown'));
    return;
  }
  const status = parseStatus(result.stdout);
  const lines = [];
  lines.push(`${tr('status.root')}: ${repo.root}`);
  lines.push(`${tr('status.branch')}: ${status.branch || '-'}`);
  lines.push(`${tr('status.upstream')}: ${status.upstream || '-'}`);
  if (!status.changes.length) {
    lines.push(`${tr('status.changes')}: 0`);
    lines.push(tr('status.cleanDetail'));
  }
  else {
    lines.push(`${tr('status.changes')}: ${status.changes.length}`);
    for (const change of status.changes) {
      const labelKey = STATUS_CODE_LABELS[change.code.trim()] || STATUS_CODE_LABELS[change.code[0]];
      const label = labelKey ? tr(labelKey) : change.code.trim();
      lines.push(`  [${label}] ${change.file}`);
    }
  }
  await showInfo(tr('status.title'), lines.join('\n'));
}

async function runInit(directory) {
  if (!directory) {
    notify('warning', tr('error.noDirectory'));
    return;
  }
  const existing = await repoRoot(directory);
  if (existing) {
    notify('info', tr('init.exists'), existing);
    return;
  }
  const result = await runGitPlain(directory, ['init']);
  if (result.code !== 0) {
    notify('error', tr('init.title'), failureText(result, 'error.unknown'));
    return;
  }
  notify('success', tr('init.success'), directory);
}

async function showInit(context) {
  const entry = context && context.selectedEntries ? context.selectedEntries[0] : null;
  const directory = directoryOfEntry(entry) || (await resolveDirectory());
  await runInit(directory);
}

async function runClone() {
  const url = await askPrompt(tr('clone.urlLabel'), tr('clone.urlRequired'), '');
  if (!url) {
    return;
  }
  const base = (await currentPath()) || (await readStorage(REPO_DIR_KEY)) || '';
  const name = url.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || 'repo';
  const suggested = base ? `${base.replace(/[\\/]+$/, '')}\\${name.replace(/\.git$/, '')}` : name;
  const target = await askPrompt(tr('clone.dirLabel'), tr('clone.dirRequired'), suggested);
  if (!target) {
    return;
  }
  const result = await runGitWithProgress('', ['clone', url, target], tr('clone.progress'));
  if (result.code !== 0) {
    notify('error', tr('clone.title'), failureText(result, 'error.unknown'));
    return;
  }
  await writeStorage(REPO_DIR_KEY, target);
  notify('success', tr('clone.success'), target);
}

async function runRemoteOperation(args, titleKey, successKey) {
  const directory = await resolveDirectory();
  if (!directory) {
    notify('warning', tr('error.noDirectory'));
    return;
  }
  const repo = await resolveRepo(directory);
  if (!repo.root) {
    notify('warning', tr('error.notARepo'), repo.reason);
    return;
  }
  const result = await runGitWithProgress(repo.root, args, tr(titleKey));
  if (result.code !== 0) {
    notify('error', tr(titleKey), failureText(result, 'error.unknown'));
    return;
  }
  const detail = (result.stdout || '').trim() || (result.stderr || '').trim();
  notify('success', tr(successKey), detail);
}

/** 同步 = 先抓取远端，再拉取当前分支。 */
async function runSync() {
  const directory = await resolveDirectory();
  if (!directory) {
    notify('warning', tr('error.noDirectory'));
    return;
  }
  const repo = await resolveRepo(directory);
  if (!repo.root) {
    notify('warning', tr('error.notARepo'), repo.reason);
    return;
  }
  const fetchResult = await runGitWithProgress(repo.root, ['fetch', '--all'], tr('sync.title'));
  if (fetchResult.code !== 0) {
    notify('error', tr('sync.title'), failureText(fetchResult, 'error.unknown'));
    return;
  }
  const pullResult = await runGitWithProgress(repo.root, ['pull'], tr('sync.title'));
  if (pullResult.code !== 0) {
    notify('error', tr('sync.title'), failureText(pullResult, 'error.unknown'));
    return;
  }
  const detail = (pullResult.stdout || '').trim() || (pullResult.stderr || '').trim();
  notify('success', tr('sync.success'), detail);
}

async function showBranches() {
  const directory = await resolveDirectory();
  if (!directory) {
    notify('warning', tr('error.noDirectory'));
    return;
  }
  const repo = await resolveRepo(directory);
  if (!repo.root) {
    notify('warning', tr('error.notARepo'), repo.reason);
    return;
  }
  const result = await runGitPlain(repo.root, ['branch', '--all', '--no-color']);
  if (result.code !== 0) {
    notify('error', tr('branches.title'), failureText(result, 'error.unknown'));
    return;
  }
  const lines = String(result.stdout || '')
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(Boolean);
  if (!lines.length) {
    await showInfo(tr('branches.title'), tr('branches.empty'));
    return;
  }
  const rendered = lines.map((line) => {
    if (line.startsWith('*')) {
      return `${tr('branches.current')}: ${line.replace(/^\*\s*/, '')}`;
    }
    return line;
  });
  await showInfo(tr('branches.title'), rendered.join('\n'));
}

async function showLog() {
  const directory = await resolveDirectory();
  if (!directory) {
    notify('warning', tr('error.noDirectory'));
    return;
  }
  const repo = await resolveRepo(directory);
  if (!repo.root) {
    notify('warning', tr('error.notARepo'), repo.reason);
    return;
  }
  const result = await runGitPlain(repo.root, [
    'log',
    '--no-color',
    '-n',
    '50',
    '--date=short',
    '--pretty=format:%h%x09%an%x09%ad%x09%s',
  ]);
  if (result.code !== 0) {
    notify('error', tr('log.title'), failureText(result, 'error.unknown'));
    return;
  }
  const rows = String(result.stdout || '')
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(Boolean);
  if (!rows.length) {
    await showInfo(tr('log.title'), tr('log.empty'));
    return;
  }
  const rendered = rows.map((row) => {
    const parts = row.split('\t');
    return parts.length >= 4 ? `${parts[0]}  ${parts[1]}  ${parts[2]}  ${parts.slice(3).join('\t')}` : row;
  });
  await showInfo(tr('log.title'), rendered.join('\n'));
}

async function showBrowse() {
  const directory = await resolveDirectory();
  if (!directory) {
    notify('warning', tr('error.noDirectory'));
    return;
  }
  const repo = await resolveRepo(directory);
  if (!repo.root) {
    notify('warning', tr('error.notARepo'), repo.reason);
    return;
  }
  const result = await runGitPlain(repo.root, ['ls-tree', '--long', 'HEAD']);
  if (result.code !== 0) {
    notify('warning', tr('browse.title'), tr('browse.noCommit'));
    return;
  }
  const entries = String(result.stdout || '')
    .split(/\r?\n/)
    .map(line => parseLsTreeLine(line))
    .filter(entry => entry && entry.type !== 'commit');
  if (!entries.length) {
    await showInfo(tr('browse.title'), tr('browse.empty'));
    return;
  }
  const rendered = entries.map((entry) => {
    const type = entry.type === 'tree' ? 'dir' : 'file';
    const size = entry.type === 'tree' ? '-' : String(entry.size);
    return `${type}\t${size}\t${entry.path}`;
  });
  await showInfo(tr('browse.title'), rendered.join('\n'));
}

async function ideCommand() {
  const stored = await readStorage(IDE_COMMAND_KEY);
  return stored || DEFAULT_IDE_COMMAND;
}

async function openInIde(directory) {
  const target = directory || (await resolveDirectory());
  if (!target) {
    notify('warning', tr('error.noDirectory'));
    return;
  }
  const command = await ideCommand();
  try {
    const result = await sigma.shell.run(command, [target]);
    if (result && result.code !== 0) {
      notify('error', tr('ide.title'), failureText(result, 'ide.failed'));
      return;
    }
    notify('success', tr('ide.success'), `${command} ${target}`);
  }
  catch {
    notify('error', tr('ide.title'), tr('ide.failed'));
  }
}

async function showOpenInIde(context) {
  const entry = context && context.selectedEntries ? context.selectedEntries[0] : null;
  const directory = directoryOfEntry(entry) || (await resolveDirectory());
  await openInIde(directory);
}

async function configureIde() {
  const current = await ideCommand();
  const answer = await askPrompt(tr('ide.cmdLabel'), tr('ide.cmdHint'), current);
  if (!answer) {
    return;
  }
  await writeStorage(IDE_COMMAND_KEY, answer);
  notify('success', tr('ide.saved'), answer);
}

// ---------------------------------------------------------------- 注册

const COMMANDS = [
  ['status', 'cmd.status', showStatus],
  ['init', 'cmd.init', showInit],
  ['clone', 'cmd.clone', runClone],
  ['fetch', 'cmd.fetch', () => runRemoteOperation(['fetch', '--all'], 'fetch.title', 'fetch.success')],
  ['pull', 'cmd.pull', () => runRemoteOperation(['pull'], 'pull.title', 'pull.success')],
  ['push', 'cmd.push', () => runRemoteOperation(['push'], 'push.title', 'push.success')],
  ['sync', 'cmd.sync', runSync],
  ['branches', 'cmd.branches', showBranches],
  ['log', 'cmd.log', showLog],
  ['browse', 'cmd.browse', showBrowse],
  ['openInIde', 'cmd.openInIde', () => openInIde(null)],
  ['configureIde', 'cmd.configureIde', configureIde],
];

export async function activate() {
  try {
    await sigma.i18n.mergeFromPath('locales');
  }
  catch {
    // 语言包合并失败时用内置文案兜底。
  }

  try {
    const value = await sigma.context.getCurrentPath();
    if (value) {
      lastKnownPath = value;
    }
  }
  catch {
    // 拿不到初始路径时靠 onPathChange 补。
  }

  try {
    track(sigma.context.onPathChange((path) => {
      lastKnownPath = path || null;
    }));
  }
  catch {
    // 监听失败不影响命令本身。
  }

  for (const [id, titleKey, handler] of COMMANDS) {
    try {
      track(sigma.commands.registerCommand({ id, title: tr(titleKey) }, handler));
    }
    catch {
      // 单条命令注册失败不影响其余命令。
    }
  }

  try {
    track(
      sigma.contextMenu.registerItem(
        { id: 'openInIde', title: tr('context.openInIde'), when: { entryType: 'directory' } },
        showOpenInIde,
      ),
    );
  }
  catch {
    // 右键菜单项注册失败时忽略。
  }

  try {
    track(
      sigma.contextMenu.registerItem(
        { id: 'init', title: tr('context.init'), when: { entryType: 'directory' } },
        showInit,
      ),
    );
  }
  catch {
    // 右键菜单项注册失败时忽略。
  }

  try {
    const directory = await currentPath();
    if (directory) {
      const root = await repoRoot(directory);
      if (root) {
        await writeStorage(REPO_DIR_KEY, root);
      }
    }
  }
  catch {
    // 记忆仓库目录失败不影响功能。
  }
}

export function deactivate() {
  while (disposables.length) {
    const disposable = disposables.pop();
    try {
      disposable.dispose();
    }
    catch {
      // 忽略释放失败。
    }
  }
}
