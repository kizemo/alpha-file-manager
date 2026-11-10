// verify-extension-sandbox-gate.mjs —— 打包前的扩展沙箱门禁
//
// 为什么要有这道门禁（2026-11-10 立项）：
//
//   1) 应用在把扩展交给 Worker 之前，会用 sandbox.ts 里的规则做一次静态筛查
//      （validateExtensionCode）。命中任一条规则就直接拒绝：不弹窗、不通知、
//      侧车一次请求都收不到 —— 用户看到的是「扩展装了，却毫无反应」。
//   2) 安装器里本来有一道同样的检查（installer/register.ps1 调
//      scripts/scan-sandbox-dynamic.cjs），但打包安装场景下 scanner 与
//      sandbox.ts 都不在安装目录里，那道检查会被静默跳过。2026-11-10 换仓库根
//      时连开发安装场景的路径也一起失效了，门禁彻底变成装饰。
//
//   ⇒ 所以门禁要放在「代码真正所在的地方」：打包之前，把随包发布的扩展入口
//     逐个拿 sandbox.ts 的规则扫一遍。应用会拒绝的扩展，到不了用户手里。
//
// 为什么不把 scanner / sandbox 一起打进安装包：
//   scanner 是 node 脚本，绝大多数用户的机器上没有 node；打进安装包后门禁仍然
//   跑不起来，只是把「跳过」换了个地方跳过。构建期校验没有这个依赖问题。
//
// 用法：node scripts/verify-extension-sandbox-gate.mjs
// 退出码：0 = 全部通过；1 = 有违规，或门禁自身无法运行（不允许静默放过）
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SCANNER = path.join(ROOT, 'scripts', 'scan-sandbox-dynamic.cjs');
const SANDBOX = path.join(ROOT, 'src', 'modules', 'extensions', 'runtime', 'sandbox.ts');
const TAURI_CONF = path.join(ROOT, 'src-tauri', 'tauri.conf.json');

function fail(message) {
  console.error(`[sandbox-gate] FAIL  ${message}`);
  process.exit(1);
}

// 门禁自身的前置条件缺一不可。缺了就要红 —— 这里放过，等于把门禁拆了。
for (const required of [SCANNER, SANDBOX, TAURI_CONF]) {
  if (!fs.existsSync(required)) {
    fail(`门禁自身的前置文件缺失: ${path.relative(ROOT, required)}`);
  }
}

const tauriConf = JSON.parse(fs.readFileSync(TAURI_CONF, 'utf8'));
const resources = tauriConf.bundle?.resources ?? {};

// 候选 = 随包发布的扩展入口。
// 入口从「被打包资源的目录」与「被打包资源的 package.json 的 main」两处收集，
// 去重后逐个扫描。新增扩展只要进 bundle.resources，就自动被门禁覆盖。
const candidates = new Map();

for (const [source, destination] of Object.entries(resources)) {
  const absolute = path.resolve(ROOT, 'src-tauri', source);

  let stats;
  try {
    stats = fs.statSync(absolute);
  }
  catch {
    continue;
  }

  if (stats.isDirectory()) {
    const indexJs = path.join(absolute, 'index.js');
    if (fs.existsSync(indexJs)) {
      candidates.set(indexJs, { label: destination, source });
    }
    continue;
  }

  if (stats.isFile() && path.basename(absolute) === 'package.json') {
    let manifest;
    try {
      manifest = JSON.parse(fs.readFileSync(absolute, 'utf8'));
    }
    catch {
      continue;
    }

    if (typeof manifest.main !== 'string' || manifest.main.length === 0) {
      continue;
    }

    const entry = path.resolve(path.dirname(absolute), manifest.main);
    if (!fs.existsSync(entry) || !entry.endsWith('.js')) {
      continue;
    }

    candidates.set(entry, {
      label: manifest.name ?? manifest.id ?? destination,
      source,
    });
  }
}

if (candidates.size === 0) {
  fail('没有从 bundle.resources 里解析出任何扩展入口 —— 门禁无从下手，请检查 tauri.conf.json');
}

console.log(`[sandbox-gate] 扩展入口 ${candidates.size} 个，规则来源 ${path.relative(ROOT, SANDBOX)}`);

let failed = 0;

for (const [entry, meta] of candidates) {
  const result = spawnSync(process.execPath, [SCANNER, entry, SANDBOX], {
    encoding: 'utf8',
  });

  const output = `${result.stdout ?? ''}${result.stderr ?? ''}`.trimEnd();
  if (output) {
    console.log(output);
  }

  if (result.status === 0) {
    console.log(`[sandbox-gate] PASS  ${meta.label}  (${path.relative(ROOT, entry)})`);
    continue;
  }

  failed += 1;
  if (result.status === 1) {
    console.error(`[sandbox-gate] VIOLATION  ${meta.label}: 扩展代码违反沙箱规则，应用会在加载前拒绝它`);
  }
  else {
    console.error(`[sandbox-gate] ERROR  ${meta.label}: 扫描器退出码 ${result.status}，规则无法提取 —— 按失败处理`);
  }
}

if (failed > 0) {
  fail(`${failed} 个扩展入口未通过沙箱门禁`);
}

console.log('[sandbox-gate] OK  全部扩展入口通过');
