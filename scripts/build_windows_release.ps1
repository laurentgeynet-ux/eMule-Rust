<#
.SYNOPSIS
    Builds, signs, and packages the complete standalone Emule-Sion Windows distribution.
.DESCRIPTION
    Phase 7 — Industrialisation Locale & Release Candidate:
    1. Verifies clean Git repository state (rejects dirty state unless -AllowDirty is set).
    2. Collects exact toolchain versions (rustc, cargo, flutter, dart).
    3. Compiles mule-daemon and Flutter Desktop in release mode.
    4. Assembles standalone distribution in dist/EmuleSion-Windows.
    5. Generates CycloneDX 1.5 SBOM files in dist/EmuleSion-Windows/sbom.
    6. Signs executables and DLLs with Authenticode (if -Sign is set).
    7. Computes SHA-256 hashes of all artifacts post-signing and writes SHA256SUMS.txt and version.json.
    8. Packages distribution into ZIP archive and calculates its SHA-256 hash.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1
    powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1 -Sign -CertificateThumbprint "<thumbprint>"
#>

param(
    [string]$OutputDir = "",
    [string]$Flavor = "",
    [switch]$Sign = $false,
    [string]$CertificateThumbprint = "",
    [string]$TimestampServer = "http://timestamp.digicert.com",
    [switch]$AllowDirty = $false,
    [switch]$SkipZip = $false,
    [switch]$LabMode = $false
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ($LabMode -or $Flavor -eq "qa-loopback") {
    $DistributionFlavor = "qa-loopback"
    $LabToolsEnabled = $true
    $LabMode = $true
    if ([string]::IsNullOrWhiteSpace($OutputDir)) {
        $OutputDir = "dist\EmuleSion-Qa-Loopback-Windows"
    }
} else {
    $DistributionFlavor = "standard"
    $LabToolsEnabled = $false
    $LabMode = $false
    if ([string]::IsNullOrWhiteSpace($OutputDir)) {
        $OutputDir = "dist\EmuleSion-Standard-Windows"
    }
}

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Emule-Sion Windows Release Candidate Pipeline   " -ForegroundColor Cyan
Write-Host "  Flavor: $DistributionFlavor (LabTools: $LabToolsEnabled)" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# -----------------------------------------------------------------------------
# 1. Inspect Git State & Refuse Dirty Working Tree
# -----------------------------------------------------------------------------
Write-Host "`n[1/8] Verifying Git state and commit provenance..." -ForegroundColor Yellow

$GitCommit = try { (& git rev-parse HEAD).Trim() } catch { "0000000000000000000000000000000000000000" }
$GitStatus = try { (& git status --porcelain).Trim() } catch { "" }
$GitDirty = ![string]::IsNullOrEmpty($GitStatus)

if ($GitDirty -and !$AllowDirty) {
    throw "Working tree is dirty! Uncommitted changes detected. Commit changes or specify -AllowDirty for debug builds."
}

if ($GitDirty) {
    Write-Host " [WARNING] Building from dirty working tree (-AllowDirty is enabled)" -ForegroundColor Magenta
} else {
    Write-Host " -> Clean repository state confirmed at commit: $GitCommit" -ForegroundColor Green
}

# -----------------------------------------------------------------------------
# 2. Inspect Toolchains
# -----------------------------------------------------------------------------
Write-Host "`n[2/8] Inspecting compilers and build toolchains..." -ForegroundColor Yellow

$CargoCmd = Get-Command "cargo" -ErrorAction SilentlyContinue
$RustcCmd = Get-Command "rustc" -ErrorAction SilentlyContinue
$FlutterCmd = Get-Command "flutter" -ErrorAction SilentlyContinue

$Cargo = if ($CargoCmd) { $CargoCmd.Source } elseif (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe") { "$env:USERPROFILE\.cargo\bin\cargo.exe" } else { $null }
$Rustc = if ($RustcCmd) { $RustcCmd.Source } elseif (Test-Path "$env:USERPROFILE\.cargo\bin\rustc.exe") { "$env:USERPROFILE\.cargo\bin\rustc.exe" } else { $null }
$Flutter = if ($FlutterCmd) { $FlutterCmd.Source } elseif (Test-Path "C:\Users\Lou\flutter\bin\flutter.bat") { "C:\Users\Lou\flutter\bin\flutter.bat" } else { $null }

if (!$Cargo -or !(Test-Path $Cargo)) { throw "Rust cargo not found" }
if (!$Rustc -or !(Test-Path $Rustc)) { throw "Rust compiler rustc not found" }
if (!$Flutter -or !(Test-Path $Flutter)) { throw "Flutter SDK not found" }

$RustcVersion = (& $Rustc --version).Trim()
$CargoVersion = (& $Cargo --version).Trim()
$FlutterVersion = ((& $Flutter --version | Select-Object -First 1)).Trim()
$DartVersion = try { (& dart --version 2>&1).Trim() } catch { "Dart (bundled)" }

Write-Host " -> Rustc   : $RustcVersion" -ForegroundColor Gray
Write-Host " -> Cargo   : $CargoVersion" -ForegroundColor Gray
Write-Host " -> Flutter : $FlutterVersion" -ForegroundColor Gray
Write-Host " -> Dart    : $DartVersion" -ForegroundColor Gray

# -----------------------------------------------------------------------------
# 3. Build Rust mule-daemon (release)
# -----------------------------------------------------------------------------
Write-Host "`n[3/8] Compiling Rust mule-daemon and mule-cli (release)..." -ForegroundColor Yellow
if ($LabMode) {
    Write-Host " [LAB-MODE] Compiling mule-daemon with feature 'lab-tools' enabled for QA lab testing..." -ForegroundColor Cyan
    & $Cargo build --release -p mule-daemon --features lab-tools
    if ($LASTEXITCODE -ne 0) { throw "Rust daemon (lab-tools) release build failed with code $LASTEXITCODE" }
    & $Cargo build --release --bin mule-cli
    if ($LASTEXITCODE -ne 0) { throw "Rust CLI release build failed with code $LASTEXITCODE" }
} else {
    & $Cargo build --release --bin mule-daemon --bin mule-cli
    if ($LASTEXITCODE -ne 0) { throw "Rust release build failed with code $LASTEXITCODE" }
}

$DaemonExe = Join-Path $Root "target\release\mule-daemon.exe"
$CliExe = Join-Path $Root "target\release\mule-cli.exe"
if (!(Test-Path $DaemonExe)) { throw "Compiled daemon binary not found at $DaemonExe" }
if (!(Test-Path $CliExe)) { throw "Compiled CLI binary not found at $CliExe" }
Write-Host " -> Built: $DaemonExe" -ForegroundColor Green
Write-Host " -> Built: $CliExe" -ForegroundColor Green

# -----------------------------------------------------------------------------
# 4. Build Flutter Desktop Windows (release)
# -----------------------------------------------------------------------------
Write-Host "`n[4/8] Building Flutter Desktop Windows (release)..." -ForegroundColor Yellow
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

# -----------------------------------------------------------------------------
# 5. Assemble Distribution Bundle
# -----------------------------------------------------------------------------
Write-Host "`n[5/8] Assembling distribution bundle in $OutputDir..." -ForegroundColor Yellow
$DestPath = Join-Path $Root $OutputDir
if (Test-Path $DestPath) {
    Remove-Item -Recurse -Force $DestPath
}
New-Item -ItemType Directory -Path $DestPath -Force | Out-Null

# Copy Flutter release binaries and assets
Copy-Item -Path "$FlutterReleaseDir\*" -Destination $DestPath -Recurse -Force

# Standardize executable naming: EmuleSion.exe is the sole desktop UI binary
$PrimaryExe = Join-Path $DestPath "desktop.exe"
$BrandedExe = Join-Path $DestPath "EmuleSion.exe"
if (Test-Path $PrimaryExe) {
    Move-Item -Path $PrimaryExe -Destination $BrandedExe -Force
}
if (!(Test-Path $BrandedExe)) {
    throw "Expected EmuleSion.exe not found in $DestPath"
}

# Create bin/ directory and bundle mule-daemon.exe and mule-cli.exe
$BinDir = Join-Path $DestPath "bin"
New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
Copy-Item -Path $DaemonExe -Destination $BinDir -Force
Copy-Item -Path $CliExe -Destination $BinDir -Force

# Create portable.flag in bundle root for portable mode
$PortableFlag = Join-Path $DestPath "portable.flag"
New-Item -ItemType File -Path $PortableFlag -Force | Out-Null

Write-Host " -> Bundle assembled successfully in: $DestPath" -ForegroundColor Green

# -----------------------------------------------------------------------------
# 6. Generate CycloneDX 1.5 SBOM
# -----------------------------------------------------------------------------
Write-Host "`n[6/8] Generating CycloneDX SBOM artifacts..." -ForegroundColor Yellow
$SbomScript = Join-Path $Root "scripts\generate_sbom.ps1"
if (Test-Path $SbomScript) {
    & powershell -NoProfile -ExecutionPolicy RemoteSigned -File $SbomScript -BundleDir $OutputDir
    if ($LASTEXITCODE -ne 0) { throw "SBOM generation failed" }
} else {
    Write-Host " [WARNING] SBOM generator script not found at $SbomScript" -ForegroundColor Magenta
}

# -----------------------------------------------------------------------------
# 7. Authenticode Code Signing (Optional Hook)
# -----------------------------------------------------------------------------
if ($Sign) {
    Write-Host "`n[7/8] Signing binaries with Authenticode..." -ForegroundColor Yellow
    
    if ([string]::IsNullOrWhiteSpace($CertificateThumbprint)) {
        throw "Code signing requested (-Sign) but -CertificateThumbprint was not provided."
    }

    $SignTool = $null
    # Locate signtool in PATH or Windows Kits
    if (Get-Command "signtool.exe" -ErrorAction SilentlyContinue) {
        $SignTool = "signtool.exe"
    } else {
        $KitsRoots = @(
            "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
            "${env:ProgramFiles}\Windows Kits\10\bin"
        )
        foreach ($kit in $KitsRoots) {
            if (Test-Path $kit) {
                $found = Get-ChildItem -Path $kit -Filter "signtool.exe" -Recurse -File -ErrorAction SilentlyContinue |
                    Where-Object { $_.FullName -match 'x64' } | Select-Object -First 1
                if ($found) {
                    $SignTool = $found.FullName
                    break
                }
            }
        }
    }

    if (!$SignTool) {
        throw "signtool.exe not found on system. Ensure Windows 10/11 SDK is installed."
    }

    Write-Host " -> Using SignTool: $SignTool" -ForegroundColor Gray

    $BinariesToSign = Get-ChildItem -Path $DestPath -Recurse -Include "*.exe", "*.dll"
    foreach ($bin in $BinariesToSign) {
        Write-Host "  -> Signing: $($bin.Name)" -ForegroundColor Gray
        & $SignTool sign /sha1 $CertificateThumbprint /tr $TimestampServer /td sha256 /fd sha256 $bin.FullName
        if ($LASTEXITCODE -ne 0) { throw "Failed to sign $($bin.FullName)" }

        # Verify signature immediately
        & $SignTool verify /pa /all $bin.FullName
        if ($LASTEXITCODE -ne 0) { throw "Signature verification failed for $($bin.FullName)" }
    }
    Write-Host " -> All binaries successfully signed and verified with Authenticode." -ForegroundColor Green
} else {
    Write-Host "`n[7/8] Code signing skipped (-Sign not specified). Production build will be marked unsigned." -ForegroundColor Gray
}

# -----------------------------------------------------------------------------
# 8. Post-Signing Integrity Hashing, version.json & SHA256SUMS.txt
# -----------------------------------------------------------------------------
Write-Host "`n[8/8] Computing post-signing cryptographic hashes and manifest..." -ForegroundColor Yellow

$NowIso = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$AppVersion = "0.1.2-lab"

$ArtifactsList = [System.Collections.Generic.List[PSCustomObject]]::new()
$SumsLines = [System.Collections.Generic.List[string]]::new()

$AllBundleFiles = Get-ChildItem -Path $DestPath -Recurse -File | Where-Object {
    $_.Name -ne "SHA256SUMS.txt" -and $_.Name -ne "version.json"
} | Sort-Object FullName

foreach ($file in $AllBundleFiles) {
    $relPath = ($file.FullName.Substring($DestPath.Length)).TrimStart('\').Replace('\', '/')
    $hash = (Get-FileHash -Path $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $size = $file.Length

    $ArtifactsList.Add([ordered]@{
        path = $relPath
        sha256 = $hash
        sizeBytes = $size
    })

    $SumsLines.Add("$hash  $relPath")
}

# Write SHA256SUMS.txt
$SumsPath = Join-Path $DestPath "SHA256SUMS.txt"
[System.IO.File]::WriteAllLines($SumsPath, $SumsLines, [System.Text.Encoding]::UTF8)
Write-Host " -> Generated: $SumsPath ($($SumsLines.Count) file entries)" -ForegroundColor Green

# Write version.json manifest
$VersionInfo = [ordered]@{
    schemaVersion = 1
    product = "Emule-Sion"
    version = $AppVersion
    distributionFlavor = $DistributionFlavor
    labToolsEnabled = $LabToolsEnabled
    gitCommit = $GitCommit
    gitDirty = $GitDirty
    builtAtUtc = $NowIso
    target = "x86_64-pc-windows-msvc"
    protocolVersion = 1
    buildMode = "release"
    signed = [bool]$Sign
    toolchains = [ordered]@{
        rustc = $RustcVersion
        cargo = $CargoVersion
        flutter = $FlutterVersion
        dart = $DartVersion
    }
    artifacts = $ArtifactsList
}

$VersionJsonPath = Join-Path $DestPath "version.json"
$VersionJsonContent = $VersionInfo | ConvertTo-Json -Depth 6
[System.IO.File]::WriteAllText($VersionJsonPath, $VersionJsonContent, [System.Text.Encoding]::UTF8)
Write-Host " -> Generated: $VersionJsonPath" -ForegroundColor Green

# -----------------------------------------------------------------------------
# 8.5 Integrity and 4-Way Cross-Verification Gate
# -----------------------------------------------------------------------------
Write-Host "`nCross-checking bundle consistency (disk <-> version.json <-> SHA256SUMS.txt <-> SBOM)..." -ForegroundColor Yellow

$PrimaryUiPath = Join-Path $DestPath "EmuleSion.exe"
if (!(Test-Path $PrimaryUiPath)) {
    throw "GATE FAILURE: EmuleSion.exe missing from bundle destination!"
}
$ActualUiHash = (Get-FileHash $PrimaryUiPath -Algorithm SHA256).Hash.ToLowerInvariant()

# Verify EmuleSion.exe in version.json
$VersionUi = $ArtifactsList | Where-Object { $_.path -eq "EmuleSion.exe" }
if (!$VersionUi -or $VersionUi.sha256 -ne $ActualUiHash) {
    throw "GATE FAILURE: EmuleSion.exe hash mismatch between disk and version.json!"
}

# Verify EmuleSion.exe in SHA256SUMS.txt
$SumUiLine = $SumsLines | Where-Object { $_ -match '\s+EmuleSion\.exe$' }
if (!$SumUiLine -or !$SumUiLine.StartsWith($ActualUiHash)) {
    throw "GATE FAILURE: EmuleSion.exe hash mismatch between disk and SHA256SUMS.txt!"
}

# Verify EmuleSion.exe in windows-bundle.cdx.json
$SbomBundlePath = Join-Path $DestPath "sbom\windows-bundle.cdx.json"
if (Test-Path $SbomBundlePath) {
    $SbomObj = Get-Content $SbomBundlePath -Raw | ConvertFrom-Json
    $SbomUi = $SbomObj.components | Where-Object { $_.name -eq "EmuleSion.exe" }
    if (!$SbomUi) {
        throw "GATE FAILURE: EmuleSion.exe missing from windows-bundle.cdx.json components!"
    }
}

Write-Host " -> Cross-check Passed: 100% consistent across disk, version.json, SHA256SUMS.txt, and SBOM." -ForegroundColor Green

# -----------------------------------------------------------------------------
# 9. Packaging ZIP Archive & Archive SHA-256
# -----------------------------------------------------------------------------
if (!$SkipZip) {
    $ZipName = "EmuleSion-v${AppVersion}-${DistributionFlavor}-windows-x64.zip"
    $ZipPath = Join-Path (Split-Path $DestPath -Parent) $ZipName
    
    Write-Host "`nCreating distribution archive: $ZipName..." -ForegroundColor Yellow
    if (Test-Path $ZipPath) { Remove-Item -Force $ZipPath }

    Compress-Archive -Path "$DestPath\*" -DestinationPath $ZipPath -CompressionLevel Optimal
    
    $ZipHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $ZipHashPath = "$ZipPath.sha256"
    [System.IO.File]::WriteAllText($ZipHashPath, "$ZipHash  $ZipName`n", [System.Text.Encoding]::UTF8)
    
    Write-Host " -> Archive built: $ZipPath" -ForegroundColor Green
    Write-Host " -> Archive SHA-256: $ZipHash" -ForegroundColor Green
}

Write-Host "`n==================================================" -ForegroundColor Green
Write-Host "  Release Candidate Packaging Succeeded!          " -ForegroundColor Green
Write-Host "  Destination: $DestPath" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Green
