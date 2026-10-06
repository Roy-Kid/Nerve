# Runs without Rust, a desktop, registry writes, or installed Nerve binaries.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $env:LOCALAPPDATA) { $env:LOCALAPPDATA = [IO.Path]::GetTempPath() }
if (-not $env:APPDATA) { $env:APPDATA = [IO.Path]::GetTempPath() }
. (Join-Path (Split-Path $PSScriptRoot -Parent) 'nerve.ps1') -Help

function Assert-True($condition, $message) {
    if (-not $condition) { throw $message }
}
function Assert-Throws($action, $message) {
    $caught = $false
    try { & $action } catch {
        $caught = $true
        Assert-True ($_.Exception.Message -like "*$message*") "Unexpected error: $_"
    }
    Assert-True $caught "Expected failure containing: $message"
}
function Invoke-Cargo { throw 'Prebuilt installation must not invoke Cargo' }

$temp = Join-Path ([IO.Path]::GetTempPath()) ('nerve-installer-test-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temp | Out-Null
try {
    $Root = $temp
    $BinaryDirectory = $null
    foreach ($exe in @('nerve-hub.exe', 'nerve-windows-core.exe', 'nerve-windows-surface.exe', 'nerve-windows-surface.dll', 'nerve-windows-surface.runtimeconfig.json', 'nerve-windows-surface.pri', 'Microsoft.UI.Xaml.dll', 'Microsoft.WindowsAppRuntime.dll', 'coreclr.dll')) {
        New-Item -ItemType File -Path (Join-Path $temp $exe) | Out-Null
    }
    $BinariesReady = $false
    Prepare-Binaries
    Assert-True ($Release -eq $temp) 'Downloaded bundle should be detected automatically'

    $BinaryDirectory = $temp
    $Root = Join-Path $temp 'checkout'
    $BinariesReady = $false
    Prepare-Binaries
    Assert-True ($Release -eq $temp) 'Explicit binary directory should work outside a bundle'

    New-Item -ItemType Directory -Path (Join-Path $temp 'zh-CN'), (Join-Path $temp 'scripts') | Out-Null
    New-Item -ItemType File -Path (Join-Path $temp 'zh-CN\resources.pri'), (Join-Path $temp 'scripts\nerve.ps1') | Out-Null
    $InstallDir = Join-Path $temp 'installation'
    # Keep the destination outside the source payload so it cannot copy itself.
    $payloadInstall = Join-Path ([IO.Path]::GetTempPath()) ('nerve-payload-test-' + [guid]::NewGuid())
    $InstallDir = $payloadInstall
    try {
        Copy-NativePayload
        Assert-True (Test-Path (Join-Path $InstallDir 'nerve-windows-surface.dll')) 'Native managed assembly must be installed'
        Assert-True (Test-Path (Join-Path $InstallDir 'zh-CN\resources.pri')) 'Native resource subfolders must be installed'
        Assert-True (-not (Test-Path (Join-Path $InstallDir 'scripts'))) 'Installer helpers are not runtime payload'
    } finally {
        $resolvedPayload = [IO.Path]::GetFullPath($payloadInstall)
        if (-not $resolvedPayload.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()))) { throw 'Unsafe payload cleanup path' }
        Remove-Item -LiteralPath $resolvedPayload -Recurse -Force
    }

    Remove-Item -LiteralPath (Join-Path $temp 'coreclr.dll')
    $BinariesReady = $false
    Assert-Throws { Prepare-Binaries } 'Missing coreclr.dll'
    New-Item -ItemType File -Path (Join-Path $temp 'coreclr.dll') | Out-Null

    Remove-Item (Join-Path $temp 'nerve-hub.exe')
    $BinariesReady = $false
    Assert-Throws { Prepare-Binaries } 'Missing nerve-hub.exe'

    $InstallDir = $temp
    function Get-Process { [pscustomobject]@{ Path = (Join-Path $InstallDir 'nerve-hub.exe') } }
    Assert-Throws { Assert-InstallStopped } 'Quit Nerve'

    $portBusy = $false
    $probe = [Net.Sockets.TcpClient]::new()
    try { $portBusy = $probe.ConnectAsync('127.0.0.1', 17890).Wait(500) } catch { } finally { $probe.Dispose() }
    if ($portBusy) {
        Assert-Throws { Assert-VerifyIsolated } 'already in use'
    } else {
        $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 17890)
        try {
            $listener.Start()
            Assert-Throws { Assert-VerifyIsolated } 'already in use'
        } finally { $listener.Stop() }
        Assert-VerifyIsolated
    }
    Write-Host 'ALL OK - prebuilt discovery, incomplete bundle, running install and verification isolation'
} finally {
    $resolvedTemp = [IO.Path]::GetFullPath($temp)
    if (-not $resolvedTemp.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()))) { throw 'Unsafe installer test cleanup path' }
    Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
}
