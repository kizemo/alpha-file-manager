# 工作记录：快捷方式关联图标 + 地址栏原生分隔符（2026-11-06）

> 本文件是**过程记录**，不是交接文档。最终结论落在
> `docs/handoff-2026-11-07-shortcut-icon-and-native-separator.md`。
> 发版流程仍处于**暂停**状态（用户 2026-11-06 指令）。

## 一、任务

1. **补完「快捷方式显示快捷方式关联的图标」** —— 3.12.0 已在 CHANGELOG / README /
   README.zh-CN / `public/changelog/*` 宣称「A shortcut shows the icon of what it points at /
   快捷方式显示目标的图标」，用户指出该功能要求**尚未完成**。
2. **修复地址栏仍显示 `/` 而不是 Windows 原生 `\`**（用户截图：
   `E:\Users\Duanyi\.minimax\.minimax\v2\assets\2026\11\06\17-53-15-779-asset_20261106-175315-779_b360770c4f7a_22f9282d-image.png`，
   输入框内容 `C:/ProgramData/Microsoft/Windows/Start Menu/Programs/14AI工具/`）。

## 二、已核实的代码事实（勿重试）

### 2.1 地址栏

| 位置 | 事实 |
|---|---|
| `address-bar-editor-dialog.vue` L1241-1246 | `CommandInput` 直接 `v-model="query"` |
| `address-bar-editor-utils.ts` L318-321 | `ensureAddressBarDirectoryQuery` = `normalizePath(path)` + **强制补尾 `/`** |
| `address-bar-editor-utils.ts` L519 / L654 / L12-14 | `resolveAddressBarSuggestions` / `resolveAddressBarTypedTarget` / `canonicalizePath` **入口自带 `normalizePath`** ⇒ `query` 内部用 `/` 还是 `\` 都能被下游归一 |
| `address-bar-editor-utils.ts` L443-445 | `addressBarQueryMeansBrowseInsideDirectory` 判 `endsWith('/')` —— 尾斜杠承载「目录浏览」语义（调用点传入的已经是归一后的 `normalizedQuery`） |
| `address-bar-editor-suggestion-row.vue` L86-89 | `{{ suggestion.path }}` 直接输出内部 `/` 形式 |
| `normalize-path.ts` L223-233 | `toNativePathSeparators(path, isWindows)`：含 `://` 的路径原样保留（`sfm://remote/...`） |
| 3.12.0 改造范围 | 只接入了**面包屑**侧（`address-bar.vue` 的 `:title`、复制路径按钮），**漏了编辑器输入框与建议行** —— 这就是本次 bug 的准确定位 |

**已有测试锚点**：`address-bar-editor-dialog.test.ts` L367
`expect(input.value).toBe('C:/Folder/')` —— 这条断言已改成 `'C:\\Folder\\'`。
该测试文件已 mock `@/stores/runtime/platform` → `isWindows: true`，
`CommandInputStub` 有 `modelValue` prop + `update:modelValue` 事件（**公开 v-model 契约可用**）。

### 2.2 快捷方式图标

| 位置 | 事实 |
|---|---|
| `src-tauri/src/system_icons.rs` L289-308 | `resolve_shortcut_icon_target`：仅 `.lnk` → `dir_reader::resolve_shortcut_target_path` → 要求**绝对 + 存在** → 排除 WindowsApps 别名 → 否则 `None` |
| `src-tauri/src/system_icons.rs` L315-348 | `resolve_shell_icon_path`：`is_file()` 分支先试快捷方式目标，失败回退 `.lnk` 自身 |
| `src-tauri/src/dir_reader/read.rs` L722-781 | `resolve_windows_shortcut_target` 用 `IShellLinkW::GetPath(..., **SLGP_RAWPATH**)` ⇒ **不展开环境变量、不解析相对路径** |
| `use-navigator-item-icon.ts` L87-90 / L119 / L140 | **只有** `builtin:system` 主题或扩展主题加载失败时才走 shell 图标管线；否则走扩展主题 / `getDefaultFileIconComponent` 通用图标 ⇒ **`.lnk` 在非 system 主题下根本拿不到目标图标** |
| `user-settings.ts` L169-170 | `folderIconTheme` 与 `fileIconTheme` **默认都是 `builtin:system`** |
| `file_icon_provider-1.0.2` | 走 `SHCreateItemFromParsingName` → `IShellItemImageFactory::GetImage(SIIGBF_ICONONLY)` → **不支持按 icon index 取图标**（`GetIconLocation` 自定义图标要另写 HICON→RGBA 管线） |
| `src-tauri/Cargo.toml` | 直接依赖是 **`windows` 0.58**；已开 feature：`Win32_UI_Shell`、`Win32_UI_Shell_Common`、`Win32_System_Environment`、`Win32_Graphics_Gdi`、`Win32_System_Com` 等 |

### 2.3 一次性探针 —— 已跑完，结论推翻了一部分原计划

探针 `probe_lnk_shell_icon_and_raw_target`（跑完后已删除）：
- 命令：`cmd /c "F:\soft\00selfmade\filemanager\.superpowers\sdd\cargo-test-lib-3121.cmd"`
- 日志：`F:\soft\00selfmade\filemanager\.superpowers\sdd\cargo-test-lib-3121.log`

输出：

```
PROBE link_icon_is_some=true
PROBE target_icon_is_some=true
PROBE link_equals_target=true
PROBE env_raw_target=Some("%TEMP%\\afm-lnk-icon-probe\\env-target.txt")
PROBE env_raw_target_exists=false
```

**结论（重要，改变实现路线）**：

1. 探针里的 `link_icon` 是**直接把 `.lnk` 路径丢给 shell** 取到的图标（不经
   `resolve_shell_icon_path`），结果与直接取目标图标**完全相同**
   ⇒ **Windows shell 自己就会把 `.lnk` 解析成目标并返回目标图标**。
2. 因此 Rust 侧「先解析目标再取目标图标」这一层**不是必需的**；环境变量目标
   （`%TEMP%\...`）解析失败时回退到 `.lnk` 自身，shell 依然给出目标图标。
3. `SLGP_RAWPATH` 确实不展开环境变量（返回字面量 `%TEMP%\...`）—— 但这一条
   **不影响图标结果**，只影响 `resolve_windows_shortcut_target` 这个「跳转用」的解析。

⇒ **Rust 侧本轮不改逻辑**（只删除探针）；真正的缺口在**前端图标主题旁路**。

## 三、最终实现（与实际落盘一致）

### A. 地址栏 —— 「显示层原生化，内部语义不变」

- `address-bar-editor-utils.ts`
  - import 增加 `toNativePathSeparators`。
  - 新增导出 `toAddressBarDisplayPath(path, isWindows)`（薄封装，纯函数、可单测）。
- `address-bar-editor-dialog.vue`
  - import 增加 `toAddressBarDisplayPath`（复用已有的 `./address-bar-editor-utils` import 块，
    不新增重复 import 语句）。
  - 新增 `const displayQuery = computed(() => toAddressBarDisplayPath(query.value, platformStore.isWindows));`
    —— `platformStore` 本来就已实例化（`usePlatformStore` 已 import）。
  - 新增 `handleQueryModelUpdate(nextValue)`：
    `pendingEditorInputCaret = 输入框 selectionStart` → `pendingEditorInputQuery = normalizePath(nextValue)`
    → `query.value = pendingEditorInputQuery`。
    **内部 `query` 永远保持正斜杠**（`normalizePath` 归一），比较键 / 缓存键 /
    「尾斜杠 = 浏览目录内」语义全部不受影响。
  - 新增 `watch(displayQuery, ...)`：只有当 `pendingEditorInputQuery === query.value`
    （即这次变化**确实**来自用户输入）时才 `await nextTick()` 后 `setSelectionRange(caret, caret)`
    复原光标；程序化改写 query（打开对话框、上移一级）时守卫会丢弃这次恢复。
  - 模板：`CommandInput` 从 `v-model="query"` 改为
    `:model-value="displayQuery"` + `@update:model-value="handleQueryModelUpdate"`。
  - **4 个 `query.value = ...` 写入点（L441 / L535 / L754 / L1144）保持不变** ——
    它们写入的本来就是 `/` 形式，展示层统一换算。
- `address-bar-editor-suggestion-row.vue`
  - 新增 `usePlatformStore()` 实例化；模板改为
    `{{ toAddressBarDisplayPath(suggestion.path, platformStore.isWindows) }}`。
  - import 合并成一条：`import { toAddressBarDisplayPath, type AddressBarSuggestion } from './address-bar-editor-utils';`

### B. 快捷方式图标 —— 前端强制走 shell 管线

- `use-navigator-item-icon.ts`
  - 新增 `isShortcutEntry`：非目录 + （`extension` 去点后等于 `lnk` **或** 路径以 `.lnk` 结尾）。
    双重判断是为了不依赖 `entry.ext` 到底带不带点。
  - `useSystemIcons` 增加两个分支：
    `!enabled → false`（保持原语义：disabled 时本来就不会走 system）、
    **`isShortcutEntry → true`（无视用户选的文件图标主题）**。
  - 结果：`.lnk` 永远走 `useSystemIcon` → `invoke('get_system_icon')` →
    Rust `resolve_shell_icon_path` → shell 图标 = **目标图标**。
- `src-tauri/src/system_icons.rs`
  - **仅删除**探针测试与探针辅助函数，**未改任何生产逻辑**（见 2.3 的探针结论）。

### C. 明确不做（本轮范围外）

- `IShellLinkW::GetIconLocation`（快捷方式**自身指定**的自定义图标）：`file_icon_provider`
  不支持按 icon index 取图标，需要自写 HICON→RGBA 管线（`Win32_Graphics_Gdi` feature 已开）。
  文档口径（CHANGELOG / README / README.zh-CN 三处一致）是「显示**目标**的图标」，
  故列为**后续增强**。
- 发版（README / CHANGELOG / aiec 站点版本说明 → prepare → 打包 → 上传 → verify →
  GitHub Release）—— 用户明确指令暂停。

## 四、门禁口径

- `npm run test`（vitest 全量，默认 reporter；`--reporter=basic` 在本仓不存在）
- `eslint .`（整仓口径；基线数字随提交变，用 `git stash` 量 HEAD 同一条命令）
- `cargo test --lib`（`--lib` 限定，别把 bin crate 拖进来）
- 真机验收**必须**用 `tauri build` 打包产物（`tauri dev` 页面内容区全空白）
- 本轮门禁脚本：`F:\soft\00selfmade\filemanager\.superpowers\sdd\run-gates-3121.cmd`
  （纯 ASCII；内部 `> logfile 2>&1` 重定向 + `echo XXX_EXIT=!ERRORLEVEL!`）


---

## 五、真机验收结论（2026-11-06 补记）

- **地址栏分隔符：PASS**。像素证据 `accept-3121-shots\04-input-zoom.png`：
  地址栏编辑器输入框内肉眼可见 `…le\filemanager\.superpowers\sdd\accept-3121-fixtures`，
  即反斜杠；同时证明「内部正斜杠语义 + 显示层原生化」这条路线成立。
- **快捷方式目标图标：PASS**。证据 `accept-3121-shots\07-iconstrip.png`
  ＋ `accept-3121-shots\08-compare-ref-vs-app.png`：
  应用内 `notepad.lnk` 的图标与系统 `ExtractAssociatedIcon(notepad.exe)` 形状一致，
  `demo.txt.lnk` 与系统 `.txt` 文档图标一致；三个 `.lnk` 图标互不相同，
  没有任何一个是通用「快捷方式小箭头」。
- 验收脚本：`…\.superpowers\sdd\afm-accept-3121e.ps1`（纯 ASCII）；配套
  `accept-3121e-run.log` / `accept-3121e-summary.txt` / `accept-3121e-uia-dump.txt`。
- **关键前提**：应用必须启动在**文件浏览器路由**（把 `user-settings.json` 的
  `startupPage` 设为 `'navigator'`，且**先杀进程再改**），否则截到的是仪表盘，
  会得出「图标没生效」的假结论。
- 验收后已恢复用户真实数据：`user-workspaces.json` 回到 2987 B / 9 个目录标签，
  `user-settings.json` 的 `startupPage` 回到 `last`，`alpha-file-manager` 进程为 0。
- 发版**仍暂停**（用户指令）：README / CHANGELOG / 站点版本说明 / `release:prepare` /
  上传 / GitHub Release 都未动。
