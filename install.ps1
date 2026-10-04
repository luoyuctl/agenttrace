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

# Detect architecture
$ARCH = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
    "X64"   { "amd64" }
    "Arm64" { "arm64" }
    default { throw "Unsupported architecture: $_" }
}

$asset = "agenttrace-windows-$ARCH.exe"
if ($Version -eq "latest") {
    $baseUrl = "https://github.com/$REPO/releases/latest/download"
} else {
    $baseUrl = "https://github.com/$REPO/releases/download/$Version"
}

Write-Host "⬇️  Downloading agenttrace $Version (windows/$ARCH)..." -ForegroundColor Cyan

$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("agenttrace-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null
try {
    $binTmp = Join-Path $tmpDir $asset
    $sumTmp = "$binTmp.sha256"
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/$asset" -OutFile $binTmp
    } catch {
        Write-Host "❌ No binary found for windows/$ARCH ($_)" -ForegroundColor Red
        Write-Host "   Build from source: git clone https://github.com/$REPO.git && cd agenttrace && cargo build --release -p agenttrace"
        exit 1
    }
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/$asset.sha256" -OutFile $sumTmp
    } catch {
        Write-Host "❌ Could not download the checksum for $asset; refusing to install an unverified binary." -ForegroundColor Red
        exit 1
    }

    # Verify checksum
    $expected = ((Get-Content -Raw $sumTmp).Trim() -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 $binTmp).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        Write-Host "❌ Checksum mismatch for $asset" -ForegroundColor Red
        Write-Host "   expected: $expected"
        Write-Host "   actual:   $actual"
        exit 1
    }
    Write-Host "🔒 SHA-256 verified" -ForegroundColor Green

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $dest = Join-Path $InstallDir $BIN
    # A running agenttrace.exe cannot be overwritten, but it can be renamed out of the way.
    if (Test-Path $dest) {
        $old = "$dest.old"
        Remove-Item -Force $old -ErrorAction SilentlyContinue
        Rename-Item -Force $dest $old
    }
    Move-Item -Force $binTmp $dest
    Remove-Item -Force "$dest.old" -ErrorAction SilentlyContinue
    Write-Host "✅ Installed to $dest" -ForegroundColor Green
} finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}

# PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$entries = @($userPath -split ';' | Where-Object { $_ })
if ($entries -notcontains $InstallDir) {
    Write-Host ""
    if ($NoModifyPath) {
        Write-Host "⚠️  $InstallDir is not in your PATH. Add it with:" -ForegroundColor Yellow
        Write-Host "     [Environment]::SetEnvironmentVariable('Path', `"$InstallDir;`" + [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"
    } else {
        [Environment]::SetEnvironmentVariable("Path", (@($InstallDir) + $entries) -join ';', "User")
        Write-Host "➕ Added $InstallDir to your user PATH. Open a new terminal to use it." -ForegroundColor Cyan
    }
}
if (($env:Path -split ';') -notcontains $InstallDir) {
    $env:Path = "$InstallDir;$env:Path"
}

Write-Host ""
Write-Host "🎉 agenttrace installed! Try:" -ForegroundColor Cyan
Write-Host "   agenttrace --latest"
Write-Host "   agenttrace            # launch TUI"
Write-Host "   agenttrace update     # upgrade later"
