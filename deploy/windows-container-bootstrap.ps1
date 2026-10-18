#Requires -Version 5.1
<#
Prepare a dedicated Windows Server 2025 Core x64 validator through AWS SSM.
Run as Administrator/SYSTEM. Exit 3010 asks SSM to reboot and rerun this script;
the persistent marker permits at most one requested reboot. No public listener,
worker credentials, AWS resources, package execution, or catalog changes here.
#>
[CmdletBinding()]
param(
    [string]$PinsPath = (Join-Path $PSScriptRoot 'windows-container-bootstrap.json'),
    [string]$StateDir = 'C:\ProgramData\SiliconAppsContainerBootstrap',
    [switch]$LibraryOnly
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

function ConvertTo-NativeArgument([string]$Value) {
    if ($Value -ne '' -and $Value -notmatch '[\s"]') { return $Value }
    $quoted = [regex]::Replace($Value, '(\\*)"', '$1$1\"')
    $quoted = [regex]::Replace($quoted, '(\\+)$', '$1$1')
    return '"' + $quoted + '"'
}

function Invoke-Checked([string]$Executable, [string[]]$Arguments, [int]$TimeoutSeconds = 120,
                        [int[]]$AllowedExitCodes = @(0)) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Executable
    $info.Arguments = (($Arguments | ForEach-Object { ConvertTo-NativeArgument $_ }) -join ' ')
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $info
    try {
        if (-not $process.Start()) { throw "Could not start $Executable" }
        $outTask = $process.StandardOutput.ReadToEndAsync()
        $errTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $process.Kill()
            $process.WaitForExit()
            throw "Timed out after $TimeoutSeconds seconds: $Executable"
        }
        $result = [pscustomobject]@{ exit_code = $process.ExitCode; stdout = $outTask.Result.Trim(); stderr = $errTask.Result.Trim() }
        if ($AllowedExitCodes -notcontains $result.exit_code) {
            $detail = $result.stderr
            if ($detail.Length -gt 4000) { $detail = $detail.Substring($detail.Length - 4000) }
            throw "$Executable exited $($result.exit_code): $detail"
        }
        return $result
    } finally { $process.Dispose() }
}

function Write-Receipt([string]$Path, $Value) {
    $temporary = $Path + '.tmp'
    [IO.File]::WriteAllText($temporary, ($Value | ConvertTo-Json -Depth 20), (New-Object Text.UTF8Encoding($false)))
    Move-Item -LiteralPath $temporary -Destination $Path -Force
}

function Assert-Artifact([string]$Path, [string]$Expected) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Expected) {
        throw "SHA-256 mismatch for $([IO.Path]::GetFileName($Path)); nothing from this download was executed."
    }
}

function Get-PinnedDownload($Pin, [string]$Destination) {
    if (-not (Test-Path -LiteralPath $Destination)) {
        $partial = $Destination + '.partial'
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $Pin.url -OutFile $partial -TimeoutSec 300
            Assert-Artifact $partial $Pin.sha256
            Move-Item -LiteralPath $partial -Destination $Destination -Force
        } finally { Remove-Item -LiteralPath $partial -Force -ErrorAction SilentlyContinue }
    }
    Assert-Artifact $Destination $Pin.sha256
}

function Assert-HyperVContainer($Inspection, [string]$Image) {
    if ($Inspection.HostConfig.Isolation -ne 'hyperv' -or
        $Inspection.HostConfig.NetworkMode -ne 'none' -or
        $Inspection.HostConfig.CpuCount -ne 1 -or
        $Inspection.HostConfig.Memory -ne 1073741824 -or
        $Inspection.Config.User -ne 'ContainerUser' -or
        $Inspection.Config.Image -ne $Image -or
        $Inspection.State.ExitCode -ne 0) {
        throw 'Container did not complete with the required Hyper-V isolation and resource restrictions.'
    }
    $mount = @($Inspection.Mounts | Where-Object { $_.Destination -eq 'C:\package' })
    if ($mount.Count -ne 1 -or $mount[0].RW) { throw 'Self-check package mount was not read-only.' }
}

function Invoke-Bootstrap {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not (New-Object Security.Principal.WindowsPrincipal($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run this bootstrap as Administrator or SYSTEM on its dedicated worker.'
    }
    $os = Get-CimInstance Win32_OperatingSystem
    $installation = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').InstallationType
    if ($os.ProductType -eq 1 -or ([version]$os.Version).Build -ne 26100 -or $installation -ne 'Server Core' -or
        -not [Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
        throw 'This bootstrap requires Windows Server 2025 Core on AMD64.'
    }
    $pins = Get-Content -LiteralPath $PinsPath -Raw | ConvertFrom-Json
    if ($pins.schema_version -ne 1 -or $pins.docker.url -notlike 'https://download.docker.com/win/static/stable/x86_64/*' -or
        $pins.python.url -notlike 'https://www.python.org/ftp/python/*' -or
        $pins.image.reference -notmatch '^mcr\.microsoft\.com/windows/servercore@sha256:[a-f0-9]{64}$') {
        throw 'Invalid pinned prerequisite manifest.'
    }
    New-Item -ItemType Directory -Path $StateDir -Force | Out-Null
    Invoke-Checked 'icacls.exe' @($StateDir, '/inheritance:r', '/grant:r', '*S-1-5-18:(OI)(CI)F', '*S-1-5-32-544:(OI)(CI)F') | Out-Null
    $statePath = Join-Path $StateDir 'state.json'
    $state = @{ reboot_count = 0 }
    if (Test-Path -LiteralPath $statePath) {
        $prior = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
        $state.reboot_count = [int]$prior.reboot_count
    }
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    # The host stays manageable through SSM. No inbound RDP, SSH, or Docker TCP.
    Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -Value 1
    if (-not (Get-NetFirewallRule -Name 'SiliconApps-DenyRemoteAdministration' -ErrorAction SilentlyContinue)) {
        New-NetFirewallRule -Name 'SiliconApps-DenyRemoteAdministration' -DisplayName 'Silicon Apps: no RDP, SSH or Docker TCP' `
            -Direction Inbound -Action Block -Protocol TCP -LocalPort 22,3389,2375,2376 -Profile Any | Out-Null
    }
    $pythonInstaller = Join-Path $StateDir ('python-' + $pins.python.version + '-amd64.exe')
    $dockerArchive = Join-Path $StateDir ('docker-' + $pins.docker.version + '.zip')
    Get-PinnedDownload $pins.python $pythonInstaller
    Get-PinnedDownload $pins.docker $dockerArchive
    $signature = Get-AuthenticodeSignature -LiteralPath $pythonInstaller
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch '(^|,\s*)O=Python Software Foundation(,|$)') {
        throw 'Python installer must have a valid Python Software Foundation Authenticode signature.'
    }
    $pythonDir = Join-Path $env:ProgramFiles 'SiliconAppsPython314'
    $python = Join-Path $pythonDir 'python.exe'
    $rebootNeeded = $false
    if (-not (Test-Path -LiteralPath $python)) {
        $installed = Invoke-Checked $pythonInstaller @('/quiet', '/norestart', 'InstallAllUsers=1', "TargetDir=$pythonDir", `
            'Include_launcher=0', 'Include_test=0', 'Include_doc=0', 'Include_tcltk=0', 'Include_tools=0', `
            'Include_pip=1', 'Shortcuts=0', 'AssociateFiles=0', 'PrependPath=0') 600 @(0,3010)
        $rebootNeeded = $installed.exit_code -eq 3010
    }
    $pythonVersion = (Invoke-Checked $python @('--version')).stdout
    if ($pythonVersion -ne ('Python ' + $pins.python.version)) { throw 'Existing Python does not match the pinned runtime.' }
    $missing = @(Get-WindowsFeature -Name Containers,Hyper-V | Where-Object { -not $_.Installed } | Select-Object -ExpandProperty Name)
    if ($missing.Count -gt 0) {
        $features = Install-WindowsFeature -Name $missing -IncludeManagementTools -Restart:$false
        if (-not $features.Success) { throw 'Windows Containers/Hyper-V features could not be enabled.' }
        $rebootNeeded = $rebootNeeded -or $features.RestartNeeded -eq 'Yes'
    }
    if ($rebootNeeded) {
        if ($state.reboot_count -ge 1) { throw 'A second reboot was requested; inspect the host instead of entering a reboot loop.' }
        $state.reboot_count = 1
        $state.phase = 'reboot_required'
        Write-Receipt $statePath $state
        Write-Output '{"status":"reboot_required","exit_code":3010}'
        exit 3010
    }
    $dockerDir = Join-Path $env:ProgramFiles 'Docker'
    $docker = Join-Path $dockerDir 'docker.exe'
    $dockerd = Join-Path $dockerDir 'dockerd.exe'
    $extracted = Join-Path $StateDir 'docker-expanded'
    Expand-Archive -LiteralPath $dockerArchive -DestinationPath $extracted -Force
    foreach ($binary in @('docker.exe','dockerd.exe')) {
        Assert-Artifact (Join-Path $extracted "docker\$binary") $pins.docker.binaries.$binary
    }
    if (Get-Service docker -ErrorAction SilentlyContinue) {
        Assert-Artifact $docker $pins.docker.binaries.'docker.exe'
        Assert-Artifact $dockerd $pins.docker.binaries.'dockerd.exe'
    } else {
        New-Item -ItemType Directory -Path $dockerDir -Force | Out-Null
        Copy-Item -Path (Join-Path $extracted 'docker\*') -Destination $dockerDir -Force
    }
    $configDir = Join-Path $env:ProgramData 'docker\config'
    New-Item -ItemType Directory -Path $configDir -Force | Out-Null
    $configPath = Join-Path $configDir 'daemon.json'
    if (Test-Path -LiteralPath $configPath) {
        $existing = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
        if (($existing.hosts -join ',') -ne 'npipe://' -or ($existing.'exec-opts' -join ',') -ne 'isolation=hyperv') {
            throw 'Existing Docker configuration differs; refusing to overwrite it or expose a TCP listener.'
        }
    } else { Write-Receipt $configPath @{ hosts = @('npipe://'); 'exec-opts' = @('isolation=hyperv'); 'log-level' = 'warn' } }
    Invoke-Checked $dockerd @('--validate', '--config-file', $configPath) | Out-Null
    if (-not (Get-Service docker -ErrorAction SilentlyContinue)) { Invoke-Checked $dockerd @('--register-service') | Out-Null }
    Set-Service docker -StartupType Automatic
    Start-Service docker
    $machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    if (($machinePath -split ';') -notcontains $dockerDir) { [Environment]::SetEnvironmentVariable('Path', "$dockerDir;$machinePath", 'Machine') }
    $env:Path = "$dockerDir;$env:Path"
    $info = $null
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        try { $info = (Invoke-Checked $docker @('info','--format','{{json .}}') 10).stdout | ConvertFrom-Json; break }
        catch { if ($attempt -eq 29) { throw }; Start-Sleep -Seconds 2 }
    }
    if ($info.OSType -ne 'windows' -or $info.ServerVersion -ne $pins.docker.version) { throw 'Docker is not the pinned Windows engine.' }
    Invoke-Checked $docker @('pull','--platform','windows/amd64',$pins.image.reference) 1800 | Out-Null
    $image = ((Invoke-Checked $docker @('image','inspect',$pins.image.reference)).stdout | ConvertFrom-Json)[0]
    if ($image.Os -ne 'windows' -or $image.Architecture -ne 'amd64' -or $image.OsVersion -ne $pins.image.os_version) {
        throw 'Pulled image does not match the pinned Windows Server 2025 AMD64 manifest.'
    }
    $fixture = Join-Path $StateDir 'fixture'
    New-Item -ItemType Directory -Path $fixture -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $fixture 'sentinel.txt') -Value 'read-only-check' -Encoding ASCII
    Invoke-Checked 'icacls.exe' @($fixture, '/grant', '*S-1-1-0:(OI)(CI)RX') | Out-Null
    $probe = @'
$ErrorActionPreference='Stop'
$user=[Security.Principal.WindowsIdentity]::GetCurrent().Name
if ($user -notmatch '\\ContainerUser$') { throw 'Unexpected container user' }
if ((Get-Content 'C:\package\sentinel.txt' -Raw).Trim() -ne 'read-only-check') { throw 'Package fixture not readable' }
$written=$false
try { Set-Content 'C:\package\sentinel.txt' 'changed' -ErrorAction Stop; $written=$true } catch {}
if ($written) { throw 'Package mount is writable' }
@{user=$user;read_only_mount=$true;architecture=$env:PROCESSOR_ARCHITECTURE}|ConvertTo-Json -Compress
'@
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($probe))
    $name = 'silicon-hyperv-check-' + [Guid]::NewGuid().ToString('N')
    try {
        Invoke-Checked $docker @('create','--name',$name,'--isolation=hyperv','--network=none','--user','ContainerUser', `
            '--cpu-count=1','--memory=1g','--mount',"type=bind,source=$fixture,target=C:\package,readonly", `
            $pins.image.reference,'powershell.exe','-NoProfile','-NonInteractive','-EncodedCommand',$encoded) | Out-Null
        $run = Invoke-Checked $docker @('start','--attach',$name) 120
        $container = ((Invoke-Checked $docker @('inspect',$name)).stdout | ConvertFrom-Json)[0]
        Assert-HyperVContainer $container $pins.image.reference
        $probeResult = $run.stdout | ConvertFrom-Json
        if (-not $probeResult.read_only_mount -or $probeResult.architecture -ne 'AMD64') { throw 'Container self-check failed.' }
    } finally { Invoke-Checked $docker @('rm','--force',$name) 30 @(0,1) | Out-Null }
    $receipt = [ordered]@{ status='ready'; verified_at=[DateTime]::UtcNow.ToString('o'); os_version=$os.Version; `
        python_path=$python; python_version=$pythonVersion; python_authenticode_thumbprint=$signature.SignerCertificate.Thumbprint; `
        docker_version=$info.ServerVersion; docker_path=$docker; image=$pins.image.reference; image_os_version=$image.OsVersion; `
        hyperv_self_check=$probeResult; isolation=$container.HostConfig.Isolation; network=$container.HostConfig.NetworkMode; `
        memory_bytes=$container.HostConfig.Memory; cpu_count=$container.HostConfig.CpuCount; reboot_count=$state.reboot_count; pins=$pins }
    Write-Receipt (Join-Path $StateDir 'verification.json') $receipt
    $state.phase='ready'
    Write-Receipt $statePath $state
    $receipt | ConvertTo-Json -Depth 20
}

if (-not $LibraryOnly) { Invoke-Bootstrap }
