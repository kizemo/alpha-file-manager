#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
//
// 本地静态服务器：在浏览器 / curl 里自检 registry.json。
//
// 用法：
//   node scripts/ext-market/serve.mjs --dir scripts/ext-market --port 8788
//   然后浏览器打开 http://127.0.0.1:8788/registry.json
//
// ⚠️ 重要限制：宿主应用（Alpha File Manager）的网络客户端使用 PublicOnlyResolver，
// 在 connect 阶段就拒绝回环 / 内网 / 链路本地地址（见 src-tauri/src/extensions/http.rs
// 与 security.rs 的 is_private_ip_address）。所以这个本地服务只能给浏览器和 curl 看，
// **宿主应用永远拉不到 127.0.0.1**。应用要读取自建市场，必须把 registry.json 放到
// 公网 https 地址（GitHub raw / GitHub Pages / 自己的服务器 + 证书）。
//
// 对 raw.githubusercontent.com 宿主还内置了 jsDelivr 兜底（build_jsdelivr_fallback_url），
// 所以「GitHub raw 放 registry.json」在国内网络下比自建服务器更稳。

import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';

const MIME_TYPES = {
  '.json': 'application/json; charset=utf-8',
  '.md': 'text/markdown; charset=utf-8',
  '.zip': 'application/zip',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.svg': 'image/svg+xml',
};

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith('--')) {
      continue;
    }
    const key = token.slice(2);
    const next = argv[index + 1];
    if (next === undefined || next.startsWith('--')) {
      args[key] = true;
    }
    else {
      args[key] = next;
      index += 1;
    }
  }
  return args;
}

const args = parseArgs(process.argv.slice(2));
const rootDir = path.resolve(String(args.dir || 'scripts/ext-market'));
const port = Number(args.port || 8788);
const host = String(args.host || '127.0.0.1');

if (!fs.existsSync(rootDir)) {
  console.error(`[ext-market] 目录不存在：${rootDir}`);
  process.exit(1);
}

const server = http.createServer((request, response) => {
  const requestPath = decodeURIComponent((request.url || '/').split('?')[0]);
  const relative = requestPath === '/' ? 'index.html' : requestPath.replace(/^\/+/, '');
  const target = path.resolve(rootDir, relative);

  if (!target.startsWith(rootDir)) {
    response.writeHead(403, { 'content-type': 'text/plain; charset=utf-8' });
    response.end('403 路径越界');
    return;
  }

  if (!fs.existsSync(target) || !fs.statSync(target).isFile()) {
    response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
    response.end('404 未找到');
    return;
  }

  const contentType = MIME_TYPES[path.extname(target).toLowerCase()] || 'application/octet-stream';
  response.writeHead(200, {
    'content-type': contentType,
    'cache-control': 'no-store',
    'access-control-allow-origin': '*',
  });
  fs.createReadStream(target).pipe(response);
});

server.listen(port, host, () => {
  console.log(`[ext-market] 本地静态服务已启动：http://${host}:${port}`);
  console.log(`[ext-market] 根目录：${rootDir}`);
  console.log('[ext-market] 提醒：宿主应用拉不到回环地址，这里只用于浏览器/curl 自检。');
});
