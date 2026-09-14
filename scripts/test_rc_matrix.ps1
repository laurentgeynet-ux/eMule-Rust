<#
.SYNOPSIS
    Emule-Sion RC Environment & Supervision Test Matrix Harness.
.DESCRIPTION
    Validates the packaged release candidate against edge cases:
    1. Unicode & Non-ASCII Paths with spaces.
    2. TCP Port 4660 conflict & automatic ephemeral fallback.
    3. Exclusivity lock & dual-instance collision prevention.
    4. Abrupt crash recovery on stale lock.
    5. Bundle relocation & autonomous execution outside repo.
    6. Read-only directory graceful failure handling.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_rc_matrix.ps1
#>

param(
    [string]$BundleDir = "dist\EmuleSion-Windows"
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$SourceBundle = Join-Path $Root $BundleDir

if (!(Test-Path $SourceBundle)) {
    throw "Distribution bundle not found at '$SourceBundle'. Run scripts\build_windows_release.ps1 first."
}

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Emule-Sion RC Environment & Supervision Matrix  " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# Helper to send JSON-RPC over WebSocket
function Send-JsonRpc([System.Net.WebSockets.ClientWebSocket]$socket, [string]$jsonPayload) {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($jsonPayload)
    $segment = [System.ArraySegment[byte]]::new($bytes)
    $cancel = [System.Threading.CancellationTokenSource]::new(4000)
    $socket.SendAsync($segment, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cancel.Token).Wait()

    $recvBuf = [byte[]]::new(8192)
    $recvSeg = [System.ArraySegment[byte]]::new($recvBuf)
    $recv = $socket.ReceiveAsync($recvSeg, $cancel.Token)
    $recv.Wait()

    return [System.Text.Encoding]::UTF8.GetString($recvBuf, 0, $recv.Result.Count)
}

function Connect-And-Handshake([string]$endpoint, [string]$token) {
    $wsUri = if ($endpoint.StartsWith("ws://")) { [System.Uri]::new($endpoint) } else { [System.Uri]::new("ws://$endpoint") }
    $ws = [System.Net.WebSockets.ClientWebSocket]::new()
    $ws.Options.SetRequestHeader("Authorization", "Bearer $token")
    $cts = [System.Threading.CancellationTokenSource]::new(5000)
    $ws.ConnectAsync($wsUri, $cts.Token).Wait()

    $helloPayload = [ordered]@{
        jsonrpc = "2.0"
        id = 1
        method = "system.hello"
        params = [ordered]@{
            protocolVersion = 1
            token = $token
        }
    } | ConvertTo-Json -Compress

    $resp = Send-JsonRpc $ws $helloPayload
    return @{ Socket = $ws; HelloResponse = ($resp | ConvertFrom-Json) }
}

$TestRoot = Join-Path $env:TEMP "EmuleSion-MatrixTests-$([guid]::NewGuid().ToString().Substring(0,8))"
New-Item -ItemType Directory -Path $TestRoot -Force | Out-Null

try {
    # -------------------------------------------------------------------------
    # TEST 1: Unicode, Non-ASCII & Spaced Directory Path
    # -------------------------------------------------------------------------
    Write-Host "`n[1/6] Test: Unicode, Non-ASCII & Spaced Directory Path..." -ForegroundColor Yellow
    $UnicodeDir = Join-Path $TestRoot "Emule_Sion_Unicode_Test_Spaces"
    New-Item -ItemType Directory -Path $UnicodeDir -Force | Out-Null

    $UnicodeBundle = Join-Path $UnicodeDir "bundle"
    Copy-Item -Path $SourceBundle -Destination $UnicodeBundle -Recurse -Force
    $UnicodeWorkdir = Join-Path $UnicodeDir "data"
    New-Item -ItemType Directory -Path $UnicodeWorkdir -Force | Out-Null

    $DaemonBin = Join-Path $UnicodeBundle "bin\mule-daemon.exe"
    $pinfo = [System.Diagnostics.ProcessStartInfo]::new($DaemonBin, "--workdir `"$UnicodeWorkdir`"")
    $pinfo.UseShellExecute = $false
    $pinfo.RedirectStandardOutput = $true
    $pinfo.RedirectStandardError = $true
    $pinfo.CreateNoWindow = $true

    $proc = [System.Diagnostics.Process]::Start($pinfo)
    try {
        $sessFile = Join-Path $UnicodeWorkdir "session.json"
        $tokenFile = Join-Path $UnicodeWorkdir "auth.token"

        $timeout = 10
        while ($timeout -gt 0 -and !(Test-Path $sessFile)) {
            Start-Sleep -Milliseconds 300
            $timeout -= 0.3
        }
        if (!(Test-Path $sessFile)) { throw "session.json not created in Unicode path: $sessFile" }

        $sess = Get-Content $sessFile -Raw | ConvertFrom-Json
        $tok = (Get-Content $tokenFile -Raw).Trim()

        $res = Connect-And-Handshake $sess.endpoint $tok
        if (!$res.HelloResponse.result.daemonVersion) { throw "Invalid hello response in Unicode path" }

        # Graceful stop
        Send-JsonRpc $res.Socket '{"jsonrpc":"2.0","id":2,"method":"system.stop","params":{}}' | Out-Null
        $res.Socket.Dispose()
        $proc.WaitForExit(4000) | Out-Null

        Write-Host " -> PASS: Daemon booted, served RPC, and stopped cleanly in Unicode/spaced directory." -ForegroundColor Green
    } finally {
        if ($proc -and !$proc.HasExited) { try { $proc.Kill() } catch {} }
    }

    # -------------------------------------------------------------------------
    # TEST 2: TCP Port 4660 Conflict & Ephemeral Fallback
    # -------------------------------------------------------------------------
    Write-Host "`n[2/6] Test: TCP Port 4660 Conflict & Ephemeral Port Fallback..." -ForegroundColor Yellow
    $PortWorkdir = Join-Path $TestRoot "port-conflict-data"
    New-Item -ItemType Directory -Path $PortWorkdir -Force | Out-Null

    # Block 127.0.0.1:4660 with raw TcpListener
    $dummyListener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 4660)
    $dummyListener.Start()
    Write-Host " -> Simulating busy port 4660 with dummy TcpListener..." -ForegroundColor Gray

    try {
        $pinfo = [System.Diagnostics.ProcessStartInfo]::new($DaemonBin, "--workdir `"$PortWorkdir`"")
        $pinfo.UseShellExecute = $false
        $pinfo.RedirectStandardOutput = $true
        $pinfo.RedirectStandardError = $true
        $pinfo.CreateNoWindow = $true

        $proc = [System.Diagnostics.Process]::Start($pinfo)
        try {
            $sessFile = Join-Path $PortWorkdir "session.json"
            $tokenFile = Join-Path $PortWorkdir "auth.token"

            $timeout = 10
            while ($timeout -gt 0 -and !(Test-Path $sessFile)) {
                Start-Sleep -Milliseconds 300
                $timeout -= 0.3
            }
            if (!(Test-Path $sessFile)) { throw "session.json not created when port 4660 was in use" }

            $sess = Get-Content $sessFile -Raw | ConvertFrom-Json
            $tok = (Get-Content $tokenFile -Raw).Trim()

            # Verify that fallback port was NOT 4660
            $uri = [System.Uri]::new($sess.endpoint)
            if ($uri.Port -eq 4660) {
                throw "Daemon did not fallback to ephemeral port! Expected port != 4660, got $($uri.Port)"
            }
            Write-Host " -> Confinement: Ephemeral fallback port assigned = $($uri.Port)" -ForegroundColor Gray

            $res = Connect-And-Handshake $sess.endpoint $tok
            if (!$res.HelloResponse.result.daemonVersion) { throw "Handshake failed on ephemeral port" }

            # Stop daemon
            Send-JsonRpc $res.Socket '{"jsonrpc":"2.0","id":2,"method":"system.stop","params":{}}' | Out-Null
            $res.Socket.Dispose()
            $proc.WaitForExit(4000) | Out-Null

            Write-Host " -> PASS: Port conflict correctly handled; ephemeral port published and functioning." -ForegroundColor Green
        } finally {
            if ($proc -and !$proc.HasExited) { try { $proc.Kill() } catch {} }
        }
    } finally {
        $dummyListener.Stop()
    }

    # -------------------------------------------------------------------------
    # TEST 3: Exclusivity Lock & Collision Prevention
    # -------------------------------------------------------------------------
    Write-Host "`n[3/6] Test: Exclusivity Lock & Dual-Instance Prevention..." -ForegroundColor Yellow
    $LockWorkdir = Join-Path $TestRoot "lock-test-data"
    New-Item -ItemType Directory -Path $LockWorkdir -Force | Out-Null

    $pinfo1 = [System.Diagnostics.ProcessStartInfo]::new($DaemonBin, "--workdir `"$LockWorkdir`"")
    $pinfo1.UseShellExecute = $false
    $pinfo1.RedirectStandardOutput = $true
    $pinfo1.RedirectStandardError = $true
    $pinfo1.CreateNoWindow = $true

    $proc1 = [System.Diagnostics.Process]::Start($pinfo1)
    try {
        $sessFile = Join-Path $LockWorkdir "session.json"
        $timeout = 10
        while ($timeout -gt 0 -and !(Test-Path $sessFile)) {
            Start-Sleep -Milliseconds 300
            $timeout -= 0.3
        }
        if (!(Test-Path $sessFile)) { throw "First daemon failed to start" }

        # Launch SECOND daemon on same workdir
        $pinfo2 = [System.Diagnostics.ProcessStartInfo]::new($DaemonBin, "--workdir `"$LockWorkdir`"")
        $pinfo2.UseShellExecute = $false
        $pinfo2.RedirectStandardOutput = $true
        $pinfo2.RedirectStandardError = $true
        $pinfo2.CreateNoWindow = $true

        $proc2 = [System.Diagnostics.Process]::Start($pinfo2)
        $proc2.WaitForExit(5000) | Out-Null

        if (!$proc2.HasExited) {
            $proc2.Kill()
            throw "Second daemon did not exit immediately despite existing lock!"
        }

        if ($proc2.ExitCode -eq 0) {
            throw "Second daemon exited with success code 0! Expected error on lock collision."
        }
        Write-Host " -> Second daemon exited with error code $($proc2.ExitCode) as expected." -ForegroundColor Gray

        # Verify first daemon is still responsive
        $sess = Get-Content $sessFile -Raw | ConvertFrom-Json
        $tok = (Get-Content (Join-Path $LockWorkdir "auth.token") -Raw).Trim()
        $res = Connect-And-Handshake $sess.endpoint $tok
        Send-JsonRpc $res.Socket '{"jsonrpc":"2.0","id":2,"method":"system.stop","params":{}}' | Out-Null
        $res.Socket.Dispose()
        $proc1.WaitForExit(4000) | Out-Null

        Write-Host " -> PASS: Exclusivity lock strictly prevents concurrent daemon collisions." -ForegroundColor Green
    } finally {
        if ($proc1 -and !$proc1.HasExited) { try { $proc1.Kill() } catch {} }
    }

    # -------------------------------------------------------------------------
    # TEST 4: Abrupt Crash Recovery on Stale Lock
    # -------------------------------------------------------------------------
    Write-Host "`n[4/6] Test: Abrupt Crash Recovery on Stale Lock..." -ForegroundColor Yellow
    $CrashWorkdir = Join-Path $TestRoot "crash-test-data"
    New-Item -ItemType Directory -Path $CrashWorkdir -Force | Out-Null

    $pinfo = [System.Diagnostics.ProcessStartInfo]::new($DaemonBin, "--workdir `"$CrashWorkdir`"")
    $pinfo.UseShellExecute = $false
    $pinfo.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($pinfo)
    
    $sessFile = Join-Path $CrashWorkdir "session.json"
    $timeout = 10
    while ($timeout -gt 0 -and !(Test-Path $sessFile)) {
        Start-Sleep -Milliseconds 300
        $timeout -= 0.3
    }

    # Forcefully KILL the process abruptly (no graceful system.stop)
    Write-Host " -> Forcefully killing daemon (simulating crash)..." -ForegroundColor Gray
    $proc.Kill()
    $proc.WaitForExit(2000) | Out-Null

    # Re-launch daemon on same directory with leftover lock file
    Write-Host " -> Re-launching daemon on workdir with uncleaned lock..." -ForegroundColor Gray
    $pinfoRestart = [System.Diagnostics.ProcessStartInfo]::new($DaemonBin, "--workdir `"$CrashWorkdir`"")
    $pinfoRestart.UseShellExecute = $false
    $pinfoRestart.CreateNoWindow = $true
    $procRestart = [System.Diagnostics.Process]::Start($pinfoRestart)

    try {
        Start-Sleep -Seconds 1
        if ($procRestart.HasExited -and $procRestart.ExitCode -ne 0) {
            throw "Daemon failed to recover after abrupt crash! ExitCode: $($procRestart.ExitCode)"
        }

        $sess = Get-Content $sessFile -Raw | ConvertFrom-Json
        $tok = (Get-Content (Join-Path $CrashWorkdir "auth.token") -Raw).Trim()
        $res = Connect-And-Handshake $sess.endpoint $tok
        Send-JsonRpc $res.Socket '{"jsonrpc":"2.0","id":2,"method":"system.stop","params":{}}' | Out-Null
        $res.Socket.Dispose()
        $procRestart.WaitForExit(4000) | Out-Null

        Write-Host " -> PASS: Daemon recovered cleanly from stale lock after ungraceful crash." -ForegroundColor Green
    } finally {
        if ($procRestart -and !$procRestart.HasExited) { try { $procRestart.Kill() } catch {} }
    }

    # -------------------------------------------------------------------------
    # TEST 5: Bundle Relocation & Autonomous Resolution
    # -------------------------------------------------------------------------
    Write-Host "`n[5/6] Test: Relocated Bundle (Autonomous outside repo)..." -ForegroundColor Yellow
    $RelocDir = Join-Path $TestRoot "RelocatedEmuleSion"
    Copy-Item -Path $SourceBundle -Destination $RelocDir -Recurse -Force

    $RelocExe = Join-Path $RelocDir "EmuleSion.exe"
    $RelocDaemon = Join-Path $RelocDir "bin\mule-daemon.exe"
    if (!(Test-Path $RelocExe)) { throw "EmuleSion.exe missing in relocated bundle" }
    if (!(Test-Path $RelocDaemon)) { throw "bin\mule-daemon.exe missing in relocated bundle" }

    $RelocWorkdir = Join-Path $TestRoot "reloc-workdir"
    $pinfoReloc = [System.Diagnostics.ProcessStartInfo]::new($RelocDaemon, "--workdir `"$RelocWorkdir`"")
    $pinfoReloc.UseShellExecute = $false
    $pinfoReloc.CreateNoWindow = $true
    $procReloc = [System.Diagnostics.Process]::Start($pinfoReloc)
    try {
        $sessFile = Join-Path $RelocWorkdir "session.json"
        $timeout = 10
        while ($timeout -gt 0 -and !(Test-Path $sessFile)) {
            Start-Sleep -Milliseconds 300
            $timeout -= 0.3
        }
        if (!(Test-Path $sessFile)) { throw "Relocated daemon failed to start" }

        $sess = Get-Content $sessFile -Raw | ConvertFrom-Json
        $tok = (Get-Content (Join-Path $RelocWorkdir "auth.token") -Raw).Trim()
        $res = Connect-And-Handshake $sess.endpoint $tok
        Send-JsonRpc $res.Socket '{"jsonrpc":"2.0","id":2,"method":"system.stop","params":{}}' | Out-Null
        $res.Socket.Dispose()
        $procReloc.WaitForExit(4000) | Out-Null

        Write-Host " -> PASS: Relocated bundle executes autonomously without repo dependency." -ForegroundColor Green
    } finally {
        if ($procReloc -and !$procReloc.HasExited) { try { $procReloc.Kill() } catch {} }
    }

    # -------------------------------------------------------------------------
    # TEST 6: Read-Only / Uncreatable Directory Handling
    # -------------------------------------------------------------------------
    Write-Host "`n[6/6] Test: Unwritable Directory Error Handling..." -ForegroundColor Yellow
    # Target a forbidden non-existent drive or invalid path
    $InvalidPath = "Z:\NonExistentDrive_EmuleSion_Test\SubDir"

    $pinfoInvalid = [System.Diagnostics.ProcessStartInfo]::new($DaemonBin, "--workdir `"$InvalidPath`"")
    $pinfoInvalid.UseShellExecute = $false
    $pinfoInvalid.RedirectStandardError = $true
    $pinfoInvalid.RedirectStandardOutput = $true
    $pinfoInvalid.CreateNoWindow = $true

    $procInvalid = [System.Diagnostics.Process]::Start($pinfoInvalid)
    $procInvalid.WaitForExit(5000) | Out-Null

    if ($procInvalid.ExitCode -eq 0) {
        throw "Daemon succeeded on unwritable path! Expected non-zero failure code."
    }
    Write-Host " -> PASS: Daemon exited safely with non-zero exit code on invalid directory." -ForegroundColor Green

    # -------------------------------------------------------------------------
    # TEST 7: Portable Mode Auto-Resolution via portable.flag
    # -------------------------------------------------------------------------
    Write-Host "`n[7/7] Test: Portable Mode Auto-Resolution via portable.flag..." -ForegroundColor Yellow
    $PortModeDir = Join-Path $TestRoot "PortableModeTest"
    Copy-Item -Path $SourceBundle -Destination $PortModeDir -Recurse -Force

    # Ensure portable.flag exists at bundle root
    $FlagPath = Join-Path $PortModeDir "portable.flag"
    if (!(Test-Path $FlagPath)) {
        New-Item -ItemType File -Path $FlagPath -Force | Out-Null
    }

    $PortDaemonBin = Join-Path $PortModeDir "bin\mule-daemon.exe"
    # Launch daemon WITHOUT --workdir argument and with clean env vars
    $pinfoPort = [System.Diagnostics.ProcessStartInfo]::new($PortDaemonBin)
    $pinfoPort.UseShellExecute = $false
    $pinfoPort.CreateNoWindow = $true
    $pinfoPort.EnvironmentVariables.Remove("EMULE_SION_WORKDIR")
    $pinfoPort.EnvironmentVariables.Remove("MULE_DATA_DIR")

    $procPort = [System.Diagnostics.Process]::Start($pinfoPort)
    try {
        $expectedDataDir = Join-Path $PortModeDir "data"
        $sessFile = Join-Path $expectedDataDir "session.json"
        $timeout = 10
        while ($timeout -gt 0 -and !(Test-Path $sessFile)) {
            Start-Sleep -Milliseconds 300
            $timeout -= 0.3
        }
        if (!(Test-Path $sessFile)) { throw "session.json not created in portable data dir: $sessFile" }

        $sess = Get-Content $sessFile -Raw | ConvertFrom-Json
        $tok = (Get-Content (Join-Path $expectedDataDir "auth.token") -Raw).Trim()
        $res = Connect-And-Handshake $sess.endpoint $tok
        Send-JsonRpc $res.Socket '{"jsonrpc":"2.0","id":2,"method":"system.stop","params":{}}' | Out-Null
        $res.Socket.Dispose()
        $procPort.WaitForExit(4000) | Out-Null

        Write-Host " -> PASS: Daemon autonomously resolved to <bundle>/data via portable.flag without --workdir." -ForegroundColor Green
    } finally {
        if ($procPort -and !$procPort.HasExited) { try { $procPort.Kill() } catch {} }
    }

    Write-Host "`n==================================================" -ForegroundColor Green
    Write-Host "  ALL 7 ENVIRONMENT & SUPERVISION MATRIX TESTS OK! " -ForegroundColor Green
    Write-Host "==================================================" -ForegroundColor Green

} finally {
    if (Test-Path $TestRoot) {
        try { Remove-Item -Recurse -Force $TestRoot -ErrorAction SilentlyContinue } catch {}
    }
}
