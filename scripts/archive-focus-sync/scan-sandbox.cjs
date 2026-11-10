// 精确复刻 sandbox.ts:207-284 的 19 条正则，逐条报告命中位置
const fs = require('fs');
const path = process.argv[2];
const code = fs.readFileSync(path, 'utf8');

const patterns = [
  [/\beval\s*\(/g, 'eval() is not allowed'],
  [/\bnew\s+Function\s*\(/g, 'new Function() is not allowed'],
  [/\.constructor\s*\(\s*['"`]/g, 'Dynamic code construction via .constructor() is not allowed'],
  [/\bdocument\s*\./g, 'Direct document access is not allowed'],
  [/\bwindow\s*\./g, 'Direct window access is not allowed'],
  [/\blocalStorage\s*\./g, 'localStorage is not allowed'],
  [/\bsessionStorage\s*\./g, 'sessionStorage is not allowed'],
  [/\bindexedDB\s*\./g, 'indexedDB is not allowed'],
  [/\bfetch\s*\(/g, 'fetch() is not allowed - use sigma.fs instead'],
  [/\bXMLHttpRequest\b/g, 'XMLHttpRequest is not allowed'],
  [/\bWebSocket\b/g, 'WebSocket is not allowed'],
  [/(?<![\w$.])self\b/g, 'Direct self access is not allowed'],
  [/(?<![\w$.])globalThis\b/g, 'Direct globalThis access is not allowed'],
  [/(?<![\w$.])postMessage\s*\(/g, 'Direct postMessage() is not allowed'],
  [/(?<![\w$.])addEventListener\s*\(/g, 'Direct addEventListener() is not allowed'],
  [/(?<![\w$.])removeEventListener\s*\(/g, 'Direct removeEventListener() is not allowed'],
  [/(?<![\w$.])dispatchEvent\s*\(/g, 'Direct dispatchEvent() is not allowed'],
  [/\bimportScripts\b/g, 'importScripts is not allowed'],
  [/(?<![\w$.])navigator\b/g, 'Direct navigator access is not allowed'],
];

const lines = code.split(/\r?\n/);
function lineOf(idx) { return code.slice(0, idx).split(/\r?\n/).length; }
function isComment(l) {
  const t = l.trim();
  return t.startsWith('*') || t.startsWith('//') || t.startsWith('/*') || t.startsWith('*');
}

let total = 0;
console.log('FILE:', path);
console.log('SIZE:', code.length, 'bytes /', lines.length, 'lines');
console.log('');
for (const [re, msg] of patterns) {
  re.lastIndex = 0;
  const hits = [];
  let m;
  while ((m = re.exec(code)) !== null) {
    hits.push({ idx: m.index, text: m[0], line: lineOf(m.index) });
    if (m.index === re.lastIndex) re.lastIndex++;
  }
  if (hits.length) {
    total += hits.length;
    console.log(`[VIOLATION] ${msg}  (${hits.length} hit${hits.length > 1 ? 's' : ''})`);
    for (const h of hits.slice(0, 12)) {
      const src = lines[h.line - 1] || '';
      console.log(`    line ${h.line}: ${JSON.stringify(h.text)}  ${isComment(src) ? '[in comment]' : '[IN CODE]'}  ${src.trim().slice(0, 100)}`);
    }
    if (hits.length > 12) console.log(`    ... +${hits.length - 12} more`);
    console.log('');
  }
}
console.log('========================================');
console.log(total === 0 ? 'RESULT: VALID (0 violations)' : `RESULT: INVALID (${total} violations)`);
process.exit(total === 0 ? 0 : 1);
