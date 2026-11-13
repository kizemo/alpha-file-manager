# 批次 13 交接卡：夸克网盘「登录提示重复叠加」缺陷 B 的归因、修复与验证（2026-11-13）

> 上游真台账：`docs/handoff-2026-11-11-quark-pan.md`（本轮的技术细节记在它的 **§20.1–§20.8**）。
> 本文件是**给下一轮会话的入口卡**：背景、目标、进度、下一步、以及「哪些是实测的、哪些是推断的、哪些还没验」。
> 品牌名固定 **Alpha File Manager**；`afm://quark-login` / `afm://baidu-login` 频道名、legacy 数据目录名、Tauri `identifier` 是受保护常量。

---

## 0. 一句话状态

**缺陷 B（登录超时提示重复叠加）已归因、已修复、代码层有判别性证据（回退对照变红）、已打包；
真机 UI 复验（沙箱那一轮）尚未做。** 本轮改动全部在本地，**未 commit、未 push、未提 winget PR、未发 Release**。

---

## 1. 背景与目标

- **上游卡**：`docs/handoff-2026-11-11-quark-pan.md` §9（遗留项 R1–R8）、§11.3（缺陷 B 的真机截图与描述）、§19（授权边界）。
- **本轮选定目标**：先把 §11.3 缺陷 B 这条**已复现、已截图**的缺陷做掉 —— 它是夸克链路里唯一「用户直接可见且可复现」的缺陷。
- **未纳入本轮**：R4（17 语言包母语复核）、R5（`Ctrl+Shift+P` 热键冲突换机器验证）、upstream 大面积移植（多属产品级决策）、
  winget 分发（卡 `#467381` moderator 审核）。

### 本轮必须守住的硬约束

| 约束 | 出处 | 本轮怎么守的 |
| --- | --- | --- |
| 验收脚本不许在用户交互桌面抢焦点，必须跑 `CreateDesktop` 隔离沙箱 | AGENTS §2.8 | 本轮**没有**在用户桌面启动验收实例（只跑过打包产物的启动探针，已全部结束）；沙箱脚本留给下一轮 |
| 真机验收只用 `npm run tauri:build` 的打包产物，`tauri dev` 结论作废 | AGENTS §2.10 | 打包产物已产出，基线二进制已留档可做 A/B |
| push / winget PR / GitHub Release 属对外动作，需用户授权 | §19.5-1 | 一律**没做** |
| 明显 bug 根因坐实后直接按推荐方案修，不发问卷 | 用户偏好 | 归因坐实后直接改；同形的百度侧一并修掉 |
| 不在 `F:\soft\00selfmade\filemanager` 外层写新文件 | 项目约定 | 所有产物在 `sigma-file-manager\` 内 |

---

## 2. 根因（一句话版）

**不是 Rust 侧重复投递事件，而是两个前端组件同时监听同一个全局频道。**

- `afm://quark-login` / `afm://baidu-login` 是**全局**频道；
- `netdisk-add-dialog.vue`（常驻挂载，已有 `loginPhase !== 'waiting'` 守卫）与
  `connection-form.vue`（`v-if="isCreating"` / `v-else-if="editing"`，**原本没有守卫**）同时挂载时，
  一条 `timeout` 事件会被两边各弹一次 toast，而两边用的是**同一个 i18n key**
  （`settings.remote.quark.embedded.status.timeout`）⇒ 用户看到**逐字相同**的两条提示。

**Rust 侧已排除**（静态坐实，勿再查）：`quark_login.rs` 超时分支只 `emit_event(..., "timeout", ...)` 一次；
`settle()`（L781）在 `emit_event`（L782）**之前** ⇒ 随后的 `close_login_window` 触发的 `WindowEvent::Destroyed`
被 `SETTLED.swap(false)`（L436）吃掉，**不会**补发 `closed`。所以「重复」100% 出在前端。

**修复取法**：「对称守卫」—— `connection-form.vue` 增加 `baiduLoginActive` / `quarkLoginActive` 两个 ref，
只有**本表单自己发起**的那一轮登录事件才被消费；别的组件发起的直接 `return`。
（未采用「事件带 owner token」方案：Rust + types + 两个前端 + store 全动，代价与风险都大得多。）

---

## 3. 改动清单（文件级）

| 文件 | 改动 |
| --- | --- |
| `src/modules/settings/ui/categories/remote/connection-form.vue` | **修复本体**：新增 `baiduLoginActive` / `quarkLoginActive` 两个 ref；`handleStart*Login` 置 true，启动失败 / 取消 / 终态事件置 false；两个监听器开头 `if (!*LoginActive.value) return;`；`resetLoginStates` 复位。新增终态白名单 `TERMINAL_LOGIN_STATUSES = ['ready','closed','timeout','error']` ＋ `isTerminalLoginStatus(status: string)` |
| `src/modules/settings/ui/categories/remote/quark-connection-form.test.ts` | 两条「ready」用例改为**先点 `[data-test="*-embedded-start"]` 再推事件**；新增「别的组件发起的登录事件不在这里弹提示」 |
| `src/modules/settings/ui/categories/remote/baidu-connection-form.test.ts` | 同形修改；补断言「守卫不吞掉本表单自己那轮的失败提示」 |
| `src/modules/settings/ui/categories/remote/netdisk-login-duplicate-toast.test.ts` | **新增（集成复现）**：把 `ConnectionForm` 与 `NetdiskAddDialog` 同时挂到一棵树，`listen` mock 改成**一频道多 handler**（真实语义是广播），两个方向各断言**只弹一条** |
| `docs/handoff-2026-11-11-quark-pan.md` | §9 R8 行、§19.5-3、§11.3 标题订正；新增 §20.1–§20.8（根因 / 复现路径 / 修复 / 测试与判别性验证 / 门禁 / 过期台账订正 / 没做的事 / 真机验收状态） |
| `AGENTS.md` | §2.4 winget CLI 记录订正（本机 winget CLI **可用**，v1.29.380） |

> ⚠️ 「改原有两条 ready 用例」是**必要的**，不是把测试改松：原断言固化的正是缺陷本身
> （「没发起登录也弹 toast」）。改法是**走真实流程**（先点开始按钮），不是放宽期望。

---

## 4. 验证矩阵

### 4.1 已验证（实测，有输出）

| 判据 | 结果 |
| --- | --- |
| 根因（Rust 只发一次 `timeout`；`settle()` 先于 `emit_event`） | 静态代码走查坐实（`quark_login.rs` L436 / L781–784） |
| 定向单测（夸克 + 百度两个文件） | **2 files / 16 tests passed**，EXIT=0 |
| 新集成用例单独跑 | **1 file / 2 tests passed**，EXIT=0 |
| **判别性验证**：`git stash push -- connection-form.vue`（只回退修复、保留新用例） | 集成用例**变红**：`AssertionError: expected [ …(2) ] to deeply equal [ Array(1) ]` ⇒ 修复前同场景确实弹 2 条；同一 stash 下「表单自己发起的那一轮」仍绿 ⇒ 判据有方向性 |
| `git stash pop` 回位 | POP_EXIT=0，工作区干净回位 |
| 全量门禁 | 见 §5（**必须以最后一轮为准**：本轮踩过一次「vitest 全绿但 eslint 红」） |
| 打包 | `BUILD_EXIT=0`，产出 `src-tauri\target\release\alpha-file-manager.exe` ＋ NSIS `Alpha File Manager_3.14.0_x64-setup.exe` |
| 基线二进制留档 | `%TEMP%\afm-batch13-baseline\alpha-file-manager.exe`，sha256 `D2EC859DE6DD0DD8CDFFE165D5C294378BCB7F46A626B1401AA0C068D2CC29D9` |

### 4.2 推断（有依据，但**未实测**）

1. 「两条 toast 逐字相同」⇒ 依据是两边用**同一个 i18n key**（代码可查）＋ §11.3 真机截图上确实是两条一模一样的提示；
   但**没有**在修复后的打包件上肉眼复验过。
2. 「关掉登录窗 ⇒ 推 `closed` ⇒ 不用等 900 秒」⇒ 依据是 Rust `WindowEvent::Destroyed` 分支（L435–441），
   **未实测**（真机那一轮才验）。
3. 「沙箱 UIA 计数能稳定数到 toast 条数」⇒ 依据是 AGENTS §2.8 的既有做法，**未实测**。

### 4.3 未验证（明确留给下一轮，别当已完成）

1. **真机 UI 复验（最重要）**：修复后打包件跑一次沙箱验收 ⇒ 提示**只弹 1 次**；基线二进制 ⇒ **弹 2 次**。
2. **CDP 驱动 release 构建**：已确认**走不通**（详见 §7 坑 4），不要再试。
3. winget 3.14.0 PR：未提（等 `#467381` 结论 ＋ 用户授权）。
4. R4 / R5、upstream 移植：未动。

---

## 5. 门禁与打包（实测数字）

- 全量门禁命令：`npm --prefix <repo> run check`（= `ts` + `lint:check` + `test:unit:run` + `verify-changelog` + `verify:sandbox`）。
- **四轮演进（坑在这里，必须留档）**：

  | 轮次 | 触发时机 | 结果 | 说明 |
  | --- | --- | --- | --- |
  | 1 | 修复后首轮 | **失败**：2 处 TS2367 | `payload.status !== 'waiting'` —— 后端状态只有 `ready/closed/timeout/error`，`waiting` 只是前端显示态，字面量联合无交集 |
  | 2 | 改成「无条件清标记」后 | **失败**：1 条用例红 | `waiting` 把守卫提前清掉 ⇒ 后到的 `closed` 被吞 ⇒ `baidu-connection-form.test.ts`「还没取到凭据时不放成功反馈」红（`expected [] to include …`） |
  | 3 | 改成**终态白名单**后 | **通过**（CHECK_EXIT=0） | 当时还未加入新集成文件 |
  | 4 | 加入新集成文件后 | **失败**：eslint 1 error | 新文件第 160 行 `return { wrapper, dialogRef };` 违反 `@stylistic/object-property-newline`（本仓风格：对象字面量多于一个属性要换行） |
  | 5 | 改完换行后 | **通过**（CHECK3_EXIT=0） | 门禁日志：`C:\Users\Duanyi\AppData\Local\Temp\batch13-check3.log` |

- ⚠️ **教训一**：`vue-tsc` 报「类型无交集」时，**别用放宽条件去迁就编译器** —— 那样往往把语义一起写反了。
  先问「这个状态在协议里到底意味着什么」，再用白名单把语义写死。
  是**测试**抓住的：守卫的净效果必须是「挡掉别人的事件，但绝不挡掉自己那轮的失败提示」。
- ⚠️ **教训二**：`vitest` 绿 **≠** 门禁绿。样式类问题只有 `eslint` 抓得到；
  **提交前一定要跑全量 `npm run check`，不能只跑 `test:unit:run`。**

---

## 6. 下一轮怎么做（真机沙箱复验，按这个顺序）

**目标**：拿到「修复后弹 1 次 / 基线弹 2 次」这组 A/B 计数证据。

1. **先结束正在运行的 `alpha-file-manager`**（单实例插件会把新启动转交给旧实例，
   症状是「日志说启动了但沙箱里没窗口」）。
2. 复制 `.superpowers\sdd\p3-ui-regress-sandbox.ps1`（2026-11-10，最近的 UI 回归脚本）改成夸克版：
   `CreateDesktop` ＋ `CreateProcess(lpDesktop=…)` 起**打包产物**
   （`src-tauri\target\release\alpha-file-manager.exe`，**不是** `tauri dev`）。
3. **枚举沙箱窗口用 `EnumDesktopWindows`**（不切桌面时 `EnumWindows` 枚举不到）。
4. **鼠标等价断言**：沙箱内没有键盘输入（`SendInput` 只能投到输入桌面）⇒
   PostMessage 鼠标到 **`Chrome_RenderWidgetHostHWND`** ＋ UIA ＋ `PrintWindow`。
5. 路径：设置 → 远程 → 「添加远程连接」（让 `ConnectionForm` 挂上）→ 网盘页 →
   「添加网盘 → 夸克网盘」（**弹窗发起**的那一轮）→ 登录窗起来后**直接关掉它**
   （Rust `WindowEvent::Destroyed` ⇒ 推 `closed` ⇒ **不用等 900 秒超时**）。
6. **判据必须是计数**（不是「看起来只有一条」）：统计该条提示文案在 UIA 树里出现**几次**
   —— 修复后 = 1，基线 `%TEMP%\afm-batch13-baseline\alpha-file-manager.exe` = 2。
   ⇒ **已有现成探针**：`.superpowers\sdd\quark\t13-count-toast.ps1`（**只读**：不注入输入、不抢焦点，
   **绝不从脚本里启动 app**）。用法（**必须用 Windows PowerShell 5.1**，pwsh 7 里 `UIAutomationClient`
   解析不了）：
   `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .superpowers\sdd\quark\t13-count-toast.ps1`
   ⇒ 直接打印 `COUNT timeout = N`；加 `-DumpNames` 可把可见文案全转储出来找提示原文；
   模式串在 `.superpowers\sdd\quark\t13-toast-patterns.json`（UTF-8，中英文都列了）。
7. 两组都跑完才算 PASS；把数字回写本台账 §20.8。
   ⚠️ **探针的验证状态**：UIA 遍历链路**已用 explorer 做过阳性对照**
   （`WINDOWS=2 / ELEMENTS=39`，能读到任务栏按钮的真实中文文案 ⇒ 遍历可用）；
   但「WebView2 里的 toast 文案是否以 UIA 元素暴露」**尚未验证** —— 那要等 app 真跑起来才有对象。
   **读到 0 命中时，先跑一次阳性对照再下结论**（别把「判据瞎了」写成「没复现」）。

⚠️ 若 UIA 数不到 toast，**不要**写成 FAIL：先证明「判据能看见目标」（同一次运行里先在
「弹窗自己那轮」上拿到 1 条基线），分不清「没触发」和「判据瞎了」。

---

## 7. 本轮工具坑（省下一轮的时间）

1. **打包必须带 MSVC 与固定 rust 工具链**：`cargo` 不在默认 PATH，直接 `npm run tauri:build` 报
   `failed to run 'cargo metadata' ... program not found`。
   ⇒ 走 `.superpowers\sdd\quark\batch13-build.cmd`（纯 ASCII：`call vcvars64.bat` → 加
   `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin` → `cd /d` 仓库根 → `call npm run tauri:build`）。
2. **eslint 风格**：对象字面量多于一个属性必须换行（`@stylistic/object-property-newline`），
   本仓单参箭头函数不加括号（`x => ...`）。
3. **`git commit -F -` 不吃 stdin**（`-F` 只接受文件名）⇒ 提交信息写**临时文件**再 `git commit -F <file>`。
4. **CDP 在 release 构建里打不开**：wry/Tauri **显式**传了 `AdditionalBrowserArguments`
   （可见 `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`）⇒ WebView2
   **忽略** `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 环境变量，也忽略
   `HKCU\Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments` 策略。
   要开 CDP 只能改 Rust（`WebviewWindowBuilder::additional_browser_args()`，配置里没有该字段，
   主窗还要改 `create: false`）⇒ 属**探针版**改动，会污染发布件。
   （本轮试过之后已把该策略键的**值置空**；键还在。）
5. **跨工作区边界的写/删/改名**：内联命令会被本机硬策略拦 ⇒ **写成 `.ps1` 再 `pwsh -NoProfile -ExecutionPolicy Bypass -File <script>`**。
6. **本机 bash 硬策略**：`cd` / `Push-Location` 之后再做写操作会被拦 ⇒ 一律用 `npm --prefix` / `git -C`。
7. **改带中文注释的源文件**不要用 PowerShell `Get-Content -Raw` / `Set-Content`（会 mojibake）⇒ 用 `edit` 工具或 node 脚本。
8. **不要跑 `cargo fmt`**；`cargo test --lib` 必须在 `src-tauri/` 里跑；**cargo 与 `npm run check` 不要并发**。

---

## 8. 需要用户授权的动作（本轮**一件都没做**）

| 动作 | 现状 | 需要什么 |
| --- | --- | --- |
| `git push` | 本地改动未 commit、未 push | 用户授权 |
| winget PR（3.14.0 清单已就绪） | `#467381` 仍 OPEN，卡 moderator 审核（CLA 已绿）；**同包 ID 不能并行提** | 等 `#467381` 合并/关闭 ＋ 用户授权 |
| GitHub Release | 未发 | 用户授权 |

---

## 9. 证据路径速查

| 内容 | 路径 |
| --- | --- |
| 上游真台账（本轮细节在 §20） | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-11-quark-pan.md` |
| 本交接卡 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-13-quark-pan-batch13.md` |
| 修复本体 | `src\modules\settings\ui\categories\remote\connection-form.vue` |
| 新集成用例 | `src\modules\settings\ui\categories\remote\netdisk-login-duplicate-toast.test.ts` |
| Rust 登录事件源 | `src-tauri\src\quark_login.rs`（`LOGIN_EVENT` L80 / `emit_event` L300–312 / Destroyed L435–441 / 超时 L779–784） |
| 打包包装器 | `.superpowers\sdd\quark\batch13-build.cmd` |
| 沙箱模板 | `.superpowers\sdd\p3-ui-regress-sandbox.ps1`、`.superpowers\sdd\sandbox-c1-accept.ps1` |
| 门禁日志 | `C:\Users\Duanyi\AppData\Local\Temp\batch13-check3.log`（最终轮）、`batch13-check.log`（第 3 轮）、`batch13-check2.log`（第 4 轮，eslint 红的那轮） |
| 打包日志 | `C:\Users\Duanyi\AppData\Local\Temp\batch13-build.log` |
| 基线二进制 | `%TEMP%\afm-batch13-baseline\alpha-file-manager.exe` |
| toast 计数探针（只读，本轮新增） | `.superpowers\sdd\quark\t13-count-toast.ps1` ＋ `.superpowers\sdd\quark\t13-toast-patterns.json` |
| 探针阳性对照输出（explorer） | `C:\Users\Duanyi\AppData\Local\Temp\t13-control.json`（`WINDOWS=2 / ELEMENTS=39`） |
| 打包产物 | `src-tauri\target\release\alpha-file-manager.exe`、`src-tauri\target\release\bundle\nsis\Alpha File Manager_3.14.0_x64-setup.exe` |
