# Sigma Listary Spike

Standalone Windows binary that validates whether UI Automation can detect / read / write external file dialog paths in 7 target apps (Chrome, Edge, Word, VS Code, DingTalk, Feishu, WeChat).

**Spec:** `../docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md`
**Plan:** `../docs/superpowers/plans/2026-09-27-listary-focus-sync.md`
**Handoff:** `../handoff-2026-09-27-focus-activation-uia-port.md`

## Implementation Route

This spike **ports** 2 files (~575 LoC) from open-source projects rather than self-writing from scratch. The port route was selected on 2026-09-27 after the original self-written 14-task spike failed 7-app matrix verification (max coverage 57% < 70% threshold).

| Spike file | Source | Lines | License |
|---|---|---|---|
| `src/uia_inject.rs` | [`inaku-Gyan/PathWrap`](https://github.com/inaku-Gyan/PathWrap) `src/os/dialog.rs` | 176 | MIT |
| `src/uia_event.rs` | [`inaku-Gyan/PathWrap`](https://github.com/inaku-Gyan/PathWrap) `src/os/monitor.rs` | 399 | MIT |

> **Round 18 update**: `src/fg_bypass.rs` (QwenLM, Apache-2.0) was removed.
> Its `EnableWindow(FALSE)` foreground-steal shield was the source of the
> filename flash + top-level focus loss reported in round 17. The QwenLM
> attribution and original 129-line file are preserved in git history via
> `git log -- sigma-listary-spike/src/fg_bypass.rs`. PathWrap upstream does
> not use `fg_bypass` either; round-18 spike follows.

## License Boilerplate

### `src/uia_inject.rs` and `src/uia_event.rs` — MIT

```
MIT License — Copyright (c) inaku-Gyan
See https://github.com/inaku-Gyan/PathWrap/blob/main/LICENSE
```

Per-file license headers are preserved verbatim at the top of each ported file.

## Build

```bash
cd sigma-listary-spike
cargo build --release
```

Output: `target/release/spike.exe`

**CRITICAL:** Task 5 validates `windows` 0.58 → 0.62 API compatibility. PathWrap uses 0.62 (direct match). QwenLM uses 0.58 (`HWND::is_null()` → `is_invalid()` upgrade).

## Run

```bash
./target/release/spike.exe --port 37421 --initial-path "C:\\Users\\foo"
```

## Verify (target matrix)

```powershell
pwsh scripts/verify_target_matrix.ps1 -SpikePath ./target/release/spike.exe
```

Exits 0 if coverage ≥ 70%, exit 2 if coverage < 70%.