**[English](README.md)** | **[中文](README.zh-CN.md)**

<h1>&nbsp;&nbsp;Alpha File Manager</h1>

**A modern file manager for Windows and Linux — with the two things you miss
most in Explorer: a real folder tree, and file dialogs that follow you between apps.**

Alpha File Manager is an independent build of
[Sigma File Manager](https://github.com/aleksey-hoffman/sigma-file-manager),
maintained by [kizemo](https://github.com/kizemo). It tracks upstream and layers
on two workflow improvements:

> 🗂️ **"Where am I?"** — a persistent folder tree that always shows the shape of where you are.
> ⚡ **"Why am I typing this path again?"** — every file dialog follows the folder you're browsing.

> [!IMPORTANT]
> **Alpha File Manager is an independent community project.** It is not affiliated
> with, endorsed by, or supported by the Sigma File Manager project. The base
> application's features and branding belong to [Aleksey Hoffman](https://github.com/aleksey-hoffman).

---

## 🗂️ Folder tree sidebar

![Alpha File Manager with the folder tree sidebar showing E:/办公文件](./docs/screenshots/tree-sidebar-v6.4.1.png)

A folder tree down the left edge that mirrors the file system and tracks
whichever pane is active — so you can see the shape of where you are and reach
any ancestor in one click.

- **Always one click away** — a dedicated `FolderTree` button in the navigator
  toolbar, deliberately *not* buried in the layout dropdown.
- **Follows everything** — tracks the address bar, the favourites panel, and the
  **active pane** in split view; each side keeps its own expansion state.
- **Stays compact** — on navigation only the current path's ancestor chain stays
  open; everything else collapses.
- **Predictable clicks** — click a row to enter it, click the chevron to expand
  without changing directory.
- **Unambiguous drives** — root nodes show volume label *and* drive letter
  (`Win (C:)`), so multi-drive and WSL setups read at a glance.
- **Remembers** — show/hide state persists across restarts.

Tracks upstream issue [#499](https://github.com/aleksey-hoffman/sigma-file-manager/issues/499) · 259 unit tests passing.

---

## ⚡ Focus Sync — file dialogs that follow you

Browse to a folder, then open a **Save As / Open** dialog in *any* Windows app —
Chrome downloads, Word, VS Code, DingTalk, Feishu — and it's already sitting in
that same folder.

**Zero configuration.** The installer bundles the extension and a small local
companion process; a Windows scheduled task starts it at logon. There is nothing
to launch by hand.

**The rules it holds itself to**

| | |
|---|---|
| **Never steals focus** | If the dialog isn't in the foreground, the write waits — it is never forced. |
| **Never leaves a dirty filename** | The address bar is the target. When a fallback must touch the filename field, it restores the original and **reads the value back** before committing. |
| **Never writes a stale path** | If Alpha FM isn't running, writes are refused rather than filling your dialog with where you *used to* be. |

The extension requests only `commands`, `toolbar`, `notifications`, and HTTP to
`127.0.0.1` — **no shell access, no filesystem access.**

> **Status** — still being hardened. Validated against native Win32 dialogs;
> behaviour varies across Chromium and UWP hosts.

---

## 📦 Getting it

**Prebuilt release:** [`v2.2.0-tree.1`](https://github.com/kizemo/alpha-file-manager/releases/tag/v2.2.0-tree.1)
— a folder-tree build from 26 Sep 2026.

> That tag predates the Focus Sync packaging work on `main`: it gives you the
> tree sidebar but not the current one-step installer. A fresh build is on the way.

Windows builds are unsigned, so SmartScreen reports *"Unknown publisher"* — click
**More info → Run anyway**.

---

## Credits

- **Upstream:** [aleksey-hoffman/sigma-file-manager](https://github.com/aleksey-hoffman/sigma-file-manager)
  by [Aleksey Hoffman](https://github.com/aleksey-hoffman). The base application,
  its features and its branding belong to upstream.
- **Focus Sync extension:** lives in THIS repository under
  [`extensions/kizemo.focus-sync/`](./extensions/kizemo.focus-sync/) —
  extension payload, sidecar source and the standalone installer.
  Released standalone installers are archived at
  [kizemo/focus-sync](https://github.com/kizemo/focus-sync).
- **Maintainer of this fork:** [kizemo](https://github.com/kizemo)

Additional code reused under its original terms:
[inaku-Gyan/PathWrap](https://github.com/inaku-Gyan/PathWrap) (MIT) ·
[QwenLM/qwen-code](https://github.com/QwenLM/qwen-code) (Apache-2.0).

---

## License

**GPL-3.0-or-later** — see [`LICENSE.md`](./LICENSE.md). Fork additions are
contributed under the same license.

As required by GPL-3, the corresponding source for every shipped binary is
available at:

| Component | Source |
|---|---|
| Alpha File Manager (this fork) | <https://github.com/kizemo/alpha-file-manager> |
| Sigma File Manager (base) | <https://github.com/aleksey-hoffman/sigma-file-manager> |
| Focus Sync extension + sidecar | bundled in this repository, `extensions/kizemo.focus-sync/` |