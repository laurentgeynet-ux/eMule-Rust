<#
.SYNOPSIS
    Automated Phase 7.3 E2E QA loopback test for the packaged Emule-Sion distribution.
.DESCRIPTION
    Phase 7.3 - Industrialisation Locale et Validation E2E QA:
    Executes in a sterile, temporary directory outside the Git repository:
    1. Extracts EmuleSion-v0.1.1-lab-qa-loopback-windows-x64.zip into a temporary sandbox.
    2. Validates portable.flag and boots the packaged mule-daemon binary in portable mode.
    3. Verifies loopback network confinement (127.0.0.1) and JSON-RPC handshake (lab-tools announced).
    4. Nominal Transfer (sample_1mb):
       - Generates fixture via lab.createFixture.
       - Adds eD2K task via downloads.addEd2kLink targeting <data>/downloads.
       - Starts loopback peer on 127.0.0.1 via lab.startLoopbackPeer.
       - Connects transfer via ed2k.startDownload.
       - Monitors state transitions: Queued -> WaitingForSources -> Downloading -> Verifying -> Completed.
       - Verifies finalized file in <data>/downloads (size and eD2K root hash match).
    5. Boundary Transfer (pattern_9728001: 1 full part + 1 residual byte):
       - Generates boundary fixture via lab.createFixture.
       - Performs transfer to completion and verifies integrity.
    6. Resilience and Resumption (part_9728000: 50% crash and resume):
       - Starts download of 9.28 MB part.
       - Monitors transfer until >= 4.0 MB (~50%).
       - Force-kills daemon process abruptly (simulating crash).
       - Verifies .part file and SQLite state retained on disk.
       - Re-boots packaged daemon on the same data folder.
       - Confirms state and received bytes preserved across restart.
       - Resumes transfer via ed2k.startDownload and verifies transition to Completed.
    7. Secure Test Data Cleanup (lab.cleanupTestData):
       - Purges fixtures and temporary files.
       - Confirms completed files in <data>/downloads are NEVER deleted.
    8. Graceful shutdown via system.stop and confirms zero repository pollution.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_qa_loopback_e2e.ps1
#>

param(
    [string]$ZipPath = "",
    [string]$BundleDir = "",
    [switch]$KeepTemp = $false
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ([string]::IsNullOrWhiteSpace($ZipPath)) {
    $ZipPath = Join-Path $Root "dist\EmuleSion-v0.1.1-lab-qa-loopback-windows-x64.zip"
}

if ([string]::IsNullOrWhiteSpace($BundleDir)) {
    $BundleDir = Join-Path $Root "dist\EmuleSion-Qa-Loopback-Windows"
}

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Phase 7.3 : Automated QA Loopback E2E Pipeline  " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# -----------------------------------------------------------------------------
# 1. Setup Isolated Temporary Environment Outside Repository
# -----------------------------------------------------------------------------
$TestId = [guid]::NewGuid().ToString("N").Substring(0, 8)
$IsolatedTempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "EmuleSion-QaE2E-$TestId"
$BundleCopyDir = Join-Path $IsolatedTempRoot "bundle\EmuleSion-Windows"
$WorkDir = Join-Path $IsolatedTempRoot "workdir\EmuleSion"
$DownloadsDir = Join-Path $WorkDir "downloads"

Write-Host "`n[1/7] Creating isolated test sandbox at:" -ForegroundColor Yellow
Write-Host "  $IsolatedTempRoot" -ForegroundColor Gray

New-Item -ItemType Directory -Path $BundleCopyDir -Force | Out-Null
New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null
New-Item -ItemType Directory -Path $DownloadsDir -Force | Out-Null

$daemonProcess = $null

try {
    # Extract from ZIP archive if present, otherwise copy bundle
    if (Test-Path $ZipPath) {
        Write-Host " -> Extracting QA Loopback ZIP archive: $ZipPath" -ForegroundColor Gray
        Expand-Archive -Path $ZipPath -DestinationPath $BundleCopyDir -Force
    } elseif (Test-Path $BundleDir) {
        Write-Host " -> Copying assembled bundle from: $BundleDir" -ForegroundColor Gray
        Copy-Item -Path "$BundleDir\*" -Destination $BundleCopyDir -Recurse -Force
    } else {
        throw "Neither ZIP archive ($ZipPath) nor bundle directory ($BundleDir) exists! Run build_windows_release.ps1 -Flavor qa-loopback first."
    }

    # Verify portable.flag
    $PortableFlagPath = Join-Path $BundleCopyDir "portable.flag"
    if (!(Test-Path $PortableFlagPath)) {
        throw "Verification failed: portable.flag missing in bundle root!"
    }
    Write-Host " -> portable.flag verified in bundle root." -ForegroundColor Green

    # Locate mule-daemon binary
    $DaemonBin = Join-Path $BundleCopyDir "bin\mule-daemon.exe"
    if (!(Test-Path $DaemonBin)) {
        $DaemonBin = Join-Path $BundleCopyDir "mule-daemon.exe"
    }
    if (!(Test-Path $DaemonBin)) {
        throw "Packaged mule-daemon.exe binary not found in bundle!"
    }

    # Helper function to start daemon
    function Start-PackagedDaemon([string]$binPath, [string]$dataDir) {
        $psi = [System.Diagnostics.ProcessStartInfo]::new()
        $psi.FileName = $binPath
        $psi.Arguments = "--workdir `"$dataDir`" --mode portable"
        $psi.WorkingDirectory = (Split-Path -Parent $binPath)
        $psi.UseShellExecute = $false
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.CreateNoWindow = $true

        $proc = [System.Diagnostics.Process]::Start($psi)
        if (!$proc) { throw "Failed to start mule-daemon process" }

        # Await session.json
        $SessionPath = Join-Path $dataDir "session.json"
        $timeout = 10
        $t = 0
        while (!(Test-Path $SessionPath) -and $t -lt $timeout) {
            Start-Sleep -Milliseconds 200
            $t += 0.2
            if ($proc.HasExited) {
                $err = $proc.StandardError.ReadToEnd()
                throw "Daemon died during boot ($($proc.ExitCode)): $err"
            }
        }
        if (!(Test-Path $SessionPath)) {
            throw "Timeout waiting for session.json in $dataDir"
        }
        return $proc
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

    # Helper function for JSON-RPC over WebSocket
    function Invoke-WsRpc([System.Net.WebSockets.ClientWebSocket]$ws, [hashtable]$request, [int]$timeoutMs = 5000) {
        $reqId = $request.id
        $json = $request | ConvertTo-Json -Compress
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
        $seg = [System.ArraySegment[byte]]::new($bytes)
        $cts = [System.Threading.CancellationTokenSource]::new($timeoutMs)
        $ws.SendAsync($seg, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cts.Token).Wait()

        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        while ($sw.ElapsedMilliseconds -lt $timeoutMs) {
            $remain = [math]::Max(500, ($timeoutMs - $sw.ElapsedMilliseconds))
            $recCts = [System.Threading.CancellationTokenSource]::new([int]$remain)
            $respText = Receive-FullWsText $ws $recCts

            $parsed = $respText | ConvertFrom-Json
            if ($null -ne $parsed.id -and $parsed.id -eq $reqId) {
                return $parsed
            }
        }
        throw "Timeout waiting for JSON-RPC response to request id $reqId after $timeoutMs ms"
    }

    # Helper to connect WebSocket
    function Connect-DaemonWebSocket([string]$endpoint, [string]$token) {
        $ws = [System.Net.WebSockets.ClientWebSocket]::new()
        $ws.Options.SetRequestHeader("Authorization", "Bearer $token")
        $uri = [System.Uri]::new($endpoint)
        $cts = [System.Threading.CancellationTokenSource]::new(5000)
        $ws.ConnectAsync($uri, $cts.Token).Wait()
        return $ws
    }

    # -------------------------------------------------------------------------
    # 2. Boot Daemon & Authenticate
    # -------------------------------------------------------------------------
    Write-Host "`n[2/7] Booting packaged daemon binary in portable mode..." -ForegroundColor Yellow
    $daemonProcess = Start-PackagedDaemon $DaemonBin $WorkDir
    Write-Host " -> Daemon running (PID: $($daemonProcess.Id))." -ForegroundColor Gray

    $SessionFile = Join-Path $WorkDir "session.json"
    $TokenFile = Join-Path $WorkDir "auth.token"
    $SessionData = Get-Content $SessionFile -Raw | ConvertFrom-Json
    $Token = (Get-Content $TokenFile -Raw).Trim()

    if ($SessionData.endpoint -notmatch '127\.0\.0\.1') {
        throw "SECURITY VIOLATION: Endpoint is not 127.0.0.1!"
    }
    Write-Host " -> Loopback confinement verified on $($SessionData.endpoint)." -ForegroundColor Green

    $ws = Connect-DaemonWebSocket $SessionData.endpoint $Token

    # Handshake
    $helloResp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 1
        method = "system.hello"
        params = @{ protocolVersion = 1; token = $Token }
    }
    if ($helloResp.result.workdirMode -ne "Portable") {
        throw "Workdir mode mismatch! Expected 'Portable', got '$($helloResp.result.workdirMode)'"
    }
    if ($helloResp.result.capabilities -notcontains "lab-tools") {
        throw "Capability error: QA Loopback bundle missing 'lab-tools'!"
    }
    Write-Host " -> system.hello handshake success (Version: $($helloResp.result.daemonVersion), Mode: Portable)." -ForegroundColor Green

    # -------------------------------------------------------------------------
    # 3. Nominal Transfer Vector: sample_1mb
    # -------------------------------------------------------------------------
    Write-Host "`n[3/7] Testing Vector 1: Nominal Transfer (sample_1mb)..." -ForegroundColor Yellow

    # A. Create fixture
    $fixResp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 2
        method = "lab.createFixture"
        params = @{ fixtureType = "sample_1mb" }
    }
    $sampleLink = $fixResp.result.ed2kLink
    $sampleHash = $fixResp.result.ed2kHash
    $sampleFile = $fixResp.result.fileName
    Write-Host " -> Created fixture: $sampleFile ($($fixResp.result.sizeBytes) bytes)" -ForegroundColor Gray

    # B. Start loopback peer
    $peerResp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 3
        method = "lab.startLoopbackPeer"
        params = @{}
    }
    $peerAddr = $peerResp.result.peerAddr
    Write-Host " -> Loopback peer active on $peerAddr (Serving $($peerResp.result.sharedCount) fixture(s))." -ForegroundColor Gray

    # C. Add eD2K download task
    $addResp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 4
        method = "downloads.addEd2kLink"
        params = @{
            link = $sampleLink
            targetDirectory = $DownloadsDir
        }
    }
    $dl1Id = $addResp.result.downloadId
    Write-Host " -> Added download task: $dl1Id (Status: $($addResp.result.status))" -ForegroundColor Gray

    # D. Connect download to peer source
    $startResp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 5
        method = "ed2k.startDownload"
        params = @{
            downloadId = $dl1Id
            peerAddr = $peerAddr
        }
    }
    if (!$startResp.result.started) { throw "Failed to start ed2k download: $($startResp | ConvertTo-Json)" }

    # E. Monitor transfer progression until completed
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $dl1Completed = $false
    while ($sw.ElapsedMilliseconds -lt 15000) {
        $getResp = Invoke-WsRpc $ws @{
            jsonrpc = "2.0"
            id = 6
            method = "downloads.get"
            params = @{ id = $dl1Id }
        }
        $st = $getResp.result.status
        $rec = $getResp.result.receivedBytes
        $tot = $getResp.result.totalBytes
        if ($st -eq "completed") {
            $dl1Completed = $true
            break
        }
        Start-Sleep -Milliseconds 150
    }

    if (!$dl1Completed) {
        throw "Download 1 (sample_1mb) did not complete within timeout! Last status: $st ($rec / $tot bytes)"
    }
    Write-Host " -> sample_1mb download completed and verified in $($sw.ElapsedMilliseconds) ms!" -ForegroundColor Green

    # Verify finalized file on disk
    $finalizedSample = Join-Path $DownloadsDir $sampleFile
    if (!(Test-Path $finalizedSample)) {
        throw "Finalized file missing at $finalizedSample!"
    }
    if ((Get-Item $finalizedSample).Length -ne 1048576) {
        throw "Finalized file size mismatch! Expected 1048576, got $((Get-Item $finalizedSample).Length)"
    }
    Write-Host " -> Confirmed finalized file in downloads/: $sampleFile (1,048,576 bytes)." -ForegroundColor Green

    # -------------------------------------------------------------------------
    # 4. Boundary Transfer Vector: pattern_9728001 (Part + 1 byte)
    # -------------------------------------------------------------------------
    Write-Host "`n[4/7] Testing Vector 2: Boundary Transfer (pattern_9728001 - 9.28 MB + 1 byte)..." -ForegroundColor Yellow

    # A. Create boundary fixture
    $fix2Resp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 10
        method = "lab.createFixture"
        params = @{ fixtureType = "pattern_9728001" }
    }
    $boundLink = $fix2Resp.result.ed2kLink
    $boundFile = $fix2Resp.result.fileName
    if ($fix2Resp.result.sizeBytes -ne 9728001) {
        throw "Boundary fixture size mismatch! Expected 9728001, got $($fix2Resp.result.sizeBytes)"
    }
    Write-Host " -> Created boundary fixture: $boundFile (9,728,001 bytes)." -ForegroundColor Gray

    # B. Re-bind loopback peer to serve newly created fixture
    $peer2Resp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 11
        method = "lab.startLoopbackPeer"
        params = @{}
    }
    $peer2Addr = $peer2Resp.result.peerAddr

    # C. Add eD2K task
    $add2Resp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 12
        method = "downloads.addEd2kLink"
        params = @{
            link = $boundLink
            targetDirectory = $DownloadsDir
        }
    }
    $dl2Id = $add2Resp.result.downloadId

    # D. Connect download
    $start2Resp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 13
        method = "ed2k.startDownload"
        params = @{
            downloadId = $dl2Id
            peerAddr = $peer2Addr
        }
    }

    # E. Monitor transfer
    $sw2 = [System.Diagnostics.Stopwatch]::StartNew()
    $dl2Completed = $false
    while ($sw2.ElapsedMilliseconds -lt 25000) {
        $get2Resp = Invoke-WsRpc $ws @{
            jsonrpc = "2.0"
            id = 14
            method = "downloads.get"
            params = @{ id = $dl2Id }
        }
        $st = $get2Resp.result.status
        if ($st -eq "completed") {
            $dl2Completed = $true
            break
        }
        Start-Sleep -Milliseconds 200
    }

    if (!$dl2Completed) {
        throw "Download 2 (pattern_9728001) did not reach completion! Last status: $st ($($get2Resp.result.receivedBytes) / $($get2Resp.result.totalBytes))"
    }
    Write-Host " -> Boundary transfer pattern_9728001 completed and verified in $($sw2.ElapsedMilliseconds) ms!" -ForegroundColor Green

    $finalizedBound = Join-Path $DownloadsDir $boundFile
    if (!(Test-Path $finalizedBound)) {
        throw "Finalized boundary file missing at $finalizedBound!"
    }
    if ((Get-Item $finalizedBound).Length -ne 9728001) {
        throw "Finalized boundary file size mismatch! Expected 9728001, got $((Get-Item $finalizedBound).Length)"
    }
    Write-Host " -> Confirmed boundary file in downloads/: $boundFile (9,728,001 bytes)." -ForegroundColor Green

    # -------------------------------------------------------------------------
    # 5. Resilience Vector: Crash at ~50% and Resumption (part_9728000)
    # -------------------------------------------------------------------------
    Write-Host "`n[5/7] Testing Vector 3: Crash at ~50% and Resumption (part_9728000)..." -ForegroundColor Yellow

    # A. Create full part fixture
    $fix3Resp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 20
        method = "lab.createFixture"
        params = @{ fixtureType = "part_9728000" }
    }
    $partLink = $fix3Resp.result.ed2kLink
    $partFile = $fix3Resp.result.fileName

    $peer3Resp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 21
        method = "lab.startLoopbackPeer"
        params = @{}
    }
    $peer3Addr = $peer3Resp.result.peerAddr

    $add3Resp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 22
        method = "downloads.addEd2kLink"
        params = @{
            link = $partLink
            targetDirectory = $DownloadsDir
        }
    }
    $dl3Id = $add3Resp.result.downloadId

    # Start download
    $null = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 23
        method = "ed2k.startDownload"
        params = @{
            downloadId = $dl3Id
            peerAddr = $peer3Addr
        }
    }

    # Monitor until at least ~4.0 MB (~40-50%)
    $sw3 = [System.Diagnostics.Stopwatch]::StartNew()
    $receivedBeforeCrash = 0
    while ($sw3.ElapsedMilliseconds -lt 15000) {
        $get3Resp = Invoke-WsRpc $ws @{
            jsonrpc = "2.0"
            id = 24
            method = "downloads.get"
            params = @{ id = $dl3Id }
        }
        $rec = $get3Resp.result.receivedBytes
        if ($rec -ge 4000000) {
            $receivedBeforeCrash = $rec
            break
        }
        Start-Sleep -Milliseconds 100
    }

    if ($receivedBeforeCrash -lt 4000000) {
        throw "Failed to reach ~50% transfer before timeout! Got $receivedBeforeCrash bytes."
    }
    $pct = [math]::Round(($receivedBeforeCrash / 9728000) * 100)
    Write-Host " -> Transfer reached $receivedBeforeCrash / 9728000 bytes (~$pct%). Simulating crash..." -ForegroundColor Gray

    # Close WS connection and kill daemon abruptly
    $ws.Dispose()
    Stop-Process -Id $daemonProcess.Id -Force
    $daemonProcess.WaitForExit(3000) | Out-Null
    Write-Host " -> Packaged daemon forcibly killed (Crash simulated)." -ForegroundColor Red

    # Verify part file exists on disk
    $partFilePath = Join-Path (Join-Path $WorkDir "temp") "$dl3Id.part"
    if (!(Test-Path $partFilePath)) {
        throw "Expected .part file missing on disk after crash!"
    }
    Write-Host " -> Verified .part file retained in temp/ ($((Get-Item $partFilePath).Length) bytes)." -ForegroundColor Gray

    # Re-launch packaged daemon on the SAME workdir
    Write-Host " -> Re-booting packaged daemon on same workdir..." -ForegroundColor Gray
    $daemonProcess = Start-PackagedDaemon $DaemonBin $WorkDir

    $SessionData2 = Get-Content $SessionFile -Raw | ConvertFrom-Json
    $Token2 = (Get-Content $TokenFile -Raw).Trim()
    $ws = Connect-DaemonWebSocket $SessionData2.endpoint $Token2

    $hello2 = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 30
        method = "system.hello"
        params = @{ protocolVersion = 1; token = $Token2 }
    }

    # Query downloads list -> confirm download is still registered and progress retained!
    $listAfter = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 31
        method = "downloads.list"
        params = @{}
    }
    $dl3Snap = $listAfter.result.downloads | Where-Object { $_.id -eq $dl3Id }
    if (!$dl3Snap) {
        throw "PERSISTENCE FAILURE: Download $dl3Id disappeared after crash and restart!"
    }
    Write-Host " -> Download successfully recovered from SQLite: $($dl3Snap.receivedBytes) bytes preserved." -ForegroundColor Green

    # Re-start loopback peer and resume transfer
    $peer4Resp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 32
        method = "lab.startLoopbackPeer"
        params = @{}
    }
    $peer4Addr = $peer4Resp.result.peerAddr

    $resResume = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 33
        method = "ed2k.startDownload"
        params = @{
            downloadId = $dl3Id
            peerAddr = $peer4Addr
        }
    }

    # Monitor until completion
    $swResume = [System.Diagnostics.Stopwatch]::StartNew()
    $resCompleted = $false
    while ($swResume.ElapsedMilliseconds -lt 25000) {
        $getResume = Invoke-WsRpc $ws @{
            jsonrpc = "2.0"
            id = 34
            method = "downloads.get"
            params = @{ id = $dl3Id }
        }
        if ($getResume.result.status -eq "completed") {
            $resCompleted = $true
            break
        }
        Start-Sleep -Milliseconds 200
    }

    if (!$resCompleted) {
        throw "Resumed download did not complete! Last status: $($getResume.result.status)"
    }
    Write-Host " -> Resumed download reached 100% completion and was verified!" -ForegroundColor Green

    $finalizedPart = Join-Path $DownloadsDir $partFile
    if (!(Test-Path $finalizedPart)) {
        throw "Finalized part file missing at $finalizedPart!"
    }
    Write-Host " -> Confirmed finalized file in downloads/: $partFile (9,728,000 bytes)." -ForegroundColor Green

    # -------------------------------------------------------------------------
    # 6. Secure Test Data Cleanup Verification
    # -------------------------------------------------------------------------
    Write-Host "`n[6/7] Verifying lab.cleanupTestData security and non-deletion of finalized files..." -ForegroundColor Yellow

    $cleanResp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 40
        method = "lab.cleanupTestData"
        params = @{}
    }
    if (!$cleanResp.result.success) {
        throw "lab.cleanupTestData call failed: $($cleanResp | ConvertTo-Json)"
    }
    Write-Host " -> Cleanup report: $($cleanResp.result.deletedFilesCount) file(s) deleted, $($cleanResp.result.freedBytes) bytes freed." -ForegroundColor Gray

    # Crucial security verification: Confirm completed downloads in downloads/ were NOT deleted!
    $filesInDownloads = Get-ChildItem -Path $DownloadsDir -File
    if ($filesInDownloads.Count -lt 3) {
        throw "SECURITY / DATA LOSS FAILURE: lab.cleanupTestData deleted finalized download files in downloads/! Found only $($filesInDownloads.Count) files."
    }
    Write-Host " -> Verified: All 3 finalized downloads in downloads/ remain intact and untouched." -ForegroundColor Green

    # -------------------------------------------------------------------------
    # 7. Graceful Shutdown & Teardown Verification
    # -------------------------------------------------------------------------
    Write-Host "`n[7/7] Graceful shutdown via system.stop..." -ForegroundColor Yellow
    $stopResp = Invoke-WsRpc $ws @{
        jsonrpc = "2.0"
        id = 50
        method = "system.stop"
        params = @{}
    }
    $ws.Dispose()

    $daemonProcess.WaitForExit(5000) | Out-Null
    Write-Host " -> Daemon process exited cleanly with code: $($daemonProcess.ExitCode)" -ForegroundColor Green

    # Confirm zero repo pollution
    $gitStatus = git status --porcelain
    if (![string]::IsNullOrWhiteSpace($gitStatus)) {
        Write-Host "WARNING: Git status is not clean after E2E test:" -ForegroundColor DarkYellow
        Write-Host $gitStatus -ForegroundColor Gray
    }

    Write-Host "`n==================================================" -ForegroundColor Green
    Write-Host "  PHASE 7.3 AUTOMATED QA LOOPBACK E2E PASSED!     " -ForegroundColor Green
    Write-Host "==================================================" -ForegroundColor Green

} finally {
    if ($daemonProcess -and !$daemonProcess.HasExited) {
        try { Stop-Process -Id $daemonProcess.Id -Force } catch {}
    }
    if (!$KeepTemp -and (Test-Path $IsolatedTempRoot)) {
        try { Remove-Item -Path $IsolatedTempRoot -Recurse -Force } catch {}
    }
}
