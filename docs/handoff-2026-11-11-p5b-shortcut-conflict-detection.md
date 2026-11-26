# Handoff — P5b：系统级热键冲突检测（Ctrl+Shift+P 修复）与 3.13.7 发版准备

- 生成时间：2026-11-11
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（会话工作目录必须是该目录本身，否则 `AGENTS.md` 不会注入）
- 分支/提交起点：`main` = `2fcf1eb8`（与 `origin/main` 同步）
- 装机版本：3.13.6，`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`
- 上一份交接：`docs/handoff-2026-11-10-p5-release-3.13.6.md`

---

## 1. 本轮目标

继续推进上一份交接文档「下一步」的第 1 项：修掉装机版上 **`Ctrl+Shift+P` 命令面板不弹出**。

---

## 2. 根因（已验证，勿再追查）

`Ctrl+Shift+P` 在这台机器上**已被外部第三方程序通过 Win32 `RegisterHotKey` 注册为系统级全局热键**，
按键在 OS 层被吞掉，**根本没有进入 AFM 的 webview**。这不是 AFM 的代码缺陷。

### 2.1 阳性/阴性对照探针结果（已验证）

| 组合键 | 结果 | 含义 |
| --- | --- | --- |
| `Ctrl+P` | `FREE` | 阳性对照：探针本身工作正常 |
| `Ctrl+Shift+P` | `TAKEN (Win32Error=1409)` | `ERROR_HOTKEY_ALREADY_REGISTERED` |
| `Ctrl+Shift+Space` | `FREE` | 可作为真机验收的替代键 |
| `Ctrl+Shift+A` / `K` / `W` | `TAKEN:1409` | 整组 `Ctrl+Shift+字母` 被占 |
| `Ctrl+Alt+P` | `TAKEN:1409` | 另有 `Ctrl+Alt` 系占用 |
| `Alt+Shift+P` | `FREE` | — |

`TAKEN_COUNT=4` ⇒ 本机存在一组系统级热键占用（疑似某输入法/启动器类软件）。

### 2.2 源码层已逐段证明 AFM 侧链路无缺陷（已验证）

`src/stores/runtime/shortcuts.ts`：
- 定义：`shortcuts.ts:169-179` `{ ctrl, shift, key: 'p' }`，`scope: 'global'`，`conditions: {}`。
- `modifiersMatch`（1147-1156）、`matchesShortcut`（1158-1214）两端都 `.toLowerCase()`，且有
  `codeLower === 'key' + expectedKey` 兜底。
- `isShortcutScopeRouteActive`（~1741-1754）的 `default` 分支用 `never` 技巧返回真值 ⇒ `global` 放行。
- `checkShortcutConditions`（1766-1786）对 `{}` 不拦。

挂载与执行：
- handler 注册点唯一：`src/modules/command-palette/command-palette.vue:292-297`（`onMounted`）。
- 组件挂载：`src/layouts/window/app.vue:37` 无条件挂载。
- `executeShortcut` 与工具栏按钮走同一路径。
- 运行期无 `handlers.clear()`；原生菜单无加速键；用户数据无扩展键位覆盖。

### 2.3 已排除、且**不要重跑**的取证路径

| 路径 | 结论 |
| --- | --- |
| `NtSuspendProcess` 逐个挂起候选进程 | `OWNER = NOT FOUND`，**结论无效**（挂起不释放全局热键） |
| 8 个配置目录 grep `ctrl+shift+p` | 0 命中 |
| 25 个候选主 exe + 8 个大镜像的字符串搜索（ASCII + UTF-16LE） | 0 命中 |

剩余手段全部不成立：
- 破坏性 `UnregisterHotKey(hwnd, id)` 暴力扫描：无法得知被移除热键的 mod/vk，无法原样恢复，会破坏用户其他软件快捷键。
- `SendInput` 真按一次再对比窗口列表：会在用户屏幕弹出未知 UI。

**结论：占用者身份不追了。** 修复方向改为「把冲突暴露给用户」。

---

## 3. 本轮已完成的代码改动（已落盘，未通过门禁）

设计：**不改默认键**（保留 VS Code 肌肉记忆）+ 在「设置 → 快捷键」做**系统级热键冲突检测**。
只在前端完成，复用已授权的 `@tauri-apps/plugin-global-shortcut`，
**不新增 Rust `#[tauri::command]`、不改 capabilities**。
冲突**只提示、不阻断保存**（用户可在占用程序退出后继续用该键）。

### 3.1 `src/stores/runtime/global-shortcuts.ts`

- `:95-108` 新增模块级导出类型：
  ```ts
  export type ShortcutAvailabilityProbe = 'free' | 'app-owned' | 'occupied';
  ```
- `:291-323` 新增 `async function probeShortcutAvailability(keys: ShortcutKeys): Promise<ShortcutAvailabilityProbe>`：
  - 无 modifier 或无 key ⇒ `'free'`（提前返回，不碰插件）。
  - `shortcutKeysToTauriFormat` 返回空 ⇒ `'free'`。
  - `registeredShortcutOwners.has(shortcutString)` ⇒ `'app-owned'`（**保护自家已注册热键，不能被探测误杀**）。
  - `register(shortcutString, () => {})` 抛错 ⇒ `'occupied'`。
  - 成功则立刻 `unregister(shortcutString)` 后返回 `'free'`；`unregister` 失败只 `console.error`，仍返回 `'free'`。
- `:696` store 返回对象加入 `probeShortcutAvailability,`（紧跟 `getShortcutKeys,`）。

> 刻意**不依赖** `@tauri-apps/plugin-global-shortcut` 的 `isRegistered`：
> 单测 mock 未提供该方法，且它只反映本应用自身注册状态，无法判外部占用。

### 3.2 `src/stores/runtime/shortcuts.ts`

- `:1449` 之后新增 `function getShortcutBindingKeys(definition: ShortcutDefinition): ShortcutKeys`，
  内部调 `resolveShortcutBindingKeys`。
- `:2135` store 返回对象加入 `getShortcutBindingKeys,`（紧跟 `getShortcutBindingLabel,`）。

### 3.3 `src/modules/settings/ui/categories/shortcuts/shortcuts.vue`（5 处）

1. `:173-177` 新状态：
   ```ts
   const systemShortcutConflict = ref(false);
   const systemShortcutConflicts = ref<Set<string>>(new Set());
   let systemConflictProbeToken = 0;
   let hasProbedSystemShortcuts = false;
   ```
2. `:179-235` 新函数：
   - `getShortcutDefinitionKey(definition)` ⇒ `` `${definition.id}-${definition.bindingSlot ?? 0}` ``。
   - `isShortcutSystemTaken(definition)`。
   - `refreshSystemShortcutConflict(keys)`：带 `systemConflictProbeToken` 竞态守卫，只认最后一次探测结果。
   - `probeVisibleShortcutsSystemConflicts()`：遍历 `visibleLocalShortcuts`、跳过 `isReadOnly`；
     **假阳性护栏**：`probeCount >= 3 && occupiedCount === probeCount` ⇒ 全部清空（说明插件未就绪，而不是「全都冲突」）。
3. `watch(recordedKeys, ...)` 内追加 `void refreshSystemShortcutConflict(keys);`。
4. `watch(isDialogOpen, ...)` 关闭时追加 `systemShortcutConflict.value = false;`。
5. `onUnmounted` 之前追加 `nextTick(() => { void probeVisibleShortcutsSystemConflicts(); });`
   （组件**没有 import `onMounted`**；`nextTick` 回调在 setup 同步段之后执行，可安全访问后声明的 `visibleLocalShortcuts`）。
6. 模板：本机快捷键表格（`v-for="definition in visibleLocalShortcuts"`，起始 `:1525`）的
   `shortcuts-table__cell--keybinding` 单元内，kbd/unassigned 之后追加
   `<Tooltip v-if="isShortcutSystemTaken(definition)">` + `<AlertTriangleIcon :size="12" class="shortcuts-table__system-conflict" />` 徽标，tooltip 文案 `t('shortcutsUI.shortcutTakenBySystem')`。
7. 模板：编辑器冲突提示区（`globalShortcutRegisterFailed` 区块之后）追加 `v-if="systemShortcutConflict"` 的
   `.shortcut-editor__conflict` 区块，文案 `{{ t('shortcutsUI.shortcutTakenBySystem') }}: {{ t('shortcutsUI.shortcutSystemConflictHint') }}`。
8. `<style>`：`.shortcuts-table__kbd` 规则之后追加
   `.shortcuts-table__system-conflict`（`color: hsl(var(--destructive))`、`margin-left: .25rem`、`flex-shrink: 0`）
   与 `.shortcuts-table__system-conflict-label`。

### 3.4 i18n 17 个 locale（已落盘）

在 `src/localization/messages/*.json` 的 `shortcutsUI` 段、`shortcutRecorderGlobalCaptureHint` **之后**插入两个键：
- `shortcutTakenBySystem`
- `shortcutSystemConflictHint`

脚本：`.superpowers/sdd/add-locale-conflict-keys.cjs`（**行级插入**，不重排整份 JSON；插入后 `JSON.parse` 自检）。
执行结果：`files_with_new_key=17`。

文案策略：`en` 与 `ch` 为人工写；其余 15 个 locale 暂时回落英文（与项目既有「未翻译键回落英文」一致）。
后续如需本地化，只改这 15 个文件里的两个值即可。

---

## 4. 状态判定

| 项目 | 状态 | 证据 |
| --- | --- | --- |
| `Ctrl+Shift+P` 根因 | **已验证** | `.superpowers/sdd/hotkey-probe.log` + 矩阵探针 |
| AFM 侧链路无缺陷 | **已验证**（源码逐段） | 见 §2.2 |
| 冲突检测代码 | **已落盘，未验证** | 见 §3 |
| 17 locale 文案 | **已落盘**（脚本自检 JSON 合法） | `.superpowers/sdd/add-locale-conflict-keys.cjs` |
| `npm run check` | **PASS**（`CHECK_EXIT=0`；`Test Files 300 passed`、`Tests 3144 passed`、stylelint `--max-warnings 0` 无告警） | `.superpowers/sdd/p5b-check.log` |
| 版本号 3.13.7 | **未改** | — |
| `CHANGELOG.md` + `public/changelog/` 14 locale | **未写** | — |
| 构建 / 打包 / 装机 | **未做** | — |
| 真机键盘验收 | **未做** | — |
| 新增单测 | **未写**（`global-shortcuts.test.ts` 的 mock 已含 `register`/`unregister`/`unregisterAll`，**不含** `isRegistered`） | `src/stores/runtime/__tests__/global-shortcuts.test.ts:20-28` |

---

## 5. 下一步（严格按序）

1. **读门禁结果**：`.superpowers/sdd/p5b-check.log`，`CHECK_EXIT` 必须为 0。
   - 有错先修（大概率是 `no-unused-vars` 类：确认 `isShortcutSystemTaken` 已被模板引用、`ShortcutAvailabilityProbe` 已被使用）。
   - 绿灯前查 mtime/sha16 稳定性，避免撞上并发写入的瞬态假红。
2. **补单测**（推荐，不阻塞发版）：在 `src/stores/runtime/__tests__/global-shortcuts.test.ts` 增 3 条：
   - `registerMock` 正常 ⇒ 返回 `'free'`，且 `unregisterMock` 被调用一次（证明探测不残留注册）。
   - `registerMock` reject ⇒ `'occupied'`。
   - 对一个已由本应用注册的组合键 ⇒ `'app-owned'`，且 `registerMock` **未被再次调用**。
   运行：`npm --prefix <abs> run test:unit:run -- src/stores/runtime/__tests__/global-shortcuts.test.ts`
   （**不要**用 `npm exec -- vitest`，会把 vitest 根解析到父目录。）
3. **改版本到 3.13.7**：`package.json`、`src-tauri/Cargo.toml`、
   `src-tauri/default-file-manager-common/Cargo.toml`、`src-tauri/default-file-manager-launcher/Cargo.toml` + `Cargo.lock`。
4. **写 `CHANGELOG.md`** + `public/changelog/` 14 个 locale（中文 locale code 是 **`ch`**）。
   内容要点：新增「快捷键被系统或其他程序占用」的检测与提示（列表徽标 + 编辑器告警），默认键位不变，告警不阻断保存。
5. **门禁再跑一次**：`npm --prefix <abs> run check *> <abs log>`。
6. **构建**：后台跑 `npm run tauri:build`（末步只复制到 `release/`，不上传）。构建脚本放 `.superpowers/sdd/*.cmd`、**纯 ASCII**、**内部 `> logfile 2>&1`**，不要用 `Tee-Object`/`| Select-String` 挂 stdout 管道。
7. **静默装机**：`.superpowers/sdd/p5-install-run.ps1` 是上一轮的可复用模板。
8. **真机键盘验收**（`tauri build` 产物 + `SendInput`，release 包 `SFM_E2E_DATA_DIR` 被编译掉 ⇒ 只能「备份真实数据 → 灌夹具 → 验收 → 恢复」）：
   - **`Ctrl+Shift+P` 预期仍然失败**（外部占用，属已知限制，**不得标 PASS**）。
   - **判别性证据（必做）**：把 `toggleCommandPalette` 改绑到 `Ctrl+Shift+Space`（已验证 FREE）⇒ 命令面板能弹出
     ⇒ 证明快捷键 → handler → 面板链路完好。
   - **同时验证新功能**：设置 → 快捷键页里 `Ctrl+Shift+P` 那一行出现系统冲突徽标；编辑器内录制 `Ctrl+Shift+P` 出现系统冲突告警。
   - 快捷键验收前先补一次 `Escape`（首启「更新日志」模态窗会吞键）。
   - 输入字符一律 `KEYEVENTF_UNICODE` 逐字符键入（`wVk=0`、`wScan=码位`、键下 `0x0004`、键上 `0x0006`，24 字符一批、批间 30 ms）。
9. **还原用户数据**：先杀进程再 restore；真实数据在 `%APPDATA%\com.sigma-file-manager.app\user-data\`（5 个 JSON）。
10. **提交**（push 需授权；GitHub Release **未授权**）。

---

## 6. 工具坑（本轮新增/复核）

1. **`edit` 的 `old_string` 必须全文件唯一**。`<span v-else class="shortcuts-keybinding-unassigned">…</span></div>`
   在组件里出现 **4 次**（1370/1442/1558/1680），直接替换报 `Found 4 occurrences`。
   解法：带上带 `shortcutsStore.getShortcutBindingLabel(definition)` 的整段做锚点。
   **注意**：`v-for="definition in visibleGlobalAppShortcuts"`（:1344）与 `visibleLocalShortcuts`（:1525）是两张不同的表，
   徽标只加在**本机快捷键表**上。
2. **批量改 JSON 用 node 行级插入**，不要 `JSON.parse` + `JSON.stringify` 整份回写（会重排格式、炸出巨大 diff）。
   脚本插完必须 `JSON.parse` 自检一次。
3. **后台门禁/构建不要挂 stdout 管道**：`npm ... run check *> <log> 2>&1`，不要 `Tee-Object` / `| Select-String`。
4. `npm run test:unit:run`，不要 `npm exec -- vitest run`（vitest 根会解析到父目录）。
5. `git commit -F -` 不读 stdin ⇒ 消息先写临时文件再 `-F <file>`。
6. 跨工作区边界的写/删/改名写成 `.ps1` 再 `pwsh -File` 执行。
7. 不要用 `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件。
8. `.superpowers/sdd/*.cmd|*.ps1` 必须纯 ASCII。

---

## 7. 证据路径

- `.superpowers/sdd/hotkey-probe.ps1` / `.log`（三键探针，已验证）
- `.superpowers/sdd/hotkey-owner.ps1` / `.log`（**无效结论，勿重跑**）
- `.superpowers/sdd/add-locale-conflict-keys.cjs`（locale 插入脚本）
- `.superpowers/sdd/p5b-check.log`（本轮门禁输出）
- `.superpowers/sdd/p5-gates.log`、`p5-build-run.log`、`p5-install-run.ps1/.log`、`p5-restore.ps1/.log`（上一轮可复用）
- 装机 exe：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`
  （3.13.6：82,338,304 B，SHA256 `C54106F3F22EF31676BC811BF037783A0FEC5D3676839643E7B52E9D5EB5AD36`）
- 用户数据：`%APPDATA%\com.sigma-file-manager.app\user-data\`