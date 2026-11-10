# Prompt — next session (tree sync v6 — Pinia store refactor)

承接 `handoff-2026-09-26-tree-sync-v6.md` —— v6 build (sha256 `ef4269a2...`) 已出炉,代码已 commit 到 `feat/tree-view`。

**必交付**:用户装机测 v6,验证 5 项 sync 场景 + ancestor load 失败兜底。`release/Sigma-File-Manager-2.2.0-x64-setup.exe` 已覆盖到 `release/`。

**分支/基线**:`feat/tree-view` HEAD = v6 commit,基于 `ec3389ba`(v5 改动前的基线)。父仓库根目录记录 plan/handoff,sigma-file-manager 子模块是代码。

**禁止事项**:不要回退 v5 imperative ref forwarding、不要改 store Set mutation 用原地 set/delete、不要让 `useFileTree.ensureLoaded` throw、不要动 baseline type 补丁(它们配合 baseline 'tree' layout 漏的类型)。

**加速模式**:本会话已跑全套 vue-tsc + 255/255 navigator 测试 + tauri build + sha256 更新 + commit。无需重新跑,直接装机测。

**已知遗留**:见 handoff §九(Extension API 扩 tree / e2e 断言 / split-view per-pane store / baseline test isolation 1 fail)。
