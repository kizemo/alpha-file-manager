# Show HN 文案 · Alpha File Manager 3.2.1

**发送目标**：https://news.ycombinator.com/showhn.html
**作者账号**：你的 HN 个人账号（建议准备一个"独立开发者"人设的账号，不要用工作身份）
**建议发送时间**：周二 / 周三 北京时间 21:00-22:00 = 美东 09:00-10:00
**字数**：标题 ≤ 80 字符，正文 ≤ 250 字（HN 首页节奏偏好简短）

---

## 方案 A · 主推"AI 可读硬盘"

### 标题

```
Show HN: Alpha File Manager – The file manager your AI agent can read
```

### 正文

```
Hey HN,

We've been hacking on a fork of Sigma File Manager for a few weeks, and
3.2.1-beta ships what we couldn't find anywhere else: a built-in MCP server
so Codex/Claude/Cursor can list your drives, browse a folder, search filenames,
work out directory sizes, and read favourites — through the file manager that
already knows about shortcuts, mounts and network shares.

It's read-only on purpose. Six tools, all GET-shaped. The server binds to
127.0.0.1 only, generates a random access key on first activation, refuses
requests from non-file:// origins, and re-checks permissions on every call
(not just on connect). Logs every call next to settings.

Setup is one click per assistant: it finds Codex/Claude/Cursor/OpenClaw/Minimax
on your computer, names the file each one reads, merges a single entry in,
backs the file up first. Twelve releases in 24 days as we iterate.

Built on Tauri 2. Ships Windows + Linux (flatpak). Free, open source.

Repo:   github.com/kizemo/alpha-file-manager
Binary: https://www.aiec.fun/alpha-file-manager/

Happy to answer anything.
```

### 字数核验

正文约 195 词 = 标题 ~75 字符 + 正文 ~1100 字符。控制在 HN 主流上限内。

---

## 方案 B · 主推"它真正替代了 Explorer"

如果你想避开 AI 钩子走"传统替代"路线：

### 标题

```
Show HN: Alpha File Manager – a file manager that actually remembers where you were
```

### 正文

```
Hey HN,

Fork of Sigma File Manager with the two changes that bugged me most in
Explorer, plus a long list of small fixes:

- The open/save file dialog in every other app follows the folder you're
  currently browsing (Focus Sync, all local, no system hooks)
- A real folder tree that persists across sessions, not the recent-files
  list Explorer calls a sidebar
- Directory search recurses on matches, not on entries. ~2s on a 1TB drive.
  Typo-tolerant (still finds "annual report" if you typed "pdf 年报").

3.2.1-beta also ships a read-only MCP server so AI assistants can browse the
same way (six tools, localhost-only, random access key).

Built on Tauri 2, Windows + Linux, 12 releases in 24 days. Independent
maintenance — full credits to Aleksey Hoffman's Sigma upstream.

Repo:   github.com/kizemo/alpha-file-manager
Binary: https://www.aiec.fun/alpha-file-manager/
```

### 何时选 B 方案

- 当 AI 钩子已经被别人用过很多次（HN 反 AI 疲劳季）
- 当你更有热情谈产品本身的体验细节
- 当你想吸引"实用工具派"用户而非"AI 派"开发者

---

## 方案 C · 主推"独立维护 + 上游 reference"

适合给"独立开发者会喜欢" 的角度：

### 标题

```
Show HN: Alpha File Manager – an independent fork of Sigma File Manager, shipping daily
```

### 正文

```
Hey HN,

I'm maintaining an independent fork of Sigma File Manager with these
changes on top:

- Folder tree sidebar that follows the active pane (Sigma has a recent-
  files panel, not a tree)
- Focus Sync: every other app's file dialog opens at the folder you're
  currently browsing (already on a separate repo, github.com/kizemo/focus-sync)
- Typo-tolerant directory search across the whole drive
- Batch-rename that warns before overwriting a file the rules didn't touch
  (the real fix, more interesting than it sounds)
- 3.2.1-beta: read-only MCP server, six tools, 127.0.0.1 only, random
  access key on first run, call log next to settings

Disclaimer: Alpha File Manager is an independent community build. The base
app, brand and trademarks belong to Aleksey Hoffman. Sponsorships and
official support channel remain on the upstream project.

12 releases in 24 days (we iterate daily). Tauri 2, Windows + Linux.

Repo:   github.com/kizemo/alpha-file-manager
Binary: https://www.aiec.fun/alpha-file-manager/
```

---

## 三方案选择建议

- 默认：**方案 A**（AI 钩子是 2026 年的最强流量入口）
- 备选：**方案 C**（如果你觉得 Aleksey 看到会不开心，A 可能让他感到"被借力"，C 的措辞更尊重上游）
- 冷门：**方案 B**（如果 HN 已经 Show 过得太多 AI 项目）

---

## 发送前 checklist

- [ ] 你已有个人 HN 账号（不是工作身份 / 不是公司官方账号）
- [ ] 账号有 ≥ 1 个真实过往评论/karma（防自推判定）
- [ ] 仓库 README hero 上方已经加了 3.2.1-beta 钩子（见 `docs/marketing/drafts/README-hero-patch.md`）—— 不加，访客点进来会困惑
- [ ] Social Preview 已经上传（见 `AFM-WEB-UI-MANUAL-STEPS.md`）—— 不加，分享卡片是空的
- [ ] v3.2.1 stable 已经发布（不是 beta）—— beta 上不了 trending 推荐

## 发送后前 6 小时

- HN 早期互动决定能否上首页。每隔 30-45 分钟回评论
- 任何问技术细节的回帖：引 README 段落，避免长篇大论
- 任何阴阳怪气（"this is just a wrapper"）：不要回，不要 dispute，let it go
- 6 小时还上不了首页：不用沮丧，第二天换 sub 重发

## 同步动作

HN 上首页后立刻同步：
- r/tauri: "Just shipped on HN — Alpha File Manager 3.2.1-beta"
- r/selfhosted: "Local-first file manager, but with an MCP server"
- r/LocalLLM: same with AI focus
- r/opensource: "Active fork of Sigma File Manager shipping daily"
- r/productivity: "The Windows file manager that finally feels like 2010 again"

5 个 sub，**今天先起草好文本（见 `reddit-afm.md`），HN 上首页后再发**——别一股脑一起发，那样 HN 会判你是 spam。
