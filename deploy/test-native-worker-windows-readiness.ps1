#requires -Version 5.1
<# Runs bounded trusted OS processes and mocked Docker orchestration only.
   No container, installed service, account, or firewall is changed. #>
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'native-worker-windows-readiness.ps1')
$Fast = Invoke-NativeReadinessProcess "$env:SystemRoot\System32\cmd.exe" @('/d', '/c', 'ver') 5
Assert-NativeReadinessCommand $Fast 'Test Windows version'
if (-not $Fast.stdout) { throw 'Trusted process output was not captured.' }
$Clock = [Diagnostics.Stopwatch]::StartNew()
$Slow = Invoke-NativeReadinessProcess "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" @('-NoProfile', '-NonInteractive', '-Command', 'Start-Sleep -Seconds 10') 1
$Clock.Stop()
if (-not $Slow.timed_out -or $Clock.Elapsed.TotalSeconds -gt 7) { throw 'Trusted process deadline or termination failed.' }
$DeadlineSeconds = [Math]::Round($Clock.Elapsed.TotalSeconds, 3)
$Image = 'mcr.microsoft.com/windows/servercore@sha256:' + ('a' * 64)
$OriginalProcess = ${function:Invoke-NativeReadinessProcess}
try {
    function Invoke-NativeReadinessProcess([string]$Executable, [string[]]$Arguments, [int]$TimeoutSeconds) {
        $script:Calls += [pscustomobject]@{ arguments = $Arguments; timeout = $TimeoutSeconds }
        $Result = @{ exit_code = 0; timed_out = $false; stdout = ''; stderr = '' }
        switch ($Arguments[0]) {
            'info' { $Result.stdout = 'windows'; if ($script:Scenario -eq 'engine') { $Result.exit_code = 1 } }
            'ps' { if ($script:Scenario -eq 'stale') { $Result.stdout = '0123456789ab' } }
            'run' {
                if ($script:Scenario -eq 'exception') { throw 'Simulated client start failure.' }
                if ($script:Scenario -eq 'timeout') { $Result.timed_out = $true; $Result.exit_code = -1 }
                if ($script:Scenario -eq 'probe') { $Result.exit_code = 1 }
                $Result.stdout = 'Microsoft Windows [Version test]'
            }
            'rm' { if ($script:Scenario -eq 'cleanup') { $Result.exit_code = 1 } }
            default { throw 'Unexpected readiness operation.' }
        }
        return $Result
    }
    foreach ($script:Scenario in @('success', 'stale', 'engine', 'timeout', 'probe', 'exception', 'cleanup')) {
        $script:Calls = @(); $Failed = $false; $Ready = $null
        try { $Ready = Initialize-NativeWindowsContainers 'docker.exe' $Image } catch { $Failed = $true }
        $ExpectedFailure = $script:Scenario -notin @('success', 'stale')
        if ($Failed -ne $ExpectedFailure) { throw "Readiness did not fail closed for $script:Scenario." }
        $Runs = @($script:Calls | Where-Object { $_.arguments[0] -eq 'run' })
        $Removes = @($script:Calls | Where-Object { $_.arguments[0] -eq 'rm' })
        if ($script:Scenario -eq 'engine') {
            if ($Runs.Count -ne 0) { throw 'An unavailable engine still started a probe.' }
            continue
        }
        if ($Runs.Count -ne 1 -or $Runs[0].timeout -ne 90) { throw 'Trusted startup probe must have its own bounded deadline.' }
        $Command = $Runs[0].arguments
        foreach ($Required in @('--pull=never', '--isolation=hyperv', '--platform=windows/amd64', '--network=none', '--memory=1g', '--cpu-count=1', '--user=ContainerUser', '--entrypoint=C:\Windows\System32\cmd.exe', $Image, '/d', '/c', 'ver')) {
            if ($Command -notcontains $Required) { throw "Trusted probe omitted $Required." }
        }
        if (@($Command | Where-Object { $_ -match '(?i)mount|volume|package' }).Count) { throw 'Trusted readiness may not mount or execute package data.' }
        $Name = $Command[[array]::IndexOf($Command, '--name') + 1]
        if ($Name -notmatch '^apps-readiness-[a-f0-9]{32}$') { throw 'Readiness container name escaped its cleanup namespace.' }
        if (@($Removes | Where-Object { $_.arguments -contains $Name -and $_.arguments -contains '--force' -and $_.timeout -eq 30 }).Count -ne 1) { throw "Readiness container cleanup was not attempted for $script:Scenario." }
        if ($script:Scenario -eq 'stale' -and @($Removes | Where-Object { $_.arguments -contains '0123456789ab' }).Count -ne 1) { throw 'Stale readiness container was not removed.' }
        if (-not $Failed -and ($Ready.status -ne 'ready' -or -not $Ready.container_removed)) { throw 'Successful readiness lacked its cleanup receipt.' }
    }
    $script:Calls = @(); $Rejected = $false
    try { Initialize-NativeWindowsContainers 'docker.exe' 'mcr.microsoft.com/windows/servercore:ltsc2025' } catch { $Rejected = $true }
    if (-not $Rejected -or $script:Calls.Count) { throw 'Readiness accepted an unpinned image.' }
} finally { Set-Item -LiteralPath function:Invoke-NativeReadinessProcess -Value $OriginalProcess }
@{ passed = $true; trusted_process_timeout_seconds = $DeadlineSeconds; scenarios = 8; failure_cleanup_verified = $true; startup_gate_fails_closed = $true } | ConvertTo-Json -Compress
