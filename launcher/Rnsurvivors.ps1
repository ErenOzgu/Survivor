# Rnsurvivors launcher.
# Asks once for the WoW 1.12.1 "Data" folder (the one with dbc.MPQ), remembers it in
# %APPDATA%\Rnsurvivors\ayarlar.txt, then starts the game. Run it again with -Ayarla to pick
# another folder. Started by Rnsurvivors.bat in the repository root.
param([switch]$Ayarla)

$ErrorActionPreference = 'Stop'
$dir = Join-Path $env:APPDATA 'Rnsurvivors'
$settings = Join-Path $dir 'ayarlar.txt'
New-Item -ItemType Directory -Force -Path $dir | Out-Null

function Get-Setting([string]$key) {
    if (Test-Path $settings) {
        foreach ($line in Get-Content $settings -Encoding UTF8) {
            if ($line -match "^\s*$key\s*=\s*(.*)$") { return $Matches[1].Trim() }
        }
    }
    return $null
}

function Test-Data([string]$path) {
    return $path -and (Test-Path (Join-Path $path 'dbc.MPQ'))
}

$data = Get-Setting 'wow_data'
if ($Ayarla -or -not (Test-Data $data)) {
    Add-Type -AssemblyName System.Windows.Forms
    $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
    $dialog.Description = 'WoW 1.12.1 "Data" klasörünü seç (içinde dbc.MPQ olan klasör)'
    if ($dialog.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) { exit 1 }
    $data = $dialog.SelectedPath
    if (-not (Test-Data $data)) {
        [System.Windows.Forms.MessageBox]::Show(
            "Bu klasörde dbc.MPQ yok:`n$data`n`nWoW kurulumunun içindeki Data klasörünü seç.",
            'Rnsurvivors') | Out-Null
        exit 1
    }
    Set-Content -Path $settings -Value "wow_data=$data" -Encoding UTF8
}

$env:WOW_DATA = $data
$root = Split-Path -Parent $PSScriptRoot
$exe = @(
    (Join-Path $PSScriptRoot 'rnsurvivors.exe'),
    (Join-Path $root 'rnsurvivors.exe'),
    (Join-Path $root 'target\release\rnsurvivors.exe')
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if ($exe) {
    Push-Location (Split-Path -Parent $exe)
    try { & $exe } finally { Pop-Location }
} else {
    # Not built yet: build and run from the repository (the first build takes a while).
    Push-Location $root
    try { cargo run --release -p rnsurvivors } finally { Pop-Location }
}
