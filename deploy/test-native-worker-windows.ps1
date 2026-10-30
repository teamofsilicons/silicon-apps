#requires -Version 5.1
<# Read-only platform checks: parse both scripts and bind actual Windows cmdlet
   parameters with WhatIf. No account, task, firewall or service is changed. #>
$ErrorActionPreference = 'Stop'
$Installer = Join-Path $PSScriptRoot 'native-worker-windows.ps1'
$Tokens = $null; $Errors = $null
$Ast = [System.Management.Automation.Language.Parser]::ParseFile($Installer, [ref]$Tokens, [ref]$Errors)
if ($Errors.Count) { throw 'Native Windows installer has PowerShell syntax errors.' }
function LiteralAssignment([string]$Name) {
    $Assignment = $Ast.Find({ param($Node)
        $Node -is [System.Management.Automation.Language.AssignmentStatementAst] -and $Node.Left.Extent.Text -eq ('$' + $Name)
    }, $false)
    $Expression = $Assignment.Right.Expression
    if ($Expression -isnot [System.Management.Automation.Language.StringConstantExpressionAst]) { throw "Expected a literal assignment for $Name." }
    return $Expression.Value
}
$Account = LiteralAssignment 'Account'
$Description = LiteralAssignment 'Description'
$Supervisor = LiteralAssignment 'Supervisor'
$null = [System.Management.Automation.Language.Parser]::ParseInput($Supervisor, [ref]$Tokens, [ref]$Errors)
if ($Errors.Count) { throw 'Native Windows supervisor has PowerShell syntax errors.' }
if ($Account.Length -gt 20 -or $Description.Length -gt 48) { throw 'Local account exceeds Windows field limits.' }
$Before = Get-LocalUser -Name $Account -ErrorAction SilentlyContinue
$TestPassword = ConvertTo-SecureString 'Nonsecret-WhatIf-only-A1!' -AsPlainText -Force
New-LocalUser -Name $Account -Description $Description -Password $TestPassword -PasswordNeverExpires -UserMayNotChangePassword -WhatIf | Out-Null
$After = Get-LocalUser -Name $Account -ErrorAction SilentlyContinue
if ([string]$Before.SID -ne [string]$After.SID) { throw 'WhatIf unexpectedly changed the local account.' }
$Settings = New-ScheduledTaskSettingsSet -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1) -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew -StartWhenAvailable
$Action = New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument '-NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File "C:\ProgramData\SiliconAppsNative\supervisor.ps1"' -WorkingDirectory 'C:\ProgramData\SiliconAppsNative'
$Trigger = New-ScheduledTaskTrigger -AtStartup
if (-not $Settings -or -not $Action -or -not $Trigger) { throw 'Task Scheduler rejected worker parameters.' }
@{ passed = $true; installer_parsed = $true; supervisor_parsed = $true; account_parameters_bound = $true; account_unchanged = $true; scheduled_task_parameters_bound = $true } | ConvertTo-Json -Compress
