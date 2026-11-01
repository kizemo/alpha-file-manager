# 接力 prompt · AFM 发版 3.9.0 + UI 改动介绍与重截图（2026-11-01）

新开会话时，把下面「主 prompt」整段发过去即可。

---

## 主 prompt（推荐）

先按顺序读这几份文档建立背景——`F:\soft\00selfmade\filemanager\AGENTS.md`（本项目全部硬约束，运行时注入的版本末尾被截断，需要时直接 read 源文件）、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-release-3.9.0-and-ui-screenshots.md`（**本轮起点**：进度快照、任务清单 T1–T10、版本号决策、门禁口径、已知踩坑、交付标准全在里面）、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\release-workflow.md`（发版五条命令与五处状态一致性）、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\release-notes-3.8.0.md` 与 `F:\soft\00selfmade\filemanager\sigma-file-manager\CHANGELOG.md` 前 25 行（升级说明与更新日志的格式模板）；然后读完用 10 行以内复述你掌握的项目背景、本次目标、当前进度与下一步计划，**再开始动手**。

动手时的硬性前提：版本号必须是 **3.9.0**（不是 3.8.0，因为 3.8.0 已对外发布且线上包与本地不同——证据在 handoff 第 0.1 节）；发版六条命令的**顺序不能乱**（`verify-changelog` 必须早于升版本号，`--write-notes` 不能省，否则会安静地发出正文为空的 GitHub release）；截图**只能用 `tauri build` 的打包产物**、不能用 `tauri dev`（dev 构建路由页内容区全空白）；README 用英文界面截图、`README.zh-CN.md` 用中文界面截图，且现有 `afm-main-window.png` 被两个 README 共用、必须拆成 `-en` / `-zh` 两套。

---

## 短版 prompt（上下文紧张时用）

先读 `F:\soft\00selfmade\filemanager\AGENTS.md` 与 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-release-3.9.0-and-ui-screenshots.md`（后者第 2 节已列出其余必读文档的完整路径），读完复述背景、目标、进度与下一步计划后再动手；版本号必须是 3.9.0，发版六条命令顺序不能乱，截图只能用打包产物，README 与中文说明分别用英文 / 中文界面截图。

---

## 两版的设计说明

| 用户要求 | 怎么落实 |
|---|---|
| 引导阅读必要文档 | 主 prompt 点名 4 份（AGENTS.md + handoff + release-workflow + 升级说明/CHANGELOG 格式模板）；短版只点 2 份，其余靠 handoff 第 2 节「必读顺序」表引导 |
| 充分掌握背景/目标/进度/下阶段计划 | 背景与铁律在 `AGENTS.md`；目标、任务清单、进度快照、下一步计划分别落在 handoff 第 3（任务 T1–T10）、第 0 节（进度快照 + 版本号决策）、第 6 节（交付标准） |
| **不重复 handoff 的具体内容** | prompt 只给**路径 + 该读什么 + 三条不可违背的前提**；8 张截图的文件名清单、命令顺序的完整形态、踩坑清单、门禁数字全部在 handoff 里，读完由新会话自己复述 |
| 文档提供完整路径 | 全部绝对路径 |
| 读完不要立即乱动 | 两版都要求「先复述、再动手」，并把版本号/命令顺序/截图口径/语言分套四个易错点提前钉死 |

---

## 提醒新会话注意的 4 个点

1. **代码在内层仓** `F:\soft\00selfmade\filemanager\sigma-file-manager`（remote `kizemo/alpha-file-manager`）；外层 `F:\soft\00selfmade\filemanager` 的 remote 是 `kizemo/focus-sync`，只放 `AGENTS.md` 与历史档，**不跟踪内层代码**。提交、推送、打 tag 一律在内层仓。
2. **`release:prepare --bump minor` 会同时改 `package.json` 与 `src-tauri/Cargo.toml` 两处版本号**，两处必须一致；`verify-changelog` 要在 bump 之前跑，否则误报。
3. **真机验收只能用 `tauri build` 产物**，`tauri dev` 的路由页内容区全空白，在它上面的界面结论一律作废。需要装机实测就直接打包，不等用户开口；`tauri build` 很慢，判断出要验就早启动、丢后台跑，后台跑时**不要挂 stdout 管道**（会让进程树跟着死），改用 `.cmd` 内部 `> logfile 2>&1`。
4. **有两处必须先问用户再动手**：① 是否顺带补 **winget 清单**（现存在到 3.6.5，3.7.0 / 3.8.0 都还没补，用户本次没点名 winget）；② **版本号取 3.9.0（minor）** 是否认可 —— handoff 第 0.1 节给的是「必须 minor、绝不能用 `--bump none` 重发 3.8.0」的硬结论，如果新会话想改成 patch（3.8.1）可以，但要在对话里说明。其余按 handoff 自行判断，不用问。
