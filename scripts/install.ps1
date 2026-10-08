param(
    [string]$Version = "",
    [string]$Archive = "",
    [string]$Sha256 = "",
    [string]$HomeDirectory = $(if ($env:SILICON_HOME) { $env:SILICON_HOME } else { $env:USERPROFILE }),
    [string]$Server = $(if ($env:APPS_URL) { $env:APPS_URL } else { "https://apps.teamofsilicons.com" }),
    [switch]$NoStartup,
    [switch]$NoPath
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path -LiteralPath $HomeDirectory -PathType Container)) { throw "$HomeDirectory`: not a directory" }
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
$target = switch ($architecture) { "x64" { "windows-x86_64" } "x86" { "windows-i686" } "arm64" { "windows-aarch64" } default { throw "Unsupported Windows architecture: $architecture" } }
$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("apps-bootstrap-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temp | Out-Null
try {
    if (-not $Archive) {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        if (-not $Version) {
            $release = Invoke-RestMethod -Uri "https://api.github.com/repos/teamofsilicons/silicon-apps/releases/latest"
            $Version = $release.tag_name -replace '^v', ''
        }
        if ($Version -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') { throw "Version must be x.y.z" }
        $name = "apps-$Version-$target.tar.gz"
        $Archive = Join-Path $temp $name
        $url = "https://github.com/teamofsilicons/silicon-apps/releases/download/v$Version/$name"
        Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $Archive
        $checksum = Invoke-WebRequest -UseBasicParsing -Uri "$url.sha256"
        $Sha256 = ($checksum.Content.Trim() -split '\s+')[0]
    }
    if (-not (Test-Path -LiteralPath $Archive -PathType Leaf)) { throw "Archive does not exist: $Archive" }
    if ($Sha256 -notmatch '^[a-fA-F0-9]{64}$') { throw "-Archive requires a trusted 64-character -Sha256 digest" }
    if ((Get-FileHash -LiteralPath $Archive -Algorithm SHA256).Hash -ine $Sha256) { throw "Checksum mismatch. Nothing was executed or installed." }
    $Archive = (Resolve-Path -LiteralPath $Archive).Path
    $binary = Join-Path $temp "silicon-apps.exe"
    # Copy one known member as raw bytes. PowerShell text redirection would corrupt an EXE.
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = "tar.exe"
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($commandName in @('silicon-apps', 'apps')) {
        $start.Arguments = '-xOzf "' + $Archive + '" bin/' + $commandName + '.exe'
        $process = [System.Diagnostics.Process]::Start($start)
        $file = [System.IO.File]::Create($binary)
        try { $process.StandardOutput.BaseStream.CopyTo($file) } finally { $file.Dispose() }
        $errorText = $process.StandardError.ReadToEnd()
        $process.WaitForExit()
        if ($process.ExitCode -eq 0) { break }
    }
    if ($process.ExitCode -ne 0) { throw "Archive extraction failed: $errorText" }
    $appId = (& $binary --json accounts | ConvertFrom-Json).app_id
    if ($LASTEXITCODE -ne 0 -or $appId -notin @('apps', 'silicon-apps')) { throw 'Unexpected bootstrap app ID' }
    & $binary --home $HomeDirectory --server $Server install $appId --archive $Archive --sha256 $Sha256
    if ($LASTEXITCODE -ne 0) { throw "Apps installation failed with exit code $LASTEXITCODE" }
    $binDirectory = Join-Path $HomeDirectory '.apps\bin'
    $command = Join-Path $binDirectory ($commandName + '.cmd')
    if (-not $NoPath) {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ($binDirectory -notin ($userPath -split ';')) {
            [Environment]::SetEnvironmentVariable('Path', ($binDirectory + ';' + $userPath), 'User')
        }
        Write-Host 'Configured your user PATH. Open a new terminal to use the command.'
    }
    Write-Host "Run: & '$command' --help"
    if (-not $NoStartup) {
        & $command --home $HomeDirectory daemon stop | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Could not request a graceful updater stop' }
        for ($attempt = 0; $attempt -lt 60; $attempt++) {
            $status = (& $command --home $HomeDirectory --json daemon status | ConvertFrom-Json)
            if ($LASTEXITCODE -ne 0) { throw 'Could not read updater status' }
            if (-not $status.running) { break }
            Start-Sleep -Seconds 1
        }
        & $command --home $HomeDirectory daemon install
        if ($LASTEXITCODE -ne 0) { Write-Warning "$commandName is installed. Startup registration is incomplete; retry: $commandName daemon install" }
    }
    Write-Host "Check automatic updates with: $commandName daemon status"
    if ($NoStartup) { Write-Host "Startup service skipped. Enable it later with: $commandName daemon install" }
} finally {
    Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
}
