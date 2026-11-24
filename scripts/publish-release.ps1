# Publica en GitHub la versión ya compilada (npm run build:release) y deja el código en el repositorio.
# Uso: npm run publish:release
#
# 1. Comprueba que release\v<versión>\ tiene los cuatro archivos y que existen las notas
#    docs\notas-<versión>.md (español e inglés, 500 caracteres como mucho cada uno).
# 2. Busca secretos (scripts/scan-secrets.mjs) y hace el commit «Versión <versión>» con todo lo pendiente.
# 3. Sube la rama y crea la versión v<versión> en GitHub con los instaladores, el portable y
#    SHA256SUMS.txt. La app (Ajustes → Buscar actualizaciones, o sola al abrir) la encuentra ahí:
#    descarga AdminOps-<versión>-Setup.exe (o el .zip en la portable) y comprueba su huella.
#
# No compila nada. Si la versión ya está publicada, para sin tocar nada.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
  $version = (Get-Content (Join-Path $root 'package.json') -Raw -Encoding UTF8 | ConvertFrom-Json).version
  $tag = "v$version"
  $dir = Join-Path $root "release\$tag"
  $files = @("AdminOps-$version-Setup.exe", "AdminOps-$version-instalador-clasico.exe", "AdminOps-$version-portable.zip", 'SHA256SUMS.txt') |
    ForEach-Object { Join-Path $dir $_ }
  foreach ($f in $files) {
    if (-not (Test-Path $f)) { throw "Falta $f. Compila antes con npm run build:release." }
  }
  $notes = Join-Path $root "docs\notas-$version.md"
  if (-not (Test-Path $notes)) { throw "Faltan las notas de la versión: $notes" }

  # De aquí en adelante, cada orden se comprueba por su código de salida. En Windows PowerShell 5.1, con 'Stop',
  # cualquier cosa que gh o git escriban en la salida de error («release not found» de una versión nueva, los
  # avisos de saltos de línea de git add, el «To github.com…» de git push) cortaba el script.
  $ErrorActionPreference = 'Continue'
  gh release view $tag *> $null
  if ($LASTEXITCODE -eq 0) { throw "La versión $tag ya está publicada en GitHub. Sube la versión con npm run setversion." }

  node scripts/scan-secrets.mjs
  if ($LASTEXITCODE) { throw 'La búsqueda de secretos encontró algo: revisa antes de publicar.' }

  git add -A
  git diff --cached --quiet
  if ($LASTEXITCODE) {
    git commit -m "Versión $version" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
    if ($LASTEXITCODE) { throw 'El commit falló.' }
  }
  $branch = git branch --show-current
  git push origin $branch
  if ($LASTEXITCODE) { throw 'No se pudo subir la rama.' }
  $sha = git rev-parse HEAD

  gh release create $tag @files --title "AdminOps $version" --notes-file $notes --target $sha
  if ($LASTEXITCODE) { throw 'No se pudo crear la versión en GitHub.' }
  Write-Host "Publicada AdminOps $version ($tag) desde $branch."
} finally {
  Pop-Location
}
