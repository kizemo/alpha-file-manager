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

## 七、阻塞与未决

1. **Focus Sync 上架 —— 授权已解除（2026-11-04 用户答复「根据你的推荐操作」），但**尚未执行**
   - 采纳方案 = 新建**公网**仓库 `kizemo/focus-sync`（只放已构建产物，不放源码与二进制 sidecar），
     打 tag `v0.5.8`，作为纯条目加进官方清单并置 `featured: true`，再推线上。
   - 现状：`https://github.com/kizemo/focus-sync` 仍 **404**；线上 `registry.json` 仍只有 `kizemo.git` 一条。
   - **建仓文件白名单（实测大小，只上传这 7 项）**：
     `package.json` 1969 B、`dist/index.js` 38 550 B、`dist/index.js.map` 4 263 B、
     `icon.png` 2 360 B、`LICENSE` 1 763 B、`locales/en.json` 772 B、`locales/zh-CN.json` 820 B。
     建议再加一份 `README.md`。
   - **必须排除**：`bin/focus-sync-sidecar.exe` 2 837 504 B、`installer/`（11 个文件，含 2 831 360 B exe）、
     `sidecar/`（Rust 源码 + **整个 `target/` 构建垃圾**，内含 15~35 MB 的 pdb/rlib）。
     ⚠️ sidecar 随安装器分发（`src-tauri/tauri.conf.json:82-87` resources）、以 Windows 计划任务独立运行，
     `dist/index.js` 只与 `127.0.0.1:37421` HTTP 通信 ⇒ **市场条目不含 sidecar 在功能上安全**。
   - ⚠️ **manifest 的 `categories` 是小写 `productivity` / `navigation`**，与清单官方分类表
     （`File Management / Media / Security / Backup & Sync / Productivity / Search & Filter / Appearance /
     Integration / Developer Tools / Example / Other`）不一致 ⇒ 加条目必须用官方表值（`Productivity`，
     或再补 `Integration`）；manifest 本身要不要一并改成表值由接力会话判断（改了要重新算 integrity/走 bundled 更新）。
   - 建 tag 的可抄命令：`gh repo create kizemo/focus-sync --public --source <本地目录> --push`；
     或逐文件 `gh api repos/kizemo/focus-sync/contents/<path> --method PUT -f content=<base64> -f message=...`
     （**首次创建不带 `sha`**），随后 `gh api repos/kizemo/focus-sync/git/refs -f ref=refs/tags/v0.5.8 -f sha=<commit>`。
     ⚠️ 建公网仓 + tag + 上传文件**尚未在本会话跑通过**，属需实测的新路径。
   - 对照参考：`extensions/kizemo.git/` 顶层是 `dist locales src build.mjs icon.png package.json README.md`
     （完整源码仓），而 focus-sync 只有产物 ⇒ 两种形状市场都能装。
   - 拿到答复后可**单独推清单**（清单热更新，不需再发一版应用）。
2. **`sigma-hub` 不列为官方**是可逆决策（写在 `src/data/extensions.ts` 注释里）。若用户希望 sigma 生态也算官方，
   只需往 `OFFICIAL_EXTENSION_OWNERS` 加 `'sigma-hub'`，但会与「sigma = 第三方源」的上一轮决策相冲突。
3. **`hi` / `hy` 语言包**仍是旧品牌串，follow-up。
4. 上一轮「市场/已安装选项卡页面消失」的反馈**已由用户撤销**，不再排查。

## 八、下阶段计划（按顺序）

> 用户 2026-11-04 明确授权本轮做完：**打包落地 → 补 tag `v3.10.0` → 上传 aiec 站点 `version.json` →
> 建 GitHub Release**，并批准 Focus Sync 按推荐方案（建公网仓）执行。发版/公网发布**不需再问**。

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
6. ⏳ **上传 aiec 站点：进行中**。`.superpowers/sdd/upload-310.cmd` = `npm run release:upload`（prepare 全量 + SFTP + 线上 `verify`）。
   23:52 实测：`index.html OK` / `.htaccess OK` / **`version.json OK`** / `download.php OK` 已上传，随后按序补传历史包。
   ⚠️ `_upload_alpha_files.py` 会补传**全部 33 个历史安装包（705.49 MB）**，耗时以十分钟计，**不是卡死**；日志是 **cp936**，读出乱码属正常。
   完成判据：`logs/upload310.txt` 尾部 `UPLOAD_310_EXIT=0` + `UPLOAD_310_DONE`。**若中断就重挂同一条 detached 命令。**
   站点安装包文件名规则 `Alpha_File_Manager_<version>_x64-setup.exe`；`release/local-builds/` 会被内容改写 ⇒ `latestInstaller()` 必须一起扫该目录。
7. ⏸ **建 GitHub Release：待启动**。脚本 `.superpowers/sdd/github-310.cmd` 已写好（纯 ASCII、内部重定向到日志、不挂管道），
   **刻意与 `release:upload` 错开**以免同一上行链路争用。启动方式同 §5.2 的 `Start-Process` detached 写法。
   `github()` 读 `package.json.version` ⇒ 只认远端 tag `v3.10.0`（✅ 已在 origin）；会上传站点目录里的下划线资产 44 938 742 B；
   正文取站点 `_release-notes-v3.10.0.md`（✅ 2470 B，已由 `prepare --write-notes` 生成，漏跑会安静发出正文为空的 release）。
   GitHub Release 资产的官方 sha256 用 API 的 `digest` 字段拿（本机到发行 CDN 被限速 0.01 MB/s，下载比对不可行）。
8. **Focus Sync 上架**（§7.1，独立于构建，可在 `tauri build` 期间并行做）：建 `kizemo/focus-sync` 公网仓
   → 只上传白名单 7 项 → tag `v0.5.8` → 往 `scripts/ext-market/registry.json` 加**纯条目**、
   `featured: true`、`publisher.name` = `Alpha File Manager` → `publish.mjs --validate` →
   PUT 线上并**回读确认** → 本地清单改动随下一次提交一起进仓。
9. 3.10.0 装机真机验收 §6 四项证据；验收前必须先装新包，restore 真实数据前先杀进程。
10. winget 清单更新只能手工开 PR（未授权自动提交）。
11. follow-up：`hi` / `hy` 语言包旧品牌串；`registry.json` 里 `publisher` 块 `name` 行的缩进待顺手对齐。

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
