<#
.SYNOPSIS
    Optional convenience setup for the portable terminal-only Warp build.

.DESCRIPTION
    The portable folder runs as-is -- this script is not required. It only adds
    the conveniences a real installer would provide, all at user scope so no
    administrator rights are needed and nothing outside your profile is touched:

      * clears Mark-of-the-Web from every file (skips the SmartScreen prompt, and
        matters on machines whose Group Policy forces a RemoteSigned policy)
      * creates a Start Menu shortcut
      * optionally a Desktop shortcut (-Desktop)
      * optionally adds this folder to your user PATH (-AddToPath)

    The Explorer "Open Warp in new tab/window" entries are NOT set up here --
    the app claims those itself on every startup, so they always point at the
    executable you actually ran.

    Run uninstall.ps1 to reverse everything, including those entries.

.PARAMETER Desktop
    Also create a Desktop shortcut.

.PARAMETER AddToPath
    Add this folder to the user PATH so `warp-oss` runs from any shell.
    Off by default, since editing PATH is the least "portable" thing here.

.PARAMETER Quiet
    Suppress the summary output.

.EXAMPLE
    .\install.ps1
    .\install.ps1 -Desktop -AddToPath
#>
[CmdletBinding()]
param(
    [switch]$Desktop,
    [switch]$AddToPath,
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'

$Root         = $PSScriptRoot
$ExePath      = Join-Path $Root 'warp-oss.exe'
$IconPath     = Join-Path $Root 'icon.ico'
$ShortcutName = 'Warp Terminal.lnk'
$Changes      = [System.Collections.Generic.List[string]]::new()

function Say($m) { if (-not $Quiet) { Write-Host $m } }

if (-not (Test-Path $ExePath)) {
    throw "warp-oss.exe not found next to this script ($Root). Run install.ps1 from inside the portable folder."
}

# --- 1. Mark-of-the-Web ------------------------------------------------------
# A folder that arrived as a zip download carries ZoneId=3 on every extracted
# file. The build already sets a process-scope Bypass before sourcing its own
# bootstrap, so the terminal works regardless -- but clearing MOTW also avoids
# the SmartScreen prompt on the exe and helps if Group Policy pins the policy
# to RemoteSigned (where a process-scope Bypass cannot win).
$blocked = Get-ChildItem -Recurse -File $Root |
    Where-Object { Get-Item $_.FullName -Stream Zone.Identifier -ErrorAction SilentlyContinue }
if ($blocked) {
    $blocked | Unblock-File
    $Changes.Add("Unblocked $($blocked.Count) file(s) (Mark-of-the-Web cleared)")
} else {
    Say 'Mark-of-the-Web: nothing to clear.'
}

# --- 2. Shortcuts ------------------------------------------------------------
function New-WarpShortcut {
    param([string]$Directory, [string]$Label)

    $linkPath = Join-Path $Directory $ShortcutName
    $shell    = New-Object -ComObject WScript.Shell
    $sc       = $shell.CreateShortcut($linkPath)
    $sc.TargetPath       = $ExePath
    $sc.WorkingDirectory = $Root
    $sc.Description      = 'Warp Terminal (portable, terminal-only build)'
    if (Test-Path $IconPath) { $sc.IconLocation = $IconPath } else { $sc.IconLocation = $ExePath }
    $sc.Save()
    $Changes.Add("$Label shortcut -> $linkPath")
}

New-WarpShortcut -Directory ([Environment]::GetFolderPath('Programs')) -Label 'Start Menu'
if ($Desktop) {
    New-WarpShortcut -Directory ([Environment]::GetFolderPath('Desktop')) -Label 'Desktop'
}

# --- 3. Explorer context menu ------------------------------------------------
# Nothing to do: the app claims "Open Warp in new tab/window" itself on every
# startup (app_services::windows::registry::register_context_menu_entries), so
# the entries always point at the executable you actually launched. Writing them
# from here risks recording a stale path, which is worse than useless -- it
# starts a second binary that fails to hand off and can kill the running one.
#
# uninstall.ps1 removes the entries if you want them gone.

# --- 4. User PATH ------------------------------------------------------------
if ($AddToPath) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries  = @($userPath -split ';' | Where-Object { $_ })
    if ($entries -contains $Root) {
        Say 'PATH: already present.'
    } else {
        $new = (@($entries) + $Root) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $new, 'User')
        $Changes.Add("Added to user PATH: $Root  (restart shells to pick it up)")
    }
}

# --- 4. Execution-policy advisory -------------------------------------------
# The only remaining way the shell integration can fail on a new machine.
$machine = Get-ExecutionPolicy -Scope MachinePolicy
$user    = Get-ExecutionPolicy -Scope UserPolicy
$warning = $null
if ($machine -eq 'Restricted' -or $user -eq 'Restricted') {
    $warning = "Group Policy sets ExecutionPolicy=Restricted. Warp's shell integration cannot be enabled; the shell will run un-Warpified."
} elseif ($machine -eq 'AllSigned' -or $user -eq 'AllSigned') {
    $warning = "Group Policy sets ExecutionPolicy=AllSigned. This build's bootstrap script is unsigned, so shell integration may not load."
}

# --- summary -----------------------------------------------------------------
if (-not $Quiet) {
    Write-Host ''
    if ($Changes.Count -eq 0) {
        Write-Host 'Nothing to do - already set up.'
    } else {
        Write-Host 'Done:'
        $Changes | ForEach-Object { Write-Host "  - $_" }
    }
    if ($warning) {
        Write-Host ''
        Write-Warning $warning
    }
    Write-Host ''
    $dataDir = Join-Path $Root 'data'
    if (Test-Path $dataDir) {
        Write-Host "Portable mode is ON. Settings and history live in: $dataDir"
        Write-Host 'Delete that folder to fall back to the per-user profile instead.'
    } else {
        Write-Host "Portable mode is OFF. Settings and history live in: $env:LOCALAPPDATA\warp\WarpOss"
        Write-Host "Create a 'data' folder next to warp-oss.exe to switch to portable mode."
    }
    Write-Host 'Reverse all of this with: .\uninstall.ps1'
}
