# Reúne en una sola carpeta todo lo que se distribuye de una versión:
#   release\v<versión>\
#     AdminOps-<versión>-Setup.exe                 instalador con interfaz propia (el normal)
#     AdminOps-<versión>-instalador-clasico.exe    instalador NSIS (admite /S para instalar en silencio)
#     AdminOps-<versión>-portable.zip              portable (AdminOps.exe + AdminOps.portable)
# Uso: npm run build:release  (compila y empaqueta)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$version = (Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
$exe = Join-Path $root 'src-tauri\target\release\adminops.exe'
$setup = Join-Path $root "src-tauri\target\release\bundle\nsis\AdminOps_${version}_x64-setup.exe"
$custom = Join-Path $root 'src-tauri\target\release\adminops-setup.exe'
foreach ($f in $exe, $setup, $custom) {
  if (-not (Test-Path $f)) { throw "No existe $f. Compila primero: npm run build:release" }
}

$out = Join-Path $root "release\v$version"
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null

# Instaladores
Copy-Item $custom (Join-Path $out "AdminOps-$version-Setup.exe")
Copy-Item $setup (Join-Path $out "AdminOps-$version-instalador-clasico.exe")

# Portable: carpeta temporal → zip
$name = "AdminOps-$version-portable"
$stage = Join-Path ([IO.Path]::GetTempPath()) "$name-$PID\$name"
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $exe (Join-Path $stage 'AdminOps.exe')
# Sensores (LibreHardwareMonitor): deben ir junto al .exe.
Copy-Item (Join-Path $root 'src-tauri\resources\lhm') (Join-Path $stage 'lhm') -Recurse
Set-Content -Path (Join-Path $stage 'AdminOps.portable') -Encoding UTF8 -Value @'
Este archivo activa el modo portable de AdminOps.
Mientras esté junto a AdminOps.exe, el diario de cambios, los análisis y los informes
se guardan en la carpeta AdminOps-data (separados por equipo), no en el PC del cliente.
'@
Compress-Archive -Path $stage -DestinationPath (Join-Path $out "$name.zip")
Remove-Item (Split-Path $stage -Parent) -Recurse -Force

# Huellas SHA-256 de todo lo que se publica. Se sube a la versión junto a los instaladores:
# AdminOps las comprueba al descargar una actualización (y se pueden comprobar a mano).
$sums = Get-ChildItem $out -File | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object {
  '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name
}
Set-Content -Path (Join-Path $out 'SHA256SUMS.txt') -Value $sums -Encoding ASCII

Write-Host ""
Write-Host "Versión $version lista en: $out"
Get-ChildItem $out | ForEach-Object { Write-Host ("  {0,-40} {1,8:N1} MB" -f $_.Name, ($_.Length / 1MB)) }
