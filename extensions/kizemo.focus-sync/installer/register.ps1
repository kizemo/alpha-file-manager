# ============================================================================
# register.ps1 — Register kizemo.focus-sync in Sigma FM's user-extensions.json
# ============================================================================
# This script is invoked by installer.nsi after copying the extension files.
# It edits %APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json
# to add (or refresh) the kizemo.focus-sync entry under installedExtensions.
#
# IMPORTANT: Close Sigma File Manager BEFORE running this script.
# Sigma FM uses a one-shot startup bootstrap from user-extensions.json. If it
# is running, its in-memory state is stale and will overwrite this file on
# its next save (within seconds/minutes), undoing the registration.
#
# Idempotent: re-running on an already-registered extension preserves the
# enabled / autoUpdate state and refreshes the manifest.
#
# -UserAppData parameter (round 19d, 2026-09-30):
#   For perMachine installers running in admin context, $env:APPDATA resolves
#   to the ADMIN's profile, NOT the original user's. The NSIS hook now calls
#   resolve_user_appdata.ps1 first to recover the real user's path and passes
#   it via -UserAppData. If omitted, falls back to $env:APPDATA (works for
#   perUser / current-context installs and for the standalone extension
#   installer which runs as the current user).
# ============================================================================

param(
    [string]$UserAppData
)

$ErrorActionPreference = 'Stop'

# ---- Resolve target APPDATA (round 19d) ----
# Priority: explicit -UserAppData parameter > $env:APPDATA (current context)
if ($UserAppData) {
    $resolvedAppData = $UserAppData
    Write-Host "==> Using explicit UserAppData from installer: $resolvedAppData"
} else {
    $resolvedAppData = $env:APPDATA
    Write-Host "==> Using current process env:APPDATA: $resolvedAppData"
}

# ---- Paths ----
$appDataDir    = Join-Path $resolvedAppData 'com.sigma-file-manager.app'
$userDataDir   = Join-Path $appDataDir 'user-data'
$jsonPath      = Join-Path $userDataDir 'user-extensions.json'

# ---- Sandbox gate (focus-21 Phase 4) ----
# register.ps1 does not copy index.js itself (installer.nsi copies the files
# before invoking this script), so we validate the DEPLOYED copy — the exact
# file Sigma FM's loader.ts will read on next startup.
# If any sandbox regex matches (even inside a comment), validateExtensionCode()
# rejects the extension and loader.ts throws BEFORE `new Worker()`: the
# extension silently never loads — no UI error, no notification, sidecar 0
# requests. Rules are read LIVE from sandbox.ts, so this gate never goes stale.
$sandboxScanner  = Join-Path $PSScriptRoot '..\..\scripts\scan-sandbox-dynamic.cjs'
$sandboxTs       = Join-Path $PSScriptRoot '..\..\sigma-file-manager\src\modules\extensions\runtime\sandbox.ts'
$deployedIndexJs = Join-Path $appDataDir 'extensions\kizemo.focus-sync\dist\index.js'
if ((Test-Path $sandboxScanner) -and (Test-Path $deployedIndexJs) -and (Test-Path $sandboxTs)) {
    Write-Host "==> Validating extension against Sigma FM sandbox (dynamic)"
    # Pass sandbox.ts explicitly: the scanner's default is CWD-relative, so
    # relying on it would FATAL (exit 2) on a machine whose CWD is elsewhere.
    & node $sandboxScanner $deployedIndexJs $sandboxTs
    if ($LASTEXITCODE -ne 0) {
        Write-Error @"
SANDBOX VALIDATION FAILED (exit $LASTEXITCODE).
Refusing to register an extension Sigma FM will reject. It would load
SILENTLY-NEVER — no UI error, no notification, sidecar gets 0 requests.
"@
        exit 1
    }
    Write-Host "==> Extension sandbox validation PASSED"
} else {
    # Packaged installs run from $INSTDIR where the repo (and scanner) is absent.
    Write-Host "==> Sandbox scanner or deployed index.js not found - gate SKIPPED (packaged install)."
}

# ---- Validation + bootstrap (round 19e, 2026-09-30) ----
# Earlier this script exited 1 if appDataDir or jsonPath was missing, which
# silently failed on fresh installs (Sigma FM never launched) and on
# %APPDATA% trees wiped between uninstall and install. Bootstrap them here.
if (-not (Test-Path $appDataDir)) {
    Write-Host "==> appDataDir missing: $appDataDir — creating."
    New-Item -ItemType Directory -Path $appDataDir -Force | Out-Null
}
if (-not (Test-Path $userDataDir)) {
    Write-Host "==> userDataDir missing: $userDataDir — creating."
    New-Item -ItemType Directory -Path $userDataDir -Force | Out-Null
}
if (-not (Test-Path $jsonPath)) {
    Write-Host "==> user-extensions.json missing — bootstrapping with empty structure."
    $bootstrap = [ordered]@{
        installedExtensions = [ordered]@{}
        recentCommandIds    = @()
    }
    $bootstrapJson = $bootstrap | ConvertTo-Json -Depth 10
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($jsonPath, $bootstrapJson, $utf8NoBom)
}

# ---- Backup ----
$timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$backupPath = "C:\Temp\user-extensions.json.bak-register-$timestamp"
try {
    Copy-Item $jsonPath $backupPath -Force
    Write-Host "==> Backup created: $backupPath"
} catch {
    Write-Warning "Backup failed (continuing anyway): $_"
}

# ---- Read existing JSON ----
Write-Host "==> Reading $jsonPath"
$json = Get-Content $jsonPath -Raw | ConvertFrom-Json

# ---- Build kizemo manifest (in declared field order) ----
$kizemoManifest = [ordered]@{
    id           = 'kizemo.focus-sync'
    name         = 'Focus Sync'
    version      = '0.5.5'
    description  = "Listary-style global focus sync. Sidecar runs as a Windows Scheduled Task (independent of Sigma FM lifecycle)."
    publisher    = [ordered]@{
        name = 'kizemo'
        url  = 'https://github.com/kizemo'
    }
    repository   = 'https://github.com/kizemo/focus-sync'
    license      = 'MIT'
    icon         = 'icon.png'
    categories   = @('productivity', 'navigation')
    tags         = @('listary', 'focus', 'sync', 'file-dialog')
    extensionType = 'api'
    main         = 'dist/index.js'
    type         = 'module'
    engines      = [ordered]@{
        sigmaFileManager = '>=2.2.0'
        extensionApi     = '>=1.0.0'
    }
    permissions  = @(
        # 'shell' REMOVED in v0.3.0 — sidecar no longer spawned by Sigma FM
        'commands',
        'toolbar',
        [ordered]@{
            name  = 'http'
            hosts = @('http://127.0.0.1:37421')
        },
        'notifications'
    )
    activationEvents = @('onStartup')
    # binaries[] REMOVED in v0.3.0 — sidecar owned by Windows Task Scheduler
    contributes  = [ordered]@{
        commands = @(
            [ordered]@{
                id          = 'focus-sync.syncNow'
                title       = 'Focus Sync — Sync current path to all open dialogs'
                description = "Push Sigma's current folder to all detected file dialogs (Chrome / Word / VS Code / etc.)"
                icon        = 'mdi-target'
            },
            [ordered]@{
                id          = 'focus-sync.toggle'
                title       = 'Focus Sync — Enable / Disable'
                description = 'Toggle focus sync on/off (sidecar stays running but stops pushing paths when disabled)'
                icon        = 'mdi-pause-circle'
            },
            [ordered]@{
                id          = 'focus-sync.toggleAutoConfirm'
                title       = 'Focus Sync — Toggle Auto-Confirm'
                description = "Toggle auto-confirm mode (default OFF). When ON, the sidecar auto-clicks Save once per dialog session — useful when you want Sigma FM path changes to immediately save."
                icon        = 'mdi-flash-auto'
            }
        )
        toolbar = @(
            [ordered]@{
                id    = 'focus-sync.toolbar'
                title = 'Focus Sync'
                icon  = 'mdi-target'
                items = @(
                    [ordered]@{
                        id        = 'syncNow'
                        title     = 'Sync Now'
                        commandId = 'focus-sync.syncNow'
                    },
                    [ordered]@{ id = 'separator' },
                    [ordered]@{
                        id        = 'toggle'
                        title     = 'Enable / Disable'
                        commandId = 'focus-sync.toggle'
                    },
                    [ordered]@{ id = 'separator2' },
                    [ordered]@{
                        id        = 'toggleAutoConfirm'
                        title     = 'Auto-Confirm (default OFF)'
                        commandId = 'focus-sync.toggleAutoConfirm'
                    }
                )
            }
        )
    }
}

# ---- Build kizemo entry (preserve user state if updating) ----
$existing = $json.installedExtensions.'kizemo.focus-sync'
$installedAt = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()

# CRITICAL: `isLocal: true` is required to prevent Sigma FM's startup
# `removeUnapprovedInstalledExtensions()` from auto-removing this entry
# (it deletes any installed extension that's NOT in the marketplace registry
# and NOT marked as local — see runtime/extensions.ts:1884).
$kizemoEntry = [ordered]@{
    autoUpdate                = if ($existing) { $existing.autoUpdate } else { $true }
    enabled                   = if ($existing) { $existing.enabled } else { $true }
    installPendingDependencies = if ($existing) { $existing.installPendingDependencies } else { $false }
    installedAt               = if ($existing -and $existing.installedAt) { $existing.installedAt } else { $installedAt }
    manifest                  = $kizemoManifest
    settings                  = [ordered]@{
        customSettings    = [ordered]@{ __binaries = @{} }
        scopedDirectories = @()
    }
    version      = '0.5.5'
    isLocal      = $true
    localSourcePath = "$resolvedAppData\com.sigma-file-manager.app\extensions\kizemo.focus-sync"
}

# ---- Apply (add-only: preserve existing entry from Sigma FM normalize) ----
# Round-2 fix (2026-09-30): the previously-verified install works because
# Sigma FM normalizes the JSON (strips whitespace, possibly corrupts em-dash
# to `E9 88 A5 3F` due to its writer's encoding bug). If we re-serialize
# via ConvertTo-Json, we produce a DIFFERENT byte-level format that Sigma FM
# then has to re-normalize on next save — and in the meantime it may reject
# the entry as "new / never seen" and skip auto-activation.
#
# Lesson from 09-29 round-19c verified install: once Sigma FM has saved an
# entry in its normalized state, leave it alone. Only ADD if missing.
$hasExisting = $json.installedExtensions.PSObject.Properties.Name -contains 'kizemo.focus-sync'

# ---- Populate __binaries (Sigma FM's binary registry) ----
# Sigma FM reads `settings.customSettings.__binaries[<binary_id>].path` to resolve
# binary paths at extension activation time (see sigma-file-manager/src/modules/extensions/
# api/create-binary-api.ts::getPath). Without this, the extension's `sigma.binary.getPath()`
# returns null even if the binary file exists on disk.
#
# Canonical path: %APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe
# Legacy shared:    %APPDATA%\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0\focus-sync-sidecar.exe
$canonicalBin = Join-Path "${resolvedAppData}\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar" 'focus-sync-sidecar.exe'
$sharedBin     = Join-Path "${resolvedAppData}\com.sigma-file-manager.app\binaries\focus-sync-sidecar\0.2.0" 'focus-sync-sidecar.exe'
$binPath = $null
if (Test-Path $canonicalBin) { $binPath = $canonicalBin }
elseif (Test-Path $sharedBin) { $binPath = $sharedBin }
else {
    Write-Host "==> Sidecar binary not at APPDATA path (expected in v0.3.0 — sidecar owned by Scheduled Task at $INSTDIR\tools\)"
}
$now = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
$binEntry = [ordered]@{
    id               = 'focus-sync-sidecar'
    path             = $binPath  # null in v0.3.0; extension v0.3.0 does NOT call sigma.binary.getPath
    version          = '0.5.5'
    repository       = 'https://github.com/kizemo/focus-sync'
    downloadUrl      = ''  # empty in v0.3.0 (no remote download; binary bundled in installer)
    installedAt      = $now
    latestVersion    = '0.5.5'
    latestCheckedAt  = $now
    hasUpdate        = $false
    source           = 'managed'
}
$kizemoEntry.settings = [ordered]@{
    customSettings    = [ordered]@{
        __binaries = [ordered]@{ 'focus-sync-sidecar' = $binEntry }
    }
    scopedDirectories = @()
}

if ($hasExisting) {
    $existingVersion = $json.installedExtensions.'kizemo.focus-sync'.version
    if ($existingVersion -ne '0.5.5') {
        # focus-17 fix: detect v0.2.0 (or older) entry and upgrade ONLY the
        # version fields in-place. Do NOT re-serialize the entire entry —
        # round 19c showed Sigma FM normalizes its JSON and re-serializing
        # breaks auto-activation.
        #
        # We patch two string fields:
        #   1. installedExtensions[id].version          (Sigma FM UI reads this)
        #   2. installedExtensions[id].manifest.version (Sigma FM also reads this)
        # All other fields (enabled, autoUpdate, settings, etc.) are preserved
        # with the byte-format Sigma FM normalized.
        $json.installedExtensions.'kizemo.focus-sync'.version = '0.5.5'
        $json.installedExtensions.'kizemo.focus-sync'.manifest.version = '0.5.5'
        Write-Host "==> UPGRADED kizemo.focus-sync entry: v$existingVersion -> v0.5.4 (other fields preserved per round 19c)"
    } else {
        Write-Host "==> kizemo.focus-sync entry ALREADY EXISTS at v0.5.4 - preserving as-is (Sigma FM normalized)"
        Write-Host "    version: $($json.installedExtensions.'kizemo.focus-sync'.version)"
        Write-Host "    enabled: $($json.installedExtensions.'kizemo.focus-sync'.enabled)"
    }
} else {
    $json.installedExtensions | Add-Member -NotePropertyName 'kizemo.focus-sync' -NotePropertyValue $kizemoEntry -Force
    Write-Host "==> Added new kizemo.focus-sync entry (v0.5.4 manifest, no binaries segment)"
}

# ---- Serialize (PowerShell ConvertTo-Json uses 4-space indent + padding;
# Sigma FM's own writer uses 2-space. Either format is valid JSON; Sigma FM
# will re-normalize the file on its next save, so we don't need to match
# its exact format here.) ----
$serialized = $json | ConvertTo-Json -Depth 100

# ---- Write back, no BOM, UTF-8 ----
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($jsonPath, $serialized, $utf8NoBom)

Write-Host "==> Wrote $jsonPath ($((Get-Item $jsonPath).Length) bytes)"
Write-Host "==> Done. Restart Sigma File Manager to see Focus Sync in the Installed tab."
exit 0