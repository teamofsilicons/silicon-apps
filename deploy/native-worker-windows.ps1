#requires -Version 5.1
#requires -RunAsAdministrator
<# Install only on a dedicated Windows worker after windows-container-bootstrap.ps1.
   No package executes directly on the host: server.py requires Hyper-V isolation. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Source,
    [Parameter(Mandatory=$true)][string]$PythonPath,
    [Parameter(Mandatory=$true)][string]$Image,
    [Parameter(Mandatory=$true)][string]$TokenFile,
    [Parameter(Mandatory=$true)][string]$TlsCert,
    [Parameter(Mandatory=$true)][string]$TlsKey,
    [Parameter(Mandatory=$true)][string]$BindAddress,
    [Parameter(Mandatory=$true)][string]$GatewayAddress,
    [ValidateRange(1,65535)][int]$Port = 4313
)
$ErrorActionPreference = 'Stop'
$Install = 'C:\ProgramData\SiliconAppsNative'
$TaskName = 'Silicon Apps Native Runner'
$Account = 'SiliconAppsRunner'
$Description = 'Silicon Apps managed Hyper-V validation worker'
if ($Image -notmatch '^mcr\.microsoft\.com/windows/servercore@sha256:[a-fA-F0-9]{64}$') { throw 'Image must be an immutable Microsoft servercore digest.' }
foreach ($Address in @($BindAddress, $GatewayAddress)) {
    $Parsed = $null
    if (-not [System.Net.IPAddress]::TryParse($Address, [ref]$Parsed)) { throw 'Bind and gateway addresses must be literal private interface IPs.' }
    $Bytes = $Parsed.GetAddressBytes()
    if ($Bytes.Length -ne 4 -or -not ($Bytes[0] -eq 10 -or ($Bytes[0] -eq 172 -and $Bytes[1] -ge 16 -and $Bytes[1] -le 31) -or ($Bytes[0] -eq 192 -and $Bytes[1] -eq 168))) { throw 'Bind and gateway addresses must be private RFC1918 IPv4 addresses.' }
}
foreach ($File in @($PythonPath, $TokenFile, $TlsCert, $TlsKey, "$Source\runner\server.py", "$Source\deploy\native-worker-launch.py")) {
    if (-not [System.IO.Path]::IsPathRooted($File) -or -not (Test-Path -LiteralPath $File -PathType Leaf)) { throw 'All input files must exist at absolute paths.' }
}
$Token = [System.IO.File]::ReadAllText($TokenFile).Trim()
if ($Token.Length -lt 32 -or $Token -match '[^\x21-\x7E]') { throw 'Worker token must contain at least 32 ASCII non-space characters.' }
if ((Get-WindowsFeature Hyper-V).InstallState -ne 'Installed' -or (Get-WindowsFeature Containers).InstallState -ne 'Installed') { throw 'Hyper-V and Containers must be installed, with any required reboot completed.' }
# SSM may predate the Docker installation and retain its earlier process PATH.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine')
$Docker = (Get-Command docker.exe -ErrorAction Stop).Source
$EngineOS = & $Docker info --format '{{.OSType}}'
if ($LASTEXITCODE -ne 0 -or $EngineOS.Trim() -ne 'windows') { throw 'A running Windows Docker engine is required.' }
& $Docker image inspect $Image *> $null
if ($LASTEXITCODE -ne 0) { throw 'Pinned image must already be pulled and validated by the platform bootstrap.' }

$Existing = Get-LocalUser -Name $Account -ErrorAction SilentlyContinue
if ($Existing -and $Existing.Description -ne $Description) { throw 'Refusing to replace an unrelated local account.' }
Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
$Random = New-Object byte[] 48
$Generator = [System.Security.Cryptography.RandomNumberGenerator]::Create()
$Generator.GetBytes($Random)
$Generator.Dispose()
# Keep all Windows password complexity classes while retaining 384 random bits.
$Password = 'Aa1!' + [Convert]::ToBase64String($Random)
$SecurePassword = ConvertTo-SecureString $Password -AsPlainText -Force
if ($Existing) { Set-LocalUser -Name $Account -Password $SecurePassword -PasswordNeverExpires $true }
else { New-LocalUser -Name $Account -Password $SecurePassword -PasswordNeverExpires -UserMayNotChangePassword -Description $Description | Out-Null }
$Administrators = Get-LocalGroup -SID 'S-1-5-32-544'
if (-not (Get-LocalGroupMember -Group $Administrators.Name | Where-Object SID -eq (Get-LocalUser $Account).SID)) {
    Add-LocalGroupMember -Group $Administrators.Name -Member $Account
}
$Identity = "$env:COMPUTERNAME\$Account"
New-Item -ItemType Directory -Force -Path $Install, "$Install\jobs", "$Install\logs" | Out-Null
# Remove inherited user access before writing credentials or private keys.
& icacls.exe $Install /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' "${Identity}:(OI)(CI)F" *> $null
if ($LASTEXITCODE -ne 0) { throw 'Could not restrict worker directory ACL.' }
Copy-Item -LiteralPath "$Source\runner\server.py" -Destination "$Install\server.py" -Force
Copy-Item -LiteralPath "$Source\deploy\native-worker-launch.py" -Destination "$Install\launch.py" -Force
Copy-Item -LiteralPath $TlsCert -Destination "$Install\server.crt" -Force
Copy-Item -LiteralPath $TlsKey -Destination "$Install\server.key" -Force
if (-not (Test-Path "$Install\venv\Scripts\python.exe")) {
    & $PythonPath -m venv "$Install\venv"
    if ($LASTEXITCODE -ne 0) { throw 'Could not create the worker Python environment.' }
}
$Python = "$Install\venv\Scripts\python.exe"
& $Python -m pip install --disable-pip-version-check -r "$Source\runner\requirements.txt"
if ($LASTEXITCODE -ne 0) { throw 'Could not install runner dependencies.' }
# Windows venv python.exe redirects to the base interpreter, which owns the
# listening socket. A firewall rule for the redirector does not match it.
$RuntimePython = & $Python -c 'import os, sys; print(os.path.realpath(sys._base_executable))'
if ($LASTEXITCODE -ne 0 -or -not [System.IO.Path]::IsPathRooted($RuntimePython) -or -not (Test-Path -LiteralPath $RuntimePython -PathType Leaf)) { throw 'Could not identify the Python executable that owns the worker listener.' }
$Config = @{
    APPS_RUNNER_TOKEN = $Token; APPS_RUNNER_HOST = $BindAddress; APPS_RUNNER_PORT = "$Port"
    APPS_RUNNER_TLS_CERT = "$Install\server.crt"; APPS_RUNNER_TLS_KEY = "$Install\server.key"
    APPS_RUNNER_IMAGES = (@{'windows-x86_64'=$Image} | ConvertTo-Json -Compress)
    TMPDIR = "$Install\jobs"
}
[System.IO.File]::WriteAllText("$Install\worker.json", ($Config | ConvertTo-Json -Compress), (New-Object System.Text.UTF8Encoding($false)))
# This supervisor runs forever and backs off on crashes. The scheduled task also
# restarts after supervisor failure and comes back on every host boot.
$Supervisor = @'
$ErrorActionPreference = 'Stop'
$base = 'C:\ProgramData\SiliconAppsNative'
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine')
while ($true) {
    try {
        # A killed supervisor/server may leave a VM behind. This is a dedicated,
        # serialized worker; clean only this runner's generated container names.
        $stale = @(& docker.exe ps --all --quiet --filter 'name=^/apps-validation-[a-f0-9]+$')
        if ($LASTEXITCODE -ne 0) { throw 'Docker engine unavailable.' }
        foreach ($container in $stale) {
            if ($container) { & docker.exe rm --force $container *> $null }
            if ($LASTEXITCODE -ne 0) { throw 'Could not clean a stale validation container.' }
        }
        foreach ($log in @('runner.log', 'runner-error.log', 'supervisor.log')) {
            $path = Join-Path "$base\logs" $log
            if ((Test-Path -LiteralPath $path) -and (Get-Item -LiteralPath $path).Length -gt 8MB) { Move-Item -LiteralPath $path -Destination "$path.previous" -Force }
        }
        & "$base\venv\Scripts\python.exe" "$base\launch.py" --config "$base\worker.json" --server "$base\server.py" 1>> "$base\logs\runner.log" 2>> "$base\logs\runner-error.log"
    } catch { Add-Content -LiteralPath "$base\logs\supervisor.log" -Value 'Runner process stopped; retrying in ten seconds.' }
    Start-Sleep -Seconds 10
}
'@
[System.IO.File]::WriteAllText("$Install\supervisor.ps1", $Supervisor, (New-Object System.Text.UTF8Encoding($false)))
$Action = New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument "-NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File `"$Install\supervisor.ps1`"" -WorkingDirectory $Install
$Settings = New-ScheduledTaskSettingsSet -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1) -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew -StartWhenAvailable
$Trigger = New-ScheduledTaskTrigger -AtStartup
Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -User $Identity -Password $Password -RunLevel Highest -Force | Out-Null
# Windows Firewall is additional to a VPC security group allowing only gateway.
$RuleName = 'Silicon Apps Native Runner Private TLS'
Remove-NetFirewallRule -DisplayName $RuleName -ErrorAction SilentlyContinue
New-NetFirewallRule -DisplayName $RuleName -Direction Inbound -Action Allow -Protocol TCP -LocalAddress $BindAddress -LocalPort $Port -RemoteAddress $GatewayAddress -Program $RuntimePython | Out-Null
Start-ScheduledTask -TaskName $TaskName
$Password = $null; $SecurePassword = $null; $Token = $null; $Config = $null
Write-Output 'Native Windows worker installed. Verify TLS, authenticated Hyper-V probes, reboot recovery, and package receipts before enabling this target.'
