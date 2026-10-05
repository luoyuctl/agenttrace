# agenttrace Windows installer (PowerShell)
# Usage: irm https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.ps1 | iex
# Or:    powershell -ExecutionPolicy Bypass -File install.ps1 [-Version v0.9.1] [-InstallDir <dir>] [-NoModifyPath]

param(
    [string]$Version = "latest",
    [string]$InstallDir = "$env:LOCALAPPDATA\agenttrace",
    [switch]$NoModifyPath
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$REPO = "luoyuctl/agenttrace"
$BIN = "agenttrace.exe"

# Detect architecture. In interactive Windows PowerShell 5.1 the bundled PSReadLine 2.0
# can shadow RuntimeInformation with a copy whose OSArchitecture is empty, so fall back
# to the environment (PROCESSOR_ARCHITEW6432 is set for a 32-bit shell on a 64-bit OS).
$osArch = try { "$([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)" } catch { "" }
if (-not $osArch) {
    $osArch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
}
$ARCH = switch ($osArch) {
    { $_ -in "X64", "AMD64" } { "amd64" }
    "Arm64"                   { "arm64" }
    default                   { throw "Unsupported architecture: '$osArch'" }
}

$asset = "agenttrace-windows-$ARCH.exe"
if ($Version -eq "latest") {
    $baseUrl = "https://github.com/$REPO/releases/latest/download"
} else {
    $baseUrl = "https://github.com/$REPO/releases/download/$Version"
}

Write-Host "Downloading agenttrace $Version (windows/$ARCH)..." -ForegroundColor Cyan

$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("agenttrace-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null
try {
    $binTmp = Join-Path $tmpDir $asset
    $sumTmp = "$binTmp.sha256"
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/$asset" -OutFile $binTmp
    } catch {
        Write-Host "ERROR: No binary found for windows/$ARCH ($_)" -ForegroundColor Red
        Write-Host "   Build from source: git clone https://github.com/$REPO.git && cd agenttrace && cargo build --release -p agenttrace"
        exit 1
    }
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/$asset.sha256" -OutFile $sumTmp
    } catch {
        Write-Host "ERROR: Could not download the checksum for $asset; refusing to install an unverified binary." -ForegroundColor Red
        exit 1
    }

    # Verify checksum
    $expected = ((Get-Content -Raw $sumTmp).Trim() -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 $binTmp).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        Write-Host "ERROR: Checksum mismatch for $asset" -ForegroundColor Red
        Write-Host "   expected: $expected"
        Write-Host "   actual:   $actual"
        exit 1
    }
    Write-Host "SHA-256 verified" -ForegroundColor Green

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $dest = Join-Path $InstallDir $BIN
    # A running agenttrace.exe cannot be overwritten, but it can be renamed out of the way.
    # Earlier parked images are removed best-effort; one still running is simply skipped.
    Get-ChildItem -Path $InstallDir -Filter "agenttrace.old*.exe" -ErrorAction SilentlyContinue |
        Remove-Item -Force -ErrorAction SilentlyContinue
    $oldName = "agenttrace.old-$PID.exe"
    $old = Join-Path $InstallDir $oldName
    $hadPrevious = Test-Path $dest
    if ($hadPrevious) {
        Rename-Item -Force -Path $dest -NewName $oldName
    }
    try {
        Move-Item -Force $binTmp $dest
    } catch {
        if ($hadPrevious) { Rename-Item -Force -Path $old -NewName $BIN }
        Write-Host "ERROR: Could not install to ${dest}: $_" -ForegroundColor Red
        exit 1
    }
    Remove-Item -Force $old -ErrorAction SilentlyContinue
    Write-Host "Installed to $dest" -ForegroundColor Green
} finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}

# PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$entries = @($userPath -split ';' | Where-Object { $_ })
if ($entries -notcontains $InstallDir) {
    Write-Host ""
    if ($NoModifyPath) {
        Write-Host "WARNING: $InstallDir is not in your PATH. Add it with:" -ForegroundColor Yellow
        Write-Host "     [Environment]::SetEnvironmentVariable('Path', `"$InstallDir;`" + [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"
    } else {
        [Environment]::SetEnvironmentVariable("Path", (@($InstallDir) + $entries) -join ';', "User")
        Write-Host "Added $InstallDir to your user PATH. Open a new terminal to use it." -ForegroundColor Cyan
    }
}
if (($env:Path -split ';') -notcontains $InstallDir) {
    $env:Path = "$InstallDir;$env:Path"
}

Write-Host ""
Write-Host "agenttrace installed! Try:" -ForegroundColor Cyan
Write-Host "   agenttrace --latest"
Write-Host "   agenttrace            # launch TUI"
Write-Host "   agenttrace update     # upgrade later"
