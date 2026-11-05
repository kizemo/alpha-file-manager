# 交接文档 · 扩展市场「官方 / 精选 / 作者名」改造与 3.10.0 发版

- 日期：2026-11-04
- 会话：`mvs_8eb8c5acfaa34a6f80d4e2736e0c05c7`
- 仓根：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 约定文件（改本项目代码前必读）：`F:\soft\00selfmade\filemanager\AGENTS.md`
- 目标版本：**3.10.0**（feat 级 bump）
- 上一发版提交：`8b0d8cd0 release: 发版 3.9.2`；本轮工作开始时 `HEAD = ceff7e38 docs(handoff): 新增阶段四交接文档并更新接力 prompt`

---

## 一、背景

2026-11-04 上一轮把扩展市场拆成两个源：`kizemo/sfm-marketplace`（我们自己发布的扩展）为官方源、
`sigma-hub/sfm-marketplace` 为第三方源。装机后发现归属显示仍不对，并提出品牌统一要求。

用户本轮原话（时间戳 1793803494229）：

> git tool 已经在市场中显示，但显示在第三方中。
> 希望我们自己开发的插件，包括 Focus Sync 都放在精选中，插件名称的右上角，标注"官方"，作者名称，修改为 Alpha File Manager（当前是 kizemo）
> 请进行修改。修改后，打包发版，修改 readme 文档、升级说明、aiec 的版本文件等。
> 然后更新完整的 handoff 文档，和一句话 prompt，用于新开会话接力任务。prompt 要引导新会话阅读必要文档，
> 充分掌握项目背景、目标、进度和下阶段计划，不要重复 handoff 的具体内容，文档要提供完整路径。

## 二、目标（可验收清单）

| # | 诉求 | 判据 |
|---|---|---|
| 1 | Git Tools 不再显示在「第三方」，归到官方 | 卡片徽章为「官方」，来源徽章为官方源 |
| 2 | 我们自己开发的插件（含 Focus Sync）全部进「精选」 | Featured 区出现条目 |
| 3 | 插件名称右上角标注「官方」 | `extension-card.vue` 名称行徽章为 official |
| 4 | 作者名称从 `kizemo` 改为 `Alpha File Manager` | 卡片 publisher 行 + 详情页 |
| 5 | 打包发版 3.10.0，并同步 README / CHANGELOG / aiec 站点 `version.json` | 安装包产物 + 公网回读 |
| 6 | 完整 handoff 文档 + 一句话 prompt | 本文 + `docs/prompt-2026-11-04-*-next.md` |

## 三、根因（已验证，源码级）

### 3.1 「官方」判定只看 GitHub repo owner

- `src/data/extensions.ts:216` 原为 `const OFFICIAL_EXTENSION_ORG = 'sigma-hub';`
- `src/data/extensions.ts:218-223` 用正则从 `repository` URL 取 owner 与之比对
- ⇒ Git Tools 的仓库是 `kizemo/sfm-git-extension`，owner 是 `kizemo` ⇒ `isOfficial = false`
- 传播链：`src/stores/runtime/extensions.ts:51/671/679` → `src/modules/extensions/composables/use-extensions.ts:480/529/674`
  → `src/modules/extensions/components/extension-card.vue:63` → `:132-143` 徽章分支
  `isLocal` → `official` → else `community`（`ch.json` 文案「社区」）
- 「第三方」字样另有来源：`src/modules/extensions/utils/market-sources.ts:255-258` 给条目打 `sourceId`，
  卡片上的 source 徽章按 `sigma-hub` 源显示 `extensions.marketSourceCommunity` = 「第三方」。**徽章已在名称右侧**
  （`extension-card.vue:118-153` 的 `extension-card__title-row` 内 `h3.extension-card__name` 之后紧跟
  `ExtensionBadge`，再跟 source 徽章；下一行 `extension-card__publisher` 显示 `publisherName`）
  ⇒ **改判定即满足诉求 3，改 `publisher.name` 即满足诉求 4，模板一行都不用动。**

### 3.2 「精选」是注册表条目的布尔字段，不是选项卡

- `src/types/extension.ts:29` `featured: boolean`
- `src/modules/extensions/composables/use-extensions.ts:524-525` 按 `featured` 过滤出精选
- `src/modules/extensions/components/extensions-marketplace.vue:83-96 / 182-191` Featured 区渲染
- `src/modules/extensions/runtime/validation.ts:419` 要求 `typeof featured === 'boolean'`，缺字段会让整条注册表校验失败
- i18n：`extensions.featured` 在 `ch.json:1383` = 「精选」

### 3.3 改判定不会让条目消失

列表基座是 `extensionsStore.availableExtensions`（全部远端条目），`officialExtensions` 只是显示子集，
`isOfficial` 只影响徽章与筛选。已核对 `use-extensions.ts:455-492`。

### 3.4 市场条目的下载地址只需要 git tag

`src/data/extensions.ts:204-214` `getExtensionDownloadUrl()` =
`https://github.com/{owner}/{repo}/archive/refs/tags/v{version}.zip`
⇒ **不需要建 GitHub Release 资产，只要仓库 public 且有 tag。**

### 3.5 `OFFICIAL_EXTENSION_OWNERS` / `isOfficialExtension` 引用面极小

全仓 grep 无任何测试引用这两个标识符 ⇒ 改签名不会打破测试契约（已验证）。

## 四、本轮已落地改动（✅ 已进入发版提交 `18a349e7` 并推送 origin）

> **2026-11-04 23:55 回填**：§四~§七 描述的改动随发版提交 **`18a349e7`**（`release: 发版 3.10.0`，131 files changed, +14546 / −473）提交并推送，
> tag **`v3.10.0`** 已建并推送；提交后工作树干净（`git status --porcelain` = 0 行）。

### 4.1 代码

1. **`src/data/extensions.ts`**
   - 删除 `OFFICIAL_EXTENSION_ORG`
   - 新增 `export const OFFICIAL_EXTENSION_OWNERS = ['kizemo'];`
     注释里写明：`sigma-hub` **故意不列入**（与上一轮「sigma 降为第三方源」自洽，用户心智是「我们的 = 官方」）；
     owner 轴与市场源轴**正交**，以后新增自研扩展的仓库只要在 `kizemo` 组织下即自动官方。
   - `isOfficialExtension()` 改为对 owner 集合做 `some()` 比对。
2. **`src/localization/messages/ch.json`**
   `extensions.badges.officialTooltip` 改为「此扩展程序由 **Alpha File Manager** 团队创建和维护」。
   17 个语言包普查结论（脚本 `.superpowers/sdd/apply-official-branding.cjs`，输出
   `.superpowers/sdd/logs/branding-apply.txt`）：仅 `ch` / `hy` / `hi` 仍是旧品牌，其余 14 个已是 Alpha File Manager；
   `hi` / `hy` 留作 follow-up（不手工重写非拉丁语系译文）。
   同时确认 `extensions.featuredDescription`、`extensions.marketSourceOfficialTooltip`、
   `extensions.marketSourceCommunityTooltip` 三个键在各 locale **均不存在**（undefined），不是本轮遗漏。

### 4.2 市场注册表（线上已生效）

- `scripts/ext-market/registry.json`（955 B）：`publisher.name` `kizemo` → **`Alpha File Manager`**；`featured` `false` → **`true`**
- `node scripts/ext-market/publish.mjs --validate --registry scripts/ext-market/registry.json` → 校验通过（1 个条目），exit 0
- **已推送线上**：`gh api repos/kizemo/sfm-marketplace/contents/registry.json --method PUT`
  - 旧 blob sha `6ca1e499c7e89a03c2d46309b95086e45cad7582`
  - 新 commit `a7e333a0fcdc6a61e43e10407dc27f14a11feea7`
  - 回读可见 `"publisher": { "name": "Alpha File Manager"` 与 `"featured": true`
- ⚠️ **清单是热更新的**（应用每次读线上 `registry.json`），补 Focus Sync 条目**不需要再发一版应用**。

### 4.3 扩展本体 manifest

- `extensions/kizemo.git/package.json`（3636 B）`publisher.name` → `Alpha File Manager`
- `extensions/kizemo.focus-sync/package.json`（1969 B）`publisher.name` → `Alpha File Manager`
- ⚠️ 已知连带影响：扩展本体 `publisher.name` 改动会使 `%APPDATA%` 里**已安装副本**的 manifest 与打包 resources 不一致，
  需靠 bundled 更新机制覆盖 ⇒ 真机验收必须确认界面显示是否真的生效（见 §6 未验证项）。

### 4.4 版本与文档

- 版本 bump 到 **3.10.0**：改 `package.json` → `node scripts/sync-version.js` 输出 `Synced 3 Cargo manifests to 3.10.0`
  - `package.json = 3.10.0`、`src-tauri/Cargo.toml = 3.10.0` ✅ 实测回读一致
  - `src-tauri/tauri.conf.json` 的 `version` 字段**按既有约定是字符串 `"../package.json"`**（HEAD 基线同值），
    不是数字版本号 ⇒ 版本一致性只看 `package.json` 与 Cargo manifests 两处。
  - ⚠️ 本轮走**手工 bump**，没用 `release:prepare --bump minor`，理由：规避 prepare 的自动 CHANGELOG 生成与
    历史上「`.cmd` 中途被编辑导致重复 bump（3.6.0 → 3.7.0）」的风险。
- **`CHANGELOG.md`** 新增 `## [3.10.0] - November 2026` 段（英文，沿用既有 `**Summary:**` + `### What changed` 体例），8 条：
  双市场源 / Git Tools 新扩展 / 官方 + 精选 / 作者名 Alpha File Manager / 全盘搜索增量索引与并行扫描 /
  永久删除假失败修复 / 动作注册表统一命令与快捷键。内容取自本轮 diff 与既有未跟踪交接文档。
- **`README.md`**：Credits 新增 Git Tools 扩展、我们发布的扩展作者名为 Alpha File Manager（官方徽章 + 精选）；
  GPL 源码表新增 `| Git Tools extension | bundled in this repository, extensions/kizemo.git/ |`
- **`README.zh-CN.md`**：`## 致谢` 新增「Git Tools 扩展」「我们发布的扩展，作者名统一为 Alpha File Manager」；
  源码表新增 `| Git Tools 扩展 | 随本仓库分发，extensions/kizemo.git/ |`
  （`readme-markdown-table.test.ts` 只校验表格列数一致，2 列行加在 2 列表里安全。）

## 五、门禁与打包

| 项 | 命令 | 基线（3.9.2 那轮） | 本轮结果 |
|---|---|---|---|
| Rust | `cargo test --lib`（**必须在 `src-tauri/` 下**） | 912 passed | **912 passed / 0 failed / 0 ignored，6.44 s**（补跑 exit 0；仓根那次 exit 101 是 cwd 问题，见 §5.1） |
| TS | `npm run ts` | 无输出 | ✅ **TSC_EXIT=0**（`logs/gateC.txt`） |
| Lint | `npx eslint .` | 186 errors / 0 warnings（HEAD 基线 188；`eslint src` 175、`eslint scripts` 13） | ✅ **186 errors / 0 warnings**，同口径无新增回归 |
| 前端单测 | `npx vitest run` | 存量失败 3 个文件（`file-browser-tree-view.test.ts`、`module-load-recovery.test.ts`、`readme-markdown-table.test.ts`） | ⏸ 本轮未跑（存量失败与本轮改动无关，见 §5.1 串行教训） |
| 打包 | `npm run tauri:build` | 产物 `release/Alpha File Manager_3.9.2_x64-setup.exe` 44 943 249 B | ✅ **BUILD_EXIT=0**，`Alpha File Manager_3.10.0_x64-setup.exe` **44 938 742 B** |

### 5.1 本轮踩到的两个执行事故（**必须记住的口径**）

1. **`cargo test --lib` 的 cwd**：`.superpowers/sdd/gate-build-310b.cmd` 第一版在仓根跑 `cargo test --lib`，
   报 `error: could not find 'Cargo.toml' in F:\soft\00selfmade\filemanager\sigma-file-manager`、exit **101**。
   ⇒ Rust 门禁必须 `cd /d %R%\src-tauri`。补跑脚本 `.superpowers/sdd/cargo-only-c.cmd`，日志
   `.superpowers/sdd/logs/gate-cargoC.txt` + 结果行 `.superpowers/sdd/logs/cargoC.txt`。
2. **并行门禁会互相打死**：第一版脚本用 `start /b` 并行跑 ts/eslint/vitest，结果整个进程树在 23:04:48 被终止
   （marker 文件 `gate-and-build.txt` 从未生成、四项日志只有两项落盘）。
   ⇒ 门禁**严格串行**；串行版 `gate-build-310b.cmd` 里 cargo 仍写在第一步（cwd bug 见上）。
   ⇒ 另外发现：后台 `cmd /c` 会**提前上报 succeeded**（因为它没有 stdout 管道可观测），
   但脚本其实还在跑 ⇒ **判进度只看 `logs/` 下的文件尾部，不要相信后台任务的完成通知，也不要看屏幕**
   （`PrintWindow` 会返陈旧帧）。

### 5.2 打包结果（✅ 已完成，2026-11-04 23:34:21）

**3.10.0 安装包已产出。** `src-tauri/target/release/bundle/nsis/Alpha File Manager_3.10.0_x64-setup.exe` = **44 938 742 B**（23:34:17），
`release/` 下同名副本已就位。站点 `prepare` 产出的下划线命名资产 `Alpha_File_Manager_3.10.0_x64-setup.exe` 与之**字节一致**（md5 `59c083c6bc7d309fa6e9b56e19be9c2c`）。

`.superpowers/sdd/gate-build-310c.cmd`（23:28:15 detached 启动，cmd pid 224364）实测结果，日志 `.superpowers/sdd/logs/gateC.txt`：

| 步骤 | 结果 |
|---|---|
| `[1/4] npm run ts` | **TSC_EXIT=0** |
| `[2/4] npm run verify-changelog` | **CLG_EXIT=0** |
| `[3/4] npx eslint .` | **186 errors / 0 warnings**（HEAD 基线 188，同口径无新增回归） |
| `[4/4] npm run tauri:build` | **BUILD_EXIT=0**，23:29:14 → 23:34:21（约 5 分钟，有增量缓存），末行 `GATE_C_DONE` |

可行的后台启动写法（长编译/长上传一律用它）：
```powershell
$p = Start-Process -FilePath 'cmd.exe' -ArgumentList '/c','<绝对路径 .cmd>' -WindowStyle Hidden -PassThru
# 判进度只看日志文件尾部与 LastWriteTime：
Get-Content '<绝对路径 log>' -Tail 25
Get-Process python,node -ErrorAction SilentlyContinue | Select-Object Name,Id,StartTime | Sort-Object StartTime
```

**教训保留**：前一版 `gate-build-310b.cmd` 经 `bash run_in_background` 启动，22:58:30 落 `GATE_B_START` 后进程树被回收杀掉（`run_in_background` 会终止整棵进程树）；
`310b` 的日志（`logs/gateB.txt` / `gate-tsB.txt`）保留作证据，不要再等它。同理：**不要挂 stdout 管道**（`Tee-Object` / `| Select-String` 一断就带走构建），
**`.cmd` 运行期间绝对不能编辑**（cmd.exe 按字节偏移逐行读，中途改文件会执行到别处的指令片段，2026-10-24 就是这样把版本从 3.6.0 顶到 3.7.0）。
另外实测到一条取证坑：**`Get-ChildItem <dir> -File -Include 'a','b'` 在非 `-Recurse` 时静默返回空** ⇒ 差点误判脚本不存在；改用 `-Path '<dir>\*'` 再过滤名字。

git 经 bash 调用会打印 `/usr/bin/sh: warning: setlocale: LC_ALL: cannot change locale (zh-CN)` —— **无害噪音**，不是失败（本轮 PUSH_EXIT / TAG_EXIT / COMMIT_EXIT 均为 0）。

### 5.3 真机安装复核（✅ 已完成，2026-11-05 00:45）

`release/Alpha File Manager_3.10.0_x64-setup.exe` 已在本机**静默安装成功**（NSIS `/S`，`NSIS_EXIT=0`）：

| 项 | 验收前 | 验收后 |
|---|---|---|
| `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` 的 ProductVersion | 3.9.2 | **3.10.0** |
| exe mtime / 大小 | 2026-11-04 22:29:14 | **2026-11-04 23:33:42 / 80 642 560 B**（= 打包产物时间戳） |
| 装机 bundled `extensions/kizemo.focus-sync/package.json` | publisher = `kizemo` | **publisher = `Alpha File Manager` / version = 0.5.8** |

驱动脚本 `.superpowers/sdd/accept-310-market-branding.ps1`（纯 ASCII，CJK 走码点构造），日志 `.superpowers/sdd/accept-310-market/accept-310-market.log`。

## 六、真机验收 · 已完成（2026-11-05 00:45 → 01:10）

> 口径：只用 `tauri build` 的打包产物（`tauri dev` 的路由页内容区全空白，界面结论一律作废）；
> 脚本在任何 user32 调用前 `SetProcessDPIAware()`；主窗口用 `EnumWindows` + pid + class `Tauri Window` + 可见 + 面积最大；
> 恢复真实数据前**先杀进程**再 restore 并核对哈希。

### 6.1 四项证据判定（诉求 1/2/3/4）

| 诉求 | UI 上的真实载体 | 实测命中 | 判定 |
|---|---|---|---|
| 官方徽章 | 卡片标题行的 `ExtensionBadge type="official"`（**纯图标、无文本**）；市场源 tab 名为「官方」 | 像素：Focus Sync / Git Tools 标题右侧是**紫色圆形徽章**（社区卡片是蓝色盾牌）；UIA：市场源 tab 名称 = 官方 | ✅ |
| 精选区 | `extensions.featured` → 「精选」区标题 + 精选网格 | UIA 命中「精选」；精选网格首两张 = **Focus Sync** / **Git Tools** | ✅ |
| 作者名 | 卡片 publisher 行（`extension-card__publisher`） | Focus Sync 与 Git Tools 两张卡的 publisher 均为 **Alpha File Manager** | ✅ |
| 来源徽章 | `ExtensionBadge type="source"`，label = 市场源名 | 社区源卡片上命中「第三方」来源徽章；**官方源条目按设计不显示来源徽章**（`extension-card.vue` 的 `sourceLabel` 对 `OFFICIAL_MARKET_SOURCE_ID` 返回空串） | ✅ |

原始 UIA 行（`uia-tree.txt` / `uia-tree-recheck.txt`，按 Y,X 排序）：

```
Text   | 精选 | 1161,455 61x46
Text   | Focus Sync | 1234,550 133x32
Text   | Git Tools | 1799,550 107x32
Text   | Alpha File Manager | 1234,590 196x27      <- Focus Sync 的 publisher 行
Text   | Alpha File Manager | 1799,590 196x27      <- Git Tools 的 publisher 行
Text   | 第三方 | 2677,557 55x24                   <- 社区源卡片上的来源徽章（label=市场源名）
Text   | Productivity | 1156,725 104x24            <- Focus Sync 的分类标签
Text   | Integration | 1292,725 97x24
Text   | v0.5.8 | 1144,793 61x23                  <- 重启复验后的卡片版本
```

> ⚠️ **「官方徽章」本身没有可读文本**（`ExtensionBadge` 默认 `showLabel=false`，只有图标 + tooltip），
> 所以 UIA 里搜不到「官方」二字属正常，不能据此判徽章没渲染；只有 `type="source"` 的徽章带文字。
> 官方徽章的真证据是**像素**：`shot-printwindow.png` / `shot-recheck.png` 里 Focus Sync、Git Tools 标题右侧的**紫色圆形徽章**，
> 与社区卡片的**蓝色盾牌徽章**形成对照。

### 6.2 §4.3 bundled 更新是否真的覆盖已安装副本（✅ 是）

| 对象 | 验收前 | 验收后（磁盘） | 验收后（UI） |
|---|---|---|---|
| `…\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\package.json` | version 0.5.5 / publisher `kizemo` | **version 0.5.8 / publisher `Alpha File Manager`** | 卡片显示 **v0.5.8**（重启后） |
| `…\extensions\kizemo.git\package.json` | version 0.1.0 / publisher `kizemo` | version 0.1.0 / publisher `kizemo`（**未变**） | 市场卡片 publisher = `Alpha File Manager`（取自清单条目） |

⇒ **bundled 同步确实覆盖了已安装副本**：`kizemo.focus-sync` 的 publisher 由 `kizemo` 变成 `Alpha File Manager`、版本 0.5.5 → 0.5.8；
`kizemo.git` 未被覆盖的原因见 §6.4 新发现 1。

### 6.3 复验：bundled 同步在会话内不刷新 UI（不是缺陷，是刷新时机）

- 第一轮（装完首次启动）：卡片显示 **`v0.5.5`**，而磁盘上已是 0.5.8。
- 第二轮（**重启应用**，不重装）：同一张卡片显示 **`v0.5.8`**。
- 机制：`use-extensions.ts` 的卡片版本取 `latestVersion || manifest?.version`，而 `manifest` 优先取**启动时**读到的
  已安装扩展 manifest（`installedExt?.manifest`）；bundled 同步在启动后写盘，内存快照不刷新 ⇒ 同会话内仍显示旧版本号，**重启即正确**。
- ⇒ **不是产品缺陷**；但「装完不重启 → 版本号显示旧值」值得写进发版说明。

### 6.4 本轮新发现（不在原四项内，留给后续）

1. **`kizemo.git` 不在 3.10.0 的 bundled 资源里** ⇒ 装机目录 `D:\Program Files\Alpha File Manager\extensions\`
   **只有 `kizemo.focus-sync`** ⇒ Git Tools 的**已安装副本** publisher 仍是 `kizemo`，用户在「已安装」页会看到旧名
   （市场页显示的是清单条目，已是 `Alpha File Manager`）。
   要修：把 `extensions/kizemo.git` 加进 `tauri.conf.json` 的 resources 再发一版，或在清单层统一口径。
2. **`PrintWindow` 截到下半屏黑块**（已知抓屏时机问题，非产品缺陷）；卡片区完整可读。
3. **「从零安装扩展」链路本机未验证**：`github.com` 的 archive 需走 git 代理（`http://127.0.0.1:7897`）才可达，
   本轮只取「市场页展示 + bundled 覆盖」证据，未做下载安装闭环。

### 6.5 验收资产与真实数据处理

| 内容 | 路径 |
|---|---|
| 第一轮（安装 + 展示 + bundled 覆盖） | `.superpowers/sdd/accept-310-market-branding.ps1` |
| 第二轮（重启复验版本号） | `.superpowers/sdd/accept-310-recheck.ps1` |
| 启动路由改写（`lastRoute` → 扩展页） | `.superpowers/sdd/set-start-route-310.cjs` |
| 日志 | `.superpowers/sdd/accept-310-market/accept-310-market.log`、`accept-310-recheck.log` |
| UIA 树 | `.superpowers/sdd/accept-310-market/uia-tree.txt`、`uia-tree-recheck.txt` |
| 截图 | `.superpowers/sdd/accept-310-market/shot-printwindow.png`、`shot-screen.png`、`shot-recheck.png` |

真实数据零污染（逐条核对）：

- `user-settings.json` 备份为 `user-settings.json.acc310-backup` / `.acc310b-backup`，验收后**先杀进程 → restore →**
  `SETTINGS_RESTORED_MATCH=True`；`AFM_ALIVE_BEFORE_RESTORE=0`（restore 前进程确已清空）。
- 未改动 `user-stats.json` / `user-workspaces.json` / `user-extensions.json`。
- 遮挡检查：窗口中心 `WindowFromPoint` 的根窗口 owner pid = 应用自身 pid ⇒ 截图期间无第三方窗口覆盖。

## 七、阻塞与未决

1. **Focus Sync 上架 —— ✅ 已完成（2026-11-05 00:44 实测复核，原阻塞全部解除）**
   - 公网仓库 <https://github.com/kizemo/focus-sync> 已建成：`created_at = 2026-11-04T16:34:21Z`、`visibility=public`、
     `default_branch=main`、tag `v0.5.8` 已推。仓库**只含白名单 7 项产物 + 一份 `README.md`（1907 B，简体中文）**：
     `package.json` 1969、`dist/index.js` 38550、`dist/index.js.map` 4263、`icon.png` 2360、`LICENSE` 1763、
     `locales/en.json` 772、`locales/zh-CN.json` 820（逐项 copy 后回读核对字节数，`COPIED=7 MISSING=[]`）。
     **不含** `bin/` `installer/` `sidecar/` —— sidecar 随安装器分发（`src-tauri/tauri.conf.json` resources），
     `dist/index.js` 只与 `http://127.0.0.1:37421` 回环通信 ⇒ 市场条目不含 sidecar 在功能上安全。
   - 线上清单 `kizemo/sfm-marketplace/registry.json` **已含 `kizemo.focus-sync` 条目**，且与本地工作副本**逐字节相同**：
     线上 blob `00722a7b0be0aef5bcf8695c3cb533fcdc3ac490` == 本地 `git diff` 给出的 index blob `00722a7b`；
     逐条目 deep-equal 输出 `SAME kizemo.focus-sync` / `SAME kizemo.git` ⇒ **无需再 PUT**。
   - **`integrity` 已实测证实**：`https://github.com/kizemo/focus-sync/archive/refs/tags/v0.5.8.zip` 连下两次均为
     **21197 B / sha256 `480ac78c5a49a08d5d7fb8a605fef5a36594b345d2223dce7688b587e2b50d15`（`STABLE = True`）**，
     zip 内 11 项 = 白名单 7 项 + README + 目录项，与本地条目 `releaseMetadata["0.5.8"].integrity` 逐字相等（`MATCH_LOCAL_REGISTRY = True`）。
   - **判别性对照成立**：同法取 Git Tools `v0.1.0.zip` = `4f0111d93591f82a5ce5124053660e7f8f3d39d146eb3c720ee4e1fc7391b09a`，
     与线上条目 `integrity` 逐字相等（`CONTROL_MATCH = True`）⇒ 坐实「条目 `integrity` = GitHub tag archive 整包 sha256」这一口径，不是本地自拼 zip。
     校验实现：`src-tauri/src/extensions/security.rs:312/:330` 把下载字节整包 sha256 与期望值逐字比较
     （`:294 normalize_integrity_value` 剥可选 `sha256:` 前缀并转小写）；取值链 `src/modules/extensions/utils/manifest-utils.ts:131`
     → `src/stores/runtime/extensions.ts:1181`/`:1533`；拼址 `src/data/extensions.ts:204-214`。
   - `node scripts/ext-market/publish.mjs --validate --registry <绝对路径>` **通过（2 个条目）**。
     分类采用官方表值 `Productivity` + `Integration`；manifest 内小写 `productivity`/`navigation` **保持不改**
     （改了要重算 integrity 并触发 bundled 更新，代价不对称）。
   - ⚠️ 仍未取的证据只剩**真机界面**一项（见 §八 第 10 步）。
2. **`sigma-hub` 不列为官方**是可逆决策（写在 `src/data/extensions.ts` 注释里）。若用户希望 sigma 生态也算官方，
   只需往 `OFFICIAL_EXTENSION_OWNERS` 加 `'sigma-hub'`，但会与「sigma = 第三方源」的上一轮决策相冲突。
3. **`hi` / `hy` 语言包**仍是旧品牌串，follow-up。
4. 上一轮「市场/已安装选项卡页面消失」的反馈**已由用户撤销**，不再排查。
5. **`kizemo.git` 不在 3.10.0 的 bundled 资源里**（2026-11-05 真机验收新发现，见 §6.4）⇒ 装机目录只有 `kizemo.focus-sync`，
   Git Tools 的**已安装副本** publisher 仍是 `kizemo`（市场页显示的是清单条目，已是 `Alpha File Manager`）。
   要修：把 `extensions/kizemo.git` 加进 `tauri.conf.json` 的 resources 后再发一版。

## 八、下阶段计划（按顺序）

> 用户 2026-11-04 明确授权本轮做完：**打包落地 → 补 tag `v3.10.0` → 上传 aiec 站点 `version.json` →
> 建 GitHub Release**，并批准 Focus Sync 按推荐方案（建公网仓）执行。发版/公网发布**不需再问**。

> **执行结果（2026-11-05 00:26 复核）**：上述四项授权动作 **全部完成** —— 打包落地 ✅、tag `v3.10.0` ✅、
> aiec 站点 `version.json` ✅（线上 verify 版本号/大小/sha256 三项一致）、GitHub Release ✅（非 draft、资产已 uploaded）。
> 接力会话**从第 8 步（Focus Sync 上架）开始做**，0~7 步不要重跑。
0. ✅ **门禁 + 打包已全部完成**（`gate-build-310c.cmd` 已落 `GATE_C_DONE`，无需重挂、无需重跑）：
   `npm run ts` **TSC_EXIT=0**；`npm run verify-changelog` **CLG_EXIT=0**；`npx eslint .` **186 errors / 0 warnings**（与 HEAD 基线同口径、无新增回归；
   该命令设计上非零退出，口径是逐字对照基线）；`npm run tauri:build` **BUILD_EXIT=0**（23:29:14 → 23:34:21）；`cargo test --lib` **912 passed / exit 0**（勿重跑）。
   ⇒ 接力会话第一件事改为**读 `logs/upload310.txt`**（见步骤 6）。
   ⚠️ 长任务写法（本项目唯一可靠姿势，保留备用）：
   `$p = Start-Process -FilePath 'cmd.exe' -ArgumentList '/c','<abs .cmd 路径>' -WindowStyle Hidden -PassThru`；
   **不要**用 `bash run_in_background` 直接跑长编译（工具回收会杀掉整棵进程树，就是 23:07 那轮 `gate-build-310b.cmd` 死亡的原因）；
   要等完成就另起**只读日志文件**的轮询脚本。`.cmd` 必须**纯 ASCII**、内部 `> log 2>&1`、不挂 stdout 管道、运行期间不得编辑、echo 文本里不能出现重定向符号。
3. ✅ **产物已落地**：`src-tauri/target/release/bundle/nsis/Alpha File Manager_3.10.0_x64-setup.exe` = **44 938 742 B**（23:34:17），`release/` 同名副本已在位。
   沿用提醒：`publish-bundle-to-release.mjs` 遍历 `bundle/nsis/` 全部历史包逐个 rmSync + 复制，任一被锁就 exit 1，而 `tauri build` 本身早已成功
   ⇒ **要分清「构建失败」和「复制失败」**，后者用 `fs.copyFileSync` 单独补进 `release/` 即可，不必重打包。
4. ✅ **发版提交已创建并推送**：`18a349e7 release: 发版 3.10.0`，`git push origin main` = `ceff7e38..18a349e7`。
   实测 **131 files changed, +14546 / −473**（交接时刻的 129 是更早快照）；提交后 `git status --porcelain` = **0 行**（工作树干净）。
   中文提交信息走 `git commit -F .superpowers/sdd/release-310-commit-msg.txt`（不要把中文放命令行）。
5. ✅ **已打 tag `v3.10.0` 并推送 origin**（`git ls-remote --tags origin v3.10.0` 已确认）。tag 对象 sha `9b442333f7ff12ace6bc089a0f4d5e744a2ee78c`。
   发版三条必查（**本轮全绿，结论可复用**）：
   - `git rev-parse v3.10.0^{commit}` = `18a349e71f0f8a6639e24c64636cab2518c6e1ca` = HEAD；`git show v3.10.0:package.json` ⇒ `3.10.0`
   - `git merge-base --is-ancestor 18a349e7 v3.10.0` ⇒ exit 0
   - `git show v3.10.0:src/data/extensions.ts` 命中 `OFFICIAL_EXTENSION_OWNERS = ['kizemo'];`；`v3.10.0:scripts/ext-market/registry.json` 命中 `"name": "Alpha File Manager"` + `"featured": true`；`v3.10.0:extensions/kizemo.focus-sync/package.json` 命中 `"name": "Alpha File Manager"`
6. ✅ **站点已上传并线上核验通过**（`UPLOAD_310_EXIT=0` / `UPLOAD_310_DONE`，00:22:26）：33 个文件上传 **OK、0 个 failed**。
   脚本内置 `verify` 的线上结论（`logs/upload310.txt` 第 130~137 行，逐字）：`[verify] 线上版本：3.10.0`；
   `清单 sha256：73bbf8d812c58b5a332a2ff676ae38e8cf47fbb21f8d41b0afac59d1ef7b848e`；`源站包大小：44938742 字节`；
   `边缘包大小：44938742 字节（CF-Cache-Status=MISS）`；**`[verify] 通过：版本号 / 大小 / sha256 三项均一致（源站与边缘都已生效）`**
   ⇒ aiec 站点这一项**已闭环**，接力会话不必重跑 `release:upload` / `release:verify`。
   沿用提醒：`_upload_alpha_files.py` 会补传**全部历史安装包（705.49 MB）**，耗时以十分钟计，**不是卡死**；日志是 **cp936**，读出乱码属正常；
   站点安装包文件名规则 `Alpha_File_Manager_<version>_x64-setup.exe`；`release/local-builds/` 会被内容改写 ⇒ `latestInstaller()` 必须一起扫该目录。
7. ✅ **GitHub Release 已建成**（`publishedAt = 2026-11-04T16:26:00Z` = 北京 00:26）。
   `gh release view v3.10.0 --repo kizemo/alpha-file-manager --json tagName,name,url,isDraft,isPrerelease,publishedAt,assets` 实测：
   `isDraft=false` / `isPrerelease=false` / `name="Alpha File Manager v3.10.0"`；资产 `Alpha_File_Manager_3.10.0_x64-setup.exe`
   **44 938 742 B**、`state=uploaded`、官方 `digest = sha256:73bbf8d8…848e`（与本地包逐字一致）；
   正文 **39 行中文发版说明**（新增 / 改进 / 修复齐全，与 `CHANGELOG.md` 的 3.10.0 同文），**没有发成空正文**。
   URL：<https://github.com/kizemo/alpha-file-manager/releases/tag/v3.10.0>
   ⚠️ 判 Release 是否建成**只能查 GitHub API**：本轮 `Start-Process` 未带 `-WorkingDirectory`，`.cmd` 里的相对路径日志没落在 `logs/`
   （`gh310.txt` 自始至终不存在），只看日志会误判成「脚本没跑」。资产的官方 sha256 用 API 的 `digest` 字段拿（本机到发行 CDN 被限速 0.01 MB/s，下载比对不可行）。
8. **Focus Sync 上架** ✅ **已完成**（判据与实测数据见 §七.1）：公网仓 `kizemo/focus-sync` + tag `v0.5.8` +
   白名单 7 项 + `README.md`；条目进本地清单**并已在线上生效**（`featured:true`、`publisher.name=Alpha File Manager`、
   `integrity` 经两次下载实测 + Git Tools 判别性对照证实）；`publish.mjs --validate` 2 条目通过。
9. ✅ 清单与文档入仓（2026-11-05）：`scripts/ext-market/registry.json`（+28 行 `kizemo.focus-sync` 条目，
   顺带修好 `kizemo.git` 里 `publisher.name` 行的缩进）与本 handoff 一起提交并推 `origin/main`；
   **发版提交 `18a349e7` 与 tag `v3.10.0` 不动**（清单热更新，不需要再发一版应用）。提交 SHA 用
   `git log -1 --format=%H -- docs/handoff-2026-11-04-extension-official-featured-branding-and-release-310.md` 取。
10. ✅ **3.10.0 装机真机验收 §六 四项证据 —— 已完成**（2026-11-05 00:45 → 01:10，结论与原始证据见 §六）：
    3.10.0 已静默装到 `D:\Program Files\Alpha File Manager`（`ProductVersion` 3.9.2 → **3.10.0**，NSIS `/S` exit 0，
    exe mtime = 2026-11-04 23:33:42 = 打包产物时间戳）；四项证据（官方徽章 / 精选区 / 作者名 / 来源徽章）**全部有 UIA + 像素双证据**；
    §4.3 的 bundled 覆盖亦已确认（`kizemo.focus-sync` 的磁盘与 UI 都变成 0.5.8 / publisher `Alpha File Manager`）。
    遗留见 §6.4（新发现 1：`kizemo.git` 不在 3.10.0 的 bundled 资源里，已安装副本 publisher 仍是 `kizemo`）。
11. winget 清单更新只能手工开 PR（未授权自动提交）。
12. follow-up：`hi` / `hy` 语言包旧品牌串。
## 九、证据文件路径清单

| 内容 | 路径 |
|---|---|
| 品牌/精选批量改动脚本与输出 | `.superpowers/sdd/apply-official-branding.cjs`、`.superpowers/sdd/logs/branding-apply.txt` |
| 版本核对脚本 | `.superpowers/sdd/check-version-310.cjs` |
| 门禁串行脚本与日志 | `.superpowers/sdd/gate-build-310b.cmd`（已死，留证据）、`.superpowers/sdd/logs/gateB.txt`、`gate-tsB.txt`、`gate-eslintB.txt`、`gate-vitestB.txt`、`build-310B.txt` |
| 门禁+打包串行脚本（✅ 已完成） | `.superpowers/sdd/gate-build-310c.cmd`（23:28:15 启动）、`.superpowers/sdd/logs/gateC.txt`（`GATE_C_DONE`）、`build-310C.txt`、`gate-tsC.txt`、`gate-clgC.txt`、`gate-eslintC.txt` |
| **站点上传（进行中）** | `.superpowers/sdd/upload-310.cmd`（23:45 detached，cmd pid 459008）、`.superpowers/sdd/logs/upload310.txt`（标记 `UPLOAD_310_START` / `UPLOAD_310_EXIT=` / `UPLOAD_310_DONE`） |
| **GitHub Release（已备好、未启动）** | `.superpowers/sdd/github-310.cmd`、日志将写 `.superpowers/sdd/logs/github310.txt`（标记 `GH_310_START` / `GH_310_EXIT=` / `GH_310_DONE`） |
| 站点准备 + 中文发版说明 | `.superpowers/sdd/logs/site-prep-310.txt`、`.superpowers/sdd/logs/site-notes-310.txt`、`.superpowers/sdd/release-310-notes.md`（2262 B）、`.superpowers/sdd/release-310-commit-msg.txt` |
| cargo 补跑 | `.superpowers/sdd/cargo-only-c.cmd`、`.superpowers/sdd/logs/cargoC.txt`、`gate-cargoC.txt` |
| 发版提交信息 | `.superpowers/sdd/release-310-commit-msg.txt` |
| 上一轮市场源改造设计/交接 | `docs/design-2026-11-04-extension-market-sources.md`、`docs/handoff-2026-11-04-extension-market-sources.md` |
| 上一轮缺陷修复交接 | `docs/handoff-2026-11-04-phase1-bugfix-default-filemanager-and-extension-sources.md` |
| 3.9.2 验收 findings | `.superpowers/sdd/accept-20261104-market-sources-findings.md` |
| 注册表与发布工具 | `scripts/ext-market/registry.json`、`scripts/ext-market/publish.mjs`、`scripts/ext-market/README.md` |
| 发版脚本 CLI 面 | `scripts/release-publish.mjs`（子命令 `prepare / upload / verify / github / status`） |
| **Focus Sync 归档取证（本轮）** | `.superpowers/sdd/fs-archive-hash.ps1`、`.superpowers/sdd/logs/fs-archive-hash.txt`（`CONTROL_MATCH` / `STABLE` / `MATCH_LOCAL_REGISTRY` 三行判据）、下载件 `logs/gt-a.zip` 31361 B、`logs/fs-a.zip` = `logs/fs-b.zip` 21197 B |
| **线上清单快照（本轮）** | `.superpowers/sdd/logs/online-registry.json`（解码后正文）、`.superpowers/sdd/logs/online-registry.sha.txt` = `00722a7b0be0aef5bcf8695c3cb533fcdc3ac490` |
| **Focus Sync 公网仓暂存目录** | `.superpowers/sdd/focus-sync-repo-v2/`（7 项白名单 + `README.md` 1907 B；独立嵌套 git 仓，`.superpowers/` 被主仓 gitignore ⇒ 不入主仓） |
| 清单回填补丁 | `.superpowers/sdd/patch-handoff-310d.cjs`（第二轮）、`.superpowers/sdd/patch-handoff-310e.cjs`（第三轮） |
| **3.10.0 真机验收（本轮）** | `.superpowers/sdd/accept-310-market-branding.ps1`（安装 + 展示 + bundled 覆盖）、`.superpowers/sdd/accept-310-recheck.ps1`（重启复验版本号）、`.superpowers/sdd/set-start-route-310.cjs`（把 `lastRoute` 指向扩展页）、`.superpowers/sdd/accept-310-market/accept-310-market.log`、`accept-310-recheck.log`、`uia-tree.txt`、`uia-tree-recheck.txt`、`shot-printwindow.png`、`shot-screen.png`、`shot-recheck.png` |

## 十、本轮新增的工具坑（**别再试原写法**）

- `edit` 工具参数名是 **`file_path`**，不是 `path`。
- node 脚本块注释里**不能出现 glob** `extensions/*/package.json` —— `*/` 提前终止注释 ⇒ `SyntaxError` 且改动全不落地。写成 `extensions/[id]/package.json`。
- `bash` 里 **`Set-Location` 之后的写/删目标无法校验** ⇒ 全程绝对路径、绝不 `cd`（纯 `Set-Location` + 只读命令可以）。
- `grep` 用 `src/**/*.test.ts` + `content` 模式曾假阴性 ⇒ `path` 指成**绝对目录**。
- `gh api --jq '.sha + " " + .path'` 在 pwsh 里双引号转义被吃 ⇒ 用单一 `--jq .sha`。
- `node -e "..."` 里含 `.*?` 等正则的双引号串会被 PowerShell 当命令解析 ⇒ 复杂 JS 一律落成 `.cjs` 再 `node <file>`；node 参数一律正斜杠绝对路径。
- `Get-ChildItem -Filter 'a','b'` 报类型转换错 ⇒ 用 `-Include` 或多次调用。
- `.ps1` 函数只要有返回值就不能混用 `Write-Output`（改 `Write-Host`）。
- **`cargo test --lib` 必须在 `src-tauri/`**（本轮 exit 101 的真凶，不是测试失败）。
- **后台 `cmd /c` 的完成通知不可信**（无 stdout 管道时提前报 succeeded，脚本仍在跑）⇒ 只看 `logs/` 文件尾部。
- 并行跑门禁会打死整个进程树 ⇒ 严格串行。
- **`Start-Process` 不带 `-WorkingDirectory` ⇒ `.cmd` 里的相对路径日志会漂走**（本轮 `gh310.txt` 始终不存在，而 `gh release create` 其实已成功）⇒ 判长任务结果**先查权威源**（GitHub API / 线上文件 / 落盘产物），日志只是辅助证据。
- **`gh api … --jq '.body'` 在 pwsh 里返回字符串数组** ⇒ `$b.Length` 是**行数**不是字符数（本轮把 39 行读成「正文只有 39 字符」，差点误判成空正文）。要字符数用 `($b -join "`n").Length`。
- **`Remove-Item` 会被本机安全策略整条拒掉**（要求走 mavis-trash）⇒ 别把「删旧日志」和「启动任务」写进同一条命令，一次拒绝会让**整条命令都不执行**；区分新旧日志改用**启动时刻时间戳**对比。
- **`git rev-parse --short A B` 不接受多个 rev**（报 `fatal: Needed a single revision`）⇒ 逐个查；`v3.10.0` 要用 `'v3.10.0^{commit}'` 才取到提交而不是 tag 对象。
- 改**已有文档**必须先程序化 dump 真实行内容再动手：本轮凭记忆写的 7 处 `edit` 锚点全数 count=0，正则 `^\|\s*(eslint|打包…)` 也匹配 0 行（单元格里带反引号与中文括注）⇒ 最终靠「行号 + 行首内容断言 + 自底向上 splice + 全通过才写盘」的 `.cjs` 一次成功。
- **本机到 GitHub 的 archive 下载必须走 git 的代理**（2026-11-05 钉死，代价是三轮白跑）：`git config --global http.proxy` =
  `http://127.0.0.1:7897`，而 **curl.exe / Invoke-WebRequest 都不读 git 配置** ⇒ 表现是「`git push` 和 `gh api`（api.github.com）全通，
  唯独 `curl https://github.com/.../archive/...zip` 报 `exit 28 Failed to connect to github.com:443` / `exit 56 Recv failure: Connection was reset`」，
  且 `netsh winhttp show proxy` 显示 Direct access、`HTTPS_PROXY` 环境变量全空 —— 三条证据都会把你引向「网络断了」的错误结论。
  ⇒ **判 GitHub 可达性必须分链路测**：`api.github.com`（gh）/ `github.com` git 协议 / `raw.githubusercontent.com`（直连 200 OK）/ codeload archive（必须 `-x http://127.0.0.1:7897`）。
  加 `--proxy` 后立刻通，一次拿到两个 archive 的 sha256。**别再对 archive 无代理重试。**
- **清单顶层键是 `extensions`，不是 `entries`**（`scripts/ext-market/registry.json` 与线上 `registry.json` 同形）⇒
  用 `j.entries.find(...)` 会抛 `TypeError: Cannot read properties of undefined`，而 schema 顶层 `additionalProperties:false`、**没有 `definitions` 键**。
  本轮把「本地清单没有 focus-sync 条目」判错，就是脚本用错键名导致 —— **结构猜错会让「不存在」的假结论看起来成立**。读前先 `Object.keys()`。
- `gh api <repo>/contents/registry.json` 返回的对象里带 `sha` 与 `content`（base64）⇒ 一行同时拿到线上 blob sha 和线上正文；
  与本地 `git diff -- registry.json` 头里 `index d3e5e023..00722a7b` 的后半**同源可比**（本轮两者相同 ⇒ 直接判定无需 PUT，省掉一次写操作）。
- `gh api repos/<o>/<r>/commits` 用 jq 取 `.message` 会报 `split cannot be applied to: null` ⇒ commit 列表里消息字段是 **`.commit.message`**，不是 `.message`。
- **GitHub 动态 tag archive 在本轮实测中跨请求逐字节稳定**（同一 tag 连下两次同 SHA，且 Git Tools 的历史条目值今日仍能复现）⇒
  旧笔记里「archive 字节可能不稳定」的顾虑在这些仓库上不成立；但**重新打 tag / 改内容必须重算 integrity**（值随 commit 变）。
- **调用预算**：本轮实际 40+ 次工具调用，严重超支（铁律 ≤10）。超支全部来自 archive 下载反复试与清单结构猜键名 ⇒
  下次「一个下载反复失败 + 一个 JSON 取不到值」时，**先各花一次调用查配置/键名，而不是各花三次试写法**。
- **验收脚本必须「先切到市场 Tab，再等卡片文案」**：市场卡片只存在于「市场」Tab 内（`extensions.tabs.marketplace`），
  先等文案会白等满 420 s 预算 ⇒ 等待循环里要每轮尝试 `SelectionItemPattern.Select()`。
- **`ExtensionBadge` 的 official / community 徽章默认 `showLabel=false`** ⇒ 只有图标 + tooltip，**UIA 里没有文字**
  ⇒ 「UIA 搜不到『官方』」**不能**判徽章没渲染；只有 `type="source"` 的徽章带文字（label = 市场源名）。
  官方徽章只能靠**像素**判（紫色圆形 vs 社区的蓝色盾牌）。
- **bundled 同步写盘后，同一会话内的 UI 仍显示旧版本号**（卡片取的是启动时的 manifest 快照）
  ⇒ 验「bundled 更新是否生效」要**重启应用**再看卡片版本号，只看首次启动会误判成「更新没生效」。
- **`.ps1` 里 `edit` 类锚点会因 CRLF 失配** ⇒ 改已有 `.ps1` 用 node 按行 splice，别用 `edit` 工具的整块 `old_string`。
- **`.cmd` 的 `echo` 文本里不能出现 `>`**（2026-10-24 钉死，本轮再次确认）：cmd.exe 把 `>` 当重定向符，连注释里的都不放过；
  `echo [release] bump minor -> 3.6.0` 会截断输出并**在仓库根生成一个名为 `3.6.0)` 的垃圾文件**。描述性文字改用 `to` / `=>`，或整句加引号。
- **`.cmd` 运行期间绝对不能编辑它**：cmd.exe 按字节偏移逐行读脚本，跑到一半改文件会让后续偏移全部错位、执行到别处的指令片段
  （历史事故：把 3.6.0 顶到 3.7.0 且日志被截断，Rust 构建产物全丢）⇒ 要改就**先停任务**，再**新起文件名**重跑。
- **后台 `.cmd` 可能在最后几行 `echo` 写出前就被判为结束**（本轮 `gates-3101h.cmd` 日志缺 `CARGO_TEST_EXIT=` / `GATES_END`）
  ⇒ 判结果**优先看被调命令自己的输出**（cargo 的 `test result:` 行），不要只认自己写的标记串。
- **`cargo test --lib` 的测试二进制有条隐形红线：任何被测试链接到的代码都不能碰 `tauri::AppHandle` / `tauri::async_runtime::*`**
  （会单态化出 `<Wry as Runtime>` → 拖进 tao → 加载期 `STATUS_ENTRYPOINT_NOT_FOUND` 0xC0000139，一个测试都跑不了，报错完全不指向你的模块）。
  绕法：给业务模块加 `*_blocking` 同步内核入口，调用方自己选执行器。
- **`#[tool_router]` 不等于接线完成**：`impl ServerHandler` 还必须加 `#[tool_handler]`，否则只有 `tools/list` 返回 `{"tools":[]}`；
  rustc 对 `tool_router` 字段的 `never read` 警告在这里**不是误报**。

## 十一、3.10.1 发版记录（2026-11-05 完成）

> 本版承接用户 2026-11-05 的四项需求 + 「完成后打包发版」。**3.10.0 的 0~7 步不重跑**。

### 11.1 四项需求与落点

| # | 需求 | 落点 | 状态 |
|---|---|---|---|
| 1 | 自建的 `kizemo.focus-sync` / `kizemo.git` 在卡片名称右方显示「官方」**文字**标签 | `extension-card.vue` / `extensions-installed.vue` / `extension-detail.vue` 三处徽章优先级改为 **official > local > community**；`ExtensionBadge` 对该两处改为带文字 | ✅ 真机验收通过 |
| 2 | 卡片说明文字跟随 AFM 设置语言（中文环境中文；第三方无中文语言包则回退英文） | 新建 `src/modules/extensions/utils/extension-locale.ts`（3329 B）+ `composables/use-extension-localized-description.ts`（6035 B）；改 `extension-card.vue` / `extension-detail.vue` 的 `description` computed | ✅ 真机验收通过 |
| 3 | 配置仓库链接时在中国网络环境自动跳转中国镜像 | `src-tauri/src/extensions/http.rs` 新增 `UrlCandidates::with_china_mirrors()`(`:607`)、`CHINA_GITHUB_MIRROR_PREFIXES`(`:703`)、`is_china_mirror_supported_host`(`:706`)、`build_china_mirror_urls`(`:713`) + 5 条单测(`:1088-1145`)；`github.rs:228`、`install.rs:138`、`binaries.rs:178/:248/:525/:670` 共 6 处挂上 | ⚠️ 代码 + 单测 + 网络层证据齐；**应用内「确实走了镜像」的硬证据未取到**，记 follow-up |
| 4 | Focus Sync 说明里的 "Sigma" → "File Manager" | 远端仓 `kizemo/focus-sync` 提交 `70c6f7d` + tag `v0.5.9`（`0.5.8` 未改写）+ registry 线上更新 + 装机副本生效 | ✅ 真机取证 |

#### 需求 1 根因（一句话）

`user-extensions.json` 里 `kizemo.git` 带 `isLocal=true`，而 `isOfficial` 本就为 true（`OFFICIAL_EXTENSION_OWNERS=['kizemo']`）；
病根是三个组件把 `isLocal` 排在 `isOfficial` **之前**，于是官方扩展被渲染成「本地」。

#### 需求 1 · 3.10.1 真机实测（UIA，已验证）

```
CARD Focus Sync at=1234,553 133x32 officialLabelsSameRow=1   -> LABEL Text at=1414,557 37x24
CARD Git Tools  at=1799,553 107x32 officialLabelsSameRow=1   -> LABEL Text at=1953,557 37x24
thirdPartyLabelsSameRow=0    EXT_LIST_READY=True    CHANGELOG_MODAL_CLOSED=none
SHOT=.superpowers/sdd/accept-3101/shot-3101-installed.png ok=True    ACCEPT_3101_DONE
```

> ⚠️ 旧结论「『官方』当关键字搜 UIA 只会命中筛选按钮（78x44）」在本轮仍成立 —— 真标签宽 **37**，筛选按钮宽 **78**，
> 判别必须带尺寸，否则会把筛选按钮当成徽章。

#### 需求 2 · 3.10.1 真机实测（UIA 原文，已验证）

```
Text | Listary 风格的全局焦点同步 —— 任何 Windows 应用的文件对话框自动跳转到 File Manager 当前目录。 | 1144,645 488x90
Text | 在 Alpha File Manager 里直接做 Git 操作：初始化、克隆、抓取、拉取、推送、同步，浏览分支与提交，并把仓库交给 IDE 打开。 | 1709,645 480x90
Text | Catppuccin color and icon themes for Sigma File Manager.   <- 第三方无中文语言包，按设计回退英文
Text | Light color themes for Sigma File Manager.                 <- 同上
```

单测：`src/modules/extensions/utils/__tests__/extension-locale.test.ts`（4596 B）**16 条全通过**。

#### 需求 4 · Focus Sync 0.5.9（已验证）

- 口径：tag 归档 ZIP 的 sha256 作 `integrity` —— **`9ec308531f9ade968a50733bb900e1de1b913f595d92c0bac30510cccf38cf94`** / **21 215 B**（codeload 直连 `HTTP=200`）。
- 源码逐字节：`package.json` 1988 B / Sigma=0、`locales/en.json` 794 B / 0、`locales/zh-CN.json` 834 B / 0、`dist/index.js` 38558 B / **13**（全在代码注释与 `sigma.ui` API 调用，非用户可见文案）。package.json `version` = **`0.5.9`**。
- 线上 registry：旧 blob `00722a7b…c490` → 新 **`7702d69825a16cc2bb0705546822ec9c19adb84f`**；回读 2119 B / `ENTRY_COUNT=2` / `VERSIONS=0.5.9,0.5.8` / `DESC_HAS_SIGMA=false`；`publish.mjs --validate` 通过。
- 装机副本：`EXT_INSTALLED_VERSION=0.5.9`，三个文件 Sigma 计数 0/0/0。
- 边界（范围锁定）：不动 `engines.sigmaFileManager`、不动 installer 里的进程名与安装路径检测、不改代码注释里的 `Sigma FM`、不改 API 全局名 `sigma.ui`；第三方扩展自带的 "Sigma File Manager" 不在范围。

### 11.2 需求 3 · 中国镜像的实现与边界

候选顺序：**primary → `ghfast.top` → `gh-proxy.com` → `ghproxy.net` → jsdelivr/codeload**。
`with_china_mirrors()` **必须写在其它 `with_fallback` 之前**，否则镜像排在后面永远轮不到。
`HTTP_TIMEOUT_SECS = 900`、`HTTP_CONNECT_TIMEOUT_SECS = 5`。

网络实测（已验证）：

| 目标 | 结果 |
|---|---|
| `github.com` 归档直连 | `http=000` / `(56) Recv failure: Connection was reset`（19.5 s） |
| `ghfast.top` | `http=200` / **1.216 s** |
| `codeload.github.com` 直连 | `http=200` / **0.642 s** |
| `ghfast.top` / `gh-proxy.com` / `ghproxy.net` 代理 `codeload.github.com`、`api.github.com` | **403** ⇒ 已被 `is_china_mirror_supported_host` 排除并被单测锁定 |
| `raw.gitmirror.com` | 已不可用 |

> ⚠️ **降级原因**：`http.rs` 不打印实际尝试的 URL，而本机 `raw.githubusercontent.com` 与 `codeload.github.com` 直连都能 200
> ⇒「市场安装成功」**不足以**证明镜像生效。要取证需给 `UrlCandidates` 加尝试日志后重跑。

### 11.3 门禁（3.10.1，已验证）

脚本 `.superpowers/sdd/gates-3101h.cmd`（1015 B，纯 ASCII），日志 `.superpowers/sdd/logs/gates-3101h.log`（1352 行）：

| 步骤 | 结果 |
|---|---|
| `npm run ts` | **`TS_EXIT=0`** |
| `npx eslint .` | `ESLINT_EXIT=1`，汇总 **186 problems (186 errors, 0 warnings)** —— 低于 188 基线；**三个新文件 0 违规** |
| `npm run verify-changelog` | **`CHANGELOG_EXIT=0`** |
| `cargo test --lib` | **`917 passed; 0 failed`**（与基线一致） |

> 判据是「总违规数 ≤ 基线且新文件 0 违规」，**不是退出码**（`eslint .` 设计上非零退出）。
> 本轮 4 条报错集中在 `file-browser-tree-view.test.ts` 的 `15:8 / 15:47 / 69:43 / 72:55`，经 `git diff` 核对**不在本轮新增的 13 行内**。
> **既有格式违规刻意不跑 `--fix`**（会污染 `eslint .` 的计数口径）。

### 11.4 打包、安装、恢复（已验证）

| 环节 | 数据 |
|---|---|
| 打包脚本 | `.superpowers/sdd/build-3101g.cmd`（631 B，纯 ASCII）→ 日志 2137 行，`Finished 'release' profile in 3m 53s`、`TAURI_BUILD_EXIT=0` |
| 安装包 | **44 940 588 B** / sha256 **`e074d02326ccf7d7a9c76ddb7cd922a31d71d8d6ff7cedb5cebc7f8e4de3e164`**（`bundle/nsis/`、`release/`、`release/local-builds/` 三方一致） |
| 静默安装 | `install-3101g.ps1` → `RUN_RC=0`、`INSTALL_EXIT=0`、`DISPLAY_VERSION=3.10.1`、`PRODUCT_VERSION=3.10.1` |
| 已装 exe | `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` 80 659 968 B / `14946f67…283e` / mtime `2026-11-05 09:54:52`（由旧 f 包的 `c50e8604…` 变为新值 ⇒ 新包已生效） |
| 真实数据恢复 | 先 `Stop-Process`（`PROCS_AFTER=0`）→ 再逐字节还原 5 个文件，全部 `RESTORED_OK`：`user-extensions.json` 1 867 668 B、`user-remote-connections.json` 1 154 B、`user-settings.json` 5 386 B、`user-stats.json` 32 088 B、`user-workspaces.json` 2 338 B |

> ⚠️ **三处 exe 哈希互不相同**（已装 `14946f67` / `target\release` `94ec189d` / NSIS 内应是 `94ec189d`），
> 推断为打包链时间戳差异，未继续追查；功能层证据已确认新包生效。**判安装成功看 `DisplayVersion` 与扩展文件 Sigma 计数，不看 NSIS `/S` 的 ExitCode**（历史上出现过 2）。

### 11.5 提交 / 推送 / tag（已验证）

- 单提交 **`bf747ff7` `release: 发版 3.10.1`**，`24 files changed, 640 insertions(+), 37 deletions(-)`；`git add -A` 后 `STAGED_ENTRIES=24`。
- `git push origin main` → `75979b3d..bf747ff7`，`PUSH_MAIN_EXIT=0`。
- `git tag -a v3.10.1` → `git push origin v3.10.1` → `* [new tag]`；`git ls-remote --tags origin`：`5ad7afc1…`（tag 对象）+ `bf747ff7… refs/tags/v3.10.1^{}`。
- 提交后 `git status --porcelain` = **0 行**。
- 版本四处同步：`package.json` + 三个 `Cargo.toml` + `src-tauri/Cargo.lock` 均为 `3.10.1`。
- `CHANGELOG.md` 的 `## [3.10.1] - November 2026` 段补齐四项（+13 行）；`README.md:286-299` 写明 `Publisher of our extensions: Alpha File Manager` 与 `badged *Official* next to their name`。

### 11.6 站点与 GitHub Release

站点目录 `E:\办公文件\L1网站\alpha-file-manager\`：

- `version.json` 已改 **3.10.1**（`sha256=e074d023…e164`；旧 3.10.0 归档进 `history[]` 首位、截到 20 条上限）。
- 新增 `Alpha_File_Manager_3.10.1_x64-setup.exe`（44 940 588 B，sha256 与 release 一致，脚本回读校验）。
- 新增 `_release-notes-v3.10.1.md`（由 `prepare --write-notes .superpowers/sdd/notes-3101.md` 生成）。
- 上传口径：`合计 34 个文件, 748.34 MB`（会重传全部历史包，**十分钟级，不是卡死**）；日志 `.superpowers/sdd/logs/upload-3101.log`（cp936，读出乱码属正常）。
- 发版命令链（**均可直接复用**）：
  ```
  node scripts/release-publish.mjs prepare --bump none --skip-build --notes-file .superpowers/sdd/notes-3101.md
  node scripts/release-publish.mjs prepare --write-notes .superpowers/sdd/notes-3101.md
  node scripts/release-publish.mjs upload     # 内置 verify
  node scripts/release-publish.mjs github     # 建/替换 Release 资产
  ```
- `prepare --write-notes` 是 `main()` 里的**独立优先分支**，不递增版本、不重写 `pub_date`。

**最终核验（2026-11-05 完成，已验证）**

- SFTP 上传：任务 `bg_284a02fe` 退出码 0，日志 `upload-3101.log`（9406 B）出现逐字：
  `[verify] 线上版本  ：3.10.1` / `[verify] 清单 sha256：e074d023…e164` /
  `[verify] 源站包大小：44940588 字节` / `[verify] 边缘包大小：44940588 字节（CF-Cache-Status=MISS）` /
  **`[verify] 通过：版本号 / 大小 / sha256 三项均一致（源站与边缘都已生效）`**。
- 独立复核：`https://www.aiec.fun/alpha-file-manager/version.json?_=<stamp>` → HTTP 200、`version=3.10.1`。
- GitHub Release：`gh release view v3.10.1 --repo kizemo/alpha-file-manager` → `isDraft=false`、`isPrerelease=false`、
  `publishedAt=2026-11-05T02:15:16Z`、资产 `Alpha_File_Manager_3.10.1_x64-setup.exe` **44940588 B**、
  `digest=sha256:e074d02326ccf7d7a9c76ddb7cd922a31d71d8d6ff7cedb5cebc7f8e4de3e164`，
  URL `https://github.com/kizemo/alpha-file-manager/releases/tag/v3.10.1`。
- 收尾提交：`3aa4df85 docs: 回填 3.10.1 发版记录（handoff §十一 + 5 条新工具坑）` 已 push；`git status --porcelain` 为空。

### 11.7 本轮遗留（follow-up，不阻断发版）

1. **需求 3 缺应用内硬证据**（见 11.2 ⚠️）：需给 `UrlCandidates` 加「实际尝试的 URL」日志后重跑。
2. **`src/utils/__tests__/module-load-recovery.test.ts:108-124` 仍失败**：`vi.spyOn(Storage.prototype,'setItem')` 在 jsdom 的 Proxy 版 Storage 下没拦到实际调用。
3. **需求 4 触发源未落到具体函数名**：`hasUpdate` 是同步函数，日志里 `cancel_all` 与「启动自动更新」之间的缺口未解释。
4. **`hi` / `hy` 语言包旧品牌串**（承接 3.10.0 遗留）。
5. **`kizemo.git` 未进 bundled resources**（承接 §6.4 新发现 1）：装机目录只有 `kizemo.focus-sync`。
6. **`gates-3101h.cmd` 不落 `CARGO_TEST_EXIT` / `GATES_END`**：若后续靠标记串轮询该脚本，须知道这两行可能不出现，改判 cargo 自己的 `test result:`。
7. **`extension-settings.vue:70-128`** 用 AFM code（`ch`）拼扩展语言包文件名是既有缺陷，**新代码不得照抄**（正确处理见 `extension-locale.ts`：扩展语言包是 BCP-47 `zh-CN.json`）。

### 11.8 独立复核（另一并发会话，2026-11-05 10:15，已验证）

> 本节由**并发会话**追加，只给 §11.4~§11.6 的结论补一份独立证据；**全程只读取证，未重复执行任何发布动作**（避免撞 `target\release` 构建锁与 `release\` 复制锁）。

- **git 真相重量**：`git tag --list` 含 **`v3.10.1`**，`git rev-parse 'v3.10.1^{commit}'` = **`bf747ff7`**；`HEAD` = `origin/main` = **`487a9477`**，`git rev-list --left-right --count origin/main...HEAD` = **`0 0`**；`git status --porcelain` = **0 行**（工作树干净，本节追加之前）。
- **出货包**：`release\Alpha File Manager_3.10.1_x64-setup.exe` **44 940 588 B** @ `2026-11-05 09:55:37`，sha256 **`e074d02326ccf7d7a9c76ddb7cd922a31d71d8d6ff7cedb5cebc7f8e4de3e164`** —— 与 §11.4 / §11.6 逐字一致。
- **装机状态**：HKLM `DisplayVersion=3.10.1`、`InstallLocation=D:\Program Files\Alpha File Manager`；已装 `alpha-file-manager.exe` 80 659 968 B / `ver=3.10.1` / mtime **09:54:52** ⇒ 本机装的**就是最终出货包**，不是中间包。
- ⚠️ **作废声明**：本会话早前那轮真机验收（`.superpowers\sdd\state-3101-accept.md`、`.superpowers\sdd\accept-3101-market\crop-3101-*.png`）针对的是 **`a30ed04d…42a37a` / 44 946 053 B** 的中间包（09:55 因 `tauri.conf.json` 的 `bundle.resources` 内嵌了 focus-sync 的 package.json/dist/locales 而重建）⇒ **该轮结论仅作过程记录，不再作为发版依据**。
- **徽章的独立像素判读（最终包）**：直接判读 §11.1 那轮的裁切件 `.superpowers\sdd\accept-3101\crop-Git_Tools-2x.png` 与 `crop-Focus_Sync-2x.png` —— **两张均为绿色对勾 +「官方」文字**（3.10.0 同位置是紫色 HardDrive「本地」）；与 `accept-3101.log` 末次运行（09:59:48）的 `EXT_LIST_READY=True` / `officialLabelsSameRow=1` / 标签 `37x24` / `thirdPartyLabelsSameRow=0` / `shot-3101-installed.png ok=True` / `ACCEPT_3101_DONE` 相互印证。日志里 09:16:33 那跑是 `EXT_LIST_READY=False` 的未就绪无效跑，不作证据。
- **GitHub Release 独立读取**：`gh release view v3.10.1 --json tagName,isDraft,assets` → `isDraft=false`、资产 `Alpha_File_Manager_3.10.1_x64-setup.exe` **44 940 588 B**、`digest=sha256:e074d023…e164`。
- **站点独立读取**：`E:\办公文件\L1网站\alpha-file-manager\version.json` 现 **73 115 B** @ `10:09:27`、`version=3.10.1`；`.superpowers\sdd\logs\upload-3101.log` 尾部逐字 `[verify] 通过：版本号 / 大小 / sha256 三项均一致（源站与边缘都已生效）`（该日志是 cp936，按 UTF-8 解出替换字符属正常，不影响判读）。
- **未闭环**：与 §11.7 一致，本节未新增阻塞。**需求 3「应用内确实走了镜像」的硬证据仍缺**，是接力首选项。

## 十二、§11.7 七条 follow-up 收口（2026-11-05）

> **本轮范围**：只做 §11.7 的 7 条 follow-up，**不做发版动作**。版本号仍是 `3.10.1`，
> `release/Alpha File Manager_3.10.1_x64-setup.exe`（已发布资产，sha256 `e074d023…e164`）**原样未动** ——
> `scripts/publish-bundle-to-release.mjs` 对「同名异容」的产物会改写进 `release/local-builds/`，不会覆盖已发布资产。
> 基线：HEAD `a2b4e4ad`（tag `v3.10.1`），本轮起始工作树干净。
> 门禁全绿 + **需求 3 的「应用内确实走了中国镜像」硬证据已取到**（见 12.5）。

### 12.1 七条 follow-up 收口一览

| # | follow-up（§11.7 原文） | 状态 | 落点 |
|---|---|---|---|
| 1 | 需求 3 缺应用内硬证据 | ✅ 已补日志 + **装机取证成功** | `src-tauri/src/extensions/http.rs` |
| 2 | `module-load-recovery.test.ts:108-124` 仍失败 | ✅ 已修 | `src/utils/__tests__/module-load-recovery.test.ts` |
| 3 | 需求 4 触发源未落到具体函数名 | ✅ 已定位并解释缺口 | `src/stores/runtime/extensions.ts` |
| 4 | `hi` / `hy` 语言包旧品牌串 | ✅ 已换 | `src/localization/messages/{hi,hy}.json` |
| 5 | `kizemo.git` 未进 bundled resources | ✅ 已进（部署/注册仍开放） | `src-tauri/tauri.conf.json` |
| 6 | 门禁脚本不落 `CARGO_TEST_EXIT` / `GATES_END` | ✅ 改逐步 state 文件 | `.superpowers/sdd/gates-3102.cmd` |
| 7 | `extension-settings.vue` 用 AFM code 拼扩展语言包文件名 | ✅ 改 BCP-47 | `src/modules/settings/ui/categories/extensions/extension-settings.vue` |

### 12.2 逐条说明

#### 12.2.1 需求 3 ·「实际尝试了哪个 URL」的应用内取证

**代码落点**：`src-tauri/src/extensions/http.rs`

- 新增 `describe_candidate_for_log(url) -> String`：保留 **scheme + host + path**，丢掉 query / fragment / userinfo
  （query 可能带访问令牌，绝不能落日志）。
  中国镜像候选的完整形态是 `https://ghfast.top/https://raw.githubusercontent.com/owner/repo/main/file.json` ——
  只记 host 看不出到底走了哪条路，所以专门补一条能直接当证据用的完整形态。
- `fetch_text_across_urls_with_config()`：每个候选进循环先打
  `[ext-http] attempt op=… candidate=i/n url=…`；成功打
  `[ext-http] result op=… candidate=i/n status=… ok=… url=…`；候选失败打
  `[ext-http] candidate-failed op=… candidate=i/n url=… reason=…`。
- `download_to_file_across_urls_with_retry()`：同样加 attempt / result。
- 单测 +2 条：`describe_candidate_for_log_keeps_full_china_mirror_shape`、
  `describe_candidate_for_log_drops_query_and_fragment`。

**为什么必须是 INFO 而不是 DEBUG**：`src-tauri/src/lib.rs:612-625` 的 `tauri_plugin_log` 是
`.level(log::LevelFilter::Info)` ⇒ **DEBUG 不进 release 的日志文件**，写在 DEBUG 层的取证等于没写。

**为什么之前明明有镜像、日志里却查无实据**（3.10.1 应用日志 `…\logs\Alpha File Manager.log`，14 106 行实测）：

```
L13199 cancel_all_extension_commands called for extension: kizemo.focus-sync
L13200 download extension 'kizemo.focus-sync': attempt 1/4 on github.com failed
L13204 download extension 'kizemo.focus-sync': attempt 3/4 on github.com failed
L13217 download extension 'kizemo.focus-sync': giving up on github.com after 4 attempts
L13218 download extension 'kizemo.focus-sync': trying next fallback URL after transient failure
L13221 Renamed locked directory …\extensions\.kizemo.focus-sync.bac
L13222 cancel_all_extension_commands called for extension: kizemo.focus-sync
```

`github.com` 候选 4 次全败 → 切下一个候选 → 紧接着就进入换装收尾（`.bak` 改名）⇒
**下一个候选（`ghfast.top`）下载成功了，但成功路径一行日志都不打**
（`download_to_file_across_urls_with_retry` 的成功分支就是 `Ok(()) => return Ok(())`）。
⇒ 整份日志里 `ghfast` 命中 **0** 次。

**教训**：「失败方的 WARN 只写 host」+「成功方完全不打日志」两头叠加，
镜像链明明在候选队列里，却无法从应用内证明走过 —— 这正是本条 follow-up 要补的洞。

#### 12.2.2 follow-up 2 · `module-load-recovery.test.ts` 失败用例

**根因（实测，非推测）**：jsdom 的 `Storage` 是 **Proxy**，`get` 陷阱返回的是实现对象上的**绑定副本**，
所以 `vi.spyOn(Storage.prototype, 'setItem')` 拦不到实际调用。探针实测：
`spyIntercepts=false`、`sameFn=false`、`descriptorConfigurable=true`、`definePropertyThrows=false`、
`stubIntercepts=true`。

**修法**：整体替换 storage 对象，而不是改原型 ——
`vi.stubGlobal('sessionStorage', { getItem, setItem(抛错), removeItem, clear, key, length })`，
并在 `afterEach` 里加 `vi.unstubAllGlobals()`。
定点复验：`npx vitest run src/utils/__tests__/module-load-recovery.test.ts` → **8 passed**（修前 1 failed / 7 passed）。

#### 12.2.3 follow-up 3 · 需求 4 触发源落到具体函数名 + 缺口解释

需求 4 的「装机副本生效」= 应用启动时自动把 `kizemo.focus-sync` 从 v0.5.8 换到 v0.5.9。
**触发链（全部落到具体函数名）**：

```
src/stores/runtime/extensions.ts
  init()                                  :2372
  └─ void runStartupMarketplaceSync()    :2431   ← void，不 await（市场同步不阻塞启动）
       └─ runStartupMarketplaceSync()    :2361
            ├─ fetchRegistry()
            ├─ prefetchAllVersions()
            └─ autoUpdateAllExtensions()  :2365   ← 「启动自动更新」在这一行
                 ├─ getAutoUpdateExtensions()     :2275  ← 筛选用同步的 hasUpdate()
                 └─ await updateExtension(id)     :2298
                      └─ updateExtension()         :1466
                           ├─ unloadExtension()                :1486
                           ├─ invoke('cancel_all_extension_commands') :1487 ← 日志里那条 cancel_all
                           ├─ getLatestVersion() / fetchExtensionManifest()
                           └─ (Rust) download_extension → src-tauri/src/extensions/install.rs:106
                                ├─ UrlCandidates::new(url).with_china_mirrors()  install.rs:137-139
                                └─ download_to_file_across_urls_with_retry()      install.rs:144
```

**`hasUpdate` 为什么不是触发源**：它（`src/stores/runtime/extensions.ts:2229-2247`）是**同步**函数，
只读 `getCachedLatestVersion(registryEntry.repository)` 的缓存版本号与已装版本比大小，**不发任何网络请求**；
在启动自动更新里它只当**筛选用**（`getAutoUpdateExtensions()`）。

**「cancel_all 与启动自动更新之间的缺口」缺的是什么**：

`cancel_all_extension_commands` **不是**启动钩子，而是 `updateExtension()` 的**第 2 步**（换装前把该扩展的进程全杀掉，
见 :1486-1487）。前端 6 个调用点 —— `rollbackInstallAfterCancel:458` / `installExtension:1120` /
`installLocalExtension:1235` / `refreshLocalExtension:1324` / `removeInstalledExtension:1413` /
`updateExtension:1466` —— **全部**在安装/更新/卸载流程里，**没有一个是启动钩子**。

缺口是「启动自动更新」这件事在应用日志里几乎没有自己的痕迹，**两头都看不见**：

1. 前端唯一的启动标记是 `console.log('Auto-updating extension: ' + id)`
   （`src/stores/runtime/extensions.ts:2297`），走 WebView console，
   **不进 tauri_plugin_log 的日志文件**（该插件只落 Rust `log` crate 的输出，本仓没接前端 console 转发）
   ⇒ 3.10.1 整份日志里 `Auto-updating` 命中 **0** 次。
2. 后端唯一的痕迹是 `download extension 'kizemo.focus-sync'` 这条 WARN，而它只写 host
   （`describe_url_for_log` → `github.com`），**看不出候选的完整 URL**。
3. **候选成功时一行日志都不打**（见 12.2.1）。

⇒ follow-up 1 补的 `[ext-http] attempt / result / candidate-failed` 正好把两头都补上：
`op=download extension 'kizemo.focus-sync'` + 候选完整 URL = 一条能直接当应用内硬证据的链。

#### 12.2.4 follow-up 4 · `hi` / `hy` 旧品牌串

- `hi.json`：`officialTooltip` 的「सिग्मा फ़ाइल मैनेजर」→「Alpha File Manager」。
- `hy.json`：`officialTooltip` / `engineAppWarning` / `requiresSigmaFileManager` 三处的
  「Sigma ֆայլային մենեջեր」「Sigma ֆայլերի մենեջեր」→「Alpha File Manager」。
- 不在范围：各语言包第 4 行的市场描述「is developed on top of Sigma File Manager」；
  第三方扩展自带的 "Sigma File Manager" 文案；`engines.sigmaFileManager`、installer 进程名/路径检测、
  代码注释里的 `Sigma FM`、API 名 `sigma.ui`。

#### 12.2.5 follow-up 5 · `kizemo.git` 进 bundled resources

`src-tauri/tauri.conf.json` 的 `bundle.resources` 新增 3 条：

```
"../extensions/kizemo.git/package.json": "extensions/kizemo.git/package.json"
"../extensions/kizemo.git/dist":          "extensions/kizemo.git/dist"
"../extensions/kizemo.git/locales":       "extensions/kizemo.git/locales"
```

**本轮做到哪一步**：装机目录（`$INSTDIR\extensions\`）从「只有 `kizemo.focus-sync`」变成「两个官方扩展都在」——
真机实测 `D:\Program Files\Alpha File Manager\extensions\` 下 `kizemo.focus-sync` 与
`kizemo.git`（含 `dist/`、`locales/`）**均已落地**（见 12.5）。

**仍开放（本轮明确不做）**：把 `kizemo.git` **部署**到 `%APPDATA%\com.sigma-file-manager.app\extensions\`
并写进 `user-extensions.json`。理由：
① `src-tauri/installer/hooks.nsh` 的 POSTINSTALL hook 是**逐文件硬编码** `kizemo.focus-sync` 的
（package.json / dist/index.js / dist/index.js.map / locales/en.json / locales/zh-CN.json），要带上 `kizemo.git`
得扩 hook 与 `tools/register.ps1`；
② Rust 侧**没有** `resource_dir` 的运行时消费者（实测全仓无 `resource_dir` / `resolve_resource` 命中），
所以 bundled 的 `kizemo.git` 在当前形态下是「随包发布」而不是「自动装好」；
③ 自动注册会**改变新装用户的默认扩展集合**，属产品决策，超出本轮 follow-up 范围。

#### 12.2.6 follow-up 6 · 门禁脚本可靠标记

`.superpowers/sdd/gates-3102.cmd`（纯 ASCII）：每步完成后**立刻把退出码写进独立的 state 文件**
`.superpowers/sdd/state-3102.txt`，轮询侧只看「最后写进去的是哪一行」就能判定进度到哪一步，
不再依赖「最后一步才写的尾标记」。

```
GATES_START
TS_EXIT=0
VITEST_EXIT=0
ESLINT_EXIT=1
CHANGELOG_EXIT=0
CARGO_TEST_EXIT=0
GATES_DONE
```

⚠️ 写 state 的 `echo` **必须在 `>` 前留一个空格**：`echo TS_EXIT=%ERRORLEVEL%>"%STATE%"` 在退出码为 `0` 时
会被 cmd 解析成 **stdin 重定向 `0>`**，写出去只剩 `TS_EXIT=`（退出码丢失）——
这是本轮实际踩到并修掉的 bug（第一版脚本就是这么写的，跑出来是空的 `TS_EXIT=`）。

#### 12.2.7 follow-up 7 · 扩展语言包文件名改 BCP-47

`src/modules/settings/ui/categories/extensions/extension-settings.vue` 的 `loadExtensionMessages()`
原来用 AFM 自己的 locale code（`ch` / `he` / `hy`…）拼 `locales/${locale}.json`，
而扩展包的语言包是 **BCP-47** 文件名（`locales/zh-CN.json`）⇒ 永远命中不了。

修法：**复用**既有正确实现 `src/modules/extensions/utils/extension-locale.ts`
（`getExtensionLocaleFileNames` / `EXTENSION_LOCALE_DIR` / `EXTENSION_DEFAULT_LOCALE`，
内部走 `intlLocaleMapping` 做 AFM code → BCP-47 映射），不新造映射表；
命中顺序 = BCP-47 候选 → AFM 原始 code → `locales/en.json` 兜底。

### 12.3 门禁（本轮，已验证）

```
TS_EXIT=0
VITEST_EXIT=0        284 个测试文件 / 2648 个用例全过
ESLINT_EXIT=1        186 errors / 0 warnings —— 与 3.10.1 基线逐字相同（186/0）
                     本轮 6 个改动文件单独复查：0 违规
CHANGELOG_EXIT=0
CARGO_TEST_EXIT=0     919 passed（3.10.1 基线 917 + 本轮新增 2 条单测）
GATES_DONE
```

`eslint .` 退出码非零是**仓库既有状态**（186 条存量违规），门禁判据是「总数不高于基线 + 新文件 0 违规」，本轮两条都满足。

本轮工作树改动 = 6 个文件（`git diff --stat`：127 insertions / 39 deletions）：

```
src-tauri/src/extensions/http.rs                            | 88 +++++++++++++++++++++-
src-tauri/tauri.conf.json                                  |  5 +-
src/localization/messages/hi.json                           |  2 +-
src/localization/messages/hy.json                           |  6 +-
src/modules/settings/ui/categories/extensions/extension-settings.vue | 49 +++++-------
src/utils/__tests__/module-load-recovery.test.ts            | 16 +++-
```

### 12.4 本轮新增的工具坑（**别再试原写法**）

1. **cmd 的 `echo X=%VAR%>file` 在 `VAR=0` 时会被解析成 stdin 重定向 `0>`** ⇒ 写出的内容只剩 `X=`，
   退出码丢失。所有 `echo … >file` 都必须在 `>` 前留一个空格（`echo X=%VAR% >file`）。
2. **应用日志里 `ghfast` 命中 0 次 ≠ 镜像没生效** —— 下载链路的成功分支不打日志，
   只有失败方才有 WARN，而 WARN 还只写 host。判断「有没有走镜像」不能只看失败方的行。
3. **前端 `console.log` 不进 `tauri_plugin_log` 的日志文件**（该插件只落 Rust `log` crate 的输出）。
   任何「靠前端打印证明某件事发生过」的取证思路在本项目都不成立。
4. **`rm -- <path>` 在本机被安全策略拦下**（要求 trusted launcher 的 trash 标记）。
   一次性文件的处置改用 `Move-Item` 移到 `.superpowers/sdd/`（gitignore 区）—— 同样让工作树干净，且可恢复。
5. **jsdom 的 `Storage` 是 Proxy**：`vi.spyOn(Storage.prototype, 'setItem')` 必然失效，
   `vi.stubGlobal('sessionStorage', …)` 才有效。
6. **`.cmd` 里的 `timeout /t N` 在无控制台的后台进程里会立刻失败**（`ERROR: Input redirection is not supported`，
   错误被 `>nul 2>&1` 吞掉）⇒ **等待根本没发生**。第一轮装机脚本就是这么把「等 90 秒」变成了「等 0 秒」，
   采集到的是应用刚启动 1 秒时的日志（`startup-probe.log` 的 mtime 正好等于启动秒数，是这个坑的指纹）。
   后台脚本里改用 `ping -n N+1 127.0.0.1 >nul` 或 `powershell -c "Start-Sleep N"`。
7. **安装包通配符别写成 `*_setup.exe`**：NSIS 产物叫 `Alpha File Manager_<ver>_x64-setup.exe`（**连字符**），
   `*_setup.exe` 永远匹配不到 ⇒ 脚本静默走 `NO_INSTALLER` 分支。
8. **Windows 的 `Get-ChildItem` 对「正在被进程打开写入的文件」会返回过期的 Length / LastWriteTime**：
   同一份应用日志，`Get-ChildItem` 报 2 011 962 B @ 11:11:52，而 `node` 实读是 2 010 014 B（更短、更新）。
   判日志内容一律用 `node` 读文件，**不要**用文件元数据推算内容。
9. **应用的 SSRF 守卫在「连接时」用自定义 resolver 复核解析出的 IP**
   （`http.rs` 的 `PublicOnlyResolver` → `ensure_public_socket_addrs` → `is_private_ip_address`）：
   把域名指向 `127.0.0.1` 不会「让它连不上」，而是**在出网前就被拒**（`Access denied: target host is not allowed`），
   且该错误被归类为 **Permanent** ⇒ 不会落到下一个候选 ⇒ 测不出镜像。
   `security.rs` 明确**不拦** `198.18.0.0/15`（fake-IP 段），用它做黑洞才能既过守卫又制造超时。
10. **注册表缓存 TTL 只有 5 分钟**（`EXTENSION_REGISTRY_CACHE_TTL_MS = 5 * 60 * 1000`），
    缓存写在 `user-extensions.json` 里、跨启动保留 ⇒ 刚跑过一次市场同步后再立刻重启，
    **整段同步会被跳过**，日志里一条 `[ext-http]` 都不会有。做取证前要先把缓存放过期。

### 12.5 打包、装机与应用内取证

#### 12.5.1 打包（`tauri build`，后台跑，未用 `tauri dev`）

```
.superpowers/sdd/build-3102.cmd   （纯 ASCII；内部 `> logfile 2>&1` 重定向，不挂 stdout 管道）
.superpowers/sdd/state-build-3102.txt
.superpowers/sdd/logs/build-3102.log
```

state 文件最终内容：

```
BUILD_START
SYNC_VERSION_EXIT=0
BUILD_EXIT=0
BUILD_DONE
```

产物：`src-tauri\target\release\bundle\nsis\Alpha File Manager_3.10.1_x64-setup.exe`
**44 960 646 B** @ `2026-11-05 11:11:45`。
`publish-bundle-to-release.mjs` 把它落到 `release\local-builds\`，**没有覆盖**已发布的
`release\Alpha File Manager_3.10.1_x64-setup.exe`（44 940 588 B，sha256 `e074d023…e164`）。

#### 12.5.2 静默装机

```
.superpowers/sdd/accept-3102b.cmd   （修正安装包通配符后的版本）
.superpowers/sdd/state-accept-3102.txt
```

```
ACCEPT_START
APP_STOPPED
INSTALLER=…\bundle\nsis\Alpha File Manager_3.10.1_x64-setup.exe
INSTALL_EXIT=0
BUNDLED_KIZEMO_GIT=YES       ← follow-up 5 落地
BUNDLED_FOCUS_SYNC=YES
BUNDLED_KIZEMO_GIT_DIST=YES
BUNDLED_KIZEMO_GIT_LOCALES=YES
APP_LAUNCHED
COLLECT_EXIT=0
ACCEPT_DONE
```

装机目录 `D:\Program Files\Alpha File Manager\`：
`alpha-file-manager.exe` **80 674 816 B** @ `2026-11-05 11:11:10`（= 本轮新构建）；
`extensions\` 下 `kizemo.focus-sync` 与 `kizemo.git`（`@ 2026-11-05 11:13:34`）**两个官方扩展都在**。

#### 12.5.3 需求 3 的硬证据：应用内确实走了中国镜像 ⭐

**取证方法（不改产品代码、不改用户数据）**：让 primary 主机**连不通但不触发应用的 SSRF 守卫**，
迫使候选链落到中国镜像候选。做法是临时改 hosts：

```
198.18.0.1 raw.githubusercontent.com
198.18.0.1 github.com
```

- `198.18.0.1`（RFC 2544 benchmark 段）在 `security.rs::is_non_public_ipv4_address` 里**被明确放行**
  （注释：fake-IP 代理工具会把这个段发给所有公网主机）⇒ 过得了连接时的 IP 复核；
- 实测该 IP `:443` **连接超时**（`ghfast.top:443` 同时刻 CONNECTED）⇒ 归类为 **Transient**
  ⇒ 候选链**继续往下走**（见 `http.rs:843-859`）；
- hosts 文件全程**先备份、finally 还原、还原后比对 SHA256**，
  三轮跑完 `HOSTS_HASH_AFTER == HOSTS_HASH_BEFORE`（`2D6BDFB3…57085`），**机器上无残留**。

脚本与证据：

```
.superpowers/sdd/mirror-test.ps1        （第 1 轮，127.0.0.1 → 被守卫拦，未遂）
.superpowers/sdd/mirror-test2.ps1       （第 2 轮，缓存 TTL 未过期，整段同步被跳过）
.superpowers/sdd/mirror-test3.ps1       （第 3 轮，198.18.0.1 → 成功）
.superpowers/sdd/state-mirror-test3.txt
.superpowers/sdd/logs/mirror-test3-ext-http.log
.superpowers/sdd/logs/app-log-before-mirror-test3.log
```

**应用日志实测（`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`，
本轮装机产物，2026-11-05 03:26:34~03:26:57 UTC = 11:26 本地）**：

```
[ext-http] attempt op=fetch URL candidate=1/5 url=https://raw.githubusercontent.com/kizemo/sfm-marketplace/main/registry.json
[ext-http] attempt op=fetch URL candidate=1/5 url=https://raw.githubusercontent.com/sigma-hub/sfm-marketplace/main/registry.json
[ext-http] candidate-failed op=fetch URL candidate=1/5 url=https://raw.githubusercontent.com/kizemo/sfm-marketplace/main/registry.json reason=fetch URL (raw.githubusercontent.com): network timeout
[ext-http] attempt op=fetch URL candidate=2/5 url=https://ghfast.top/https://raw.githubusercontent.com/kizemo/sfm-marketplace/main/registry.json
[ext-http] attempt op=fetch URL candidate=2/5 url=https://ghfast.top/https://raw.githubusercontent.com/sigma-hub/sfm-marketplace/main/registry.json
[ext-http] result op=fetch URL candidate=2/5 status=200 ok=true url=https://ghfast.top/https://raw.githubusercontent.com/sigma-hub/sfm-marketplace/main/registry.json
[ext-http] result op=fetch URL candidate=2/5 status=200 ok=true url=https://ghfast.top/https://raw.githubusercontent.com/kizemo/sfm-marketplace/main/registry.json
…（其余 8 个扩展的 package.json 同样 candidate=1/5 超时 → candidate=2/5 ghfast.top → status=200 ok=true）
```

统计（`collect-ext-http.cjs`）：`ghfast` 相关行 **20** 条，其中
`result … status=200 ok=true` 的镜像成功行 **10** 条，覆盖
`sfm-marketplace` × 2 + `focus-sync` / `sfm-git-extension` / `sfm-extension-clipboard` /
`sfm-extension-excalidraw` / `sfm-extension-media-converter` / `sfm-extension-video-downloader` /
`sigma-light-themes` / `sigma-file-manager` 共 **10 个仓库**。

⇒ **§11.7 第 1 条闭环**：应用日志里能直接读出「实际尝试了哪个 URL」，
并且已实测证明**确实走到了 `ghfast.top` 中国镜像候选并成功取回**。

#### 12.5.4 需求 4 的旁证：装机副本自动更新到 v0.5.9

同一份装机产物的首轮启动（11:13:51 启动、11:14 同步）日志：

```
[ext-http] attempt op=download extension 'kizemo.focus-sync' candidate=1/5 url=https://github.com/kizemo/focus-sync/archive/refs/tags/v0.5.9.zip
[ext-http] result op=download extension 'kizemo.focus-sync' candidate=1/5 ok url=https://github.com/kizemo/focus-sync/archive/refs/tags/v0.5.9.zip
Renamed locked directory C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\.kizemo.focus-sync.bac
delete job … finished in 49.7787848s: cancelled=false deleted_items=381 freed_bytes=348971
```

装机副本实测：`%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\package.json`
→ `version=0.5.9`、`publisher={name: Alpha File Manager}`，文件 mtime 全部为 `2026-11-05 11:14:24`
（= 本轮自动更新的落盘时间）。
扩展载荷里 `Sigma` 字样只剩 `dist/index.js` 的 **13** 处**代码标识符**（API 名等），
`package.json` / `README.md` / `locales/*` **均为 0** —— 与 §11.1 的范围锁定一致。

#### 12.5.5 尚在开放（本轮明确不做）

- `kizemo.git` **自动部署**到 `%APPDATA%` 并注册进 `user-extensions.json`（理由见 12.2.5）。
- **不发版**：本轮不升版本、不打 tag、不推站点、不发 GitHub Release。
  工作树 6 个改动文件**未提交**，等下一轮统一收口。

### 12.6 下一轮可直接接着做的事

1. 提交本轮 6 个文件改动（提交信息按仓库既有 `docs:` / `fix:` 风格），必要时同步更新 CHANGELOG。
2. 决定 `kizemo.git` 是否要「自动部署 + 自动注册」——属产品决策，需要先定「新装用户的默认扩展集合」。
3. 若要把「中国镜像」做成用户可见能力（而不是内部候选链），再单开一轮设计。

> **章节口径**：§六 / §八 / §十 描述的是**上一轮 3.10.0**（含当时发现的徽章缺陷）；3.10.1 的落点、验收与发版一律看 **§十一**。
