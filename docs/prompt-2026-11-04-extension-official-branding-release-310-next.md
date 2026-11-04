# 一句话接力 prompt（2026-11-05 · 3.10.0 发版 / 扩展官方化 / Focus Sync 上架）

**先完整读 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md`（背景、目标、已完成、判据、证据路径、工具坑全在里面，尤其 §六 验收清单、§八 剩余步骤、§十 本轮踩坑），再读 `F:\soft\00selfmade\filemanager\AGENTS.md` 的项目约定；本任务只剩 §八 第 10 步「装 3.10.0 后做真机界面验收」这一项，按 handoff 的路径和判据自己接手，不要从零重做已完成的事。**

## 必要文档的完整路径

| 用途 | 绝对路径 |
|---|---|
| **接力主文档**（背景/目标/进度/下阶段/证据/坑） | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md` |
| 项目硬约束与真机验收要点（必读） | `F:\soft\00selfmade\filemanager\AGENTS.md` |
| 更早批次的交接（如需追前史） | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\` 下其余 `handoff-*.md` |
| 待验收的安装包 | `F:\soft\00selfmade\filemanager\sigma-file-manager\release\Alpha File Manager_3.10.0_x64-setup.exe` |
| 真机验收可复用资产 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\`（`accept-uia-dump.ps1` / `accept-nav.ps1` / `accept-market-filter.ps1` / `app-key.ps1` / `dash-accept-data.cjs` / `shift-occluders.ps1`） |
| 扩展市场清单（已入仓、线上已生效） | `F:\soft\00selfmade\filemanager\sigma-file-manager\scripts\ext-market\registry.json` |
| 本轮取证日志 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\logs\fs-archive-hash.txt` |

## 读文档时要抓住的三件事

1. **发版已完成**（提交、tag、站点、GitHub Release、清单上架均已实测核验）——判据在 handoff §五 §七，别重跑。
2. **唯一剩余项是真机界面证据**——四项 + Focus Sync 精选可见性，清单见 handoff §六；前置条件与坑也在 §六，尤其「必须用打包产物、不能用 dev 构建」。
3. **结论一律标注「已验证 / 推断 / 未验证」**，取证口径照 handoff §十 与 `AGENTS.md` §2，不要凭 UI 显示或记忆下判断。
