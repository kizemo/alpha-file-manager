# prompt-focus-18-auto-confirm-next

接力 task:**focus-18 path activation + Auto-Confirm toggle 实现**

---

## 一句话提示(必读)

读 `handoff-2026-10-01-focus-18-path-activation-auto-confirm.md` 全文(11 节),然后按 `docs/superpowers/plans/2026-10-01-focus-18-path-b-auto-confirm-toggle-plan.md` 的 9 个 Task 顺序执行,完成后把 `release/Sigma-File-Manager-2.2.0-focus-18-setup.exe` 打包好交给用户做 7-app 矩阵手动测试,**不要 commit git**(等用户授权)。

## 关键约束(必读)

- **rustc 必须 1.90**(`/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin/`);stable/1.97 在这台机器损坏
- **每次 cargo 前** `export PATH="/c/Users/Duanyi/.rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin:$PATH"`
- **保留 focus-17 已修**: windows_subsystem / log 文件 / basename 函数 — focus-18 在它们之上叠加
- **fg_bypass.rs 从 `git show 905db31:sigma-listary-spike/src/fg_bypass.rs`** 恢复
- **NO commit during execution**, Task 8 才 batch commit(且仅在用户授权后)
- **完成判据 §7 11 项必须全过**

## 优先级

1. 读 handoff §3 必读顺序
2. 读 Mavis plan §0 TL;DR
3. 用 `superpowers:executing-plans` 技能开始 Task 1
4. 中途有疑问 ping Mavis 文档(或用户)