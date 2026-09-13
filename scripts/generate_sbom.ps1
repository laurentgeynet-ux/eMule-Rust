<#
.SYNOPSIS
    Generates CycloneDX 1.5 JSON Software Bill of Materials (SBOM) for Emule-Sion.
.DESCRIPTION
    1. Generates sbom/rust.cdx.json from Cargo.lock and cargo metadata.
    2. Generates sbom/flutter.cdx.json from apps/desktop/pubspec.lock.
    3. Generates sbom/windows-bundle.cdx.json from the assembled Windows bundle.
    4. Generates sbom/sbom.cdx.json consolidating the application distribution components.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\generate_sbom.ps1 -BundleDir dist\EmuleSion-Windows
#>

param(
    [string]$BundleDir = "dist\EmuleSion-Windows",
    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path $Root "$BundleDir\sbom"
}

if (!(Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$NowIso = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$AppVersion = "0.1.0-lab"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Emule-Sion CycloneDX 1.5 SBOM Generator         " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# -----------------------------------------------------------------------------
# 1. Rust SBOM (sbom/rust.cdx.json)
# -----------------------------------------------------------------------------
Write-Host "`n[1/4] Generating Rust CycloneDX SBOM from Cargo dependencies..." -ForegroundColor Yellow

$MetadataRaw = & cargo metadata --format-version 1
$Metadata = $MetadataRaw | ConvertFrom-Json

$RustComponents = [System.Collections.Generic.List[PSCustomObject]]::new()

foreach ($pkg in $Metadata.packages) {
    # Skip workspace internal crates from library dependency listing if needed, or mark as application/library
    $isWorkspace = $Metadata.workspace_members -contains $pkg.id
    
    $componentType = if ($isWorkspace) { "application" } else { "library" }
    $purl = "pkg:cargo/$($pkg.name)@$($pkg.version)"
    
    $hashes = @()
    # Check if checksum is available in metadata or Cargo.lock
    $licenses = @()
    if ($pkg.license) {
        $licenses += @{ license = @{ id = $pkg.license } }
    }

    $component = [ordered]@{
        type = $componentType
        name = $pkg.name
        version = $pkg.version
        purl = $purl
        scope = if ($isWorkspace) { "required" } else { "required" }
    }
    if ($licenses.Count -gt 0) {
        $component["licenses"] = $licenses
    }
    if (![string]::IsNullOrEmpty($pkg.description)) {
        $component["description"] = $pkg.description
    }

    $RustComponents.Add([PSCustomObject]$component)
}

$RustSbom = [ordered]@{
    '$schema' = "http://cyclonedx.org/schema/bom-1.5.json"
    bomFormat = "CycloneDX"
    specVersion = "1.5"
    serialNumber = "urn:uuid:$([guid]::NewGuid())"
    version = 1
    metadata = [ordered]@{
        timestamp = $NowIso
        component = [ordered]@{
            type = "application"
            name = "mule-daemon"
            version = $AppVersion
            description = "Emule-Sion Rust daemon & protocol core"
        }
    }
    components = $RustComponents
}

$RustSbomPath = Join-Path $OutputDir "rust.cdx.json"
$RustSbom | ConvertTo-Json -Depth 10 | Out-File -FilePath $RustSbomPath -Encoding utf8
Write-Host " -> Generated: $RustSbomPath ($($RustComponents.Count) components)" -ForegroundColor Green

# -----------------------------------------------------------------------------
# 2. Flutter / Dart SBOM (sbom/flutter.cdx.json)
# -----------------------------------------------------------------------------
Write-Host "`n[2/4] Generating Flutter CycloneDX SBOM from pubspec.lock..." -ForegroundColor Yellow

$PubspecLockPath = Join-Path $Root "apps\desktop\pubspec.lock"
$FlutterComponents = [System.Collections.Generic.List[PSCustomObject]]::new()

if (Test-Path $PubspecLockPath) {
    $Lines = Get-Content $PubspecLockPath
    $currentPkg = $null
    $currentDep = $null
    $currentVer = $null
    $currentSha = $null
    $currentSource = $null

    foreach ($line in $Lines) {
        if ($line -match '^  ([a-zA-Z0-9_]+):$') {
            if ($currentPkg -and $currentVer) {
                $comp = [ordered]@{
                    type = "library"
                    name = $currentPkg
                    version = $currentVer
                    purl = "pkg:pub/$currentPkg@$currentVer"
                    scope = if ($currentDep -match 'dev') { "optional" } else { "required" }
                }
                if ($currentSha) {
                    $comp["hashes"] = @(@{ alg = "SHA-256"; content = $currentSha })
                }
                $FlutterComponents.Add([PSCustomObject]$comp)
            }
            $currentPkg = $Matches[1]
            $currentDep = $null
            $currentVer = $null
            $currentSha = $null
            $currentSource = $null
        }
        elseif ($line -match '^\s+dependency:\s*"?([^"\s]+)"?') {
            $currentDep = $Matches[1]
        }
        elseif ($line -match '^\s+version:\s*"?([^"\s]+)"?') {
            $currentVer = $Matches[1]
        }
        elseif ($line -match '^\s+sha256:\s*"?([a-fA-F0-9]{64})"?') {
            $currentSha = $Matches[1]
        }
        elseif ($line -match '^\s+source:\s*"?([^"\s]+)"?') {
            $currentSource = $Matches[1]
        }
    }
    if ($currentPkg -and $currentVer) {
        $comp = [ordered]@{
            type = "library"
            name = $currentPkg
            version = $currentVer
            purl = "pkg:pub/$currentPkg@$currentVer"
            scope = if ($currentDep -match 'dev') { "optional" } else { "required" }
        }
        if ($currentSha) {
            $comp["hashes"] = @(@{ alg = "SHA-256"; content = $currentSha })
        }
        $FlutterComponents.Add([PSCustomObject]$comp)
    }
}

$FlutterSbom = [ordered]@{
    '$schema' = "http://cyclonedx.org/schema/bom-1.5.json"
    bomFormat = "CycloneDX"
    specVersion = "1.5"
    serialNumber = "urn:uuid:$([guid]::NewGuid())"
    version = 1
    metadata = [ordered]@{
        timestamp = $NowIso
        component = [ordered]@{
            type = "application"
            name = "desktop"
            version = $AppVersion
            description = "Emule-Sion Flutter Desktop Client"
        }
    }
    components = $FlutterComponents
}

$FlutterSbomPath = Join-Path $OutputDir "flutter.cdx.json"
$FlutterSbom | ConvertTo-Json -Depth 10 | Out-File -FilePath $FlutterSbomPath -Encoding utf8
Write-Host " -> Generated: $FlutterSbomPath ($($FlutterComponents.Count) components)" -ForegroundColor Green

# -----------------------------------------------------------------------------
# 3. Windows Bundle Artifacts SBOM (sbom/windows-bundle.cdx.json)
# -----------------------------------------------------------------------------
Write-Host "`n[3/4] Generating Windows Bundle Artifacts SBOM..." -ForegroundColor Yellow

$BundlePath = Join-Path $Root $BundleDir
$BundleComponents = [System.Collections.Generic.List[PSCustomObject]]::new()

if (Test-Path $BundlePath) {
    $Files = Get-ChildItem -Path $BundlePath -Recurse -File | Where-Object {
        $_.FullName -notmatch '\\sbom\\' -and $_.Name -ne "SHA256SUMS.txt" -and $_.Name -ne "version.json"
    }

    foreach ($file in $Files) {
        $relPath = ($file.FullName.Substring($BundlePath.Length)).TrimStart('\').Replace('\', '/')
        $sha = (Get-FileHash -Path $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        
        $compType = if ($file.Extension -eq ".exe") { "application" } elseif ($file.Extension -eq ".dll") { "library" } else { "file" }

        $comp = [ordered]@{
            type = $compType
            name = $relPath
            version = $AppVersion
            hashes = @(
                @{
                    alg = "SHA-256"
                    content = $sha
                }
            )
        }
        $BundleComponents.Add([PSCustomObject]$comp)
    }
}

$BundleSbom = [ordered]@{
    '$schema' = "http://cyclonedx.org/schema/bom-1.5.json"
    bomFormat = "CycloneDX"
    specVersion = "1.5"
    serialNumber = "urn:uuid:$([guid]::NewGuid())"
    version = 1
    metadata = [ordered]@{
        timestamp = $NowIso
        component = [ordered]@{
            type = "application"
            name = "EmuleSion-Windows"
            version = $AppVersion
            description = "Emule-Sion Standalone Windows Release Bundle"
        }
    }
    components = $BundleComponents
}

$BundleSbomPath = Join-Path $OutputDir "windows-bundle.cdx.json"
$BundleSbom | ConvertTo-Json -Depth 10 | Out-File -FilePath $BundleSbomPath -Encoding utf8
Write-Host " -> Generated: $BundleSbomPath ($($BundleComponents.Count) artifacts)" -ForegroundColor Green

# -----------------------------------------------------------------------------
# 4. Consolidated Distribution SBOM (sbom/sbom.cdx.json)
# -----------------------------------------------------------------------------
Write-Host "`n[4/4] Consolidating complete release CycloneDX SBOM..." -ForegroundColor Yellow

$AllComponents = [System.Collections.Generic.List[PSCustomObject]]::new()
$AllComponents.AddRange($BundleComponents)
$AllComponents.AddRange($RustComponents)
$AllComponents.AddRange($FlutterComponents)

$ConsolidatedSbom = [ordered]@{
    '$schema' = "http://cyclonedx.org/schema/bom-1.5.json"
    bomFormat = "CycloneDX"
    specVersion = "1.5"
    serialNumber = "urn:uuid:$([guid]::NewGuid())"
    version = 1
    metadata = [ordered]@{
        timestamp = $NowIso
        component = [ordered]@{
            type = "application"
            name = "Emule-Sion"
            version = $AppVersion
            description = "Emule-Sion Closed Loopback eD2K Client & Node"
        }
    }
    components = $AllComponents
}

$ConsolidatedPath = Join-Path $OutputDir "sbom.cdx.json"
$ConsolidatedSbom | ConvertTo-Json -Depth 10 | Out-File -FilePath $ConsolidatedPath -Encoding utf8
Write-Host " -> Generated: $ConsolidatedPath ($($AllComponents.Count) total components)" -ForegroundColor Green

Write-Host "`n==================================================" -ForegroundColor Green
Write-Host "  CycloneDX SBOM Generation Succeeded!            " -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
