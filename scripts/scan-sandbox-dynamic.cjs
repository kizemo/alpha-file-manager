#!/usr/bin/env node
/**
 * scan-sandbox-dynamic.cjs — 从 Sigma FM 源码【动态提取】sandbox 正则并扫描 extension
 *
 * 为什么不硬编码 19 条:
 *   scan-sandbox.cjs 里的 19 条是 2026-10-07 的快照。
 *   实测 sandbox.ts 在 git 历史中变更过:
 *     891fb146  Move to ESM - Use sandbox to fully isolate extensions
 *     ba3e5a3f  Add .constructor() with string args to static code validator patterns
 *   ⇒ Sigma FM 升级可能新增/修改/删除 pattern,硬编码快照会静默失效。
 *
 * 本脚本直接 parse src/modules/extensions/runtime/sandbox.ts,
 * 提取所有 `pattern: /regex/flags` 字面量,再逐条扫描目标文件。
 *
 * 2026-10-09: 本仓就是 Sigma FM 的 fork,不再需要跨仓拼路径。
 *
 * 用法:
 *   node scripts/scan-sandbox-dynamic.cjs <extension-js-file> [sandbox-ts-file]
 *
 * 退出码: 0 = 干净, 1 = 有违规, 2 = 无法提取 pattern(必须人工检查)
 */

const fs = require('fs');
const path = require('path');

// ── 参数 ────────────────────────────────────────────────────────────────
const targetFile = process.argv[2];
const sandboxFile = process.argv[3]
  || path.join(__dirname, '..', 'src', 'modules', 'extensions', 'runtime', 'sandbox.ts');

if (!targetFile || !fs.existsSync(targetFile)) {
  console.error(`[FATAL] target not found: ${targetFile}`);
  process.exit(2);
}
if (!fs.existsSync(sandboxFile)) {
  console.error(`[FATAL] sandbox.ts not found: ${sandboxFile}`);
  console.error(`        请显式传入路径: node scan-sandbox-dynamic.cjs <target> <sandbox.ts>`);
  process.exit(2);
}

// ── 从 sandbox.ts 提取 pattern 字面量 ───────────────────────────────────
const sandboxSrc = fs.readFileSync(sandboxFile, 'utf8');

// 定位 dangerousPatterns 数组(如果存在),否则扫描整个文件的 pattern: /.../flags
const arrayMatch = sandboxSrc.match(/const\s+dangerousPatterns\s*=\s*\[/);
const scope = arrayMatch
  ? sandboxSrc.slice(arrayMatch.index)
  : sandboxSrc;

// 匹配 { pattern: /.../flags, message: '...' } 结构
const entryRe = /pattern:\s*(\/(?:\\.|[^/\n\\])+\/[gimsuy]*)\s*,\s*message:\s*(['"`])(?:\\.|(?!\2)[^\\])*\2/g;

const patterns = [];
let m;
while ((m = entryRe.exec(scope)) !== null) {
  const literal = m[1];
  try {
    patterns.push(new RegExp(literal.slice(1, literal.lastIndexOf('/')), literal.slice(literal.lastIndexOf('/') + 1)));
  } catch (e) {
    console.error(`[WARN] 无法编译 pattern ${literal}: ${e.message}`);
  }
}

if (patterns.length === 0) {
  console.error('[FATAL] 未能从 sandbox.ts 提取任何 pattern。');
  console.error('        sandbox.ts 结构可能已变(例如改用了常量数组)。');
  console.error('        【必须人工检查】不要假设"没有违规"。');
  process.exit(2);
}

// ── 扫描 ────────────────────────────────────────────────────────────────
const code = fs.readFileSync(targetFile, 'utf8');
const lines = code.split(/\r?\n/);
const lineOf = idx => code.slice(0, idx).split(/\r?\n/).length;
const isComment = l => {
  const t = l.trim();
  return t.startsWith('*') || t.startsWith('//') || t.startsWith('/*');
};

console.log('='.repeat(70));
console.log('DYNAMIC SANDBOX SCAN');
console.log('='.repeat(70));
console.log('sandbox source :', sandboxFile);
console.log('patterns found :', patterns.length, '(从源码动态提取,非硬编码)');
console.log('target         :', targetFile);
console.log('');

let total = 0;
for (const re of patterns) {
  re.lastIndex = 0;
  const hits = [];
  let mm;
  while ((mm = re.exec(code)) !== null) {
    hits.push({ idx: mm.index, text: mm[0], line: lineOf(mm.index) });
    if (mm.index === re.lastIndex) re.lastIndex++;
  }
  if (hits.length) {
    total += hits.length;
    console.log(`[VIOLATION] /${re.source}/${re.flags}  (${hits.length})`);
    for (const h of hits.slice(0, 10)) {
      const src = (lines[h.line - 1] || '').trim();
      console.log(`    line ${h.line}: ${JSON.stringify(h.text)}  ${isComment(src) ? '[in comment]' : '[IN CODE]'}  ${src.slice(0, 90)}`);
    }
    if (hits.length > 10) console.log(`    ... +${hits.length - 10} more`);
    console.log('');
  }
}

console.log('='.repeat(70));
console.log(total === 0
  ? `RESULT: VALID (0 violations / ${patterns.length} patterns)`
  : `RESULT: INVALID (${total} violations)`);
process.exit(total === 0 ? 0 : 1);
