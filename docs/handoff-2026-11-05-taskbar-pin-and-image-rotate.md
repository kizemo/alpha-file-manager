# 交接文档 · 任务栏固定持久化 + 图片旋转翻转右键菜单（2026-11-05 接力）

> **本轮只做交接**。两项需求的实现、真机验收、打包发版（commit / push / tag / 站点 / GitHub Release）
> 全部由**新会话**执行。本文件提供背景、目标、根因（源码级）、当前进度、下阶段计划、证据路径与工具坑，
> 供新会话直接开工，**不需要重新摸索**。
>
> 相关文档：
> - 项目硬约束：`F:\soft\00selfmade\filemanager\AGENTS.md` §2
> - 上一轮（§11.7 七条 follow-up 收口 + 需求 3 镜像硬证据）：
>   `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md`（§11.7 遗留清单 L575–583；§12 本轮收口 L598–L965）

---

## 一、需求原文与出处

用户 2026-11-05 提出（原文照录）：

1. 「在任务栏设置了 AFM 固定在任务栏，但每次更新后，固定在任务栏的图标都会消失。希望如果设置过在任务栏固定，更新后不要消失」
2. 「在目录视图，希望在图片格式的文件，右键菜单添加『向左旋转、向右旋转、左右翻转、上下翻转』四个菜单，且点击之后，能自动保存对图片的编辑，不需要确认。」

并追加：改完后**通过验收 → 打包发版 → commit、push → 改 README 与更新日志 → 更新 aiec 的版本信息**；
且明确「**上述任务请在新会话完成**」。

---

## 二、背景与约束（动手前必须知道）

- 项目：**Alpha File Manager**（AFM），Tauri 2 + Vue 3 桌面文件管理器，仓库 `F:\soft\00selfmade\filemanager\sigma-file-manager`。
- 品牌名统一 **Alpha File Manager**；**受保护常量**（mDNS 服务名、legacy 数据目录名、Tauri `identifier=com.sigma-file-manager.app`）不得改动。
- 真机验收**必须用 `tauri build` 的打包产物**，不能用 `tauri dev`（dev 构建的路由页面内容区全空白，界面结论一律作废）。
- **本轮不发版**（本文件只是交接）。版本号当前 `3.10.1`，新会话建议升 **3.11.0**（本轮新增两个用户可见功能）。
- 第三方扩展自带的 "Sigma File Manager" 文案、`engines.sigmaFileManager`、API 名 `sigma.ui` 不在本轮范围。

---

## 三、目标与验收判据（可验收清单）

### 需求 1 · 任务栏固定在更新后不消失

| # | 判据 | 说明 |
|---|---|---|
| 1.1 | **手动双击新版 setup.exe 升级后，任务栏固定仍在** | 主路径，最容易复现 |
| 1.2 | **应用内更新通道升级后，固定仍在** | 次路径，与 1.1 同根因、需单独验证 |
| 1.3 | 开始菜单快捷方式**没有被删掉重建**（`UnpinShortcut` 未被调用） | 用快捷方式文件的 `System.AppUserModel.ID` 与任务栏固定项对照 |
| 1.4 | 升级后应用图标**仍是新版**（快捷方式 `IconLocation` 指向 `$INSTDIR\alpha-file-manager.exe`，路径不变即自动跟随） | 与固定消失是两件事，别混判 |
| 1.5 | **卸载**（非升级）时仍会摘掉固定 | 这是**期望行为**，不要被「修好了」误伤；回归要测 |

### 需求 2 · 图片文件右键菜单旋转 / 翻转，点了直接保存

| # | 判据 | 说明 |
|---|---|---|
| 2.1 | 目录视图里，**图片文件**的右键菜单出现「向左旋转 / 向右旋转 / 左右翻转 / 上下翻转」四项 | 非图片文件**不得**出现这四项 |
| 2.2 | 点击后**直接写回原文件**，**不弹确认框** | 用户明确要求「不需要确认」 |
| 2.3 | 旋转 90° 后文件**尺寸互换**、像素方向正确 | 用自造的靶子图片验，别拿用户真实照片 |
| 2.4 | 翻转后像素左右/上下镜像正确 | 同上 |
| 2.5 | 保存后**列表缩略图 / 信息面板预览随之刷新** | 若不刷新，是缓存问题，需处理 |
| 2.6 | 非图片、目录、远程位置、虚拟位置**不出现或被正确拦截** | 走既有动作门禁 |
| 2.7 | **SVG 不出现这四项** | `image` crate 不吃 SVG（见 §4.2） |

---

## 四、根因（已验证到源码级）

### 4.1 需求 1：固定为什么会在更新后消失

**因果链（全部可在 `src-tauri/installer/installer.nsi` 逐行对上）**：

```
用户双击新版 setup.exe（或应用内更新下载后用 shell 打开，见下）
  └─ 新安装器 .onInit 读 /UPDATE          installer.nsi:552-554  →  $UpdateMode = 0
       └─ 检测到旧安装 → 调旧 uninstall.exe installer.nsi:373 / :407（ExecWait）
            └─ 只有当「新安装器自己」带 /UPDATE 时，才给旧卸载器追加 /UPDATE
                                                   installer.nsi:377
               ${IfThen} $UpdateMode = 1 ${|} StrCpy $R1 "$R1 /UPDATE" ${|}
               ⇒ 手动双击 / 应用内更新这条路：$UpdateMode = 0 ⇒ 旧卸载器【没有】/UPDATE
                    └─ 旧 uninstall.exe un.onInit 读 /UPDATE   installer.nsi:834-836 → UpdateMode = 0
                         └─ Uninstall 段：${If} $UpdateMode <> 1        installer.nsi:887
                              ├─ !insertmacro DeleteAppUserModelId     installer.nsi:888
                              ├─ !insertmacro UnpinShortcut  <lnk>      installer.nsi:895  ← 任务栏固定被摘掉
                              └─ Delete "$SMPROGRAMS\...\Alpha File Manager.lnk"  installer.nsi:896
```

同时 `CreateOrUpdateStartMenuShortcut`（`installer.nsi:978-1018`）在 `$UpdateMode = 1` 时 **L1004-1006 直接 `Return`**（连快捷方式都不重建）；
非 update 模式下则走 L1012-1013 重建快捷方式 + `SetLnkAppUserModelId`。

⇒ **两条路都指向同一个结论：固定消失不是「图标变了」，而是「快捷方式被删 + AUMID 被清 + 主动 Unpin」。**

**应用内更新通道同源**：
`src/modules/app-updater/composables/use-app-updater.ts:254` `invoke('download_release_installer', …)` 下载 →
同文件 `:306` `await openPathDefault(downloadedPath)` 用**系统默认方式打开安装包** ⇒ 等价于手动双击 ⇒ **不带 `/UPDATE`**。

**为什么不能靠「改旧卸载器」解决**：`src-tauri/installer/installer.nsi` 头部 AFM PATCH 注释（L4–L18）写明——
升级路径调用的是**已发布的旧卸载器二进制**，无法更改；这也是本仓 vendored 这份 Tauri v2.12.0 模板的唯一原因。

**Tauri utils.nsh 里的三个宏**（`src-tauri/target/.../utils.nsh` 或 `.superpowers/sdd/tauri-utils.nsh` 可对照）：
`SetLnkAppUserModelId`（L77–103，写 `PKEY_AppUserModel_ID`）、`DeleteAppUserModelId`（L105–120）、`UnpinShortcut`（L121+）。

**修复方向（择一或组合，新会话自行定夺）**：

- **A（推荐，主路径）** 改 vendored 模板 `installer.nsi:377`：**只要检测到旧安装且是同一产品，就无条件给旧卸载器追加 `/UPDATE`**，
  不再依赖「新安装器自己带 /UPDATE」。⇒ 手动双击升级也保留快捷方式与固定。
  ⚠️ 该文件是 vendored 模板，改动必须同步 `src-tauri/installer/__tests__/nsis-template-patch.test.ts`（该测试守护 AFM PATCH 的存在与位置），
  并按仓库惯例补一条新补丁的断言；改完**必须真跑一次打包**（NSIS 语法错只在打包时炸）。
- **B（应用内更新通道）** 新增一个 Rust 命令，用 `ShellExecuteW` 带 `/UPDATE` 参数启动下载好的安装包；
  改动面比 A 小、只覆盖应用内更新，但**手动双击那条路仍是坏的** ⇒ 建议 A+B 一起做。
- **C（兜底，不解决 unpin）** POSTINSTALL hook 里重写一次带同一 AUMID 的 `.lnk` —— 只恢复快捷方式，**恢复不了任务栏固定**（Windows 没有公开的「固定」API）⇒ 不能单独作为方案。

**动手前必须先核对的两个事实**：
1. `SetLnkAppUserModelId` 写进快捷方式的 AUMID 是否**稳定**（跨版本不变）。若 AUMID 变了，固定必然丢，与 unpin 无关。
2. 运行时是否设置了与快捷方式一致的 AUMID（Tauri v2 默认会设 `SetCurrentProcessExplicitAppUserModelID`）。
   ⇒ 这两条要在真机上量，不能靠猜。

### 4.2 需求 2：四个菜单项落在哪、能不能不加依赖

**好消息：Rust 侧不需要新增依赖。** `src-tauri/Cargo.toml` 已有：

```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "gif", "bmp", "webp", "tiff", "ico"] }
```

⇒ `image::imageops::rotate90 / rotate180 / rotate270 / flip_horizontal / flip_vertical` 直接可用，按原格式写回即可。

**前端复用点（都别新造）**：

| 用途 | 现成实现 | 备注 |
|---|---|---|
| 判断是否图片 | `isImageFile` — `src/modules/navigator/components/file-browser/utils.ts` | 已在剪贴板工具栏、信息面板多处复用 |
| 菜单外壳 | `src/components/dir-entry-interactive/dir-entry-context-menu.vue`（142 行） | 渲染 `FileBrowserActionsMenu` + `FileBrowserExtensionMenuItems` |
| 内置动作清单 | `src/modules/navigator/components/file-browser/file-browser-actions-menu.vue` | 右上角那一排纯图标按钮 + 菜单项都在这里 |
| **可见性机制（按文件类型过滤的正确落点）** | `src/core/actions/context-menu-adapter.ts`：`ContextMenuActionInput` / `buildContextMenuActionRegistry` / `isContextMenuActionAvailable(action, visibility)` | 「只在图片上出现」应走这里，**不要在模板里 `v-if` 硬编码** |
| 动作类型联合 | `src/modules/navigator/components/file-browser/types.ts` 的 `ContextMenuAction` | 新增 4 个动作要进这个联合类型 |
| 动作执行中心 | `src/composables/use-dir-entry-actions.ts:638 handleContextMenuAction(action, entries)` | |
| **写类动作门禁** | `src/utils/entry-action-policy.ts` `isActionBlockedByEntryPolicy` | 新增写动作**必须**过门禁（远程 / 虚拟位置 / 系统盘挂载根） |
| SVG 排除 | `src/utils/resolve-image-display-src.ts:7` `ORIGINAL_ONLY_IMAGE_EXTENSIONS = new Set(['svg'])` | `image` crate 不吃 SVG ⇒ SVG 不能进这四项 |
| 现成图片预览 | `src/modules/navigator/components/info-panel/composables/use-info-panel-image-preview.ts` | 保存后刷新预览可能复用它 |

**「自动保存不确认」的实现要点**：
- 旋转/翻转后**直接写回原路径**，不弹对话框；失败要出 toast（读/写失败、文件被占用、只读）。
- **待定的产品决策**（新会话要定，见 §七 开放项）：是否保留 `.bak` 原图备份、JPEG 重编码质量取值、是否支持撤销。
- 缩略图缓存要失效（`src/modules/settings/ui/categories/storage/thumbnail-cache.vue` 有相关设置）。

---

## 五、当前进度

### 5.1 本轮（本文件这一轮）做了什么

- ✅ 定位两条需求的**源码落点**与**根因**（§四），全部有行号可对。
- ✅ 取齐发版链路锚点（§六）。
- ❌ **没有改任何产品代码**；工作树仍只有上一轮的 7 个未提交文件（见 §5.2）。
- ❌ 没有打包、没有装机、没有发版。

### 5.2 ⚠️ 工作树现状：上一轮 7 个文件**未提交**（发版前必须先收口）

`git status --porcelain`（HEAD = `a2b4e4ad`，tag `v3.10.1`，与 `origin/main` 同步）：

```
 M docs/handoff-2026-11-04-extension-official-featured-branding-and-release-310.md   （已回填 §12）
 M src-tauri/src/extensions/http.rs                                                 （需求 3 镜像取证日志）
 M src-tauri/tauri.conf.json                                                        （kizemo.git 进 bundled resources）
 M src/localization/messages/hi.json                                                （旧品牌串）
 M src/localization/messages/hy.json                                                （旧品牌串）
 M src/modules/settings/ui/categories/extensions/extension-settings.vue             （扩展语言包 BCP-47）
 M src/utils/__tests__/module-load-recovery.test.ts                                （jsdom Storage Proxy 修复）
```

`git diff --stat` = **7 files, 495 insertions(+), 39 deletions(-)**。
这批改动的门禁数字（已验证）：`vitest` 284 文件 / 2648 用例全过；`eslint .` **186 errors / 0 warnings**（= 3.10.1 基线逐字相同，6 个改动文件 0 违规）；
`cargo test --lib` **919 passed**（基线 917 + 新增 2）。

⇒ **新会话第一步：把这 7 个文件单独提交**（否则新需求的 diff 会和它们混在一起，commit / 发版都说不清）。
上一轮的完整结论、证据路径、工具坑见
`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md` §12。

### 5.3 上一轮已闭环、无需重做的事

- 需求 3「中国镜像」的应用内硬证据**已取到**（应用日志里能读出实际尝试的 URL，实测走到 `ghfast.top` 候选并 `status=200 ok=true`）。
- 3.10.1 已发布并对外生效（站点 + GitHub Release），**不要重做**。
- follow-up 5 的 `kizemo.git` 已进 bundled resources 并装机验证；「自动部署 + 自动注册」仍开放（属产品决策）。

---

## 六、发版链路（照抄即可，别重新摸索）

| 环节 | 命令 / 路径 | 备注 |
|---|---|---|
| 版本三处一致 | `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` | 由 `npm run sync-version`（`scripts/sync-version.js`）同步，**三处必须一致** |
| 打包 | `npm run tauri:build` | = `sync-version` + `tauri build` + `publish-bundle-to-release.mjs`（**只复制到 `release/`，不上传**） |
| 发布编排 | `npm run release:status` / `release:prepare` / `release:upload` / `release:verify` / `release:github` | `scripts/release-publish.mjs` |
| 更新日志校验 | `npm run verify-changelog` | `scripts/verify-changelog.mjs`：会校验 `CHANGELOG.md` 与 `public/changelog/<locale>/changelog.md`（**14 个语言目录**：ch/de/es/fa/fr/hi/it/ja/pt/ru/sl/tr/ur/vi）同步 ⇒ **改一处必须同步另一处** |
| README | `README.md` + `README.zh-CN.md` | 两个都要改 |
| **aiec 站点版本信息** | `E:\办公文件\L1网站\alpha-file-manager\`：`version.json` + `_release-notes-vX.Y.Z.md` + 安装包 | Rust 侧 `src-tauri/src/app_updater.rs:22` `UPDATE_MANIFEST_URL = "https://www.aiec.fun/alpha-file-manager/version.json"` 拉这个清单 |
| GitHub Release | `gh release create vX.Y.Z …`（仓库 `kizemo/alpha-file-manager`） | 资产名 `Alpha_File_Manager_<ver>_x64-setup.exe` |
| winget | `npm run check:winget`（`docs/winget/check-manifest.mjs`） | 清单在 `docs/winget/` |
| 应用内更新清单 | `src-tauri/src/app_updater.rs` 的常量与解析逻辑 | 改站点 version.json 形状要同步这里 |

**建议版本号：3.11.0**（新增两个用户可见功能 → minor）。版本号改完立刻 `git diff` 核对三处一致。

---

## 七、下阶段计划（新会话按顺序执行即可）

### 阶段 0 · 收口上一轮遗留
1. 提交 §5.2 的 7 个文件（单独一个 commit），必要时同步 `CHANGELOG.md`（走 `npm run verify-changelog` 校验）。
2. 确认 `git status --porcelain` 干净、`origin/main` 已同步。

### 阶段 1 · 需求 1（任务栏固定持久化）
3. 先在真机上**复现**「升级后固定消失」，并确认 §4.1 里那三个待核事实（AUMID 稳定性 / 运行时 AUMID / `UnpinShortcut` 是否真被调用）。
4. 实现（建议 A+B 组合），同步更新 `src-tauri/installer/__tests__/nsis-template-patch.test.ts`。
5. 门禁：`npm run verify-nsis-warnings.ps1`（`scripts/verify-nsis-warnings.ps1`）+ 真跑 `npm run tauri:build`（NSIS 语法错只在打包时暴露）。

### 阶段 2 · 需求 2（图片旋转 / 翻转）
6. Rust 侧：新增命令（读图 → `image::imageops` → 按原格式写回），**不加新依赖**；错误要能回传（只读 / 被占用 / 解码失败）。
7. 前端侧：4 个动作进 `ContextMenuAction` 联合类型 → 在 `core/actions/context-menu-adapter.ts` 的可见性机制里加「仅图片」谓词（复用 `isImageFile`，排除 SVG）→ 在 `file-browser-actions-menu.vue` 渲染 → 执行走 `use-dir-entry-actions.ts` 的既有链路 → **必须过 `isActionBlockedByEntryPolicy` 门禁**。
8. 文案：`src/localization/messages/**` 共 **17 个语言包**（中文 locale code 是 `ch` 不是 `zh`），至少中英文补齐，其余按仓库既有惯例处理。
9. 缩略图 / 预览刷新：保存后失效对应缓存。

### 阶段 3 · 门禁与验收
10. 门禁全量（口径见 §八）。
11. **真机验收**（必须用 `tauri build` 产物，不得用 `tauri dev`）：
    - 需求 1：装机 → 固定任务栏 → 升级 → 固定仍在（手动双击 + 应用内更新两条路都测）。
    - 需求 2：**自造靶子图片**（别用用户真实照片）→ 右键旋转 → 文件尺寸/像素方向正确 → 无确认弹窗 → 缩略图已更新。
12. 破坏性验收的靶子必须自造；验收后清理。

### 阶段 4 · 发版
13. 版本号三处 → 3.11.0；`npm run sync-version`。
14. `npm run tauri:build`（**后台跑**，`.superpowers/sdd/*.cmd` 纯 ASCII、内部重定向到日志文件）。
15. 静默装机 + 验收（可复用 `.superpowers/sdd/accept-3102b.cmd` 的形状，**改名新写**，不要在运行中编辑 `.cmd`）。
16. commit + push + tag `v3.11.0`。
17. 站点：上传安装包 + 更新 `version.json` + 新增 `_release-notes-v3.11.0.md`。
18. GitHub Release：`gh release create v3.11.0 …`，核对资产大小与 sha256。
19. README.md / README.zh-CN.md / `CHANGELOG.md` / `public/changelog/<14 个语言>/changelog.md` 全部同步；`npm run verify-changelog` 过。
20. winget 清单（如沿用本轮惯例）。
21. 回填本文件（新增一节，记录本轮结论、证据路径、工具坑）。

### 开放项（需要产品判断，新会话要拿主意）
- 需求 2：是否保留原图 `.bak` 备份？JPEG 重编码质量取值？是否要撤销？
- 需求 1：方案 A 会让「升级」路径**始终**保留快捷方式与固定 —— 需确认这与「卸载应摘掉固定」的期望不冲突（判据 1.5）。
- 上一轮遗留：`kizemo.git` 是否「自动部署 + 自动注册」（会改变新装用户的默认扩展集合）。

---

## 八、门禁口径（照抄，勿改口径）

```
npx eslint .        →  基线 186 errors / 0 warnings（3.10.1 基线）；判据 = 总数不高于基线 + 新改文件 0 违规
                     （eslint 不读 .gitignore，量的时候要 `git stash` 量 HEAD，别拿工作副本的数字对）
npm run test:unit:run →  vitest 全量（上一轮 284 文件 / 2648 用例）
cargo test --lib    →  上一轮 919 passed；测试二进制**不得**链接到 tauri::AppHandle / tauri::async_runtime::*
npm run verify-changelog → CHANGELOG 与 14 个语言目录的 changelog.md 同步
```

---

## 九、已验证 / 推断 / 未验证（三分法，交接必须诚实标注）

### 已验证（本轮实测或源码逐行核对）
- `installer.nsi:377` 的 `/UPDATE` 追加**以新安装器自身的 `$UpdateMode` 为条件**；`:887` 的快捷方式删除 + `UnpinShortcut` **以旧卸载器的 `$UpdateMode <> 1` 为条件**；`:1004-1006` update 模式下 `CreateOrUpdateStartMenuShortcut` 直接 `Return`。
- 应用内更新是 `openPathDefault(downloadedPath)` 打开下载好的安装包（`use-app-updater.ts:254 / :306`），**不带 `/UPDATE`**。
- `image` crate 0.25（含 png/jpeg/gif/bmp/webp/tiff/ico）已在 `src-tauri/Cargo.toml` 依赖里。
- `isImageFile` 存在于 `src/modules/navigator/components/file-browser/utils.ts`；`ORIGINAL_ONLY_IMAGE_EXTENSIONS = new Set(['svg'])` 在 `resolve-image-display-src.ts:7`。
- 菜单可见性机制在 `src/core/actions/context-menu-adapter.ts`；动作门禁在 `src/utils/entry-action-policy.ts`。
- 工作树 7 个未提交文件、`git diff --stat` 数字、版本三处均为 `3.10.1`、HEAD `a2b4e4ad`。
- 上一轮门禁数字：vitest 284/2648、eslint 186/0、cargo 919。

### 推断（源码一致，但**未在真机上复现**，新会话需实测确认）
- 「固定消失」的完整因果链（手动双击 与 应用内更新 两条路都会触发 `UnpinShortcut`）——**逻辑上闭合，但本轮没在真机升级复现过**。
- `SetLnkAppUserModelId` 写入的 AUMID 是否跨版本稳定（若不稳定，固定必丢且与 unpin 无关）。
- 运行时 AUMID 是否与快捷方式一致（Tauri v2 默认会设，但**未核**）。
- `UnpinShortcut` 在本机 Windows 版本上的实际效果（不同 Win10/Win11 行为可能不同）。

### 未验证（下一轮才成立）
- 3.11.0 的任何数字（构建产物大小、sha256、门禁计数、装机结果）。
- 需求 1 / 需求 2 的任何实现方案与 UI 效果。
- 站点 `version.json` 的新形状与 GitHub Release 资产核对结果。

---

## 十、证据文件路径

**本轮（交接）新增**
- 本文件：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-05-taskbar-pin-and-image-rotate.md`

**上一轮（§12 已在 handoff 文档内，摘录关键路径）**
- 上一轮 handoff：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md`（§12 = L598–L965）
- 门禁脚本：`.superpowers\sdd\gates-3102.cmd` / `.superpowers\sdd\state-3102.txt`
- 打包脚本：`.superpowers\sdd\build-3102.cmd` / `.superpowers\sdd\state-build-3102.txt` / `.superpowers\sdd\logs\build-3102.log`
- 装机脚本：`.superpowers\sdd\accept-3102b.cmd` / `.superpowers\sdd\state-accept-3102.txt`
- 镜像取证：`.superpowers\sdd\mirror-test{,2,3}.ps1` / `.superpowers\sdd\state-mirror-test3.txt` / `.superpowers\sdd\logs\mirror-test3-ext-http.log`
- 应用日志（Rust 侧落盘）：`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`（时间戳是 **UTC**，比本地少 8 小时；应用运行时该文件被独占锁，用 `File.Open(...,'FileShare.ReadWrite')` 或 node 读）

**本轮要重点看的源码锚点**

| 主题 | 路径 |
|---|---|
| NSIS 模板（vendored，唯一有 AFM PATCH） | `src-tauri\installer\installer.nsi`（1041 行；AFM PATCH 在 L382） |
| NSIS 钩子 | `src-tauri\installer\hooks.nsh`（1207 行；PREINSTALL / POSTINSTALL） |
| NSIS 补丁守护测试 | `src-tauri\installer\__tests__\nsis-template-patch.test.ts` |
| NSIS 告警门禁 | `scripts\verify-nsis-warnings.ps1` |
| 应用内更新（前端） | `src\modules\app-updater\composables\use-app-updater.ts`（L254 / L306） |
| 应用内更新（Rust） | `src-tauri\src\app_updater.rs`（L22 清单 URL） |
| 右键菜单外壳 | `src\components\dir-entry-interactive\dir-entry-context-menu.vue` |
| 内置动作清单 | `src\modules\navigator\components\file-browser\file-browser-actions-menu.vue` |
| 菜单可见性机制 | `src\core\actions\context-menu-adapter.ts` |
| 动作执行中心 | `src\composables\use-dir-entry-actions.ts`（L638） |
| 动作门禁 | `src\utils\entry-action-policy.ts` |
| 图片判定 / SVG 排除 | `src\modules\navigator\components\file-browser\utils.ts` / `src\utils\resolve-image-display-src.ts` |
| Rust 图片能力 | `src-tauri\Cargo.toml`（`image` 0.25） |

---

## 十一、本轮工具坑（新增 3 条，其余沿用上一轮 §12.4 的 10 条）

1. **`installer.nsi` 是 vendored 的 Tauri 模板**：改它要同步 `__tests__/nsis-template-patch.test.ts`，且**必须真跑一次打包**才算数 ——
   NSIS 语法错（宏未定义、`${If}` 配错）**只在打包时炸**，前端/单测全绿也发现不了。
2. **本仓的 `.cmd` / `.ps1` 必须纯 ASCII**（cmd.exe 按系统 ANSI 代码页读，中文注释会变乱码并被当命令执行）。
   `.cmd` 的 `echo` 里 `>` 前**必须留空格**，否则 `%ERRORLEVEL%` 为 `0` 时会被解析成 stdin 重定向 `0>`。
3. **长编译后台跑**：`.superpowers/sdd/*.cmd` 内部用 `> logfile 2>&1` 重定向到文件，**不要挂 stdout 管道**（`Tee-Object` / `| Select-String` 一挂，进程树跟着死）；
   **脚本运行期间禁止编辑它**（cmd 按字节偏移逐行读，改了会执行错位的指令片段）—— 要改先停任务，再**新建一个文件名**重跑。

---

## 十二、一句话 prompt（交给新会话）

> 接手 Alpha File Manager 的「任务栏固定持久化 + 图片旋转翻转右键菜单」两项新需求的实现、真机验收与 3.11.0 发版：
> 先完整读 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-05-taskbar-pin-and-image-rotate.md`
> （背景 / 需求原文 / 根因与源码锚点 / 当前进度与未提交改动 / 下阶段计划 / 门禁口径 / 已验证·推断·未验证 / 证据路径 / 工具坑），
> 再读 `F:\soft\00selfmade\filemanager\AGENTS.md` §2 的项目硬约束与真机验收要点，
> 以及上一轮交接 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md` 的 §11.7 与 §12；
> 掌握背景、目标、进度与下阶段计划后按该 handoff 的阶段 0→4 执行，自己决定实现细节，每个验收点都要给可核对的证据。
