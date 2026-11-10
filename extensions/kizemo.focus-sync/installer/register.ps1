# ============================================================================
# register.ps1 - Register kizemo.focus-sync in Alpha File Manager's user-extensions.json
# ============================================================================
# This script is invoked by installer.nsi after copying the extension files.
# It edits %APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json
# to add (or refresh) the kizemo.focus-sync entry under installedExtensions.
#
# IMPORTANT: Close Sigma File Manager BEFORE running this script.
# Alpha File Manager uses a one-shot startup bootstrap from user-extensions.json. If it
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

# ---------------------------------------------------------------------------
# v0.5.9 (2026-10-30) encoding fix.
#
# This file is UTF-8 without BOM, and PowerShell 5.1 parses BOM-less scripts
# using the system ANSI code page (cp936 on this machine). Any non-ASCII
# literal in the source therefore lands in the output as mojibake: the
# em-dash (U+2014, UTF-8 bytes E2 80 94) decoded as cp936 became "U+95C2 + ?",
# which is exactly the "Focus Sync U+95C2?Sync current path" garbage that
# landed in user-extensions.json.
#
# Fix: keep the source pure ASCII and inject the em-dash at runtime via
# [char]0x2014. Everything below is ASCII-only on purpose.
# ---------------------------------------------------------------------------
$emdash = [char]0x2014
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

# ---- Extension version: single source of truth ----
# Read the version from the DEPLOYED package.json. installer.nsi copies
# package.json into the extension folder in Stage 3, and this script runs in
# Stage 5, so the deployed file is the shipped artifact and its version is
# authoritative. The hardcoded '0.5.5' that used to live in this file had
# drifted behind package.json (0.5.9) for several releases; deriving it from
# the deployed manifest makes that class of drift impossible.
# NO hardcoded fallback: a literal here is exactly the drift the comment above
# warns about (it said '0.5.5' while package.json said 0.5.9). Fall back to the
# copy shipped next to this script instead, which installer.nsi always stages.
$extensionVersion = ''
$deployedPackageJson = Join-Path $appDataDir 'extensions\kizemo.focus-sync\package.json'
if (Test-Path $deployedPackageJson) {
    try {
        $pkg = [System.IO.File]::ReadAllText($deployedPackageJson, [System.Text.Encoding]::UTF8) | ConvertFrom-Json
        if ($pkg.version) { $extensionVersion = [string]$pkg.version }
    } catch {
        Write-Host "==> WARNING: could not read version from $deployedPackageJson : $_"
    }
}
# Second source: the package.json shipped beside this script. Covers a first
# install where Stage 3 has not staged the deployed copy yet.
if (-not $extensionVersion) {
    $shippedPackageJson = Join-Path $PSScriptRoot '..\package.json'
    if (Test-Path $shippedPackageJson) {
        try {
            $pkg = [System.IO.File]::ReadAllText($shippedPackageJson, [System.Text.Encoding]::UTF8) | ConvertFrom-Json
            if ($pkg.version) { $extensionVersion = [string]$pkg.version }
        } catch {
            Write-Host "==> WARNING: could not read version from $shippedPackageJson : $_"
        }
    }
}
# No version anywhere => refuse to write a manifest with an empty version.
# Fail loudly instead of registering an extension loaders cannot upgrade.
if (-not $extensionVersion) {
    Write-Host "==> FATAL: no extension version found (deployed=$deployedPackageJson, shipped=$PSScriptRoot\..\package.json)"
    exit 1
}
Write-Host "==> Extension version: $extensionVersion"

# ---- Sandbox gate (focus-21 Phase 4) ----
# register.ps1 does not copy index.js itself (installer.nsi copies the files
# before invoking this script), so we validate the DEPLOYED copy - the exact
# file Alpha File Manager's loader.ts will read on next startup.
# If any sandbox regex matches (even inside a comment), validateExtensionCode()
# rejects the extension and loader.ts throws BEFORE `new Worker()`: the
# extension silently never loads - no UI error, no notification, sidecar 0
# requests. Rules are read LIVE from sandbox.ts, so this gate never goes stale.
$sandboxScanner  = Join-Path $PSScriptRoot '..\..\scripts\scan-sandbox-dynamic.cjs'
$sandboxTs       = Join-Path $PSScriptRoot '..\..\sigma-file-manager\src\modules\extensions\runtime\sandbox.ts'
$deployedIndexJs = Join-Path $appDataDir 'extensions\kizemo.focus-sync\dist\index.js'
if ((Test-Path $sandboxScanner) -and (Test-Path $deployedIndexJs) -and (Test-Path $sandboxTs)) {
    Write-Host "==> Validating extension against Alpha File Manager sandbox (dynamic)"
    # Pass sandbox.ts explicitly: the scanner's default is CWD-relative, so
    # relying on it would FATAL (exit 2) on a machine whose CWD is elsewhere.
    & node $sandboxScanner $deployedIndexJs $sandboxTs
    if ($LASTEXITCODE -ne 0) {
        Write-Error @"
SANDBOX VALIDATION FAILED (exit $LASTEXITCODE).
Refusing to register an extension Alpha File Manager will reject. It would load
SILENTLY-NEVER - no UI error, no notification, sidecar gets 0 requests.
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
# silently failed on fresh installs (Alpha File Manager never launched) and on
# %APPDATA% trees wiped between uninstall and install. Bootstrap them here.
if (-not (Test-Path $appDataDir)) {
    Write-Host "==> appDataDir missing: $appDataDir - creating."
    New-Item -ItemType Directory -Path $appDataDir -Force | Out-Null
}
if (-not (Test-Path $userDataDir)) {
    Write-Host "==> userDataDir missing: $userDataDir - creating."
    New-Item -ItemType Directory -Path $userDataDir -Force | Out-Null
}
if (-not (Test-Path $jsonPath)) {
    Write-Host "==> user-extensions.json missing - bootstrapping with empty structure."
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
# v0.5.9 (2026-10-30): read the JSON as UTF-8 explicitly.
# Get-Content -Raw defaults to the ANSI code page in PS 5.1, which turns
# any non-ASCII byte sequence in the file into mojibake before
# ConvertFrom-Json ever sees it.
$json = [System.IO.File]::ReadAllText($jsonPath, [System.Text.Encoding]::UTF8) | ConvertFrom-Json

# ---- Build kizemo manifest (in declared field order) ----
$kizemoManifest = [ordered]@{
    id           = 'kizemo.focus-sync'
    name         = 'Focus Sync'
    version      = $extensionVersion
    description  = "Listary-style global focus sync. The sidecar runs as a child process of Alpha File Manager and exits with the app."
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
        # 'shell' REMOVED in v0.3.0 - sidecar no longer spawned by Alpha File Manager
        'commands',
        'toolbar',
        [ordered]@{
            name  = 'http'
            hosts = @('http://127.0.0.1:37421')
        },
        'notifications'
    )
    activationEvents = @('onStartup')
    # binaries[] REMOVED in v0.3.0 - sidecar owned by Windows Task Scheduler
    contributes  = [ordered]@{
        commands = @(
            [ordered]@{
                id          = 'focus-sync.syncNow'
                title       = "Focus Sync $emdash Sync current path to all open dialogs"
                description = "Push Sigma's current folder to all detected file dialogs (Chrome / Word / VS Code / etc.)"
                icon        = 'mdi-target'
            },
            [ordered]@{
                id          = 'focus-sync.toggle'
                title       = "Focus Sync $emdash Enable / Disable"
                description = 'Toggle focus sync on/off (sidecar stays running but stops pushing paths when disabled)'
                icon        = 'mdi-pause-circle'
            },
            [ordered]@{
                id          = 'focus-sync.toggleAutoConfirm'
                title       = "Focus Sync $emdash Toggle Auto-Confirm"
                description = "Toggle auto-confirm mode (default OFF). When ON, the sidecar auto-clicks Save once per dialog session $emdash useful when you want Alpha File Manager path changes to immediately save."
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

# CRITICAL: `isLocal: true` is required to prevent Alpha File Manager's startup
# `removeUnapprovedInstalledExtensions()` from auto-removing this entry
# (it deletes any installed extension that's NOT in the marketplace registry
# and NOT marked as local - see runtime/extensions.ts:1884).
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
    version      = $extensionVersion
    isLocal      = $true
    localSourcePath = "$resolvedAppData\com.sigma-file-manager.app\extensions\kizemo.focus-sync"
}

# ---- Apply (add-only: preserve existing entry from Alpha File Manager normalize) ----
# Round-2 fix (2026-09-30): the previously-verified install works because
# Alpha File Manager normalizes the JSON (strips whitespace, possibly corrupts em-dash
# to `E9 88 A5 3F` due to its writer's encoding bug). If we re-serialize
# via ConvertTo-Json, we produce a DIFFERENT byte-level format that Alpha File Manager
# then has to re-normalize on next save - and in the meantime it may reject
# the entry as "new / never seen" and skip auto-activation.
#
# Lesson from 09-29 round-19c verified install: once Alpha File Manager has saved an
# entry in its normalized state, leave it alone. Only ADD if missing.
$hasExisting = $json.installedExtensions.PSObject.Properties.Name -contains 'kizemo.focus-sync'

# ---- Populate __binaries (Alpha File Manager's binary registry) ----
# Alpha File Manager reads `settings.customSettings.__binaries[<binary_id>].path` to resolve
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
    Write-Host "==> Sidecar binary not at APPDATA path (expected: the sidecar is owned by the Alpha File Manager main process)"
}
$now = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
$binEntry = [ordered]@{
    id               = 'focus-sync-sidecar'
    path             = $binPath  # null in v0.3.0; extension v0.3.0 does NOT call sigma.binary.getPath
    version          = $extensionVersion
    repository       = 'https://github.com/kizemo/focus-sync'
    downloadUrl      = ''  # empty in v0.3.0 (no remote download; binary bundled in installer)
    installedAt      = $now
    latestVersion    = $extensionVersion
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
    # Patch a stale manifest description IN PLACE.
    # The description is user-visible in the Installed tab, and older installs
    # shipped the text "Sidecar runs as a Windows Scheduled Task" - which
    # describes the RETIRED architecture and misleads users about who owns the
    # sidecar now. Same in-place string-field technique as the version patch
    # below: only this one field is rewritten, the entry is not re-serialized.
    $canonicalDescription = "Listary-style global focus sync. The sidecar runs as a child process of Alpha File Manager and exits with the app."
    $existingManifest = $json.installedExtensions.'kizemo.focus-sync'.manifest
    if ($existingManifest -and $existingManifest.description -ne $canonicalDescription) {
        $existingManifest.description = $canonicalDescription
        Write-Host "==> Patched stale manifest description (sidecar is a child process of Alpha File Manager)"
    }
    $existingVersion = $json.installedExtensions.'kizemo.focus-sync'.version
    if ($existingVersion -ne $extensionVersion) {
        # focus-17 fix: detect v0.2.0 (or older) entry and upgrade ONLY the
        # version fields in-place. Do NOT re-serialize the entire entry -
        # round 19c showed Alpha File Manager normalizes its JSON and re-serializing
        # breaks auto-activation.
        #
        # We patch two string fields:
        #   1. installedExtensions[id].version          (Alpha File Manager UI reads this)
        #   2. installedExtensions[id].manifest.version (Alpha File Manager also reads this)
        # All other fields (enabled, autoUpdate, settings, etc.) are preserved
        # with the byte-format Alpha File Manager normalized.
        $json.installedExtensions.'kizemo.focus-sync'.version = $extensionVersion
        $json.installedExtensions.'kizemo.focus-sync'.manifest.version = $extensionVersion
        Write-Host "==> UPGRADED kizemo.focus-sync entry: v$existingVersion -> v$extensionVersion (other fields preserved per round 19c)"
    } else {
        Write-Host "==> kizemo.focus-sync entry ALREADY EXISTS at v$extensionVersion - preserving as-is (Alpha File Manager normalized)"
        # v0.5.9 (2026-10-30): repair mojibake titles left behind by the
        # ANSI-decode bug. The mojibake sequence is the em-dash decoded as
        # cp936 (U+95C2 followed by '?'). We rewrite the three known command
        # titles in place by id, and leave every other field untouched.
        $canonicalTitles = @{
            'focus-sync.syncNow'         = "Focus Sync $emdash Sync current path to all open dialogs"
            'focus-sync.toggle'           = "Focus Sync $emdash Enable / Disable"
            'focus-sync.toggleAutoConfirm'= "Focus Sync $emdash Toggle Auto-Confirm"
        }
        $existingCommands = $json.installedExtensions.'kizemo.focus-sync'.manifest.contributes.commands
        if ($existingCommands) {
            foreach ($cmd in $existingCommands) {
                if ($canonicalTitles.ContainsKey($cmd.id)) {
                    if ($cmd.title -ne $canonicalTitles[$cmd.id]) {
                        Write-Host "==> Repairing mojibake title for '$($cmd.id)'"
                        $cmd.title = $canonicalTitles[$cmd.id]
                    }
                }
            }
        }
        Write-Host "    version: $($json.installedExtensions.'kizemo.focus-sync'.version)"
        Write-Host "    enabled: $($json.installedExtensions.'kizemo.focus-sync'.enabled)"
    }
} else {
    $json.installedExtensions | Add-Member -NotePropertyName 'kizemo.focus-sync' -NotePropertyValue $kizemoEntry -Force
    Write-Host "==> Added new kizemo.focus-sync entry (v$extensionVersion manifest, no binaries segment)"
}

# ---- Serialize (PowerShell ConvertTo-Json uses 4-space indent + padding;
# Alpha File Manager's own writer uses 2-space. Either format is valid JSON; Alpha File Manager
# will re-normalize the file on its next save, so we don't need to match
# its exact format here.) ----
$serialized = $json | ConvertTo-Json -Depth 100

# ---- Write back, no BOM, UTF-8 ----
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($jsonPath, $serialized, $utf8NoBom)

Write-Host "==> Wrote $jsonPath ($((Get-Item $jsonPath).Length) bytes)"
Write-Host "==> Done. Restart Sigma File Manager to see Focus Sync in the Installed tab."
exit 0