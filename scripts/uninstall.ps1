# Run PowerShell as Administrator.
$ErrorActionPreference = "SilentlyContinue"

Stop-Service KeyCounterService -Force
sc.exe delete KeyCounterService

Remove-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" -Name "KeyCounterCollector" -ErrorAction SilentlyContinue
[Environment]::SetEnvironmentVariable("KEYCOUNTER_CONFIG", $null, "Machine")

Remove-Item "$env:ProgramFiles\KeyCounter" -Recurse -Force -ErrorAction SilentlyContinue
Write-Host "KeyCounter service and collector registration removed."
