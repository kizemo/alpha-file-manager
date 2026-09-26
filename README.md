**[English](README.md)** | **[中文](README.zh-CN.md)**

<h1>
  <img valign="middle" src="https://github.com/aleksey-hoffman/sigma-file-manager/raw/main/.github/media/logo-1024x1024.png" width="64px">
  &nbsp;&nbsp;Sigma File Manager (kizemo fork)
</h1>

"Sigma File Manager" is a free, open-source, quickly evolving, modern file manager (explorer / finder) app for Windows and Linux.

This repository is a personal fork maintained by [kizemo](https://github.com/kizemo) that tracks upstream [`aleksey-hoffman/sigma-file-manager`](https://github.com/aleksey-hoffman/sigma-file-manager) and adds tree-view and other ergonomic improvements on top of it. The fork does **not** ship its own installers or releases; build from source or sync with upstream for binary downloads.

## Folder tree sidebar (the main fork feature)

![Sigma File Manager with tree sidebar showing E:/办公文件 directory](./docs/screenshots/tree-sidebar-v6.4.1.png)

A left-side **folder tree sidebar** that mirrors the file system and follows the active pane, so you always see where you are and can jump up/down the hierarchy with a single click.

- **Toggle:** a dedicated `FolderTree` icon button in the navigator toolbar — it is intentionally **not** inside the layout dropdown, so the toggle stays one click away at all times.
- **Sync sources:** the tree automatically follows the address bar, the favorites / quick-access panel, and the **active pane** in split-view, so each side keeps its own tree state.
- **Single-path expand:** when you navigate, only the ancestor chain of the current path stays open; previously-open branches elsewhere are collapsed, keeping the tree compact and scannable.
- **Click semantics:** clicking a row navigates to that folder; clicking the chevron expands or collapses the branch without changing the current directory.
- **Drive labels:** root nodes show both the volume label and the drive letter, e.g. `Win (C:)`, so multi-WSD / multi-drive setups are disambiguated at a glance.
- **Persistence:** the show/hide state survives restarts via `userSettings.navigator.showFolderTree`.
- **Implementation branch:** [`feat/tree-sidebar-v6-1`](https://github.com/kizemo/sigma-file-manager/tree/feat/tree-sidebar-v6-1), HEAD `8caf14ae` — 6 atomic commits, ~3000 lines including tests and docs.
- **Tests:** 259 unit tests passing.
- **Upstream tracking:** this work is tracked against upstream issue [#499](https://github.com/aleksey-hoffman/sigma-file-manager/issues/499).

## Coming soon

- **File picker dialog focus sync** (Listary-style): when Sigma's own file picker is opened from another app, the address bar / tree will pre-focus the directory the requesting app started in. Scoped to Sigma's picker only; no global system hooks. Implementation plan lives at `docs/superpowers/plans/2026-09-26-dialog-focus-sync.md` in the meta repo and will be ported into this fork once the picker plumbing is in place.

## Credits

- Upstream: [aleksey-hoffman/sigma-file-manager](https://github.com/aleksey-hoffman/sigma-file-manager) by [Aleksey Hoffman](https://github.com/aleksey-hoffman). All product features, branding, and release pipelines belong to upstream.
- Fork maintainer: [kizemo](https://github.com/kizemo).

## License

GPL-3.0-or-later — see [`LICENSE.md`](./LICENSE.md). Fork additions are contributed under the same license.