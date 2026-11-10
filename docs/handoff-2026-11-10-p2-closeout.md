# Handoff — P2 收尾（体积治理 + 入库 + 打包实机验证）

- 日期：2026-11-10
- 项目根：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 上游会话 handoff：`docs/handoff-2026-11-10-p2-inner-volume-cleanup.md`、`docs/handoff-2026-11-10-root-unification.md`
- 本轮 HEAD：`2cabae4a chore: 换根收尾适配与历史归档入库`（**未 push**）
- 工作树：干净（`git status --porcelain -uall` 计数 = 0，本文件提交前）

---

## 1. 背景与目标

上一轮完成了「根目录统一（外层 → 内层 `sigma-file-manager\`）」和「P2 内层体积清理」，
留下 4 项需要拍板的待办 + 2 项未验证项。本轮用户授权「几项未拍板事宜按推荐直接执行」，
因此不再发问卷，按工程判断逐项处置并补齐验证。

---

## 2. 本轮已完成（含证据）

### 2.1 350 条未提交项入库

- 组成：6 个 ` M`（`.gitignore`、`eslint.config.ts`、`extensions/kizemo.focus-sync/sidecar/scripts/manual-detect-test.ps1`、
  `src-tauri/src/reader.rs`、`src-tauri/src/state.rs`、`src-tauri/src/uia_inject.rs`，全部是换根收尾适配）
  + 345 个 `??`（`docs` 334、`scripts` 9、`AGENTS.md` 1、`extensions` 1）。
- `AGENTS.md` 此前**从未入库**，本轮首次纳入版本管理。
- 结果：`350 files changed, 88342 insertions(+), 21 deletions(-)`，commit `2cabae4a`。
- 提交信息文件：`.superpowers/sdd/commit-msg-1.txt`（`git commit -F <file>`，`-F -` 不读 stdin）。

### 2.2 临时探针迁出 sidecar 源码树

- `extensions/kizemo.focus-sync/sidecar/src/bin/repro_save_dialog.rs` → `_acceptance/probes/repro_save_dialog.rs`。
- 理由：sidecar `Cargo.toml` 只有 `[lib] name="spike"` + `[[bin]] name="spike" path="src/main.rs"`，
  **没有 `autobins = false`**，留在 `src/bin/` 会被 cargo 当 bin target 一起编译。
- `_acceptance/` 已被 `.gitignore:69` 覆盖 ⇒ 不入库。落地核对：`BIN_LEFT=0`。

### 2.3 `.superpowers/sdd/superseded` 清除

- 内容：`alpha-file-manager.exe.prev`、`.prev2`、`Alpha File Manager_3.6.1_x64-setup.acceptance-rebuild.exe`、`bundle-nsis\`。
- 执行：顶层单条 `rm -- "<abs>"` → `mavis-trash: moved to trash`。复测 `superseded=MISSING`。约 832 MB。

### 2.4 `npm run tauri:build` 打包成功（第 3 次尝试才真正编译）

- 产物：
  - `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.5_x64-setup.exe` — **43.30 MB，mtime 2026/11/10 15:03:46**
  - `src-tauri\target\release\alpha-file-manager.exe` — **82,334,720 B（78.50 MB），mtime 2026/11/10 15:03:47**
- 日志确认：`Cargo manifests are already synced to 3.13.5` → `Running beforeBuildCommand npm run build`
  → `vue-tsc --build` 通过 → vite 产出 `dist` 全量资产。
- 版本号未变（仍 3.13.5）；`release\` 目录里同名安装包 mtime 仍是 **2026/11/10 0:21:24** ⇒
  `scripts/publish-bundle-to-release.mjs` 未覆盖同名旧文件（按版本跳过）。
- ⚠️ 前两次「构建成功」是假成功：批处理里 `>> "%LOG%"` 追加重定向在后台上下文被拒
  （`ERROR_SHARING_VIOLATION`），`call npm.cmd run tauri:build` 与 `echo BUILD_END` 两行被**静默跳过**，
  脚本仍 `exit 0`、日志只有 343/398 字节且无 `BUILD_END`。**判据必须看产物 mtime，不能看脚本退出码 + 日志是否存在。**

### 2.5 实机启动验证（本轮补齐的「未验证项」）

**结论：PASS。新构建可独立启动并正常渲染主窗口。**

| 项 | 实测 |
|---|---|
| 进程 | `alpha-file-manager.exe` PID 存活，起窗口后持续运行 |
| 主窗口 | class `Tauri Window`，title `Alpha File Manager`，最大化 **3862×2110**（`-11,-11`） |
| WebView2 | 子进程 **6 个**（`msedgewebview2.exe`） |
| 内容区 | 230 种颜色、墨水像素（lum<170）**2.36%**、呈列对齐的表格行结构 ⇒ 已渲染，非空白 |
| 时序 | 5s 与 20s 两次采样字节数一致（521,975 B）⇒ 稳定态，无延迟空白/闪烁 |

执行方式：`.superpowers/sdd/p2-launch-verify3.ps1`、`p2-launch-verify4.ps1`；日志 `.superpowers/sdd/p2-launch-verify.log`；
截图 `.superpowers/sdd/shots/p2-closeout-main-20261110-150736.png`、`p2-settled-t05-*.png`、`p2-settled-t20-150927.png`。

验证后**已关闭新构建、并重新拉起装机版**（`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`）恢复原状态。

### 2.6 体积复测（构建后）

| 项 | 数值 |
|---|---|
| 项目总计 | **19,362.6 MB / 199,676 files**（P2 清理后为 20,135.2 MB） |
| `src-tauri\target\release` | 13,610.7 MB / 153,004 |
| `.superpowers\sdd` | 3,375.0 MB / 11,435 |
| `.superpowers\sdd\shots` | 615.2 MB / 274（保留） |
| `dist` | 38.8 MB / 1,976（P2 删过，本次 vite build 重建） |
| `.superpowers\sdd\superseded` | MISSING（已清除） |

净变化 = `superseded` −832 MB + `dist` 重建 +38.8 MB。

### 2.7 门禁基线（P2 清理后实测，本轮未改动源码故未重跑）

- `npm run ts` → **EXIT=0**
- `npm run lint:eslint:check` → **EXIT=1，177 errors / 0 warnings**（**既有基线**，非本轮引入）
- `npm run test:unit:run` → **EXIT=0，299 test files / 3134 tests 全通过**（141.90 s）
- 本轮 `tauri:build` 内部的 `vue-tsc --build` 已再次验证类型检查通过。

---

## 3. 已拍板决策（本轮按推荐执行）

| 对象 | 决策 | 理由 |
|---|---|---|
| `src-tauri\target\release`（13.6 GB） | **保留** | 删到回收站不释放磁盘（同盘 F:）；删掉则每次真机验收付全量 release 重编代价。本轮增量构建约 5 分钟即为保留价值的实证 |
| `.superpowers\sdd\shots`（615.2 MB） | **保留** | 唯一性验收截图不可再生成 |
| `.superpowers\sdd\superseded`（832 MB） | **删除** | 语义即「被取代」 |
| F: 盘回收站（91 项 / 36.17 GB） | **不动** | 不可逆、且混有 practilog 等其他项目条目，属「删用户数据」类；真正释放磁盘需用户明确点头 |
| `repro_save_dialog.rs` | 不入库，移 `_acceptance/probes/` | 避免 cargo 误识别 bin target |

---

## 4. 已验证 / 推断 / 未验证

**已验证（有产物或日志）**
- commit `2cabae4a` 落地，工作树零残留。
- `superseded` 已进回收站（`MISSING`）。
- NSIS 安装包与主 exe 均在 15:03 刷新；`tauri:build` 全链路（sync-version → vite build → cargo → bundle）跑通。
- 新构建独立启动、主窗口出现、内容区已渲染、稳定不闪。
- 项目体积 19,362.6 MB。

**推断（未直接取数）**
- 构建耗时约 4.5–5 分钟：由「npm 启动 ≈14:59 → 安装包 15:03:46」估算。`TAURI_BUILD_EXIT=`/`ELAPSED_SEC=` 两行未落到 artifacts（后台任务状态 `succeeded`、产物已生成 ⇒ 判定成功，但**退出码与秒数无原始记录**）。

**未验证 / 未做**
- **未 push**（对外发布需授权）。远端 `origin = https://github.com/kizemo/alpha-file-manager.git`。
- **装机版仍是旧构建**：`D:\Program Files\Alpha File Manager\` 里是 2026/11/10 0:21 的 3.13.5，**不是 15:03 新构建**。要同步需运行新 `setup.exe`。
- **未重跑全量单测**：本轮只有 staging/commit + 删目录，未改源文件内容。
- `docs/archive/focus-sync-legacy/docs/superpowers/spike-reports/2026-09-27-raw.log` 被嵌套 `.gitignore` 忽略，归档少 1 文件（记录不修）。
- eslint 177 errors 是既有基线，本轮未处理。

---

## 5. 本轮踩到并已坐实的工具坑（下轮直接避）

1. **单实例插件会让「新构建启动」假失败**：`tauri-plugin-single-instance` 生效时，装机版实例在跑，
   新 exe 启动后 **15 秒内自行退出、EXITCODE=0**。这不是崩溃。验证新构建前必须先关掉装机版实例。
2. **`Process.MainWindowHandle` 会选中错误的窗口**：它先返回 22×22 的隐藏辅助窗口
   （class `com.sigma-file-manager.app-siw`，标题同 class 名），截出来是 155 字节空白图。
   正解：`EnumWindows` + `GetWindowThreadProcessId` 过滤，取**可见且 ≥800×600 的最大窗口**（class `Tauri Window`）。
3. **平均值亮度网格会把细字迹抹平**：48×22 均值图上「看起来一片均匀浅色」被误读成「内容区空白」，
   实际内容是稀疏文本。判读 UI 是否渲染，用**墨水像素占比**（lum<170 的数量）+ 内容区**颜色种数**，
   不要用区域均值。
4. **PowerShell 函数把所有未捕获输出当返回值**：`Log()` 内部 `Write-Output` 会让函数返回数组，
   把截图路径污染成 `"SHOT_t20=... BYTES=... <path>"`，后续 `Bitmap.FromFile` 直接抛异常。修法：`Log(...) | Out-Null`。
5. **`tauri:build` 唯一可用调用形态**：pwsh 内先导入 vcvars 环境（`cmd /c '"<vcvars64.bat>" >nul 2>&1 && set'`，逐行
   `SetEnvironmentVariable(...,'Process')`），再 `cmd /c cd /d "<root>" && npm run tauri:build`。
   批处理里用 `>>` 追加重定向 + 后台上下文 = 静默跳过命令行。
6. `git commit -F -` 不读 stdin（只接受文件名）；消息先落 ASCII 路径文件。
7. `Set-Location` 后再写/删会被本地安全策略拦（`write/delete targets cannot be verified safely after a command
   working directory change`）⇒ 一律 `git -C <repo>`、全绝对路径参数式调用。
8. 可回收删除必须**顶层单条** `rm -- "<abs>"`；嵌进复合命令会报
   `bare or non-canonical mavis-trash is untrusted`。
9. 复制原语只有 `[System.IO.Directory]::CreateDirectory()` + `[System.IO.File]::Copy($src,$dst,$true)`；
   `Copy-Item` / `robocopy /MOVE` / `Compress-Archive -Force` 会被策略拒。
10. 后台命令**不许挂 stdout 管道**（`Tee-Object` / `Select-String`），否则进程树被标 `Command aborted`。

---

## 6. 遗留待办（按建议优先级）

1. **是否清空 F: 盘回收站 91 项 / 36.17 GB** —— 唯一能真正释放磁盘的动作，不可逆，**必须用户点头**。
   F: 现状：`used=342.3 GB free=88.7 GB`；回收站混有其他项目条目，需逐条确认或按需保留。
2. **是否把 15:03 的新安装包同步到 `release\`** —— 同名 3.13.5 未被 publish 脚本覆盖。
   建议先查 `scripts/publish-bundle-to-release.mjs` 的跳过条件（按版本号跳过？），再决定手工替换或改脚本。
3. **是否同步装机** —— 运行 `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.5_x64-setup.exe`。
4. **是否 push** —— `2cabae4a` 仅在本地；push 属对外发布，需明确授权。
5. **eslint 177 errors 既有基线** —— 是否开专项清理，需用户决定（属「改变架构方向/大量耗时」类）。
6. 可选小事：修 `docs/archive/focus-sync-legacy/.../2026-09-27-raw.log` 被嵌套 `.gitignore` 挡掉的问题。

---

## 7. 关键文件索引

- 项目宪章：`AGENTS.md`
- 上轮 handoff：`docs/handoff-2026-11-10-root-unification.md`、`docs/handoff-2026-11-10-p2-inner-volume-cleanup.md`
- 本轮 handoff：`docs/handoff-2026-11-10-p2-closeout.md`（本文件）
- 构建/验证装置：`.superpowers/sdd/p2-launch-verify{2,3,4}.ps1`、`.superpowers/sdd/p2-launch-verify.log`、
  `.superpowers/sdd/p2-build-run2.log`、`.superpowers/sdd/commit-msg-1.txt`、`.superpowers/sdd/shots/`
- 迁出的探针：`_acceptance/probes/repro_save_dialog.rs`
- 打包产物：`src-tauri\target\release\alpha-file-manager.exe`、`src-tauri\target\release\bundle\nsis\`
- 版本同步：`package.json`（`tauri:build = npm run sync-version && tauri build && node ./scripts/publish-bundle-to-release.mjs`）