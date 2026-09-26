# Empaqueta la versión portable: adminops.exe + marcador AdminOps.portable en un .zip.
# Uso: npm run build:portable  (compila y empaqueta)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$version = (Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
$exe = Join-Path $root 'src-tauri\target\release\adminops.exe'
if (-not (Test-Path $exe)) { throw "No existe $exe. Ejecuta primero: npm run tauri build" }

$name = "AdminOps-$version-portable"
$out = Join-Path $root 'dist-portable'
$stage = Join-Path $out $name
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $exe (Join-Path $stage 'AdminOps.exe')
Set-Content -Path (Join-Path $stage 'AdminOps.portable') -Encoding UTF8 -Value @'
Este archivo activa el modo portable de AdminOps.
Mientras esté junto a AdminOps.exe, el diario de cambios, los análisis y los informes
se guardan en la carpeta AdminOps-data (separados por equipo), no en el PC del cliente.
'@

$zip = Join-Path $out "$name.zip"
Remove-Item $zip -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "Portable listo: $zip"
