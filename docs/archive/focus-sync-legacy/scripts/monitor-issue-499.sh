#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Bash equivalent of monitor-issue-499.ps1 for git-bash on Windows or WSL.
# Usage: ./monitor-issue-499.sh [--once] [--interval-min N]

set -euo pipefail

REPO="aleksey-hoffman/sigma-file-manager"
ISSUE_NUMBER=499
STATE_FILE="$(dirname "$0")/.issue-499-state.json"
INTERVAL_MIN=720  # 12 hours default
ONCE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --once) ONCE=1; shift ;;
        --interval-min) INTERVAL_MIN="$2"; shift 2 ;;
        *) echo "Unknown arg: $1" >&2; exit 1 ;;
    esac
done

check_once() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Checking issue #$ISSUE_NUMBER in $REPO..."

    if ! command -v gh >/dev/null 2>&1; then
        echo "  ERROR: gh CLI not found in PATH" >&2
        return 1
    fi

    # Load state.
    local last_seen_id=""
    if [[ -f "$STATE_FILE" ]]; then
        last_seen_id=$(jq -r '.LastSeenCommentId // empty' "$STATE_FILE" 2>/dev/null || echo "")
    fi

    # Fetch comments.
    local comments_json
    comments_json=$(gh issue view "$ISSUE_NUMBER" --repo "$REPO" --comments --json comments 2>/dev/null)
    if [[ $? -ne 0 ]]; then
        echo "  ERROR: gh CLI failed" >&2
        return 1
    fi

    local total
    total=$(echo "$comments_json" | jq '.comments | length')
    if [[ "$total" -eq 0 ]]; then
        echo "  No comments on issue."
        return 0
    fi

    # Find new comments.
    local new_count=0
    local -a new_authors=()
    local -a new_previews=()
    if [[ -z "$last_seen_id" ]]; then
        new_count=$total
    else
        local start_idx
        start_idx=$(echo "$comments_json" | jq --arg id "$last_seen_id" '.comments | map(.id == $id) | index(true)')
        if [[ "$start_idx" == "null" || -z "$start_idx" ]]; then
            new_count=$total
            start_idx=0
        else
            new_count=$((total - start_idx - 1))
            start_idx=$((start_idx + 1))
        fi
        if [[ $new_count -gt 0 ]]; then
            mapfile -t new_authors < <(echo "$comments_json" | jq -r ".comments[$start_idx:$total] | .[].author.login")
            mapfile -t new_previews < <(echo "$comments_json" | jq -r ".comments[$start_idx:$total] | .[].body" | head -c 120)
        fi
    fi

    if [[ $new_count -eq 0 ]]; then
        echo "  No new comments since last check."
        return 0
    fi

    echo "  $new_count new comment(s)!"
    for ((i=0; i<new_count; i++)); do
        echo "  - [${new_authors[$i]}] ${new_previews[$i]}..."
    done

    # Update state.
    local latest_id
    latest_id=$(echo "$comments_json" | jq -r '.comments[-1].id')
    jq -n --arg id "$latest_id" --arg time "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        '{LastSeenCommentId: $id, LastCheckTime: $time}' > "$STATE_FILE"

    return 0
}

if [[ $ONCE -eq 1 ]]; then
    check_once
    exit $?
fi

echo "Monitoring issue #$ISSUE_NUMBER every $INTERVAL_MIN minutes. Ctrl+C to stop."
while true; do
    check_once
    sleep "$((INTERVAL_MIN * 60))"
done