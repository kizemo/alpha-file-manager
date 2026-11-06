# 交接文档：快捷方式目标图标 + 地址栏原生分隔符（2026-11-07）

> 内层仓：`F:\soft\00selfmade\filemanager\sigma-file-manager`
> 基线提交：`9da52a16 docs: 3.12.0 发版收尾交接文档`
> **发版流程仍处于暂停状态**（用户 2026-11-06 明确指令：本轮不改 README / CHANGELOG /
> aiec 站点版本说明，不跑 `release:prepare`，不上传，不发 GitHub Release）。
> 本轮**未提交任何 commit**（沿用「4 个侧车文件刻意不提交」的约束）。

---

## 一、用户的两条诉求与完成状态

| # | 诉求 | 状态 |
|---|---|---|
| 1 | 快捷方式要显示**快捷方式关联的图标**（3.12.0 已宣称但未完成） | 代码已改完，门禁全绿；**真机截图待补** |
| 2 | 地址栏显示的斜杠要改成 Windows 原生 `\`（用户截图证据） | 代码已改完，门禁全绿；**真机截图待补** |

---

## 二、根因（两处都精确定位到行）

### 2.1 地址栏 `/` 而不是 `\`

- 应用**内部**的路径统一是正斜杠 `normalizePath`（`path.replace(/\\/g,'/')`）——
  这是设计，不是 bug：比较键、缓存键、`sfm://remote/...` 的协议语义、以及
  「路径以 `/` 结尾 = 浏览该目录内部」这条语义判定（`addressBarQueryMeansBrowseInsideDirectory`
  判 `endsWith('/')`）全都依赖正斜杠。
- 3.12.0 的原生分隔符改造**只接入了面包屑侧**（`address-bar.vue` 的 `:title`、
  复制路径按钮），**漏了地址栏编辑器输入框与建议行**：
  - `address-bar-editor-dialog.vue` 的 `CommandInput` 直接 `v-model="query"`，
    而 `query` 初始值来自 `ensureAddressBarDirectoryQuery()`（`normalizePath` + 强制补尾 `/`）。
  - `address-bar-editor-suggestion-row.vue` 的 `{{ suggestion.path }}` 直接输出内部 `/` 形式。

**修法：显示层原生化，内部语义不变。**

### 2.2 快捷方式看不到目标图标

- 前端 `src/composables/use-navigator-item-icon.ts` 的 `useSystemIcons`
  **只有** `builtin:system` 主题（或扩展主题加载失败）时为 true；
  用户一旦把文件图标主题换成别的东西，`.lnk` 就落到
  `getDefaultFileIconComponent()` 的通用 `FileIcon` ⇒ **目标图标彻底看不到，需求不成立**。
- Rust 侧 `system_icons.rs::resolve_shell_icon_path` 对 `.lnk` 会先试
  `resolve_shortcut_icon_target()`（用 `IShellLinkW::GetPath(..., SLGP_RAWPATH)` 解析目标），
  解析失败就回退到 `.lnk` 自身。
- **一次性探针实测（已跑完并删除）**：
  ```
  PROBE link_icon_is_some=true
  PROBE target_icon_is_some=true
  PROBE link_equals_target=true          <-- 关键
  PROBE env_raw_target=Some("%TEMP%\\afm-lnk-icon-probe\\env-target.txt")
  PROBE env_raw_target_exists=false
  ```
  探针里的 `link_icon` 是**直接把 `.lnk` 路径丢给 shell** 取到的，结果与直接取目标图标
  **完全相同** ⇒ **Windows shell 自己就会把 `.lnk` 解析成目标并返回目标图标**。
  ⇒ 因此 Rust 侧「先解析目标再取目标图标」这一层**不是必需的**；
  环境变量目标（`%TEMP%\...`）解析失败后回退 `.lnk` 自身，shell 依然给出目标图标。
  ⇒ **Rust 侧本轮未改任何生产逻辑**（只删掉探针），缺口全在前端主题旁路。

---

## 三、本轮改动清单

### 3.1 前端（4 个源文件 + 3 个测试文件）

| 文件 | 改动 |
|---|---|
| `src/modules/navigator/components/file-browser/address-bar-editor-utils.ts` | import 增加 `toNativePathSeparators`；新增导出 `toAddressBarDisplayPath(path, isWindows)` |
| `src/modules/navigator/components/file-browser/address-bar-editor-dialog.vue` | 新增 `displayQuery` computed、`handleQueryModelUpdate()`、带 `pendingEditorInputQuery` 守卫的光标复原 `watch`；`CommandInput` 由 `v-model="query"` 改为 `:model-value="displayQuery"` + `@update:model-value="handleQueryModelUpdate"`；**4 个 `query.value = ...` 写入点保持不变**（内部仍是正斜杠） |
| `src/modules/navigator/components/file-browser/address-bar-editor-suggestion-row.vue` | 新增 `usePlatformStore()` 实例化；路径文本改为 `toAddressBarDisplayPath(suggestion.path, platformStore.isWindows)`；import 合并为 `import { toAddressBarDisplayPath, type AddressBarSuggestion } from './address-bar-editor-utils';` |
| `src/composables/use-navigator-item-icon.ts` | 新增 `isShortcutEntry`（非目录 + `extension` 去点后为 `lnk` **或** 路径以 `.lnk` 结尾，双重判断不依赖 `entry.ext` 带不带点）；`useSystemIcons` 增加 `!enabled → false` 与 **`isShortcutEntry → true`（无视图标主题）** 两个分支 |
| `src-tauri/src/system_icons.rs` | **仅删除**探针测试 `probe_lnk_shell_icon_and_raw_target` 与辅助函数 `create_shortcut_with_raw_target`；生产逻辑一字未动 |

### 3.2 测试

| 文件 | 改动 |
|---|---|
| `__tests__/address-bar-editor-dialog.test.ts` | 原断言 `expect(input.value).toBe('C:/Folder/')` → `'C:\\Folder\\'`；新增 `shows the typed path with native windows separators` |
| `__tests__/address-bar-editor-utils.test.ts` | 新增 `describe('toAddressBarDisplayPath')` 三个用例（Windows 反斜杠 / `sfm://` 原样 / 非 Windows 保持正斜杠） |
| `src/composables/__tests__/use-navigator-item-icon.test.ts` | 新增 `always uses the system icon for shortcuts so the target icon stays visible`（`fileIconTheme = builtin:default` + `extension: 'lnk'` ⇒ `iconSrc === 'system-icon'`） |

---

## 四、已验证 / 推断 / 未验证

### 4.1 已验证（有命令与日志）

| 结论 | 证据 |
|---|---|
| vitest：3 个受影响测试文件 **41 passed / 0 failed** | `F:\soft\00selfmade\filemanager\.superpowers\sdd\vitest-3121.log` |
| `cargo test --lib`：**936 passed / 0 failed / 1 ignored**（比探针期 937 少 1，正是删掉的探针） | `...\.superpowers\sdd\cargo-test-lib-3121b.log` |
| `eslint .` = **180 errors / 0 warnings**；本轮改动的 7 个文件**单检 0 问题** | `...\.superpowers\sdd\eslint-3121.log`（全量日志）；单检命令 `npx eslint <7 个文件>` 无输出 |
| 无 eslint 回归 | 基线口径 `eslint .` = 188 errors（2026-10-23 用 `git stash` 在 HEAD 上同一条命令量得）；本轮 180 ≤ 188。**基线数字随提交变，复量时务必用同一口径** |
| shell 自己就把 `.lnk` 解析成目标并返回目标图标 | 探针输出 `PROBE link_equals_target=true`（日志 `...\.superpowers\sdd\cargo-test-lib-3121.log`） |
| 侧车三处部署位仍是 0.5.18（sha256 全等 `A7424E1273A945123BBFFBD29ED04ACF30722C5E9E6B5D2EA891D3519281780D`） | `node -e` 逐一哈希 |

### 4.2 推断（逻辑成立，但没有真机截图）

- `.lnk` 在**任意**图标主题下都会走 `invoke('get_system_icon')` → Rust `resolve_shell_icon_path`
  → shell 返回**目标图标**。单测层面验证了「确实调用了 system 图标管线」，
  但**没有在打包产物里看到图标**。
- 地址栏输入框与建议行在 Windows 上显示 `C:\...`。单测验证了 DOM `input.value`，
  但**没有在打包产物里截图**。
- 断链 `.lnk` → 目标解析失败 → 回退 `.lnk` 自身 → shell 返回通用快捷方式箭头图标
  （与文档口径「断链回退通用快捷方式图标」一致）。**未在真机确认外观。**

### 4.3 未验证（本轮明确没做）

- **真机验收**（`tauri build` 产物 + 截图取证）—— 本轮已启动打包，
  日志 `...\.superpowers\sdd\tauri-build-3121.log`，脚本 `...\.superpowers\sdd\build-3121.cmd`。
- `IShellLinkW::GetIconLocation`（快捷方式**自身指定**的自定义图标）——
  `file_icon_provider` 不支持按 icon index 取图标，需自写 HICON→RGBA 管线
  （`Win32_Graphics_Gdi` feature 已开）。文档口径（CHANGELOG / README / README.zh-CN
  三处一致）是「显示**目标**的图标」，故列为**后续增强**，不在本轮范围。
- 发版全流程（暂停中）。

---

## 五、证据路径

| 内容 | 路径 |
|---|---|
| 工作记录（过程、探针、方案演进） | `sigma-file-manager\docs\work-2026-11-06-shortcut-icon-and-native-separator.md` |
| 上一轮 focus-sync 交接 | `sigma-file-manager\docs\handoff-2026-11-06-focus-sync-save-dialog-fix.md` |
| 用户原始截图（地址栏 `/`） | `E:\Users\Duanyi\.minimax\.minimax\v2\assets\2026\11\06\17-53-15-779-asset_20261106-175315-779_b360770c4f7a_22f9282d-image.png` |
| vitest 日志 | `.superpowers\sdd\vitest-3121.log` |
| cargo test 日志 | `.superpowers\sdd\cargo-test-lib-3121b.log` / `cargo-test-lib-3121b.log` |
| eslint 日志 | `.superpowers\sdd\eslint-3121.log` |
| 探针日志（结论） | `.superpowers\sdd\cargo-test-lib-3121.log` |
| 打包脚本 / 日志 | `.superpowers\sdd\build-3121.cmd` / `tauri-build-3121.log` |
| 三道门禁脚本 | `.superpowers\sdd\run-gates-3121.cmd` |
| 代码片段存档 | `.superpowers\sdd\snips-3121.txt`、`snips-rust-3121.txt`、`snips-final-3121.txt`、`snips-utiltest.txt` |

---

## 六、本轮踩到的工具坑（写给下一棒）

1. **bash 里 `Set-Location` 后再写文件会被硬安全策略拦掉**
   （`write/delete targets cannot be verified safely after a command working directory change`）。
   ⇒ 一律把 `cd` 放进 `.cmd` 内部，bash 只负责 `cmd /c "<绝对路径>.cmd"`。
2. **`.cmd` 里 `echo` 不能出现 `>`** —— cmd 会当重定向符，连注释和 echo 文本里的都不放过
   （上一轮因此在仓库根目录多出一个名为 `3.6.0)` 的垃圾文件）。
   描述性文字改用 `to` / `=>`。
3. **`.cmd` 里 `setlocal EnableDelayedExpansion` + `!ERRORLEVEL!`**（不是 `%ERRORLEVEL%`）。
4. **`.cmd` 运行期间绝对不能编辑它** —— cmd.exe 按字节偏移逐行读，改动会让后面几行
   执行到别处的指令片段（上一轮把版本号从 3.6.0 顶到 3.7.0，产物全丢）。
   要改就**先停任务**，再**新建一个文件名**重跑。
5. **`npm` 在 Windows 上是 `.cmd` shim**，在 `.cmd` 里要 `call npm.cmd` / `call npx.cmd`。
6. **`vitest --reporter=basic` 在本仓库这版不存在**，会直接中止。用默认 reporter，
   再抓 `Test Files` / `Tests` 两行。
7. **`Grep` 工具的 `output_mode` 在本环境对单文件路径只回 `Found N files`**，
   取内容不可靠 ⇒ 改用 `read`（offset/limit）或 `node -e` 精确抽取（两者都验证可用）。
8. **同一个文件的多次 `edit` 不能并行**（read-modify-write 竞态）；不同文件可以并行。
9. **绝不用 PowerShell 的 `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件**
   （ANSI 解码会把中文读成乱码再写回去）。改 `.vue`/`.ts`/`.json`/`.rs` 用编辑工具；
   批量改 JSON 用 node `fs.readFileSync/writeFileSync(..., 'utf8')`；
   写完验 `s.match(/\uFFFD/g)` 为空。

---

## 七、下一步（按优先级）

1. ~~真机验收~~ —— **已完成**（2026-11-06，结论见文末「八、真机验收结论」）
   - 打包：`cmd /c "F:\soft\00selfmade\filemanager\.superpowers\sdd\build-3121.cmd"`
     （**故意不跑 `publish-bundle-to-release.mjs`**，避免覆盖 `release/` 里的 3.12.0 包）。
   - 装包后验两件事：① 地址栏编辑器输入框 + 建议行显示 `C:\...`；
     ② 把文件图标主题切成非 system（例如内置 default），`.lnk` 仍显示**目标程序**图标。
   - 取证手段：`PrintWindow(hwnd, hdc, 2)` / 屏幕 BitBlt（hover、菜单必须用后者）；
     **UIA 读不到 Radix Toast**，toast 出现≠UIA 查得到。
   - **验收脚本开头必须 `SetProcessDPIAware()`**，否则 UIA 物理像素与
     `SetCursorPos`/`GetWindowRect` 的逻辑像素差 1.5 倍（上一轮「点按钮毫无反应」的真凶）。
   - 本机 DPI 144（150%），量到的值 ÷ 1.5 才是 CSS 逻辑像素。
2. **发版（用户放行后再做）**
   - 顺序：`release:prepare --bump patch` → `--write-notes` → `upload` → `verify` → `github`。
   - ⚠️ **公网 3.12.0 安装包内嵌的侧车仍是 0.5.15（sha256
     `b061d8a25445f29ed1da7597e969700b9cd9ffdc0fc1191a2a9a3171373a1a7d`，2,841,600 B）**，
     也就是说 focus-sync 的「另存为」修复**没进任何公开发布物**，必须靠下一次
     `tauri build` 把 0.5.18 带进去，并在升级日志里写明。
   - 新版本升级日志还要写：① 快捷方式图标不再受图标主题影响；
     ② 地址栏编辑器与建议行改用 Windows 原生 `\`。
3. **遗留**：`C:\Users\Duanyi\afm-accept-3120\` 与 `afm-accept-3120\dest\` 两个空目录
   需用户手工删除（trash 拒收工作区外目录级删除）。
4. **后续增强**（未做）：`GetIconLocation` 自定义图标支持（HICON→RGBA 自绘管线）。


---

## 八、真机验收结论（2026-11-06 已完成）

> 本节为 2026-11-06 晚补记，覆盖上文「七、下一步」的第 1 项。

### 8.1 验收环境

- 产物：`src-tauri\target\release\bundle\nsis\Alpha File Manager_3.12.0_x64-setup.exe`
  （45,115,317 B，2026-11-06 18:30:34，`TAURI_BUILD_EXIT=0`）
- 已安装 exe：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`
  （81,321,472 B，2026-11-06 18:29:54，即本轮新构建）
- ⚠️ **故意没有跑 `publish-bundle-to-release.mjs`**：`release/` 里的 3.12.0 正式包未被覆盖。

### 8.2 地址栏分隔符 —— PASS（像素证据）

- 证据：`F:\soft\00selfmade\filemanager\.superpowers\sdd\accept-3121-shots\04-input-zoom.png`
- 事实：地址栏编辑器浮层（`ControlType.Window | 编辑地址 | 635,95 1291x1178`）内的输入框，
  **肉眼可见反斜杠**：`…le\filemanager\.superpowers\sdd\accept-3121-fixtures`。
- 这同时证明「显示层原生化」真的生效：粘贴进来的原始反斜杠会先被 `normalizePath`
  转成正斜杠（内部语义不变），**显示时又变回反斜杠**。

### 8.3 快捷方式目标图标 —— PASS（像素对照）

- 夹具目录：`…\.superpowers\sdd\accept-3121-fixtures\`
  - `notepad.lnk`（目标 `C:\Windows\System32\notepad.exe`）
  - `demo.txt.lnk`（目标 `demo.txt`）
  - `broken.lnk`（目标 `C:\nonexistent-xyz\gone.exe`，故意断链）
  - `demo.txt`（对照组，真实文本文件）
- 主证据：`accept-3121-shots\07-iconstrip.png`
  （4 个 8× 放大图标拼条，顺序 `notepad.lnk | demo.txt.lnk | broken.lnk | demo.txt`）
  ＋逐个 `accept-3121-shots\07icon-*.png`
- 独立对照：`accept-3121-shots\08-compare-ref-vs-app.png`
  - 左 1 = 系统 `ExtractAssociatedIcon(notepad.exe)`，左 2 = 应用内 `notepad.lnk` 的图标
  - 左 3 = 系统 `ExtractAssociatedIcon(demo.txt)`，左 4 = 应用内 `demo.txt.lnk` 的图标
  - **成对形状一致** ⇒ 应用显示的确实是**目标**的图标。
- 三个 `.lnk` 的图标**互不相同**（记事本 / 文本文档 / 通用应用窗口），
  没有任何一个长着通用「快捷方式小箭头」⇒ 「快捷方式显示快捷方式关联的图标」已达成。

### 8.4 本轮新增的坑（下一轮别再踩）

1. **应用启动落在仪表盘路由 ⇒ 文件列表被盖住 ⇒ 截出来的是仪表盘**。
   `.lnk` 的 DOM 节点在 UIA 树里存在（名字、坐标都对），但屏幕像素是仪表盘的，
   于是「看起来图标不对」其实是**截错了页面**。
   ⇒ 解决办法：`user-settings.json` 的 `startupPage` 改成 `'navigator'`
   （类型见 `src/types/user-settings.ts` 的 `StartupPage`；路由名 `navigator` → `/navigator`），
   **先杀进程再改**（应用退出时会把内存状态写回磁盘）。
2. **图标拼接报 `System.Object[] → System.UInt32`**：在 PowerShell 里对「可能是数组的变量」
   取 `.Width`/`.Height` 会触发成员枚举，返回 `Object[]`。
   ⇒ 尺寸在创建时就存成显式 `int`；拼图用 `[System.Drawing.Rectangle]::new(...)`，
   不要用 `New-Object Type(a,b,c,d)`。
3. `System.Windows.Automation.Point` **在本机 PS 5.1 解析不到**
   （`Unable to find type [System.Windows.Automation.Point]`）
   ⇒ `AutomationElement.FromPoint` 这条可见性校验走不通；
   改用「图标节点自身有非空 BoundingRectangle」＋ 截图肉眼复核。
4. 应用窗口左下角被「已固定 / 最近使用 / 全部」大浮层盖住时，文件列表只剩左侧一条窄缝
   （图标列在物理 x≈606）。⇒ 裁剪区域必须避开浮层；本轮靠浮层左缘的窄条取到了 4 个图标。
5. 图标节点特征：**行内 28×28 的小图**，位置约 `x = 行左缘 + 35`、`y = 行上缘 + 37`。
   取法：在行元素的 descendants 里挑「12..110 px 的近正方形、且在行左缘 +320 px 以内」的最小的那个。
6. 应用路由状态由 `user-settings.json` 决定，但**启动目录**由 `user-workspaces.json` 决定；
   两个都要改，缺一个就量不到想要的那个目录。

### 8.5 用户真实数据的恢复（已完成）

- 恢复顺序：**先杀 `alpha-file-manager.exe` → 再 restore**（AGENTS.md 老坑）。
- `user-workspaces.json`：622 B（夹具）→ **2987 B**，9 个目录标签（含 `F:/soft/00selfmade/writedown`）✓
- `user-settings.json`：`startupPage` `navigator` → **`last`**，5722 B ✓
- 备份原件：`…\.superpowers\sdd\user-workspaces.acc3121-backup.json`、
  `…\.superpowers\sdd\user-settings.acc3121-backup.json`
- 恢复后 `Get-Process alpha-file-manager` = 0 ✓
- 验收脚本：`…\.superpowers\sdd\afm-accept-3121e.ps1`（纯 ASCII）；
  日志 `accept-3121e-run.log`、摘要 `accept-3121e-summary.txt`、UIA dump `accept-3121e-uia-dump.txt`。

## 九、发版结论（3.12.1，2026-11-06 已完成并线上核验）

用户 2026-11-06 19:38 指令「请打包、发版，push 到 GitHub 和 aiec，更新日志、readme、版本文件等」，
解除了此前「暂停发版」的旧约束。以下每一行都有命令退出码或线上回读支撑。

### 9.1 版本与发布内容

- 版本走 **patch：3.12.0 → 3.12.1**。三项改动都是补齐 3.12.0 已宣称但未真正到位的行为，
  外加侧车 bug 修复，不构成新功能，因此升级日志口径全部是 **Fixed，不写 New**。
- 发布内容三项：① 快捷方式显示目标图标，不再受图标主题与「系统图标」开关影响；
  ② 地址栏编辑器与建议行改用 Windows 原生 `\`（内部仍是正斜杠语义，光标不跳）；
  ③ focus-sync「另存为」侧车修复 **0.5.15 → 0.5.18**。

### 9.2 打包与产物（`PREPARE_EXIT=0`）

- `npm run release:prepare -- --bump patch`：vite `8.1.3` / 4013 modules，
  Rust `Finished release profile in 3m 53s`、`Compiling alpha-file-manager v3.12.1`。
- `package.json` = `3.12.1`；`src-tauri/Cargo.toml` `version = "3.12.1"`。
- `release\Alpha File Manager_3.12.1_x64-setup.exe` **45,123,898 B @19:54:36**；
  3.12.0 正包（45,112,805 B）**未被覆盖**。
- sha256 = `e54266295931a2e930998e6dcf349e5aee4a0d0f0bde4e234772e9796e8a5b76`，
  与站点 `version.json` **成对更新**（这是发版流水线的硬规则，不能只改一边）。
- **内嵌侧车坐实**：`src-tauri\target\release\tools\focus-sync-sidecar.exe`
  = 2,854,400 B / sha256 `A7424E1273A945123BBFFBD29ED04ACF30722C5E9E6B5D2EA891D3519281780D`
  = **0.5.18**。公网 3.12.0 安装包内嵌的是带 bug 的 0.5.15，0.5.18 是靠本次打包才进包。
- ⚠️ 侧车 sha 的权威位置是 `extensions\kizemo.focus-sync\bin\focus-sync-sidecar.exe`，
  **不是** `src-tauri/resources/tools/`。

### 9.3 文档盖章（17 份全 `OK`、`FFFD=0`、复跑全 `SKIP`）

`…\.superpowers\sdd\stamp-docs-3121.cjs`：根 `CHANGELOG.md` 171902→173743 B、
13 个非中文 locale 各 +1841 B、`ch` 58209→58876 B、`README.md` 21837→22552 B、
`README.zh-CN.md` 11291→11581 B。`assets` 目录没有 changelog，已跳过。
`verify-changelog.mjs`：47 条目、`[3.12.1] - November 2026` 顺序正确、`OK CHANGELOG.md 覆盖当前版本`。
文案按文件惯例分流（根 CHANGELOG 与 13 locale 用英文，`public/changelog/ch/` 用中文段落式，
README 两份各自语言表格），三处口径一致。README 只新增 3.12.1 小节，不删既有 3.9.0 小节。
站点 `index.html:130` 动态读 `version.json`，页面版本文案**不手改**。

### 9.4 门禁（分项口径，`GATE_SUMMARY ts=0 vchg=0 eslint=1 vitest=0`）

- `vue-tsc --build` = **0**；`verify-changelog` = **0**；
  `vitest` = **0** ⇒ `Test Files 289 passed (289)`、`Tests 2706 passed (2706)`、Duration 84.27 s。
- `eslint . --max-warnings 1` = **1** ⇒ `180 problems (180 errors, 0 warnings)`。
  **已证实 180 条全部不在本次改动文件里**：对 `use-navigator-item-icon`、`address-bar-editor`、
  `normalize-path`、`system-icons`、`user-stats`、`favorite-categor` 六个 pattern 在 lint 报告里
  逐条匹配，命中数均为 0。报错文件是 `file-tree.ts`、`folder-tree.ts`、`open-in-explorer.ts` 等存量，
  规则集中在 `@stylistic/object-property-newline` / `eol-last` / `padding-line-between-statements`。
- 历史基线 **186–188 errors** ⇒ 180 低于基线，**不是回归**。
  结论：**`npm run check` 在本仓设计上不可能 exit 0**，对外汇报一律按分项口径给数字。

### 9.5 winget manifest 3.12.1（`CM_EXIT=0`，8 项全绿）

- `docs/winget/manifests/k/kizemo/AlphaFileManager/3.12.1/` 四件套齐：
  `installer.yaml` 703→703 B、`locale.en-US.yaml` 2526→2918 B、`locale.zh-CN.yaml` 1707→1981 B、
  `kizemo.AlphaFileManager.yaml` 293→293 B。4 个新文件全 `FFFD=0`、LF。
- `node docs\winget\check-manifest.mjs 3.12.1` 8 项全 `+`，含
  `InstallerSha256 matches the local installer (45123898 bytes)`、
  `InstallerUrl points at this version's GitHub release asset`。
- **只本地补齐、不提 winget-community 仓库的 PR**（用户口径）。
- 盖章脚本 `…\.superpowers\sdd\stamp-winget-3121.cjs` 的写法要点（第一版锚点式替换已失败）：
  改为「`rmSync` 3.12.1 → 从 3.12.0 整体复制 → 全局 `split/join` 替换 → block scalar 版本段整行替换」，
  天然可重复。四个坑见 §9.8。

### 9.6 aiec 站点上传 + 线上核验（`UPLOAD_EXIT=0`）

- `npm run release:upload` 后自动跑 `verify`：23 个历史安装包全部 `OK md5+size 一致`，
  `[DONE] 已上传到 https://www.aiec.fun/alpha-file-manager/`。
- `verify` 回读：线上版本 **3.12.1**、发布日期 `2026-11-06T11:54:41Z`、
  清单 sha256 与本地包 sha256 **逐字相同**（`e54266295931a2e9…6e8a5b76`）、
  **源站包大小 45123898**（绕过缓存）、**边缘包大小 45123898**（`CF-Cache-Status=MISS`）⇒
  `通过：版本号 / 大小 / sha256 三项均一致（源站与边缘都已生效）`。
- 口径提醒：`verify` 只有**源站**大小不一致才 fail，Cloudflare 边缘不一致只 warn。

### 9.7 GitHub Release + git（`GITHUB_EXIT=0`）

- `git add -A` + `git reset -q -- extensions/kizemo.focus-sync/sidecar/src/bin` ⇒ `STAGED=45`。
  含 17 份文档 + 3 份新增 docs + 4 个 winget yaml + 7 个 sidecar `.rs` + 两个版本文件 + `Cargo.lock` +
  两个 workspace 成员 `Cargo.toml` + `system_icons.rs` + `use-navigator-item-icon.ts` 及测试 +
  4 个地址栏前端文件 + 2 个测试。
- `git commit -F …\.superpowers\sdd\commit-3121.txt` ⇒ **`dc173bd8`**，父提交 `9da52a16`。
- `git push origin main` ⇒ `9da52a16..dc173bd8`（`PUSH_MAIN_EXIT=0`）；
  `git push origin v3.12.1` ⇒ `* [new tag]`（`PUSH_TAG_EXIT=0`）。
- **顺序必须是 push 先于 `release:github`**：`scripts/release-publish.mjs` 的 `github()`
  走 `gh release create <tag>` 且**不带 `--target`** ⇒ 若 tag 不在远端，Release 会指向旧提交。
  已回读 `gh api repos/kizemo/alpha-file-manager/tags`：`v3.12.1 -> dc173bd85d8c245adfde16cd129f3bad089e01ad` ✓
- Release 回读（`gh api …/releases/tags/v3.12.1`）：`tag_name=v3.12.1`、
  `name=Alpha File Manager v3.12.1`、**`body` 长度 888**（非空正文，不是漏跑 `--write-notes` 的空壳）、
  资产 `Alpha_File_Manager_3.12.1_x64-setup.exe` `size=45123898` `state=uploaded`。
  https://github.com/kizemo/alpha-file-manager/releases/tag/v3.12.1

### 9.8 本轮新增的坑（写给下一棒）

- **`.cmd` 的 `echo` 里不能出现 `>`**：cmd.exe 把它当重定向符，连注释和 echo 文本里的都不放过。
  症状是「日志少一行 + `git status` 冒出一个看不懂的垃圾文件」。描述性文字改用 `to` / `=>`。
- **`.cmd` 运行期间绝对不能编辑它**：cmd.exe 按字节偏移逐行读，中途改文件会让后面几行偏移错位，
  执行到别处的指令片段（2026-10-24 就因此重复跑了 `release:prepare --bump minor`，版本被顶掉、产物全丢）。
- **winget 盖章四坑**（第一版脚本全部踩中）：① `installer.yaml` 里 `PackageVersion: 3.12.0`
  天然出现 **2 次** ⇒ 「唯一命中」断言必败，要用全局 `split/join`；② locale 文件的 `Description` 是
  **YAML block scalar**（`Description: |-`），要替换的是缩进的版本段行
  （en-US 以 `Version 3.12.0` 开头、zh-CN 以 `3.12.0 版` 开头），用
  `/^  3\.12\.0[^\n]*$/m` 与 `/^  Version 3\.12\.0[^\n]*$/m` 各唯一命中；
  「本地网络浏览器。」属于 `ShortDescription`，当 `Description` 锚点会 0 命中；
  ③ 资产文件名 `Alpha_File_Manager_3.12.0_x64-setup.exe` 里的版本号会漏替 ⇒ `check-manifest` 报
  `InstallerUrl … expected …3.12.1…`；④ `ReleaseDate` **只在 `installer.yaml` 存在**，
  别对四个文件统一断言。
- `check-manifest.mjs` **只传裸版本号**；传目录会被拼成不存在的路径而报四文件全 missing。
- **FFFD 探针不要写成** `s.replace(String.fromCharCode(65533),'#')` 再 `match(/#/g)` ——
  `replace` 只替换首个，数到的是 `# yaml-language-server` 注释行，**自造假阳性**。
  正确写法：`(s.match(/\uFFFD/g)||[]).length`。
- **后台任务 `status=succeeded` 不代表脚本内部命令成功**（`.cmd` 没有 `exit /b`）；
  `npm` 输出经管道缓冲，跑完才落盘。判定终态一律看日志里的 `*_EXIT=` / `GATE_SUMMARY` 行。
- **上传日志里的中文显示成 `` 是 stdout 代码页问题**（子进程输出按错误代码页解码），
  不是上传失败 —— 结论以 `[verify] 通过` 与 `UPLOAD_EXIT=0` 为准。
- `bash` 里 `Set-Location` 之后再写文件会被硬策略拒绝
  （`write/delete targets cannot be verified safely after a command working directory change`）⇒
  写文件一律用 `write` / `edit` 工具 + 绝对路径。
- `gh … --json` 走管道时 stderr 会污染 JSON ⇒ 用 `--jq` 在服务端裁字段，或 `2>$null`。

### 9.9 遗留事项

- `extensions/kizemo.focus-sync/sidecar/src/bin/repro_save_dialog.rs` **未入库**（一次性复现工具，
  已在 commit message 里说明）；侧车二进制由 `.gitignore:37/40` 排除。
- `C:\Users\Duanyi\afm-accept-3120` 两个空目录待用户手工删除（不要代为删除）。
- `IShellLinkW::GetIconLocation` 列为后续增强；本轮 Rust 侧**未改生产逻辑**
  （探针实证 shell 自身透传目标图标）。
- 上一轮 `%TEMP%\afm-baseline-eslint` 基线 worktree 已被清掉，现量 HEAD 需重建。
- 本机 focus-sync 侧车 0.5.18 在跑（计划任务 `KizemoFocusSync`，端口 37421）。
