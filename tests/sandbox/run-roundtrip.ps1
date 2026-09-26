# Prueba de ida y vuelta del catálogo en Windows Sandbox (aislado y desechable).
# Requiere la característica "Espacio aislado de Windows" (Windows Pro/Enterprise).
# Uso:  powershell -ExecutionPolicy Bypass -File tests\sandbox\run-roundtrip.ps1 [-Exe ruta\adminops.exe]
param(
  [string]$Exe = (Join-Path $PSScriptRoot '..\..\src-tauri\target\release\adminops.exe'),
  [int]$TimeoutMinutes = 20
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path "$env:windir\System32\WindowsSandbox.exe")) {
  throw 'Windows Sandbox no está habilitado: Activar o desactivar características de Windows → "Espacio aislado de Windows".'
}
if (-not (Test-Path $Exe)) { throw "No existe $Exe. Compila antes: npm run tauri build" }

$work = Join-Path $PSScriptRoot 'work'
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $work | Out-Null
Copy-Item $Exe (Join-Path $work 'AdminOps.exe')
Copy-Item (Join-Path $PSScriptRoot 'inside.ps1') $work

$wsb = Join-Path $work 'roundtrip.wsb'
Set-Content $wsb -Encoding UTF8 -Value @"
<Configuration>
  <Networking>Disable</Networking>
  <MappedFolders>
    <MappedFolder><HostFolder>$work</HostFolder><SandboxFolder>C:\work</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
  </MappedFolders>
  <LogonCommand><Command>powershell -NoProfile -ExecutionPolicy Bypass -File C:\work\inside.ps1</Command></LogonCommand>
</Configuration>
"@
Start-Process $wsb

$done = Join-Path $work 'done.txt'
$deadline = (Get-Date).AddMinutes($TimeoutMinutes)
while (-not (Test-Path $done)) {
  if ((Get-Date) -gt $deadline) { throw 'Tiempo agotado esperando al sandbox.' }
  Start-Sleep -Seconds 5
}
$code = [int](Get-Content $done)
$r = Get-Content (Join-Path $work 'roundtrip.json') -Raw | ConvertFrom-Json
"Total $($r.total) · OK $($r.ok) · No reversibles $($r.notReversible) · Avisos $($r.warnings)"
$r.cases | Where-Object outcome -ne 'ok' | Format-Table id, outcome, detail -Wrap
exit $code
