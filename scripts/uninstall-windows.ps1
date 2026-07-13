# uninstall-windows.ps1 - Manual escape-hatch uninstall for Rewind on Windows.
#
# Stops the running process, removes user data directories, runs the NSIS
# uninstaller in silent mode, and cleans up residual files.
# Idempotent: safe to run multiple times.
#
# Usage (PowerShell):
#   .\scripts\uninstall-windows.ps1
#
# This script is NOT called by the in-app uninstall button. It exists for
# users whose install is broken and can't reach the Settings page.

#Requires -Version 5.0

$ErrorActionPreference = "SilentlyContinue"

$BundleId = "com.rewind.app"
$Removed = @()

function PrintRemoved($item) {
    if ($item) { $script:Removed += $item }
}

# --- Kill running process ----------------------------------------------------

$proc = Get-Process -Name "rewind" -ErrorAction SilentlyContinue
if ($proc) {
    $proc | Stop-Process -Force
    PrintRemoved "killed running Rewind process"
    Start-Sleep -Seconds 1
}

# --- Remove user data directories --------------------------------------------

$AppData = Join-Path $env:APPDATA $BundleId
if (Test-Path $AppData) {
    Remove-Item -Recurse -Force $AppData
    PrintRemoved $AppData
}

$LocalAppData = Join-Path $env:LOCALAPPDATA $BundleId
if (Test-Path $LocalAppData) {
    Remove-Item -Recurse -Force $LocalAppData
    PrintRemoved $LocalAppData
}

# --- Run NSIS uninstaller (silent) -------------------------------------------

$Uninstaller = "C:\Program Files\Rewind\Uninstall.exe"
if (Test-Path $Uninstaller) {
    Start-Process -FilePath $Uninstaller -ArgumentList "/S" -Wait -NoNewWindow
    PrintRemoved "NSIS uninstaller executed"
}

# Also check per-user install location
$UserUninstaller = Join-Path $env:LOCALAPPDATA "Rewind\Uninstall.exe"
if (Test-Path $UserUninstaller) {
    Start-Process -FilePath $UserUninstaller -ArgumentList "/S" -Wait -NoNewWindow
    PrintRemoved "per-user NSIS uninstaller executed"
}

# Remove Program Files directory if it still exists
$ProgramFiles = "C:\Program Files\Rewind"
if (Test-Path $ProgramFiles) {
    Remove-Item -Recurse -Force $ProgramFiles
    PrintRemoved $ProgramFiles
}

# --- Remove autostart entry (registry) ---------------------------------------

$RunKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$regValue = Get-ItemProperty -Path $RunKey -Name "Rewind" -ErrorAction SilentlyContinue
if ($regValue) {
    Remove-ItemProperty -Path $RunKey -Name "Rewind"
    PrintRemoved "autostart registry entry"
}

# --- Remove Start Menu shortcut ---------------------------------------------

$StartMenuShortcut = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Rewind.lnk"
if (Test-Path $StartMenuShortcut) {
    Remove-Item -Force $StartMenuShortcut
    PrintRemoved $StartMenuShortcut
}

# --- Summary -----------------------------------------------------------------

Write-Host ""
Write-Host "Rewind uninstall complete."
if ($Removed.Count -gt 0) {
    Write-Host "Removed:"
    foreach ($item in $Removed) {
        Write-Host "  - $item"
    }
} else {
    Write-Host "Nothing to remove - Rewind was already clean."
}
