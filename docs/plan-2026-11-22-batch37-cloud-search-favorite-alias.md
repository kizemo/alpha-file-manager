# 批次 37 计划：云盘加入全局检索 + 连接名称预填 + 收藏显示名（别名）

> 依据：`docs/handoff-2026-11-21-batch35-remote-thumbnail-delivery.md`（批次 35 已发布 3.16.1，工作树干净）
> 本文件是**批次 37 的落点真源**。三个写集互不重叠，Worker 各自只碰自己那一列。

## 0. 本轮三项需求与结论

| # | 需求 | 结论 |
|---|---|---|
| 1 | 云盘能否加入全局检索 | **可以，但不是「加个开关」** —— 全局检索当前是纯本地索引（`scan.rs:780` `WalkDir`），对 `sfm://remote/...` 零感知。本轮用已有的 `RemoteProvider::read_dir`（`remote/provider.rs:56`）递归遍历，写进**独立的云盘索引**，查询时与本地结果合并 |
| 2 | 新建网盘时名称预填 | **做**。`settings.remote.netdisk.baidu/quark/aliyun` 三个 i18n key 已存在（`ch.json:818-820`）⇒ 不需要新增语言包 |
| 3 | 收藏夹条目可改显示名 | **做**。只改收藏菜单里的显示名，**不动原始文件名/目录名** |

## 1. 需求 1：云盘加入全局检索

### 1.1 可行性依据（已核实，非推断）

- 全局索引 = tantivy，`global_search/` 整个模块 grep `remote|sftp|ftp|cloud|provider` **零命中** ⇒ 当前对远程零感知。
- `scan.rs:895-903` 的「盘根必须 `Path::exists()`」过滤会把 `sfm://remote/...` 直接滤掉；`scan.rs:771` 的 `std::fs::read_dir` 也读不了远程。
- `normalize_path`（`src-tauri/src/utils.rs:18`）**只是 `replace('\\', "/")`** ⇒ `sfm://remote/...` 原样保留，不会被改坏。
- `RemoteEntry`（`remote/provider.rs:16-35`）已带 `name / path / is_dir / size / modified_time` ⇒ **索引需要的元数据全都有，不需要任何新远程 API**。
- `RemoteProvider::read_dir`（`provider.rs:56`）是 async，`tokio` 是 `src-tauri/Cargo.toml:54` 的直接依赖（`rt` + `rt-multi-thread`）⇒ 可以在 tokio runtime 里 await。
- 前端导航已能打开 `sfm://remote/...`（`remote/commands.rs:627` `remote_read_dir`）⇒ 检索结果点得开。

### 1.2 关键架构裁决：**独立云盘索引，不混进本地索引**

**为什么不能把云盘塞进本地索引的 `drive_roots`**：

- `incremental.rs:380` 的 `same_drive_roots` 决定「全量重扫 vs 增量」。把远程 id 混进根集合 ⇒ 每轮判定不一致 ⇒ **每轮退化成 22 分钟全量重扫**（`scan.rs:905-915` 注释里已记过同类坑：插着 U 盘时实测一轮 638 秒）。
- `index_watcher.rs:365` 只 watch 本地盘根；`RemoteProvider` **没有 watch 能力** ⇒ 远程只能全量遍历，每轮都遍历 = 每轮几千次网络往返 + 百度/夸克 API 配额消耗。

**因此**：云盘走**自己的 tantivy 索引**（`<appData>/remote-index/`），只在「勾选集合变化」或用户手动重建时刷新；本地索引的增量管线**一行不动**。

### 1.3 写集

| 层 | 文件 | 改动 |
|---|---|---|
| Rust 模型 | `src-tauri/src/remote/connection.rs:74-100` | 加 `#[serde(default)] pub include_in_global_search: bool`（**没有这个键的老文件读出来就是 false**） |
| Rust 索引 | `src-tauri/src/global_search/remote_index.rs`（**新建**） | 递归遍历 + 建索引 + 查询 + 清理 |
| Rust 查询 | `src-tauri/src/global_search/query.rs:142` `global_search_query` | 合并本地与云盘结果 |
| Rust 命令 | `src-tauri/src/global_search/commands.rs` | 新增 `global_search_index_remote_connections` |
| Rust 注册 | `src-tauri/src/lib.rs` + `src-tauri/permissions/app-commands.toml` | 注册新命令（`app-commands.toml` 用**字母序**） |
| TS 模型 | `src/types/remote.ts:60-82` / `:91-104` | `includeInGlobalSearch?: boolean`（镜像 Rust，注释写明逐字镜像约定） |
| TS fixture | `src-shared/remote-connection.sample.json` | 加 `"includeInGlobalSearch": false`（Rust 侧 `registry.rs:175` 与 TS 侧 `remote-connection-schema.test.ts` 双向读它） |
| TS store | `src/stores/storage/remote-connections.ts:555-` | 落盘带新字段；`:302-306` 的 `applyRecord` 归一化 `?? false` |
| TS UI | `netdisk-add-dialog.vue` + `connection-form.vue` | 各加一个「加入全局检索」Checkbox |

### 1.4 行为约定（Worker 必须照做）

1. **勾选默认 = 不勾**（`false`）。理由：建索引要走网络、耗时与配额都不可忽略，默认打开等于替用户做决定。
2. 索引只在**显式触发**时重建：设置页加一个「重建云盘索引」按钮；连接保存后若勾选状态变了也触发一次。
3. 遍历**必须有上限**：最大深度（复用 `scanDepth` 量级即可）与最大条目数（常量封顶），超限要**在日志与 UI 上说清楚**，不能静默截断。
4. 单个连接失败**不能拖垮整轮**：按连接隔离，一个云盘 400/超时，其它照常建索引。
5. 取消勾选 = 该连接的条目从云盘索引里**删掉**（重建时按当前勾选集合整体重建）。
6. 检索结果里云盘条目必须能**分组显示连接名**（前端用 `sfm://remote/<连接id>/` 前缀 + 连接表取名）。

## 2. 需求 2：新建网盘时预填名称

| 文件 | 改动 |
|---|---|
| `src/modules/settings/ui/categories/remote/netdisk-add-dialog.vue:118` | `name.value = ''` → 预填对应 provider 的显示名（按 `protocol` 取 baidu / quark 两个 key） |
| `src/modules/settings/ui/categories/remote/connection-form.vue:788` / `:840` | 新建且是网盘形态时预填 `t('settings.remote.netdisk.aliyun')`；**编辑已有连接时不得覆盖用户填的名字** |

- 三个 i18n key **已存在**，本项**不新增任何语言包**。
- 预填只发生在「新建」，「重开弹窗 / 编辑 / 保存后重置」这三条路径都要想清楚，不得把用户已输入的名字冲掉。

## 3. 需求 3：收藏显示名（别名）

### 3.1 语义

- 别名**只影响收藏菜单 / 收藏卡上的显示**，**不碰原始文件名、目录名、路径**。
- 别名为空 ⇒ 回退到现有 `getPathDisplayName(path, t)` 行为（**零回归**）。

### 3.2 写集

| 层 | 文件 | 改动 |
|---|---|---|
| 模型 | `src/types/user-stats.ts:35-41` `FavoriteItem` | 加 `alias?: string` |
| 持久化 | `src/stores/storage/user-stats.ts:230-259` | `normalizeFavorites` 回填 `alias`（老记录为 `''`） |
| 持久化 | `src/stores/storage/user-stats.ts:300-312` | `addToFavorites(path, isFile, categoryId, alias?)` |
| 持久化 | `src/stores/storage/user-stats.ts` | 新增 `setFavoriteAlias(path, alias)` action（**复用现有 `favorites` 键的整份写盘**，不加新 set() 键） |
| 添加对话框 | `file-browser-favorite-category-dialog.vue` | 加「显示名称（可选）」输入，`confirm` 事件多带一个 alias 参数 |
| 添加对话框 | `dashboard-new-favorite-dialog.vue` | 同上 |
| 管理 | `dashboard-entry-actions-menu.vue:52-57,86-141` | 加「编辑显示名称」项 ⇒ 复用 `dashboard-name-dialog.vue`（已是通用改名弹窗） |
| 渲染 | `entry-card.vue:40-43` | 有 alias 用 alias，否则 `getPathDisplayName` |
| 渲染 | `quick-access-entry.vue:25-31` | 同上（alias 从 `quick-access-panel.vue:198-206` 传下来） |
| 渲染 | `dashboard.vue:669-671` | `getFavoriteDisplayName` 同步（a11y/tooltip 要跟可见名一致） |

### 3.3 不受影响（已核实）

- Rust 侧**没有收藏持久化**（只有 MCP 只读快照 `mcp/tools.rs:422`），本项**不动 Rust**。
- `normalize-path.ts:218` `getPathDisplayName` 保留为**回退**，不改。

## 4. 新增 i18n key（**17 个语言包必须同步**：`ch de en es fa fr he hi hy it ja pt ru sl tr ur vi`）

| key | 中文值 |
|---|---|
| `settings.remote.field.includeInSearch` | 加入全局检索 |
| `settings.remote.field.includeInSearchHint` | 勾选后，该连接的文件会进入全局检索索引（需手动重建云盘索引） |
| `dashboard.actions.renameFavorite` | 编辑显示名称 |
| `dashboard.newFavoriteDialog.displayNameLabel` | 显示名称 |
| `dashboard.newFavoriteDialog.displayNamePlaceholder` | 留空则使用原始名称 |
| `fileBrowser.favoriteCategoryDialog.displayNameLabel` | 显示名称 |
| `fileBrowser.favoriteCategoryDialog.displayNamePlaceholder` | 留空则使用原始名称 |

门禁：`messages-key-alignment.test.ts`（每个语言包叶子 key 集合必须与 `en.json` **完全相等**）、`messages-used-keys-exist.test.ts`（源码里的 `t('...')` 必须在 `en.json` 存在）⇒ **7 个 key × 17 个语言包，一个都不能少**。

## 5. Worker 分配（写集零交集）

| Worker | 写集 |
|---|---|
| **W37-A（Rust）** | `src-tauri/src/remote/connection.rs`、`src-tauri/src/global_search/remote_index.rs`(新)、`src-tauri/src/global_search/{commands,query,mod}.rs`、`src-tauri/src/lib.rs`、`src-tauri/permissions/app-commands.toml` |
| **W37-B（收藏别名）** | `src/types/user-stats.ts`、`src/stores/storage/user-stats.ts`、`src/modules/navigator/components/file-browser/file-browser-favorite-category-dialog.vue`、`src/modules/navigator/components/file-browser/utils/file-browser-favorites.ts`、`src/modules/navigator/components/file-browser/file-browser.vue`、`src/modules/dashboard/components/{dashboard-new-favorite-dialog,dashboard-entry-actions-menu,entry-card}.vue`、`src/modules/dashboard/pages/dashboard.vue`、`src/modules/nav-sidebar/components/{quick-access-panel,quick-access-entry}.vue` |
| **W37-C（连接表单）** | `src/types/remote.ts`、`src/stores/storage/remote-connections.ts`、`src/modules/settings/ui/categories/remote/{connections-section,connection-form,netdisk-add-dialog}.vue`、`src/modules/settings/ui/categories/search/global-search.vue`、`src-shared/remote-connection.sample.json` |

**语言包（17 个 JSON）与 CHANGELOG 由根会话统一改** —— 否则两个 Worker 同时改同一批 JSON 必然冲突。

## 6. 门禁与验收

| 装置 | 判据 |
|---|---|
| `cargo test --lib -- --test-threads=1` | 0 failed（基线 1601 passed / 1 ignored） |
| `npm run ts` | EXIT=0 |
| `npm run lint:check` | EXIT=0（`--max-warnings 0`） |
| `npm run test:unit:run` | 全绿（基线 3492） |
| `npm run tauri:build` | 打包成功（真机验收唯一合法产物） |
| 真机 | ① 新建网盘名称已预填 ② 「加入全局检索」勾选后能建索引 ③ 搜到云盘文件并能打开 ④ 收藏夹能改显示名且**原始文件名不变** |

**J1_*/J2_* 阈值一律不改**（批次 35 §6 已锁）。
