# Files 项目对标分析（2026-11-02）

> **本文是分析文档，不是交接文档。** 交接入口仍是
> `docs/handoff-2026-11-02-phase4-winget-and-search-scan.md`（优先级高于本文）。
> 本文只回答一个问题：**files-community/Files 有哪些能力，是 Alpha File Manager 下一阶段值得借鉴或引入的。**
>
> 面向用户的文字一律简体中文；代码标识符、路径、命令、技术专有名词保持原样。

---

## §0 证据来源与口径

### 0.1 Files 侧

| 来源 | 取法 | 用途 |
|---|---|---|
| 仓库目录树 | `git clone --depth 1 --filter=blob:none --no-checkout` 到 `%TEMP%\files-analysis`，**2421 个文件** | 功能域划分 |
| 动作命令面 | `src/Files.App/Actions/**` 共 **172 个文件**（Navigation 39 / Content 34 / FileSystem 31 / Open 19 / Global 14 / Show 11 / Display 7 / Git 6 / Shelf 3 / Sidebar 3 / Start 2） | 功能清单的硬证据（命令面 ≈ 功能面） |
| 官方文档站 | `files.community/docs` 侧栏 | 官方口径的功能分类 |
| v4.3 发布说明 | `files.community/blog/posts/v4-3-0`，2026-10-11 | 最新工程方向 |
| 仓库元数据 | GitHub API：46450 star / 3012 fork / MIT / C# / **最后推送 2026-11-01** / open issues 467 | 定位与活跃度 |

⚠️ 仓库**没有根目录 `README.md`**，`/readme` API 返回的是 `.github/README.md`（营销向），
所以功能清单以**代码结构 + 官方文档站**为准，不以 README 为准。

### 0.2 AFM 侧

| 来源 | 取法 |
|---|---|
| `README.zh-CN.md` 功能表 | 已有能力的第一手口径 |
| `src/modules/` 目录清单（20 个模块） | 前端功能域 |
| `src-tauri/src/` 文件 + 子目录清单（41 文件 / 10 子目录） | 后端能力域 |
| `git grep -l` 关键词探针 | 判定 AFM 某能力「有没有」，避免凭印象 |

### 0.3 许可证与可移植性（先说结论）

- Files 主体 **MIT**（部分文件 **MPL-2.0**）。AFM 是 **GPL-3.0-or-later**。
  ⇒ **MIT 代码在许可证上可以并入 GPL-3 项目**（保留版权声明即可），不存在法律障碍。
- 但**技术栈完全不同**：Files = C# + WinUI 3 + Windows App SDK（**纯 Windows**、MSIX/Store 分发）；
  AFM = Tauri v2 + Vue 3 + Rust（**Windows + Linux**、NSIS 安装包 + 站点 + winget）。
  ⇒ **代码基本不可直接移植**，能迁移的只有**交互范式、查询语言设计、工程模式**。
- 这一条决定了下面所有建议的形态：**借鉴「怎么做」，不抄「用什么做」。**

---

## §1 一句话结论

> **Files 走的是「Windows 原生外壳 + 不自建全盘索引」；AFM 走的是「跨平台 Web 技术栈 + 自建全盘索引」。**
> ⇒ 对 AFM 真正有价值的，**不是** Files 的某个具体功能，而是三条更底层的判断：
> ① **搜索不一定需要全盘索引**（Files 直接没有）；
> ② **性能才是 Files 的主战场**（v4.3 整版都是性能：启动、导航、搜索、内存）；
> ③ **外壳集成深度是 Windows 文件管理器的真正护城河**（Shell COM、Jump List、预览宿主、注册为默认程序）。
>
> 结论落到行动：**下一阶段不该抄功能清单，而该用 Files 的证据重排 AFM 的任务优先级。**

---

## §2 Files 功能全景（按域，含代码坐标）

| 域 | Files 的实现 | 关键坐标 |
|---|---|---|
| **导航外壳** | 侧栏树 / 标签页 / 双栏 / 列视图 / 面包屑 Omnibar | `Views/Shells/{Modern,Column}ShellPage`、`Views/Layouts/{Details,Grid,Columns}LayoutPage`、`Actions/Navigation/*`（23 个） |
| **Omnibar** | 面包屑**兼**路径编辑器**兼**搜索框；FTP 路径也有导航箭头 | `Actions/Global/EditPathAction` |
| **搜索** | **目录内实时搜索** + **自然语言查询** + **标签查询语言** | `Utils/Storage/Search/{FolderSearch,NaturalLanguageSearch}.cs`、`TagQueryExpression.cs`、`TagTerm.cs` |
| **Git** | init / clone / fetch / pull / push / sync；仓库浏览；**在 IDE 打开**；操作**进度上报**；失败给**真实错误**而非超时 | `Actions/Git/*`（6）、`Utils/Git/*`、`Services/Git/*`、`Dialogs/{GitHubLogin,CloneRepo,AddBranch}Dialog` |
| **云盘** | OneDrive / Google Drive / Dropbox 等**同步状态检测**，作为独立卷呈现 | `Utils/Cloud/{CloudDrivesManager,CloudDriveSyncStatus,ICloudDetector}.cs` |
| **Shelf（文件台）** | 独立于剪贴板的暂存区，跨目录搬运文件 | `Actions/Shelf/{Copy,Cut,Delete}ItemFromShelfAction` |
| **Home + Widgets** | 首页可插拔小组件（快速访问、驱动器…），可增删可开关 | `Utils/Widgets/WidgetsHelpers.cs`、`Views/HomePage.xaml` |
| **状态中心** | 后台操作统一入口，带进度模型 | `Utils/StatusCenter/{StatusCenterHelper,StatusCenterItem,StatusCenterItemProgressModel}.cs` |
| **Windows 外壳集成** | Shell COM、上下文菜单、`Open with`、ShellNew、跳转列表、快速访问、最近项、壁纸、预览宿主 | `Utils/Shell/*`（14）、`Services/Windows/*`（10） |
| **第三方预览宿主** | 可调用 **QuickLook / Seer / PowerToys Peek** 做预览 | `Services/PreviewPopupProviders/*`（4） |
| **回收站** | 还原 / 全部还原 / 清空 | `Actions/FileSystem/{RestoreRecycleBin,RestoreAllRecycleBin,EmptyRecycleBin}Action` |
| **压缩** | 7-Zip 内嵌（`7z.dll`/`7z64.dll`/`7zArm64.dll`），压缩/解压/跳过项处理/超限提示 | `Dialogs/{CreateArchive,DecompressArchive,CompressSkippedItems,FileTooLarge}Dialog` |
| **批量改名** | 带预览 | `Dialogs/BulkRenameDialog` |
| **权限与提权** | 以管理员身份运行（带确认）、格式化磁盘、创建备用数据流 | `Dialogs/ElevateConfirmDialog`、`Actions/FileSystem/{FormatDrive,CreateAlternateDataStream}Action` |
| **可配置性** | **工具栏自定义页**、**键位编辑器**、动作页、标签页管理、文件标签页 | `Views/Settings/{ToolbarCustomizationPage,ActionsPage,TagsPage}.xaml`、`Dialogs/KeyBindingEditorDialog` |
| **显示控制** | 信息窗格 / 详情窗格 / 预览窗格 / Shelf 窗格 / 侧栏 / 工具栏 / 隐藏项 / 文件扩展名 / 过滤头，各自有开关 | `Actions/Show/*`（11） |
| **排序与分组** | 自然排序、文件优先/文件夹优先/混排、**按列分组**、列宽自适应 | `Actions/Display/{Sort,Group,AutoFitColumns}Action` |
| **历史** | 全局 **Undo / Redo**、重开已关闭标签、复制标签 | `Actions/Global/{Undo,Redo}Action`、`Actions/Navigation/ReopenClosedTabAction` |
| **紧凑覆盖层** | 快捷键切换的紧凑浮层 | `Actions/Global/{Enter,Exit,Toggle}CompactOverlayAction` |

---

## §3 逐项对比：AFM 现状 vs Files

图例：✅ AFM 已有且相当或更强 · 🟡 AFM 形态不同 · ❌ AFM 没有（探针实测未命中）

### 3.1 两侧都强，**不必抄**的

| 能力 | AFM | Files |
|---|---|---|
| 常驻目录树 | ✅ | ✅（拖放到 tab 也支持） |
| 标签页 / 双栏 | ✅ **且有 Files 没有的「左右互换」按钮 + 同步浏览开关** | ✅ |
| 命令面板 | ✅ | ✅ |
| 标记 Tags | ✅ | ✅ |
| 收藏 | ✅ **按分类**（Files 无分类） | ✅ |
| 终端 | ✅（`src-tauri/src/terminal/`） | ✅ |
| 快速查看 | ✅ 内建 | ✅ 但走**第三方宿主**（QuickLook/Seer/Peek） |
| 背景图 | ✅ **且有 Photoshop 式混合模式** | ✅ |
| 压缩包 | ✅ Zip | ✅ 7-Zip 全格式 |
| 批量改名 | ✅ **带实时预览 + 冲突拦截 + 撤销** | ✅ |
| 状态中心 | ✅ | ✅ |
| FTP | ✅ **SFTP/FTPS + 证书固定**（更强） | ✅ |
| 映射网络驱动器 | ✅ | ✅ |
| 全局撤销/重做 | 🟡 **仅批量改名有 undo，无 redo**（探针 `redo` = 0 文件） | ✅ 全局 Undo/Redo |
| 工具栏 / 侧栏 / 隐藏项 开关 | ✅ | ✅ |

### 3.2 **AFM 没有、Files 有**（按借鉴价值排序）

| # | 能力 | 证据（AFM 探针） | 价值 | 成本 |
|---|---|---|---|---|
| **1** | **Git 集成**（init/clone/fetch/pull/push/sync + 仓库浏览 + 在 IDE 打开） | `gitstatus\|libgit\|git_repo\|gitclient` = **0 文件** | ★★★★★ 与 AFM 的「AI 助手 + MCP」定位天然合流 | 高（需 Rust git 库 + 凭据管理） |
| **2** | **列视图（Columns View）** | `column_view\|columnview\|columns_layout` = **0 文件** | ★★★★ macOS 式逐级展开，AFM 只有列表/网格 | 中 |
| **3** | **自然语言查询**（`size:>1GB` / `type:pdf` / `modified:本周`） | AFM `query.rs:526` 注释明确：**全盘侧只做文件名子串粗筛** | ★★★★★ AFM 已有字段（size/type/mtime 都在 schema 里），纯前端解析即可 | **低**（最高性价比） |
| **4** | **标签查询语言**（AND / OR / NOT） | `TagQueryExpression.cs` / `TagTerm.cs` | ★★★★ AFM 有标签但无组合查询 | 低 |
| **5** | **Shelf 文件台**（跨目录暂存区） | `shelf` = **0 文件** | ★★★★ 批量整理场景，AFM 的剪贴板只做单次 | 中 |
| **6** | **可插拔 Widgets（首页组件）** | AFM 有 dashboard，但是固定模块 | ★★★ AFM 已有扩展市场 ⇒ 可做成**扩展市场里的 widget 类型** | 中 |
| **7** | **工具栏自定义页** | `toolbar_custom` = **0 文件** | ★★★ | 低 |
| **8** | **键位编辑器（可视化改键）** | `shortcut_editor` = **0 文件**；AFM 只在设置里列快捷键 | ★★★ AFM 已有 `src/stores/runtime/shortcuts.ts` 体系 | 中 |
| **9** | **Windows 外壳集成深度** | Jump List = **0 文件**；`CompactOverlay` = **0 文件** | ★★★★ AFM 只做了「注册为默认程序」 | 中～高（Shell COM） |
| **10** | **自然排序**（`Folder 2` 排在 `Folder 10` 前） | `naturalsort` = **0 文件** | ★★★ AFM 有排序，但按位置分列时顺序可能不自然 | **低** |
| **11** | **按列分组 / 列宽自适应** | `group_by` = **0 文件** | ★★★ | 低 |
| **12** | **重开已关闭标签 / 复制标签** | 未探命中 | ★★ | 低 |
| **13** | **以管理员身份运行（带确认框）** | — | ★★★ AFM 有 `windows_installation.rs` 但无运行时提权 | 中 |
| **14** | **创建备用数据流（ADS）** | `alternate.?data.?stream` = **0 文件** | ★★ Windows 特有，Linux 不适用 | 低 |
| **15** | **云盘同步状态作为独立卷** | `onedrive\|cloud_drive\|dropbox` = 4 文件（疑似误命中） | ★★★ AFM 有远程（SFTP）但无云盘 | 高 |
| **16** | **格式化磁盘 / 回收站全部还原** | — | ★★ 高危操作，AFM 缺失是有道理的 | — |
| **17** | **自动更新检查（运行中轮询）** | AFM 有 `app_updater` + `version_json` | ★★ AFM 是手动检查 | 低 |

### 3.3 **Files 没有、AFM 独有**（保持优势，别在借鉴时被稀释）

| 能力 | 说明 |
|---|---|
| **Focus Sync** | 任意应用的打开/另存为对话框跟随当前目录。Files 没有对等物（它靠 Explorer 的 shell 集成） |
| **内置 MCP 服务** | 六个只读工具 + 六家助手一键部署。Files 无 |
| **递归比较与同步** | 时间→大小→校验和，与 WinSCP 同序；「方向待定」不猜方向；计划先于动手；**永不删东西**。Files 无 |
| **SFTP / FTPS + 主机密钥与证书固定** | Files 只有明文 FTP |
| **双栏「左右互换」+ 同步浏览** | Files 的 Dual pane 没有互换按钮 |
| **扩展市场 / 扩展体系** | 命令、页面、快捷键、设置项都可扩展。Files 无插件体系 |
| **目录内搜索是真递归** | Files 的 `FolderSearch` 是当前目录实时过滤 |
| **Linux 支持** | Files 是纯 Windows |

---

## §4 对任务 F（全盘搜索极慢）的直接影响 —— 本次分析最有价值的一条

### 4.1 现状事实（来自阶段四交接文档，已实测）

- 查询侧：**21 ms**（191 万文档）⇒ **不是瓶颈，不要改查询侧**
- 扫描侧：四盘 **G: → F: → D: → C: 串行**，每轮 **12.2–12.5 分钟**，四轮高度一致
  ⇒ 结构性串行，不是抖动。并行化理论下界 ≈ max(C:) ≈ **6.3 分钟**
- **AFM 的 `global_search` 里没有任何 watcher / mtime 驱动的增量刷新**（探针 `watch|notify|mtime` 在该目录 0 命中）

### 4.2 Files 的做法：**它压根不建全盘索引**

Files 的搜索由三块拼成，**没有一处是「预先扫全盘建索引」**：

1. `FolderSearch.cs` —— 当前目录内的实时过滤（打开目录即有结果）
2. `NaturalLanguageSearch.cs` —— 自然语言算子在**当前目录树**上求值
3. `TagQueryExpression.cs` —— 标签的 AND/OR/NOT 组合

v4.3 的「Faster search」优化的是**结果渲染速度**，不是索引构建速度 —— 因为没有索引要建。

### 4.3 由此得到的三条结论

| # | 结论 | 对任务 F 的含义 |
|---|---|---|
| **A** | **并行化是「治标」，增量/免重建才是「治本」** | 阶段四把候选列为「四盘并行」与「避免全量重建」，Files 的证据把后者从「备选」提升为**主候选** |
| **B** | **AFM 已有 watcher 基础设施可以复用** | `src-tauri/src/dir_watcher.rs` 已存在（用于别的用途）；把它接到 tantivy writer 上做增量更新，即可让 12.5 分钟的每轮全量重建变成「首次全量 + 之后增量」 |
| **C** | **如果保留全盘索引，它的形态应该是「查询语言」而不是「文件名匹配」** | AFM 全盘侧现在只做文件名子串粗筛（`query.rs:526` 注释自陈），而 schema 里 size/type/mtime 字段齐全 ⇒ **加自然语言算子几乎零后端成本**，却让全盘搜索立刻比 Files 强 |

### 4.4 建议的任务 F 方案取向（供决策，未写代码）

```
优先级 1：增量索引（watcher 驱动）—— 根治，消除 12.5 分钟每轮重建
优先级 2：四盘并行 —— 兜底，首次全量从 12.5 min 压到 ~6.3 min
优先级 3：自然语言算子 —— 独立价值，顺手做
```

⚠️ 无论选哪条，**验收口径不变**：同一探针、同一埋点、重跑 4 轮，**拿实测数字说话**。
⚠️ 动 Rust 前必读：`cargo test --lib` 里被链接到的代码**不能碰 `tauri::AppHandle` / `tauri::async_runtime::*`**
（会把 tao 拖进测试二进制 → `STATUS_ENTRYPOINT_NOT_FOUND` 0xC0000139，一个测试都跑不了）。
绕法：加 `*_blocking` 同步内核入口，调用方自己选执行器。

---

## §5 借鉴清单（分档，可直接进下阶段计划）

### A 档 —— 低风险、高性价比，建议直接排进下阶段

| 项 | 借鉴点 | 落点 | 预估 |
|---|---|---|---|
| A1 | **自然语言查询算子**（`size:` / `type:` / `modified:` / `name:`） | `src-tauri/src/global_search/query.rs`（解析层）+ 前端搜索框提示 | 小～中 |
| A2 | **自然排序** | 目录树 / 面包屑 / 列表排序三处统一 | 小 |
| A3 | **列视图（Columns View）** | `src/modules/navigator` 新增一种布局 | 中 |
| A4 | **工具栏自定义页** | 设置页新增一页 | 小～中 |
| A5 | **键位编辑器** | 复用 `src/stores/runtime/shortcuts.ts` | 中 |
| A6 | **按列分组 / 列宽自适应** | 列表视图增强 | 小 |

### B 档 —— 需要评估或与既有投入取舍

| 项 | 借鉴点 | 取舍点 |
|---|---|---|
| B1 | **Git 集成** | AFM 已押注「MCP + AI 助手」，Git 能力与该定位同源；但 Rust 侧需引入 git 库并处理凭据，**建议做成扩展而非内置**（走扩展市场，与 focus-sync 同思路） |
| B2 | **Shelf 文件台** | AFM 已有剪贴板与批量操作，Shelf 是「跨会话暂存区」；与批量改名、比较同步配合价值大 |
| B3 | **Widget 化首页** | AFM 的 dashboard 是固定模块；改成可插拔后，**扩展市场能供用户装 widget** ⇒ 与 A 档扩展体系同构 |
| B4 | **Windows 外壳集成深化**（Jump List / 上下文菜单 / 预览宿主） | AFM 已能接管默认程序，可再补 Jump List 与「用 QuickLook/Peek 预览」 |
| B5 | **列视图 / 标签查询语言**之外的 **重开已关闭标签 / 复制标签 / 全局 redo** | 小而独立的体验补齐，建议打包成一批「会话与历史」改进 |
| B6 | **运行时提权（Run as admin）** | AFM 有 `windows_installation.rs`，运行时提权需处理 UAC 与失败回退 |

### C 档 —— 明确不建议照搬

| 项 | 原因 |
|---|---|
| **自建 WinUI / 改用 C#** | 技术栈与跨平台目标冲突；MIT 允许抄，但抄了等于放弃 AFM 的 Tauri/Vue/Rust 与 Linux 支持 |
| **磁盘格式化 / 批量清空回收站** | 高危不可逆操作，AFM 定位是日常整理工具，不是系统管理工具 |
| **创建 ADS（备用数据流）** | Windows 专有，且属小众；放在扩展里更合适，不进核心 |
| **云盘作为独立卷** | 依赖各家云盘 SDK，成本高；AFM 已有远程服务器能力线，同一需求已被 SFTP 覆盖大半 |
| **Files 的「无索引」路线本身** | AFM 的全盘搜索 + MCP「搜索文件」工具是**对外承诺的能力**，直接学 Files 砍掉索引 = 砍掉卖点 |

---

## §6 工程机制层面可直接借鉴的 8 条（Files 有实现，可读源码取经）

> ⚠️ 只读思路，不抄代码（技术栈不同）；MIT 许可本身允许移植，但跨语言移植成本高于重写。

| # | 机制 | Files 坐标 | 借鉴价值 |
|---|---|---|---|
| 1 | **长任务进度上报 + 真实错误（不报通用超时）** | `Utils/StatusCenter/StatusCenterItemProgressModel.cs` | ★★★★★ v4.3 明确把「失败时给真实错误而不是通用超时」列为改进；**AFM 的删除作业与搜索作业都该照此改** |
| 2 | **索引/缓存与文件系统变更解耦**（watcher + cache） | `Services/Storage/ServerDriveWatcher.cs`、`StorageCacheService.cs` | ★★★★★ 对应 AFM 任务 F 的「避免全量重建」 |
| 3 | **侧栏/标签页的拖放高亮用强调色边框** | v4.3「Improved drop indicators」 | ★★★ 纯 UI，成本极低 |
| 4 | **Git 仓库探测在打开目录时并行做、不阻塞** | v4.3「Git repositories are now detected faster」 | ★★★★ 对应 AFM「打开大目录卡顿」的同类问题 |
| 5 | **自然语言搜索算子** | `Utils/Storage/Search/NaturalLanguageSearch.cs` | ★★★★★ 见 §4.3-C |
| 6 | **标签查询表达式（AND/OR/NOT）** | `Utils/Storage/Search/TagQueryExpression.cs` | ★★★★ |
| 7 | **自定义工具栏 + 键位编辑器的产品形态** | `Views/Settings/ToolbarCustomizationPage.xaml`、`Dialogs/KeyBindingEditorDialog.xaml` | ★★★ |
| 8 | **动作层与 UI 解耦（`IAction` + `GeneratedRichCommand`）** | `src/Files.App/Actions/IAction.cs`、`BaseUIAction.cs` | ★★★★ **对 AFM 最有架构价值**：AFM 的命令目前散在组件里；统一到「动作 = 唯一事实源」后，命令面板、快捷键、工具栏、右键菜单、扩展命令可以共用一份注册表 |

---

## §7 风险与不可照搬的部分

1. **性能观不可直接对标**。Files v4.3 的「启动快」是 WinUI 3 + 自带 .NET 运行时 + 60% 更小包的成果；
   AFM 是 WebView2 + Rust，**冷启动 3–5 分钟**是本项目既有事实（AGENTS.md 明记）。
   不要把 Files 的性能数字当 AFM 的目标。
2. **外壳集成有平台代价**。Jump List / Shell COM / ADS / 提权全是 **Windows 专有**，
   AFM 同时支持 Linux ⇒ 这类能力**必须做平台隔离**（AFM 已有 `terminal/{windows,linux,macos}.rs` 这类分层先例）。
3. **Windows Store / MSIX 分发不可照搬**。AFM 走 **NSIS + 站点 + winget**，Files 走 MSIX + Store + winget。
4. **别把 Files 的"简洁"当"缺"**。Files 没有全盘索引，是因为它的用户画像与 AFM 不同；
   照抄会让 AFM 丢掉 MCP「搜索文件」工具与全局搜索这条对外承诺。
5. **探针纪律仍然适用**。本文全部结论都有代码坐标或探针命中数支撑；任何一条若要变成实施方案，
   仍需**真机验收 + 打包产物**（`tauri dev` 的界面结论一律作废）。

---

## §8 建议的下一步（本文不改代码，供决策）

### 立刻可做（不与既有任务冲突）

1. **任务 F 方案重排**：把「增量索引」提到并行化之前，用 §4.3 的证据支撑，
   并把「自然语言算子」拆成独立小任务（与 F 解耦，A 档可先落地）。
2. **A 档六项**（自然语言算子 / 自然排序 / 列视图 / 工具栏自定义 / 键位编辑器 / 分组与列宽）
   可组成一个「浏览与操作效率」批次。

### 需要用户拍板的

| 决策点 | 选项 |
|---|---|
| 任务 F 走「增量索引」还是「四盘并行」 | 增量（根治，成本高）/ 并行（兜底，见效快）/ 两者都做（增量为主 + 并行加速首轮） |
| Git 集成进核心还是进扩展市场 | 内置（重）/ 扩展（轻，与 focus-sync 同思路）/ 不做 |
| 列视图做不做 | 做（AFM 缺 macOS 式导航）/ 不做（保持列表+网格，摊薄战线） |
| 是否引入 Files 的「动作注册表」架构 | 引入（大重构，收益是命令/快捷键/工具栏/扩展共用一份事实源）/ 暂不（先补功能再重构） |

### 不受本阶段阻塞、可并行推进

- **任务 E（winget 3.9.2 PR）**：仍被外部审核员卡住，Agent 无法代劳，**保持等待**。
- **任务 G（focus-sync settings 腰斩）**：仍需用户确认同步状态。

---

## §9 附：本文用到的可复用命令

```powershell
# Files 侧：只取目录树，不下载 blob
git clone --depth 1 --filter=blob:none --no-checkout https://github.com/files-community/Files.git $env:TEMP\files-analysis
git -C $env:TEMP\files-analysis ls-tree -r --name-only HEAD

# AFM 侧：判定某能力「有没有」（不要凭印象）
git -C 'F:\soft\00selfmade\filemanager\sigma-file-manager' grep -l -i -E 'column_view|gitstatus|shelf|naturalsort' -- src src-tauri/src
```

> ⚠️ `git grep -l` 的命中数**只能证明「有这个名字」**，不等于「有完整实现」。
> 本文中所有 ❌ 结论都基于**零命中 + 该域的文件清单交叉验证**；所有 ✅ 结论基于 README 功能表或直接读到源码。
