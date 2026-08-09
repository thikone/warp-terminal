<#
.SYNOPSIS
    Reverses install.ps1 for the portable terminal-only Warp build.

.DESCRIPTION
    Removes the shortcuts and the user PATH entry that install.ps1 created.

    It does NOT delete the portable folder itself -- delete that yourself when
    you are done with it.

    It does NOT delete your settings, history or logs unless you pass
    -RemoveUserData, which is destructive and asks first.

.PARAMETER RemoveUserData
    Also delete %LOCALAPPDATA%\warp\WarpOss (settings.toml, command history,
    the SQLite database and logs). Prompts before deleting.

.PARAMETER Force
    Skip the confirmation prompt for -RemoveUserData. No effect on its own.

.PARAMETER Quiet
    Suppress the summary output.

.EXAMPLE
    .\uninstall.ps1
    .\uninstall.ps1 -RemoveUserData
#>
[CmdletBinding()]
param(
    [switch]$RemoveUserData,
    [switch]$Force,
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'

$Root         = $PSScriptRoot
$ShortcutName = 'Warp Terminal.lnk'
$ExePath      = Join-Path $Root 'warp-oss.exe'
$Changes      = [System.Collections.Generic.List[string]]::new()

function Say($m) { if (-not $Quiet) { Write-Host $m } }

# --- 1. Shortcuts ------------------------------------------------------------
# Only remove shortcuts that actually point at THIS folder, so a second copy of
# the portable build elsewhere keeps its own Start Menu entry.
foreach ($dir in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))) {
    $linkPath = Join-Path $dir $ShortcutName
    if (-not (Test-Path $linkPath)) { continue }
    try {
        $shell  = New-Object -ComObject WScript.Shell
        $target = $shell.CreateShortcut($linkPath).TargetPath
    } catch {
        $target = $null
    }
    if ($target -and $target -ne $ExePath) {
        Say "Skipping $linkPath (points at another install: $target)"
        continue
    }
    Remove-Item $linkPath -Force
    $Changes.Add("Removed shortcut: $linkPath")
}

# --- 2. User PATH ------------------------------------------------------------
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath) {
    $entries = @($userPath -split ';' | Where-Object { $_ })
    $kept    = @($entries | Where-Object { $_ -ne $Root })
    if ($kept.Count -ne $entries.Count) {
        [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')
        $Changes.Add("Removed from user PATH: $Root  (restart shells to pick it up)")
    }
}

# --- 3. Optional user data ---------------------------------------------------
# In portable mode user state lives next to the binary; otherwise it is in the
# per-user profile. Target whichever this install actually uses.
$portableData = Join-Path $Root 'data'
$dataDir = if (Test-Path $portableData) { $portableData } else { Join-Path $env:LOCALAPPDATA 'warp\WarpOss' }
if ($RemoveUserData) {
    if (Test-Path $dataDir) {
        $go = $Force
        if (-not $go) {
            Write-Host ''
            Write-Warning "This permanently deletes settings, command history, logs and the local database in:"
            Write-Host "  $dataDir"
            $go = (Read-Host 'Type DELETE to confirm') -ceq 'DELETE'
        }
        if ($go) {
            Remove-Item -Recurse -Force $dataDir
            $Changes.Add("Deleted user data: $dataDir")
        } else {
            Say 'User data left untouched.'
        }
    } else {
        Say "No user data found at $dataDir"
    }
}

# --- summary -----------------------------------------------------------------
if (-not $Quiet) {
    Write-Host ''
    if ($Changes.Count -eq 0) {
        Write-Host 'Nothing to undo.'
    } else {
        Write-Host 'Done:'
        $Changes | ForEach-Object { Write-Host "  - $_" }
    }
    if (-not $RemoveUserData -and (Test-Path $dataDir)) {
        Write-Host ''
        Write-Host "Settings and history kept in: $dataDir"
        Write-Host 'Pass -RemoveUserData to delete them too.'
    }
    Write-Host ''
    Write-Host "The portable folder itself was not removed. Delete it manually: $Root"
}
