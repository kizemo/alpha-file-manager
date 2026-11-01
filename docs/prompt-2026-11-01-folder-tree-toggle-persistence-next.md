# 接力 prompt · AFM 目录树开关持久化修复验收 + 下一阶段（2026-11-01）

新开会话时，把下面「主 prompt」整段发过去即可。

---

## 主 prompt（推荐）

先按顺序读这几份文档建立背景——`F:\soft\00selfmade\filemanager\AGENTS.md`（本项目全部硬约束与踩坑清单，运行时注入的版本末尾被截断，需要时直接 read 源文件）、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-folder-tree-toggle-persistence.md`（**本轮起点**：进度快照、已锁定结论、任务清单 T1–T4、门禁口径、已知踩坑、交付标准全在里面）、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\release-workflow.md`（发版六条命令与五处状态一致性）、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\version-info.md`（版本号三处同步规则）、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\release-notes-3.9.0.md` 与 `F:\soft\00selfmade\filemanager\sigma-file-manager\CHANGELOG.md` 前 25 行（升级说明与更新日志的格式模板）、`F:\soft\00selfmade\filemanager\sigma-file-manager\src\stores\storage\user-settings.ts`（缺陷根因现场）、`F:\soft\00selfmade\filemanager\sigma-file-manager\src\stores\schemas\schema-utils.ts`（允许键集合的派生逻辑）、`F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\pages\navigator.vue`（目录树开关的运行时读写，**只读参考**）；然后读完用 10 行以内复述你掌握的项目背景、本次目标、当前进度与下一步计划，**再开始动手**。

动手时的硬性前提：代码在内层仓 `F:\soft\00selfmade\filemanager\sigma-file-manager`（外层 `F:\soft\00selfmade\filemanager` 的 remote 是 `kizemo/focus-sync`，**不要动**）；**真机验收只能用 `tauri build` 的打包产物**、不能用 `tauri dev`（dev 构建路由页内容区全空白，在它上面的界面结论一律作废），需要装机实测就直接打包、不等用户开口，打包很慢就早启动丢后台跑且**不要挂 stdout 管道**；**验收前必须备份真实用户数据 → 灌夹具 → 验收 → 恢复**，恢复顺序是「**先杀进程 → 再 restore**」，恢复后核对 sha256 与字节数、别把夹具留给用户；**允许键集合是从默认设置对象派生的**，任何新设置项必须先进默认对象，否则启动加载时被静默丢弃（写侧不校验，所以文件里有值也读不回来）；**不要**用 `--bump none` 重发 3.9.0（线上已有 `v3.9.0`，覆盖同版本会把 sha256 与更新提示搞乱），下一版走 3.9.1。

---

## 短版 prompt（上下文紧张时用）

先读 `F:\soft\00selfmade\filemanager\AGENTS.md` 与 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-folder-tree-toggle-persistence.md`（后者第 2 节已列出其余必读文档的完整路径），读完复述背景、目标、进度与下一步计划后再动手；真机验收只能用打包产物不能用 `tauri dev`；验收必须「备份真实数据 → 灌夹具 → 验收 → 恢复」，且先杀进程再 restore、恢复后核对 sha256；代码只动内层仓；下一版是 3.9.1，不能用 `--bump none` 重发 3.9.0。

---

## 两版的设计说明

| 用户要求 | 怎么落实 |
|---|---|
| 引导阅读必要文档 | 主 prompt 点名 9 份（AGENTS.md + handoff + release-workflow + version-info + 升级说明/CHANGELOG 模板 + 三份根因源码）；短版只点 2 份，其余靠 handoff 第 2 节「必读顺序」表引导 |
| 充分掌握背景/目标/进度/下阶段计划 | 背景与铁律在 `AGENTS.md`；目标与作业落在 handoff 第 3 节（T1–T4）、已锁定结论与进度落在第 0/1 节、下阶段计划与交付标准落在第 6 节 |
| **不重复 handoff 的具体内容** | prompt 只给**路径 + 该读什么 + 四条不可违背的前提**；脚本清单、T1–T4 的完整步骤、门禁数字、踩坑清单全部留在 handoff 里，读完由新会话自己复述 |
| 文档要提供完整路径 | 全部绝对路径 |
| 读完不要立即乱动 | 两版都要求「先复述、再动手」，并把「内层仓 / 打包产物 / 备份恢复顺序 / 版本号」四个易错点提前钉死 |

---

## 提醒新会话注意的 5 个点

1. **代码在内层仓** `F:\soft\00selfmade\filemanager\sigma-file-manager`（remote `kizemo/alpha-file-manager`）；外层 `F:\soft\00selfmade\filemanager` 的 remote 是 `kizemo/focus-sync`，只放 `AGENTS.md` 与历史档，**不跟踪内层代码**。提交、推送、打 tag 一律在内层仓。
2. **本次修复只动了默认设置对象**（`src/stores/storage/user-settings.ts`），**没改** `src/modules/navigator/**` 的运行时逻辑、**没改** schema 版本号 ⇒ 单测能证明「加载路径不再丢弃该键」，但**证明不了**「用户点一下开关、重启后仍然生效」⇒ **真机验收是必做项，不是可选项**，且要带**修复前的阴性对照**。
3. **winget PR #467381 现在只等社区版主 `Moderator-Approved`**，不需要用户做任何操作（CLA 已达成，标签 `Needs-CLA` 是残留）。若往该分支推新 commit 导致校验重跑，可能又要 CLA —— **那种情况必须让账号本人执行**，agent 不得代选立场。
4. **有三件事需要用户拍板**（其余按 handoff 自行判断，不用问）：① 目录树修复验收通过后是否发 **3.9.1**；② 是否顺带把 winget 清单补到 3.9.1（现存到 3.6.5）；③ 是否同步 upstream（AFM 与 upstream 已深度分叉，merge base `fed49cee`，判断口径见 `AGENTS.md` §2.2）。
5. **不要用 `tauri dev` 做任何界面结论**，也不要为了省事在 dev 构建上截图；**不要**在没验收通过时把修复推上 `origin/main`。
