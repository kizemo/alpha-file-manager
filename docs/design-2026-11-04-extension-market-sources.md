# 扩展市场：自建市场 + 在官方市场基础上加自建入口（设计与实施计划）

- 日期：2026-11-04
- 状态：分析完成 + 市场侧工具已交付；宿主侧接线未开工
- 相关文档：`docs/handoff-2026-11-04-extension-market-sources.md`（交接）、`docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md`（官方扩展开发文档）、`scripts/ext-market/README.md`（工具包用法）

---

## 一、任务拆解

用户诉求原文：**「扩展，希望搭建一个自己的市场或在 sigma 市场基础上添加自己市场的入口。请分析任务并执行。」**

拆成三个可独立回答的问题：

1. **「自己的市场」是什么？** —— 一份自己控制的 `registry.json` 清单 + 一个让宿主能读到它的入口。
2. **「在 sigma 市场基础上加自己市场的入口」= 哪种形态？** —— 应用内多市场源（官方源保留 + 自建源并列），还是只是加一个跳转到自建市场网页的外链。
3. **扩展二进制要不要也自托管？** —— 自建市场只托管清单（包仍在 GitHub Releases），还是连 release ZIP 也放自己的服务器。

问题 3 是工作量的分水岭（3 倍差距），问题 2 决定 UI 形态。

---

## 二、结论（先说结果）

**推荐：应用内「多市场源」模型（官方源 + 自建源并列），Phase 1 只自托管清单，扩展包仍走 GitHub Releases，Rust 侧零改动。**

- 自建市场的落地成本 = **一份 `registry.json` 放在公网 https 地址**（本轮已交付生成与校验工具，用户当天就能把自己的扩展挂上去）。
- 宿主接线 = 前端 store / UI / 存储 / 17 个语言包 / 单测，**不动 Rust**（应用侧拉取已支持任意公网 https 域名）。
- 二进制自托管列为 Phase 2（可选），需要改安装链路 + 安全白名单，不建议现在做。

---

## 三、现状勘察（结论均已逐条落到文件行号）

### 3.1 市场是单源硬编码

- `src/data/external-links.ts:29-35` —— 唯一的市场地址常量：
  `https://raw.githubusercontent.com/sigma-hub/sfm-marketplace/main/registry.json`
- `src/stores/runtime/extensions.ts:690` `fetchRegistry()` —— 拉这个 URL，解析、校验、过滤、写缓存、卸载未登记扩展。
- 全仓没有第二个市场地址，也没有「市场源」概念。

### 3.2 清单与条目结构（宿主校验规则）

`src/modules/extensions/runtime/validation.ts`：

| 规则 | 位置 |
| --- | --- |
| `assertValidRegistryData`：根必须是对象，`schemaVersion` 必填且必须 `x.y.z`（`isVersionString` = `/^\d+\.\d+\.\d+(?:-[\w.]+)?$/`，:32-34），`extensions` 必为数组，每条过 `isValidRegistryEntry` | :450-468、:32-34 |
| `isValidRegistryEntry`：`id` 非空且过 `EXTENSION_ID_PATTERN`；`name`、`description` 非空；`publisher` 必填且只有 `name`/`url`；`repository` 非空；`featured` 必须布尔；`categories` 非空字符串数组；`tags` 可选字符串数组；`integrity` 可选非空字符串；`releaseMetadata` 的版本键必须是 `x.y.z` | :394-448 |
| 类型定义 | `src/types/extension.ts:23-42` |

### 3.3 安装链路绑定 GitHub（这决定了「自建市场」的边界）

条目的 `repository` 字段被当成 GitHub 仓库用：

- manifest：`getExtensionManifestUrl(repository, ref)` → `raw.githubusercontent.com/<owner>/<repo>/<ref>/package.json`（`src/data/extensions.ts:176-183`）
- 版本列表：`fetch_github_tags` → GitHub tags API / atom feed（`src-tauri/src/extensions/github.rs`）
- 安装包：GitHub release ZIP / `codeload.github.com/<owner>/<repo>/archive/...`（`src-tauri/src/extensions/http.rs:670-682`）
- 完整性：`releaseMetadata[version].integrity`（形如 `sha256:<hex>`）对 release ZIP 校验（`src/modules/extensions/utils/manifest-utils.ts:126-132`）

⇒ **只要条目的 `repository` 指向一个 GitHub 仓库，自建市场就能完整复用现有安装链路，一行 Rust 都不用改。**

### 3.4 网络策略（关键硬约束）

- 应用侧拉取：`fetch_url_text` → `validate_remote_url` + `build_remote_http_client()`（`src-tauri/src/extensions/github.rs:225-232`）
- 该客户端使用 `PublicOnlyResolver`：**回环 / 内网 / 链路本地 / 云元数据地址在 connect 阶段被拒**（`src-tauri/src/extensions/http.rs:144-146`、`src-tauri/src/extensions/security.rs:116-126`）
- ⇒ **自建市场必须放在公网 https 地址**，`http://127.0.0.1:8788` 这类本地服务宿主永远拉不到（`scripts/ext-market/serve.mjs` 只是浏览器自检工具）
- 兜底：`build_jsdelivr_fallback_url` 只对 `raw.githubusercontent.com` 生效（`src-tauri/src/extensions/http.rs:613-618`）⇒ **把 registry.json 放 GitHub raw，国内可用性比自建服务器更好**

### 3.5 卸载判定（最高风险点）

`src/stores/runtime/extensions.ts:1894-1930` `removeUnapprovedInstalledExtensions()`：

> 任何「非本地安装」且**不在本次拉到的 registry 里**的已安装扩展 ⇒ 会被 `removeInstalledExtension` 卸载。

后果：直接加第二个源而不改这里 ⇒ **自建源里的扩展每次刷新市场都会被卸掉**（官方 registry 里没有它们）。这是本次接线必须第一优先修的点。

### 3.6 可达性过滤的网络开销

`src/modules/extensions/utils/registry-utils.ts:16-62`：每个条目都会探一次 manifest URL（`getRegistryManifestUrl(repo,'main')`）才算「可达」。条目越多、源越多，首屏网络开销线性增长。

### 3.7 存储层

`src/stores/storage/extensions.ts`：`extensionsData` 里 `registryCache` 是**单个** `{ data, fetchedAt }`（:41-47、:288-295），读在 `loadStorageData` / `applyExtensionsDataRecord`，写在 `saveStorageData`（:157-170）。

⚠️ 本仓库已踩过的坑（见 `AGENTS.md`）：**新增顶层字段必须同时改「读白名单」与「写列表」**，只改一边 ⇒ 界面有、重启即丢。

---

## 四、方案对比

| 方案 | 用户体验 | 改动面 | 风险 | 结论 |
| --- | --- | --- | --- | --- |
| **L1 仅加外链入口**（市场页一个「我的市场」按钮，跳自建市场网页） | 要离开应用、回到网页下载，再手动装文件夹 | 极小（一个按钮 + 17 locale） | 低，但没解决「在应用内安装」 | 兜底方案，可作为 L2 的补充 |
| **L2 应用内多市场源**（官方源 + 自建源并列，可增删） | 自建扩展在应用内直接装、可更新 | 前端 store + UI + 存储 + 17 locale + 单测；**Rust 零改动** | 中（卸载判定必须先修） | ✅ **推荐** |
| **L3 自托管二进制**（release ZIP 也放自己的服务器） | 同 L2，但摆脱 GitHub 依赖 | 安装链路重写 + 下载白名单 + 完整性校验重做 | 高 | Phase 2 可选 |

**推荐路径：先 L2（做完后自建市场立刻可用），L1 作为 L2 的补充入口，L3 不做。**

---

## 五、推荐方案（L2）设计

### 5.1 数据结构（新增 `src/types/extension.ts`）

```ts
export type ExtensionMarketSourceKind = 'official' | 'custom';

export type ExtensionMarketSource = {
  id: string;                  // 稳定 id：'official' 或 'custom-<hash>'
  name: string;                // 展示名
  url: string;                 // registry.json 完整地址
  kind: ExtensionMarketSourceKind;
  enabled: boolean;
  builtIn?: boolean;           // 官方源：不可删、不可改 url
};

// 条目带上来源，便于市场页按来源过滤
export type ExtensionRegistryEntryWithSource = ExtensionRegistryEntry & { sourceId: string };
```

### 5.2 拉取与合并（改造 `fetchRegistry`）

```
sources = [官方源(内置, 恒启用)] + 用户自建源(enabled)
1) 并发拉取各源（Promise.allSettled，单源失败不阻塞其它源）
2) 每源：fetchUrlText(url) → assertValidRegistryData → 条目打上 sourceId
   - 失败源：沿用该源上一次的缓存条目（标记 stale），并把该源错误单独记录（不写全局 registryError）
3) 合并：官方源优先；id 冲突时先到先得（官方在前），冲突写 console.warn
4) 合并完成后再调用 removeUnapprovedInstalledExtensions（判定集 = 所有源条目的 id 全集）
5) 缓存改为按源存：registryCaches: Record<sourceId, { data, fetchedAt }>
```

**要点**：第 2 步的「失败源保留旧条目」是防误卸载的关键；第 4 步必须放在合并之后。

### 5.3 卸载判定修正

`removeUnapprovedInstalledExtensions` 的入参改为「合并后的全集」。同时补一条：**用户手动停用的源，其条目不参与卸载判定**（停用 ≠ 下架）。

### 5.4 UI 改动

| 位置 | 改动 |
| --- | --- |
| `src/modules/extensions/components/extensions-marketplace.vue` | 顶部加来源切换条：`全部 / 官方 / <自建源名×N>`；条目卡片上加来源角标 |
| `src/modules/settings/ui/categories/extensions/`（现有目录） | 新增「市场源」卡片：源列表（名称、URL、启用开关、删除）+「添加市场源」对话框（名称 + URL 必填，URL 需为公网 https） |
| `src/modules/extensions/pages/extensions.vue` | 市场页与安装页的 tab 结构无需改，仅传参 |

### 5.5 i18n（17 个语言包）

新增键（中文写在 `ch.json`）：

```
extensions.marketSources            市场源
extensions.marketSourceAdd          添加市场源
extensions.marketSourceName         名称
extensions.marketSourceUrl          清单地址
extensions.marketSourceOfficial    官方市场
extensions.marketSourceCustom       自建市场
extensions.marketSourceAll          全部
extensions.marketSourceEmpty        还没有自建市场源
extensions.marketSourceRemove       移除市场源
extensions.marketSourceRemoveConfirm 移除后该源的扩展将不再显示，确认移除？
extensions.marketSourceInvalidUrl   清单地址必须是公网 https 地址
extensions.marketSourceUnreachable  市场源暂时无法访问，正在使用上次缓存
```

### 5.6 存储改动

- `ExtensionStorageData` 增加 `marketSources: ExtensionMarketSource[]` 与 `registryCaches: Record<string, { data; fetchedAt }>`。
- **读侧**：`loadStorageData` + `applyExtensionsDataRecord` 都要加。
- **写侧**：`saveStorageData` 都要加。
- 官方源不落盘（内置常量），只有自建源落盘。

### 5.7 Rust 侧

**无改动。** 应用侧 `fetch_url_text` 已支持任意公网 https 域名；扩展自身 HTTP API 的 host 白名单（`load_extension_http_allowed_hosts`）只约束扩展自己发请求，与市场拉取无关。

---

## 六、风险清单

| 风险 | 触发条件 | 后果 | 处置 |
| --- | --- | --- | --- |
| **自建扩展被自动卸载** | 加了第二源但没改卸载判定 | 用户扩展消失 | §5.2/§5.3，必须先修 |
| 源不可达时市场整体不可用 | 自建源 404 / 域名过期 | 刷新报「市场不可用」 | 单源失败降级为「用缓存 + 标记 stale」，不写全局错误 |
| 市场源 URL 指到内网 | 用户填 `http://192.168.x.x/registry.json` | 宿主永远拉不到（PublicOnlyResolver） | 添加时前端校验必须是公网 https，并提示本地服务不可用 |
| 条目 id 冲突 | 官方源与自建源有同名 id | 装到错的包 | 官方源优先 + 冲突告警；UI 上标出来源 |
| 首屏变慢 | 每条都探 manifest（§3.6） | 源多时网络开销线性增长 | 探测只在「安装前」做，或对自建源沿用信任策略（需拍板） |
| 存储字段只改一边 | 漏改读/写列表 | 界面有、重启丢 | §5.6 的读写两侧一起改，并加往返单测 |

---

## 七、验收计划

1. **单测（vitest）**：合并优先级、id 冲突、单源失败降级、跨源卸载判定、存储字段往返。
2. **门禁**：`vue-tsc`、`eslint .`（对齐基线 186）、`vitest`（对齐基线 14 红）、`cargo test --lib`（本方案**无需改 Rust**，仍要跑以证明没回归）。
3. **真机验收**（必须 `tauri build` 产物，`tauri dev` 结论作废）：
   - 把 `scripts/ext-market/registry.json` 推到**公网 https**（GitHub raw 或 GitHub Pages）⇒ 宿主里添加为自建源 ⇒ 市场页能看到该源 ⇒ 能安装 ⇒ 能更新 ⇒ 能卸载；
   - 断网 / 改坏 URL ⇒ 验证「降级用缓存、不误卸载」；
   - 官方源与自建源条目同时在册 ⇒ 验证卸载判定不误删自建扩展（**本方案的核心断言**）。
4. **产物**：最终 exe 必须不含调试端口（`tauri.conf.json` 的 `additionalBrowserArgs` 回滚后重打）。

---

## 八、工作量与分期

| 阶段 | 内容 | 量级 |
| --- | --- | --- |
| 阶段 0（**本轮已交付**） | 市场侧工具包：生成 / 校验 / sha256 / 本地自检 / schema / README + 示例 registry.json | 已完成 |
| 阶段 1（下一会话） | 宿主接线：类型 + 存储 + store 改造 + 市场页来源切换 + 设置页管理 + 17 locale + 单测 + 门禁 + 打包 + 真机验收 | 中等（单会话可完成） |
| 阶段 2（可选） | L1 外链入口；L3 自托管二进制 | 小 / 大 |

---

## 九、本轮已交付

| 文件 | 说明 |
| --- | --- |
| `scripts/ext-market/publish.mjs` | 条目生成 / 更新 / 移除 / 校验 / sha256 计算；校验规则与宿主 `assertValidRegistryData` 逐字对齐 |
| `scripts/ext-market/serve.mjs` | 本地静态服务（**仅浏览器自检**，宿主拉不到回环地址） |
| `scripts/ext-market/registry.schema.json` | 清单 JSON Schema，可用于外部 CI |
| `scripts/ext-market/README.md` | 用法、部署方式（GitHub raw / Pages / 自建服务器）、硬约束 |
| `scripts/ext-market/registry.json` | 示例清单（已含 `kizemo.git` 条目） |

自检结果：`publish` 退出 0、`validate` 退出 0（1 个条目）、`--sha256` 输出 `sha256:<hex>`。

---

## 十、未决问题（需要拍板）

1. **探测策略**：自建源条目是否也逐条探 manifest（§3.6）？信任自建源可省掉 N 次请求，但会失去「条目失效即隐藏」的保护。建议：**保留探测，但只对将要展示的条目做**，或加一个「跳过校验」开关。
2. **自建源的信任级别**：自建源里的扩展安装时是否仍强制走 GitHub release + sha256 校验？建议**保持强制**（安装链路不变，安全性不降）。
3. **市场源是否允许 `file://` 或本地目录**：当前网络策略一律拒绝；如需要「本地市场」体验，需要单独加一条 Rust 侧的本地文件读取通道（不建议现在做）。
4. **自建市场的公网落点**：GitHub raw（推荐，国内可用性好）、GitHub Pages，还是自己的服务器？
