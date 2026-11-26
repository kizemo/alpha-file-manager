# 交接文档 · 2026-11-08 · 3.13.0 发版（位置页分区改造 + 百度网盘接入收口）

> 本文是**本轮唯一交接入口**。§22 之前的内容按需回查 `docs/handoff-2026-11-08-baidu-netdisk-phase1.md`。
> 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`；分支：`feat/baidu-remote-provider` → 已快进合并到 `main`。

---

## §1 本轮目标（用户 2026-11-08 的三条反馈）

1. **「位置」页分区改造**：像主页一样分区 —— 最上面「分区」（盘符），下面依次「网盘」「远程连接」；未配置则留空且**不显示标题**。
2. **合并发布**：当前分支合并到 `main`，commit + push；更新 README、CHANGELOG、aiec 版本信息。
3. **百度网盘登录自动化深度分析**：系统浏览器/内嵌 webview 登录 → AFM 读取登录态自动填入。

用户随后追加指令：**「请先修改好位置页面的问题，再打包发版」** —— 位置页未改好前不得打包发版。

---

## §2 结论摘要

| 反馈 | 状态 | 落点 |
| --- | --- | --- |
| 1 位置页分区 | ✅ 已实现 + 真机验收通过 | `src/utils/remote-location-refs.ts`、`file-browser-entry-groups.ts`、`file-browser-virtual-rows.ts` |
| 2 合并发布 | ✅ 已完成（commit / merge / push / 站点 / GitHub） | 见 §5 |
| 3 百度网盘登录自动化 | ✅ 只出分析报告，**未改认证代码** | `docs/2026-11-08-baidu-login-auto-fill-feasibility.md` |

---

## §3 需求 1 的实现要点（改了什么、为什么这么改）

### 3.1 口径

- 「位置」页 = 导航器虚拟目录 `sfm://locations`，**不另造视图**，扩展既有 grid/list 分区机制。
- 归类口径唯一：`isBaiduPanProtocol` ⇒ 协议属网盘的连接进「网盘」分区，其余进「远程连接」分区。
- 分区键类型：`LocationGroup = 'drives' | 'netdisk' | 'remote'`（`src/types/dir-entry.ts`）。

### 3.2 关键文件

| 文件 | 作用 |
| --- | --- |
| `src/types/dir-entry.ts` | 新增 `LocationGroup` 类型与 `DirEntry.location_group?` 字段 |
| `src/utils/remote-location-refs.ts` | **模块级注册表**：`setRemoteLocationRefs` / `getRemoteLocationRefs` / `subscribeRemoteLocationRefs` / `buildRefsFingerprint`；带 `protocol` 字段；内容指纹不变则不通知监听者；监听者异常被隔离（`console.error`） |
| `src/utils/virtual-locations.ts` | `getRemoteLocationEntries()` 按协议标 `netdisk`/`remote`；`buildLocationsDirectoryFromDrives` 给盘符标 `drives` |
| `src/modules/navigator/components/file-browser/file-browser-entry-groups.ts` | `FileBrowserLocationGroups` / `hasLocationGroupEntries()` / `groupFileBrowserLocationEntries()` / `getFileBrowserSectionEntryOrder()` |
| `.../utils/file-browser-virtual-rows.ts` | `FileBrowserSectionKey` 扩三区；`GRID_SECTION_STICKY_INDEXES` 增 14/15/16；`createSectionBuilders` 构造三个位置分区并 **`filter(section => section.entries.length > 0)`** ⇒ 空分区连标题都不出现 |
| `.../file-browser-grid-view.vue` / `file-browser-list-view.vue` | 图标走 `SECTION_ICONS` + `<component :is>`；`getSectionLabel` 映射 `drives→fileBrowser.partitions`、`netdisk→settings.remote.nav.netdisk`、`remote→settings.remote.nav.remoteSection` |
| `.../composables/use-file-browser-navigation.ts` | import `subscribeRemoteLocationRefs`；在 `watch(sharedDrives, ...)` 之后注册订阅回调 —— 仅当 `isVirtualLocationPath(currentPath.value) && dirContents.value` 时 `void silentRefresh()`；`onUnmounted` 内退订 |

### 3.3 关键设计决策

- **刷新用订阅而非轮询**。远程连接条目由 store 侧 `src/stores/storage/remote-connections.ts:67`（`watch(connections, …, { immediate: true })`）反向写入注册表 —— `src/utils/**` 是纯工具模块，**不得 import `@/stores/**`**（实测会拖坏 8 条 drag-session 用例）。
- 所有增删改都走 `connections.value = ...` 重新赋值 ⇒ watch 必然触发。
- 注册表内容指纹（path/name/protocol 三元组）不变 ⇒ 不通知 ⇒ 不触发无意义的目录重读。
- i18n：17 个语言包新增 `fileBrowser.partitions`（脚本 `.superpowers/sdd/i18n-locations-partitions.mjs`，`WRITTEN=17 PROBLEMS=0`）。

### 3.4 「位置」入口 UI 真相

`src/modules/navigator/components/file-browser/file-browser-toolbar-nav-buttons.vue` 第 158–171 行：`HomeIcon` 按钮，`data-test="go-locations-button"`，`@click="emit('goHome')` → 导航到 `sfm://locations`，tooltip = `t('locations')` = 「位置」。

---

## §4 测试与门禁

### 4.1 新增/追加测试

- 新建 `src/modules/navigator/components/file-browser/__tests__/file-browser-location-groups.test.ts`（10 例）。
- 新建 `src/utils/__tests__/remote-location-refs.test.ts`（6 例：变化通知 / 内容不变不通知 / 改名算变化 / 退订 / 监听者抛错隔离 / 非法 id 跳过）。
- 追加 `src/utils/__tests__/virtual-locations.test.ts` 2 例 + `afterEach(() => setRemoteLocationRefs([]))`。

### 4.2 门禁（本轮最终结果）

```bash
npx vue-tsc --build    # exit 0
npx vitest run         # 297 files / 2797 tests 全通过
npx eslint .           # 177 errors / 0 warnings（与基线一致；exit 1 是存量正常码）
node scripts/verify-changelog.mjs   # exit 0（49 条目）
```

### 4.3 变异验证

`.superpowers/sdd/mutate-remote-refs-notify.cjs`（apply/restore 模式，**用备份副本还原，不能 `git checkout`**）：
打掉「内容没变就不通知」判定 → 6 条测试全红；restore 后文件完整（`MARKER=true SUB=true PROTO=true FFFD=0`）。

> ⚠️ 本轮踩过：曾误用 `git checkout -- src/utils/remote-location-refs.ts`，把该文件未提交的新改动（`protocol` 字段与订阅机制）全部抹掉，已按完整内容重写恢复。**变异还原一律用备份副本。**

---

## §5 发版记录（3.13.0）

### 5.1 版本与文档

- 版本 3.12.2 → **3.13.0**：`package.json` + 3 个 Cargo 清单（`npm run sync-version`）+ `src-tauri/Cargo.lock`。
- `CHANGELOG.md` 顶部新增 `## [3.13.0] - November 2026`；`public/changelog/` 15 locale 已插入（`.superpowers/sdd/release-3130.mjs`，BOM/U+FFFD 校验通过）。
- `README.md` 新增 `### What changed in 3.13.0`；`README.zh-CN.md` 新增 `### 3.13.0 改了什么`。
- 站点发布说明：`.superpowers/sdd/release-notes-3.13.0.md` → `prepare --write-notes` 生成 `E:\办公文件\L1网站\alpha-file-manager\_release-notes-v3.13.0.md`。

### 5.2 打包与安装

- `.superpowers/sdd/build-3130.cmd`（纯 ASCII，vcvars64 + Rust PATH + `> build-3130.log 2>&1`）→ **`BUILD_EXIT=0`**。
- 产物：`src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.0_x64-setup.exe`（45,370,188 B / 43.27 MiB），已复制到 `release\`。
- `.superpowers/sdd/install-3130.cmd`（`call "<exe>" /S`）→ **`INSTALL_EXIT=0`**；`D:\Program Files\Alpha File Manager\alpha-file-manager.exe` ProductVersion=**3.13.0**。

### 5.3 Git

| 提交 | 内容 |
| --- | --- |
| `d1bb55b8` | feat(locations): 「位置」页按分区/网盘/远程连接分区展示，空分区不显示标题（28 文件） |
| `11b11762` | docs(baidu): 补百度网盘登录自动化可行性分析报告 |
| `8e7bf810` | chore(release): 3.13.0 —— 版本号、更新日志与 README（22 文件） |

`main` 由 `6eabbc67` 快进到 `8e7bf810`，已 `git push origin main`；特性分支也已推送。

### 5.4 真机验收

脚本 `.superpowers/sdd/accept-locations.ps1`（自校准：先点 rail 找到含「网格」按钮的工具栏，以其 rect 为锚点向左扫导航按钮，命中「分区」即停）。

结果（真实用户数据）：

```
TOOLBAR_READY rail=129
ANCHOR_GRID=585,54 37x36
SWEEP dx=-61 at 524,76 PARTITION=True NETDISK=True REMOTECONN=True
HEADER_PARTITION hits=1 / HEADER_NETDISK hits=1 / HEADER_REMOTECONN hits=1 / HEADER_REMOTE hits=0
VERDICT=LOCATIONS_FOUND
```

截图 `.superpowers/sdd/accept-3130/shot-locations.png` 人工查看：**分区 5 / 网盘 1（百度网盘）/ 远程连接 3（速豹堂、aiec.fun、jury.xin）**，页签名 `位置`、路径 `sfm://locations`、共 9 个项目 —— 与用户要求一致。

### 5.5 站点与 GitHub

- `release:prepare --bump none --skip-build --notes-file .superpowers/sdd/release-notes-3.13.0.md` → 站点 `version.json` 升到 3.13.0，旧版 3.12.2 已归档进 `history[]`，安装包已复制到站点源目录（sha256 一致）。
- `release:prepare --write-notes .superpowers/sdd/release-notes-3.13.0.md` → 生成 `_release-notes-v3.13.0.md`（带构建信息，供 GitHub 正文读取）。
- `release:upload` → SFTP 全量上传 **exit 0**（`_upload_alpha_files.py` 会把站点目录里**全部** .exe 重传一遍，约 1.5 GB，耗时约 20 分钟属正常），随后自动公网回读校验通过。
- `release:github` → GitHub 兜底 Release 已创建。

### 5.6 对外发布结果（已回读确认）

**站点**（`https://www.aiec.fun/alpha-file-manager/version.json`，带 cache-busting 回读）：

| 项 | 值 |
| --- | --- |
| 线上版本 | `3.13.0` |
| 发布日期 | `2026-11-08T14:02:54Z` |
| 清单 sha256 | `21c8bef950663b76f9bd78ef88780f6d0ecb21373874b4eb98a721b82c9fc1c4` |
| 下载直链 | `https://www.aiec.fun/alpha-file-manager/Alpha_File_Manager_3.13.0_x64-setup.exe` |
| 源站 / 边缘包大小 | 45,370,188 字节 / 45,370,188 字节（`CF-Cache-Status=MISS`） |
| 回读结论 | 版本号 / 大小 / sha256 **三项均一致**（源站与边缘都已生效） |

站点 `index.html` **无硬编码版本号与安装包文件名**（实测 `EXE_REFS=[]`、`VER_LITERALS=[]`）⇒ 下载入口读 `version.json`，站点页无需改。

**GitHub**：

| 项 | 值 |
| --- | --- |
| Release | `https://github.com/kizemo/alpha-file-manager/releases/tag/v3.13.0` |
| tag 指向 | `f6f8f865`（= 本地 `main` HEAD） |
| 草稿 / 预发布 | `isDraft=false` / `isPrerelease=false`（会作为 Latest release） |
| 发布时间 | `2026-11-08T14:13:45Z` |
| 资产 | `Alpha_File_Manager_3.13.0_x64-setup.exe`，45,370,188 字节，digest sha256 与清单一致 |
| 正文 | 已写入中文发布说明（非空，长度正常） |

> ⚠️ **`release:prepare` 默认 `--bump patch` 且默认重新打包**。本轮首次误跑把 3.13.0 顶成 **3.13.1** 并触发重复构建，已 `git checkout -- package.json src-tauri/**` 还原版本号并停掉任务。
> ⇒ 固定写法：`node scripts/release-publish.mjs prepare --bump none --skip-build --notes-file <说明文件>`。

---

## §6 需求 3 · 百度网盘登录自动化结论（摘要）

完整报告：`docs/2026-11-08-baidu-login-auto-fill-feasibility.md`（32,837 B，UTF-8 无 BOM）。

- **方案 a（内嵌 WebView2 登录 + Rust `cookies_for_url()`）可行，且仓库已实测**：`src-tauri/src/baidu_probe.rs` 读出 HttpOnly `BDUSS`（192 字符）+ `STOKEN`，第二次可零输入。缺的只是产品化：`remote_baidu_begin_login` + 轮询 + 事件回传 + DPAPI 落盘，**零新增依赖**。
- **方案 b（Chrome 127+ ABE）基本不可行**；方案 d（`unsupported_grant_type`）、方案 f 不可行；方案 c（自建 OAuth）有条件可行。
- **旧结论被推翻**：`250528` / `266719` 只是内部 app_id，不是可用的公开 client_id。
- **红线**：不得挪用第三方 `client_id/secret`；cookie 明文不进日志/错误/前端/URL。

---

## §7 本轮工具坑（勿重试）

1. `cmd /c start /wait "" "<exe>" /S` → `Invalid switch - "/S"`（PowerShell 吞掉空标题）⇒ 用 `.cmd` 内 `call "<exe>" /S`。
2. `AutomationElement::FindFirst(...)` **静态方法在本机不可用** ⇒ 必须用 `$root.FindFirst(TreeScope, cond)` 实例方法。
3. UIA `InvokePattern.Invoke()` 对 left rail 按钮**静默无效** ⇒ 必须 `SetCursorPos` + `mouse_event` 真实点击。
4. `git checkout -- <file>` 会抹掉该文件未提交的新改动 ⇒ 变异还原用备份副本。
5. bash 安全策略：`cd` 之后出现写操作（New-Item）被拦截 ⇒ 写操作放在 `Set-Location` 之前。
6. `PrintWindow` 不渲染 hover/overlay 层；`TreeScope::Descendants + TrueCondition` 全扫在本应用部分区域不可用（rail 图标无名字）。
7. `release:prepare` 默认会 bump 版本号并重新打包 —— 见 §5.5。

---

## §8 仍未闭环 / 下一步可做

| 项 | 状态 | 说明 |
| --- | --- | --- |
| 隔离配置下「未配置网盘/远程时不显示标题」的**真机**取证 | ⚠️ 未做（**不影响结论**） | 代码与单测已覆盖：`file-browser-location-groups.test.ts` 的 `renders no section header when only the drives group exists` 断言 `sections.length===1` 且 `headerRowIndex===-1` —— 空分区连段头行都不生成。真机侧用全新 APPDATA 启动时应用停在初始态，脚本找不到文件浏览器工具栏锚点（`TOOLBAR_MISSING` / `NO_TOOLBAR_ANCHOR` exit 3）。可改用地址栏 `Ctrl+L` → `sfm://locations` 进入后重跑 |
| 需求 3 产品化（自动填 cookie） | 未开始 | 报告已给出可落地的 4 步接线方案，**本轮明确不改认证代码** |
| `extensions/kizemo.focus-sync/sidecar/src/bin/` | 未跟踪 | 与本轮无关的既有未跟踪目录，**故意不提交** |
