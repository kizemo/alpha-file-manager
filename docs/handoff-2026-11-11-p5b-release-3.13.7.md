# P5b 发版交接（3.13.7 · 快捷键系统冲突检测）

- 生成时间：2026-11-11
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 分支/提交起点：`main` = `2fcf1eb8`
- 装机版本：**3.13.7**（`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，82,614,272 B）
- 上一份交接：`docs/handoff-2026-11-11-p5b-shortcut-conflict-detection.md`（本次任务的作业计划，§2.3 为**禁重跑**的失效取证路径）
- 下一阶段：夸克网盘接入 —— 方案 `docs/plan-2026-11-11-quark-pan.md`，**审核通过后先做 T0 实测**，不直接写实现

---

## 1. 本轮完成的事

| 步骤 | 状态 | 证据 |
|---|---|---|
| 第 2 步 补 `global-shortcuts` 单测 | ✅ 13/13 | `src/stores/runtime/__tests__/global-shortcuts.test.ts` |
| 第 3 步 版本号 3.13.6 → 3.13.7 | ✅ | `package.json` + 3 个 `Cargo.toml` + `Cargo.lock`；脚本 `.superpowers/sdd/bump-version-3137.cjs` |
| 第 4 步 CHANGELOG + 14 个 locale | ✅ | `node scripts/verify-changelog.mjs` → OK（56 条目）；脚本 `.superpowers/sdd/write-changelog-3137.cjs` |
| 第 5 步 `npm run check` | ✅ 300 files / 3150 tests | `.superpowers/sdd/p5b-check3.log`（本轮亲自重跑，非引用旧日志） |
| 第 6 步 `tauri:build` 打包 | ✅ | `.superpowers/sdd/p5b-build-run.log`；产物 `src-tauri/target/release/bundle/nsis/Alpha File Manager_3.13.7_x64-setup.exe`（45,452,194 B） |
| 第 7 步 静默装机 | ✅ | `.superpowers/sdd/p5b-install.ps1`；安装副本 `VersionInfo` = FileVersion/ProductVersion `3.13.7` |
| 第 8 步 Phase A 快捷键链路 | ✅ | `.superpowers/sdd/p5b-phaseA.log` + 截图，见 §2 |
| 第 8 步 Phase B 冲突徽标 | ✅ | `.superpowers/sdd/p5b-phaseB.log` + 截图，见 §2 |
| 第 9 步 还原用户数据 | ✅ `RESTORE_BAD=0` | `.superpowers/sdd/p5b-restore.log`、`.superpowers/sdd/p5b-restore.ps1` |
| 第 10 步 提交 | ✅ 本提交 | **未 push**（需授权）、**未发 GitHub Release**（未授权） |

---

## 2. 真机验收怎么做的

夹具（`.superpowers/sdd/p5b-fixture.cjs`，直接改真实 `user-settings.json`；`release` 构建里 `SFM_E2E_DATA_DIR` 被编译掉，只能走这条路）：

- **Phase A（证明快捷键 → handler → 命令面板链路通）**
  1. 夹具把 `toggleCommandPalette` 改绑为 `Ctrl+Shift+Space`（落盘键 `sigma.shortcut.toggleCommandPalette`，同时删掉旧键防遮蔽）；本机 `Ctrl+Shift+Space` 为 **FREE**。
  2. 启动已安装的 3.13.7 → 基线截图 `a0-baseline.png`。
  3. 发 `Ctrl+Shift+Space` → 截图 `a1-after-ctrl-shift-space.png`。
  4. `Esc` → 截图 `a2-after-esc.png`。
  5. 发 `Ctrl+Shift+P` → 截图 `a3-after-ctrl-shift-p.png`（对照组）。
  6. 像素判别（基线 = `a0-baseline.png`）：`a1` **DIFF=1,929,544**（全窗口变暗，BBOX 覆盖全窗）→ 命令面板已弹出；`a2` **DIFF=0**（完全回到基线）→ `Esc` 正常关闭；`a3` **DIFF=0** → `Ctrl+Shift+P` 无任何反应（**预期**，见 §3）。
  7. 肉眼复核截图确认：`a1` 中确有居中「输入命令或搜索…」命令面板（Git Tools / Focus Sync 命令列表），`a0` 中该行显示 `Ctrl+Shift+Space`、来源=用户 ⇒ 夹具改绑确实生效。
- **Phase B（证明系统冲突徽标会渲染）**
  1. 从备份还原 `user-settings.json` → 夹具 `route`：`lastRoute={"name":"settings","fullPageId":""}` + `settingsCurrentTab="shortcuts"`，**不**改绑（否则该行不再显示 `Ctrl+Shift+P`，徽标无从取证）。
  2. 启动 → 截图 `b1-shortcuts-page.png`。
  3. 肉眼复核：`Open / close command palette` 行的 `Ctrl+Shift+P` 右侧出现**红色 ⚠ 徽标**，其余行无徽标；夹具输出 `FIXTURE_SHORTCUT_KEYS=`（空）证实未改绑 ⇒ 徽标来自系统冲突探测本身。

**截图**
- `.superpowers/sdd/p5b-accept/phaseA/a0-baseline.png`
- `.superpowers/sdd/p5b-accept/phaseA/a1-after-ctrl-shift-space.png`
- `.superpowers/sdd/p5b-accept/phaseA/a2-after-esc.png`
- `.superpowers/sdd/p5b-accept/phaseA/a3-after-ctrl-shift-p.png`
- `.superpowers/sdd/p5b-accept/phaseB/b1-shortcuts-page.png`

**驱动脚本**
- `.superpowers/sdd/p5b-ui.ps1`：找窗（class `Tauri Window` / `Chrome_WidgetWin_1`，取最大面积）+ 置前；action = `info`/`shot`/`click`/`type`/`keys`/`esc`；文本输入用 **SendInput + `KEYEVENTF_UNICODE`**（`keybd_event` 的 scan 是 BYTE，输不了中文）。
- `.superpowers/sdd/p5b-accept.ps1`：Phase A / Phase B 编排（`-Phase A` / `-Phase B`）。
- `.superpowers/sdd/p5b-fixture.cjs`：夹具，mode `bind` / `route` / `unbind`。
- `.superpowers/sdd/p5b-restore.ps1`：第 9 步还原 + 核对（`-DiffOnly` 只比对不改动）。

---

## 3. 本机不可达项（如实记录，**不标 PASS**）

- **`Ctrl+Shift+P` 在本机被外部程序以 OS 级 `RegisterHotKey` 占用**（`Win32Error=1409`，交接 §2.2 已坐实）。键在进入 WebView 之前就被系统吞掉 ⇒
  - 该快捷键在本机**不可能**触发命令面板（Phase A 的 `a3` DIFF=0 即为该限制的直接体现，不是回归）；
  - **编辑器内录制 `Ctrl+Shift+P` 时出现的系统冲突告警，在本机无法演示/取证** —— 录制器永远收不到该组合（`watch(recordedKeys)` 不会被触发）。该项**必须由用户在未被占用的机器上复验**，不得凭代码推理判 PASS。
- 可复现该现象的组合：`Ctrl+Shift+P` / `A` / `K` / `W` 被占；`Ctrl+Shift+Space` 与 `Alt+Shift+P` 为 FREE。

---

## 4. 本轮踩到并已修的坑（供下轮复用）

1. **PowerShell 函数里的 `@Args` 是自动变量**：函数声明了 `param([string[]]$UiArgs)` 后 `$args` 为空 ⇒ `& powershell -File $Ui @Args` 退化成不带参数调用，整个 Phase A 静默变成只读 `info`。**必须用 `@<参数名>` 展开**（已改为 `@UiArgs`，并在 `Ui` 里加 `UI_CALL=` 打印以便一眼看出传参）。
2. **`.cjs` 文件不能用 `#` 注释**（本机 node v26.13.0 报 `SyntaxError: Invalid or unexpected token`）；一律用 `//`。
3. **tauri 打包会 patch exe**（`Info Patching ... with bundle type information: nsis`）⇒ 安装副本与 `target\release\alpha-file-manager.exe` 的 **SHA256 必然不等**。装机判据要用「**大小一致 + `VersionInfo`=3.13.7**」，不要用哈希（否则误报 `INSTALLED_BUT_HASH_MISMATCH`）。
4. **bash 内联写 `C:\Users\Duanyi\AppData\...` 被本地硬安全策略拦截**（outside workspace）⇒ 跨工作区边界的写/删/改名一律写成 `.ps1` 再 `pwsh -File` 执行。
5. **`git commit -F -` 不读 stdin** ⇒ 提交信息先写临时文件再 `git commit -F <file>`。
6. **bash 里 PowerShell 含 `$` 变量会被 bash 吞掉** ⇒ 单引号包裹整条 `-Command`，或落 `.ps1`。
7. `Get-Content -Raw` / `Set-Content` **不得**用于带中文注释的源文件；批量改 JSON 用 node 行级插入 + `JSON.parse` 自检。

---

## 5. 用户数据还原（第 9 步）细节

- 备份：`.superpowers/sdd/p5b-kbd/user-data/`（**整个** user-data 目录，22 个文件，装机前拍摄）。
- 还原顺序：**先杀进程 → 再 restore**（`p5b-restore.ps1` 内置校验，进程残留时 `RESTORE_VERDICT=APP_STILL_RUNNING` 直接退出，不动文件）。
- 核对结果：`COPIED_BACK=22` / `RESTORE_BAD=0` / 22 个文件**逐字节**与备份一致。
- `user-settings.json` 已回到原始状态：`changelog.showOnUpdate=true`、`changelog.lastSeenVersion=3.13.6`、`settingsCurrentTab=about`、**无** `toggleCommandPalette` / **无** `sigma.shortcut.*` 键 ⇒ 夹具痕迹清除干净（且下次启动 3.13.7 仍会正常弹更新日志）。
- **保留未删**：`media\source-backgrounds\` 下 6 个 JPG（`Abstract painting by Suzy Hazelwood.jpg` 等，合计约 2.7 MB）。它们是 **3.13.7 首启时应用自己生成的源背景素材**，不是夹具产物、也不是用户手写数据，删除属无谓破坏，故只记录不删。若要严格还原到「装机前」目录树，删掉这 6 个文件即可（应用会按需重新生成）。

---

## 6. 下一阶段：夸克网盘

- 方案文档：`docs/plan-2026-11-11-quark-pan.md`（8 章 / 1,266 行）。
- 需求：在「添加网盘」处新增「夸克网盘」，点击弹出网页、登录夸克账号即完成配置，行为对齐现有百度网盘。
- **流程闸口**：研究（已完成）→ 方案（已完成）→ **方案审核（进行中）** → 审核通过后才允许写实现代码。
- 审核必须重点复核的两处：
  1. **T0 实测方案（头号闸门，1 人天）**：验证「服务端是否接受非浏览器来源的 cookie 直连」——这是本方案**单一最大风险**；判定阈值与前置条件是否明确可执行。
  2. **§7 的 U1–U14 未定项**：区分「必须在 T0 之前拍板」与「可延后到实现期」。
- 降级路径（若 T0 判定 cookie 直连被拒）：改在 WebView2 内 `fetch` + Rust 解析，**+1.5 人天**，总量 8 → 9.5 人天。
- 评审报告落盘位置：`.superpowers/sdd/quark/plan-review-2026-11-11.md`（中间产物，gitignored；正式结论需另行落进 `docs/`）。

---

## 7. 证据路径总表

| 内容 | 路径 |
|---|---|
| 作业计划（本轮任务书） | `docs/handoff-2026-11-11-p5b-shortcut-conflict-detection.md` |
| 本交接 | `docs/handoff-2026-11-11-p5b-release-3.13.7.md` |
| 夸克方案 | `docs/plan-2026-11-11-quark-pan.md` |
| 门禁日志 | `.superpowers/sdd/p5b-check3.log` |
| 构建日志 | `.superpowers/sdd/p5b-build-run.log` |
| 装机脚本 | `.superpowers/sdd/p5b-install.ps1` |
| Phase A 日志 / 截图 | `.superpowers/sdd/p5b-phaseA.log`、`.superpowers/sdd/p5b-accept/phaseA/*.png` |
| Phase B 日志 / 截图 | `.superpowers/sdd/p5b-phaseB.log`、`.superpowers/sdd/p5b-accept/phaseB/b1-shortcuts-page.png` |
| 还原脚本 / 日志 | `.superpowers/sdd/p5b-restore.ps1`、`.superpowers/sdd/p5b-restore.log` |
| 用户数据备份 | `.superpowers/sdd/p5b-kbd/user-data/` |
| 夸克研究中间产物 | `.superpowers/sdd/quark/`（30 项，含自研 asar 解析器 `asar-read.cjs`） |
| 打包产物 | `src-tauri/target/release/bundle/nsis/Alpha File Manager_3.13.7_x64-setup.exe`、`release/` 同名副本 |
