// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * 功能开关（feature flags）。
 *
 * **这里放「暂时不做 / 暂时不做出来」的功能，不放「实验性」功能。**
 * 开关关闭时：用户界面里完全不出现这个功能，但**代码本体保留在仓库里**，
 * 恢复只需要把常量翻回 `true`，不必重写实现。
 *
 * 每个关闭的开关都必须在 `docs/` 里有一份对应的待办文档，写清恢复步骤。
 */

/**
 * OneDrive 支持：暂时下线（2026-11-24 用户裁决「整条 OneDrive 线延期，先做别的任务」）。
 *
 * 关闭后的语义（**只关入口，不删数据**）：
 *
 * - 设置 → 远程 → 「添加网盘」里**不再出现** OneDrive 按钮；
 * - 连接表单里的 OneDrive 区块（应用（客户端）ID 输入框、「开始授权」按钮、授权状态）
 *   **不再渲染** ⇒ 旧连接无法再发起授权；
 * - 用户已经存下的 OneDrive 连接**仍然显示、仍然可以删除**（不丢用户数据、不崩）；
 * - 后端 `remote_onedrive_*` 系列命令**保留编译**（`src-tauri/src/remote/onedrive_*.rs` 原样不动），
 *   只是界面不再触发。
 *
 * ⚠️ **不要顺手把 `src-tauri/src/remote/onedrive_*.rs` 删掉** —— 恢复时要重跑真机验收，
 * 代码留着才能一键翻回来。
 *
 * 恢复步骤见：`docs/plan-2026-11-24-onedrive-deferral-resume.md`
 * （发布形态的账号/租户/应用注册口径另见 `docs/plan-2026-11-24-onedrive-deferral-resume.md`
 *  的附录与 `.superpowers/sdd/b41/` 下的四份底稿）。
 */
export const ONEDRIVE_ENABLED = false;
