# 交接：阶段 1 收官 + 两个线上缺陷（默认文件管理器未生效 / 扩展市场源与选项卡）

> 面向下一会话：本文是**唯一的接力入口**。先读本文，再按 §九「下阶段计划」执行。
> 配套的一句话 prompt 见 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\prompt-2026-11-04-phase1-bugfix-default-filemanager-and-extension-sources-next.md`。

- 日期：2026-11-04（晚间追加）
- 仓库根：`F:\soft\00selfmade\filemanager\sigma-file-manager`
- 仓库约定：`F:\soft\00selfmade\filemanager\AGENTS.md`（**动手前必读，697 行**）
- 上一份交接（本仓库主链路）：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-market-sources.md`（已回写「九、阶段 1 完成」）
- 设计文档：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\design-2026-11-04-extension-market-sources.md`

---

## 一、一句话现状

阶段 1（宿主侧「市场源」接线）**已交付并通过真机验收（B29 40/40）**；用户随后报了两个缺陷：①在 **MiniMax Code** 里点「打开文件所在目录」仍然拉起系统资源管理器而不是 AFM；②AFM 的扩展页里**只显示 sigma 源、没有 alpha 源**，且**「市场 / 已安装」两个选项卡的页面都消失**。

缺陷 ① 的机制层原因**已坐实**：本机注册表里**不存在任何 AFM 的目录处理程序注册**。
缺陷 ② 的前半段（**缺 alpha 源**）**已坐实是产品缺口**；后半段（选项卡页面消失）**尚未复现**，需下一会话在真机上定位。

---

## 二、项目背景（读本文需要的最小充分背景）

- 本仓库是 **Alpha File Manager（AFM）**，上游为 `sigma-file-manager`（品牌名 **Alpha File Manager 已锁定**，不要改）。
- GitHub 主仓库：`kizemo/alpha-file-manager`；上游市场清单：`sigma-hub/sfm-marketplace`。
- 扩展体系：应用内「扩展市场」直接安装扩展。扩展包走 GitHub Releases（`archive/refs/tags/v{version}.zip`），**清单**走 `registry.json`。
- 本轮刚交付的「阶段 1 = 应用内多市场源（L2）」：官方源（内置、指向 sigma-hub）保留 + 用户自建源可增删，**Rust 侧零改动**。
- fork 自建的示例市场：`kizemo/sfm-marketplace`（public，含 `registry.json`）——**这正是用户口中「alpha 源」最可能的落点**（见 §五.2）。

---

## 三、阶段 1 已交付内容（不需要重做）

代码（14 源文件 + 17 语言包）：类型 `src/types/extension.ts`；工具 `src/modules/extensions/utils/market-sources.ts`；存储 `src/stores/storage/extensions.ts`；runtime `src/stores/runtime/extensions.ts`；组合式 `src/modules/extensions/composables/use-extensions.ts`；UI `src/modules/settings/ui/categories/extensions/market-sources.vue`（新）、`src/modules/extensions/components/extensions-marketplace.vue`（重写）、`extension-card.vue`、`extension-badge.vue`、`src/modules/extensions/pages/extensions.vue`、`src/modules/settings/ui/categories/extensions/index.vue`；i18n 17 包各 +14 键。

门禁（全通过）：`npm run ts` exit 0 · `npx eslint .` **186 errors / 0 warnings**（基线 188，`git stash push -u` 现量）· stylelint 0 · vitest 扩展相关 **37/37** · `cargo test --lib` **912 passed**。

交付安装包：`F:\soft\00selfmade\filemanager\sigma-file-manager\release\Alpha File Manager_3.9.2_x64-setup.exe`（44 946 582 B，SHA256 `45856f92ba76c4ff03f853733151b96236bd584f331c7ba66abde8cbb2d48c16`），内嵌 exe 已验证**不含调试端口**。

验收脚本与报告：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b29-market-sources-accept.cjs`、`.superpowers\sdd\accept-b29\report.txt`、`.superpowers\sdd\accept-b29-findings.md`。

---

## 四、用户报告原文（逐字保留，别改写诉求）

> 1. 在MiniMax code中，点击打开文件所在目录，启动的仍然时系统资源管理器，而不是AFM，说明之前修改默认没有完全成功；
> 2. 拓展中，不仅在设置的拓展功能页面，源只显示sigma的源，alpha的源没有显示，并且查看拓展、已安装拓展的选项卡页面都消失啦。

---

## 五、本轮已核实的事实（带取证方式，可复核）

### 5.1 机器与进程现状（取证时间 2026-11-04 21:30–21:35）

| 事实 | 取证结果 |
| --- | --- |
| 正在运行的 AFM | 进程 `alpha-file-manager`，可执行文件 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` |
| 运行的是**本轮新构建** | 该 exe sha256 = `065B2185853731F71B412855D6DD94FDF172AA8DC7A8EA60A6D837879303BB35`，与交付安装包内嵌 exe **逐字一致**；mtime 2026-11-04 20:41:54 |
| 安装目录内容 | `alpha-file-manager.exe` / `uninstall.exe`(mtime 21:28:14) / `extensions/`(10-29) / `tools/`(21:28:24) —— **没有 `default-file-manager-launcher.exe`**（需确认它是否在 `tools\` 下） |
| 干扰项 | `C:\Program Files\Alpha File Manager` **存在但是空目录**（旧安装残留）——排查时别把它当安装位置 |
| 用户数据 | `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json`，mtime 2026-11-04 21:33:36 |
| 存储结构 | 顶层键含 `registryCaches`、`marketSources`、**旧单数 `registryCache`** ⇒ 新旧结构并存，迁移生效 |
| 自建源数量 | **`marketSources: []`（一个都没有）**；`registryCaches` 只有 `official` 一个键，内含 **6 条**，`fetchedAt=1793799060730`（= 21:31 本地）⇒ **官方源刚刚拉取成功，网络没问题** |
| 已安装扩展 | 3 个：`kizemo.focus-sync` 0.5.5、`kizemo.git` 0.1.0、`sigma.clipboard` 1.0.1，**都有 manifest**，**都没有 `sourceId`**（本地安装态，正常） |
| 设置页停留位置 | `user-settings.json` 的 `settingsCurrentTab="extensions"` ⇒ 用户当时停在**设置 → 拓展** |

### 5.2 「alpha 源没有显示」——**已坐实是产品缺口**（不是渲染 bug）

代码事实：`src/stores/runtime/extensions.ts:145-157`

```
officialMarketSource = { id:'official', name: i18n.global.t('extensions.marketSourceOfficial'),
                         url: externalLinks.extensionsRegistryUrl, kind:'official', enabled:true, builtIn:true }
allMarketSources     = [ officialMarketSource, ...marketSources ]   // marketSources = 用户自建源
```

- `src/data/external-links.ts:29-35`：`extensionsRegistryUrl = https://raw.githubusercontent.com/sigma-hub/sfm-marketplace/main/registry.json` —— **全仓只有这一个内置市场地址**。
- i18n `extensions.marketSourceOfficial`（中文）= **「官方」**；`extensions.tabs` = `{marketplace:"市场", installed:"已安装", dependencies:"依赖项", overview:"概览", permissions:"权限", changelog:"更新日志", commands:"命令"}`。
- 用户真实数据 `marketSources: []` ⇒ 界面上**只可能出现 sigma 一个源**。

**结论**：阶段 1 的实现里**只有 sigma 一个内置源**；fork 自己的 `kizemo/sfm-marketplace`（即用户说的「alpha 源」）**没有被做成内置源**，只能由用户手动添加 ⇒ 用户看到「只有 sigma 源」是**当前实现的真实行为，不是渲染故障**。

**建议修复方向（待拍板后实施）**：把 fork 自建市场提升为**第二个内置源**（`builtIn: true`、不可删不可改 URL），URL = `https://raw.githubusercontent.com/kizemo/sfm-marketplace/main/registry.json`，并把 `extensions.marketSourceOfficial`（「官方」）拆成两个具名键（如 `marketSourceOfficialSigma` / `marketSourceOfficialAlpha`，17 个语言包同步）。注意随之而来的**合并优先级**与**卸载判定**要保持 Phase 1 的口径：官方源优先、判定集 = 合并后 id 全集 ∪ 已停用源缓存条目、任一启用源彻底不可达则跳过判定。

### 5.3 「打开文件所在目录仍走系统资源管理器」——**机制层原因已坐实**

注册表现状（只读查询，2026-11-04 21:31）：

| 查询 | 结果 |
| --- | --- |
| `HKCU:\Software\Classes\Directory\shell\open\command` | **不存在** |
| `HKCU:\Software\Classes` 下任何含 Alpha/Sigma/afm/FileManager 的键 | **一个都没有** |
| `HKLM:\SOFTWARE\Classes\Directory\shell\open\command` | **不存在**（复核一下 `HKCR\Directory\shell\open\command`，通常指向 explorer.exe；这条值得单独确认） |

⇒ **AFM 从未在这台机器上注册成目录（文件夹）处理程序**，因此任何外部程序（MiniMax Code、记事本「打开所在文件夹」等）打开目录时，Windows 仍走系统资源管理器。**「打开文件所在目录」在 MiniMax Code 里拉起 Explorer，与 AFM 内部逻辑无关 —— 是系统关联缺失。**

涉及代码落点：

| 位置 | 作用 |
| --- | --- |
| `src-tauri/src/default_file_manager.rs` | 常量 `FOLDER_OPEN_COMMAND_KEY` / `FOLDER_EXPLORE_COMMAND_KEY` / `OPEN_NEW_WINDOW_COMMAND_KEY`（写入见 373-379 行）；`is_default_file_manager`（669 行）；`set_default_file_manager`（tauri command，`src-tauri/src/lib.rs:388-390` 注册） |
| `src-tauri/src/default_file_manager/registry.rs:329-342` | `write_shell_command`：在 **HKEY_CURRENT_USER** 下建键、写默认值、写空 `DelegateExecute` |
| `src-tauri/default-file-manager-launcher/` | 独立启动器 crate：把 `explorer.exe /select,"C:\dir\f.txt"` 改写成 `--sigma-select=…` 再转发主程序 |
| `src/utils/launch-directories.ts` | L3 改造（2026-11-03，未提交）：`parseLaunchArguments` / `getLaunchSelectRequest` / `resolveSelectLaunchTarget` |
| `src/utils/default-file-manager.ts` | `isDefaultFileManagerSafe()`（FORK-MODIFICATION 2026-11-03，未提交） |
| `src/utils/open-in-explorer.ts` | AFM **自己**的「打开文件所在目录」入口 |
| `src/modules/settings/ui/categories/experimental/default-file-manager.vue`（+ 同名 `.test.ts`） | 设置 → 实验 → 默认文件管理器 的开关 UI |

**三条候选根因（必须逐条判别，不要凭猜）**：

- **R1 用户没打开开关 / 开关没生效**：设置 → 实验 → 「设为默认资源管理器」是否被打开？打开后 `HKCU\Software\Classes\Directory\shell\open\command` 是否被写入？若 UI 显示已启用但注册表为空 ⇒ 是 `set_default_file_manager` 实现或 UI 调用的 bug。
- **R2 启动器缺失**：注册表里的命令指向 `default-file-manager-launcher.exe`，若该 exe 没随安装包落到磁盘（当前安装目录里没见到，见 §5.1）⇒ 注册即便写了也会执行失败。**先确认它是否在 `D:\Program Files\Alpha File Manager\tools\` 下**。
- **R3 消费方根本不查关联**：MiniMax Code 若用 Electron 的 `shell.showItemInFolder()`，它**硬编码**拉起 `explorer.exe`，**任何**系统关联都改不动 ⇒ 缺陷 ① 的修复责任不在 AFM，而在 MiniMax Code 侧。**判别方法**：在 MiniMax Code 的安装目录（`E:\Users\Duanyi\.minimax\.minimax` 数据目录之外的程序目录）里搜 asar/JS 是否含 `showItemInFolder`；或用 ProcMon 观察它是否 `CreateProcess explorer.exe`（直接调 explorer 就说明是 R3）。若确认是 R3，AFM 侧只需保证「设置里能看到自己已是默认资源管理器」，并在文档里写清限制。

### 5.4 「市场 / 已安装选项卡页面消失」——**尚未复现**（下一会话的主战场）

**已排除**（有据）：

- 不是语言包问题：设置页的「市场源」卡片能正常渲染（`extensions.marketSources` 有值），说明新语言包已生效。
- 不是 store 崩溃：设置页能读到 `allMarketSources`（官方源显示正常）⇒ `officialMarketSource` / `allMarketSources` 计算属性正常。
- 不是网络：官方 registry 在 21:31 刚成功拉取 6 条并落进 `registryCaches.official`。
- 不是卡片组件的空指针：`src/modules/extensions/components/extension-card.vue:31-39` 的 `sourceLabel` 对无 `sourceId` 的扩展返回 `''`，且徽标是 `v-if="sourceLabel"` 才渲染 ⇒ 3 个已装扩展（都无 `sourceId`）不会因此崩。
- 不是设置页结构被我改坏：`src/modules/settings/ui/categories/extensions/index.vue` 的 diff 只有两行（新增 `<MarketSources />`）。

**尚未排除的重点**（下一会话按此顺序查）：

1. **页面级渲染异常**：用 CDP 版 exe + **真实用户数据**（不是夹具）打开 `/extensions`，抓 Vue 运行时错误。**注意**：Phase 1 的 B29 验收跑的是 **CDP 构建 + 夹具数据**，**从未验证过「全新安装 + 真实数据」**——这正是本缺陷的漏网区。CDP 版 exe：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\afm-cdp.exe`（仅测量工具，**不是交付物**）；CDP 客户端 `.superpowers\sdd\b17-cdp.cjs`。
2. **空态 ≠ 消失**：确认「选项卡页面」是**白屏**还是**正常渲染但列表为空**。市场页数据来自 `registry.extensions`（6 条已在缓存里），已安装页数据来自 `installedExtensionsWithManifest`（3 个有 manifest）。若两者都空 ⇒ 指向 `use-extensions.ts` 的过滤/映射，而不是渲染崩溃。取 `pinia._s.get('extensions')` 快照即可分辨。
3. **`useRestoredActiveTab('extensions','marketplace')`**（`src/composables/use-restored-active-tab.ts`）：用户真实 `user-settings.json` 里若存了非法 tab 值，可能让 Radix `Tabs` 没有任何 trigger 处于激活态（表现为「页面空了」）。
4. **网络告警条**：`extensions.vue` 顶部有 `hasGitHubConnectivityIssue` 的 `Collapsible` + `Alert`（`src/modules/extensions/utils/github-connectivity-status.ts`）——本机 `github.com:443` 不可达（见 §八），该告警**应当**出现；若它展开时把内容区挤没，也要一并看。

---

## 六、验证状态分级（按 AGENTS.md 口径）

**已验证**

- 用户跑的是本轮新构建（exe SHA256 与交付包内嵌 exe 逐字一致）。
- 本机**无任何** AFM 目录处理程序注册（HKCU/HKLM 查询为空）。
- 安装目录里没有 `default-file-manager-launcher.exe`（顶层未见，`tools\` 待确认）。
- 产品里只有 sigma 一个内置市场源；用户真实数据 `marketSources: []`；官方源 21:31 拉取成功（6 条）。
- 设置页市场源卡片、存储读写两侧、合并/降级/卸载判定逻辑本身在 B29 中 40/40 通过（夹具数据 + CDP 构建）。

**推断（待验）**

- 缺陷 ① 的直接原因是「AFM 未被注册为默认文件管理器」，而不是 MiniMax Code 的实现问题 —— R3 尚未排除。
- 缺陷 ② 的「选项卡页面消失」最可能是**真实数据 + 全新安装**这一组合触发的渲染/空态问题（B29 从未覆盖该组合）。
- 「alpha 源」= `kizemo/sfm-marketplace`（用户口中的「我的市场」），落点 `https://raw.githubusercontent.com/kizemo/sfm-marketplace/main/registry.json`。

**未验证**

- 缺陷 ① 的三条候选根因（R1/R2/R3）哪一条（或几条）成立。
- 「选项卡页面消失」的真实触发条件与根因。
- 用户是否开启了设置里的「默认文件管理器」开关、是否曾手动用过该开关。

---

## 七、证据路径（完整路径）

| 路径 | 内容 |
| --- | --- |
| `F:\soft\00selfmade\filemanager\AGENTS.md` | 仓库铁律，**动手前必读** |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-market-sources.md` | 阶段 1 交接（含「九、阶段 1 完成」） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\design-2026-11-04-extension-market-sources.md` | 阶段 1 设计（§5.2 合并、§5.3 卸载判定、§五 风险表） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\prompt-2026-11-04-phase1-bugfix-default-filemanager-and-extension-sources-next.md` | 本文的一句话 prompt |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\src\default_file_manager.rs` | 默认资源管理器注册/检测/恢复 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\src\default_file_manager\registry.rs` | `write_shell_command`（HKCU 写入） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\default-file-manager-launcher\` | 启动器 crate（`/select,` → `--sigma-select=`） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\utils\launch-directories.ts` | L3 启动参数解析（未提交改动） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\utils\default-file-manager.ts` | `isDefaultFileManagerSafe`（未提交改动） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\utils\open-in-explorer.ts` | AFM 自身「打开文件所在目录」 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\settings\ui\categories\experimental\default-file-manager.vue` | 设置里的默认资源管理器开关 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\stores\runtime\extensions.ts` | 市场源 runtime（145-157 官方源、734 fetchRegistry） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\extensions\utils\market-sources.ts` | 市场源工具函数与 id 派生 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\data\external-links.ts` | `extensionsRegistryUrl`（唯一内置市场地址） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\settings\ui\categories\extensions\market-sources.vue` | 设置页市场源卡片 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\extensions\components\extensions-marketplace.vue` | 市场页（来源切换条） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\extensions\pages\extensions.vue` | 扩展页（tabs 445-570 行） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-b29-findings.md` | B29 三轮归因与证据 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\b29-market-sources-accept.cjs` | B29 验收脚本 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\afm-cdp.exe` | CDP 版 exe（仅测量工具） |
| `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-extensions.json` | 真实扩展数据（排查时**只读**，改前备份） |
| `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-settings.json` | 真实设置数据 |
| `D:\Program Files\Alpha File Manager\` | 实际安装目录 |

---

## 八、本轮踩坑与工具约束（沿用 AGENTS.md，此处只列本次新增/复用的）

- **`github.com:443` 本机不可达**（3 次独立复现 15s 超时）⇒ 扩展包下载 URL `https://github.com/<owner>/<repo>/archive/refs/tags/v{version}.zip` 必失败；真实落点 `codeload.github.com/.../v0.1.0` 直连 **200 / 31361 B** ⇒ **包完好，是网络出口问题**。`raw.githubusercontent.com` 间歇不可达（21:31 那次官方源拉取是成功的）。
- **真机验收必须用 `tauri build` 产物**，`tauri dev` 构建的路由页内容区全空白 ⇒ 界面结论一律作废。
- **release 构建下 `SFM_E2E_DATA_DIR` 不生效** ⇒ 要动真实数据必须「备份 → 灌夹具 → 验收 → **先杀进程** → restore → 核对 SHA256」。
- **`.cmd` 运行期间绝对不能编辑它**（cmd.exe 按字节偏移逐行读）；`.cmd` 必须纯 ASCII；`echo` 里不能出现 `>`。
- **PowerShell 双引号包 `node -e "…"` 会把反引号当转义符** ⇒ 含中文/反引号的脚本内容用写文件工具落盘再执行。
- **不要用 `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件**；批量改用 node `fs.readFileSync/writeFileSync(..., 'utf8')`。
- **eslint 基线用 `git stash push -u` 现量**（当前全仓 186 / 基线 188；`eslint src` 口径不同，别混）。
- **vitest 要排除 `.superpowers/**` 与 `graphify-out/**`**；`cargo test --lib` 必须在 `src-tauri/` 下跑。
- **注册表查询用 `HKCU:` / `HKLM:` PSDrive**；`Get-ItemProperty` 读默认值是 `. '(default)'`。
- **`C:\Program Files\Alpha File Manager` 是空目录残留**，别当成安装位置；真实安装在 `D:\Program Files\Alpha File Manager`。
- 本仓库工作区**有大量未提交改动**（阶段 1 的 14 源文件 + 17 语言包 + 更早批次的 `src-tauri/src/*.rs`、`docs/*`、`eslint.config.ts`、`src/utils/launch-directories.ts` 等）⇒ 动手前先 `git status --short` 看清楚，别把别人的未提交改动当成你的。

---

## 九、下阶段计划（按此顺序做，不要打乱）

### A. 先把两个现象**复现**出来（不猜、不改代码）

1. 读 `src-tauri/src/default_file_manager.rs` 的三个常量与写入路径，确认 AFM 成为默认资源管理器时**应该**写哪些键；把「应该写的键」与「本机实际键」逐条对照。
2. 确认 `default-file-manager-launcher.exe` 在磁盘上的位置（`D:\Program Files\Alpha File Manager\` 与 `tools\`），确认安装包（NSIS 脚本 `src-tauri/installer/hooks.nsh`）**是否应该**装它 —— 若本该装而没装 ⇒ R2 坐实，属安装器缺陷。
3. 在 AFM 设置 → 实验里开关一次「默认文件管理器」，立刻重新查注册表：键是否出现、值是否指向 launcher。**这一步同时验证 R1。**
4. 判别 R3：确认 MiniMax Code 的「打开文件所在目录」走的是 `shell.showItemInFolder`（硬编码 explorer.exe）还是 `shell.openPath`（查关联）。查不到源码就用 ProcMon 看它是否直接 `CreateProcess explorer.exe`。
5. 用 **CDP 版 exe + 真实用户数据**（备份后）打开 `/extensions`，抓：UIA 树、屏幕截图、`pinia._s.get('extensions')` 快照（重点看 `registry.extensions` 长度、`installedExtensionsWithManifest` 是否为空、`activeTab` 值）⇒ 判别「白屏」还是「空列表」。

### B. 修缺陷 ①（默认文件管理器）

- 按 A 的结论定位：注册没写 / 写了但没生效 / 消费方不查关联。
- 若属 AFM 侧：修注册写入（键路径、launcher 落地、失败要有可见报错，别静默成功）、补单测、补安装器改动（**任何 NSIS 改动先跑 makensis 当门禁**）。
- 若属消费方侧（R3）：AFM 侧不做无效改动，改为在设置页/文档里说明限制，并**不要**让用户以为能生效。

### C. 修缺陷 ②（市场源 + 选项卡）

- **C1 补 alpha 源**：把 `kizemo/sfm-marketplace` 提升为第二个内置源（`builtIn`、不可删不可改 URL），拆 i18n 具名键并同步 17 个语言包；确认合并优先级与卸载判定仍符合 Phase 1 口径（尤其「两个官方源同时不可达时不卸载」）。
- **C2 选项卡页面消失**：按 A.5 的结论修（可能落在 `use-extensions.ts` 过滤、`use-restored-active-tab`、或某处渲染异常）；**必须**在「全新安装 + 真实数据」组合下复验，而不是只跑夹具。

### D. 门禁与验收（沿用 Phase 1 口径）

- 门禁：`npm run ts` · `npx eslint .`（基线现量）· stylelint（改动的 vue）· vitest（全量，排除 `.superpowers`/`graphify-out`）· `cargo test --lib`（在 `src-tauri/` 下）。
- 真机验收：`npm run tauri:build` 打包产物（**不是 `tauri dev`**）→ 装机 → 走「备份 → 灌夹具 → 验收 → 杀进程 → 恢复 → 核对 SHA256」。
- 交付安装包必须**不含调试端口**（`src-tauri/tauri.conf.json` 的 `additionalBrowserArgs` 用完回滚再打包；判据：内嵌 exe 的字节流里没有 `remote-debugging-port` 字符串，且要有同尺寸对照品证明判别性成立）。
- 公网发布（若新增 alpha 源清单变更）：发布属**对外不可逆动作**，需用户明确授权后再执行。

### E. 收尾

- 把结论追加进 `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-b29-findings.md` 或新建同目录 findings，并在本文追加「十、修复结果」段（**已验证 / 推断 / 未验证** 分级 + 证据路径）。

---

## 十、修复结果（2026-11-04 回写）

### 10.1 缺陷 ①「MiniMax Code 点『打开文件所在目录』仍拉起资源管理器」

**结论：判定 R3（消费方不走文件关联），AFM 侧无法修复；不做无效改动。**

| 级别 | 结论 | 证据 |
|---|---|---|
| 已验证 | AFM 安装目录顶层只有 `alpha-file-manager.exe` 与 `uninstall.exe`，**没有 launcher** —— 这是正常设计：launcher 由 `launcher::ensure_deployment()` 在运行时部署到 `%LOCALAPPDATA%\com.sigma-file-manager.default-file-manager\`，不走 NSIS | `D:\Program Files\Alpha File Manager` 目录清单；`src-tauri/src/windows_installation.rs:33`（`DEFAULT_FILE_MANAGER_LAUNCHER_FILE_NAME`） |
| 已验证 | 该部署目录存在且内容完整：`sigma-file-manager-launcher.exe`（445 440 B）、`launch-target.txt`（内容为 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`）、`registry-snapshot.json`（18 151 B） | `C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.default-file-manager\` |
| 已验证 | 当前为「开关关闭」态：`HKCU:\Software\Classes\Directory\shell\open\command` 与 `HKCU:\Software\Classes\Drive\shell\open\command` 均 `EXISTS=False` | 注册表查询（命令见 §九·A.1 口径） |
| 已验证 | MiniMax Code 的「在文件夹中显示文件」handler 是 `electron_1.shell.showItemInFolder(filePath)` —— Electron 该 API 硬编码走系统「在资源管理器中显示」，**不查文件关联** | `D:\Program Files\MiniMax Code\resources\app.asar`（355 784 232 B）内 `IPC_CHANNELS.SYSTEM_OPEN_FILE_IN_FOLDER`；探针产物 `.superpowers\sdd\mcode-asar-probe.txt`（8 404 B） |
| 已验证 | 但同进程的 `SYSTEM_OPEN_DIR`（`system:open-dir`）走 `await electron_1.shell.openPath(dirPath)` ⇒ **查系统文件关联** ⇒ AFM 的默认文件管理器开关对 MiniMax Code「打开文件夹」类入口**有效** | `.superpowers\sdd\mcode-open-dir-probe.txt`（13 289 B）、`.superpowers\sdd\mcode-open-dir-handler.txt`（9 606 B） |
| 推断 | 所谓「拉起 explorer.exe」的机制来自 Electron `shell.showItemInFolder` 的平台实现，**未**做 `SHOpenFolderAndSelectItems` 实机探针 —— 机制层未取证，但「不走关联」这一行为层结论已由 asar 源码坐实 | — |
| 未验证 | AFM 开关打开后，MiniMax Code「打开文件所在目录」是否仍然无效的**端到端实机验证**（需先打开开关改真实注册表，本轮未执行，避免副作用） | — |

**处置**：按 §九·B 的 R3 分支执行 —— 不做无效注册表写。设置页开关本来就在
**设置 → 实验性 → 将 Alpha File Manager 设为默认文件管理器**，且已带风险提示文案（UIA 实测：
标题 `Text|将 Alpha File Manager 设为默认文件管理器 @550x41@979,511`，说明 `Text|此选项会修改系统注册表，并且可能会对您的设备产生意外的副作用。请自行承担风险后继续。 @1009x30@979,553`）。
用户若需在 MiniMax Code 里跳到 AFM，应使用「打开文件夹」类入口（走 `shell.openPath`），
而不是「打开文件所在目录」（走 `shell.showItemInFolder`）。

**交接文档 §5.4 / C2 撤回**：用户已确认「市场/已安装选项卡页面并未消失」，本轮 UIA 复验也证实
页面正常（见 10.2 的 B/C 项），因此 §九·A.5 与 §九·C2 的排查**不执行**。

### 10.2 缺陷 ②「扩展页缺 alpha 源」—— 已修复并真机验收

**用户口径覆盖原建议**：不是「alpha 作第二个内置源」，而是
**`kizemo/sfm-marketplace` 为默认官方源（排第一）**，`sigma-hub/sfm-marketplace` 降为**第三方源**列在下面。

改动（6 个源文件 + 17 个语言包）：

1. `src/data/external-links.ts` —— `extensionsRegistryOrg` 改 `'kizemo'`；新增
   `extensionsCommunityRegistryOrg='sigma-hub'`、`extensionsCommunityRegistryFullRepoName`、
   `extensionsCommunityRegistryUrl`（并在 `export default` 导出）；原「可改」注释改为「常量、固定启用、不可删、不可改地址」。
2. `src/types/extension.ts` —— `ExtensionMarketSourceKind` 扩为 `'official' | 'community' | 'custom'`。
3. `src/modules/extensions/utils/market-sources.ts` —— 新增 `COMMUNITY_MARKET_SOURCE_ID = 'community'`；
   `normalizeMarketSourceRecord` 三档 kind 兜底。
4. `src/stores/runtime/extensions.ts` —— `officialMarketSource`（kizemo，`kind='official'`）+ 新增
   `communityMarketSource`（sigma-hub，`kind='community'`），两者 `enabled/builtIn` 恒为 `true`；
   `allMarketSources = [officialMarketSource, communityMarketSource, ...marketSources]`（官方在前）。
5. `src/modules/settings/ui/categories/extensions/market-sources.vue` —— `builtInMarketSources` computed +
   `sourceKindLabel(kind)` 三档；内置源用 `v-for` 渲染（含 error/stale 状态），不渲染删除按钮。
6. `src/modules/extensions/components/extensions-marketplace.vue` —— `sourceTabs` 的 `name` 简化为
   `source.name`；import 收缩为 `import type { RegistrySourceState }`。

未改 `src/stores/storage/extensions.ts`：缓存已按 `registryCaches[sourceId]` 分桶，社区源自动落
`registryCaches['community']`。卸载判定保持既有语义（`mergedRegistry.failedSourceIds.length === 0` 才做）。

17 个语言包各插入 1 个 `marketSourceCommunity`（`ch`=第三方、`en/de/it`=Community、`ja`=コミュニティ 等），
脚本 `.superpowers/sdd/add-locale-community-key.cjs` 幂等，输出 `changed=17 skipped=0`，全部 `JSON.parse` 通过。

#### 真机验收（3.9.2 打包产物，已验证）

产物 `Alpha File Manager_3.9.2_x64-setup.exe`（**44 943 249 B**，2026-11-04 22:29:50），
静默装机后 `alpha-file-manager.exe` mtime `2026-11-04 22:29:14`（`EXE_CHANGED=True`），
新实例 pid 1439368、主窗口 hwnd 6164778。

| 项 | 结论 | 证据 |
|---|---|---|
| 扩展页选项卡页面 | **正常存在**：`市场` / `已安装 3` / `依赖项` 三个 TabItem ⇒ 用户「页面消失」确认为误报 | `.superpowers/sdd/logs/sidebar-4.txt` |
| 两个内置源都进 UI | 来源筛选条出现 `Button\|全部` / `Button\|官方` / `Button\|第三方` | 同上 |
| 两源内容真实渲染 | 列表同时有 kizemo 的 `Git Tools`（v0.1.0，已安装）与 sigma-hub 的 `Clipboard History`/`Video Downloader`/`Excalidraw`/`Media Converter`，另有 cos-overclock、hetima 条目 | 同上 |
| 筛选判别性（官方） | 点 `官方` 后只剩 `Git Tools` / `kizemo`，**无任何 sigma-hub 条目** | `logs/filter-official.txt` |
| 筛选判别性（第三方） | 点 `第三方` 后只剩 sigma-hub ×4 + cos-overclock + hetima，**无 kizemo 条目** | `logs/filter-community.txt` |
| 已安装页 | 完整可用：统计区 `3 总计 / 3 已启用 / 0 已禁用 / 2 本地 / 3.9 MB`；条目 `Focus Sync`（kizemo，v0.5.5，2.8 MB）、`Git Tools`（kizemo，v0.1.0）、`Clipboard History`（sigma-hub，v1.0.1） | `logs/tab-installed.txt` |
| 设置页市场源列表 | 官方源 `https://raw.githubusercontent.com/kizemo/sfm-marketplace/main/registry.json`（徽章「官方」）在上；第三方源 `https://raw.githubusercontent.com/sigma-hub/sfm-marketplace/main/registry.json`（徽章「第三方」）在下；两行均在空态文案「还没有添加任何市场源。」**之上** ⇒ 属内置源而非自建源 | `logs/sidebar-3.txt` |

**未验证**：两个源在 UI 中渲染出的条目是**本次实时拉取**还是读 `registryCaches` 缓存 —— 验收时**未点「刷新」**。
这不影响「两个源已接入 UI 且能按源筛选」这一已验证结论。

### 10.3 门禁结果

| 门禁 | 结果 |
|---|---|
| `npm run ts` | 通过（无输出） |
| `npx eslint .` | **186 errors / 0 warnings**；基线 `git stash push -u` 现量 = 188，差 2 来自前序未提交改动；本轮改动的 6 个源文件 + `extensions.test.ts` 单跑均 EXIT=0 |
| `npm run lint:stylelint:check` | 167 errors / 0 warnings，全部落在未触碰文件（存量）；两个改动 `.vue` 单跑 EXIT=0 |
| `cargo test --lib` | **912 passed / 0 failed / 0 ignored**（`logs/cargo-test.log`） |
| vitest 全量 | `4 failed / 283 files`、`15 failed / 2629 tests`。其中 `extensions.test.ts` 的 1 条**本轮引入**（新增社区源后 mock 未应答社区 URL）→ 已修，单跑 `15 passed (15)`；其余 3 个失败文件为**存量、与本轮无关**（`file-browser-tree-view.test.ts` 的 `Need to install with 'app.use' function`、`module-load-recovery.test.ts`、`readme-markdown-table.test.ts`）（`logs/vitest-round.log`） |

**交付物不含调试端口（已验证）**：`src-tauri/tauri.conf.json` 中无 `remote-debugging-port` /
`additionalBrowserArgs`，且该文件 `git status` 干净。

### 10.4 证据索引（`.superpowers/sdd/`）

- 构建：`logs/tauri-build.log`（`Finished release profile … 3m 20s` → `Running makensis` → 1 bundle）
- 装机：`logs/accept-market-a.log`、`shots/accept-market-a.png`、脚本 `accept-market-a.ps1`
- 导航与页面：`logs/sidebar-probe.txt`、`logs/sidebar-0..5.txt`、`logs/setp-{enter,general,experimental}.txt`
- 筛选判别：`logs/filter-official.txt`、`logs/filter-community.txt`、`logs/tab-installed.txt`、脚本 `accept-market-filter.ps1`
- 缺陷①：`mcode-asar-probe.txt`、`mcode-open-dir-probe.txt`、`mcode-open-dir-handler.txt`
- 详细 findings：`.superpowers/sdd/accept-20261104-market-sources-findings.md`

### 10.5 可复用验收脚本（全部纯 ASCII）

```powershell
$sdd='F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd'
# 通用 UIA 深度枚举（-All 含无名节点）
powershell -NoProfile -ExecutionPolicy Bypass -File "$sdd\accept-uia-dump.ps1" -All -Out "$sdd\logs\accept-uia-all.txt"
# 侧边栏 6 个无名导航项逐个 Invoke + 关键词判定（设置=IDX3，扩展=IDX4）
powershell -NoProfile -ExecutionPolicy Bypass -File "$sdd\accept-sidebar-probe.ps1" -Start 0 -Count 6 -Out "$sdd\logs\sidebar-probe.txt"
# 分步导航：Kind:Name:OutFile（Name 用 \uXXXX 转义；Side:N 走侧边栏序号）
powershell -NoProfile -ExecutionPolicy Bypass -File "$sdd\accept-nav.ps1" -OutDir "$sdd\logs" -Steps 'Side:3:\\u5b9e\\u9a8c\\u6027:setp-experimental.txt'
# 来源筛选判别（点 官方 / 第三方 / 已安装 3）
powershell -NoProfile -ExecutionPolicy Bypass -File "$sdd\accept-market-filter.ps1" -OutDir "$sdd\logs"
```

### 10.6 未完成 / 留给下一轮

- 缺陷① 机制层（`SHOpenFolderAndSelectItems`）实机探针未做；若要彻底定论，可用 ProcMon 观察
  MiniMax Code 在触发该入口时是否直接 `CreateProcess explorer.exe`。
- 两个市场源的「刷新」实时拉链路未单独验收（`raw.githubusercontent.com` 在本机曾超时，但未表现为功能故障）。
- 上一轮留下的 3 个存量 vitest 失败文件仍未处理，与本次改动无关。
