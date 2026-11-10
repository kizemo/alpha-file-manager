# Monitor Issue #499 — Setup Guide

Periodically checks if `aleksey-hoffman/sigma-file-manager` issue #499 has new comments (specifically the maintainer's response to our fork sync request).

Two scripts provided:
- `monitor-issue-499.ps1` — native PowerShell, Windows-friendly, supports desktop toast notifications
- `monitor-issue-499.sh` — bash version, works in git-bash on Windows or WSL/Linux

## Prerequisites

- `gh` CLI installed and authenticated (`gh auth status` should show your account)
- `jq` (only required for bash version; PowerShell uses `ConvertFrom-Json` natively)

## Quick test (one-shot check)

```powershell
# PowerShell
.\monitor-issue-499.ps1 -Once
```

```bash
# bash
./monitor-issue-499.sh --once
```

## Continuous monitoring

### Option A: Run in foreground (terminal session)

```powershell
# Check every 30 minutes (1800 seconds = 30 min, default is 12 hours)
.\monitor-issue-499.ps1 -CheckIntervalMinutes 30
```

```bash
./monitor-issue-499.sh --interval-min 30
```

When a new comment arrives, you'll see output + a Windows desktop toast (PowerShell version).

### Option B: Windows Task Scheduler (recommended for unattended)

1. Open `taskschd.msc`
2. Create Basic Task:
   - **Name**: Monitor issue #499
   - **Trigger**: Daily, repeat every 12 hours
   - **Action**: Start a program
     - Program: `powershell.exe`
     - Arguments: `-ExecutionPolicy Bypass -File "F:\soft\00selfmade\filemanager\scripts\monitor-issue-499.ps1" -Once`
   - **Run whether user is logged on or not**: optional

3. The script writes to `.issue-499-state.json` in the scripts folder — only notifies on NEW comments since the last check.

## How state works

- First run: no state file → reports all current comments
- Subsequent runs: only reports comments newer than `LastSeenCommentId`
- State file: `.issue-499-state.json` (gitignored, lives next to the script)

If you want to "reset" monitoring (e.g., to see all comments again), delete the state file:

```bash
rm .issue-499-state.json
```

## What to do when notified

When the script reports a new comment from `aleksey-hoffman`:

1. Read the full comment:
   ```bash
   gh issue view 499 --repo aleksey-hoffman/sigma-file-manager --comments
   ```

2. Apply the response branch from `docs/superpowers/specs/2026-09-26-upstream-pr-strategy-design.md` Step 2:
   - "Send PR" → open `pr/tree-sidebar` branch, cherry-pick 6 commits, drop [fork-keep] markers
   - "Wait, I'm close to done" → freeze, observe upstream
   - "No thanks" → archive, fork-only forever

3. Start a new Claude Code session with: "From handoff.md, see issue #499 maintainer reply — apply Step 2 branch and continue."

## Why I (the agent) can't run this persistently

The agent session ends between conversations. The script + scheduler is the bridge — it runs on your machine, persists across sessions, and notifies you when action is needed.