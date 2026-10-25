# Compila el instalador de Windows firmado para actualizaciones y prepara release/.
# Uso (desde la carpeta del proyecto):  .\scripts\release-windows.ps1 "Notas de la versión"
param([string]$Notas = "")
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$key = Join-Path $root "firmas\cwf-updater.key"
$pwFile = Join-Path $root "firmas\firma-updater.txt"
if (!(Test-Path $key)) { throw "Falta $key" }
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ((Get-Content $pwFile | Select-String "^Contrase") -split ":\s*", 2)[1].Trim()
Push-Location $root
try {
  npm run tauri build -- --bundles nsis --config src-tauri/tauri.release.conf.json
  node scripts/latest.mjs $Notas
} finally { Pop-Location }
