#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
//
// 自建扩展市场的「发布器」：把本地扩展目录登记进 registry.json。
//
// 用法（仓库根目录执行）：
//   1) 新增 / 更新一个扩展条目
//      node scripts/ext-market/publish.mjs \
//        --extension extensions/kizemo.git \
//        --repository https://github.com/kizemo/sfm-extension-kizemo-git \
//        --registry scripts/ext-market/registry.json
//   2) 移除一个条目
//      node scripts/ext-market/publish.mjs --remove kizemo.git --registry scripts/ext-market/registry.json
//   3) 校验现有 registry.json 是否符合宿主校验规则
//      node scripts/ext-market/publish.mjs --validate --registry scripts/ext-market/registry.json
//   4) 计算某个 release ZIP 的 integrity（宿主认的格式是 sha256:<hex>）
//      node scripts/ext-market/publish.mjs --sha256 D:\path\to\release.zip
//
// 校验规则与宿主 src/modules/extensions/runtime/validation.ts 逐字对齐，
// 不对齐的条目会在宿主侧被 assertValidRegistryData 直接丢弃。

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const KNOWN_CATEGORIES = [
  'File Management',
  'Media',
  'Security',
  'Backup & Sync',
  'Productivity',
  'Search & Filter',
  'Appearance',
  'Integration',
  'Developer Tools',
  'Example',
  'Other',
];

const ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;
const VERSION_PATTERN = /^\d+\.\d+\.\d+(?:-[\w.]+)?$/;

function fail(message) {
  console.error(`[ext-market] 失败：${message}`);
  process.exit(1);
}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith('--')) {
      fail(`无法识别的参数：${token}`);
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

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  }
  catch (error) {
    fail(`读取 JSON 失败：${file} → ${error.message}`);
  }
}

function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function isPlainObject(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * 与宿主 isValidRegistryEntry + isValidRegistryPublisher + isValidRegistryVersionMetadata 对齐。
 * 返回错误列表，空数组表示通过。
 */
function validateRegistryEntry(entry) {
  const errors = [];

  if (!isPlainObject(entry)) {
    return ['条目必须是对象'];
  }

  if (!isNonEmptyString(entry.id) || !ID_PATTERN.test(entry.id)) {
    errors.push('id 必须是非空字符串，且只允许字母/数字/点/下划线/连字符');
  }

  if (!isNonEmptyString(entry.name)) {
    errors.push('name 不能为空');
  }

  if (!isNonEmptyString(entry.description)) {
    errors.push('description 不能为空');
  }

  if (!isPlainObject(entry.publisher) || !isNonEmptyString(entry.publisher.name)) {
    errors.push('publisher.name 不能为空');
  }
  else if (!Object.keys(entry.publisher).every(key => key === 'name' || key === 'url')) {
    errors.push('publisher 只允许 name 与 url 两个字段');
  }

  if (!isNonEmptyString(entry.repository)) {
    errors.push('repository 不能为空');
  }

  if (typeof entry.featured !== 'boolean') {
    errors.push('featured 必须是布尔值');
  }

  if (!Array.isArray(entry.categories) || entry.categories.length === 0 || entry.categories.some(category => !isNonEmptyString(category))) {
    errors.push('categories 必须是非空字符串数组');
  }

  if (entry.tags !== undefined && (!Array.isArray(entry.tags) || entry.tags.some(tag => !isNonEmptyString(tag)))) {
    errors.push('tags 必须是字符串数组');
  }

  if (entry.integrity !== undefined && !isNonEmptyString(entry.integrity)) {
    errors.push('integrity 必须是非空字符串');
  }

  if (entry.releaseMetadata !== undefined) {
    if (!isPlainObject(entry.releaseMetadata)) {
      errors.push('releaseMetadata 必须是对象');
    }
    else {
      for (const [version, metadata] of Object.entries(entry.releaseMetadata)) {
        if (!VERSION_PATTERN.test(version)) {
          errors.push(`releaseMetadata 的版本键 "${version}" 不合法（必须是 x.y.z，且不带 v 前缀）`);
        }
        if (!isPlainObject(metadata)) {
          errors.push(`releaseMetadata["${version}"] 必须是对象`);
          continue;
        }
        if (metadata.integrity !== undefined && !isNonEmptyString(metadata.integrity)) {
          errors.push(`releaseMetadata["${version}"].integrity 必须是非空字符串`);
        }
        if (metadata.manifest !== undefined && !isNonEmptyString(metadata.manifest)) {
          errors.push(`releaseMetadata["${version}"].manifest 必须是非空字符串`);
        }
      }
    }
  }

  return errors;
}

function validateRegistryDocument(document) {
  const errors = [];

  if (!isPlainObject(document)) {
    return ['registry 根节点必须是对象'];
  }

  if (!isNonEmptyString(document.schemaVersion) || !VERSION_PATTERN.test(document.schemaVersion)) {
    errors.push('schemaVersion 必须是 x.y.z 形式的版本号（宿主 isVersionString 要求三段）');
  }

  if (!Array.isArray(document.extensions)) {
    errors.push('extensions 必须是数组');
    return errors;
  }

  const seen = new Set();
  document.extensions.forEach((entry, index) => {
    const entryErrors = validateRegistryEntry(entry);
    entryErrors.forEach(error => errors.push(`extensions[${index}]（${entry && entry.id ? entry.id : '未知 id'}）：${error}`));
    if (entry && isNonEmptyString(entry.id)) {
      if (seen.has(entry.id)) {
        errors.push(`extensions[${index}]：id "${entry.id}" 重复`);
      }
      seen.add(entry.id);
    }
  });

  return errors;
}

function sha256File(file) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(file));
  return hash.digest('hex');
}

function resolveExtensionManifest(extensionDir) {
  const manifestPath = path.join(extensionDir, 'package.json');
  if (!fs.existsSync(manifestPath)) {
    fail(`扩展目录里没有 package.json：${manifestPath}`);
  }
  return readJson(manifestPath);
}

function buildEntry(manifest, args) {
  const id = args.id || manifest.id;
  if (!isNonEmptyString(id)) {
    fail('扩展清单里没有 id，且命令行没有 --id');
  }

  const repository = args.repository || manifest.repository;
  if (!isNonEmptyString(repository)) {
    fail('必须用 --repository 指定扩展仓库地址（宿主用它拼 manifest / 版本 / release ZIP 地址）');
  }

  const categories = args.categories
    ? String(args.categories).split(',').map(item => item.trim()).filter(Boolean)
    : (Array.isArray(manifest.categories) && manifest.categories.length > 0 ? manifest.categories : ['Other']);

  const publisher = {
    name: args['publisher-name'] || (manifest.publisher && manifest.publisher.name) || String(id).split('.')[0],
  };
  const publisherUrl = args['publisher-url'] || (manifest.publisher && manifest.publisher.url);
  if (isNonEmptyString(publisherUrl)) {
    publisher.url = publisherUrl;
  }

  const entry = {
    id,
    name: args.name || manifest.name || id,
    description: args.description || manifest.description || '',
    publisher,
    repository: String(repository).replace(/\/+$/, ''),
    featured: args.featured === true || args.featured === 'true',
    categories,
  };

  const tags = args.tags
    ? String(args.tags).split(',').map(item => item.trim()).filter(Boolean)
    : (Array.isArray(manifest.tags) ? manifest.tags : []);
  if (tags.length > 0) {
    entry.tags = tags;
  }

  const version = args.version || manifest.version;
  const integrity = args.integrity || (isNonEmptyString(manifest.integrity) ? manifest.integrity : undefined);
  if (isNonEmptyString(version) && isNonEmptyString(integrity)) {
    entry.releaseMetadata = { [version]: { integrity } };
    const manifestUrl = args['manifest-url'];
    if (isNonEmptyString(manifestUrl)) {
      entry.releaseMetadata[version].manifest = manifestUrl;
    }
  }
  else if (isNonEmptyString(integrity)) {
    entry.integrity = integrity;
  }

  return entry;
}

function loadOrCreateRegistry(registryPath) {
  if (!fs.existsSync(registryPath)) {
    return { schemaVersion: '1.0.0', extensions: [] };
  }
  const document = readJson(registryPath);
  if (!isPlainObject(document)) {
    fail(`registry 根节点不是对象：${registryPath}`);
  }
  if (!Array.isArray(document.extensions)) {
    fail(`registry 缺 extensions 数组：${registryPath}`);
  }
  return document;
}

function writeRegistry(registryPath, document) {
  const sorted = {
    schemaVersion: document.schemaVersion || '1.0.0',
    extensions: [...document.extensions].sort((left, right) => String(left.id).localeCompare(String(right.id))),
  };
  fs.writeFileSync(registryPath, `${JSON.stringify(sorted, null, 2)}\n`, 'utf8');
  return sorted;
}

function main() {
  const args = parseArgs(process.argv.slice(2));

  if (args.sha256) {
    const file = String(args.sha256);
    if (!fs.existsSync(file)) {
      fail(`文件不存在：${file}`);
    }
    console.log(`sha256:${sha256File(file)}`);
    return;
  }

  const registryPath = path.resolve(String(args.registry || 'scripts/ext-market/registry.json'));

  if (args.validate) {
    const document = loadOrCreateRegistry(registryPath);
    const errors = validateRegistryDocument(document);
    if (errors.length > 0) {
      console.error(`[ext-market] 校验失败，共 ${errors.length} 处问题：`);
      errors.forEach(error => console.error(`  - ${error}`));
      process.exit(1);
    }
    console.log(`[ext-market] 校验通过：${registryPath}（${document.extensions.length} 个条目）`);
    return;
  }

  if (args.remove) {
    const document = loadOrCreateRegistry(registryPath);
    const target = String(args.remove);
    const before = document.extensions.length;
    document.extensions = document.extensions.filter(entry => entry.id !== target);
    if (document.extensions.length === before) {
      fail(`registry 里没有 id 为 "${target}" 的条目`);
    }
    writeRegistry(registryPath, document);
    console.log(`[ext-market] 已移除条目 ${target}，剩余 ${document.extensions.length} 条`);
    return;
  }

  if (!args.extension) {
    fail('缺少 --extension <扩展目录>（或改用 --remove / --validate / --sha256）');
  }

  const extensionDir = path.resolve(String(args.extension));
  const manifest = resolveExtensionManifest(extensionDir);
  const entry = buildEntry(manifest, args);

  const entryErrors = validateRegistryEntry(entry);
  if (entryErrors.length > 0) {
    console.error('[ext-market] 生成的条目不合法：');
    entryErrors.forEach(error => console.error(`  - ${error}`));
    process.exit(1);
  }

  const unknownCategories = entry.categories.filter(category => !KNOWN_CATEGORIES.includes(category));
  if (unknownCategories.length > 0) {
    console.warn(`[ext-market] 提醒：以下分类不在官方分类表里（宿主不校验分类，但市场审核可能退回）：${unknownCategories.join(', ')}`);
  }

  const document = loadOrCreateRegistry(registryPath);
  const existingIndex = document.extensions.findIndex(item => item.id === entry.id);
  if (existingIndex >= 0) {
    document.extensions[existingIndex] = entry;
    console.log(`[ext-market] 已更新条目 ${entry.id}`);
  }
  else {
    document.extensions.push(entry);
    console.log(`[ext-market] 已新增条目 ${entry.id}`);
  }

  const sorted = writeRegistry(registryPath, document);

  const documentErrors = validateRegistryDocument(sorted);
  if (documentErrors.length > 0) {
    console.error('[ext-market] 写入后整体校验失败：');
    documentErrors.forEach(error => console.error(`  - ${error}`));
    process.exit(1);
  }

  console.log(`[ext-market] 完成：${registryPath}（${sorted.extensions.length} 个条目）`);
  console.log(JSON.stringify(entry, null, 2));
}

main();
