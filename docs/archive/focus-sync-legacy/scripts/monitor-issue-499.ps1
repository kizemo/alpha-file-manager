# SPDX-License-Identifier: GPL-3.0-or-later
# Monitor issue #499 in aleksey-hoffman/sigma-file-manager for maintainer reply.
# Usage: .\monitor-issue-499.ps1 [-Once]
#   -Once: check once and exit. Default: loop every CHECK_INTERVAL_MINUTES.

param(
    [switch]$Once = $false,
    [int]$CheckIntervalMinutes = 720  # 12 hours
)

$ErrorActionPreference = 'Stop'
$Repo = 'aleksey-hoffman/sigma-file-manager'
$IssueNumber = 499
$StateFile = Join-Path $PSScriptRoot '.issue-499-state.json'

# Load last-seen comment ID (if any).
$lastSeenId = $null
if (Test-Path $StateFile) {
    try {
        $state = Get-Content $StateFile -Raw | ConvertFrom-Json
        $lastSeenId = $state.LastSeenCommentId
    } catch {
        Write-Warning "Could not parse state file: $_"
    }
}

function Get-LatestCommentId {
    $json = gh issue view $IssueNumber --repo $Repo --comments --json comments 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "gh CLI failed for issue $IssueNumber in $Repo"
    }
    $parsed = $json | ConvertFrom-Json
    if ($parsed.comments.Count -eq 0) { return $null }
    return $parsed.comments[-1].id
}

function Get-NewComments {
    $json = gh issue view $IssueNumber --repo $Repo --comments --json comments 2>$null
    $parsed = $json | ConvertFrom-Json
    $comments = @($parsed.comments)
    if (-not $lastSeenId) { return $comments }

    $idx = -1
    for ($i = 0; $i -lt $comments.Count; $i++) {
        if ($comments[$i].id -eq $lastSeenId) { $idx = $i; break }
    }
    if ($idx -lt 0) { return $comments }
    return $comments[($idx + 1)..($comments.Count - 1)]
}

function Notify-Desktop {
    param([string]$Title, [string]$Body)
    # Windows toast notification.
    Add-Type -AssemblyName System.Windows.Forms
    $balloon = New-Object System.Windows.Forms.NotifyIcon
    $balloon.Icon = [System.Drawing.SystemIcons]::Information
    $balloon.BalloonTipTitle = $Title
    $balloon.BalloonTipText = $Body
    $balloon.Visible = $true
    $balloon.ShowBalloonTip(10000)
    Start-Sleep -Seconds 1
    $balloon.Dispose()
}

function Check-Once {
    Write-Host "[$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')] Checking issue #$IssueNumber in $Repo..."
    try {
        $newComments = Get-NewComments
        if ($newComments.Count -eq 0) {
            Write-Host "  No new comments."
            return
        }

        Write-Host "  $($newComments.Count) new comment(s) found!"
        foreach ($c in $newComments) {
            $author = $c.author.login
            $preview = ($c.body -replace "`n", ' ').Substring(0, [Math]::Min(120, $c.body.Length))
            Write-Host "  - [$author] $preview..."
        }

        # Update state.
        $latestId = (Get-LatestCommentId)
        @{ LastSeenCommentId = $latestId; LastCheckTime = (Get-Date -Format 'o') } |
            ConvertTo-Json | Set-Content $StateFile

        # Desktop notification.
        $lastComment = $newComments[-1]
        Notify-Desktop -Title "Issue #$IssueNumber new comment" -Body "From $($lastComment.author.login): $($lastComment.body.Substring(0, [Math]::Min(80, $lastComment.body.Length)))..."

    } catch {
        Write-Error "Check failed: $_"
    }
}

if ($Once) {
    Check-Once
    exit 0
}

# Loop mode.
Write-Host "Monitoring issue #$IssueNumber every $CheckIntervalMinutes minutes. Press Ctrl+C to stop."
while ($true) {
    Check-Once
    Start-Sleep -Seconds ($CheckIntervalMinutes * 60)
}