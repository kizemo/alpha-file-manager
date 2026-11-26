# Reddit 5 个 Sub 帖文 · Alpha File Manager

> 关键提醒：**5 个 sub 别同时发**！发完第一个后等 6-12 小时，
> 看哪个 sub 上首页，再发第二个。一起发会触发 Reddit 反 spam。

每个 sub 给：
- 标题（遵守各 sub 的字数规则 + 风格）
- 正文（约 150-300 词）
- 选择的时机
- 自推阈值

---

## 1 · r/tauri（最贴合受众）

### Sub 概况
- 27k+ 订阅，Tauri 工程师密度最高
- 标题模板：技术角度、有 release 号、贴近 Tauri 概念
- 自推规则：明示"我是作者"可，但语气不能广告

### 标题

```
Built an MCP server into a Tauri 2 file manager — six read-only tools, localhost only (release 3.2.1-beta)
```

### 正文

```
Hi everyone,

We've been shipping Alpha File Manager — a Tauri 2 file manager fork — for
about 24 days now. v3.2.1-beta adds something I think is the first MCP server
productionised inside a desktop Tauri app:

  • `list_directory`, `get_drives`, `get_mountable_devices`, `search_files`,
    `get_directory_size`, `get_favorites`
  • All GET-shaped. No write op is implemented; not "off by default", not
    "toggled off" — just not there.
  • Localhost only (`127.0.0.1`), random access key on first run, request
    origin allow-list (rejects non-file://), permission re-checked every call.
  • One-click setup for Codex, Cursor, Claude Code, Claude Desktop, OpenClaw,
    Minimax Code. The setting panel finds each one's config file, parses it,
    merges one entry in, backs the file up first.

The server side is Rust, in `src-tauri`. The Tauri side just exposes
`app_handle().state::<tauri::Wry>()` to it via a custom command. The
`rand`-based token generation and the origin allow-list are not terribly
interesting — what I think is worth sharing is the call-log format and the
`*.alpha-mcp.bak` filename pattern for backups; happy to talk more about
either.

12 releases in 24 days. Built on top of Sigma File Manager (fork), so all the
heavy lifting on the Vue side is upstream.

Repo: github.com/kizemo/alpha-file-manager
Download: https://www.aiec.fun/alpha-file-manager/

Feedback on the Tauri integration welcome.
```

### 投递时机
HN 上首页的 4-12 小时后（不要同一天）

---

## 2 · r/selfhosted

### Sub 概况
- 大社区，喜欢"本地跑、不联网"的项目
- 标题模板：强调 "self-hostable" / "no cloud" / "local"
- 自推阈值：低

### 标题

```
Alpha File Manager 3.2.1-beta — local-first file manager with an MCP server for AI assistants
```

### 正文

```
Short take: a Tauri-based desktop file manager you can point an AI assistant
at, without anything ever leaving the machine.

Why I'm posting in selfhosted and not in an AI sub:
- The MCP server binds to 127.0.0.1 only (no open port on the network)
- No telemetry, no upload, no account, no "free tier expires"
- Every call is logged in plain text next to your settings — review it any
  time in any editor
- All six tools are read-only — the desktop app itself can do writes; the
  AI-facing surface deliberately cannot

It also fixes the things I bounced off Explorer with for years:
- The open/save dialog in any other app follows the folder you're browsing
- A real folder tree that tracks the active pane
- Typo-tolerant search across the whole drive, ~2s on a 1TB volume
- Batch rename that warns before overwriting files the rules didn't touch

Repo: github.com/kizemo/alpha-file-manager
Download: https://www.aiec.fun/alpha-file-manager/
```

### 投递时机
HN / r/tauri 之后 6-12 小时

---

## 3 · r/LocalLLaMA 或 r/LocalLLM

> r/LocalLLaMA 是更大的 sub（180k+），但偏好本地 LLM 自托管生态
> r/LocalLLM 比较新（4万），对 MCP 钩子更兴奋
> 两者都试，看哪个互动高

### Sub 概况
- 偏好"AI + 本地"组合
- MCP 是 2025-2026 的热门词
- 自推阈值：中

### 标题

```
We wired Claude / Cursor / Codex to a local file manager via MCP — six read-only tools, nothing leaves the machine (3.2.1-beta)
```

### 正文

```
Sharing because a few people here have asked for "let the LLM see my files
without uploading anything" patterns:

Alpha File Manager 3.2.1-beta ships an MCP server inside the desktop app.
The model can list, search, measure, and read favourites — through the file
manager, not by OCR-ing your screen. Six read-only tools, no writes
implemented at the protocol level.

Implementation notes that might be useful:
- The server is in `src-tauri/`, exposes a single Tauri command
- Tokens are generated with `rand` on first activation, persisted in
  user settings
- The origin allow-list rejects non-file:// at the socket layer, not the
  application layer
- Permission levels re-checked on every call (no upgrade-after-connect)
- Logs each call to a text file next to settings — reviewable in any editor

The tooling is small. The interesting part is the deployment shape: it
doesn't require running a separate server, doesn't need a CLI, doesn't
require touching config files by hand (one click per assistant in
Settings → Experimental).

Repo: github.com/kizemo/alpha-file-manager
Download: https://www.aiec.fun/alpha-file-manager/
```

### 投递时机
r/selfhosted 之后 6-12 小时

---

## 4 · r/opensource

### Sub 概况
- 偏严肃一点，对 fork / 独立项目态度友好
- 标题模板：项目陈述、有 release 节奏、能讲清"独立"含义
- 自推阈值：中

### 标题

```
Alpha File Manager — an independent fork of Sigma File Manager shipping daily, with an MCP server in 3.2.1-beta
```

### 正文

```
Mods: I'm a maintainer of Alpha File Manager. Not affiliated with the
upstream Sigma File Manager project beyond forking it. The README on
the repo disclaims this in the second paragraph.

Why I'm posting: independent forks of actively-maintained projects don't
always get seen by the people who'd benefit from them. Sigma File Manager
has 6.7K stars and 287 forks; Alpha sits as one of them but adds a
substantial set of changes:

- Built-in MCP server (3.2.1-beta) — six read-only tools, localhost-only
- Folder tree sidebar that tracks the active pane
- Focus Sync (file dialogs follow the folder you're browsing)
- Typo-tolerant directory search (~2s on a 1TB drive)
- Batch rename with collision pre-warning
- 17 smaller fixes and UX upgrades

Twelve releases in 24 days. We are not upstream and don't take its
sponsorships or support traffic.

Repo: github.com/kizemo/alpha-file-manager
Download: https://www.aiec.fun/alpha-file-manager/
```

### 投递时机
发 r/LocalLLM 后 6-12 小时

---

## 5 · r/productivity

### Sub 概况
- 偏好"省时间""少步骤"工具
- 受众偏 productivity 工具派，不太关心代码
- 自推阈值：中，标题别太广告

### 标题

```
The Windows file manager that finally remembers which folder you were in — and lets AI read it
```

### 正文

```
Background: I've been bouncing off Windows Explorer for years. The two
things that bugged me most — open/save dialogs always start in 'Documents',
and the sidebar being a recent-files list rather than a tree — got fixed
in the same project I'm here to share.

Alpha File Manager is what I switched to. Independent fork of Sigma File
Manager, three weeks of daily releases, available on Windows + Linux.

The "let's AI read it" bit: the latest beta ships an MCP server so
Claude / Cursor / Codex can list folders, search filenames, and check
directory sizes — through the file manager, with no uploads. Six tools,
read-only, localhost-only. Not a separate process to manage, no config
editing.

If you've been wanting an Explorer replacement that actually remembers
where you were, this is worth the 30 seconds it takes to install.

Repo: github.com/kizemo/alpha-file-manager
Download: https://www.aiec.fun/alpha-file-manager/
```

### 投递时机
r/opensource 之后 6-12 小时

---

## 总体调度（推荐顺序）

| Step | Sub | 标题钩子 | 触发条件 |
|---|---|---|---|
| 1 | HN Show HN | AI 钩子 | 现在 → 周二/三晚 9 点 |
| 2 | r/tauri | Tauri 工程角度 | HN 上首页后 4 小时 |
| 3 | r/selfhosted | self-hosting 角度 | r/tauri 后 6 小时 |
| 4 | r/LocalLLM 或 r/LocalLLaMA | AI + 本地钩子 | r/selfhosted 后 6 小时 |
| 5 | r/opensource | 独立 fork 故事 | r/LocalLLM 后 6 小时 |
| 6 | r/productivity | 文件管理痛点 | r/opensource 后 6 小时 |

如果某个 sub 反应冷淡（< 30 upvote in 24h），跳过下一个，不补发——信号是读者画像不对。

## 不要做的事

- ❌ 同时贴 5 个 sub（Reddit 算法会标记）
- ❌ 用同一个账号在不同 sub 完全相同地发（会被所有 sub 标记 spam）
- ❌ 在帖子正文里贴"我们获得了 X stars!"——会让正经观众离开
- ❌ 用"X 个月没更新"作话术——给自己挖坑
- ❌ 用 karma < 50 的新账号——会卡 new-account filter
