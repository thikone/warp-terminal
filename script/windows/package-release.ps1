<#
.SYNOPSIS
    Stage, archive and describe a portable terminal-only release.

.DESCRIPTION
    Produces build\pkg-<MMdd>\warp-terminal-<tag>-windows-x64[.zip], its SHA-256,
    and release notes written by `claude` from the commits since the previous
    release tag.

    The tag is read out of the built exe's ProductVersion rather than derived
    here, so the archive can never claim a version the binary does not have. Pass
    -Build to derive the next tag and build it first.

    Every file is copied from its canonical source in the repo. Nothing is taken
    from build\portable\, which holds live user state -- settings, the sqlite
    database and its WAL, logs and telemetry. The staged folder is audited for
    those before anything is archived.

    No git tag is created and nothing is pushed. Unpushed commits are reported,
    because a release tag on a commit nobody can fetch is worse than no tag.

.PARAMETER Build
    Derive the next unused tag and build it before packaging.

.PARAMETER Tag
    With -Build, the tag to build instead of the derived one. Without -Build the
    tag always comes from the binary and this is ignored.

.PARAMETER NoNotes
    Skip the `claude` call and use the commit subjects as the changes list.

.PARAMETER Force
    Package even if the binary fails its PE subsystem check.

.EXAMPLE
    .\script\windows\package-release.ps1

.EXAMPLE
    .\script\windows\package-release.ps1 -Build
#>

Param(
    [Switch]$Build,
    [String]$Tag,
    [Switch]$NoNotes,
    [Switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'release-tag.ps1')

$RepoRoot     = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Channel      = 'stable'
$BuildProfile = 'rlto'
$Branch       = 'terminal-only'

function Write-Step { param([String]$Text) Write-Host "`n$Text" -ForegroundColor Cyan }

Push-Location $RepoRoot
try {
    # ---- build (optional) ------------------------------------------------
    if ($Build) {
        $buildTag = if ($Tag) { $Tag } else { Get-NextReleaseTag }
        & (Join-Path $PSScriptRoot 'build-portable.ps1') -Tag $buildTag
    }

    # ---- identify the binary --------------------------------------------
    $exe = Join-Path $RepoRoot "target\$BuildProfile\warp-oss.exe"
    if (-not (Test-Path $exe)) {
        throw "No binary at $exe. Run build-portable.ps1 first, or pass -Build."
    }

    $binaryTag = (Get-Item $exe).VersionInfo.ProductVersion
    if (-not (Test-ReleaseTag $binaryTag)) {
        throw "Binary reports ProductVersion '$binaryTag', which is not a release tag. It was built without GIT_RELEASE_TAG -- rebuild with build-portable.ps1."
    }

    $subsystem = Get-PeSubsystem $exe
    if ($subsystem -ne 2) {
        $complaint = "Binary links as PE subsystem $subsystem (3 = CONSOLE), so it was built without the release_bundle feature: a console window will flash and assets are not embedded."
        if (-not $Force) { throw "$complaint Rebuild with build-portable.ps1, or pass -Force." }
        Write-Warning $complaint
    }

    Write-Step "Packaging $binaryTag"
    Write-Host ("  binary   {0} MB" -f [math]::Round((Get-Item $exe).Length / 1MB, 1))

    # ---- changelog baseline ---------------------------------------------
    $previousTag = @(Get-ReleaseTags) |
        Where-Object { $_ -ne $binaryTag } |
        Select-Object -Last 1

    $commits = @()
    if ($previousTag) {
        $records = (@(git log --no-merges --format="%h%x1f%s%x1f%b%x1e" "$previousTag..HEAD") -join "`n") -split "`u{1e}"
        $commits = @($records | Where-Object { $_.Trim() } | ForEach-Object {
            $parts = $_.Trim() -split "`u{1f}"
            [PSCustomObject]@{ Hash = $parts[0]; Subject = $parts[1]; Body = $parts[2] }
        })
        Write-Host "  since    $previousTag ($($commits.Count) commits)"
    } else {
        Write-Host '  since    (no previous release tag reachable from HEAD)'
    }

    # ---- stage -----------------------------------------------------------
    if ($binaryTag -notmatch '^v0\.\d{4}\.(\d{2})\.(\d{2})\.') {
        throw "Cannot read a date out of '$binaryTag'."
    }
    $pkgRoot = Join-Path $RepoRoot ('build\pkg-{0}{1}' -f $Matches[1], $Matches[2])
    $name    = "warp-terminal-$binaryTag-windows-x64"
    $stage   = Join-Path $pkgRoot $name
    $zip     = Join-Path $pkgRoot "$name.zip"

    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $stage, "$stage\x64", "$stage\data" | Out-Null

    # target\<profile>\ holds the exe plus the DLLs app/build.rs staged beside it.
    foreach ($file in 'warp-oss.exe', 'conpty.dll', 'dxcompiler.dll', 'dxil.dll') {
        Copy-Item (Join-Path $RepoRoot "target\$BuildProfile\$file") (Join-Path $stage $file)
    }
    Copy-Item (Join-Path $RepoRoot "target\$BuildProfile\x64\OpenConsole.exe") "$stage\x64\OpenConsole.exe"

    foreach ($file in 'msvcp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll') {
        Copy-Item (Join-Path $RepoRoot "app\assets\windows\x64\$file") (Join-Path $stage $file)
    }

    Copy-Item (Join-Path $RepoRoot 'app\assets\bundled\bootstrap\pwsh.ps1') "$stage\pwsh.ps1"
    Copy-Item (Join-Path $RepoRoot "app\channels\$Channel\icon\no-padding\icon.ico") "$stage\icon.ico"
    Copy-Item (Join-Path $RepoRoot 'script\windows\portable\install.ps1') "$stage\install.ps1"
    Copy-Item (Join-Path $RepoRoot 'script\windows\portable\uninstall.ps1') "$stage\uninstall.ps1"

    @'
This folder makes Warp portable: its presence next to warp-oss.exe redirects
settings, state, cache and logs here instead of your user profile.

Delete this folder to use the normal per-user locations instead.
'@ | Set-Content "$stage\data\README.txt" -Encoding utf8

    # ---- audit -----------------------------------------------------------
    $leaked = Get-ChildItem $stage -Recurse -File | Where-Object {
        $_.Name -match 'settings\.toml|keybindings\.yaml|warp\.sqlite|telemetry|credential|\.log(\.old\.\d+)?$|\.pdb$'
    }
    if ($leaked) {
        Remove-Item $stage -Recurse -Force
        throw ("User data would have shipped; staging discarded:`n  " + (($leaked | ForEach-Object { $_.FullName }) -join "`n  "))
    }

    Get-ChildItem $stage -Recurse -File | Unblock-File
    Write-Host "  staged   $(@(Get-ChildItem $stage -Recurse -File).Count) files, no user data"

    # ---- archive ---------------------------------------------------------
    Write-Step 'Archiving'
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path $stage -DestinationPath $zip -CompressionLevel Optimal
    $zipItem = Get-Item $zip
    $sha256  = (Get-FileHash $zip -Algorithm SHA256).Hash
    Write-Host ("  {0} MB" -f [math]::Round($zipItem.Length / 1MB, 1))
    Write-Host "  $sha256"

    # ---- release notes ---------------------------------------------------
    Write-Step 'Release notes'

    if ($commits.Count -eq 0) {
        $changes = '- No code changes since the previous release.'
        Write-Host '  nothing to describe'
    } elseif ($NoNotes) {
        $changes = ($commits | ForEach-Object { "- $($_.Subject)" }) -join "`n"
        Write-Host '  -NoNotes: using commit subjects'
    } else {
        $log = ($commits | ForEach-Object { "## $($_.Subject)`n$($_.Body)".Trim() }) -join "`n`n"

        $prompt = @"
You are writing the "what changed" section of a GitHub release for a
stripped-down, terminal-only fork of the Warp terminal. It ships as a portable
Windows x64 folder.

Below are the commits since the previous release, $previousTag, each with its
subject and body. Turn them into release notes.

Rules:
- Output only a markdown bullet list. No heading, no preamble, no sign-off.
- Write for someone deciding whether to upgrade: describe the behaviour they
  will notice, not the implementation.
- Group related commits into a single bullet. Put what matters most first.
- Omit commits with no user-visible effect (refactors, formatting, test-only or
  build-only changes) unless nothing else would remain.
- No commit hashes, no file paths, no function names.
- At most two sentences per bullet.

Commits:

$log
"@

        try {
            $changes = ($prompt | claude -p --output-format text 2>&1 | Out-String).Trim()
            if ($LASTEXITCODE -ne 0 -or -not $changes) { throw "claude exited with $LASTEXITCODE" }
            Write-Host "  written by claude from $($commits.Count) commits"
        } catch {
            Write-Warning "claude failed ($_); falling back to commit subjects."
            $changes = ($commits | ForEach-Object { "- $($_.Subject)" }) -join "`n"
        }
    }

    $notes = @"
Windows x64 portable build of the terminal-only fork.

$changes

Extract anywhere and run ``warp-oss.exe`` -- no installer. Settings live in
``data\`` next to the exe; delete that folder to use the normal per-user location
instead. ``install.ps1`` is optional: it only adds shortcuts and a PATH entry.

The binary is unsigned, so SmartScreen will warn on first run --
"More info" then "Run anyway".

SHA-256 (zip):
``$sha256``
"@

    $notesPath = Join-Path $pkgRoot "$name.RELEASE_NOTES.md"
    $notes | Set-Content $notesPath -Encoding utf8

    # ---- report ----------------------------------------------------------
    Write-Step 'Done'
    Write-Host "  zip      $zip"
    Write-Host "  notes    $notesPath"
    Write-Host "  tag      $binaryTag"
    Write-Host "  next     $(Get-NextReleaseTag)   (for the release after this one)"

    git rev-parse --verify --quiet "origin/$Branch" 2>$null | Out-Null
    if ($LASTEXITCODE -eq 0) {
        $unpushed = @(git log --oneline "origin/$Branch..HEAD")
        if ($unpushed.Count) {
            Write-Host ''
            Write-Warning "$($unpushed.Count) commit(s) not on origin/$Branch. Push before publishing, or the release tag will point at a commit nobody can fetch:"
            $unpushed | ForEach-Object { Write-Host "    $_" }
        }
    } else {
        Write-Host ''
        Write-Warning "No origin/$Branch ref locally, so unpushed commits could not be checked."
    }

    Write-Host ''
    Write-Host 'Publish:' -ForegroundColor Cyan
    Write-Host "  1. push $Branch"
    Write-Host "  2. new release, tag $binaryTag, target $Branch"
    Write-Host "  3. attach the zip, paste the notes"
} finally {
    Pop-Location
}
