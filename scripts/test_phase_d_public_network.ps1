# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Laurent Geynet "Loulach"

<#
.SYNOPSIS
    Phase D - Validation reseau reel supervisee (docs/qa/PLAN_TEST_COMPLET.md).
.DESCRIPTION
    Extrait le bundle STANDARD (pas de lab.*), demarre le daemon en
    profil public par defaut (UPnP actif) et exerce le vrai reseau :
      D.1  bootstrap server.met via URL publique reelle (anti-SSRF
           autorise le public en profil PublicEd2kKadPreview)
      D.2  connexion TCP a un serveur eD2K reel + HighID/LowID
      D.3  recherche globale serveur
      D.4  bootstrap Kad via nodes.dat reel + contacts + recherche
      D.5  kill switch : coupure nette puis reactivation
      D.6  arret propre + unmap UPnP borne
    ATTENTION : ce script EMET DU TRAFIC REEL (serveurs eD2K publics,
    Kad DHT, gateway UPnP). A n'executer que sur une session supervisee.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test_phase_d_public_network.ps1
#>

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

# URLs canoniques eMule (upd.emule-security.org, maintenues par la
# communaute eMule Security) - verifiees vivantes le 2026-10.
$ServerMetUrl = "https://upd.emule-security.org/server.met"
$NodesDatUrl  = "https://upd.emule-security.org/nodes.dat"

$script:Results = New-Object System.Collections.Generic.List[object]
$script:ReqId = 2000

function Add-Result([string]$Section, [string]$Name, [string]$Outcome, [string]$Detail = "") {
    $script:Results.Add([pscustomobject]@{
        Section = $Section; Name = $Name; Outcome = $Outcome; Detail = $Detail
    })
    $color = switch ($Outcome) { "PASS" { "Green" } "FAIL" { "Red" } "SKIP" { "DarkYellow" } default { "Yellow" } }
    $detailStr = if ($Detail) { "  ($Detail)" } else { "" }
    Write-Host ("  [{0}] {1}{2}" -f $Outcome, $Name, $detailStr) -ForegroundColor $color
}

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

function Invoke-WsRpc([System.Net.WebSockets.ClientWebSocket]$ws, [hashtable]$request, [int]$timeoutMs = 15000) {
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

function Test-Rpc {
    param(
        [System.Net.WebSockets.ClientWebSocket]$Ws,
        [string]$Section, [string]$Name, [string]$Method,
        [hashtable]$Params = @{},
        [string]$Expect = "ok",
        [scriptblock]$Validate = $null,
        [int]$TimeoutMs = 15000
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
                Add-Result $Section $Name "FAIL" "expected error, got result"
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

function Connect-DaemonWebSocket([string]$endpoint, [string]$token) {
    $ws = [System.Net.WebSockets.ClientWebSocket]::new()
    $ws.Options.SetRequestHeader("Authorization", "Bearer $token")
    $ws.ConnectAsync([System.Uri]::new($endpoint), [System.Threading.CancellationTokenSource]::new(5000).Token).Wait()
    return $ws
}

# =======================================================================
# 0. Sandbox + boot (bundle STANDARD, profil public par defaut)
# =======================================================================
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Phase D - Reseau reel supervise (eD2K + Kad)   " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  ATTENTION: trafic reseau reel vers serveurs eD2K/Kad publics" -ForegroundColor Yellow

$zip = Get-ChildItem (Join-Path $Root "dist") -Filter "eMuleRust-*-standard-windows-x64.zip" |
       Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (!$zip) { throw "Standard bundle ZIP not found in dist/ - run build_windows_release.ps1 first" }

$WorkRoot = Join-Path $env:TEMP ("eMuleRust-PhaseD-" + (New-Guid).ToString("N").Substring(0,8))
$BundleDir = Join-Path $WorkRoot "bundle"
$WorkDir = Join-Path $WorkRoot "workdir"
New-Item -ItemType Directory -Force -Path $BundleDir, $WorkDir | Out-Null
Expand-Archive -Path $zip.FullName -DestinationPath $BundleDir -Force
$DaemonBin = Join-Path $BundleDir "bin\mule-daemon.exe"
if (!(Test-Path $DaemonBin)) { throw "bin\mule-daemon.exe not found in extracted bundle" }
Write-Host " -> Bundle: $($zip.Name)" -ForegroundColor Gray
Write-Host " -> Sandbox: $WorkRoot" -ForegroundColor Gray

$daemonProcess = $null
$ws = $null
$exitCode = 0

try {
    # Boot : pas de --profile (public par defaut), UPnP actif.
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $DaemonBin
    $psi.Arguments = "--workdir `"$WorkDir`" --mode portable"
    $psi.WorkingDirectory = (Split-Path -Parent $DaemonBin)
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $daemonProcess = [System.Diagnostics.Process]::Start($psi)
    # Drain continu - evite le blocage pipe ~64 Ko.
    $script:DaemonStdoutTask = $daemonProcess.StandardOutput.ReadToEndAsync()
    $script:DaemonStderrTask = $daemonProcess.StandardError.ReadToEndAsync()

    $sessionPath = Join-Path $WorkDir "session.json"
    $t = 0
    while (!(Test-Path $sessionPath) -and $t -lt 15) {
        Start-Sleep -Milliseconds 200; $t += 0.2
        if ($daemonProcess.HasExited) { throw "Daemon died during boot ($($daemonProcess.ExitCode))" }
    }
    if (!(Test-Path $sessionPath)) { throw "Timeout waiting for session.json" }

    $session = Get-Content $sessionPath -Raw | ConvertFrom-Json
    $endpoint = $session.endpoint
    $token = (Get-Content (Join-Path $WorkDir "auth.token") -Raw).Trim()
    Write-Host " -> Daemon up (PID $($daemonProcess.Id)) on $endpoint" -ForegroundColor Gray

    $ws = Connect-DaemonWebSocket $endpoint $token
    # L'auth se valide via le handshake hello (protocolVersion + token).
    $hello = Test-Rpc $ws "D.0" "system.hello" "system.hello" @{ protocolVersion = 1; token = $token }
    if (!$hello) { throw "Handshake refused - aborting public network test" }

    $status = Test-Rpc $ws "D.0" "system.status (profil public)" "system.status" -Validate {
        param($r) if ($r.networkProfile -notmatch 'public') { "unexpected profile: $($r.networkProfile)" }
    }

    # ===================================================================
    # D.1 Bootstrap server.met via URL publique reelle
    # ===================================================================
    Write-Host "`n[D.1] Import server.met (URL publique reelle)" -ForegroundColor Yellow
    $upd = Test-Rpc $ws "D.1" "servers.updateFromUrl (reel)" "servers.updateFromUrl" @{ url = $ServerMetUrl } -TimeoutMs 30000
    $srvList = Test-Rpc $ws "D.1" "servers.list" "servers.list" -Validate {
        param($r) if ($r.servers.Count -lt 1) { "no servers imported" }
    }
    if ($upd) { Write-Host "     -> imported=$($upd.imported) filtered=$($upd.filtered)" -ForegroundColor Gray }

    # ===================================================================
    # D.2 Connexion TCP serveur eD2K reel + ID
    # ===================================================================
    Write-Host "`n[D.2] Connexion serveur eD2K reel" -ForegroundColor Yellow
    $serverAddr = $null
    if ($srvList -and $srvList.servers.Count -gt 0) {
        $serverAddr = $srvList.servers[0].address
        Write-Host "     -> trying server: $($srvList.servers[0].name) ($serverAddr)" -ForegroundColor Gray
    }
    if ($serverAddr) {
        Test-Rpc $ws "D.2" "servers.connect (reel)" "servers.connect" @{ address = $serverAddr } -TimeoutMs 30000 -Expect "any" | Out-Null
        # Poll jusqu'a Connected (TCP handshake + login eD2K ~10-30s)
        $connected = $false
        for ($i = 0; $i -lt 12 -and !$connected; $i++) {
            Start-Sleep -Seconds 5
            $snap = $null
            try { $snap = (Invoke-WsRpc $ws @{ jsonrpc="2.0"; id=(++$script:ReqId); method="servers.status"; params=@{} } 10000).result } catch {}
            if ($snap -and $snap.status -eq "Connected") { $connected = $true }
            elseif ($snap) { Write-Host "     -> status: $($snap.status)" -ForegroundColor Gray }
        }
        if ($connected) {
            $cid = $snap.clientId
            $idKind = if ($cid -ge 16777216) { "HighID" } else { "LowID" }
            Add-Result "D.2" "server session Connected" "PASS" "$idKind (clientId=$cid, server=$($snap.server_address))"
            Test-Rpc $ws "D.2" "servers.querySources (reel)" "servers.querySources" @{ fileHash = "0123456789ABCDEF0123456789ABCDEF" } -Expect "any" -TimeoutMs 20000 | Out-Null
        } else {
            Add-Result "D.2" "server session Connected" "FAIL" "not connected after 60s (last: $($snap.status))"
        }
    } else {
        Add-Result "D.2" "server session Connected" "SKIP" "no server in list"
    }

    # ===================================================================
    # D.3 Recherche globale serveur
    # ===================================================================
    Write-Host "`n[D.3] Recherche globale" -ForegroundColor Yellow
    if ($connected) {
        $sr = Test-Rpc $ws "D.3" "servers.search (reel)" "servers.search" @{ term = "ubuntu" } -Expect "any" -TimeoutMs 30000
        if ($sr -and $sr.result) { Write-Host "     -> results received" -ForegroundColor Gray }
    } else {
        Add-Result "D.3" "servers.search (reel)" "SKIP" "not connected"
    }

    # ===================================================================
    # D.4 Kad : bootstrap nodes.dat reel + contacts + recherche
    # ===================================================================
    Write-Host "`n[D.4] Kad bootstrap + recherche" -ForegroundColor Yellow
    Test-Rpc $ws "D.4" "kad.bootstrapFromUrl (reel)" "kad.bootstrapFromUrl" @{ url = $NodesDatUrl } -TimeoutMs 30000 -Expect "any" | Out-Null
    Test-Rpc $ws "D.4" "kad.startUdp" "kad.startUdp" @{} -Expect "any" | Out-Null
    # Bootstrap actif vers le 1er contact importe (ip+port requis)
    $contact = Test-Rpc $ws "D.4" "kad.listContacts" "kad.listContacts" -Expect "any"
    if ($contact -and $contact.contacts -and $contact.contacts.Count -gt 0) {
        $c0 = $contact.contacts[0]
        Write-Host "     -> bootstrap vers $($c0.ip):$($c0.udpPort)" -ForegroundColor Gray
        Test-Rpc $ws "D.4" "kad.bootstrap (contact reel)" "kad.bootstrap" @{ ip = $c0.ip; port = $c0.udpPort } -Expect "any" | Out-Null
    }
    $kadOk = $false
    for ($i = 0; $i -lt 12 -and !$kadOk; $i++) {
        Start-Sleep -Seconds 5
        $ks = $null
        try { $ks = (Invoke-WsRpc $ws @{ jsonrpc="2.0"; id=(++$script:ReqId); method="kad.status"; params=@{} } 10000).result } catch {}
        if ($ks) {
            Write-Host "     -> kad.status: udp=$($ks.isUdpActive) contacts=$($ks.totalContacts) fw=$($ks.udpFirewallStatus) tx=$($ks.activeTransactions)" -ForegroundColor Gray
            if ($ks.totalContacts -gt 0) { $kadOk = $true }
        }
    }
    if ($kadOk) {
        Add-Result "D.4" "kad contacts > 0" "PASS"
        Test-Rpc $ws "D.4" "kad.searchKeywords (reel)" "kad.searchKeywords" @{ term = "ubuntu" } -Expect "any" -TimeoutMs 30000 | Out-Null
        Test-Rpc $ws "D.4" "kad.searchSources (reel)" "kad.searchSources" @{ fileHash = "0123456789ABCDEF0123456789ABCDEF"; fileSize = 1024 } -Expect "any" -TimeoutMs 30000 | Out-Null
    } else {
        Add-Result "D.4" "kad contacts > 0" "FAIL" "no contacts after 40s"
    }

    # ===================================================================
    # D.5 Kill switch : coupure nette puis reactivation
    # ===================================================================
    Write-Host "`n[D.5] Kill switch" -ForegroundColor Yellow
    # Le kill switch produit est network.disconnectAll / network.enable.
    Test-Rpc $ws "D.5" "network.disconnectAll" "network.disconnectAll" @{} | Out-Null
    Start-Sleep -Seconds 3
    Test-Rpc $ws "D.5" "servers.status (post-disconnectAll)" "servers.status" -Validate {
        param($r) if ($r.status -eq "Connected") { "still connected after disconnectAll" }
    } | Out-Null
    Test-Rpc $ws "D.5" "network.enable" "network.enable" @{} | Out-Null

    # ===================================================================
    # D.6 UPnP status + arret propre (unmap borne 500 ms)
    # ===================================================================
    Write-Host "`n[D.6] UPnP + arret" -ForegroundColor Yellow
    $upnp = Test-Rpc $ws "D.6" "upnp.status" "upnp.status" -Expect "any"
    if ($upnp) { Write-Host "     -> upnp: $($upnp | ConvertTo-Json -Compress -Depth 3)" -ForegroundColor Gray }
    Test-Rpc $ws "D.6" "system.stop" "system.stop" @{} | Out-Null
    try { $ws.Dispose() } catch {}
    $sw2 = [System.Diagnostics.Stopwatch]::StartNew()
    $daemonProcess.WaitForExit(15000) | Out-Null
    if (!$daemonProcess.HasExited) {
        Stop-Process -Id $daemonProcess.Id -Force
        Add-Result "D.6" "daemon exit" "FAIL" "did not exit within 15s"
    } else {
        Add-Result "D.6" "daemon exit code $($daemonProcess.ExitCode) in $($sw2.ElapsedMilliseconds)ms" ($(if ($daemonProcess.ExitCode -eq 0) {"PASS"} else {"FAIL"}))
    }
    if (Test-Path $sessionPath) { Add-Result "D.6" "session.json cleanup" "FAIL" "session.json still present" }
    else { Add-Result "D.6" "session.json cleanup" "PASS" }
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
    # Dump stderr en cas d'echec pour diagnostic
    if ($script:DaemonStderrTask -and $script:DaemonStderrTask.IsCompleted) {
        $err = $script:DaemonStderrTask.Result
        if ($err) {
            $logPath = Join-Path $WorkRoot "daemon-stderr.log"
            [System.IO.File]::WriteAllText($logPath, $err)
            Write-Host " -> daemon stderr: $logPath" -ForegroundColor Gray
        }
    }
}

# ===================================================================
# Rapport
# ===================================================================
$pass = ($script:Results | Where-Object { $_.Outcome -eq "PASS" }).Count
$fail = ($script:Results | Where-Object { $_.Outcome -eq "FAIL" }).Count
$skip = ($script:Results | Where-Object { $_.Outcome -eq "SKIP" }).Count

Write-Host "`n==================================================" -ForegroundColor Cyan
Write-Host "  RAPPORT PHASE D                                 " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
$script:Results | Group-Object Section | ForEach-Object {
    $p = ($_.Group | Where-Object { $_.Outcome -eq "PASS" }).Count
    $f = ($_.Group | Where-Object { $_.Outcome -eq "FAIL" }).Count
    $s = ($_.Group | Where-Object { $_.Outcome -eq "SKIP" }).Count
    Write-Host ("  {0,-10} PASS={1,-3} FAIL={2,-3} SKIP={3}" -f $_.Name, $p, $f, $s)
}
Write-Host "  ------------------------------------------"
Write-Host ("  TOTAL: {0} PASS / {1} FAIL / {2} SKIP" -f $pass, $fail, $skip) -ForegroundColor ($(if ($fail -eq 0) {"Green"} else {"Red"}))
$script:Results | Where-Object { $_.Outcome -eq "FAIL" } | ForEach-Object {
    Write-Host ("  FAIL: [{0}] {1} - {2}" -f $_.Section, $_.Name, $_.Detail) -ForegroundColor Red
}

if ($fail -gt 0) { $exitCode = 1 }
Write-Host ("`n  PHASE D PUBLIC NETWORK: {0}" -f $(if ($exitCode -eq 0) {"PASSED"} else {"FAILED"})) -ForegroundColor ($(if ($exitCode -eq 0) {"Green"} else {"Red"}))
exit $exitCode
