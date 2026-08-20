# Run PowerShell as Administrator.
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$InstallDir = "$env:ProgramFiles\KeyCounter"
$DataDir = "$env:ProgramData\KeyCounter"

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
New-Item -ItemType Directory -Force -Path "$DataDir\data" | Out-Null

${service} = Get-Service -Name KeyCounterService -ErrorAction SilentlyContinue
if ($null -ne $service -and $service.Status -ne 'Stopped') {
	Stop-Service -Name KeyCounterService -Force -ErrorAction Stop
	$service.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(15))
}
Get-Process -Name keycounter-collector -ErrorAction SilentlyContinue | Stop-Process -Force

Copy-Item "$Root\target\release\keycounter-service.exe" "$InstallDir\" -Force
Copy-Item "$Root\target\release\keycounter-collector.exe" "$InstallDir\" -Force
Copy-Item "$Root\config\config.yaml" "$DataDir\config.yaml" -Force

# Configure both executables to use the same absolute config file.
$ConfigPath = "$DataDir\config.yaml"
[Environment]::SetEnvironmentVariable("KEYCOUNTER_CONFIG", $ConfigPath, "Machine")
$env:KEYCOUNTER_CONFIG = $ConfigPath

# Service account should not be an interactive administrator.
if ($null -eq $service) {
	sc.exe create KeyCounterService binPath= "`"$InstallDir\keycounter-service.exe`"" start= auto DisplayName= "KeyCounter Service" | Out-Null
} else {
	sc.exe config KeyCounterService binPath= "`"$InstallDir\keycounter-service.exe`"" start= auto | Out-Null
}
sc.exe description KeyCounterService "Privacy-preserving keyboard usage aggregation service."

# Start Collector when an interactive user logs in.
$RunKey = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"
New-ItemProperty -Path $RunKey -Name "KeyCounterCollector" -Value "`"$InstallDir\keycounter-collector.exe`"" -PropertyType String -Force | Out-Null

if ((Get-Service -Name KeyCounterService).Status -ne 'Running') {
	Start-Service KeyCounterService
}
Start-Process -FilePath "$InstallDir\keycounter-collector.exe" -WorkingDirectory $InstallDir
Write-Host "Installed KeyCounter. Service and collector are running."
