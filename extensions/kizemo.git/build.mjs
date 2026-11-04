// SPDX-License-Identifier: MIT
// 构建脚本：把 locales/zh-CN.json 生成成回退文案表，再用 esbuild 打成单文件 ESM。
// 用法：node build.mjs
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

const here = dirname(fileURLToPath(import.meta.url));

const zh = JSON.parse(readFileSync(resolve(here, 'locales/zh-CN.json'), 'utf8'));

const generated = [
  '// 由 build.mjs 自动生成，请勿手工修改。',
  '// 作用：当宿主的语言包合并失败时，扩展仍然显示简体中文而不是裸的 key。',
  `export const FALLBACKS = ${JSON.stringify(zh, null, 2)};`,
  '',
].join('\n');

mkdirSync(resolve(here, 'src'), { recursive: true });
writeFileSync(resolve(here, 'src/generated-fallbacks.js'), generated, 'utf8');

await build({
  entryPoints: [resolve(here, 'src/index.js')],
  outfile: resolve(here, 'dist/index.js'),
  bundle: true,
  format: 'esm',
  platform: 'neutral',
  target: 'es2022',
  sourcemap: true,
  legalComments: 'none',
  logLevel: 'info',
});

console.log('built dist/index.js');
