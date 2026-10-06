<#
.SYNOPSIS
    Nerve dev launcher for Windows — the counterpart to scripts/nerve.sh.

.DESCRIPTION
    Explicit flags only, like the bash one: nothing here runs unless it was
    asked for by name. Written for Windows PowerShell 5.1, which every
    Windows 10 and 11 ships — requiring pwsh 7 would mean a second install
    before the first run.

    scripts/nerve.sh stays the launcher on macOS and Linux; it is bash,
    xcodebuild and tmux, none of which apply here.
#>
[CmdletBinding()]
param(
    [switch]$Help,
    [switch]$Build,
    [switch]$Run,
    [switch]$Install,
    [switch]$Uninstall,
    [switch]$Demo,
    [switch]$VerifyLoop,
    [switch]$VerifySurface,
    [switch]$Test,
    [string]$BinaryDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Root = Split-Path -Parent $PSScriptRoot
$Release = Join-Path $Root 'target\windows'
$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\Nerve'
$StartMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$Aumid = 'Nerve.Surface'
$Ingest = 'http://127.0.0.1:17890'
$BinariesReady = $false

# The portable crates. nerve-tmux-surface is unix-only by design — tmux has no
# Windows port — so it is never named here.
$Crates = @('-p', 'nerve-platform', '-p', 'nerve-hub',
            '-p', 'nerve-surface-core', '-p', 'nerve-windows-surface')

function Show-Help {
    @'
Nerve (Windows)

  -Build           Rust hub/core + self-contained WinUI 3 publish (.NET 10 SDK)
  -Run             start the tray surface (build only when no binaries supplied)
  -Install         copy to %LOCALAPPDATA%\Programs\Nerve, Start Menu shortcut
  -Uninstall       remove all of the above, including the Run key
  -Demo            POST the demo fixture (the hub must already be up)
  -VerifyLoop      ingest contract end to end
  -VerifySurface   headless surface self-test against a real hub
  -BinaryDirectory use existing binaries from this folder (no Rust required)
  -Test            Rust tests + native host protocol tests
  -Help            this

Nothing runs without a flag.
'@ | Write-Host
}

function Invoke-Cargo {
    param([string[]]$Arguments)
    Push-Location $Root
    try {
        & cargo @Arguments
        if ($LASTEXITCODE -ne 0) { throw "cargo $($Arguments -join ' ') failed" }
    } finally {
        Pop-Location
    }
}

function Build-All {
    Invoke-Cargo @('build', '--release', '-p', 'nerve-hub', '-p', 'nerve-windows-surface', '--bin', 'nerve-hub', '--bin', 'nerve-windows-core')
    $script:Release = Join-Path $Root 'target\windows'
    & dotnet publish (Join-Path $Root 'surfaces\windows\Nerve.Windows.csproj') -c Release -p:Platform=x64 -o $Release --nologo
    if ($LASTEXITCODE -ne 0) { throw 'WinUI 3 publish failed' }
    Copy-Item (Join-Path $Root 'target\release\nerve-hub.exe'), (Join-Path $Root 'target\release\nerve-windows-core.exe') $Release -Force
    $script:BinariesReady = $true
}

function Prepare-Binaries {
    if ($script:BinariesReady) { return }
    if ($BinaryDirectory) {
        $script:Release = (Resolve-Path -LiteralPath $BinaryDirectory).Path
    } elseif ((Test-Path (Join-Path $Root 'nerve-hub.exe')) -and
              (Test-Path (Join-Path $Root 'nerve-windows-surface.exe'))) {
        # The CI download contains binaries beside the scripts folder.
        $script:Release = $Root
    } else {
        Build-All
        return
    }
    foreach ($exe in @('nerve-hub.exe', 'nerve-windows-core.exe', 'nerve-windows-surface.exe', 'nerve-windows-surface.dll', 'nerve-windows-surface.runtimeconfig.json', 'nerve-windows-surface.pri', 'Microsoft.UI.Xaml.dll', 'Microsoft.WindowsAppRuntime.dll', 'coreclr.dll')) {
        if (-not (Test-Path -LiteralPath (Join-Path $Release $exe) -PathType Leaf)) {
            throw "Missing $exe in $Release. Download and extract the complete Nerve Windows package."
        }
    }
    $script:BinariesReady = $true
}

function Assert-InstallStopped {
    foreach ($name in @('nerve-hub', 'nerve-windows-core', 'nerve-windows-surface')) {
        foreach ($process in @(Get-Process -Name $name -ErrorAction SilentlyContinue)) {
            if ($process.Path -and
                ([IO.Path]::GetDirectoryName($process.Path) -eq $InstallDir)) {
                throw 'Quit Nerve and other Nerve surfaces, wait 30 seconds for the hub to stop, then retry.'
            }
        }
    }
}

function Assert-VerifyIsolated {
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        if ($client.ConnectAsync('127.0.0.1', 17890).Wait(500)) {
            throw 'Port 17890 is already in use. Quit existing Nerve surfaces and wait for the hub to stop before verification.'
        }
    } catch [System.AggregateException] {
        # Connection refused: no existing hub to modify with the fixture.
    } finally {
        $client.Dispose()
    }
}

function Wait-Hub {
    param([int]$Seconds = 10)
    for ($i = 0; $i -lt ($Seconds * 10); $i++) {
        try {
            Invoke-RestMethod -Uri "$Ingest/v1/health" -TimeoutSec 1 | Out-Null
            return $true
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }
    return $false
}

function Copy-NativePayload {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    # Native self-contained apps have DLLs, PRI files and resource subfolders.
    # Install the complete payload, not just the two executable launchers.
    if ([IO.Path]::GetFullPath($Release) -ne [IO.Path]::GetFullPath($InstallDir)) {
        foreach ($item in Get-ChildItem -LiteralPath $Release) {
            if ($item.Name -notin @('scripts', 'fixtures')) {
                Copy-Item -LiteralPath $item.FullName -Destination $InstallDir -Recurse -Force
            }
        }
    }
}

function Install-Nerve {
    Prepare-Binaries
    Assert-InstallStopped
    Copy-NativePayload

    # A Start Menu entry for launching the installed surface. Notification
    # identity is registered separately below; WScript does not set an AUMID.
    New-Item -ItemType Directory -Force -Path $StartMenu | Out-Null
    $link = Join-Path $StartMenu 'Nerve.lnk'
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($link)
    $shortcut.TargetPath = Join-Path $InstallDir 'nerve-windows-surface.exe'
    $shortcut.WorkingDirectory = $InstallDir
    $shortcut.Description = 'Nerve'
    $shortcut.Save()

    # DisplayName for the toast; the surface writes this too, idempotently.
    $key = "HKCU:\Software\Classes\AppUserModelId\$Aumid"
    New-Item -Path $key -Force | Out-Null
    Set-ItemProperty -Path $key -Name 'DisplayName' -Value 'Nerve'

    Write-Host "Installed to $InstallDir"
    Write-Host "Shortcut: $link"
    Write-Host ''
    Write-Host 'Windows 11 hides new tray icons in the overflow by default —'
    Write-Host 'drag it out of the chevron to keep it visible.'
}

function Uninstall-Nerve {
    Assert-InstallStopped
    foreach ($path in @((Join-Path $StartMenu 'Nerve.lnk'),
                        "HKCU:\Software\Classes\AppUserModelId\$Aumid",
                        $InstallDir)) {
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Recurse -Force
        }
    }
    $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    if (Get-ItemProperty -Path $runKey -Name 'Nerve' -ErrorAction SilentlyContinue) {
        Remove-ItemProperty -Path $runKey -Name 'Nerve' -Force
    }
    Write-Host 'Removed the binaries, the shortcut, the AUMID and the Run key. Preferences were kept.'
}

function Invoke-Demo {
    $fixture = Join-Path $Root 'fixtures\demo_snapshot.json'
    $body = Get-Content $fixture -Raw
    Invoke-RestMethod -Uri "$Ingest/v1/snapshot" -Method Post `
        -ContentType 'application/json' -Body $body | ConvertTo-Json -Depth 6
}

function Invoke-VerifyLoop {
    Prepare-Binaries
    Assert-VerifyIsolated
    $hub = Start-Process (Join-Path $Release 'nerve-hub.exe') -ArgumentList 'serve' `
        -PassThru -WindowStyle Hidden
    try {
        if (-not (Wait-Hub)) { throw 'the hub never answered /v1/health' }

        $fixture = Join-Path $Root 'fixtures\demo_snapshot.json'
        Invoke-RestMethod -Uri "$Ingest/v1/snapshot" -Method Post `
            -ContentType 'application/json' -Body (Get-Content $fixture -Raw) | Out-Null

        $jobs = Invoke-RestMethod -Uri "$Ingest/v1/jobs"
        $jobs = @($jobs)
        if ($jobs.Count -lt 1) { throw 'the hub accepted a snapshot and listed no jobs' }

        # An unknown route must be a 404, not a body — the same thing the
        # surface's own transport test asserts.
        try {
            Invoke-RestMethod -Uri "$Ingest/v1/nope" -TimeoutSec 2 | Out-Null
            throw 'an unknown route answered instead of 404'
        } catch {
            if (-not $_.Exception.Response) { throw }
            $code = [int]$_.Exception.Response.StatusCode
            if ($code -ne 404) { throw "unknown route answered $code" }
        }

        Write-Host "ALL OK — $($jobs.Count) job(s) through the loop"
    } finally {
        Stop-Process -Id $hub.Id -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-VerifySurface {
    Prepare-Binaries
    Assert-VerifyIsolated
    $surface = $null
    $second = $null
    $hub = Start-Process (Join-Path $Release 'nerve-hub.exe') -ArgumentList 'serve' `
        -PassThru -WindowStyle Hidden
    try {
        if (-not (Wait-Hub)) { throw 'the hub never answered /v1/health' }
        Invoke-RestMethod -Uri "$Ingest/v1/snapshot" -Method Post `
            -ContentType 'application/json' `
            -Body (Get-Content (Join-Path $Root 'fixtures\demo_snapshot.json') -Raw) | Out-Null

        # The surface's own tests already cover the icon and the tooltip; what
        # only a real Windows box can add is that the binary starts, takes its
        # lock, and holds a stream against a live hub.
        $surface = Start-Process (Join-Path $Release 'nerve-windows-surface.exe') `
            -PassThru -WindowStyle Hidden
        Start-Sleep -Seconds 3
        if ($surface.HasExited) { throw "the surface exited with $($surface.ExitCode)" }

        $health = Invoke-RestMethod -Uri "$Ingest/v1/health"
        if ($health.watchers -ne 1) { throw "expected one attached surface, got $($health.watchers)" }

        # A second copy must stand down rather than take a second stream.
        $second = Start-Process (Join-Path $Release 'nerve-windows-surface.exe') `
            -PassThru -WindowStyle Hidden
        if (-not $second.WaitForExit(10000)) { throw 'the second instance did not stand down within 10 seconds' }
        if ($second.ExitCode -ne 0) { throw "the second instance exited $($second.ExitCode)" }
        if ($surface.HasExited) { throw 'the second instance killed the first' }
        $health = Invoke-RestMethod -Uri "$Ingest/v1/health"
        if ($health.watchers -ne 1) { throw 'the second instance opened an extra stream' }

        Stop-Process -Id $surface.Id -Force
        Write-Host 'ALL OK — surface attaches, and a second instance stands down'
    } finally {
        foreach ($process in @($second, $surface)) {
            if ($null -ne $process -and -not $process.HasExited) {
                Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            }
        }
        Stop-Process -Id $hub.Id -Force -ErrorAction SilentlyContinue
    }
}

if ($Help -or -not ($Build -or $Run -or $Install -or $Uninstall -or $Demo -or
                    $VerifyLoop -or $VerifySurface -or $Test)) {
    Show-Help
    return
}

if ($Uninstall -and ($Install -or $Run -or $Build -or $VerifyLoop -or $VerifySurface)) {
    throw '-Uninstall must run separately from installation, launch and verification.'
}

if ($Build)         { Build-All }
if ($Test) {
    Invoke-Cargo (@('test') + $Crates)
    Invoke-Cargo @('build', '-p', 'nerve-windows-surface', '--bin', 'nerve-windows-core')
    & dotnet run --project (Join-Path $Root 'surfaces\windows\tests\Nerve.Windows.Tests.csproj') -- $Root
    if ($LASTEXITCODE -ne 0) { throw 'Native host protocol tests failed' }
}
if ($Install)       { Install-Nerve }
if ($Uninstall)     { Uninstall-Nerve }
if ($Demo)          { Invoke-Demo }
if ($VerifyLoop)    { Invoke-VerifyLoop }
if ($VerifySurface) { Invoke-VerifySurface }
if ($Run) {
    Prepare-Binaries
    $runDirectory = if ($Install) { $InstallDir } else { $Release }
    Start-Process (Join-Path $runDirectory 'nerve-windows-surface.exe') -WindowStyle Hidden
}
