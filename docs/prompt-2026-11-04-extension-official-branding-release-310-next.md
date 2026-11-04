# 一句话 prompt · 接力 3.10.0 发版收尾

> 本文件是给**新会话**的第一条输入。把下面「★ 直接复制这段发给新会话 ★」整段粘过去即可。
> 本文件刻意**不复述**交接文档内容 —— 背景/目标/进度/计划全在文档里，避免两处口径漂移。

---

## ★ 直接复制这段发给新会话 ★

请先阅读 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md`
这份交接文档（2026-11-05 00:20 回填版，已反映 v3.10.0 发版后的真实状态），掌握项目背景、目标、进度与下阶段计划后再动手；
并按需阅读 `F:\soft\00selfmade\filemanager\AGENTS.md`（本仓 Agent 约定与真机验收硬要点）。
本次要接的具体活儿见该文档 §八「下阶段任务」，按编号从 0 开始做。

---

## 为什么是这几个路径（给人类看的备注，不必发给新会话）

| 路径 | 作用 |
|---|---|
| `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-04-extension-official-featured-branding-and-release-310.md` | **主文档**。§一背景 / §二目标 / §三进度 / §四已落地改动 / §五门禁与打包真实结果 / §六发版步骤与命令 / §七剩余三项 / §八下阶段任务清单 / §九证据路径 / §十交付物与自检 |
| `F:\soft\00selfmade\filemanager\AGENTS.md` | 仓库约定 + 真机验收的全部坑（DPI、UIA、注入、门禁基线口径）；改代码前必读 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\logs\upload310.txt` | 站点上传日志：结尾出现 `UPLOAD_310_EXIT=0` 才算传完，随后才能跑 `npm run release:verify` |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\CHANGELOG.md` | 3.10.0 升级说明（发版正文已同步到站点 `_release-notes-v3.10.0.md`） |

## 新会话开局会看到的既成事实（写在 handoff 里，此处仅提醒别重复劳动）

- 代码改动、门禁、打包、发版提交 `18a349e7`、`main` 推送、tag `v3.10.0` —— **全部已完成**，三条必查全绿。
- 站点 `version.json` + `index.html` + `.htaccess` + `download.php` + `Alpha_File_Manager_3.10.0_x64-setup.exe` —— **已上传 OK**。
- 剩余活儿只有三件：**建 GitHub Release**、**线上 `verify`**、**Focus Sync 公网仓库 + 精选条目 + 真机验收**。
- 本机装着 v3.9.2，**它是当前唯一能用的回滚包**；在打出 3.10.0 的可分发 zip 之前别删 `release/` 与 `src-tauri/target/release/bundle/nsis/` 下的产物。
