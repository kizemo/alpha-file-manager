// 全面检查 extension 加载链路上所有可能的 gate
const fs = require('fs');
const path = require('path');

const base = process.env.APPDATA + '\\com.sigma-file-manager.app\\extensions\\kizemo.focus-sync';
const reg = process.env.APPDATA + '\\com.sigma-file-manager.app\\user-data\\user-extensions.json';
const manifest = JSON.parse(fs.readFileSync(path.join(base, 'package.json'), 'utf8'));
const regJson = JSON.parse(fs.readFileSync(reg, 'utf8'));
const regEntry = regJson.installedExtensions['kizemo.focus-sync'];

const checks = [];
const add = (name, pass, detail) => checks.push({ name, pass, detail });

// Gate 1: manifest 必填字段
add('manifest.id', manifest.id === 'kizemo.focus-sync', manifest.id);
add('manifest.name', !!manifest.name, manifest.name);
add('manifest.version', !!manifest.version, manifest.version);
add('manifest.extensionType=api', manifest.extensionType === 'api', manifest.extensionType);
add('manifest.main', !!manifest.main, manifest.main);
add('manifest.activationEvents includes onStartup',
  Array.isArray(manifest.activationEvents) && manifest.activationEvents.includes('onStartup'),
  JSON.stringify(manifest.activationEvents));

// Gate 2: main 文件存在
const mainPath = path.join(base, manifest.main);
add('main file exists on disk', fs.existsSync(mainPath), mainPath);
if (fs.existsSync(mainPath)) {
  const st = fs.statSync(mainPath);
  add('main file non-empty', st.size > 0, st.size + ' bytes');
}

// Gate 3: engines 用 >= 而不是 ^
const eng = manifest.engines || {};
add('engines.sigmaFileManager no caret', !String(eng.sigmaFileManager || '').includes('^'), eng.sigmaFileManager);
add('engines.extensionApi no caret', !String(eng.extensionApi || '').includes('^'), eng.extensionApi);

// Gate 4: 注册状态
add('registered enabled=true', regEntry.enabled === true, String(regEntry.enabled));
add('installPendingDependencies=false', regEntry.installPendingDependencies === false, String(regEntry.installPendingDependencies));
add('isLocal=true', regEntry.isLocal === true, String(regEntry.isLocal));
add('localSourcePath matches', regEntry.localSourcePath === base, regEntry.localSourcePath);

// Gate 5: permissions 格式(http 必须是 object 不能是裸字符串)
const perms = manifest.permissions || [];
const httpObj = perms.find(p => typeof p === 'object' && p && p.name === 'http');
add('http permission is object form', !!httpObj, JSON.stringify(httpObj));
add('http host allowlist has 127.0.0.1:37421',
  !!(httpObj && Array.isArray(httpObj.hosts) && httpObj.hosts.includes('http://127.0.0.1:37421')),
  httpObj ? JSON.stringify(httpObj.hosts) : 'N/A');
add('no bare "http" string permission', !perms.includes('http'), JSON.stringify(perms));

// Gate 6: exports 形态(extensionType=api 需要 ESM export activate)
const code = fs.readFileSync(mainPath, 'utf8');
add('has export activate (or export default)', /export\s+(async\s+)?function\s+activate|export\s+default/.test(code),
  (code.match(/export\s+[^\n]{0,40}/g) || []).slice(0, 3).join(' | '));

// Gate 7: has deactivation? (some hosts require it)
add('has deactivate export', /export\s+(async\s+)?function\s+deactivate/.test(code), 'optional');

// Gate 8: sidecar 端口是否与 extension 内常量一致
const portMatch = code.match(/127\.0\.0\.1:(\d+)/);
add('extension SIDECAR port', !!portMatch, portMatch ? portMatch[0] : 'NOT FOUND');
if (portMatch) add('port matches sidecar 37421', portMatch[1] === '37421', portMatch[1]);

// Gate 9: 依赖文件
add('locales/en.json exists', fs.existsSync(path.join(base, 'locales', 'en.json')), '');
add('locales/zh-CN.json exists', fs.existsSync(path.join(base, 'locales', 'zh-CN.json')), '');
add('index.js.map exists (optional)', fs.existsSync(path.join(base, 'dist', 'index.js.map')), '');

console.log('='.repeat(70));
console.log('EXTENSION LOAD CHAIN GATE CHECK');
console.log('='.repeat(70));
let fails = 0;
for (const c of checks) {
  if (!c.pass) fails++;
  console.log(`${c.pass ? 'PASS' : 'FAIL'}  ${c.name.padEnd(42)} ${c.detail ?? ''}`);
}
console.log('='.repeat(70));
console.log(fails === 0 ? 'ALL GATES PASS' : `${fails} GATE(S) FAILING`);
