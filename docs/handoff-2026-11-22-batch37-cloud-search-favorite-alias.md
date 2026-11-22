# 批次 37 交接文档 —— 云盘接入全局检索 / 新建网盘名称预填 / 收藏显示名（别名）

- 日期：2026-11-22
- 版本：**3.16.1 → 3.17.0**（`package.json` + 3 个 Cargo manifest 已 `sync-version`）
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 批次 37 计划真源：`docs/plan-2026-11-22-batch37-cloud-search-favorite-alias.md`
- 上一批次交接：`docs/handoff-2026-11-21-batch35-remote-thumbnail-delivery.md`

---

## 1. 用户需求与本批次的结论

| # | 需求 | 结论 |
|---|---|---|
| 1 | 百度网盘等云盘能否做文件检索；新建网盘时加「加入全局检索」选项，由添加者决定 | **可行且已实现**。云盘文件走**独立 tantivy 索引**（`<appData>/remote-index/`），不混进本地 `drive_roots`。连接记录上新增 `includeInGlobalSearch` 开关，默认**不勾**（建索引要走网络、吃供应商 API 配额，不替用户决定）。 |
| 2 | 新建网盘时名称框默认填入对应网盘名称 | **已实现**。`resolveRemoteConnectionNameDefault(protocol, existingName, translate)`：编辑态原样返回已有名字（绝不覆盖用户填的）；非网盘协议返回空串；新建时取 provider 显示名。 |
| 3 | 收藏夹管理 / 添加收藏对话框可改收藏显示名，**只改显示名不改原始文件名/目录名** | **已实现**。`FavoriteItem.alias` 只影响 UI 显示（dashboard 收藏卡 + 侧栏快速访问）；路径与真实文件名零改动；别名为空 ⇒ 完全回退 `getPathDisplayName`。 |

**未改动**：`J1_*` / `J2_*` 阈值一个没动（已核对 `git diff`，无相关行）。

---

## 2. 已完成 / 已验证

### 2.1 四道门禁（全绿）

| 门禁 | 结果 | 证据 |
|---|---|---|
| `cargo test --lib` | ✅ 退出码 0，**1617 tests** | 根会话复跑；artifact 头 `<bash_status>Command exited with code 0` |
| `npm run ts`（vue-tsc） | ✅ TS=0 | `.superpowers/sdd/b37/gates2/ts.log` |
| `npm run lint:check` | ✅ LINT=0（eslint 0 problem + stylelint 0 error） | `.superpowers/sdd/b37/gates2/lint.log` |
| `npm run test:unit:run` | ✅ UNIT=0，**3546 tests / 329 files** | `.superpowers/sdd/b37/gates2/unit.log` |
| `npm run verify-changelog` | ✅ OK，`[3.17.0] - November 2026` 在最前 | 命令输出 |
| `npm run verify:sandbox`（构建内含） | ✅ 2 个扩展入口 0 violation | `.superpowers/sdd/b37/build/build.log` |

本轮修掉的 3 个门禁问题（**都是本批次自己引入的**，不是历史遗留）：

1. `src/modules/settings/ui/categories/search/global-search.vue` stylelint `order/properties-order`：`padding` 必须排在 `margin` 之前 —— 已交换。
2. `docs/plan-2026-11-22-batch37-cloud-search-favorite-alias.md` 第 61 行表格：单元格里的字面 `|` 把行撑成 3 列 —— 已改写该单元格。
3. `src/modules/settings/ui/categories/remote/netdisk-add-dialog.test.ts`「名称为空时不保存」：名称预填后该用例前提失效 —— 已补 `setValue('')` 显式清空，**保留原测试意图**（空名仍然不保存），通过数不变。

### 2.2 语言包

- 17 个语言包（`ch de en es fa fr he hi hy it ja pt ru sl tr ur vi`）各补 **15 个新 key**。
- **叶子 key 集合与 `en.json` 完全相等**：各 1664 leaves，0 missing / 0 extra —— 脚本 `.superpowers/sdd/b37/i18n-align-check.cjs`。
- 格式门禁：2 空格缩进、LF、无 BOM、末尾一个换行（`JSON.stringify(obj,null,2)+'\n'`）。

新增 15 个 key：

```
settings.remote.field.includeInSearch
settings.remote.field.includeInSearchHint
settings.globalSearch.remoteIndexTitle
settings.globalSearch.remoteIndexDescription
settings.globalSearch.rebuildRemoteIndex
settings.globalSearch.rebuildRemoteIndexDescription
settings.globalSearch.noRemoteInSearch
globalSearch.remoteIndexing
globalSearch.remoteIndexDone
globalSearch.remoteIndexFailed
dashboard.actions.renameFavorite
dashboard.newFavoriteDialog.displayNameLabel
dashboard.newFavoriteDialog.displayNamePlaceholder
fileBrowser.favoriteCategoryDialog.displayNameLabel
fileBrowser.favoriteCategoryDialog.displayNamePlaceholder
```

### 2.3 CHANGELOG

- 根 `CHANGELOG.md`：顶部插入 `## [3.17.0] - November 2026`（Summary + `### What changed` + 3 条 `**Added**`）。英文用 **"favorite"**（美式，仓库 9:6 占优）。
- 14 本地化 `public/changelog/<loc>/changelog.md`：各插入 3.17.0 段落 —— 脚本 `.superpowers/sdd/b37/i18n-changelog-317.mjs`，验收脚本 `.superpowers/sdd/b37/check-locale-changelog.ps1`。
  - 验收结果：**14/14 OK**，每个文件 `+10/-0`（纯插入，**删除数 0**），首个 `## [` 版本都是 `3.17.0`，无 BOM、无 U+FFFD、无 CRLF。
  - `ch` 的小节标题是 `### 改了什么`，其余 13 个沿用 `### What changed`；`ch` 日期 `2026 年 11 月`，其余 `November 2026`。

---

## 3. 关键落点（file:line）

### 3.1 Rust —— 云盘索引（W37-A）

| 文件 | 位置 | 内容 |
|---|---|---|
| `src-tauri/src/global_search/remote_index.rs` | 新建，390 行 | 独立 tantivy 索引；`REMOTE_INDEX_MAX_DEPTH=8`、`REMOTE_INDEX_MAX_ENTRIES=20_000`；纯函数 `walk_should_continue` |
| 同上 | `:85` `collect_remote_entries` | async 遍历；单连接失败隔离；截断必 `log::warn!` |
| 同上 | `:216` `write_remote_index` | **先删目录再建** ⇒ 取消勾选的连接从索引里消失 |
| 同上 | `:250` `open_remote_reader` | 无索引目录 / schema 不符 ⇒ 返回 `None`（不是错误） |
| `src-tauri/src/global_search/commands.rs` | `:83` | 新命令 `global_search_index_remote_connections(app, connection_ids) -> Result<u64, String>` |
| `src-tauri/src/global_search/query.rs` | `:276` `search_remote_index` | 读云盘索引 |
| 同上 | `:313` `merge_search_results` | 按 path 去重保留高分；同分本地优先；稳定排序；截断到 limit |
| `src-tauri/src/lib.rs` | `:525` | 命令注册 |
| `src-tauri/src/remote/connection.rs` | `:110` | `pub include_in_global_search: bool`（`#[serde(default)]` ⇒ 老文件缺键即 false） |
| `src-tauri/permissions/app-commands.toml` | — | 放行新命令 |

**零 `tauri::` 引用**：`remote_index.rs` 不碰 `tauri::AppHandle` / `tauri::async_runtime`，否则 `cargo test --lib` 会在**加载期** `STATUS_ENTRYPOINT_NOT_FOUND` 崩掉。

越界但纯机械的改动：9 处 `#[cfg(test)]` 结构体字面量补 `include_in_global_search: false`（`registry.rs` 4 处、`aliyun/baidu/quark/ftp/ftps_provider.rs` 各 1 处），**生产逻辑零改动**。

### 3.2 前端 —— 收藏显示名（W37-B）

| 文件 | 内容 |
|---|---|
| `src/types/user-stats.ts` | `FavoriteItem.alias?: string` |
| `src/stores/storage/user-stats.ts:345` | `setFavoriteAlias(path, alias)`；空串=清空；走现成 `favorites` 整份写盘，**无新存储键** |
| 同上 | `normalizeFavorites` 只在 `typeof record.alias === 'string'` 时保留（老记录**不补键**） |
| `src/utils/favorite-display-name.ts:22` | `resolveFavoriteDisplayName(path, alias, translate)` 纯函数，回退链 **alias → getPathDisplayName → path** |
| `src/modules/dashboard/components/dashboard-entry-actions-menu.vue` | 加「编辑显示名称」项（prop `showRenameFavorite` + emit `rename-favorite`） |
| `src/modules/dashboard/components/dashboard-name-dialog.vue` | 加 4 个可选 prop，复用为别名编辑器 |
| `src/modules/dashboard/components/dashboard-new-favorite-dialog.vue` | 新增「显示名称（可选）」输入 |
| `src/modules/navigator/components/file-browser/file-browser-favorite-category-dialog.vue` | 别名输入**仅 `entryNames.length === 1` 时显示**（多选不显示） |
| `src/modules/dashboard/components/entry-card.vue` / `src/modules/nav-sidebar/components/quick-access-entry.vue` / `quick-access-panel.vue` / `dashboard.vue` | 渲染接入别名 |

### 3.3 前端 —— 连接表单（W37-C）

| 文件 | 内容 |
|---|---|
| `src/types/remote.ts:91` | `RemoteConnection.includeInGlobalSearch?` |
| `src/types/remote.ts:280` | `resolveRemoteConnectionNameDefault(protocol, existingName, translate)` |
| `src/types/remote.ts:302` | `getGlobalSearchConnectionIds(connections)` |
| `src/stores/storage/remote-connections.ts:144` | `normalizeConnectionRecord()`（缺键 ⇒ false） |
| 同上 `:172` | computed `globalSearchConnections` |
| 同上 `:610` | `saveConnection` 落盘 `includeInGlobalSearch: form.includeInGlobalSearch === true` |
| `src/modules/settings/ui/categories/remote/netdisk-add-dialog.vue` | 预填 provider 名 + 复选框（`data-test="field-include-in-search"`） |
| `src/modules/settings/ui/categories/remote/connection-form.vue` | 编辑态**不覆盖**已有名字；勾选回填；reset 重置 |
| `src/modules/settings/ui/categories/remote/connections-section.vue` | 勾选集合变化 ⇒ **串行**触发重建（见 §5.4） |
| `src/modules/settings/ui/categories/search/global-search.vue` | 新增「云盘检索」区块：空态 / 重建全部 / 每条连接单独重建 / 条目数 / 配额提示 |

---

## 4. 跨端契约（改任何一边都必须同步改另一边）

| 项 | 值 |
|---|---|
| Tauri 命令名 | `global_search_index_remote_connections` |
| JS 调用 | `invoke<number>('global_search_index_remote_connections', { connectionIds: string[] })` |
| 返回 | 写入索引的条目总数（u64） |
| **空数组语义** | **清空索引并返回 0，不报错** |
| 字段（Rust） | `RemoteConnection.include_in_global_search`（`#[serde(default)]`） |
| 字段（JSON/TS） | `includeInGlobalSearch` |
| 云盘索引目录 | `<appData>/remote-index/` |
| 本地索引目录 | `<appData>/global-search/index/`（**不得混**） |
| 远端路径 | `sfm://remote/<connectionId>/...`；`RemoteEntry.path` 由 provider 的 `build_remote_path` 产出完整路径 |

---

## 5. 本轮踩过的坑 / 关键决策

### 5.1 架构裁决：云盘索引必须独立
把远程 id 混进 `GlobalSearchSettings.drive_roots` 会被 `scan.rs` 的存在性过滤丢弃并破坏增量判定 ⇒ 退化成 22 分钟全量重扫；且远程 provider 没有 watch 能力。**已否决**，改为独立索引目录。

### 5.2 取消勾选 = 从索引删除
重建按**当前勾选集合整体重建**（先删目录再建），不是增量删除。

### 5.3 索引只在显式触发时重建
连接保存后勾选集合变化时 + 全局检索设置页「重建云盘索引」按钮；**不做周期性遍历**。

### 5.4 连接页自动重建必须串行（根会话自查修的 bug）
后端「按集合整体重建」是**先删索引目录再重写**，两次并发重建互相踩可能留下**半个索引**。已改为串行 + 合并：`remoteIndexRebuildRunning` 标志 + `pendingRemoteIndexIds` 记录最新集合 + `drainRemoteIndexRebuild()` 递归 drain（用递归避开 `no-await-in-loop`；对象字面量写成多行避开 `@stylistic/object-property-newline`）。

### 5.5 语言包 / CHANGELOG 由根会话统一改
Worker 禁止碰 `src/localization/messages/*.json`、`CHANGELOG.md`、`public/**`、`docs/**`，避免冲突。

### 5.6 后台 worker 的教训
派「写 14 本地化 changelog」的 worker 跑了很久却**一个文件都没改**（git status 只有根 CHANGELOG.md 被改）⇒ 根会话**直接停掉它自己接管**，用 node 脚本一次性写完 14 个。**派这类批量写入 worker 必须盯 git status，不能等它自报完成。**

### 5.7 后台打包必须自己带 Rust 工具链
`npm run tauri:build` 在后台 shell 里报 `failed to run 'cargo metadata' ... program not found`。打包脚本开头必须加：

```powershell
$env:RUSTUP_TOOLCHAIN='1.90.0'
$env:PATH='C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;C:\Users\Duanyi\.cargo\bin;' + $env:PATH
```

### 5.8 后台构建不许挂 stdout 管道
用 `*> "log.txt"` 重定向，**不要** `| Tee-Object` / `| Out-File`（后者会占住文件句柄，导致别的进程读日志报 "being used by another process"）。

---

## 6. 真机验收

- 产物：`npm run tauri:build` ⇒ `src-tauri/target/release/alpha-file-manager.exe`（末步只复制到 `release/`，**不上传**）。
- 验收脚本：`.superpowers/sdd/b37/b37-accept.ps1`（隔离桌面 `CreateDesktop` + `STARTUPINFO.lpDesktop`；点击用 `PostMessage` WM_LBUTTONDOWN/UP 注入到 Chromium render host ⇒ **鼠标等价**；断言只读 UIA 树）。
- 步骤文件：`.superpowers/sdd/b37/steps-*.json`（UTF-8；中文标签只放这里，`.ps1` 必须是纯 ASCII）。

- 结论：**见 §10「真机验收结果」**（pass3 5 PASS / 0 FAIL，pass4d 13 PASS / 0 FAIL，两轮全绿）。

---

## 7. 未验证 / 风险

1. **云盘索引的真实数据检索**依赖百度/夸克/阿里云盘上游 API 可用性。批次 35 末尾阿里云盘曾返回 HTTP 400；若仍坏，相关验收项按 handoff §9 第 9 条流程记 **UNDETERMINED**（先看截图报错文案 → 按 exe 路径精查防火墙 → 看远程缩略图缓存 mtime），**不得**记 PASS/FAIL。
2. **取消勾选后索引条目确实消失**、**多连接混合检索的排序**这两条目前只有单测覆盖，真机未跑（需要真实网盘凭据）。
3. 批次 35 遗留项（阿里云盘上游 HTTP 400）**本批次未触碰**。

---

## 8. 下一步（接力后按这个顺序）

> 批次 37 的实现、打包、真机验收（pass3 + pass4d）**已全部完成**，结论见 §10。下面是仍可继续的项。

1. **需要真实网盘凭据的验收**（§10.4 第 1/2/3 条）：拿真实百度 / 夸克 / 阿里账号登录，走完
   「新建网盘 → 勾「加入全局检索」→ 重建云盘索引 → 全局检索里搜到网盘文件 → 取消勾选后条目消失 → 多网盘混合排序」。
2. **可复用的验收脚本**：`b37-accept.ps1` 已加 `waitReady` / `clickSidebarIndex` / `clickActionMenu` /
   `assertNamePresent` 等算子（见 §10.3）；以后验收新界面照这个模式加步骤 JSON 即可，不必再写一次性脚本。
3. **发布前**：再跑一次 `npm run check`（ts + lint + unit + verify-changelog + verify:sandbox）；
   若改过前端，务必 `npm run tauri:build` 后再跑沙箱验收（dev 界面不作数）。
4. **别碰**：J1_*/J2_* 阈值、受保护常量（mDNS 名、legacy 数据目录名、Tauri identifier）。

---

## 9. 常用命令（都已验证可用）

```powershell
$root='F:/soft/00selfmade/filemanager/sigma-file-manager'

# 前端门禁
npm --prefix $root run ts
npm --prefix $root run lint:check
npm --prefix $root run test:unit:run
npm --prefix $root run verify-changelog

# 单个测试文件
node "$root/scripts/vitest-run.mjs" run src/utils/__tests__/favorite-display-name.test.ts

# Rust（绝不用 Set-Location）
$env:RUSTUP_TOOLCHAIN='1.90.0'
$env:PATH='C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;C:\Users\Duanyi\.cargo\bin;' + $env:PATH
cargo test --manifest-path "$root/src-tauri/Cargo.toml" --lib

# 语言包 / changelog 脚本
node "$root/.superpowers/sdd/b37/i18n-align-check.cjs"
node "$root/.superpowers/sdd/b37/i18n-changelog-317.mjs" $root
powershell -NoProfile -ExecutionPolicy Bypass -File "$root/.superpowers/sdd/b37/check-locale-changelog.ps1"

# 打包（真机验收唯一合法产物）
npm --prefix $root run tauri:build
```

---

## 10. 真机验收结果（2026-11-22）

**被测产物**：`src-tauri/target/release/alpha-file-manager.exe`（85,265,920 B，mtime 2026-11-22 00:53:20）；
安装包 `release/Alpha File Manager_3.17.0_x64-setup.exe`（46,004,374 B，与 `src-tauri/target/release/bundle/nsis/` 下同名产物同尺寸，已核对一致）。

**沙箱形态**：`CreateDesktop` + `STARTUPINFO.lpDesktop` 独立桌面跑 release exe；点击 = `PostMessage` WM_LBUTTONDOWN/UP 注入 Chromium render host（鼠标等价，不碰操作员桌面）；断言只读 UIA 树；每轮结束对 appdata 做「备份 → 非破坏式覆盖恢复」。

### 10.1 pass3：云盘检索 / 网盘名称预填 / 加入全局检索（5 PASS / 0 FAIL）

步骤 `.superpowers/sdd/b37/steps-pass3.json`，日志 `.superpowers/sdd/b37/accept-pass3.log`，dump `accept/uia-dump-0.txt`、`accept/uia-dump-remote.txt`、`accept/uia-dump-aliyun.txt`。

| 判据 | 结果 |
| --- | --- |
| 设置→搜索页出现「云盘检索」区块（说明 + 空态 + 重建提示） | PASS |
| 空态文案「暂无已加入检索的网盘」 | PASS |
| 远程→阿里云盘：名称输入框**已预填**「阿里云盘」 | PASS |
| 「加入全局检索」复选框存在且**默认 Off**（不替用户决定） | PASS |
| 操作员桌面前景窗口前后一致（沙箱没抢焦点） | PASS |

### 10.2 pass4d：收藏显示名（别名）端到端（13 PASS / 0 FAIL）

步骤 `.superpowers/sdd/b37/steps-pass4d.json`，日志 `.superpowers/sdd/b37/accept-pass4d.log`，**证据 = UIA dump** `accept4/uia-dump-p4d-*.txt`（同目录的 PNG 是空白帧，见 §10.3）。

判据链：

1. 仪表盘 →「收藏」页签 → 操作栏 ⋮ →「新建收藏」⇒ 弹窗出现「**显示名称**」标签 + 输入框（UIA 里 Edit 的 Name 即「显示名称」，来自 `label[for]` + `input[id]` 关联）；
2. 该输入框**初始为空**（留空 = 用原始名称）；
3. 填路径 `F:/soft/00selfmade/filemanager/sigma-file-manager` + 别名 `AFM别名验收` → 点「添加」；
4. 收藏卡无障碍名变为 `AFM别名验收 — F:/soft/00selfmade/filemanager 文件夹 刚刚 …`
   ⇒ **显示名被替换，底层目录名/路径一字未动**（卡片副标题仍是原路径）；
5. appdata 还原后复查：json 里搜不到 `AFM别名验收` ⇒ 验收**没污染真实数据**。

### 10.3 本轮踩过的坑（脚本已改，别再犯）

- **沙箱截图（`PrintWindow`）是空白帧，不能当证据**：pass4d 的两张 PNG 字节数完全相同、sha256 一致、内容全白 —— WebView2 的 DXGI 表面不经 `PrintWindow` 合成（沙箱桌面里同理）。**本批次真机证据一律以 UIA dump 为准**（`accept*/uia-dump-*.txt`，里面是真实无障碍名），PNG 只能算「跑到了这一步」的旁证，别拿它下结论。
- **断言只扫 `ControlType.Text` 会假红**：收藏卡标题并入了父级 Button 的 Name（`AFM别名验收 — …`），Text 节点里根本没有。已加 `assertNamePresent`（全控件 Name 通配扫描）。**断言红了先分清「功能没生效」还是「断言挂错控件」——后者要改判据，不是改产品。**
- **别硬编码屏幕坐标**：连续两轮之间 app 窗口宽度从 3840 变成 2560，硬编码点全偏。已改成从 UIA 树动态定位：`clickSidebarIndex`（导航栏图标按钮按列筛 x<90 再按 y 排序取第 N 个）、`clickActionMenu`（页签行之下、排除侧栏列的最靠上无名 Button = 操作栏 ⋮）。
- **webview 窗口先于 Vue 挂载**：此时 UIA 树是空的，脚本会误判「侧栏没渲染」。已加 `waitReady`（等 Button 数 ≥ 阈值再开跑）。
- **appdata 备份目录被 WebView2 缓存锁住**：`Remove-Item -Recurse` 报 "being used by another process"，直接把脚本打断（exit 非 0）。已改成按时间戳新建备份目录 + 复制前先停进程 + `Copy-Item -ErrorAction SilentlyContinue`。
- 新增算子：`waitReady` / `clickSidebarIndex` / `clickActionMenu` / `clickAt` / `url` / `assertUrl` / `assertEditPresent` / `setEditValue` / `assertNamePresent` / `wait`（`.ps1` 仍是纯 ASCII，中文只放步骤 JSON）。

### 10.4 仍未判定（**不得**记 PASS / FAIL）

1. **百度 / 夸克网盘的名称预填**：这两家走 `startAdd()` → 内嵌登录窗 → **登录成功后才弹录入表单**，需要真实网盘凭据，真机无法凭空验证。阿里云盘走 `connection-form.vue`（不需要登录），已等价验证预填 + 复选框逻辑。
2. **云盘索引的真实数据检索**：依赖上游网盘 API（批次 35 末尾阿里云盘曾 HTTP 400，本批次未触碰），按 §7 第 1 条流程记 **UNDETERMINED**。
3. **取消勾选后索引条目确实消失**、**多连接混合检索排序**：目前只有单测覆盖，真机未跑（需真实凭据）。

---

## 11. 提交前门禁复跑结论（2026-11-22 13:52）

`npm run check`（`CI=true`）全绿，日志 `.superpowers/sdd/b37/gates3/check.log`：

| 门禁 | 结果 |
| --- | --- |
| `ts`（`vue-tsc --build`） | 0 error |
| `lint:check`（eslint + stylelint，均 `--max-warnings 0`） | 0 problem |
| `test:unit:run` | **329 files / 3547 tests 全通过**（Duration 102.51s） |
| `verify-changelog` | OK，`[3.17.0] - November 2026` 在最前，共 61 条目 |
| `verify:sandbox` | 2 个扩展入口、20 条动态模式、**0 violation** |

- 用例数由批次 36 基线 3546 增至 **3547**（+1，来自新增的 `src/utils/__tests__/favorite-display-name.test.ts`），既有用例零删改。
- 提交前另核对：`J1_|J2_` 阈值 grep 命中 **0**；`package.json` 版本 **3.17.0**；
  `src-tauri/target/release/alpha-file-manager.exe`（85,265,920 B，mtime `2026-11-22T00:53:20`）
  的 mtime 晚于 `src/`、`src-shared/`、`src-tauri/src` 下**全部**源码 ⇒ 复用该产物，**不需要重跑 `tauri build`**。
- `AGENTS.md` 已新增 **§2.11 沙箱真机验收的五个硬事实**，并显式**修正** §2.8 里「`PrintWindow` 在沙箱照样能出图」的旧口径（窗口框可出、内容区可能全白）。
- 工作区状态：`HEAD = 0526669f`，批次 37 改动 **81 项**（73 改 + 8 新增，0 删除），已合并为批次 37 **单一 commit**（`git log --oneline --grep=batch37` 可见，含本节）。
