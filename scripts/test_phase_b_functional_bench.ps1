# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Laurent Geynet "Loulach"

<#
.SYNOPSIS
    Phase B - Banc fonctionnel RPC complet (docs/qa/PLAN_TEST_COMPLET.md).
.DESCRIPTION
    Extrait le bundle QA loopback, demarre le daemon en mode portable et
    exerce l'integralite de la surface JSON-RPC : identite/credits,
    cycle download, formats .met/.dat, partages/uploads, sources,
    securite (anti-SSRF, ipfilter, profils), Kad, settings/limits,
    categories, amis, historique, listener, systeme.
    Verdict : PASS/FAIL par appel + rapport final. Hors-ligne strict.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test_phase_b_functional_bench.ps1
#>

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

# -----------------------------------------------------------------------
# Resultats
# -----------------------------------------------------------------------
$script:Results = New-Object System.Collections.Generic.List[object]
$script:ReqId = 1000

function Add-Result([string]$Section, [string]$Name, [string]$Outcome, [string]$Detail = "") {
    $script:Results.Add([pscustomobject]@{
        Section = $Section; Name = $Name; Outcome = $Outcome; Detail = $Detail
    })
    $color = switch ($Outcome) { "PASS" { "Green" } "FAIL" { "Red" } default { "Yellow" } }
    $detailStr = if ($Detail) { "  ($Detail)" } else { "" }
    Write-Host ("  [{0}] {1}{2}" -f $Outcome, $Name, $detailStr) -ForegroundColor $color
}

# Envoie un appel JSON-RPC et classe le resultat.
# Expect : "ok" (result requis), "err" (error requis), "any" (les deux).
function Test-Rpc {
    param(
        [System.Net.WebSockets.ClientWebSocket]$Ws,
        [string]$Section, [string]$Name, [string]$Method,
        [hashtable]$Params = @{},
        [string]$Expect = "ok",
        [scriptblock]$Validate = $null,
        [int]$TimeoutMs = 8000
    )
    $script:ReqId++
    try {
        $resp = Invoke-WsRpc $Ws @{ jsonrpc = "2.0"; id = $script:ReqId; method = $Method; params = $Params } $TimeoutMs
        $hasErr = ($null -ne $resp.error)
        switch ($Expect) {
            "ok" {
                if ($hasErr) {
                    Add-Result $Section $Name "FAIL" "unexpected error: $($resp.error.message)"
                    return $null
                }
                if ($Validate) {
                    $vErr = & $Validate $resp.result
                    if ($vErr) { Add-Result $Section $Name "FAIL" $vErr; return $resp.result }
                }
                Add-Result $Section $Name "PASS"
                return $resp.result
            }
            "err" {
                if ($hasErr) { Add-Result $Section $Name "PASS" "rejected: $($resp.error.message)"; return $resp.error }
                Add-Result $Section $Name "FAIL" "expected error, got result: $($resp.result | ConvertTo-Json -Compress -Depth 3)"
                return $null
            }
            default {
                if ($hasErr) { Add-Result $Section $Name "PASS" "business error accepted: $($resp.error.message)" }
                else { Add-Result $Section $Name "PASS" "ok" }
                return $resp
            }
        }
    } catch {
        Add-Result $Section $Name "FAIL" "exception: $($_.Exception.Message)"
        return $null
    }
}

# -----------------------------------------------------------------------
# Helpers WS (meme pattern que test_qa_loopback_e2e.ps1)
# -----------------------------------------------------------------------
function Receive-FullWsText([System.Net.WebSockets.ClientWebSocket]$ws, [System.Threading.CancellationTokenSource]$cts) {
    $ms = [System.IO.MemoryStream]::new()
    $buf = [byte[]]::new(8192)
    $recSeg = [System.ArraySegment[byte]]::new($buf)
    do {
        $task = $ws.ReceiveAsync($recSeg, $cts.Token)
        $task.Wait()
        $ms.Write($buf, 0, $task.Result.Count)
    } while (!$task.Result.EndOfMessage)
    return [System.Text.Encoding]::UTF8.GetString($ms.ToArray())
}

function Invoke-WsRpc([System.Net.WebSockets.ClientWebSocket]$ws, [hashtable]$request, [int]$timeoutMs = 8000) {
    $reqId = $request.id
    $json = $request | ConvertTo-Json -Compress -Depth 10
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
    $seg = [System.ArraySegment[byte]]::new($bytes)
    $cts = [System.Threading.CancellationTokenSource]::new($timeoutMs)
    $ws.SendAsync($seg, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cts.Token).Wait()

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt $timeoutMs) {
        $remain = [math]::Max(500, ($timeoutMs - $sw.ElapsedMilliseconds))
        $recCts = [System.Threading.CancellationTokenSource]::new([int]$remain)
        try {
            $respText = Receive-FullWsText $ws $recCts
        } catch {
            throw "WS receive timeout/error for id $reqId"
        }
        $parsed = $respText | ConvertFrom-Json
        if ($null -ne $parsed.id -and $parsed.id -eq $reqId) {
            return $parsed
        }
    }
    throw "Timeout waiting for JSON-RPC response to request id $reqId after $timeoutMs ms"
}

function Connect-DaemonWebSocket([string]$endpoint, [string]$token) {
    $ws = [System.Net.WebSockets.ClientWebSocket]::new()
    $ws.Options.SetRequestHeader("Authorization", "Bearer $token")
    $ws.ConnectAsync([System.Uri]::new($endpoint), [System.Threading.CancellationTokenSource]::new(5000).Token).Wait()
    return $ws
}

function Start-PackagedDaemon([string]$binPath, [string]$dataDir) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $binPath
    # --profile LocalLab + --upnp false : confinement loopback strict,
    # zero trafic sortant (les bundles QA demarrent sinon en profil
    # public avec auto-map UPnP sur la vraie gateway).
    $psi.Arguments = "--workdir `"$dataDir`" --mode portable --profile LocalLab --upnp false"
    $psi.WorkingDirectory = (Split-Path -Parent $binPath)
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    if (!$proc) { throw "Failed to start mule-daemon process" }
    # Drain continu - evite le blocage pipe ~64 Ko.
    $script:DaemonStdoutTask = $proc.StandardOutput.ReadToEndAsync()
    $script:DaemonStderrTask = $proc.StandardError.ReadToEndAsync()

    $sessionPath = Join-Path $dataDir "session.json"
    $t = 0
    while (!(Test-Path $sessionPath) -and $t -lt 15) {
        Start-Sleep -Milliseconds 200
        $t += 0.2
        if ($proc.HasExited) {
            throw "Daemon died during boot ($($proc.ExitCode))"
        }
    }
    if (!(Test-Path $sessionPath)) { throw "Timeout waiting for session.json" }
    return $proc
}

# -----------------------------------------------------------------------
# Generateurs de fixtures .met/.dat minimaux valides
# -----------------------------------------------------------------------
function Write-BinFile([string]$Path, [byte[]]$Bytes) {
    [System.IO.File]::WriteAllBytes($Path, $Bytes)
    return $Path
}

function New-ServerMet {
    # [0xE0][u32 count=1][u32 ip][u16 port][u32 tagcount=0]
    $b = [System.Collections.Generic.List[byte]]::new()
    $b.Add(0xE0)
    $b.AddRange([BitConverter]::GetBytes([uint32]1))
    $b.AddRange([byte[]](195, 154, 122, 10))
    $b.AddRange([BitConverter]::GetBytes([uint16]4661))
    $b.AddRange([BitConverter]::GetBytes([uint32]0))
    return $b.ToArray()
}

function New-NodesDatV0 {
    # [u32 count=1][16B id][u32 ip][u16 udp][u16 tcp]
    $b = [System.Collections.Generic.List[byte]]::new()
    $b.AddRange([BitConverter]::GetBytes([uint32]1))
    $b.AddRange([byte[]](0x11,0x11,0x11,0x11,0x11,0x11,0x11,0x11,0x11,0x11,0x11,0x11,0x11,0x11,0x11,0x11))
    $b.AddRange([byte[]](80, 80, 80, 80))
    $b.AddRange([BitConverter]::GetBytes([uint16]4672))
    $b.AddRange([BitConverter]::GetBytes([uint16]4662))
    return $b.ToArray()
}

function New-ClientsMet {
    # v0x11 legacy : [0x11][u32 count=1][record 38B = 16B key + u32 upLo + u32 dlLo + u32 lastSeen + u32 upHi + u32 dlHi + u16 reserved]
    $b = [System.Collections.Generic.List[byte]]::new()
    $b.Add(0x11)
    $b.AddRange([BitConverter]::GetBytes([uint32]1))
    $rec = [byte[]]::new(38)
    for ($i = 0; $i -lt 16; $i++) { $rec[$i] = 0x42 }
    [BitConverter]::GetBytes([uint32]12345).CopyTo($rec, 16)   # uploaded lo
    [BitConverter]::GetBytes([uint32]6789).CopyTo($rec, 20)    # downloaded lo
    [BitConverter]::GetBytes([uint32]1700000000).CopyTo($rec, 24) # last seen
    $b.AddRange($rec)
    return $b.ToArray()
}

function New-EmfriendsMet {
    # [0x0E][u32 count=1][16B hash][u32 ip][u16 port][u32 lastseen][u32 lastchatted][u32 tagcount=0]
    $b = [System.Collections.Generic.List[byte]]::new()
    $b.Add(0x0E)
    $b.AddRange([BitConverter]::GetBytes([uint32]1))
    $b.AddRange([byte[]](0x33,0x33,0x33,0x33,0x33,0x33,0x33,0x33,0x33,0x33,0x33,0x33,0x33,0x33,0x33,0x33))
    $b.AddRange([BitConverter]::GetBytes([uint32]0x0100007F))   # 127.0.0.1 LE
    $b.AddRange([BitConverter]::GetBytes([uint16]4662))
    $b.AddRange([BitConverter]::GetBytes([uint32]1700000000))
    $b.AddRange([BitConverter]::GetBytes([uint32]1700000000))
    $b.AddRange([BitConverter]::GetBytes([uint32]0))
    return $b.ToArray()
}

function New-CancelledMet {
    # v1 : [0x0F][0x01][u32 seed][u32 count=1][16B masked hash][u8 tagcount=0]
    $b = [System.Collections.Generic.List[byte]]::new()
    $b.Add(0x0F)
    $b.Add(0x01)
    $b.AddRange([BitConverter]::GetBytes([uint32]0x12345678))
    $b.AddRange([BitConverter]::GetBytes([uint32]1))
    $b.AddRange([byte[]](0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77,0x77))
    $b.Add(0x00)
    return $b.ToArray()
}

function New-IpfilterDat {
    return [System.Text.Encoding]::ASCII.GetBytes("1.2.3.4 - 1.2.3.9 , 100 , bench-blocked`r`n# comment`r`n5.6.7.8 - 5.6.7.8 , 090 , single`r`n")
}

# =======================================================================
# 0. Sandbox + boot
# =======================================================================
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Phase B - Banc fonctionnel RPC (loopback)       " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

$zip = Get-ChildItem (Join-Path $Root "dist") -Filter "eMuleRust-*-qa-loopback-windows-x64.zip" |
       Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (!$zip) { throw "QA loopback bundle ZIP not found in dist/ - run build_windows_release.ps1 -Flavor qa-loopback first" }

$WorkRoot = Join-Path $env:TEMP ("eMuleRust-PhaseB-" + (New-Guid).ToString("N").Substring(0,8))
$BundleDir = Join-Path $WorkRoot "bundle"
$WorkDir = Join-Path $WorkRoot "workdir"
New-Item -ItemType Directory -Force -Path $BundleDir, $WorkDir | Out-Null
Expand-Archive -Path $zip.FullName -DestinationPath $BundleDir -Force
# Le daemon RPC est bin\mule-daemon.exe (eMuleRust.exe = UI Flutter).
$DaemonBin = Get-ChildItem $BundleDir -Recurse -Filter "mule-daemon.exe" | Select-Object -First 1 -ExpandProperty FullName
if (!$DaemonBin) { throw "mule-daemon.exe not found in extracted bundle" }
Write-Host " -> Bundle: $($zip.Name)" -ForegroundColor Gray
Write-Host " -> Sandbox: $WorkRoot" -ForegroundColor Gray

$daemonProcess = $null
$ws = $null
$exitCode = 0

try {
    $daemonProcess = Start-PackagedDaemon $DaemonBin $WorkDir
    $SessionData = Get-Content (Join-Path $WorkDir "session.json") -Raw | ConvertFrom-Json
    $Token = (Get-Content (Join-Path $WorkDir "auth.token") -Raw).Trim()
    $ws = Connect-DaemonWebSocket $SessionData.endpoint $Token
    Write-Host " -> Daemon up (PID $($daemonProcess.Id)) on $($SessionData.endpoint)" -ForegroundColor Gray

    $hello = Invoke-WsRpc $ws @{ jsonrpc="2.0"; id=1; method="system.hello"; params=@{ protocolVersion=1; token=$Token } }
    Add-Result "B.0" "system.hello handshake" "PASS" "v$($hello.result.version)"

    # Dossier de fixtures partage + repertoire de partage pour shares.add
    $FixDir = Join-Path $WorkRoot "bench-files"
    New-Item -ItemType Directory -Force -Path $FixDir | Out-Null
    $shareDir = Join-Path $WorkRoot "share-dir"
    New-Item -ItemType Directory -Force -Path $shareDir | Out-Null
    "eMuleRust phase-B share content" | Out-File -Encoding ascii (Join-Path $shareDir "bench-share.txt")

    # ===================================================================
    # B.1 Identite, SUI, credits
    # ===================================================================
    Write-Host "`n[B.1] Identite / SUI / Credits" -ForegroundColor Yellow
    $id1 = Test-Rpc $ws "B.1" "identity.status" "identity.status" -Validate {
        param($r) if ($null -eq $r.hasKey -and $null -eq $r.pubkeyHash) { "missing identity fields" }
    }
    $script:hash1 = if ($id1) { $id1.pubkeyHash } else { $null }
    Test-Rpc $ws "B.1" "identity.rotate" "identity.rotate" | Out-Null
    Test-Rpc $ws "B.1" "identity.status (post-rotate)" "identity.status" -Params @{} -Validate {
        param($r) if ($script:hash1 -and $r.pubkeyHash -eq $script:hash1) { "pubkeyHash unchanged after rotate" }
    } | Out-Null
    Test-Rpc $ws "B.1" "identity.importEmuleIdentity (missing file)" "identity.importEmuleIdentity" @{ cryptkeyPath = "Z:\nonexistent\cryptkey.dat" } -Expect "err" | Out-Null
    Test-Rpc $ws "B.1" "credits.list" "credits.list" | Out-Null
    Test-Rpc $ws "B.1" "credits.stats" "credits.stats" | Out-Null
    $clientsMet = Write-BinFile (Join-Path $FixDir "clients.met") (New-ClientsMet)
    Test-Rpc $ws "B.1" "credits.importClientsMet (valid)" "credits.importClientsMet" @{ metPath = $clientsMet } | Out-Null
    $badMet = Write-BinFile (Join-Path $FixDir "garbage.met") ([byte[]](0xDE,0xAD,0xBE,0xEF))
    Test-Rpc $ws "B.1" "credits.importClientsMet (malformed)" "credits.importClientsMet" @{ metPath = $badMet } -Expect "err" | Out-Null

    # ===================================================================
    # B.2 Cycle download (hors transfert reel = e2e dedie)
    # ===================================================================
    Write-Host "`n[B.2] Cycle download" -ForegroundColor Yellow
    $fixResp = Test-Rpc $ws "B.2" "lab.createFixture" "lab.createFixture" @{ fixtureType = "sample_1mb" }
    $fixLink = $fixResp.ed2kLink; $fixHash = $fixResp.ed2kHash; $fixPath = $fixResp.filePath

    $addResp = Test-Rpc $ws "B.2" "downloads.addEd2kLink (valid)" "downloads.addEd2kLink" @{ link = $fixLink }
    $script:dlId = $addResp.downloadId
    Test-Rpc $ws "B.2" "downloads.addEd2kLink (malformed)" "downloads.addEd2kLink" @{ link = "ed2k://|file|x|abc|zz|/" } -Expect "err" | Out-Null
    Test-Rpc $ws "B.2" "downloads.addEd2kLink (non-ed2k)" "downloads.addEd2kLink" @{ link = "http://example.com/f" } -Expect "err" | Out-Null
    Test-Rpc $ws "B.2" "downloads.list" "downloads.list" -Validate {
        param($r) if ($r.downloads.Count -lt 1) { "no download listed" }
    } | Out-Null
    Test-Rpc $ws "B.2" "downloads.get" "downloads.get" @{ id = $dlId } -Validate {
        param($r) if ($r.id -ne $script:dlId) { "wrong download returned" }
    } | Out-Null
    Test-Rpc $ws "B.2" "downloads.pause" "downloads.pause" @{ id = $dlId } | Out-Null
    Test-Rpc $ws "B.2" "downloads.resume" "downloads.resume" @{ id = $dlId } | Out-Null
    Test-Rpc $ws "B.2" "downloads.pause (again)" "downloads.pause" @{ id = $dlId } | Out-Null
    Test-Rpc $ws "B.2" "downloads.rename" "downloads.rename" @{ id = $dlId; newName = "bench-renamed.bin" } | Out-Null
    Test-Rpc $ws "B.2" "downloads.setPriority" "downloads.setPriority" @{ id = $dlId; priority = "high" } "any" | Out-Null
    Test-Rpc $ws "B.2" "downloads.setMaxSources" "downloads.setMaxSources" @{ id = $dlId; maxSources = 8 } "any" | Out-Null
    $catDir = Join-Path $WorkRoot "cat-dir"; New-Item -ItemType Directory -Force -Path $catDir | Out-Null
    Test-Rpc $ws "B.2" "downloads.setDestination" "downloads.setDestination" @{ id = $dlId; targetDirectory = $catDir } "any" | Out-Null
    Test-Rpc $ws "B.2" "downloads.getSources" "downloads.getSources" @{ id = $dlId } -Expect "any" | Out-Null
    Test-Rpc $ws "B.2" "downloads.retry" "downloads.retry" @{ id = $dlId } -Expect "any" | Out-Null
    Test-Rpc $ws "B.2" "downloads.isCancelled" "downloads.isCancelled" @{ fileHash = $fixHash } | Out-Null
    Test-Rpc $ws "B.2" "downloads.inspectPartMet (missing)" "downloads.inspectPartMet" @{ metPath = "Z:\nope.part.met" } -Expect "err" | Out-Null
    Test-Rpc $ws "B.2" "downloads.importPartMet (missing)" "downloads.importPartMet" @{ metPath = "Z:\nope.part.met" } -Expect "err" | Out-Null
    $cancelMet = Write-BinFile (Join-Path $FixDir "cancelled.met") (New-CancelledMet)
    Test-Rpc $ws "B.2" "downloads.importCancelledMet (valid)" "downloads.importCancelledMet" @{ metPath = $cancelMet } -Expect "any" | Out-Null
    Test-Rpc $ws "B.2" "downloads.importCancelledMet (malformed)" "downloads.importCancelledMet" @{ metPath = $badMet } -Expect "err" | Out-Null
    # debug.injectRange sur le download puis suppression
    Test-Rpc $ws "B.2" "debug.injectRange" "debug.injectRange" @{ downloadId = $dlId; offset = 0; dataHex = "DEADBEEF01020304" } -Expect "any" | Out-Null
    Test-Rpc $ws "B.2" "downloads.setCategory" "downloads.setCategory" @{ id = $dlId; categoryId = "" } -Expect "any" | Out-Null
    # NB : downloads.remove reporte a la fin de B.5 - les sources sont
    # liees au fileHash du download.

    # ===================================================================
    # B.3 Formats .met/.dat (imports via RPC)
    # ===================================================================
    Write-Host "`n[B.3] Formats .met / .dat" -ForegroundColor Yellow
    $serverMet = Write-BinFile (Join-Path $FixDir "server.met") (New-ServerMet)
    Test-Rpc $ws "B.3" "servers.inspectMet (valid)" "servers.inspectMet" @{ path = $serverMet } | Out-Null
    $truncMet = Write-BinFile (Join-Path $FixDir "server_trunc.met") ([byte[]](0xE0,0x01,0x00,0x00,0x00))
    Test-Rpc $ws "B.3" "servers.inspectMet (truncated)" "servers.inspectMet" @{ path = $truncMet } -Expect "err" | Out-Null
    Test-Rpc $ws "B.3" "servers.importMet (valid)" "servers.importMet" @{ path = $serverMet } | Out-Null
    Test-Rpc $ws "B.3" "servers.list (post-import)" "servers.list" -Validate {
        param($r) if ($r.servers.Count -lt 1) { "imported server not listed" }
    } | Out-Null

    $nodesDat = Write-BinFile (Join-Path $FixDir "nodes.dat") (New-NodesDatV0)
    Test-Rpc $ws "B.3" "kad.inspectNodesDat (valid)" "kad.inspectNodesDat" @{ path = $nodesDat } | Out-Null
    Test-Rpc $ws "B.3" "kad.importNodesDat (valid)" "kad.importNodesDat" @{ path = $nodesDat } | Out-Null
    Test-Rpc $ws "B.3" "kad.inspectNodesDat (malformed)" "kad.inspectNodesDat" @{ path = $badMet } -Expect "err" | Out-Null

    $emfriends = Write-BinFile (Join-Path $FixDir "emfriends.met") (New-EmfriendsMet)
    Test-Rpc $ws "B.3" "friends.importEmfriendsMet (valid)" "friends.importEmfriendsMet" @{ metPath = $emfriends } -Expect "any" | Out-Null
    Test-Rpc $ws "B.3" "friends.importEmfriendsMet (malformed)" "friends.importEmfriendsMet" @{ metPath = $badMet } -Expect "err" | Out-Null
    Test-Rpc $ws "B.3" "shares.importKnownMet (missing)" "shares.importKnownMet" @{ metPath = "Z:\nope\known.met" } -Expect "err" | Out-Null

    # ===================================================================
    # B.4 Partages / uploads
    # ===================================================================
    Write-Host "`n[B.4] Partages / uploads" -ForegroundColor Yellow
    # shares.add enregistre UN fichier (fileHash/fileName/fileSize/filePath)
    Test-Rpc $ws "B.4" "shares.add" "shares.add" @{ fileHash = $fixHash; fileName = "sample_1mb.bin"; fileSize = 1048576; filePath = $fixPath } | Out-Null
    Test-Rpc $ws "B.4" "shares.list" "shares.list" -Validate {
        param($r) if ($r.sharedFiles.Count -lt 1) { "share not listed" }
    } | Out-Null
    $shareKey = $fixHash
    if ($shareKey) {
        Test-Rpc $ws "B.4" "shares.rename" "shares.rename" @{ fileHash = $shareKey; newName = "bench-share-renamed.txt" } -Expect "any" | Out-Null
        Test-Rpc $ws "B.4" "shares.setEnabled(false)" "shares.setEnabled" @{ fileHash = $shareKey; enabled = $false } -Expect "any" | Out-Null
        Test-Rpc $ws "B.4" "shares.setUploadPriority" "shares.setUploadPriority" @{ fileHash = $shareKey; priority = "high" } -Expect "any" | Out-Null
        Test-Rpc $ws "B.4" "shares.setCategory" "shares.setCategory" @{ fileHash = $shareKey; categoryId = "" } -Expect "any" | Out-Null
    } else {
        Add-Result "B.4" "shares mutations" "FAIL" "no share hash available"
    }
    Test-Rpc $ws "B.4" "shares.rescan" "shares.rescan" @{ directory = $shareDir } -Expect "any" | Out-Null
    Test-Rpc $ws "B.4" "uploads.list" "uploads.list" | Out-Null
    Test-Rpc $ws "B.4" "uploads.status" "uploads.status" | Out-Null
    Test-Rpc $ws "B.4" "uploads.cancel (no active)" "uploads.cancel" @{ clientAddress = "127.0.0.1:1" } -Expect "any" | Out-Null
    if ($shareKey) {
        Test-Rpc $ws "B.4" "shares.remove" "shares.remove" @{ fileHash = $shareKey } -Expect "any" | Out-Null
    }

    # ===================================================================
    # B.5 Sources
    # ===================================================================
    Write-Host "`n[B.5] Sources" -ForegroundColor Yellow
    Test-Rpc $ws "B.5" "sources.addManual" "sources.addManual" @{ fileHash = $fixHash; address = "127.0.0.1:9999" } -Expect "any" | Out-Null
    Test-Rpc $ws "B.5" "sources.addManual (bad addr)" "sources.addManual" @{ fileHash = $fixHash; address = "not-an-addr" } -Expect "err" | Out-Null
    Test-Rpc $ws "B.5" "sources.list" "sources.list" @{ fileHash = $fixHash } | Out-Null
    Test-Rpc $ws "B.5" "sources.status" "sources.status" @{ fileHash = $fixHash } -Expect "any" | Out-Null
    Test-Rpc $ws "B.5" "sources.connect (dead peer)" "sources.connect" @{ address = "127.0.0.1:1" } -Expect "any" | Out-Null
    Test-Rpc $ws "B.5" "sources.disconnect" "sources.disconnect" @{ address = "127.0.0.1:1" } -Expect "any" | Out-Null
    Test-Rpc $ws "B.5" "sources.clear" "sources.clear" @{ fileHash = $fixHash } | Out-Null
    Test-Rpc $ws "B.5" "downloads.remove" "downloads.remove" @{ id = $script:dlId } | Out-Null

    # ===================================================================
    # B.6 Securite : anti-SSRF, ipfilter, profils
    # ===================================================================
    Write-Host "`n[B.6] Securite" -ForegroundColor Yellow
    Test-Rpc $ws "B.6" "servers.updateFromUrl (link-local SSRF)" "servers.updateFromUrl" @{ url = "http://169.254.169.254/latest/meta-data" } "err" | Out-Null
    Test-Rpc $ws "B.6" "servers.updateFromUrl (private LAN)" "servers.updateFromUrl" @{ url = "http://192.168.1.254/server.met" } "err" | Out-Null
    Test-Rpc $ws "B.6" "servers.updateFromUrl (file scheme)" "servers.updateFromUrl" @{ url = "file:///c:/windows/win.ini" } "err" | Out-Null
    Test-Rpc $ws "B.6" "servers.updateFromUrl (public, reject LocalLab)" "servers.updateFromUrl" @{ url = "http://8.8.8.8/server.met" } "err" | Out-Null
    # URL loopback autorisee par la policy -> vrai fetch, connexion
    # refusee sur port ferme -> erreur metier (pipeline complet, offline)
    Test-Rpc $ws "B.6" "servers.updateFromUrl (loopback, conn refused)" "servers.updateFromUrl" @{ url = "http://127.0.0.1:9/server.met" } "err" | Out-Null
    Test-Rpc $ws "B.6" "kad.bootstrapFromUrl (link-local SSRF)" "kad.bootstrapFromUrl" @{ url = "http://169.254.169.254/x" } "err" | Out-Null
    Test-Rpc $ws "B.6" "kad.bootstrapFromUrl (file scheme)" "kad.bootstrapFromUrl" @{ url = "file:///etc/passwd" } "err" | Out-Null

    $ipf = Write-BinFile (Join-Path $FixDir "ipfilter.dat") (New-IpfilterDat)
    Test-Rpc $ws "B.6" "filter.importFile" "filter.importFile" @{ filePath = $ipf } "any" | Out-Null
    Test-Rpc $ws "B.6" "filter.getStatus" "filter.getStatus" | Out-Null
    Test-Rpc $ws "B.6" "filter.setEnabled(true)" "filter.setEnabled" @{ enabled = $true } "any" | Out-Null
    Test-Rpc $ws "B.6" "filter.setEnabled(false)" "filter.setEnabled" @{ enabled = $false } "any" | Out-Null
    Test-Rpc $ws "B.6" "filter.clear" "filter.clear" | Out-Null
    Test-Rpc $ws "B.6" "network.getPolicy" "network.getPolicy" -Validate {
        param($r) if (!$r.profile) { "missing profile" }
    } | Out-Null
    Test-Rpc $ws "B.6" "network.setProfile (LocalLab)" "network.setProfile" @{ profile = "LocalLab" } | Out-Null
    Test-Rpc $ws "B.6" "network.getPolicy (confinement)" "network.getPolicy" -Validate {
        param($r) if ($r.profile -notmatch 'LocalLab|local') { "expected LocalLab, got $($r.profile)" }
    } | Out-Null
    Test-Rpc $ws "B.6" "network.setProfile (invalid)" "network.setProfile" @{ profile = "TotalChaos" } "err" | Out-Null
    Test-Rpc $ws "B.6" "network.getAuditLog" "network.getAuditLog" | Out-Null
    Test-Rpc $ws "B.6" "network.setBudgets" "network.setBudgets" @{ budgets = @{} } "any" | Out-Null
    Test-Rpc $ws "B.6" "network.disconnectAll" "network.disconnectAll" | Out-Null
    Test-Rpc $ws "B.6" "network.enable" "network.enable" | Out-Null
    Test-Rpc $ws "B.6" "listener.status" "listener.status" | Out-Null
    Test-Rpc $ws "B.6" "listener.stop" "listener.stop" @{} "any" | Out-Null
    Test-Rpc $ws "B.6" "listener.start" "listener.start" @{} "any" | Out-Null
    Test-Rpc $ws "B.6" "upnp.status" "upnp.status" | Out-Null
    Test-Rpc $ws "B.6" "upnp.setEnabled(false)" "upnp.setEnabled" @{ enabled = $false } "any" | Out-Null

    # ===================================================================
    # B.7 Kad
    # ===================================================================
    Write-Host "`n[B.7] Kad" -ForegroundColor Yellow
    Test-Rpc $ws "B.7" "kad.status" "kad.status" | Out-Null
    Test-Rpc $ws "B.7" "kad.listContacts" "kad.listContacts" | Out-Null
    Test-Rpc $ws "B.7" "kad.clearContacts" "kad.clearContacts" | Out-Null
    Test-Rpc $ws "B.7" "kad.startUdp" "kad.startUdp" @{} "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.bootstrap (dead node)" "kad.bootstrap" @{ ip = "127.0.0.1"; port = 1 } "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.findNode" "kad.findNode" @{ targetId = "00112233445566778899AABBCCDDEEFF" } "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.ping" "kad.ping" @{ ip = "127.0.0.1"; port = 4672 } "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.searchKeywords" "kad.searchKeywords" @{ term = "bench" } "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.searchSources" "kad.searchSources" @{ fileHash = "0123456789ABCDEF0123456789ABCDEF" } "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.searchNotes" "kad.searchNotes" @{ fileHash = "0123456789ABCDEF0123456789ABCDEF" } "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.publishSource" "kad.publishSource" @{ fileHash = "0123456789ABCDEF0123456789ABCDEF"; fileName = "bench.bin"; fileSize = 1024 } "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.publishKeywords" "kad.publishKeywords" @{ fileHash = "0123456789ABCDEF0123456789ABCDEF"; keywords = @("bench") } "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.publishNote" "kad.publishNote" @{ fileHash = "0123456789ABCDEF0123456789ABCDEF"; fileName = "bench.bin"; fileSize = 1024; comment = "bench"; rating = 3 } "any" | Out-Null
    Test-Rpc $ws "B.7" "kad.stopUdp" "kad.stopUdp" @{} "any" | Out-Null

    # ===================================================================
    # B.8 Systeme / settings / categories / amis / historique
    # ===================================================================
    Write-Host "`n[B.8] Systeme / settings / supervision" -ForegroundColor Yellow
    Test-Rpc $ws "B.8" "system.status" "system.status" -Validate {
        param($r) if ($null -eq $r.healthy -and $null -eq $r.status) { "no health field" }
    } | Out-Null
    Test-Rpc $ws "B.8" "settings.get" "settings.get" | Out-Null
    Test-Rpc $ws "B.8" "settings.set (nickname)" "settings.set" @{ nickname = "BenchBot" } | Out-Null
    Test-Rpc $ws "B.8" "settings.get (verify nickname)" "settings.get" -Validate {
        param($r) if ($r.nickname -ne "BenchBot") { "nickname not persisted" }
    } | Out-Null
    Test-Rpc $ws "B.8" "settings.set (unknown key tolerated?)" "settings.set" @{ bogusKey123 = 1 } "any" | Out-Null
    Test-Rpc $ws "B.8" "settings.setRetentionDays" "settings.setRetentionDays" @{ retentionDays = 30 } | Out-Null
    Test-Rpc $ws "B.8" "settings.resetDownloadDir" "settings.resetDownloadDir" @{} "any" | Out-Null
    Test-Rpc $ws "B.8" "limits.get" "limits.get" | Out-Null
    Test-Rpc $ws "B.8" "limits.set (1MB/s)" "limits.set" @{ downloadBytesPerSec = 1048576 } | Out-Null
    Test-Rpc $ws "B.8" "limits.get (verify)" "limits.get" -Validate {
        param($r) if ($r.downloadBytesPerSec -gt 1572864) { "limit not applied" }
    } | Out-Null
    Test-Rpc $ws "B.8" "limits.set (unlimited)" "limits.set" @{ downloadBytesPerSec = 0 } | Out-Null

    $cat = Test-Rpc $ws "B.8" "categories.create" "categories.create" @{ name = "BenchCat" }
    $catId = if ($cat) { $cat.id } else { $null }
    if (!$catId -and $cat) { $catId = $cat.category.id }
    # categories.list retourne un tableau nu (pas d'enveloppe {categories})
    Test-Rpc $ws "B.8" "categories.list" "categories.list" -Validate {
        param($r) if (@($r).Count -lt 1) { "category not listed" }
    } | Out-Null
    if ($catId) {
        Test-Rpc $ws "B.8" "categories.update" "categories.update" @{ id = $catId; name = "BenchCat2" } "any" | Out-Null
        Test-Rpc $ws "B.8" "categories.reorder" "categories.reorder" @{ categoryIds = @($catId) } "any" | Out-Null
        Test-Rpc $ws "B.8" "categories.delete" "categories.delete" @{ id = $catId } "any" | Out-Null
    }
    Test-Rpc $ws "B.8" "friends.add" "friends.add" @{ userHash = "A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4"; name = "BenchFriend" } "any" | Out-Null
    Test-Rpc $ws "B.8" "friends.list" "friends.list" | Out-Null
    Test-Rpc $ws "B.8" "friends.remove" "friends.remove" @{ userHash = "A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4" } "any" | Out-Null
    Test-Rpc $ws "B.8" "history.list" "history.list" | Out-Null
    Test-Rpc $ws "B.8" "history.stats" "history.stats" | Out-Null
    Test-Rpc $ws "B.8" "history.export (json)" "history.export" @{ format = "json" } "any" | Out-Null
    Test-Rpc $ws "B.8" "history.clear" "history.clear" | Out-Null
    Test-Rpc $ws "B.8" "stats.resetLifetimeTotals" "stats.resetLifetimeTotals" @{} "any" | Out-Null
    Test-Rpc $ws "B.8" "servers.status" "servers.status" | Out-Null
    Test-Rpc $ws "B.8" "servers.setEnabled(false)" "servers.setEnabled" @{ address = "127.0.0.1:4661"; enabled = $false } "any" | Out-Null
    Test-Rpc $ws "B.8" "servers.search (no conn)" "servers.search" @{ term = "bench" } "any" | Out-Null
    Test-Rpc $ws "B.8" "servers.querySources" "servers.querySources" @{ fileHash = "0123456789ABCDEF0123456789ABCDEF" } "any" | Out-Null
    Test-Rpc $ws "B.8" "servers.connect (dead)" "servers.connect" @{ address = "127.0.0.1:1" } "any" | Out-Null
    Test-Rpc $ws "B.8" "servers.disconnect" "servers.disconnect" @{} "any" | Out-Null
    Test-Rpc $ws "B.8" "servers.delete" "servers.delete" @{ address = "195.154.122.10:4661" } "any" | Out-Null
    Test-Rpc $ws "B.8" "servers.clear" "servers.clear" @{} "any" | Out-Null
    Test-Rpc $ws "B.8" "lab.status" "lab.status" | Out-Null

    # ===================================================================
    # Arret propre
    # ===================================================================
    Write-Host "`n[Teardown] system.stop" -ForegroundColor Yellow
    Test-Rpc $ws "Teardown" "system.stop" "system.stop" @{} | Out-Null
    try { $ws.Dispose() } catch {}
    $daemonProcess.WaitForExit(8000) | Out-Null
    if (!$daemonProcess.HasExited) {
        Stop-Process -Id $daemonProcess.Id -Force
        Add-Result "Teardown" "daemon exit" "FAIL" "did not exit within 8s"
    } else {
        Add-Result "Teardown" "daemon exit code $($daemonProcess.ExitCode)" ($(if ($daemonProcess.ExitCode -eq 0) {"PASS"} else {"FAIL"}))
    }
}
catch {
    Write-Host "`nFATAL: $($_.Exception.Message)" -ForegroundColor Red
    Add-Result "FATAL" "bench aborted" "FAIL" $_.Exception.Message
    $exitCode = 1
}
finally {
    try { if ($ws) { $ws.Dispose() } } catch {}
    if ($daemonProcess -and !$daemonProcess.HasExited) {
        Stop-Process -Id $daemonProcess.Id -Force -ErrorAction SilentlyContinue
    }
}

# ===================================================================
# Rapport
# ===================================================================
$pass = ($script:Results | Where-Object { $_.Outcome -eq "PASS" }).Count
$fail = ($script:Results | Where-Object { $_.Outcome -eq "FAIL" }).Count

Write-Host "`n==================================================" -ForegroundColor Cyan
Write-Host "  RAPPORT PHASE B                                 " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
$script:Results | Group-Object Section | ForEach-Object {
    $p = ($_.Group | Where-Object { $_.Outcome -eq "PASS" }).Count
    $f = ($_.Group | Where-Object { $_.Outcome -eq "FAIL" }).Count
    Write-Host ("  {0,-10} PASS={1,-3} FAIL={2}" -f $_.Name, $p, $f)
}
Write-Host "  ------------------------------------------"
Write-Host ("  TOTAL: {0} PASS / {1} FAIL" -f $pass, $fail) -ForegroundColor ($(if ($fail -eq 0) {"Green"} else {"Red"}))
$script:Results | Where-Object { $_.Outcome -eq "FAIL" } | ForEach-Object {
    Write-Host ("  FAIL: [{0}] {1} - {2}" -f $_.Section, $_.Name, $_.Detail) -ForegroundColor Red
}

if ($fail -gt 0) { $exitCode = 1 }
Write-Host ("`n  PHASE B FUNCTIONAL BENCH: {0}" -f $(if ($exitCode -eq 0) {"PASSED"} else {"FAILED"})) -ForegroundColor ($(if ($exitCode -eq 0) {"Green"} else {"Red"}))
exit $exitCode
