# Compila la versión final y la empaqueta en release\v<versión>\.
# Uso: npm run build:release
#
# Rust incrusta en el .exe las rutas de compilación (para los mensajes de
# pánico), p. ej. C:\Users\<tu-usuario>\.cargo\... Se sustituyen por rutas
# genéricas para no distribuir datos del equipo de desarrollo.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$remap = @(
  "--remap-path-prefix=$root=adminops",
  "--remap-path-prefix=$env:USERPROFILE=~"
)
if ($env:CARGO_HOME) { $remap += "--remap-path-prefix=$env:CARGO_HOME=cargo" }
$env:RUSTFLAGS = (@($env:RUSTFLAGS) + $remap | Where-Object { $_ }) -join ' '

Push-Location $root
try {
  npx tauri build
  if ($LASTEXITCODE) { throw "tauri build falló ($LASTEXITCODE)" }
  & (Join-Path $PSScriptRoot 'package-release.ps1')
} finally {
  Pop-Location
}

# Comprobación: el ejecutable no debe contener el nombre del usuario que compila.
$exe = Join-Path $root 'src-tauri\target\release\adminops.exe'
$text = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($exe))
$leaks = [regex]::Matches($text, [regex]::Escape($env:USERPROFILE)).Count
if ($leaks) { throw "El ejecutable contiene $leaks rutas de $env:USERPROFILE" }
Write-Host "Comprobado: el ejecutable no contiene rutas del equipo de compilación."
