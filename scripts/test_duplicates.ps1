# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Laurent Geynet "Loulach"

<#
.SYNOPSIS
    Garde-fou anti-duplication Rust pour eMuleRust.
.DESCRIPTION
    Execute scripts/find_duplicates.py (analyse hors-ligne : symboles homonymes,
    corps de fonctions identiques ou identiques apres renommage, fenetres de
    clones inter-fichiers) et echoue si un NOUVEAU groupe de clones exacts (B1)
    apparait hors de la liste blanche versionnee scripts/dup_allowlist.txt.

    Les sections A (homonymes) et B2 (corps normalises) sont informatives :
    elles contiennent des faux positifs legitimes (methodes de trait, squelettes
    d'encodeurs filaires, accesseurs bit-field) qui ne doivent pas bloquer la CI.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_duplicates.ps1
#>

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$Report = Join-Path $Root "target\dup-report.txt"
$Allowlist = Join-Path $Root "scripts\dup_allowlist.txt"

if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    Write-Error "python introuvable - requis pour scripts/find_duplicates.py"
    exit 1
}

Write-Host "==> Analyse de duplication Rust (symboles + corps + fenetres)..." -ForegroundColor Yellow
python scripts/find_duplicates.py | Out-File -Encoding utf8 $Report
if ($LASTEXITCODE -ne 0) {
    Write-Error "find_duplicates.py a echoue."
    exit $LASTEXITCODE
}
Write-Host "  Rapport complet : $Report" -ForegroundColor DarkGray

# -- Baseline ---------------------------------------------------------------
$baseline = Select-String -Path $Report -Pattern '^== Baseline ==\s+(\d+) fichiers, (\d+) lignes' |
    Select-Object -First 1
if ($baseline) {
    Write-Host ("  Baseline : {0} fichiers, {1} lignes" -f $baseline.Matches[0].Groups[1].Value, $baseline.Matches[0].Groups[2].Value) -ForegroundColor DarkGray
}

# -- Liste blanche ----------------------------------------------------------
$allowed = @{}
Get-Content $Allowlist | ForEach-Object {
    $line = $_.Trim()
    if ($line.Length -gt 0 -and -not $line.StartsWith('#')) {
        $allowed[$line] = $true
    }
}

# -- Section B1 : groupes de corps strictement identiques ---------------------
$lines = Get-Content $Report
$inB1 = $false
$newGroups = @()
$knownGroups = @()
foreach ($line in $lines) {
    if ($line -match '^== B1\.') { $inB1 = $true; continue }
    if ($line -match '^== B2\.') { break }
    if ($inB1 -and $line -match '^\s*\[(.+)\] x(\d+)') {
        $group = $Matches[1]
        if ($allowed.ContainsKey($group)) {
            $knownGroups += $group
            $allowed.Remove($group)
        } else {
            $newGroups += $group
        }
    }
}

foreach ($stale in $allowed.Keys) {
    Write-Host "  [INFO] Entree de liste blanche devenue obsolete : [$stale]" -ForegroundColor DarkYellow
}

if ($newGroups.Count -gt 0) {
    Write-Host "`n  [ECHEC] Nouveaux clones exacts detectes :" -ForegroundColor Red
    foreach ($g in $newGroups) {
        Write-Host "    - [$g]" -ForegroundColor Red
    }
    Write-Host "  Voir $Report (section B1). Mutualiser le code ou justifier" -ForegroundColor Red
    Write-Host "  chaque exception dans scripts/dup_allowlist.txt." -ForegroundColor Red
    exit 1
}

Write-Host ("  [OK] Aucun nouveau clone exact ({0} groupe(s) blanchi(s), voir liste)." -f $knownGroups.Count) -ForegroundColor Green
exit 0
