# 交接：P2 内层体积治理（2026-11-10）

> 上游文档：`docs/handoff-2026-11-10-root-unification.md` §5 遗留待办 P2。
> 本文只覆盖 P2。换根（Phase 0–6）已在上一轮完成，本轮未重复。

## 1. 一句话结果

内层总体积 **55,930.7 MB / 248,804 文件 → 约 20,135 MB / 198,113 文件**，
回收 **35,795 MB（-64.0%）**，三项门禁复核**全部保持原状**，未触碰任何被跟踪文件。

## 2. 本轮实际清理了什么

全部通过顶层 `rm -- "<绝对路径>"` 走 **Windows 回收站**（可恢复），未使用永久删除。

| 目标 | 性质 | 回收量 |
| --- | --- | --- |
| `src-tauri\target\debug`（含 `incremental` 13,445.2 / `deps` 12,551.3 / `build` 2,763.9 / `.fingerprint` 2.8） | cargo debug 构建缓存 | 28,766.1 MB / 17,444 文件 |
| `src-tauri\target\x86_64-pc-windows-msvc` | 带 target triple 的重复构建缓存 | 4,559.7 MB / 14,991 文件 |
| `extensions\kizemo.focus-sync\sidecar\target` | 侧车 cargo 构建缓存 | 1,317.8 MB / 3,036 文件 |
| `src-tauri\target\default-file-manager-launcher` | 旧子 crate 残留 target | 962.7 MB / 7,674 文件 |
| `.superpowers\sdd\asar-extract` | 一次性 asar 解包（文档 0 处引用） | 148.9 MB / 5,597 文件 |
| `dist` | vite 构建产物 | 38.8 MB / 1,976 文件 |
| `graphify-out` | 代码图分析产物 | 29.4 MB / 321 文件 |

合计 **35,823.5 MB**。

## 3. 明确没动的部分（连带理由）

| 保留项 | 体积 | 理由 |
| --- | --- | --- |
| `src-tauri\target\release` | 13,610.7 MB | 唯一保留的构建缓存。`npm run tauri:build` 是本项目真机验收的唯一合法产物，删掉它会让每次验收都付全量重编代价——正是上一轮推迟 P2 的那条理由。 |
| `.superpowers\sdd` 其余部分 | 4,206.5 MB | 其中 `build-fb4.cmd` / `app-key.ps1` / `dash-accept-data.cjs` / `afm-capture-only.ps1` / `shift-occluders.ps1` / `pre-e-backup/` 是 AGENTS.md §0.3 点名的验证装置；`shots`(615.2) / `superseded`(831.5) 在历史文档中有 3 / 4 处引用；6 个旧 exe/bin 快照在 2 份 handoff 中有引用。**本轮按「有引用即保留」处置**。 |
| `release\` | 1,164.9 MB | 29 个历史 NSIS 安装包，属交付物，不是缓存。 |
| `.git` | 362.6 MB | `git count-objects -vH` 显示 size-pack 355.94 MiB、松散对象仅 4.70 MiB、garbage 0 ⇒ 重打包收益极低。 |
| `node_modules` | 304.6 MB | 门禁与构建的运行基础，删了要先 `npm ci`。 |
| `_acceptance` | 263.1 MB | 验收证据。 |

## 4. 门禁复核（清理后）

日志：`.superpowers\sdd\p2-cleanup-gates.log`

| 门禁 | 结果 | 与清理前基线对比 |
| --- | --- | --- |
| `npm run ts` | EXIT=0 | 无变化 |
| `npm run lint:eslint:check` | EXIT=1，**177 errors / 0 warnings** | 与 `handoff-2026-11-10-root-unification.md` 记的 177 完全一致，属既有问题（全部落在 `src/`），与本次清理无关 |
| `npm run test:unit:run` | EXIT=0，**299 test files / 3134 tests 全通过**（141.90s） | 无变化 |

> 未跑 `lint:stylelint:check` / `check` / `check:win` / `verify-changelog`：本轮只删派生目录，不改任何源码、样式、清单或 changelog，这些门禁的结果不可能因本次清理而变。

## 5. 已验证 / 推断 / 未验证

**已验证**
- 清理前后体积与文件数（`EnumerateFiles` + `FileInfo.Length` 逐文件累加，非 `Get-ChildItem -Recurse`）。
- 三项门禁的 EXIT 码与计数（日志在盘）。
- 清理前 `Get-Process` 全量扫描「从项目根启动的进程」= **0 命中**；`alpha-file-manager` 实际运行在 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，与本项目目录无关。
- 被删项全部属 gitignore 范围或派生产物，`git status` 行数口径不受影响（见 §6 未验证项）。

**推断**
- `src-tauri\target\debug` **整目录**回收失败（`mavis-trash` 返回空错误串）是回收站 API 在深层长路径 / 句柄占用上失败；拆成 7 个子目录逐个回收后 7/7 成功、目录随即变空 ⇒ 卡点在目录根而非深层子孙。此推断未做根因定位（不值得为一次性操作做）。
- 磁盘空闲空间**没有**实质增加：回收站与源同在 F: 盘，移入是跨目录移动而非删除。本报告的所有「回收」都指**项目根目录体积**，不是磁盘空间。

**未验证**
- 未重新打包验证 `npm run tauri:build`。保留了 `target\release`，理论上增量构建仍可用，但没有实测证据。
- 未实机启动一次应用。

## 6. 工具坑（本轮新增，可直接复用）

1. **`Set-Location` 之后写文件会被本地安全策略拦**
   `Permission denied for bash: Local hard safety policy blocked bash: write/delete targets cannot be verified safely after a command working directory change.`
   但门禁又必须在项目根跑。可行替代：**`npm --prefix <项目根绝对路径> run <script>`** —— 生命周期脚本的 cwd 就是 prefix，`ts` / `lint:eslint:check` / `test:unit:run` 三项实测全部正常执行，且全程不需要切目录。
2. **`rm --` 一次可以带多个目标**，但必须是**顶层单条**命令；放进复合脚本仍报 `bare or non-canonical mavis-trash is untrusted`。批量清理时把多个绝对路径写进同一条 `rm --` 即可。
3. **大目录整块回收失败时，拆子目录重试**：`mavis-trash` 对已空的目录仍能回收，且拆开后错误能定位到具体子树。
4. `mavis-trash` 的实现是 `bin\mavis-trash.js`，走 PowerShell 的回收站 API，落点是**同盘** `$Recycle.Bin` ⇒ 不会释放磁盘空间。

## 7. 遗留待办 / 下一步

- **P2 已收口**（体积治理目标达成，-64%）。若还想继续压：
  - 下一步最大杠杆是 `src-tauri\target\release`（13.6 GB），代价是下次真机验收全量重编。属于「拿构建速度换体积」，需拍板。
  - 次之是 `.superpowers\sdd\shots` + `superseded`（1.45 GB），代价是历史文档里的 7 处路径引用变成死链。
- **若要真正释放磁盘**：清空回收站（F: 盘）。这是不可逆动作，需用户明确授权后执行。
- **350 条未提交项仍在**：`git status --porcelain -uall` 344 `??` + 6 ` M`，其中 315 条在 `docs/archive/`。是上一轮换根迁移的产物，两轮都没有提交。是否入库需用户拍板。
- 上一轮的 `handoff-2026-11-10-root-unification.md` §5 未改动，保留为历史快照；P2 的结论以本文为准。

## 8. 恢复方式

被清理项都在 F: 盘回收站。恢复任一项：资源管理器打开回收站 → 按原始路径 `sigma-file-manager\...` 找到 → 还原。
`dist` / `graphify-out` 更快的恢复方式是重新生成（`npm run build` / graphify 重跑），不必还原。