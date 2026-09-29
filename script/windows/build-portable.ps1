<#
.SYNOPSIS
    Build the portable terminal-only Warp binary for Windows x64.

.DESCRIPTION
    Wraps the build recipe documented in README.md so the flags that matter
    cannot be forgotten:

      * --features release_bundle,gui -- `release_bundle` is required, not
        optional. It enables rust-embed's debug-embed, without which assets load
        from absolute build-machine paths and the binary is not portable, and it
        sets `windows_subsystem = "windows"`, without which a console window
        flashes on launch and lingers after exit.
      * GIT_RELEASE_TAG -- embedded as the exe's ProductVersion. Unset, the
        version resource reads a bare "v0" and package-release.ps1 cannot name
        the archive.
      * no --target triple -- app/build.rs stages the Windows DLLs into
        target/<profile>/, a path that excludes the triple. Passing it puts the
        exe somewhere the staged layout is not.

    Both invariants are verified against the linked binary afterwards, so a
    wrong build fails here rather than in someone's hands.

.PARAMETER Tag
    Release tag to embed. Defaults to the first unused tag for today, so a plain
    run always produces a distinct version. Pass an existing tag to rebuild that
    version.

.PARAMETER SkipVerify
    Skip the post-build checks on ProductVersion and PE subsystem.

.EXAMPLE
    .\script\windows\build-portable.ps1

.EXAMPLE
    .\script\windows\build-portable.ps1 -Tag v0.2026.09.29.12.00.terminal_01
#>

Param(
    [String]$Tag,
    [Switch]$SkipVerify
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'release-tag.ps1')

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Channel  = 'stable'   # picks app\channels\<name>\icon -> the exe icon
$BuildProfile  = 'rlto'

Push-Location $RepoRoot
try {
    if (-not $Tag) { $Tag = Get-NextReleaseTag }
    if (-not (Test-ReleaseTag $Tag)) {
        throw "Tag '$Tag' does not match v0.<yyyy>.<MM>.<dd>.<HH>.<MM>.terminal_<NN>."
    }

    Assert-WarpNotRunning

    $protoc = Resolve-Protoc
    if (-not $protoc) {
        throw 'protoc not found. Install it (winget install Google.Protobuf) or set $env:PROTOC.'
    }

    $env:PROTOC             = $protoc
    $env:CARGO_FULL_PROFILE = $BuildProfile
    $env:CARGO_BIN_NAME     = $Channel
    $env:WARP_APP_NAME      = 'WarpOss'
    $env:GIT_RELEASE_TAG    = $Tag

    Write-Host "Building $Tag" -ForegroundColor Cyan
    Write-Host "  profile  $BuildProfile"
    Write-Host "  channel  $Channel"
    Write-Host "  protoc   $protoc"
    Write-Host ''

    $started = Get-Date
    cargo build -p warp --profile $BuildProfile --bin warp-oss --features release_bundle,gui
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE." }

    $exe = Join-Path $RepoRoot "target\$BuildProfile\warp-oss.exe"
    if (-not (Test-Path $exe)) { throw "Build reported success but $exe is missing." }

    if (-not $SkipVerify) {
        $version = (Get-Item $exe).VersionInfo.ProductVersion
        if ($version -ne $Tag) {
            throw "Binary reports ProductVersion '$version' but '$Tag' was requested. GIT_RELEASE_TAG did not reach the build."
        }

        $subsystem = Get-PeSubsystem $exe
        if ($subsystem -ne 2) {
            throw "Binary links as PE subsystem $subsystem (3 = CONSOLE); expected 2 (WINDOWS). The release_bundle feature did not apply, so a console window would flash."
        }
    }

    $item    = Get-Item $exe
    $elapsed = (Get-Date) - $started

    Write-Host ''
    Write-Host 'Build OK' -ForegroundColor Green
    Write-Host ("  {0}" -f $exe)
    Write-Host ("  {0} MB   {1}   {2:mm\mss\s}" -f [math]::Round($item.Length / 1MB, 1), $Tag, $elapsed)
    if (-not $SkipVerify) {
        Write-Host '  verified: ProductVersion matches, PE subsystem 2 (no console)'
    }
    Write-Host ''
    Write-Host 'Package it with:' -ForegroundColor Cyan
    Write-Host '  .\script\windows\package-release.ps1'
} finally {
    Pop-Location
}
