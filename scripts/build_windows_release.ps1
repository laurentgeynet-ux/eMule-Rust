<#
.SYNOPSIS
    Builds and packages the complete standalone Emule-Sion Windows distribution.
.DESCRIPTION
    1. Compiles the Rust backend mule-daemon in release mode.
    2. Builds the Flutter Desktop Windows client in release mode.
    3. Bundles both binaries, version metadata, and dependencies into dist/EmuleSion-Windows.
#>

param(
    [string]$OutputDir = "dist\EmuleSion-Windows"
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Emule-Sion Windows Release Packaging Pipeline   " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# 1. Check tools
$Cargo = if (Get-Command "cargo" -ErrorAction SilentlyContinue) { "cargo" } else { "$env:USERPROFILE\.cargo\bin\cargo.exe" }
$Flutter = if (Get-Command "flutter" -ErrorAction SilentlyContinue) { "flutter" } else { "C:\Users\Lou\flutter\bin\flutter.bat" }

if (!(Test-Path $Cargo)) {
    throw "Rust cargo not found at $Cargo"
}
if (!(Test-Path $Flutter)) {
    throw "Flutter SDK not found at $Flutter"
}

# 2. Build Rust mule-daemon in release mode
Write-Host "`n[1/4] Compiling Rust mule-daemon (release)..." -ForegroundColor Yellow
& $Cargo build --release --bin mule-daemon
if ($LASTEXITCODE -ne 0) { throw "Rust release build failed with code $LASTEXITCODE" }

$DaemonExe = Join-Path $Root "target\release\mule-daemon.exe"
if (!(Test-Path $DaemonExe)) {
    throw "Compiled daemon binary not found at $DaemonExe"
}
Write-Host " -> Built: $DaemonExe" -ForegroundColor Green

# 3. Build Flutter Windows in release mode
Write-Host "`n[2/4] Building Flutter Desktop Windows (release)..." -ForegroundColor Yellow
Push-Location "apps\desktop"
try {
    & $Flutter build windows --release
    if ($LASTEXITCODE -ne 0) { throw "Flutter release build failed with code $LASTEXITCODE" }
} finally {
    Pop-Location
}

$FlutterReleaseDir = Join-Path $Root "apps\desktop\build\windows\x64\runner\Release"
if (!(Test-Path $FlutterReleaseDir)) {
    throw "Flutter release output directory not found at $FlutterReleaseDir"
}

# 4. Assemble distribution bundle
Write-Host "`n[3/4] Assembling standalone distribution in $OutputDir..." -ForegroundColor Yellow
$DestPath = Join-Path $Root $OutputDir
if (Test-Path $DestPath) {
    Remove-Item -Recurse -Force $DestPath
}
New-Item -ItemType Directory -Path $DestPath -Force | Out-Null

# Copy Flutter release files
Copy-Item -Path "$FlutterReleaseDir\*" -Destination $DestPath -Recurse -Force

# Create bin/ directory and bundle mule-daemon.exe
$BinDir = Join-Path $DestPath "bin"
New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
Copy-Item -Path $DaemonExe -Destination $BinDir -Force
Copy-Item -Path $DaemonExe -Destination $DestPath -Force

# 5. Generate version.json
Write-Host "`n[4/4] Writing release metadata (version.json)..." -ForegroundColor Yellow
$CommitHash = try { (git rev-parse --short HEAD).Trim() } catch { "unknown" }
$NowIso = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$VersionInfo = [PSCustomObject]@{
    product = "Emule-Sion"
    platform = "windows-x64"
    version = "0.1.0"
    protocolVersion = 1
    commit = $CommitHash
    buildTimestampUtc = $NowIso
    daemonExecutable = "bin/mule-daemon.exe"
    appExecutable = "desktop.exe"
}

$VersionJsonPath = Join-Path $DestPath "version.json"
$VersionInfo | ConvertTo-Json -Depth 4 | Out-File -FilePath $VersionJsonPath -Encoding utf8

Write-Host "`n==================================================" -ForegroundColor Green
Write-Host "  Build Succeeded! Distribution ready at:         " -ForegroundColor Green
Write-Host "  $DestPath" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Green
