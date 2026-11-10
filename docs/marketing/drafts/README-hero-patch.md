# README Hero Patch · 2026-10-17

这两个 README（英文 / 中文）的 hero 句没改动原文，只是在 hero 上方**加一行 2026 年的"AI/MCP" 钩子**。这样：
- 旧的"传统替代 Explorer"角度保留
- 新的"AI 可读硬盘"角度先入为主
- 全部都是 additive 修改，不动既有内容

## 改动方式

每个 README 只在 hero 上面加一段 callout/quote，剩余 100% 不动。
任何问题可一条 Edit 还原。

## ⚠️ 推荐用法

请以 **PR** 形式 push 到 main（不要 force / 直接覆盖）。
本仓库 main 上禁止非 PR 形式的直推（per `~/.claude/CLAUDE.md`）。

PR 标题建议：`docs(readme): lead with the 3.2.1-beta AI/MCP hook`
PR body 用 `docs/marketing/promotion-strategy-alpha-fm-weavepage-2026-10-17.md` 的 § 5.1 段说明背景。

---

## 英文 README 改动（README.md）

### 改前

```markdown
**[English](README.md)** | **[中文](README.zh-CN.md)**

<h1>&nbsp;&nbsp;Alpha File Manager</h1>
```

### 改后

```markdown
**[English](README.md)** | **[中文](README.zh-CN.md)**

> **🆕 3.2.1-beta ships an MCP server** — let AI assistants (Claude, Codex,
> Cursor, OpenClaw, Minimax Code) list your drives, search filenames,
> and read favourites through your file manager, not by OCR-ing
> screenshots. Read-only by design. Six tools. Localhost-only. One-click
> per assistant. → [What's new in 3.2.1-beta](#-whats-new-in-321-beta) ·
> [Turn it on](#-an-assistant-can-now-ask-your-file-manager)

<h1>&nbsp;&nbsp;Alpha File Manager</h1>
```

> Note: README 下方已经有一节 `🆕 What's new in 3.2.1-beta` 和
> `🤖 An assistant can now ask your file manager`，链接锚点保留原样。
> `(#-whats-new-in-321-beta)` 与 `(#-an-assistant-can-now-ask-your-file-manager)`
> 是 GitHub Markdown 把 emoji 当 heading 时的标准 anchor，可直接用。

---

## 中文 README 改动（README.zh-CN.md）

### 改前

```markdown
**[English](README.md)** | **[中文](README.zh-CN.md)**

# Alpha File Manager
```

### 改后

```markdown
**[English](README.md)** | **[中文](README.zh-CN.md)**

> **🆕 3.2.1-beta 内置 MCP server** —— 让 Claude / Codex / Cursor / OpenClaw / Minimax
> Code 这类 AI 助手直接列你的盘、搜文件名、读收藏夹，**通过文件管理器**，不是
> OCR 截图猜。默认只读不写。6 个工具。绑定 localhost 不可外连。每个助手一键
> 配置。→ [看 3.2.1-beta 更新](#-3-2-1-beta-更新内容) · [开启方法](#-ai-助手可以问你的文件管理器了)

# Alpha File Manager
```

> 中文版的锚点按你当前 README 内的实际标题原文为准，我没读过全文无法预测
> GitHub 渲染的 anchor 字符。你 push 后**如果发现锚点 404**，把这两处
> 改成 `# 跳到 3.2.1-beta 那一节的真实字符`。

---

## 为什么是这个改法

1. **不删一行原文** ——
2. **不重新编号小节** —— 旧锚点全部兼容
3. **不调 HTML** —— `<h1>` 留着
4. **AI/MCP 这条主线就是 2026 年的最强 hook** —— 放在 hero 上面，第一次
   刷到的人就看见，而不是埋到 `What's new` 节才看见
5. **保留「基于 Sigma File Manager」的独立声明** —— 那段 [!IMPORTANT] 不动

---

## 备选改法（如果你觉得这条 callout 风格不适合）

### 备选 A · 极简版

> 🆕 **3.2.1-beta** ships an MCP server — six read-only tools that let AI
> assistants browse your drives. See [below](#-whats-new-in-321-beta).

（中英文各 1 行，长度更紧）

### 备选 B · 标题下沉

不调 hero，而是在 `🆕 What's new in 3.2.1-beta` 节**上方**插入一个 banner：

```markdown
---

## 🤖 TL;DR — 3.2.1-beta is the AI release

A build of Alpha File Manager that comes with a **Model Context Protocol**
server. Tell Claude, Codex or Cursor to list your drives, search filenames or
read your favourites, and the calls actually go through your file manager —
not through OCR. Read-only. Localhost-only. Six tools. → [read on](#-whats-new-in-321-beta)

---
```

这个版本会把原 README 断成几段（`---` 视觉分隔），效果略夺目。

---

## 不推荐的改法

- ❌ **直接改 hero 句 "A modern file manager for Windows and Linux — with the
  two things you miss most in Explorer..."** —— 这句写得够好（"two things you
  miss most in Explorer" 是经典 hook），贸然替换属于"为了改而改"
- ❌ **加 community badge（Discord / Twitter 等链接）** —— 现在没有对应
  账号。先做基建，账号建好再补
- ❌ **塞任何 ⭐ "Star us if..." 类诉求** —— 这种话 GitHub 用户普遍反感
