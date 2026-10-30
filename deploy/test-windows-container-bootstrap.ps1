#Requires -Version 5.1
[CmdletBinding()]
param([string]$BootstrapPath = (Join-Path $PSScriptRoot 'windows-container-bootstrap.ps1'))
$ErrorActionPreference = 'Stop'
. $BootstrapPath -LibraryOnly
$checks = 0
function Assert-Equal($Actual, $Expected) {
    if ($Actual -cne $Expected) { throw "Expected [$Expected], got [$Actual]" }
    $script:checks++
}
function Assert-Rejected([scriptblock]$Action) {
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (-not $rejected) { throw 'Unsafe input was accepted.' }
    $script:checks++
}
Assert-Equal (ConvertTo-NativeArgument '') '""'
Assert-Equal (ConvertTo-NativeArgument 'simple') 'simple'
Assert-Equal (ConvertTo-NativeArgument 'C:\Program Files\Docker') '"C:\Program Files\Docker"'
Assert-Equal (ConvertTo-NativeArgument 'a"b') '"a\"b"'
Assert-Equal (ConvertTo-NativeArgument 'a b\') '"a b\\"'
$image = 'mcr.microsoft.com/windows/servercore@sha256:' + ('a' * 64)
$fixture = '{"HostConfig":{"Isolation":"hyperv","NetworkMode":"none","CpuCount":1,"Memory":1073741824},"Config":{"User":"ContainerUser","Image":"IMAGE"},"State":{"ExitCode":0},"Mounts":[{"Destination":"C:\\package","RW":false}]}'
$good = $fixture.Replace('IMAGE', $image) | ConvertFrom-Json
Assert-HyperVContainer $good $image
$checks++
foreach ($change in @('process','network','cpu','memory','user','image','exit','mount')) {
    $bad = $fixture.Replace('IMAGE', $image) | ConvertFrom-Json
    switch ($change) {
        'process' { $bad.HostConfig.Isolation = 'process' }
        'network' { $bad.HostConfig.NetworkMode = 'nat' }
        'cpu' { $bad.HostConfig.CpuCount = 2 }
        'memory' { $bad.HostConfig.Memory = 0 }
        'user' { $bad.Config.User = 'ContainerAdministrator' }
        'image' { $bad.Config.Image = 'mcr.microsoft.com/windows/servercore:ltsc2025' }
        'exit' { $bad.State.ExitCode = 1 }
        'mount' { $bad.Mounts[0].RW = $true }
    }
    Assert-Rejected { Assert-HyperVContainer $bad $image }
}
$temporary = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $temporary | Out-Null
    $file = Join-Path $temporary 'artifact.txt'
    [IO.File]::WriteAllText($file, 'verified')
    $hash = (Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant()
    Assert-Artifact $file $hash
    $checks++
    Assert-Rejected { Assert-Artifact $file ('0' * 64) }
    $receipt = Join-Path $temporary 'receipt.json'
    Write-Receipt $receipt @{ status='ready'; nested=@{ verified=$true } }
    Assert-Equal ((Get-Content $receipt -Raw | ConvertFrom-Json).status) 'ready'
    Assert-Equal (Test-Path ($receipt + '.tmp')) $false
    Assert-Equal (Invoke-Checked 'cmd.exe' @('/d','/c','exit','7') 10 @(7)).exit_code 7
    Assert-Rejected { Invoke-Checked 'cmd.exe' @('/d','/c','exit','7') 10 | Out-Null }
    Assert-Rejected { Invoke-Checked (Join-Path $PSHOME 'powershell.exe') @('-NoProfile','-Command','Start-Sleep -Seconds 10') 1 | Out-Null }
} finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
@{passed=$true;checks=$checks} | ConvertTo-Json -Compress
