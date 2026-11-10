# Handoff — Phase 2a 路线修正: 从 Tauri Sidecar 集成 改为 Sigma 插件

**会话日期**: 2026-09-28
**会话上下文**: 用户授权 README apply + 7-app GUI 验证 + 推进主线
**承接**: `handoff-2026-09-27-spike-complete-next-phase.md` (spike PASS at HEAD `905db31`)
**本次状态**: Phase 2a 路线从"Sigma Tauri sidecar 集成"修正为"Sigma 插件系统" (kizemo/focus-sync v0.2.0 已发布)

---

## 一、关键发现（路线修正）

### 1.1 之前描述是错的

- `docs/superpowers/specs/2026-09-27-listary-focus-sync-integration-design.md` (19 KB) 描述的是 **Tauri sidecar 集成**方式（Sigma 内 Rust commands + Pinia store + 打包进 NSIS）
- 实际 Phase 2a 的真正目标是 **Sigma 插件系统**（符合 `@sigma-file-manager/api` v1.0.0）
- 这份过时 spec 已移到 `docs/_archive/` (仍可参考作历史，但不再作为实施依据)

### 1.2 Sigma 插件系统确实存在（已验证）

| 证据 | 详情 |
|---|---|
| 上游 `packages/api/` | 官方扩展 API 目录 |
| 上游 `src-tauri/capabilities/` | Tauri capabilities 权限管理 |
| `@sigma-file-manager/api` v1.0.0 | 第三方插件引用 (extension package.json) |
| `kizemo/focus-sync` v0.2.0 | 已发布的插件示例 (2026-09-27 release) |
| `kizemo/sigma-batch-rename` | 已发布的另一个插件 (公开 repo) |
| `release/extension/` (本地) | 已有的 v0.2.0 extension manifest + dist + sidecar |

---

## 二、真实 Phase 2a 任务（非技术语言）

### 2.1 现状（通俗）

- ✅ **v0.2.0 插件已发布**（kizemo/focus-sync，2026-09-27）
- ✅ **安装方法**：用户在 Sigma 里 "Extensions → Add from URL"，输入 `https://github.com/kizemo/focus-sync/releases/latest/download/package.json`，自动下载安装
- ✅ **功能已可用**：工具栏出现 "Focus Sync" 下拉菜单，"Sync Now" 推送当前目录给所有文件对话框
- ⚠️ **7-app GUI 验证未完成**（用户机器前置：Word/VS Code/WeChat 已装 ✅）

### 2.2 任务清单

#### 阶段 1: 7-app GUI 验证（你这边做，1-2 天）

**每个 app 测试场景**：
1. 安装 focus-sync v0.2.0 到 Sigma
2. 在 app 里打开"另存为/打开/上传"对话框
3. 切到 Sigma，切到**别的目录**（比如从 C:\ 切到 D:\）
4. 切回对话框 — **应该自动跳转到 D:\**
5. 工具栏点 "Sync Now" 也能触发同样效果

**测试矩阵（7 个 app）**：
| # | App | 系统场景 | 测试对话框类型 |
|---|---|---|---|
| 1 | Chrome | 浏览器下载 | Save As (Chromium) |
| 2 | Edge | 浏览器下载 | Save As (Chromium) |
| 3 | Word (你已装 ✅) | 办公文档 | Save As (Win32) |
| 4 | VS Code (你已装 ✅) | 代码编辑器 | Open Folder (Electron) |
| 5 | 钉钉 | 工作沟通 | 发送文件 (Chromium) |
| 6 | 飞书 | 工作沟通 | 上传文件 (Chromium) |
| 7 | 微信 (你已装 ✅) | 个人沟通 | 发送文件 (Chromium) |

**判定标准**：
- ≥5/7 通过 = **PASS**（v0.3.0 release + 提 PR 上游）
- 3-4/7 通过 = **MARGINAL**（修复后再提交）
- ≤2/7 通过 = **STOP**（重新评估 Listary 路线可行性）

**记录方式**：每个 app 截图 + 文字记录，填到 `docs/superpowers/test-plans/2026-09-28-focus-sync-7-app-verify.md`

#### 阶段 2: 修复（如有失败 app）

- 失败的 app：看 spike 端日志（Windows Event Viewer 或 sidecar stdout）
- 调整 uia_inject.rs 的 selector 或 strategy
- 重新 build + publish v0.3.0 release

#### 阶段 3: v0.3.0 release（验证通过后）

- `git tag v0.3.0 && git push --tags` 在 kizemo/focus-sync repo
- GitHub Actions 自动 build sidecar + publish release
- assets: `focus-sync-sidecar-windows-x64.zip` + `package.json` + SHA256
- 更新 release body 写 changelog

#### 阶段 4: PR 给上游 aleksey-hoffman（验证通过 + v0.3.0 后）

**两种策略**：
- **策略 A**（**推荐**）：在 `kizemo/sigma-file-manager` fork 里开 `feat/plugin-focus-sync` 分支，把 focus-sync 插件源码放进 `plugins/focus-sync/` 目录，提交 PR 到 upstream `feat/plugin-focus-sync` 分支
- **策略 B**：在 issue 里发起"focus-sync 收录请求"，引用 kizemo/focus-sync repo 和 v0.3.0 release，让原开发者选择如何整合

**预期**：
- 沿用 `kizemo/sigma-batch-rename` 已知 PR 模板
- 准备 showcase 文案（focus-sync 的功能截图 + 7-app 验证结果）
- 引用 upstream `packages/api` 文档说明这是合规插件

**预期时间线**：
- 之前 PR #546（settings）4 天无活动，原开发者 review 节奏慢
- 准备好心理预期：1-2 周才有反馈

#### 阶段 5: 自己 fork release 整合（与阶段 4 并行）

- 用 `sFM-marketplace` 或类似工具制作 v2.3.0-focus.2 整合 installer
- 内容：Sigma v2.2.0 内核 + tree sidebar v6.1 + focus-sync v0.3.0 预装
- 上传 GitHub Releases（kizemo/sigma-file-manager）

#### 阶段 6: 更新 README + 监控（最后）

- fork README.md 加 v2.3.0-focus.2 release 链接
- 监控 PR #546（原 4 天无活动）+ 监控新提的 focus-sync PR

---

## 三、与之前 spec 的对比

### 之前 (Tauri sidecar 路线，已过时)

- Sigma-side Rust commands/spike.rs
- Sigma-side Pinia listary store
- NSIS 打包 sidecar 进 resources/
- 假设：用户用 Sigma + 单进程 sidecar

### 现在 (Sigma 插件路线)

- Sigma 插件系统（@sigma-file-manager/api v1.0.0）
- TypeScript extension + sidecar 二进制（独立 GitHub Releases）
- 用户在 Sigma Extensions 添加 URL 安装
- 支持多个插件并存（focus-sync + batch-rename + 未来更多）

**优势**：
- 复用 kizemo/sigma-batch-rename 已建立的 PR 流程
- sidecar 在 GitHub Releases 自动 build/publish
- 插件可独立升级（不需要重新发布 Sigma）
- 已有真实用户测试基础（v0.1.0 → v0.2.0 已经过 spike PASS）

---

## 四、避坑提示（累积）

### 4.1 不要做的事

- ❌ 不要回到 Tauri sidecar 集成（spec 已存档）
- ❌ 不要把 focus-sync 集成进 Sigma 内核（违背插件路线）
- ❌ 不要用 v0.2.0 在 7-app 验证前就提交上游（避免提前暴露半成品）
- ❌ 不要 push force 到 kizemo/focus-sync main（插件 release 标签已发）
- ❌ 不要修改 spike 的 3 个移植文件版权头（MIT/Apache-2.0 法律要求）

### 4.2 接力时的明确指令

**下一会话第一句话**：
```
承接 `handoff-2026-09-28-plugin-route.md`。
Phase 2a 路线已修正为 Sigma 插件系统（kizemo/focus-sync v0.2.0 已发布）。
请确认 7-app GUI 验证状态后决定下一步。
```

### 4.3 引用顺序

1. 本文档 (你正在读)
2. `docs/superpowers/specs/_archive/2026-09-27-listary-focus-sync-integration-design.md`（过时 spec，仅参考）
3. `handoff-2026-09-27-spike-complete-next-phase.md`（spike PASS 状态）
4. `kizemo/focus-sync` README + docs/spec.md（插件架构细节）

---

## 五、GitHub 现状（截止 2026-09-28）

| 项 | 状态 |
|---|---|
| kizemo/sigma-file-manager fork | main HEAD = `54186f87`（README 已 push） |
| kizemo fork open PRs | PR #546（settings, OPEN 4 天无活动） |
| aleksey-hoffman/sigma-file-manager upstream | main 无 kizemo 的 tree PR |
| Issue #499（Navigator tree view）| CLOSED，kizemo 评论未被回复 |
| Issue #547（dialog focus sync）| CLOSED，kizemo 自撤回 |
| kizemo/focus-sync v0.2.0 | 2026-09-27 release，含 sidecar + package.json + SHA256 |
| kizemo/sigma-batch-rename | 公开 repo（Sigma 插件示例） |
| 7-app 验证 | **未做**（待你机器上手动测） |

---

## 六、出环境状态

- 当前 cc-haha session: `8edd422a-867b-4a72-bb16-a1f765a80f00`
- 当前 work_dir: `F:\soft\00selfmade\filemanager` (meta-repo)
- 当前 branch: `feat/recovery-2026-09-28` (4 commits ahead of main)
- 本机 Word/VS Code/WeChat: 已装 ✅
- GitHub auth: gh CLI 已配置（kizemo 用户）

---

## 七、Out of scope（继续不做）

- macOS / Linux（Windows UI Automation 是 Windows 专有）
- DLL 注入（AV / WDAC 风险）
- 修改 spike 现有 3 个移植文件版权头
- 上游自动合并（PR 流程走人工评审）
- 关闭 PR #546（kizemo 主动撤回 settings PR，等确认后再决定）
- 重复提交 tree PR（kizemo 已口头提议，原开发者不响应就保留 fork）

---

## 八、给用户的具体下一步

1. **你跑 7-app 验证**（1-2 天）：
   - 装 focus-sync v0.2.0
   - 测 7 个 app，记录结果
   - 失败 3+ 个就 STOP 重评

2. **同时**：我写测试记录模板 + 更新监控脚本支持 PR #546

3. **验证通过后**：
   - v0.3.0 release（自动 CI）
   - 提交 PR 给上游
   - 自己 fork 整合 installer