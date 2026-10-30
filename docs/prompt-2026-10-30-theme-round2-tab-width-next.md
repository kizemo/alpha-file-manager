# 接力 prompt · AFM 主题第二轮细化 + 标签自适应宽度（2026-10-30）

新开会话时，把下面「主 prompt」整段发过去即可。

---

## 主 prompt（推荐）

先读这几份文档建立背景——`F:\soft\00selfmade\filemanager\AGENTS.md`（本项目全部硬约束，末尾被运行时截断，需要时直接 read 源文件）、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-10-30-theme-round2-tab-width.md`（**本轮起点**：4 组任务清单、已探明的落点行号、门禁基线、建议施工顺序、失败路径清单）、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-10-30-brand-themes-and-toolbar-colors.md`（上一棒 3.7.0 的实现与真机取证细节，是本轮的改动基线）、`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\theme-round2-2026-10-30\ANCHORS.md`（改动前的行号锚点，动手前先读它而不是重读源码）；然后加载 `/huashu-design` 技能并按它的规范给本轮配色/宽度方案出三个方向的初稿供我选定，**在我拍板前不要改任何源码、不要跑构建或打包命令**；读完先用 10 行以内复述你掌握的项目背景、已锁定的设计原则、当前进度（3.7.0 已发布并装机）与本轮 4 组任务的落点，然后停下等我指令。

---

## 短版 prompt（上下文紧张时用）

先读 `F:\soft\00selfmade\filemanager\AGENTS.md` 与 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-10-30-theme-round2-tab-width.md`（后者第 1 节已列出其余必读文档与侦察底稿的完整路径），再加载 `/huashu-design`；读完复述背景、进度与本轮 4 组任务的落点后停下等我指令，**不要动手**。

---

## 两版的设计说明

| 用户要求 | 怎么落实 |
|---|---|
| 引导阅读必要文档 | 主 prompt 点名 4 份；短版只点 2 份，其余靠 handoff 第 1 节的「必读顺序」表引导 |
| 充分掌握背景/目标/进度/下阶段计划 | 背景与原则在 `AGENTS.md` + `handoff-2026-10-30-brand-themes-and-toolbar-colors.md`；目标与下阶段计划在 `handoff-2026-10-30-theme-round2-tab-width.md` 第 2（任务）、3（落点）、5（施工顺序）、9（交付标准）节；进度快照在该文档第 0 节 |
| **不重复 handoff 的具体内容** | prompt 只给**路径 + 该读什么**，4 组任务的细节、行号、陷阱、门禁数字全部在文档里，读完由新会话自己复述 |
| 文档提供完整路径 | 全部用绝对路径 |
| 调用 `/huashu-design` 保证更美观 | 两版都显式要求加载该技能，并按其「任何新设计先出三个方向初稿给用户选」的规范先出稿 |
| 读完不要执行动作、等指令 | 两版都写了「不要改源码 / 不要跑构建或打包 / 等我指令」 |

---

## 提醒新会话注意的 4 个点

1. **代码在内层仓** `F:\soft\00selfmade\filemanager\sigma-file-manager`（remote `kizemo/alpha-file-manager`）；
   外层 `F:\soft\00selfmade\filemanager` 的 remote 是 `kizemo/focus-sync`，只放 `AGENTS.md` 与历史档，
   **不跟踪内层代码**（`.gitignore` 里明确忽略 `sigma-file-manager/`）。提交、推送、打 tag 一律在内层仓。
2. **`edit` 工具改 `src/` 会「报成功但磁盘不变」**，`read`/`grep` 工具读 `src/` 会返回**过期视图**。
   改源码只能靠 node `.cjs` 脚本（`fs.readFileSync`/`writeFileSync(..., 'utf8')`）；
   读源码用 pwsh `Get-Content` 或 node dump。改完必须独立复核（字节数 / `git diff --stat`）。
3. **真机验收只能用 `tauri build` 产物**，`tauri dev` 的路由页内容区全空白，在它上面的界面结论一律作废。
   需要装机实测就直接打包，不等用户开口；`tauri build` 很慢，判断出要验就早启动、丢后台跑，
   后台跑时**不要挂 stdout 管道**（会让进程树跟着死），改用 `.cmd` 内部 `> logfile 2>&1`。
4. **有两处必须先出方案给我拍板再动手**：① `/huashu-design` 的三个设计方向选哪个；
   ② 任务 3 标签宽度的 `max-width` 取值与「按组内标签数计宽」的现有逻辑怎么协调
   （`tab.vue:248` 现在是 `tabGroup.length === 2 ? 200px : 100px`）。别自己拍。
