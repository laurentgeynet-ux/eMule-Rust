<#
.SYNOPSIS
    Unified Verification Script for Emule-Sion (Rust backend + Flutter Desktop).
.DESCRIPTION
    Runs:
    1. Cargo Clippy on workspace with -D warnings
    2. Cargo Test on all workspace crates
    3. Flutter Analyze on apps/desktop
    4. Flutter Test on apps/desktop
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\verify_all.ps1
#>

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Emule-Sion Full Stack Verification (Rust + Dart) " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# 1. Cargo Clippy
Write-Host "`n==> [1/4] Running Cargo Clippy (lints & safety checks)..." -ForegroundColor Yellow
cargo clippy --workspace --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo Clippy failed."
    exit $LASTEXITCODE
}
Write-Host "  [OK] Cargo Clippy passed." -ForegroundColor Green

# 2. Cargo Test
Write-Host "`n==> [2/4] Running Cargo Test Suite across workspace crates..." -ForegroundColor Yellow
cargo test --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo Test suite failed."
    exit $LASTEXITCODE
}
Write-Host "  [OK] All Cargo tests passed." -ForegroundColor Green

# 3. Flutter Analyze
Write-Host "`n==> [3/4] Running Flutter Analyze on desktop app..." -ForegroundColor Yellow
Push-Location "apps/desktop"
try {
    flutter analyze
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Flutter Analyze failed."
        exit $LASTEXITCODE
    }
    Write-Host "  [OK] Flutter Analyze passed without warnings." -ForegroundColor Green

    # 4. Flutter Test
    Write-Host "`n==> [4/4] Running Flutter Test Suite..." -ForegroundColor Yellow
    flutter test
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Flutter Test suite failed."
        exit $LASTEXITCODE
    }
    Write-Host "  [OK] All Flutter tests passed." -ForegroundColor Green
} finally {
    Pop-Location
}

Write-Host "`n==================================================" -ForegroundColor Green
Write-Host "  [SUCCESS] All verification steps passed (100% Green)! " -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
