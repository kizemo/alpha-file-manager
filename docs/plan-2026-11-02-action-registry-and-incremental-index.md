# 阶段五实施计划 · 动作注册表 + 增量索引（2026-11-02）

> **本文是计划文档。** 接力入口仍是 `docs/handoff-2026-11-02-phase4-winget-and-search-scan.md`；
> 对标分析见 `docs/analysis-2026-11-02-files-benchmark.md`。
> 面向用户的文字一律简体中文；代码标识符、路径、命令保持原样。

---

## §0 已定的三项决策（用户拍板，本轮不再讨论）

| # | 决策 | 结论 |
|---|---|---|
| 1 | 全盘搜索扫描侧（任务 F） | **增量索引（watcher 驱动）为主** —— 根治，消除每轮 12.2–12.5 分钟全量重建；四盘并行降为兜底 |
| 2 | Git 集成归属 | **做成扩展，走扩展市场** —— 核心不膨胀，与 focus-sync 同思路 |
| 3 | 动作注册表架构 | **现在就引入** —— 一次做对，后续所有命令类功能都受益 |

---

## §1 阶段五范围与顺序

```
批次 1  动作注册表内核 + 第一个适配器（命令面板）   ← 纯前端，可单测验证，无需真机打包
批次 2  注册表接快捷键 store + 快捷键设置页可编辑
批次 3  注册表接工具栏 + 右键菜单 + 扩展命令 API
批次 4  任务 F 增量索引（watcher → tantivy 增量写）   ← Rust，风险最高，独立验收
批次 5  A 档其余：自然语言算子 / 自然排序 / 分组与列宽
批次 6  批量 2：列视图 / 工具栏自定义 / 键位编辑器
批次 7  Git 扩展（走扩展市场，复用 focus-sync 的打包与安装思路）
```

**批次 1–3 与批次 4 无依赖**，可交替推进；批次 5–6 依赖批次 1–3 的注册表（否则同一批能力会实现成三套入口）。

⚠️ 批次 4 单独一次真机验收，**必须重跑 4 轮全盘扫描拿实测数字**，不能只交代码。

---

## §2 批次 1–3：动作注册表

### 2.1 现状勘察（已实测，文件坐标）

AFM **已有**的全部零件：

| 零件 | 位置 | 性质 |
|---|---|---|
| 命令对话框 UI 组件 | `src/components/ui/command/*`（command.vue / command-item / command-group / command-shortcut / command-input …） | **通用 UI 组件**，不含任何业务命令 |
| 快捷键 store | `src/stores/runtime/shortcuts.ts` | 命令与快捷键绑定关系 |
| 扩展命令 API | `src/modules/extensions/api/create-commands-api.ts` | 扩展**已经能加命令** |
| 扩展键位 API | `src/modules/extensions/api/keybindings.ts` | 扩展已经能加快捷键 |
| 快捷键设置页 | `src/modules/settings/ui/categories/shortcuts/` | 现状是**只读列举** |
| 右键菜单 | `src/components/dir-entry-interactive/dir-entry-context-menu.vue` 等 | 每个菜单各自硬编码 |
| 工具栏 | `src/modules/window-toolbar/` | 按钮各自硬编码 |

⇒ **缺的不是零件，是把它们统到一份事实源的「注册表」。**

### 2.2 目标架构

```
                  ┌──────────────────────────────────┐
                  │   Action Registry（唯一事实源）   │
                  │   id / 标题 / 图标 / 快捷键 /      │
                  │   上下文谓词 / 执行器 / 启用态     │
                  └──────────────────────────────────┘
                        ▲      ▲      ▲      ▲      ▲
        适配器：   命令面板 │ 快捷键 │ 工具栏 │右键菜单│ 扩展命令 API
```

**关键设计点**

1. **注册表是纯数据 + 执行器**，不依赖任何 UI 组件 ⇒ 可在无 DOM 的环境单测。
2. **上下文谓词**决定命令当前是否可用（是否选中条目、是否在远程、是否在搜索态）。
   参照 Files 的 `IAction.IsExecutable` / `IsAccessibleGlobally`。
3. **命令面板 / 工具栏 / 右键菜单 / 扩展 API 全部变成注册表的「投影」**，
   不再各自硬编码命令清单 —— 这是本批次的核心收益。
4. **快捷键与命令解耦**：快捷键绑定指向注册表 id；同一个命令可有多组快捷键。
5. **扩展命令走同一条注册路径**：扩展 API 调 `registry.register()`，
   于是扩展命令自动出现在命令面板与（可选）右键菜单里 —— 现状是扩展命令自成一套。

### 2.3 分批实施（TDD）

| 批次 | 交付物 | 单测判据 |
|---|---|---|
| 1 | `src/core/actions/`：类型定义 + 注册表实现 + 「命令面板适配器」 | 注册 / 注销 / 冲突 id 报错 / 谓词过滤 / 上下文求值顺序；**必须有负对照** |
| 2 | 快捷键 store 改为「绑 id」；快捷键设置页改为可编辑 | 绑定解析、冲突检测、设置页改键后持久化 |
| 3 | 工具栏与右键菜单改为读注册表；扩展命令 API 改为走注册表 | 投影结果与注册表一致；扩展注册后命令面板能搜到 |

**统一要求**（沿用本项目既有做法）

- 先写测试拿到 **RED**，再实现拿 **GREEN**；日志落 `.superpowers/sdd/`。
- 每批至少一条**负对照**（改坏实现必须红）。
- 门禁：`npm --prefix <repo> run ts` = 0；改动文件 eslint = 0（绝对路径）；
  `npm --prefix <repo> run test:unit:run -- <files>` 全绿；stylelint 只允许 OLD 违规。
- **eslint 基线口径**：用 `git stash push -u` 量 HEAD 的 `eslint .`，同一条命令逐字对照；
  拿旧数字对会误判回归（基线随提交变）。
- 契约测试用 `not.toContain` 钉死字面量时，注释里**不得出现**被钉的前缀。

### 2.4 风险

| 风险 | 处置 |
|---|---|
| 范围过大拖死本阶段 | 批次 1 只做「内核 + 命令面板」一个适配器，其余适配器分批 |
| 快捷键语义变化导致用户既有绑定失效 | 批次 2 必须**保留旧格式兼容读取**，且迁移失败时回落旧逻辑 |
| 扩展 API 是对外契约 | 批次 3 保留 `createCommands()` 的**旧签名**，新增注册路径为可选 |
| 引入大批新文件 | 每批结束 `git diff --stat` 复核，只含预期文件；新文件整份算 NEW lint |

---

## §3 批次 4：任务 F 增量索引（Rust，风险最高）

### 3.1 目标

把「每轮全量重建 191 万文档索引（12.2–12.5 min）」改成
**「首次全量 + 之后由文件系统变更事件增量更新」**。

### 3.2 方案要点

- **复用已有 watcher**：`src-tauri/src/dir_watcher.rs` 已存在（当前用于别的用途），
  接到 tantivy writer 上做增量：`create` / `update` / `delete` 三类事件分别对应
  增删改文档。
- **按根分区驱动**：每块盘一个 watcher 通道，避免单通道事件风暴。
- **降级策略**：watcher 不可用（不支持的文件系统、网络盘、权限不足）时
  **回落到现有全量扫描**，保证功能不退化。
- **首次全量仍需并行**：第一次建索引时四盘并行（12.5 min → ~6.3 min），
  与增量方案叠加后「首次」和「后续」都快。

### 3.3 硬约束（动 Rust 前必读）

- 🚨 **`cargo test --lib` 里任何被测试链接到的代码都不能碰 `tauri::AppHandle` 或
  `tauri::async_runtime::*`** —— 会把 tao 整条窗口链拖进测试二进制，
  tao 的 `comctl32.dll!TaskDialogIndirect` 属 comctl32 v6，测试二进制没有 manifest
  ⇒ 加载期 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉，**一个测试都跑不了**，
  且报错完全看不出跟你的模块有关。
  **绕法**：给业务模块加 `*_blocking` 同步内核入口，调用方自己选执行器。
- ⚠️ `target\debug` 已在本阶段被清除（69.74 GiB）⇒ **下一次 `cargo build` / `cargo test --lib` 要从零重编**（约十几分钟），别以为是环境坏了。
- `#[tool_router]` 不等于接线完成：`impl ServerHandler` 上必须加 `#[tool_handler]`。

### 3.4 验收口径（不可改）

- 同一探针：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\task3-search-accept-20261102-v2.ps1`
- 同一埋点：`src-tauri/src/global_search/scan.rs:346`
  格式 `[global-search] scan root timings: root={} elapsed={}ms indexed={} depth={}`
- **重跑 4 轮全盘扫描**，对比基线（每轮 728,966 / 748,192 / 730,123 / 750,079 ms）。
- 判据：① 首轮 ≤ 6.3 min（四盘并行生效）；② 稳态（增量）轮 **显著低于 12.2 min**；
  ③ 增量更新后**查询结果与全量重建一致**（用同一批已知路径做交叉验证）。
- ⚠️ **判后台任务状态看文件系统（文件数是否停止增长），不要看屏幕**
  —— `PrintWindow` 会返回陈旧帧。
- ⚠️ **真机验收必须用 `tauri build` 打包产物**；`tauri dev` 的界面结论一律作废。
- ⚠️ **验收计时期间不跑任何编译类命令**（抢 CPU 会虚高读数）。

---

## §4 批次 7：Git 扩展

- 形态：**扩展**，不是内置功能。参照 `extensions/kizemo.focus-sync/` 的目录与打包思路。
- 能力对标 Files：`init` / `clone` / `fetch` / `pull` / `push` / `sync`、仓库浏览、
  **在 IDE 打开**、**长操作进度上报**、**失败给真实错误而非通用超时**。
- 与批次 1–3 的关系：扩展命令**必须走动作注册表**（否则又变成第四套入口）。
  ⇒ **批次 7 排在批次 3 之后**，这是硬顺序。

---

## §5 本阶段不做的（明确排除，避免摊薄）

- 云盘作为独立卷（成本高，且 AFM 已有远程服务器能力线）
- 磁盘格式化 / 批量清空回收站（高危不可逆）
- 创建 ADS（备用数据流）—— 若做，放扩展
- 引入 WinUI / 改用 C#（技术栈与跨平台目标冲突）
- 学 Files「不建全盘索引」—— AFM 的全盘搜索与 MCP 搜索工具是对外承诺，砍掉即砍卖点

---

## §6 与既有任务的边界

| 任务 | 状态 | 本阶段处置 |
|---|---|---|
| E（winget 3.9.2 PR） | 等外部审核员 `Moderator-Approved`，Agent 代劳不了 | **保持等待**，不碰那个 PR 分支 |
| G（focus-sync settings 腰斩） | 需用户确认同步状态 | 本阶段未获确认前不动 `user-extensions.json` |
| 任务 3 查询侧 | 已定案 21 ms，不是瓶颈 | **不改查询侧** |

---

## §7 接力用的一句话 prompt

```
按 docs/plan-2026-11-02-action-registry-and-incremental-index.md 执行批次 1：
在 src/core/actions/ 建立动作注册表内核，并让命令面板改为读注册表。
先写单测拿到 RED，再实现拿 GREEN；门禁 = npm run ts + 改动文件 eslint + 相关单测全绿。
不要动 docs/handoff-2026-11-01-relay-four-tasks.md 与 docs/winget-cla-signing-guide.md
（用户自己的未提交改动）。
```
