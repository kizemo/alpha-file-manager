# 自建扩展市场工具包

本目录提供「自建扩展市场」的**市场侧**工具：生成 / 维护 `registry.json`、本地自检、格式校验。

宿主侧（应用内新增「市场源」入口、合并多个源、设置页管理）的接线方案见
`docs/design-2026-11-04-extension-market-sources.md`。

## 0. 先读这一段：宿主的硬约束

| 约束 | 结论 |
| --- | --- |
| 市场清单 | `registry.json`，结构 `{ schemaVersion, extensions[] }`，`schemaVersion` 必须是 `x.y.z` |
| 条目必填 | `id` / `name` / `description` / `publisher.name` / `repository` / `featured` / `categories` |
| `id` | 必须与扩展 `package.json` 的 `id` 完全一致 |
| `repository` | 宿主用它拼 manifest、版本列表与 release ZIP 地址，**当前实现要求 GitHub 仓库**（`owner/repo` 或完整 URL 都行） |
| `releaseMetadata` | 可选；键是不带 `v` 的版本号（`1.0.0` ↔ tag `v1.0.0`），`integrity` 形如 `sha256:<hex>` |
| 网络 | 宿主客户端用 `PublicOnlyResolver`，**回环 / 内网 / 链路本地地址在 connect 阶段就被拒** ⇒ 市场必须放在公网 https 地址 |
| 兜底 | `raw.githubusercontent.com` 有内置 jsDelivr 兜底 ⇒ 放 GitHub raw 在国内网络下比自建服务器更稳 |

一句话：**自建市场 = 一份你控制的 `registry.json`，放在公网 https 上；扩展本体仍然用 GitHub Releases 分发。**

## 1. 快速开始

### 1.1 生成 / 更新一个条目

```bash
node scripts/ext-market/publish.mjs \
  --extension extensions/kizemo.git \
  --repository https://github.com/kizemo/sfm-extension-kizemo-git \
  --registry scripts/ext-market/registry.json
```

可选参数：

| 参数 | 作用 |
| --- | --- |
| `--name` / `--description` | 覆盖清单里的显示名 / 描述 |
| `--categories A,B` | 覆盖分类（官方分类表见下） |
| `--tags a,b` | 覆盖标签 |
| `--featured` | 标记精选 |
| `--publisher-name` / `--publisher-url` | 覆盖发布者 |
| `--integrity sha256:<hex>` | 写入 `releaseMetadata[version].integrity` |
| `--manifest-url <url>` | 写入 `releaseMetadata[version].manifest` |
| `--version 1.0.0` | 覆盖要登记的版本号 |
| `--id <id>` | 覆盖条目 id（默认取扩展清单的 id） |

官方分类表：`File Management` / `Media` / `Security` / `Backup & Sync` / `Productivity` /
`Search & Filter` / `Appearance` / `Integration` / `Developer Tools` / `Example` / `Other`。

### 1.2 移除条目

```bash
node scripts/ext-market/publish.mjs --remove kizemo.git --registry scripts/ext-market/registry.json
```

### 1.3 校验（发布前必跑）

```bash
node scripts/ext-market/publish.mjs --validate --registry scripts/ext-market/registry.json
```

校验规则与宿主 `src/modules/extensions/runtime/validation.ts` 的
`assertValidRegistryData` / `isValidRegistryEntry` 逐字对齐 ⇒ 本地能过的条目，宿主才收。

### 1.4 计算 release ZIP 的 integrity

GitHub release 的自动源码包（`Source code (zip)`）就是宿主下载的包：

```bash
node scripts/ext-market/publish.mjs --sha256 D:\downloads\sfm-extension-kizemo-git-0.1.0.zip
# => sha256:9f2c...
```

把这个值填进 `--integrity`，宿主安装时就会校验下载包的完整性；不填则跳过校验。

### 1.5 本地自检（浏览器 / curl）

```bash
node scripts/ext-market/serve.mjs --dir scripts/ext-market --port 8788
# 浏览器打开 http://127.0.0.1:8788/registry.json
```

⚠️ 这个本地服务**只给浏览器和 curl 用**：宿主应用拉不到 `127.0.0.1`（见上文硬约束）。

## 2. 部署到公网（三种）

| 方式 | 做法 | 市场地址 | 备注 |
| --- | --- | --- | --- |
| GitHub raw（推荐） | 把 `registry.json` 放进你自己的仓库 | `https://raw.githubusercontent.com/<owner>/<repo>/main/registry.json` | 宿主有 jsDelivr 兜底，国内可用性最好 |
| GitHub Pages | 把 `registry.json` 放 `docs/` 或站点目录 | `https://<owner>.github.io/<repo>/registry.json` | 纯静态，无需服务器 |
| 自己的服务器 | Nginx / Caddy 放 `/registry.json` | `https://你的域名/registry.json` | 需要公网 IP + TLS 证书；**不能是内网地址** |

发布流程建议：

1. 扩展仓库打 tag（`v0.1.0`）并推 release。
2. 用 `--sha256` 算出 release ZIP 的完整性值。
3. 用 `--extension` 生成 / 更新条目。
4. `--validate` 过一遍。
5. 把 `registry.json` 推到公网。
6. 宿主「刷新市场」即可看到（宿主接线完成后）。

## 3. 与宿主接线的边界

本工具包只负责**市场侧**。宿主侧还需要（见设计文档 §四）：

- 多市场源拉取与合并（官方源 + 自建源）；
- 设置页的「市场源」管理界面；
- 市场页的来源切换；
- `removeUnapprovedInstalledExtensions` 的判定要改成「所有源都没有才卸载」。

## 4. 文件清单

| 文件 | 作用 |
| --- | --- |
| `publish.mjs` | 条目生成 / 更新 / 移除 / 校验 / sha256 计算 |
| `serve.mjs` | 本地静态服务器（浏览器自检） |
| `registry.schema.json` | `registry.json` 的 JSON Schema，可用于外部 CI 校验 |
| `registry.json` | 你的市场清单本体（由 `publish.mjs` 生成） |
