# Cambia la versión de AdminOps en todos los sitios a la vez.
# Uso: npm run setversion 1.1.5
$ErrorActionPreference = 'Stop'
$v = $args[0]
if (-not $v) { throw 'Falta la versión. Uso: npm run setversion 1.1.5' }
if ($v -notmatch '^\d+\.\d+\.\d+$') { throw "«$v» no es una versión válida (ejemplo: 1.1.5)." }
$root = Split-Path $PSScriptRoot -Parent

function Replace-Version($path, $pattern, $replacement) {
  $full = Join-Path $root $path
  $text = [IO.File]::ReadAllText($full)
  if ($text -notmatch $pattern) { throw "No se encontró la versión en $path" }
  [IO.File]::WriteAllText($full, [regex]::Replace($text, $pattern, $replacement, 1))
}

Replace-Version 'package.json'            '"version": "\d+\.\d+\.\d+"'  "`"version`": `"$v`""
Replace-Version 'src-tauri/tauri.conf.json' '"version": "\d+\.\d+\.\d+"' "`"version`": `"$v`""
Replace-Version 'src-tauri/Cargo.toml'    '(?m)^version = "\d+\.\d+\.\d+"' "version = `"$v`""

# Cargo.lock se actualiza solo con una comprobación rápida.
Push-Location (Join-Path $root 'src-tauri')
try { cargo metadata --format-version 1 --offline > $null } finally { Pop-Location }

Write-Host "Versión $v puesta en package.json, tauri.conf.json, Cargo.toml y Cargo.lock."
