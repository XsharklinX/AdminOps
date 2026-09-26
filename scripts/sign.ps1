# Firma de código para `tauri build` (bundle.windows.signCommand).
# Tauri lo llama con cada binario a firmar: el .exe de la app, el instalador y el desinstalador.
#
# Sin certificado configurado NO falla: avisa y deja el binario sin firmar.
# Configura UNA de estas opciones con variables de entorno:
#   ADMINOPS_CERT_THUMBPRINT   Huella SHA1 de un certificado instalado en el almacén de Windows.
#   ADMINOPS_CERT_PFX          Ruta a un .pfx (+ ADMINOPS_CERT_PASSWORD si tiene contraseña).
# Opcional:
#   ADMINOPS_TIMESTAMP_URL     Servidor de sellado de tiempo (por defecto DigiCert).
param([Parameter(Mandatory = $true)][string]$File)
$ErrorActionPreference = 'Stop'

$thumb = $env:ADMINOPS_CERT_THUMBPRINT
$pfx = $env:ADMINOPS_CERT_PFX
if (-not $thumb -and -not $pfx) {
  Write-Host "[firma] Sin certificado configurado: $(Split-Path $File -Leaf) queda sin firmar."
  exit 0
}

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
  Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $signtool) { throw 'signtool.exe no encontrado: instala el Windows 10/11 SDK.' }

$timestamp = if ($env:ADMINOPS_TIMESTAMP_URL) { $env:ADMINOPS_TIMESTAMP_URL } else { 'http://timestamp.digicert.com' }
$signArgs = @('sign', '/fd', 'SHA256', '/tr', $timestamp, '/td', 'SHA256', '/d', 'AdminOps')
if ($thumb) {
  $signArgs += @('/sha1', $thumb)
} else {
  $signArgs += @('/f', $pfx)
  if ($env:ADMINOPS_CERT_PASSWORD) { $signArgs += @('/p', $env:ADMINOPS_CERT_PASSWORD) }
}

& $signtool @signArgs $File
if ($LASTEXITCODE) { throw "signtool devolvió $LASTEXITCODE al firmar $File" }
Write-Host "[firma] Firmado: $(Split-Path $File -Leaf)"
