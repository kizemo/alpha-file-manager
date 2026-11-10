# Handoff — P2 收尾跟进（回收站清理 / release 同步 / 覆盖安装 / push / eslint 清零）

- 日期：2026-11-10
- 项目根：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 上轮 handoff：`docs/handoff-2026-11-10-p2-closeout.md`（§6「遗留待办」1–5 项即本轮执行范围）
- 本轮 HEAD：`55bf310d style: 清零 eslint 门禁错误（177 -> 0，不改运行时行为）`
- 远端：`origin = https://github.com/kizemo/alpha-file-manager.git`（`upstream = https://github.com/aleksey-hoffman/sigma-file-manager.git`，不 push）
- 工作树：干净（`git status --porcelain -uall` 计数 = 0）

---

## 1. 背景与目标

上轮 handoff §6 留了 5 项待拍板事项（清空回收站 / 同步 release / 覆盖安装 / push / eslint 专项）。
本轮用户一次性点名全部执行。本轮全部按推荐方案直接落地，未再发问卷。

---

## 2. 本轮已完成（含证据）

### 2.1 清空 F: 盘回收站

- **清理前枚举**：77 项 `$I*` 元数据，合计 **38.25 GB**。最大项全部是本项目 P2 清理产物：
  `target\debug\incremental` 13,445.20 MB、`target\debug\deps` 12,551.28 MB、
  `target\x86_64-pc-windows-msvc` 4,559.69 MB、`target\debug\build` 2,763.86 MB、
  sidecar `target` 1,317.83 MB、`default-file-manager-launcher` 962.73 MB、
  `superseded` 831.47 MB、`asar-extract` 148.90 MB。
  另有 2 项非本项目条目：`F:\soft\16AI\Cl…` 1,303.03 MB、`F:\soft\00selfmade\pr…` 504.69 MB。
- **执行**：`Clear-RecycleBin -DriveLetter F -Force` ⇒ `CLEAR=OK`。
- **结果**：F: 空闲 **88.62 GB → 122.54 GB（释放 33.92 GB）**。
- **残留**：`$IRSP9E9`（88 B 孤儿 `$I` 元数据桩，记录 `F:\soft\16AI\Cl…` 1.27 GB），
  其对应 `$R` 负载已不存在；重复执行 `Clear-RecycleBin` 仍返回 OK 但清不掉它。**无害残桩，不占空间。**

### 2.2 15:03 新安装包同步进 `release\`（按设计已完成，无需手工覆盖）

关键结论：`scripts/publish-bundle-to-release.mjs` 的跳过条件**不是「按版本号跳过」**，而是：

| 目标状态 | 脚本行为 |
|---|---|
| 目标不存在 | 落 `release\` 顶层 |
| 内容相同 | 不动 |
| **内容不同** | **改写 `release\local-builds\` 并告警**（避免本地重建包冒充 GitHub 已发布资产） |

实测落点：

| 文件 | 大小 | mtime | SHA256 |
|---|---|---|---|
| `release\local-builds\Alpha File Manager_3.13.5_x64-setup.exe` | 45,413,511 B | 15:03:46 | `F7296A1EE0FE729EAEF39BB7ABE1F5C7BFD3C173240879437E2B548C0F21BA11` |
| `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.5_x64-setup.exe` | 45,413,511 B | 15:03:46 | 同上（**逐字节一致**） |
| `release\Alpha File Manager_3.13.5_x64-setup.exe`（顶层） | 45,482,899 B | 0:21:24 | `A94F20A6D43D67DDBBF8449B1B5D0B8A1F77B01D15E722097D6219FDA1E2608F` |

- 顶层那份是**已发布资产镜像**，本轮**故意不覆盖**：覆盖会让本地重建包冒充 GitHub 已发布资产，
  并破坏 `scripts/release-publish.mjs` 的 `latestInstaller()` 口径与 `version.json` 的 sha256 对账。
- `release/` 被 `.gitignore:45`（`/release/`）覆盖 ⇒ 该目录改动不进仓库、`git status` 看不到，属正常。

### 2.3 用新 setup 覆盖安装

- **安装前**：`alpha-file-manager.exe` PID 82656 在跑（`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，
  82,688,000 B @ 01:02:46）；注册表
  `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Alpha File Manager` =
  DisplayVersion `3.13.5` / InstallLocation `D:\Program Files\Alpha File Manager`。
- 会话已提升权限（`IS_ADMIN=True`）；先杀 `alpha-file-manager` + `focus-sync-sidecar`（`APP_LEFT=0`）。
- **守卫逻辑核实**（`src-tauri/installer/early-guard.ps1` + `hooks.nsh`）：
  同版本号时按**安装包自身 SHA256** 与安装日志中最后一次 POSTINSTALL 的 `installerSha256` 比对。
  上次 `f5ad8182…`、新包 `f7296a1e…` ⇒ 属「同版本不同构建」，守卫会放行；`/FORCE` 亦放行。
  NSIS 侧 `StrCmp` 不支持 `*` 通配，故 `/FORCE` 识别在 PowerShell 侧用正则完成。
- **执行**：`Start-Process -FilePath <setup> -ArgumentList '/S','/FORCE' -Wait`
  ⇒ **INSTALL_EXIT=0，耗时 26.1 s**。
- **安装日志**（`$env:TEMP\alpha-file-manager-install.log`）新增 PREINSTALL + POSTINSTALL 两行：
  `sameVersionReinstall=yes`、`instDir='D:\Program Files\Alpha File Manager'`、
  `installerSha256=f7296a1ee0fe729eaef39bb7abe1f5c7bfd3c173240879437e2b548c0f21ba11`。
- **装机产物刷新**：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe` = **82,334,720 B**
  （旧值 82,688,000），mtime 15:03:10；`uninstall.exe` 16:28:49、`tools` 16:28:57。
- **重启验证**：进程存活（PID 223756）；`EnumWindows` 枚举到主窗口 class `Tauri Window`、1936×1048、可见；
  WebView2 子进程 25 个；`FileVersion / ProductVersion` = 3.13.5。

### 2.4 push `2cabae4a` + `1c7b49a4`

- `git -C <repo> push origin main` ⇒ `dadf02fc..1c7b49a4  main -> main`，`PUSH_EXIT=0`。
- 之后 `git status -sb` = `## main...origin/main`（无 ahead / behind）。
- 本轮 eslint 清理（`55bf310d`）+ 本 handoff 另行成 commit 推送，不与上两个 commit 混在一起。

### 2.5 eslint 专项清理：177 errors → 0

- **基线**（HEAD `1c7b49a4` 上量，同一条命令 `eslint .`）：**177 errors / 0 warnings / 24 个文件**，全 severity 2。
  规则分布：`@stylistic/object-property-newline` 86、`padding-line-between-statements` 27、
  `object-curly-newline` 22、`vue/max-attributes-per-line` 9、`@stylistic/arrow-parens` 8、
  `eol-last` 8、`brace-style` 4、`@typescript-eslint/no-require-imports` 4、
  `lines-between-class-members` 3、`@intlify/vue-i18n/no-raw-text` 2、
  `@typescript-eslint/no-unused-vars` 2、`func-style` 1、`@stylistic/max-statements-per-line` 1。
- **自动修复**：`npm run lint:eslint`（= `eslint . --fix`）修掉 **167 条**，
  改动 23 个文件、352 insertions / 85 deletions，**纯格式化**。
- **剩余 10 条定点修复**：

| 文件 | 规则 | 方案 |
|---|---|---|
| `scripts/check-extension-gates.cjs` | `@typescript-eslint/no-require-imports` | `eslint.config.ts` 增加 `files: ['**/*.cjs']` 覆盖块，关闭该规则 |
| `scripts/scan-sandbox-dynamic.cjs` | 同上 | 同上 |
| `src/modules/home/components/__tests__/home-banner-click-opens-background-manager.test.ts` | `func-style` | 箭头函数改函数声明 |
| `src/modules/navigator/components/file-browser/file-browser-thumbnail-size-dialog.vue` | `@intlify/vue-i18n/no-raw-text` | `'✓'` 移入 `<script setup>` 常量 `CHECK_MARK` 后 `{{ CHECK_MARK }}` 渲染 |
| `src/modules/navigator/components/file-browser/file-browser-tree-view.vue` | 同上 | `'!'` 移入常量 `LOAD_ERROR_MARK` 后 `{{ LOAD_ERROR_MARK }}` 渲染 |
| `src/modules/navigator/composables/__tests__/remote-tree-reveal.test.ts` | `@stylistic/max-statements-per-line` | 单行 `await new Promise(...)` 拆两行 |
| `src/modules/navigator/composables/__tests__/use-file-tree.test.ts` | `@typescript-eslint/no-unused-vars` | 去掉未使用的 `const tree =`，**保留 `makeTree({...})` 调用副作用** |
| `src/modules/navigator/composables/use-file-tree.ts` | 同上 | 删除未使用的 `type Ref` 导入 |

- **「不改变运行时行为」的证据**：逐文件 diff 通读并落盘
  （`.superpowers/sdd/eslint-diff-review.txt`、`.superpowers/sdd/eslint-diff-review2.txt`），
  除上表 8 处外**全部是格式化改动**：换行/缩进/模板属性折行/尾行换行/`brace-style`（`} catch {` → `}\ncatch {`）/
  `@stylistic/arrow-parens`（`(p) =>` → `p =>`）。无逻辑改写。
- **终态**：`npm run lint:eslint:check`（= `eslint . --max-warnings 0`）= **EXIT=0，0 errors / 0 warnings**。

### 2.6 全量门禁复核（清理后）

| 门禁 | 命令 | 结果 |
|---|---|---|
| 类型 | `npm run ts`（`vue-tsc --build`） | **EXIT=0** |
| ESLint | `npm run lint:eslint:check` | **EXIT=0（0 errors / 0 warnings）** |
| 单测 | `npm run test:unit:run` | **EXIT=0，299 test files / 3136 tests 全通过**（92.08 s） |
| Changelog | `npm run verify-changelog` | **EXIT=0** |

- 日志：`.superpowers/sdd/gates-2026-11-10.log`。
- eslint 基线 JSON：`.superpowers/sdd/eslint-baseline.json`（177 基线）、
  `.superpowers/sdd/eslint-after-autofix.json`（自动修复后剩余 10 条）。

---

## 3. 已拍板决策（本轮按推荐执行）

| 对象 | 决策 | 理由 |
|---|---|---|
| `release\` 顶层同名安装包 | **不覆盖** | 顶层是已发布资产镜像，覆盖会让本地重建包冒充 GitHub 资产，破坏 `release-publish.mjs` 的 `latestInstaller()` 与 `version.json` sha256 对账。新包已由 publish 脚本落在 `release\local-builds\` |
| 同版本覆盖安装参数 | `/S /FORCE` | 守卫按构建身份（安装包 SHA256）判定，同版本不同构建本就会放行；`/FORCE` 用于确定性。**不传 `/D=`** —— NSIS `/D=` 与模板读注册表的先后顺序不确定，改由 `InstallLocation` + `AlphaNormalizeInstDir` 归一化 |
| 装机 exe 与 `target\release\alpha-file-manager.exe` 的 3 字节差异 | **属设计内，非缺陷** | 差异位于 `__TAURI_BUNDLE_TYPE_VAR_UNKNOWN__` → `__TAURI_BUNDLE_TYPE_VAR_NSIS__`（偏移 72,197,744，3 bytes），是 tauri bundler 在打进 NSIS 的那份副本上改写 bundle-type 标记所致。装机 exe 与 setup 内嵌 payload SHA256 **完全一致**（`1FEBF3BA…`）；`target\release` 那份是未打标的 cargo 原始产物（`2E5B2FB6…`） |
| eslint `.cjs` 的 `no-require-imports` | **config 级豁免**，不改扩展名 | `scan-sandbox-dynamic.cjs` 是 focus-sync 安装链的真实门禁，`installer/register.ps1:109`、`manual-install.ps1:50` 硬编码其 `.cjs` 路径；转 ESM/改名会破坏安装链且无功能收益 |
| `no-raw-text` 的 `'✓'` / `'!'` | **脚本常量 + `{{ }}` 渲染** | 渲染输出逐字符不变；不给 17 个语言包新增纯标点翻译，也不把标点塞进 `ignoreText` 白名单 |
| eslint 清理的提交粒度 | **单独成 commit** | 不与 `2cabae4a` / `1c7b49a4` 混在一起，便于回溯「格式化」与「入库」两类变更 |

---

## 4. 已验证 / 推断 / 未验证

**已验证（有产物或日志）**
- F: 回收站已清空，空闲 88.62 GB → 122.54 GB。
- 15:03 新包已按 publish 脚本设计落在 `release\local-builds\`，与 `bundle\nsis\` 下同名包逐字节一致。
- 覆盖安装 `INSTALL_EXIT=0`（26.1 s），装机 exe 已刷新为 82,334,720 B @ 15:03:10，重启后主窗口正常。
- `dadf02fc..1c7b49a4` 已 push 到 `origin/main`。
- eslint 从 177 errors 降到 **0 errors / 0 warnings**；`vue-tsc --build`、3136 个单测、`verify-changelog` 全绿。
- 本轮 commit `55bf310d` 落地，工作树零残留。

**推断（未直接取数）**
- 3 字节差异的成因：是逐字节定位到 `__TAURI_BUNDLE_TYPE_VAR_UNKNOWN__` / `__TAURI_BUNDLE_TYPE_VAR_NSIS__`
  两个字符串常量后推断出「tauri bundler 在打进 NSIS 的副本上改写 bundle-type 标记」，
  **未反编译打包器实现做独立确认**。但三方 SHA256 对账（装机 exe = setup 内嵌 payload ≠ cargo 原始产物）与之自洽。

**未验证 / 未做**
- **未做真机界面回归**：本轮只改了格式化与 lint 级别缺陷，未改渲染逻辑；判定依据是 3136 个单测全绿
  （含模板渲染断言），**未重跑 `npm run tauri:build` + 装机截图**。
- `docs/archive/` 与 `extensions/kizemo.focus-sync/installer/*.ps1` 中对 `scan-sandbox-dynamic.cjs` 的
  **引用路径已因 2026-11-10 换根而部分失效**（如 `register.ps1:110` 仍写
  `..\..\sigma-file-manager\src\modules\extensions\runtime\sandbox.ts`）—— 属既有遗留，不在本轮范围。
- 回收站残留 `$IRSP9E9`（88 B 孤儿元数据桩）未能清除，无害。
- `CHANGELOG.md` 未新增条目：本轮是纯 lint/格式化清理，无用户可见行为变化，`verify-changelog` 门禁已通过。

---

## 5. 本轮踩到并已坐实的工具坑（下轮直接避）

1. **`Clear-RecycleBin` 对孤儿 `$I` 桩无效**：桩对应的 `$R` 负载已不存在时，
   清空命令返回 `OK` 但桩仍在。不影响磁盘可用量，不要为它反复试探删除命令。
2. **NSIS `StrCmp` 不支持 `*` 通配**：`/FORCE` 这类带开关的判定若依赖 NSIS 字符串比较会失效，
   需在 PowerShell 侧用正则完成。
3. **`eslint .` 不读 `.gitignore`**：`.superpowers/**`、`graphify-out/**`、`_acceptance/**`
   必须写进 `eslint.config.ts` 的 `globalIgnores`，否则门禁数字不可复现
   （同一份代码工作副本 689 errors / 干净检出 177 errors）。
4. **`edit` 工具一次调用只接受一组 `old_string`/`new_string`**：想一次改多处会静默丢改动。
   且 `old_string` 在大文件里可能命中多处（本轮 `use-file-tree.test.ts` 命中 12 次）——
   必须补足上下文让匹配唯一。
5. **C# `Add-Type` 片段里 `uint self` 直接赋 `pr.Id`（int）编译失败** ⇒ 需显式 `(uint)pr.Id`。
6. **`Get-Process ... .MainWindowHandle` 会选到 22×22 隐藏辅助窗口**（沿用上轮坑 #2）：
   主窗口必须 `EnumWindows` + `GetWindowThreadProcessId` 过滤，取可见且 ≥800 宽、`class = "Tauri Window"` 的窗口。
7. `git commit -F -` 不读 stdin（只接受文件名）；消息先落 `.superpowers/sdd/commit-msg-*.txt`。
8. `Set-Location` 后再写/删会被本地安全策略拦 ⇒ 一律 `git -C <repo>`、`npm --prefix <绝对路径>`。
9. 后台命令**不许挂 stdout 管道**（`Tee-Object` / `Select-String`），必须重定向到文件。

---

## 6. 遗留待办（按建议优先级）

1. **真机界面回归（建议做）**：本轮格式化改动涉及 4 个 `.vue` 文件的模板属性折行，
   虽然 3136 单测全绿，但建议下一轮 `npm run tauri:build` 后在装机版上跑一遍目录树 / 缩略图尺寸弹窗 / 右键菜单的实机点击。
2. **focus-sync 安装链的换根路径修复**：`extensions/kizemo.focus-sync/installer/register.ps1:110`
   与 `manual-install.ps1:50` 仍指向换根前的 `..\..\sigma-file-manager\src\modules\extensions\runtime\sandbox.ts`。
   属既有遗留，**改动会触及安装链，需单独排期**。
3. **eslint 基线口径已变**：现在基线是 0。以后量基线仍须用同一条 `eslint . --max-warnings 0` 并在 HEAD 上量。
4. 可选小事：`.superpowers/sdd/setup-extract/alpha-file-manager.exe`（45 MB 级缓存）可删；
   `$IRSP9E9` 孤儿桩（88 B）无害。

---

## 7. 关键文件索引

- 项目宪章：`AGENTS.md`
- 上轮 handoff：`docs/handoff-2026-11-10-p2-closeout.md`、`docs/handoff-2026-11-10-p2-inner-volume-cleanup.md`
- 本轮 handoff：`docs/handoff-2026-11-10-p2-closeout-followup.md`（本文件）
- 打包产物：`src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.5_x64-setup.exe`
- release 镜像：`release\local-builds\`（新包）/ `release\`（已发布资产镜像，故意不覆盖）
- 装机目录：`D:\Program Files\Alpha File Manager\`
- 安装日志：`$env:TEMP\alpha-file-manager-install.log`；守卫日志：`$env:TEMP\alpha-file-manager-early-guard.log`
- eslint 数据：`.superpowers\sdd\eslint-baseline.json`、`.superpowers\sdd\eslint-after-autofix.json`
- 门禁日志：`.superpowers\sdd\gates-2026-11-10.log`
- diff 复核：`.superpowers\sdd\eslint-diff-review.txt`、`.superpowers\sdd\eslint-diff-review2.txt`
- 提交信息文件：`.superpowers\sdd\commit-msg-eslint.txt`
