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
  # Instalador con interfaz propia: lleva dentro el instalador NSIS recién generado.
  # Comparte la carpeta target (dependencias ya compiladas y sin bloqueos del antivirus).
  $env:CARGO_TARGET_DIR = Join-Path $root 'src-tauri\target'
  Push-Location (Join-Path $root 'installer')
  try {
    cargo build --release --features custom-protocol
    if ($LASTEXITCODE) { throw "El instalador personalizado no compiló ($LASTEXITCODE)" }
  } finally {
    Pop-Location
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
  }
  & (Join-Path $PSScriptRoot 'package-release.ps1')
} finally {
  Pop-Location
}

# Comprobación: los ejecutables no deben contener el nombre del usuario que compila.
foreach ($name in 'adminops.exe', 'adminops-setup.exe') {
  $exe = Join-Path $root "src-tauri\target\release\$name"
  $text = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($exe))
  $leaks = [regex]::Matches($text, [regex]::Escape($env:USERPROFILE)).Count
  if ($leaks) { throw "$name contiene $leaks rutas de $env:USERPROFILE" }
}
Write-Host "Comprobado: los ejecutables no contienen rutas del equipo de compilación."
