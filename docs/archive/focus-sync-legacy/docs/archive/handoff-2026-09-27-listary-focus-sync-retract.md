# Handoff — Sigma Fork: Dialog Focus Sync retracted, Listary scope plan

**会话日期**: 2026-09-27
**承接**: `handoff.md` (v6.4.1 时代的整体快照) + 多个 tree sync + dialog focus sync handoff
**本次状态**: Dialog focus sync scope 错配确认并撤回;v2.2.0-focus.1 release 删除;README "Coming soon" 段删除;issue #547 礼貌关闭。下一步规划 Listary 风格全局系统 hook feature。
**下次第一句话**: "从 handoff-2026-09-27-listary-focus-sync-retract.md 接力,开始 brainstorming Listary 全局 hook feature 的设计"

---

## 一、当前已确定的事实(不要再讨论)

### 已经 ship 的功能(可用)

| 功能 | Release | Branch | 状态 |
|---|---|---|---|
| 目录树侧边栏(Folder Tree Sidebar) | `v2.2.0-tree.1` | `feat/tree-sidebar-v6-1` (HEAD `8caf14ae`) | ✅ 工作正常,装机验证通过 |
| 6 个 fork commit(259 unit tests pass)| | 同上 | |

**这才是 fork 真正能用的部分。**

### 已经撤回(不要重新引入)

| 项目 | 状态 | 原因 |
|---|---|---|
| Release `v2.2.0-focus.1` (Dialog Focus Sync) | 🗑️ 已删除 | scope 错配 — 没用户能触发的 UI 入口 |
| `feat/dialog-focus-sync` 分支的 11 个 commit | 保留(代码作参考,不做 release)| 见下方 "scope 错配详细" |
| README.md / README.zh-CN.md "Coming soon: dialog focus sync" | 🗑️ 已删除(commit `049dc9c1` 已 push 到 origin/main) | 同上 |
| 上游 issue #547 (Dialog Focus Sync) | 🗑️ 已关闭(comment 解释撤回原因)| 同上 |

### 关键 commit 列表(meta 仓库)

| Commit | 内容 |
|---|---|
| `7c20be1` | docs: open upstream issue #547 (现已 close)|
| `049dc9c1` (fork repo origin/main) | docs(readme): retract misleading dialog focus sync coming-soon note |
| (fork repo,release 撤回)| `gh release delete v2.2.0-focus.1 --repo kizemo/sigma-file-manager --yes` |

---

## 二、scope 错配详细(必读,避免重蹈覆辙)

**核心教训**:fork 之前,先验证 scope 是否**对普通用户实际可触发**。

### 我们的实现做了什么

12 个 commit 实现的 `feat/dialog-focus-sync`:

- Rust:`SigmaPicker` (IFileDialog COM wrapper) + `commands_picker.rs` (3 个 Tauri commands:`picker_open` / `picker_set_folder` / `picker_close`) + dedicated STA worker thread (mpsc channel + oneshot reply)
- 前端:Pinia store `dialog-picker.ts` + `useCurrentNavigatorPath` composable + `navigator.vue` 的 `watch` + `onFocusChanged` listener
- Permissions:`picker_*` capabilities
- 测试:178 lib tests + 11 picker tests + 236 navigator Vitest tests 全绿

### scope 错在哪里

**用户期望**:Listary 风格 — 任何 Windows 文件对话框(浏览器下载、Office 另存为、VS Code 打开)切到 Sigma 再切回,自动跳到 Sigma 当前目录。

**实际实现**:只 work 当 **Sigma 自己通过 `picker_open` Tauri command 弹出 picker dialog**。

但 **Sigma UI 没有任何按钮触发 `picker_open`**。Sigma 是文件浏览器,不是文件选择器。普通用户日常使用 Sigma 时**根本不会触发 picker**。

用户测试场景:浏览器下载 → Chrome 自己用 IFileDialog 弹"另存为" → 我们的 hook **完全无效**(那是 Chrome 进程的 dialog,跟我们无关)。

### 为什么走错路

我(控制器 agent)在 brainstorming 阶段讨论过"全局 hook" vs "只适配 Sigma",但没讲清楚:
- ✅ 全局 hook = Listary,需要 DLL 注入 + COM 拦截,4-8 周工作量,商业软件级复杂度
- ⚠️ 只适配 Sigma 我说是"中等难度",但 **没讲"普通用户碰不到"**

技术评估本身没错,但 **没传达 scope 局限**给用户,导致用户期望 Listary 体验。

---

## 三、下一步目标:真正的 Listary-style 全局 hook

### 用户意图(2026-09-27 明确确认)

> "我还是希望实现listary这个焦点切换的功能。"
> "其他软件下载、上传文件,只要是文件选择窗口,都应该在切换sigma窗口后,自动跳转路径。"

### 功能定义

| 触发条件 | 行为 |
|---|---|
| 用户在 Sigma File Manager 打开,Sigma 主窗口有焦点 | Sigma 后台 watch 当前 navigator 路径变化 |
| 任何 Windows 应用弹出文件对话框(Chrome 另存为、Office 打开、VS Code 等)| 全局 hook 检测到该对话框获得焦点 |
| 用户 Alt-Tab 离开该对话框切到 Sigma | hook 捕获焦点切换 + Sigma 当前路径 |
| 用户在 Sigma 主窗口浏览到新路径 | Sigma 把新路径 emit 给 hook |
| 用户 Alt-Tab 回对话框 | hook 调 `IFileDialog::SetFolder(newPath)` 让对话框跳到新路径 |

### 技术路径(已调研,见 handoff-2026-09-25-tree-sync-retro 类似的 brainstorm 输出)

#### 方案 A:UI Automation (推荐先做 spike)

- 用 `UIAutomationCore.dll` 的 `IUIAutomation` 接口
- 检测当前 foreground 窗口的 dialog 元素
- 读取 file dialog 的"地址栏" / "文件名列"path 通过 `ValuePattern`
- 写入用 `InvokePattern` 或 `ValuePattern::SetValue`
- **优点**:不需要 DLL 注入,杀软友好,Windows 官方 API
- **缺点**:部分 app 禁 UIA(legacy `GetOpenFileName` 不支持),慢(每 N ms 轮询)
- **工作量**:2-3 周 spike,4-6 周完整实现
- **覆盖**:Chrome、Edge、Office、VS Code 大概率 work;Total Commander、Directory Opus 部分支持;老的 `GetOpenFileName` 不支持

#### 方案 B:DLL 注入 + IFileDialog COM 拦截

- 用 `SetWindowsHookEx` + `CBTProc` 或 `WH_SHELL` hook
- 注入到 explorer.exe 和所有启动 IFileDialog 的进程
- Hook `IFileDialogEvents::Advise` 监听 OnFolderChange
- IPC(命名管道 / 共享内存)从 hook DLL 到 Sigma 进程读当前路径
- **优点**:真正的 Listary 体验,瞬时响应
- **缺点**:**被几乎所有杀软标记为可疑**(常见恶意软件 pattern)。代码签名证书必须 EV。Windows Defender Application Control (WDAC) 会拒绝
- **工作量**:3-4 周 spike(反向工程保护),6-10 周完整实现
- **覆盖**:几乎所有 IFileDialog 实例(Chrome、Office、VS Code、Total Commander 等)

#### 方案 C:Hybrid(短期可交付,长期不完美)

- 优先方案 A(UI Automation)读
- SetFolder 用 SendInput 模拟键盘输入到地址栏(`Ctrl+L` 输入 path + `Enter`)
- **优点**:不需要 IFileDialog COM 实例引用,所有 dialog 通用
- **缺点**:hack,某些 dialog 没法 SendInput(被禁用),慢
- **工作量**:2 周实现
- **覆盖**:大部分 modern dialog

### 推荐路径

**先做 1 周方案 A 的 spike**:
- prototype UI Automation 检测 + 读取 path + 设置 path
- target app matrix:Chrome download、Word save-as、VS Code open、Total Commander、Photos app
- 记录覆盖率(>70% 才考虑进入完整实现,否则 escalate 到方案 B)

如果方案 A spike 失败或覆盖不够:
- 评估方案 C 短期交付
- 或者承认 Listary 是商业软件的护城河,fork 不能做

---

## 四、下一会话的明确动作清单

按这个顺序执行(用户已批准方案 3 选项 = 实现 Listary 风格,但要从 spike 开始):

1. **brainstorming**:用 `superpowers:brainstorming` skill,确认 spike 边界(target app matrix、UI Automation 选型、IPC pattern)
2. **write spec**:创建 `docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md`,写明 spike 的 acceptance criteria
3. **write plan**:创建 `docs/superpowers/plans/2026-09-27-listary-focus-sync.md`,把 spike 拆成可执行 task
4. **execute spike**:跑 spike,记录 target app matrix 覆盖率
5. **decide**:基于 spike 结果,选择方案 A / B / C 进入完整实现,或决定不做了

**绝对不要**:
- 跳过 spike 直接开始完整实现(4-6 周打水漂风险)
- 跳过 brainstorming 直接写 spec(scope 没对齐就写设计会重蹈覆辙)
- 假设"简单 Listary clone = 简单"(它是商业软件 10+ 年的护城河)

---

## 五、必读文件(按顺序)

1. **本 handoff**(你正在读)
2. `F:/soft/00selfmade/filemanager/handoff.md` — fork 整体状态快照(v6.4.1 era)
3. `F:/soft/00selfmade/filemanager/docs/superpowers/specs/2026-09-26-upstream-pr-strategy-design.md` — PR 沟通策略(对上游维护者的态度模板仍然适用)
4. `F:/soft/00selfmade/filemanager/docs/superpowers/plans/2026-09-26-dialog-focus-sync.md` — 上次 plan,**作为反例参考**(展示如何"看着完成但实际 scope 错配"),不是模板
5. `F:/soft/00selfmade/filemanager/sigma-file-manager/` 仓库的 `feat/dialog-focus-sync` 分支(11 commits)— 代码作为方案 A/B 的 reference,但**不要 release**
6. `F:/soft/00selfmade/filemanager/handoff-2026-09-25-tree-sync-retro.md` — tree sync 失败的复盘(类似的 scope 教训)

---

## 六、避坑提示(从 dialog focus sync 1.0 学到的)

1. **永远先验证 scope 是否对普通用户实际可触发** — 如果新功能没 UI 入口,做出来是死代码
2. **永远不要做 "Listary 替代品" 这种 scope 假设** — 商业软件 10+ 年的护城河,先 spike 再决定
3. **Tauri 2.x strict ACL 不要碰** — 任何在 `src-tauri/permissions/` 加手工 TOML 都会启用 strict mode,所有 `#[tauri::command]` 都需要 explicit allow。已踩过坑(`c1f51d29` fix)。如果将来真的需要权限文件,先**完整枚举**所有 user commands 再加 TOML
4. **不要 ship 没用户能触发的功能** — 即使代码完美,没 UI 入口就是 dead code
5. **fork-self-publish + PR upstream 同时进行是矛盾的** — 选定一个立场,不要混

---

## 七、给后续 session 的明确 instructions(写在 prompt 里,见 prompt-next-listary-focus-sync.md)

下一会话接力时:
- 先读本 handoff(理解 scope 错配教训)
- 再读 prompt-next-listary-focus-sync.md(看到一句话 prompt)
- 第一句话引用 prompt 文件,不要复述 handoff 内容
- 动作链:brainstorming → spec → plan → spike → decide
- spike 必须先做(1 周内完成 target app matrix 验证)
- spike 结果不通过,**不要承诺完整实现**

---

## 八、out of scope(已经在 1.0 撤回,不要重做)

- `feat/dialog-focus-sync` 分支的 11 个 commit(代码可以参考,但**不要**直接扩展 / cherry-pick / release)
  - `picker_open` / `picker_set_folder` / `picker_close` commands(reusable 作为 Sigma 内部 picker API,但需要新 UI 入口才有意义)
  - Pinia store `dialog-picker.ts`(同上)
  - `useCurrentNavigatorPath` composable(可能 reuse,但 navigator path 已经有了 equivalent)
  - SigmaPicke r Rust class(reusable 作为 future Sigma 内部 picker 实现)
  - dedicated STA worker thread + mpsc channel pattern(标准 Rust 模式,可参考但不需要 reuse)
- `docs/superpowers/plans/2026-09-26-dialog-focus-sync.md` — 作为反例存档,**不要按它做新 plan**

---

## 九、tree 功能的当前状态(继续可用,不要动)

- `feat/tree-sidebar-v6-1` 分支 HEAD `8caf14ae`,6 commits,259 unit tests pass
- Release `v2.2.0-tree.1` 仍在 GitHub(`Sigma.File.Manager_2.2.0_x64-setup.exe`,sha256 `c63ef919...`)
- 本地 `F:\soft\00selfmade\filemanager\sigma-file-manager\release\Sigma-File-Manager-2.2.0-tree.1-x64-setup.exe` 仍是有效 build
- README origin/main 的"kizemo fork additions" 段仍正确描述 tree feature
- README 已撤回 "Coming soon: dialog focus sync" 段(不误导)

**不要 rebase 或 squash** `feat/tree-sidebar-v6-1` 分支 — 它是已发布的稳定 feature。

---

## 十、tree + dialog feature 已完成工作的 git history 速查

### Fork repo branch states(2026-09-27)

```
main                              → README 撤回 dialog focus sync 段 (049dc9c1),剩下 tree feature
feat/tree-sidebar-v6-1            → 6 commits,HEAD 8caf14ae,稳定
feat/dialog-focus-sync            → 12 commits,HEAD c1f51d29(撤回,代码保留,future reference)
```

### Local files
```
F:\soft\00selfmade\filemanager\sigma-file-manager\release\
├── Sigma-File-Manager-2.2.0-tree.1-x64-setup.exe (15.14 MB, sha256 c63ef919...)
├── SHA256SUMS.txt
└── README.md
```

(v2.2.0-focus.1 binary 已被删除,本地 release 目录只剩 tree 版本的 binary)

---

## 十一、监控脚本现状

`scripts/monitor-issue-499.ps1` 和 `scripts/monitor-issue-499.sh` 仍在 meta repo,可用。
需要监控 issue #547 的话,可以复制改 ID(#547 已关闭所以暂时不需要)。