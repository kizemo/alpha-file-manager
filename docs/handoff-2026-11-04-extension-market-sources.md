# 交接：自建扩展市场（阶段 1 = 宿主侧「市场源」接线）

> **状态（2026-11-04 收尾）：阶段 1 已完成，并通过真机验收（B29 40/40 全绿）。**
> 详见文末「九、阶段 1 完成」；上文「三、当前进度」「五、验证状态分级」描述的是开工前状态，以文末为准。


- 日期：2026-11-04
- 上一份交接：`docs/handoff-2026-11-03-dir-load-hang-fix.md`（批次 4 增量索引 / 批次 7 Git 扩展，已全部闭环）
- 本轮设计文档：`docs/design-2026-11-04-extension-market-sources.md`（**先读这份**）
- 工具包说明：`scripts/ext-market/README.md`

---

## 一、背景

用户诉求：「扩展，希望搭建一个自己的市场或在 sigma 市场基础上添加自己市场的入口。」

宿主当前只有**一个**硬编码的市场地址（`src/data/external-links.ts:33` → `sigma-hub/sfm-marketplace/main/registry.json`），用户想在自己的扩展生态里也开一个市场，并让应用里能看见、能安装。

## 二、目标

1. **阶段 0（已完成）**：市场侧工具 —— 生成 / 校验 / 发布自己的 `registry.json`。
2. **阶段 1（本轮未做，下一会话做）**：宿主侧「多市场源」—— 官方源保留，自建源可增删，扩展在应用内直接安装、更新、卸载。
3. 阶段 2（可选，本轮明确不做）：市场页加「我的市场」外链入口；扩展二进制自托管。

## 三、当前进度

| 项 | 状态 |
| --- | --- |
| 现状勘察（源码行号级） | ✅ 完成，见设计文档 §三 |
| 方案对比与推荐（多市场源 / Rust 零改动） | ✅ 完成，见设计文档 §四、§五 |
| 阶段 0 工具包 | ✅ 已交付并自检通过 |
| 阶段 1 宿主接线 | ❌ **未开工** |

**阶段 0 交付物**（全部在 `scripts/ext-market/`）：`publish.mjs`（条目生成/更新/移除/校验/sha256）、`serve.mjs`（浏览器自检用本地静态服务）、`registry.schema.json`、`README.md`、`registry.json`（已含 `kizemo.git` 条目）。

## 四、下阶段计划（阶段 1，按此顺序做）

1. **读设计文档 §五**，按它给的类型 / 存储 / store / UI / i18n 清单实现。
2. **先修卸载判定**：`src/stores/runtime/extensions.ts:1894` `removeUnapprovedInstalledExtensions()` 现在只看单个 registry，多源下会把自建扩展卸掉。必须改成「所有源合并后的 id 全集」才判定。
3. **存储两侧一起改**：`src/stores/storage/extensions.ts` 的 `loadStorageData` / `applyExtensionsDataRecord`（读）与 `saveStorageData`（写）都要加 `marketSources` 与 `registryCaches`。本仓库已踩过「只改一边 ⇒ 重启即丢」。
4. **store 改造**：`fetchRegistry()`（`src/stores/runtime/extensions.ts:690`）改为并发拉多源、单源失败降级用缓存、合并时官方源优先。
5. **UI**：市场页加来源切换条；`src/modules/settings/ui/categories/extensions/` 加「市场源」管理卡片。
6. **i18n**：17 个语言包各加设计文档 §5.5 列的 12 个键（中文写 `ch.json`）。
7. **单测**：合并优先级、id 冲突、单源失败降级、跨源卸载判定、存储字段往返。
8. **门禁**：`vue-tsc`、`eslint .`（基线 186）、`vitest`（基线 14 红）、`cargo test --lib`（虽不改 Rust，也要证明没回归）。
9. **打包 + 真机验收**：必须 `tauri build` 产物（`tauri dev` 界面结论作废）；验收需要把 `registry.json` 放到**公网 https**（宿主拒回环/内网）；最终产物不得含调试端口。

## 五、验证状态分级

> ⚠️ 本节为**开工前**状态；阶段 1 的实际验证结论见文末「九、阶段 1 完成」。

### 已验证

- 工具包可用：`publish` / `validate` / `--sha256` / 两个脚本 `node --check` 全部退出 0（日志 `.superpowers/sdd/b33-ext-market.log`）。
- 校验规则与宿主一致：对照 `src/modules/extensions/runtime/validation.ts:394-468` 逐条实现。
- 应用侧拉取只允许**公网**地址：`src-tauri/src/extensions/github.rs:225-232` → `validate_remote_url` + `build_remote_http_client`（`PublicOnlyResolver`）。
- `raw.githubusercontent.com` 有 jsDelivr 兜底：`src-tauri/src/extensions/http.rs:613-618`。
- 官方条目格式：来自 `docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md:527-557` + 宿主校验代码。

### 推断

- 「自建市场只要 `registry.json` 放公网 + 条目 `repository` 指向 GitHub 仓库，就能完整复用现有安装链路，Rust 零改动」——依据是安装链路全部经 `getExtensionManifestUrl` / GitHub tags / codeload ZIP（§3.3），**未做真机跑通验证**。
- 探测策略（§十.1）对首屏耗时的影响量级未实测。

### 未验证

- 宿主「多市场源」实现本身（本轮未写一行宿主代码）。
- 自建源扩展在真机上「安装 → 更新 → 卸载」全链路。
- 自建源不可达时的降级与「不误卸载」行为。

## 六、证据路径

| 路径 | 内容 |
| --- | --- |
| `docs/design-2026-11-04-extension-market-sources.md` | 本轮设计与实施计划（**下一会话的第一份必读**） |
| `scripts/ext-market/README.md` | 工具包用法 + 部署方式 + 硬约束 |
| `scripts/ext-market/registry.json` | 示例清单（`kizemo.git` 条目） |
| `.superpowers/sdd/b33-ext-market.log` | 工具包自检日志（5 个退出码全 0） |
| `docs/handoff-2026-11-03-dir-load-hang-fix.md` | 上一轮交接（批次 4 / 批次 7 已闭环，勿重做） |

## 七、本轮踩坑与工具约束

- **宿主网络策略是硬约束**：`PublicOnlyResolver` 在 connect 阶段拒绝回环 / 内网 / 链路本地 ⇒ 自建市场不能放 `http://127.0.0.1:port`；`serve.mjs` 只是浏览器自检工具。验收必须用公网 https（GitHub raw 最稳）。
- **`schemaVersion` 必须三段**（`isVersionString` = `/^\d+\.\d+\.\d+/`），写 `"1.0"` 会被宿主直接丢弃。
- **`releaseMetadata` 的版本键不带 `v`**（`1.0.0` ↔ tag `v1.0.0`）。
- **`repository` 必须是 GitHub 仓库**（完整 URL 或 `owner/repo` 都行），它决定 manifest / 版本 / ZIP 地址。
- 工具坑（沿用 `AGENTS.md`）：`.cmd` 必须纯 ASCII、`echo` 里不能出现 `>`、运行期间不能编辑；bash 里写文件/重定向/`Copy-Item` 会被拦；长编译要 `> logfile 2>&1`。
- 本轮新增坑：**仓库里新增 `scripts/**` 工具目录时，`eslint .` 会把它算进整仓基线** —— 本轮新增的 `.cjs` 工具已确认 0 违规，但仍要在量基线时用 `git stash push -u` 现量，别拿旧数字对。

## 八、未决问题（需要拍板，见设计文档 §十）

1. 自建源条目是否仍逐条探 manifest（省 N 次请求 vs 失去「失效即隐藏」）。
2. 自建源扩展安装时是否仍强制 GitHub release + sha256 校验（建议保持强制）。
3. 是否需要 `file://` / 本地目录作为市场源（当前一律拒绝）。
4. 自建市场公网落点：GitHub raw（推荐）/ GitHub Pages / 自建服务器。

---

## 九、阶段 1 完成（2026-11-04 收尾）

> 本节由阶段 1 实施会话回写；上文「三、当前进度」「五、验证状态分级」描述的是**开工前**状态，以本节为准。

### 9.1 交付范围

宿主侧「市场源」接线全部落地（**Rust 侧零改动**）：

| 层 | 落点 |
| --- | --- |
| 类型 | `src/types/extension.ts`：`ExtensionMarketSource` / `ExtensionMarketSourceKind` / `ExtensionRegistryEntryWithSource` / `ExtensionRegistryCache` / `MergedExtensionRegistry`；`ExtensionStorageData` 补 `registryCaches?` / `marketSources?` |
| 工具 | `src/modules/extensions/utils/market-sources.ts`：`isPublicHttpsUrl` / `normalizeMarketSourceUrl` / `createCustomMarketSourceId`（FNV-1a → `custom-<hash>`）/ `buildCustomMarketSource` / `normalizeMarketSourceRecord` / `mergeMarketSourceRegistries` / `collectApprovedExtensionIds` |
| 存储 | `src/stores/storage/extensions.ts`：读白名单 + 写列表**同时**加入 `marketSources` / `registryCaches`；旧 `registryCache` 迁移为 `registryCaches['official']`；`updateRegistryCacheForSource` / `getRegistryCacheForSource` / `removeRegistryCacheForSource` / `setMarketSources` |
| runtime | `src/stores/runtime/extensions.ts`：`fetchRegistry` 多源并发（`Promise.allSettled`）→ 合并 → 冲突 warn → 源状态 → 跨源卸载判定；`addMarketSource` / `removeMarketSource` / `setMarketSourceEnabled`；`registry` ref 改 `MergedExtensionRegistry \| null` |
| 组合式 | `src/modules/extensions/composables/use-extensions.ts`：`filteredExtensions` 补 `sourceId` / `sourceName` |
| UI | `src/modules/settings/ui/categories/extensions/market-sources.vue`（新）、`src/modules/extensions/components/extensions-marketplace.vue`（重写）、`extension-card.vue`、`extension-badge.vue`、`src/modules/extensions/pages/extensions.vue`、`src/modules/settings/ui/categories/extensions/index.vue` |
| i18n | 17 个语言包各新增 14 键（中文 locale code = `ch`） |

### 9.2 关键行为约定

1. **官方源不落盘**：内置常量、`builtIn: true`、不可删 / 不可改 URL；只有自建源落盘。
2. **自建源 id 由 URL 派生**（FNV-1a → `custom-<hash>`），同 URL 重复添加视为「已存在」静默成功。
3. **合并优先级**：官方源优先、先到先得；id 冲突时 `console.warn` 并在 UI 标注来源。
4. **卸载判定**：判定集 = 合并后 id 全集 ∪ 已停用源缓存条目；**只有所有启用源都成功或降级成功时**才执行判定，任一源彻底不可达 ⇒ 整体跳过（不误卸载）。
5. **单源失败降级**：用该源上次缓存并标 `stale`；错误只记 `registrySourceStates`，全局 `registryError` 仅在所有源都失败时置位。
6. **删源连带删缓存**；已停用源的缓存条目仍进展示列表，避免已装扩展凭空消失。
7. `registry` ref 只持有 `{ schemaVersion, extensions }`，**不暴露** `conflicts` / `failedSourceIds`。

### 9.3 门禁结果

| 门禁 | 命令 | 结果 |
| --- | --- | --- |
| 类型 | `npm run ts` | exit 0 |
| Lint | `npx eslint .` | **186 errors / 0 warnings**（基线 188，用 `git stash push -u` 现量） |
| 样式 | stylelint（改动 6 个 vue） | 0 违规 |
| 单测 | vitest（扩展相关 3 文件） | **37/37 全绿** |
| Rust | `cargo test --lib`（`src-tauri/` 下，MSVC） | exit 0，**912 tests 通过** |

### 9.4 公网发布物

| 仓库 | 内容 |
| --- | --- |
| `kizemo/sfm-marketplace`（public） | `registry.json` + `README.md` |
| `kizemo/sfm-git-extension`（public） | 扩展源码，tag `v0.1.0`；根 `package.json` raw 200 / 3624 B |

清单：`scripts/ext-market/registry.json`，条目 `kizemo.git`，`repository=https://github.com/kizemo/sfm-git-extension`，`releaseMetadata['0.1.0'].manifest=https://raw.githubusercontent.com/kizemo/sfm-git-extension/main/package.json`；`node scripts/ext-market/publish.mjs --validate` 通过。

### 9.5 交付安装包

- 路径：`release/Alpha File Manager_3.9.2_x64-setup.exe`（= `src-tauri/target/release/bundle/nsis/…`）
- 大小 44 946 582 B，SHA256 `45856f92ba76c4ff03f853733151b96236bd584f331c7ba66abde8cbb2d48c16`
- **无调试端口**：7z 解包内嵌 `alpha-file-manager.exe`（80 642 560 B，sha256 `065b2185…303bb35`）不含 `remote-debugging-port`；对照 `.superpowers/sdd/afm-cdp.exe`（同尺寸）**含**该字符串 ⇒ 判别性成立
- `src-tauri/tauri.conf.json` 已回滚（与 HEAD 一致，无 `additionalBrowserArgs`）

### 9.6 真机验收：B29 40/40 全绿

三轮：第一轮 33/40（7 FAIL 全是脚本缺陷）、第二轮 34/40（6 FAIL 全是网络时序）、**第三轮 40/40 全绿**（exit 0）。脚本：`.superpowers/sdd/b29-market-sources-accept.cjs`（含 `patch-b29.cjs` / `patch-b29b.cjs` 两轮修补）；报告与截图：`.superpowers/sdd/accept-b29/`；归因与证据：`.superpowers/sdd/accept-b29-findings.md`。

覆盖：P1 夹具源持久化（id `custom-7fosct`）· P1b 双源公网拉取 · P2 设置卡片只读/可删 · P3 非法 URL toast 与语言包逐字相等 · P4 不可达源降级且全局 `registryError` 仍 null · P5 来源切换条 + 来源角标 · P6 按源过滤 · P7 停用源后条目仍在 · P8 删源后内存+磁盘同时消失且旧单数字段已迁移 · P9 安装探测 · P10 四个真实数据文件 SHA256 与备份逐字一致（**真机数据零污染**）。

### 9.7 环境限制（不阻塞交付，但影响覆盖面）

- 本机 `github.com:443` 不可达（session 内 3 次独立复现 15s 超时）⇒ 扩展包下载 URL `https://github.com/<owner>/<repo>/archive/refs/tags/v{version}.zip` 必然失败；GitHub 302 的真实落点 `codeload.github.com/kizemo/sfm-git-extension/zip/refs/tags/v0.1.0` 直连 **200 / 31361 B** ⇒ **包完好，是网络出口问题**。
- `raw.githubusercontent.com` 间歇不可达（runner 前 15 次探测失败，第 16 次成功）。
- 因此「从未安装状态的自建源扩展安装」这一环无法在真机跑闭环：真机数据里 `kizemo.git` 已是已安装态。该下载/安装链路是既有实现，本特性**未改动** `installExtension`；市场源侧新增部分已由 P5–P8 覆盖。

### 9.8 下一阶段建议（阶段 2）

1. 先解决「从未安装态的自建源扩展」闭环验收（需要可达 `github.com` 的出口，或把包分发改到可达的公网 CDN）。
2. 复核 `filterReachableRegistryEntries` 的逐条 manifest 探测（`src/modules/extensions/utils/registry-utils.ts:41`）在网络抖动时把可达条目误判为不可达的问题（设计文档 §十.1 未决项 1）。
3. 阶段 2 若要做二进制自托管（L3），需要动 Rust（下载 / 校验 / sha256），届时再评估。
