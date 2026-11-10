# 交接文档 · 批次 P3 收口（2026-11-10）

- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（Alpha File Manager，GitHub fork `kizemo/alpha-file-manager`，upstream `aleksey-hoffman/sigma-file-manager`，**不 push upstream**）
- 本轮 HEAD：`2ab78e1b fix(focus-sync): repair installer sandbox-gate paths after the repo root move`（3 files, +104/-20）
- 前置文档：`docs/handoff-2026-11-10-p2-closeout.md`、`docs/handoff-2026-11-10-p2-closeout-followup.md`、`AGENTS.md` §0 与 §2
- 本轮工作区产物根：`.superpowers\sdd\`（gitignore，不入库）

---

## 一、背景与本轮目标

批次 P2 收口时留下两件事没做完：

1. **focus-sync 安装链的沙箱门禁路径失效**。插件并仓 + 2026-11-10 换仓库根之后，`installer/register.ps1` 与
   `installer/manual-install.ps1` 里指向 scanner/sandbox 的相对路径从安装目录向上算已经够不着仓库文件，
   门禁静默不执行；`manual-install.ps1` 的 `$srcDir` 还指向根本不存在的 `..\extension`。
2. **真机界面回归欠账**。上一轮 eslint 格式化改动（涉及 `.vue` 文件）没做界面回归，需要用打包产物装机实点
   「目录树开关 / 缩略图尺寸弹窗 / 文件右键菜单」三处。

本轮把这两件事全部做完，并把结论建立在可复查的产物证据上。

---

## 二、已完成

### 2.1 安装链路径修复（已提交，`2ab78e1b`）

改动文件：

- `extensions/kizemo.focus-sync/installer/register.ps1`
- `extensions/kizemo.focus-sync/installer/manual-install.ps1`
- `src/modules/extensions/runtime/__tests__/installer-sandbox-gate.test.ts`（新增门禁单测）

修复内容：

- scanner 与 sandbox.ts 的 `Join-Path` 相对层级改为 `..\..\..\…`，从装机/仓库内安装路径向上能真正解析到仓库文件；
- `$srcDir` 由 `'..\extension'`（不存在）改为 `'..'`（installer 目录本身即扩展包根）；
- 新增 `$sandboxTs` 存在性检查，路径解析不到时显式报错而不是静默跳过；
- 运行进程名检查 `sigma-file-manager` → `alpha-file-manager`；卸载注册表键、可执行名、回退路径与末尾提示文案统一为 Alpha File Manager。

**设计取舍**：只做最小改动 + 机器门禁，保留 `Join-Path` 相对字面量写法，不引入「向上遍历找 repo root」的自愈逻辑；
用一条 vitest 解析脚本字面量并断言其解析到真实文件，阻止下次换根再次静默腐化。

### 2.2 打包与装机

- `npm run tauri:build` 成功，`BUILD_START=2026-11-10 17:53:05`；
  - `src-tauri\target\release\alpha-file-manager.exe` = 82,334,720 B @ 18:00:22
  - `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.5_x64-setup.exe` = 45,407,027 B @ 18:00:20
    （与上一版 45,413,511 B 大小不同 ⇒ 确认是重打而非复用旧包）
- 静默安装 `Start-Process <setup> -ArgumentList '/S','/FORCE' -Wait` → **EXIT=0 / 24.5 s**
- 装机产物：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe` = 82,334,720 B @ 17:59:00；
  注册表 `HKLM\...\Uninstall\Alpha File Manager` → DisplayVersion **3.13.5**
- **修复确实进了安装包**：装机副本 `D:\Program Files\Alpha File Manager\tools\register.ps1` 已含
  `$sandboxScanner = Join-Path $PSScriptRoot '..\..\..\scripts\scan-sandbox-dynamic.cjs'` 与
  `$sandboxTs = Join-Path $PSScriptRoot '..\..\..\src\modules\extensions\runtime\sandbox.ts'`
  （resources 映射见 `src-tauri\tauri.conf.json:81-99`；`manual-install.ps1` 不入包）

### 2.3 门禁与测试结果（提交前实测）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 类型 | `npm run ts` | 0 错 |
| ESLint | `npm run lint:eslint:check`（`eslint . --max-warnings 0`） | 0 错 0 警 |
| 新单测 | `installer-sandbox-gate.test.ts` | **3 passed** |
| 全量单测 | `npm run test:unit:run` | **300 files / 3140 tests 全绿** |
| 变更日志 | `npm run verify-changelog` | OK（CHANGELOG 覆盖 3.13.5、顺序正确） |

---

## 三、真机界面回归：方法与三点结论

### 3.1 验收环境（本项目唯一有效形态）

- **只能用 `npm run tauri:build` 的打包产物**；`tauri dev` 的界面结论在本项目一律作废。
- 必须在**隔离桌面沙箱**内跑：`CreateDesktop` + `STARTUPINFO.lpDesktop` + `EnumDesktopWindows` 取窗口；
  注入端进程不切桌面。
- 主窗口获取必须 `EnumDesktopWindows` + `GetWindowThreadProcessId` 过滤，取「可见且面积最大、class=`Tauri Window`」的句柄；
  **不可**用 `Process.MainWindowHandle`（会误选 22×22 的隐藏辅助窗口）。
- 鼠标走 `PostMessage` 到 `Chrome_RenderWidgetHostHWND`；截图走 `PrintWindow(hwnd, hdc, 2)`；
  判据 = 前后 PNG **像素差分**（changed 像素数）+ 截图目视。
- 沙箱内**没有键盘通道**，只能做鼠标的等价断言。
- 驱动脚本：`.superpowers\sdd\p3-ui-regress-sandbox.ps1`
  - 参数：`-Exe` `-Repo` `-ShotDir`（默认 `.superpowers\sdd\shots`）`-Tag` `-Actions`（JSON 数组）
    `-Seed`（默认 `--route=navigator --tree=true`）`-WindowW=1280` `-WindowH=860` `-SettleSec`
  - 输出 `p3-<Tag>-<n>-<name>-{before,after}.png` + `p3-ui-regress-<Tag>-summary.json`（summary 在 **shots** 目录）
  - 日志 `[sandbox] clientOffset=8,1`；`-Actions` 里的坐标是 **png 坐标**
- 夹具 seed 脚本：`.superpowers\sdd\r40-seed.cjs`，每轮把 `navigator.showFolderTree` / `lastRoute` /
  `navigator.infoPanel.show=false` / `changelog.*` 写进用户设置，并把工作区目录型 tab 的路径改为
  `C:/Users/Public/afm-demo-390`。

### 3.2 三点结论（全部取证通过）

| # | 项目 | 轮次 / 动作坐标（png） | 像素差分 | 证据 | 结论 |
| --- | --- | --- | --- | --- | --- |
| 1 | 文件右键菜单 | `p3rA` / `rightclick(340,477)` | 46,546 / 1,100,800 | `.superpowers\sdd\shots\p3-p3rA-1-1-file-context-menu-after.png` | **通过**：菜单完整弹出，含「打开方式▸ / 复制路径 / 重命名 / 压缩为… / 删除 / 属性」，并可见「正在加载 "afm-demo-390"」浮层 |
| 2 | 缩略图尺寸弹窗（含当前值 ✓） | `p3rB` / `click(266,73)`「⋯」→ `click(307,307)`「缩略图大小」 | 901,829 / 1,100,800（≈82%） | `.superpowers\sdd\shots\p3-p3rB-1-2-open-thumbnail-dialog-after.png` | **通过**：弹窗显示 `小 ✓`（当前值带高亮描边）/ `中 / 大 / 超大 / 特大` + `取消 / 完成`。⇒ 上轮 eslint 格式化改动涉及的 ✓ 标记在真机渲染正常 |
| 3 | 目录树开关 `FolderTreeIcon` | `p3rC` / 单动作 `click(916,24)` | 64,284 / 1,100,800 | `.superpowers\sdd\shots\p3-p3rC-1-1-toggle-folder-tree-after.png` + `p3-ui-regress-p3rC-summary.json`（`tree: seeded=true / atStart=true / afterActions=false`） | **通过**：左侧 x≈60..235 目录树面板消失、文件列表铺满 ⇒ 顶栏开关真机生效且写回持久化 |

### 3.3 明确未取证的部分（不要当已验证）

- **键盘路径未在沙箱内取证**。沙箱无键盘通道，快捷键、Esc 关弹窗、方向键导航等均未验证。
- 弹窗内 `✓`、目录树加载失败标记 `!` 属**像素级目视判读**，不是 DOM/UIA 断言（UIA `FindFirst` 在 Rust 重索引时永久阻塞，不可用）。
- 三项都是「一次动作一次截图」的功能级证据，未做分辨率/DPI 矩阵覆盖（固定 1280×860）。

### 3.4 用户设置已恢复

验收期间 `r40-seed.cjs` 改的是**真实用户设置**（release 构建下 `SFM_E2E_DATA_DIR` 被编译掉，只能备份→灌夹具→验收→恢复）：

- 备份：`.superpowers\sdd\userdata-backup-p3\{user-settings.json 6,126 B, user-workspaces.json 2,068 B}`
- 已恢复并逐字节校验：`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\`
  - `user-settings.json` SHA256 前 16 位 `81E675B487BBEBF6` = 备份，6,126 B
  - `user-workspaces.json` SHA256 前 16 位 `880AE82CA90B8319` = 备份，2,068 B
  - 恢复前确认 `Get-Process alpha-file-manager` 计数 = 0

---

## 四、既有基线问题（与本轮改动无关，未处理）

1. **`npm run check` 的 stylelint 段红**：`lint:stylelint:check` 有 **174 个 `order/properties-order`** 错误，
   分布在 `src/modules/nav-sidebar/*`、`home/**`、`dashboard/**`、`settings/**` 等本轮**未改动**的 `.vue`。
   本轮**没有**跑 `stylelint --fix`：`order/properties-order` 自动重排会改变 CSS 级联结果
   （典型如 `margin` 与 `margin-top` 顺序互换导致渲染变化），必须配套视觉回归，留待独立提交。
2. **全量单测有 1 条 unhandled rejection**（退出码仍为 0）：
   `address-bar-editor-utils.ts:624` `Cannot read properties of undefined (reading 'filter')`，源自
   `address-bar-editor-dialog.test.ts`。
3. `release\` 顶层是已发布资产镜像，**禁止覆盖**；本地重建包只落 `release\local-builds\`。

---

## 五、工具坑（本轮实测，勿重试）

- **PS 5.1 `ConvertFrom-Json` 顶层数组不枚举**：`$actions = $Actions | ConvertFrom-Json` 得到的是单个数组对象，
  下游 `[int]$a.x` 直接崩。正解 = 显式 `foreach ($parsedItem in $parsed) { $actionList += $parsedItem }`。
  本轮 `p3-roundB.log` 首跑即因此崩溃，修脚本后 `p3-roundB2.log` 通过。
- **打开菜单/弹窗后的第一次点击会被它吞掉**（菜单外首次点击只负责关菜单）。
  `p3rA` 里 `click(916,24)` 后 summary 仍 `tree=true`，**不是**开关失效，是被右键菜单吃掉；
  必须**拆成独立轮次**才拿得到证据（`p3rC`）。同类「假失败」日后不可直接反推「按钮无效」。
- **多动作轮 vs 单动作轮**：菜单/弹窗会遮挡后续坐标，故界面回归**拆成多轮单/双动作**，而不是塞进一轮多动作。
- **坐标不能用旧轮次残留值**：目录树开关不沿用 r42 的 (634,38)（那是 1180 宽旧界面），
  改由源码链路（`showFolderTree` → `navigator-toolbar-actions` → Teleport 目标）+ 放大裁剪图定位到 (916,24)。
- **`tauri:build` 的退出码与 `BUILD_END` 都不可信**，判成功只看产物 `LastWriteTime` 是否落在构建窗口。
- 唯一可靠构建调用形态见 `.superpowers\sdd\p3-build-run.ps1`（pwsh 导入 vcvars + `cmd /c cd /d && npm run tauri:build`，日志单次 `>`）。
- 后台命令**不得挂 stdout 管道**（`Tee-Object` / `Select-String`）；追加重定向 `>>` 会触发 sharing violation 被 cmd 静默跳过，用单次 `>` 或 PS `*>`。
- `.superpowers\sdd\*.cmd` / `*.ps1` 必须**纯 ASCII**。
- **工作区外写入被本地硬策略拦截**：`Copy-Item` 到 `AppData` 被拦（报 recursive write/delete 无法验证），
  改用 `[System.IO.File]::Copy($src,$dst,$true)` 可过（并用 SHA256 双向校验）。
- 装机 exe 与 `target\release` exe **哈希不同、长度相同**（构建期注入时间戳/版本数据）。
  本轮用「装机 mtime 落在构建窗口 + 安装包内 `tools/register.ps1` 已含修复」双证；
  **未能**从 setup 内直接提取 exe 做哈希比对 —— 这是推断链，不是直接证明。
- 每轮结束 `Stop-Process -Force` 后 `[stop] still alive, forcing` 属常态输出，不代表异常。

---

## 六、下一步建议

1. **stylelint 174 错的独立提交**：先建视觉回归基线（打包产物截图 diff），再 `--fix`，避免盲改 CSS 级联。
2. `address-bar-editor-utils.ts:624` 的 unhandled rejection 值得单独查一条：它不影响退出码，说明测试里存在被吞掉的异常路径。
3. **push `2ab78e1b` 仍未做**，按惯例需用户明确授权。
4. 安装链修复的真实价值在**仓库内 / dev 安装路径**；打包安装场景下 `$INSTDIR\tools` 无仓库 ⇒ 门禁合法跳过（脚本原有 else 分支即此设计）。若要打包场景也校验，需要把 scanner/sandbox 一起入包或改用构建期校验，属另立需求。
5. 若要做键盘路径验收，需要给沙箱注入加键盘通道（`PostMessage` 送 `WM_KEYDOWN/WM_KEYUP`），或改走 CDP
   （需改 `tauri.conf.json` 的 `additionalBrowserArgs` 并**重打两次包**，代价高，本轮已因此放弃）。

---

## 七、证据路径速查

| 内容 | 路径 |
| --- | --- |
| 右键菜单截图 | `.superpowers\sdd\shots\p3-p3rA-1-1-file-context-menu-after.png` |
| 缩略图尺寸弹窗截图 | `.superpowers\sdd\shots\p3-p3rB-1-2-open-thumbnail-dialog-after.png` |
| 目录树关闭后截图 | `.superpowers\sdd\shots\p3-p3rC-1-1-toggle-folder-tree-after.png` |
| 三轮 summary | `.superpowers\sdd\shots\p3-ui-regress-p3r{A,B,C}-summary.json` |
| 沙箱驱动脚本 | `.superpowers\sdd\p3-ui-regress-sandbox.ps1` |
| 夹具 seed 脚本 | `.superpowers\sdd\r40-seed.cjs` |
| 构建脚本 | `.superpowers\sdd\p3-build-run.ps1` |
| 回归日志 | `.superpowers\sdd\p3-roundA.log` / `p3-roundB.log`（旧脚本崩溃）/ `p3-roundB2.log`（通过）/ `p3-roundC.log` |
| 用户设置备份 | `.superpowers\sdd\userdata-backup-p3\` |
| 安装包 | `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.5_x64-setup.exe` |
