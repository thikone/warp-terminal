<#
.SYNOPSIS
    Shared helpers for building and packaging the portable terminal-only build.

.DESCRIPTION
    Dot-sourced by build-portable.ps1 and package-release.ps1; not meant to be
    run on its own.

    Release tags have the shape `v0.<yyyy>.<MM>.<dd>.12.00.terminal_<NN>`, which
    is what every existing release tag uses. The time component is fixed rather
    than taken from the build clock, so a tag follows from its date alone;
    same-day re-releases are told apart by <NN>.
#>

Set-StrictMode -Version Latest

$script:ReleaseTagPattern = '^v0\.\d{4}\.\d{2}\.\d{2}\.\d{2}\.\d{2}\.terminal_\d{2}$'

function Test-ReleaseTag {
    param([Parameter(Mandatory)][AllowEmptyString()][String]$Tag)
    return $Tag -match $script:ReleaseTagPattern
}

function Get-ReleaseTags {
    <#
    Release tags reachable from HEAD, oldest first.

    Sorted by plain refname rather than version: every field is fixed-width and
    zero-padded, so lexicographic order is already chronological, and git's
    version sort does not reliably order the `terminal_NN` suffix.

    Unreachable tags are dropped. One left on an abandoned line would otherwise
    be picked as the changelog baseline and yield a meaningless commit range.
    #>
    $found = @()
    foreach ($tag in @(git tag -l '*terminal_*' --sort=refname)) {
        if (-not (Test-ReleaseTag $tag)) { continue }
        git merge-base --is-ancestor $tag HEAD 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { $found += $tag }
    }
    return $found
}

function Get-NextReleaseTag {
    <#
    The first unused tag for a date. Counts every tag with that date prefix, not
    just reachable ones, so a tag already published elsewhere is never reused.
    #>
    param([DateTime]$Date = (Get-Date))

    $prefix = 'v0.{0:yyyy}.{0:MM}.{0:dd}.12.00.terminal_' -f $Date
    $used = @()
    foreach ($tag in @(git tag -l "$prefix*")) {
        if ($tag -match '_(\d+)$') { $used += [int]$Matches[1] }
    }
    # Measure-Object hands back a Double, which the integer-only d2 specifier rejects.
    $next = if ($used.Count) { [int](($used | Measure-Object -Maximum).Maximum) + 1 } else { 1 }
    return ('{0}{1:d2}' -f $prefix, $next)
}

function Get-PeSubsystem {
    <#
    2 = WINDOWS (no console), 3 = CONSOLE. Read from the PE optional header
    because .NET exposes no API for it, and it is the only direct evidence that
    the `release_bundle` feature was on: that feature sets
    `windows_subsystem = "windows"`, without which a console window flashes.
    #>
    param([Parameter(Mandatory)][String]$ExePath)

    $stream = [IO.File]::OpenRead($ExePath)
    try {
        $reader = New-Object IO.BinaryReader($stream)
        $stream.Position = 0x3C
        $peOffset = $reader.ReadInt32()
        # PE signature (4) + COFF header (20) + Subsystem's offset in the
        # optional header (68).
        $stream.Position = $peOffset + 4 + 20 + 68
        return $reader.ReadUInt16()
    } finally {
        $stream.Dispose()
    }
}

function Resolve-Protoc {
    if ($env:PROTOC -and (Test-Path $env:PROTOC)) { return $env:PROTOC }

    $onPath = Get-Command protoc -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }

    $winget = Join-Path $env:LOCALAPPDATA `
        'Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin\protoc.exe'
    if (Test-Path $winget) { return $winget }

    return $null
}

function Assert-WarpNotRunning {
    <#
    app/build.rs stages conpty.dll, dxcompiler.dll, dxil.dll and OpenConsole.exe
    into target/<profile>/, and a running instance holds those open, which fails
    the build. Reported rather than resolved: killing it would take the user's
    terminal sessions with it.
    #>
    $running = Get-Process -Name warp-oss -ErrorAction SilentlyContinue
    if ($running) {
        $ids = ($running | ForEach-Object { $_.Id }) -join ', '
        throw "warp-oss is running (PID $ids). It holds the staged DLLs open, so the build would fail. Close it and re-run."
    }
}
