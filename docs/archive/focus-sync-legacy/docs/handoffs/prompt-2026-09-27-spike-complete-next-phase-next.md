# Next Prompt — Integration Phase After Spike PASS

承接 `handoff-2026-09-27-spike-complete-next-phase.md`。当前 HEAD `905db31`,spike 决策 PASS,11 任务全部完成。

**下一步必交付**:
1. **README apply**:`sigma-file-manager/README.md` + `README.zh-CN.md` 的"Listary-style focus sync (Experimental)"段已写入文件,但 `sigma-file-manager/` 在本 meta-repo `.gitignore` 里,需要切换到那个 fork repo 单独 commit + push
2. **写 Phase 2 集成 spec**:`docs/superpowers/specs/2026-09-XX-listary-focus-sync-integration-design.md`(Sigma Tauri command + spike.exe daemon spawn + named-pipe IPC)
3. **(用户机器前置)** 7-app GUI 全验证:本机只装 4 个 app,需补 Word / VS Code / WeChat 后跑 `sigma-listary-spike/scripts/verify_target_matrix.ps1 -SpikePath <path> -Manual`

**禁止事项**:
- 不要开 `feat/dialog-focus-sync` 分支
- 不要 rebase `feat/tree-sidebar-v6-1`
- 不要走 14-task 自写 spike 路线(已废)
- 不要改 spike 已 commit 的 3 个移植文件(版权头固定)
- 不要把 spike binary 推到 upstream `kizemo/sigma-file-manager`

**加速提示**:spike binary 已构建并实测通过,集成 spec 是新写文档(不用动 spike 代码);README 改动只差一次 fork repo commit + push。