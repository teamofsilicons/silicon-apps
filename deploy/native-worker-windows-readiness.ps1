#requires -Version 5.1
<# Trusted startup readiness only. Uploaded programs still have the runner's
   independent 25-second limit and are never passed to these functions. #>
function ConvertTo-ReadinessArgument([string]$Value) {
    if ($Value -ne '' -and $Value -notmatch '[\s"]') { return $Value }
    $Quoted = [regex]::Replace($Value, '(\\*)"', '$1$1\"')
    $Quoted = [regex]::Replace($Quoted, '(\\+)$', '$1$1')
    return '"' + $Quoted + '"'
}

function Invoke-NativeReadinessProcess([string]$Executable, [string[]]$Arguments, [int]$TimeoutSeconds) {
    $Info = New-Object System.Diagnostics.ProcessStartInfo
    $Info.FileName = $Executable
    $Info.Arguments = (($Arguments | ForEach-Object { ConvertTo-ReadinessArgument $_ }) -join ' ')
    $Info.UseShellExecute = $false
    $Info.CreateNoWindow = $true
    $Info.RedirectStandardOutput = $true
    $Info.RedirectStandardError = $true
    $Process = New-Object System.Diagnostics.Process
    $Process.StartInfo = $Info
    try {
        if (-not $Process.Start()) { throw 'Could not start the trusted readiness process.' }
        $Output = $Process.StandardOutput.ReadToEndAsync()
        $Errors = $Process.StandardError.ReadToEndAsync()
        $TimedOut = -not $Process.WaitForExit($TimeoutSeconds * 1000)
        if ($TimedOut) {
            $Process.Kill()
            if (-not $Process.WaitForExit(5000)) { throw 'Readiness process did not stop after its deadline.' }
        }
        if (-not $Output.Wait(5000) -or -not $Errors.Wait(5000)) { throw 'Readiness process output did not close.' }
        return @{ exit_code = $Process.ExitCode; timed_out = $TimedOut; stdout = $Output.Result.Trim(); stderr = $Errors.Result.Trim() }
    } finally { $Process.Dispose() }
}

function Assert-NativeReadinessCommand($Result, [string]$Operation) {
    if ($Result.timed_out -or $Result.exit_code -ne 0) { throw "$Operation failed or exceeded its startup deadline." }
}

function Initialize-NativeWindowsContainers([string]$Docker, [string]$Image) {
    if ($Image -notmatch '^mcr\.microsoft\.com/windows/servercore@sha256:[a-fA-F0-9]{64}$') { throw 'Readiness requires the pinned Microsoft servercore image.' }
    $Engine = Invoke-NativeReadinessProcess $Docker @('info', '--format', '{{.OSType}}') 15
    Assert-NativeReadinessCommand $Engine 'Docker readiness'
    if ($Engine.stdout -ne 'windows') { throw 'Readiness requires a Windows container engine.' }
    # Recover only our generated containers after a killed supervisor or host reboot.
    $Stale = Invoke-NativeReadinessProcess $Docker @('ps', '--all', '--quiet', '--filter', 'name=^/apps-(validation|readiness)-[a-f0-9]+$') 15
    Assert-NativeReadinessCommand $Stale 'Stale container discovery'
    foreach ($Id in ($Stale.stdout -split '\r?\n')) {
        if (-not $Id) { continue }
        if ($Id -notmatch '^[a-f0-9]{12,64}$') { throw 'Docker returned an unexpected container identifier.' }
        $Removed = Invoke-NativeReadinessProcess $Docker @('rm', '--force', $Id) 30
        Assert-NativeReadinessCommand $Removed 'Stale container cleanup'
    }
    $Name = 'apps-readiness-' + [Guid]::NewGuid().ToString('N')
    $Timer = [Diagnostics.Stopwatch]::StartNew()
    try {
        # The only guest command is the image's own cmd.exe/ver. No package,
        # host mount, credentials, or network is available inside this VM.
        $Probe = Invoke-NativeReadinessProcess $Docker @('run', '--pull=never', '--name', $Name,
            '--isolation=hyperv', '--platform=windows/amd64', '--network=none', '--memory=1g', '--cpu-count=1',
            '--user=ContainerUser', '--entrypoint=C:\Windows\System32\cmd.exe', $Image, '/d', '/c', 'ver') 90
        Assert-NativeReadinessCommand $Probe 'Trusted Hyper-V startup probe'
        if (-not $Probe.stdout) { throw 'Trusted Hyper-V startup probe returned no Windows version.' }
    } finally {
        # Cleanup is required even when starting the client fails or it times out.
        $Removed = Invoke-NativeReadinessProcess $Docker @('rm', '--force', $Name) 30
        Assert-NativeReadinessCommand $Removed 'Trusted startup container cleanup'
        $Timer.Stop()
    }
    return @{ status = 'ready'; ready_at = [DateTime]::UtcNow.ToString('o'); image = $Image;
        container_name = $Name; warmup_seconds = [Math]::Round($Timer.Elapsed.TotalSeconds, 3);
        container_removed = $true; isolation = 'hyperv'; network = 'none'; user = 'ContainerUser';
        memory_bytes = 1073741824; cpu_count = 1; trusted_command = 'cmd.exe /d /c ver'; windows_version = $Probe.stdout }
}

function Write-NativeReadinessReceipt([string]$Path, $Value) {
    $Temporary = $Path + '.new'
    [IO.File]::WriteAllText($Temporary, ($Value | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding($false)))
    Move-Item -LiteralPath $Temporary -Destination $Path -Force
}
