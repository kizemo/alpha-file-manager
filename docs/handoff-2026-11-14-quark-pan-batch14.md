# 批次 14 交接卡：缺陷 B 真机 UI A/B 复验 ＋ 顺带修掉「toast 空壳」独立缺陷（2026-11-14）

> 上游真台账：`docs/handoff-2026-11-11-quark-pan.md`（本轮的技术细节记在它的 **§20.9**）。
> 本文件是**给下一轮会话的入口卡**：背景、目标、进度、下一步、以及「哪些是实测的、哪些是推断的、哪些还没验」。
> 品牌名固定 **Alpha File Manager**；`afm://quark-login` / `afm://baidu-login` 频道名、legacy 数据目录名、Tauri `identifier` 是受保护常量。

---

## 0. 一句话状态

**批次 13 唯一的遗留项（缺陷 B 的真机 UI 计数复验）已做完：基线弹 2 次 / 修复后弹 1 次，A/B PASS。**
过程中发现并修掉了第二个**独立**缺陷（toast 渲染成空壳、标题与副标题全不显示），也已重新打包并真机复验通过。
本轮改动**已 commit、未 push**；未提 winget PR、未发 GitHub Release、未做 upstream 移植。

---

## 1. 背景与目标

- **上游卡**：`docs/handoff-2026-11-11-quark-pan.md` §20.8（真机验收做到哪一步、卡在哪、下一步怎么走）。
- **本轮选定目标**：只做**缺陷 B 的真机 UI A/B 复验** —— 在 `CreateDesktop` 隔离沙箱里，用**打包产物**复现
  「修复后弹 1 次 / 基线弹 2 次」的**计数证据**，并回写 §20.8。
- **未纳入本轮**：upstream 移植、R4（17 语言包母语复核）、R5（`Ctrl+Shift+P` 换机器验证）、winget 分发。

### 本轮必须守住的硬约束

| 约束 | 出处 | 本轮怎么守的 |
| --- | --- | --- |
| 验收脚本不许在用户交互桌面抢焦点，必须跑 `CreateDesktop` 隔离沙箱 | AGENTS §2.8 | 全部三轮（基线 / 修复后 ×2）都在独立沙箱桌面里跑；脚本自身不切桌面、不 `SetForegroundWindow`、不发 `SendInput` |
| 真机验收只用 `npm run tauri:build` 的打包产物，`tauri dev` 结论作废 | AGENTS §2.10 | 三轮都用 `target\release` 或留档基线 exe；`tauri dev` 全程没用过 |
| push / winget PR / GitHub Release 属对外动作，需用户授权 | §19.5-1 | 一律**没做** |
| 明显 bug 根因坐实后直接按推荐方案修，不发问卷 | 用户偏好 | 空 toast 缺陷根因坐实后直接修（改包装器而非改 7 处调用点）＋补单测 |
| 恢复真实数据顺序：先杀进程 → 再 restore | AGENTS / 上轮教训 | 已按此执行，restore 后核对字节数 6326 |

---

## 2. 结论一：缺陷 B 真机 A/B 复验 **PASS**

| 轮次 | 二进制 | burst 帧 0 比基线帧多出的 UIA 节点 | 判定 |
| --- | --- | --- | --- |
| 基线 `t14base2` | `%TEMP%\afm-batch13-baseline\alpha-file-manager.exe`（2026-11-12 01:01，sha256 `D2EC859D…29D9`） | **2 个 `ListItem`**：`700,742,534x50` ＋ `700,804,534x50` | 基线弹 **2** 次 |
| 修复后 `t14fix2` | 批次 13 的 `target\release\alpha-file-manager.exe` | **1 个 `ListItem`**：`700,804,534x50` | 修复后弹 **1** 次 |
| 修复后 `t14fix3` | 本轮重打包（另含空 toast 修复） | **1 个 `ListItem`**：`700,737,534x117`，`name=[登录还没完成，登录窗口就被关掉了。登录窗口已关闭。]` | 修复后弹 **1** 次，**文字可见** |

- **计数口径 = UIA `ListItem` 实例数**（`Notifications alt+T` → `List` → `ListItem`），像素差分作交叉验证。
  ⚠️ **不能用 toast 文案计数**：修复前两轮的卡片是空壳（见 §3），文案口径恒为 0，会得出假结论。
- 弹前基准帧（step-10 / step-12）与 burst 末帧（frame 39）两轮完全一致 ⇒ 起点无残留。

## 3. 结论二：顺带修掉「toast 空壳」独立缺陷

- **现象**：toast 卡片 534×50 出现在右下角，但**卡内无任何文字**。
  像素硬证据：卡片区 103 种颜色，**最暗通道仅 176**，卡片底色 (243,238,229)，无任何 <150 的深色像素。
- **根因链**：`app-toast.ts` 包装器签名 `toast(message: string, data?)` 把**第一个实参当字符串 title**；
  但全仓 7 处 `toast(` 调用**全是对象形态** `{ title, description }`
  （`connection-form.vue:254,263,318,325`、`netdisk-add-dialog.vue:133,162,171`）
  ⇒ `componentProps.data.title` 变成**对象** ⇒ `toast-header.vue` 的 `props.title.indexOf('|')` 对对象调用字符串方法
  ⇒ **抛错** ⇒ 整棵 `ToastHeader` 渲染成注释节点 ⇒ 卡片只剩背景/边框。
- **修法**：包装器 `normalizeInput()` 同时兼容两种入参形态（不改动 7 处调用点）；
  `toast-header.vue` 再加一层防御（title 非字符串时不再抛错）。
- **回归锁**：新建 `src/components/ui/toaster/app-toast.test.ts`，挂**真的** `Toaster` ＋ **真的** `toast()`（不走 mock）。
  ⚠️ 旧的 `netdisk-login-duplicate-toast.test.ts` 把整个 toaster 模块 mock 掉 ⇒ 从来没发现过这个问题。
- **与 A/B 的关系**：git 溯源显示 `app-toast.ts` 最后一次改动是 `db684e30`（2026-04-05），
  对象形态调用由 `41120bc1`（2026-11-11）引入 ⇒ **基线与修复后二进制的 toaster 代码完全一致**，A/B 计数不被污染。

---

## 4. 改动清单（文件级）

| 文件 | 改动 |
| --- | --- |
| `src/components/ui/toaster/app-toast.ts` | 新增 `ToastInput` 类型与 `normalizeInput()`；`showStaticToast` 接受两种入参形态，统一产出 `{ title: string, description: string }`；对象形态里其余键（duration 等）透传给 sonner；末尾类型断言放宽为 `as unknown as typeof sonnerToast` |
| `src/components/ui/toaster/toast-header.vue` | `formattedTitle` 里 `typeof props.title === 'string' ? props.title : ''`（防御：非字符串标题不再整棵树崩掉） |
| `src/components/ui/toaster/app-toast.test.ts` | **新建**：真 `Toaster` ＋ 真 `toast()`，断言两种形态都能渲染出标题文字 |
| `docs/handoff-2026-11-11-quark-pan.md` | 新增 **§20.9**（方法 / A/B 数据 / 与 §11.3 的差异登记 / 空 toast 根因与修法 / 工具坑 / 收尾状态） |
| `docs/handoff-2026-11-14-quark-pan-batch14.md` | 本文件 |

> 本轮**没有改动** `connection-form.vue` / `netdisk-add-dialog.vue` / 任何 Rust 源码。

---

## 5. 验证矩阵

### 5.1 已验证（实测，有输出）

| 判据 | 结果 |
| --- | --- |
| 基线轮 13 步沙箱链路 | 全通（`t14base2-run.log`：tab-remote → btn-add-remote → tab-netdisk → btn-quark → wait-login → dump-windows → close-login） |
| 基线弹 2 次 | burst 帧 0 多出 **2 个 `ListItem`**（140 节点 vs 基线 138） |
| 修复后弹 1 次 | burst 帧 0 多出 **1 个 `ListItem`**（139 节点 vs 基线 138） |
| 修复后（v3）仍弹 1 次 | `COUNT_closed=1`，40 帧里**只有帧 0–3 命中 1 次**，其余 36 帧为 0 |
| toast 文字真的渲染出来了 | UIA：`ListItem name=[登录还没完成，登录窗口就被关掉了。登录窗口已关闭。]` ＋ `Text name=[登录窗口已关闭。]`；像素：`t14dark` 帧 1–3 `dark=775`，旧产物全程 `dark=0` |
| 空 toast 根因（测试层） | 修复前 `app-toast.test.ts` 抛 `TypeError: titleText.indexOf is not a function`（`toast-header.vue:24`）；修复后 **2/2 通过** |
| 重新打包 | `cmd /c .superpowers\sdd\quark\batch13-build.cmd` ⇒ `BUILD_EXIT=0`（cargo `Finished release profile in 3m 47s`），新 exe 2026-11-14 01:38:17，NSIS 43.35 MiB |
| 全量门禁 | 见 §6（以最后一轮为准） |
| 真实设置已恢复 | `t14-seed.cjs restore` ⇒ `restored bytes=6326`（= 备份字节数），恢复前已结束 `alpha-file-manager` |

### 5.2 推断（有依据，但**未实测**）

1. 「§11.3 截图里 toast 有可见文字，而本轮修复前实测是空卡片」⇒ 依据是代码演进时间线
   （对象形态调用 2026-11-11 才引入，§11.3 截图拍于当天更早）；
   **没有**回头重拍 §11.3 当时的构建来逐帧比对。
2. 「`toast.success/error/info` 等方法仍只接受字符串标题」⇒ 依据是代码可查；
   当前全仓无调用点，**没有**为它们补对象形态的用例。
3. 「对象形态里除 `title/description` 外的键会透传给 sonner」⇒ 代码如此实现，
   **未实测**（没有真实调用点会带 `duration` 之类的键）。

### 5.3 未验证（明确没做）

1. **装机复验**（NSIS 安装 → 真实用户目录 → 人工点一次「添加网盘 → 夸克网盘」）：
   本轮全部在**沙箱 + 灌夹具设置**下完成，**没有**走真实安装流程。
2. **扫码真实登录**（`ready` 终态）：本轮一律用 `WM_CLOSE` 关登录窗触发 `closed`，
   `ready` / `timeout` 两条终态**没有**在真机上跑过（`timeout` 要等 900 秒）。
3. **百度侧同形缺陷**的沙箱真机复验：本轮只复验了夸克侧。
4. `git push`、winget PR、GitHub Release、upstream 移植（均属对外/大工程动作）。

---

## 6. 本轮工具坑（省得下一轮再踩）

1. **`Add-Type` 的 C# 段：pwsh 7 编译不过，Windows PowerShell 5.1 才过。**
   pwsh 7 的 `Add-Type -ReferencedAssemblies 'System.Drawing'` 是**替换**默认引用集 ⇒
   `System.Drawing.Color` 找不到（CS1069，需要 `System.Drawing.Primitives`）；5.1 是追加 ⇒ 能编译。
   ⇒ **UIA / C# 探针脚本一律用 `powershell.exe` 跑**。
2. **脚本被强杀时看不到任何输出**：`Say` 用 `Write-Host` ＋ 自写 `run.log`，
   死在 `Add-Type` 之前 ⇒ `run.log` 根本不会被创建 ⇒ 现象是「零输出 + 退出码 -1」。
   解法：套一层 **wrapper**（`try { & script.ps1 } catch { 写错误到标记文件 }`），一次拿到真异常。
   本轮已落盘 `.superpowers\sdd\quark\t14-wrap.ps1` / `t14-wrap3.ps1`（纯 ASCII）。
3. **打包必须走 `.superpowers\sdd\quark\batch13-build.cmd`**：
   本机 `cargo` **不在默认 PATH**（`~/.cargo\bin` 根本不存在），实际在
   `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`，且需要 vcvars64 的 MSVC 环境
   ⇒ 直接 `npm run tauri:build` 必报 `failed to run 'cargo metadata' ... program not found`。
4. **恢复真实用户设置**：release 构建下 `SFM_E2E_DATA_DIR` 无效，只能
   「备份真实文件 → 灌夹具 → 验收 → 恢复」；备份在 `.superpowers\sdd\quark\t14-backup\user-settings.json`。
5. **沙箱轮必须串行**：单实例插件会把新启动转交给旧实例；
   开跑前先结束 `alpha-file-manager` 并清掉 `msedgewebview2.exe`
   （命令行含 `com.sigma-file-manager.app`）的孤儿，否则下次启动 exit 101。
6. **窗口尺寸固定 1280×900**：webview 渲染面在 y≈1018 以下是黑区；
   1800 高时按钮落到窗口外、1400 高时「添加远程连接」落在渲染区外。
7. `cargo` 与 `npm run check` 不并发（项目禁令）。

---

## 7. 证据路径（全部在仓库内）

| 内容 | 路径 |
| --- | --- |
| 真台账（§20.9 本轮详情） | `docs/handoff-2026-11-11-quark-pan.md` |
| 沙箱探针脚本（13 步链路，纯 ASCII） | `.superpowers\sdd\quark\t14-quark-toast-sandbox.ps1` |
| 探针 wrapper（捕获真异常） | `.superpowers\sdd\quark\t14-wrap.ps1` / `t14-wrap3.ps1` |
| 计划 / 标签 / 文案模式 | `.superpowers\sdd\quark\t14-plan2.json`（`burst=40, interval=150`）、`t14-labels.json`、`t14-toast-patterns.json` |
| UIA 逐帧差分脚本 | `.superpowers\sdd\quark\t14-uia-diff.cjs` |
| 像素辅助脚本 | `.superpowers\sdd\quark\t14-dark.ps1`（暗像素统计，已验证可用） |
| A/B 差分输出 | `.superpowers\sdd\quark\t14-uia-ab.txt` |
| 三轮日志 / 逐帧 UIA 树 / 截图 | `.superpowers\sdd\quark\t14-shots\t14base2-*`、`t14fix2-*`、`t14fix3-*` |
| 设置夹具与备份 | `.superpowers\sdd\quark\t14-seed.cjs`、`.superpowers\sdd\quark\t14-backup\user-settings.json` |

---

## 8. 下一步（按优先级）

1. **装机复验**（若用户要用真实安装流程验收）：`release\local-builds` / NSIS 安装包 →
   真实用户目录 → 人工点一次「添加网盘 → 夸克网盘」→ 关掉登录窗 → 看 toast 是不是**一条带字**的提示。
2. **`ready` / `timeout` 两条终态**的沙箱真机验证：`ready` 需要真实扫码登录态；
   `timeout` 要等 900 秒（`GIVE_UP`），可以用 Rust 侧把超时临时调小来做探针版验证（**会污染发布件，不建议**）。
3. **百度侧**同形缺陷的沙箱真机复验（改 `i18nPrefix` 即可复用本轮脚本）。
4. 对外动作（`git push` / winget PR / GitHub Release）**等用户明确授权**再动。
