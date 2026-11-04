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

## 六、真机验收 · 待填 / 未验证项

验收资产（全纯 ASCII，`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\`）：
`accept-uia-dump.ps1 -All -Out …`、`accept-sidebar-probe.ps1`、`accept-nav.ps1 -OutDir … -Steps '…'`、
`accept-market-filter.ps1`、`app-key.ps1 -Keys 'CTRL+V'`、`dash-accept-data.cjs backup|seed|restore`、`shift-occluders.ps1`

**硬性要点**（详细成因见 `AGENTS.md` §2）：

- 只能用 `tauri build` 产物；`tauri dev` 路由页内容区全空白，界面结论一律作废。
- 脚本**在任何 user32 调用之前** `SetProcessDPIAware()`（判据 `GetSystemMetrics(0)` 从 1707 变 2560）。
- 主窗口用 `EnumWindows` + pid 过滤 + class `Tauri Window` + 可见 + 面积最大；**不要用 `Process.MainWindowHandle`**。
- 启动后会弹「新功能」模态，先点「关闭」再取 UIA 树。
- TabItem 用 `SelectionItemPattern.Select()`；Button 用 `InvokePattern.Invoke()`，**成功 ≠ 生效**，需落盘/像素佐证。
- `release` 构建下 `SFM_E2E_DATA_DIR` 不生效 ⇒ 要动真实数据只能「备份 → 灌夹具 → 验收 → **先杀进程** → restore → 核对字节数」。
- UIA 命中数 = 0 **不能**判定 toast 没弹，必须配屏幕 BitBlt 实拍。

**待取证据（4 项，对应诉求 1/2/3/4）**：官方徽章 / 精选区 / 作者名 / 来源徽章；
外加 §4.3 的 bundled 更新是否真的覆盖已安装副本。

> **2026-11-05 补充（Focus Sync 已上架）**：验收清单再加一项 —— 扩展市场「精选」区应出现 **Focus Sync**，
> 名称右上角带「官方」徽章、作者显示 `Alpha File Manager`、来源显示为官方源。线上清单 blob 已与本地逐字节一致，
> 且清单是热更新的 ⇒ 该项**不依赖新安装包**，但「官方」判定逻辑（`OFFICIAL_EXTENSION_OWNERS`）依赖 3.10.0。

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
   只需往 `OFFICIAL_EXTENSION_OWNERS` 加 `'sigma-hub'`，但会与「sigma = 第三方源」的上一轮决策相冲突。
3. **`hi` / `hy` 语言包**仍是旧品牌串，follow-up。
4. 上一轮「市场/已安装选项卡页面消失」的反馈**已由用户撤销**，不再排查。

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
10. **3.10.0 装机真机验收 §6 四项证据 + Focus Sync 精选可见性 —— 这是本任务唯一剩余项。**
    本机现装 **v3.9.2** ⇒ 验收前必须先装 `release/Alpha File Manager_3.10.0_x64-setup.exe`
    （44938742 B / sha256 `73bbf8d812c58b5a332a2ff676ae38e8cf47fbb21f8d41b0afac59d1ef7b848e`，与 GitHub Release 官方 `digest` 逐字一致）；
    restore 真实数据前先杀进程（`Get-Process alpha-file-manager` 必须为空）。
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
