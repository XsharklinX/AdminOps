# Mide arranque, memoria y CPU de AdminOps (objetivos de la Fase 7).
# Requiere una compilación sin manifiesto de administrador:
#   $env:ADMINOPS_ASINVOKER=1; $env:CARGO_TARGET_DIR="src-tauri/target-bench"; npx tauri build --no-bundle
# Uso: powershell -ExecutionPolicy Bypass -File scripts\bench.ps1 [-Exe ruta] [-Seconds 20] [-BrowserArgs '...']
#
# La instancia medida usa su propia carpeta de WebView2, así que sus procesos
# se identifican sin confundirlos con otra AdminOps abierta.
param(
  [string]$Exe = (Join-Path $PSScriptRoot '..\src-tauri\target-bench\release\adminops.exe'),
  [int]$Seconds = 20,
  [string]$BrowserArgs = '',
  # Coordenadas (relativas a la ventana) de un clic antes de medir, p. ej. un elemento de la barra lateral.
  [int]$ClickX = -1,
  [int]$ClickY = -1,
  [int]$AfterClickSeconds = 4,
  # Clics adicionales en secuencia: "x,y,segundos;x,y,segundos"
  [string]$ThenClicks = '',
  # Guarda una captura de la ventana medida (para comprobar que se pinta bien).
  [string]$Screenshot = '',
  # Página con la que arranca (id de la barra lateral: tools, users, install…), sin hacer clics.
  # Varias separadas por comas: pasa a la siguiente cada 5 s (p. ej. 'tickets,dashboard').
  [string]$Page = ''
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System; using System.Runtime.InteropServices;
public class BenchWin {
  public delegate bool P(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(P f, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, int d, IntPtr e);
  public struct R { public int L, T, Ri, B; }
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  public static void Click(IntPtr h, int x, int y) {
    SetForegroundWindow(h); System.Threading.Thread.Sleep(300);
    R r; GetWindowRect(h, out r); SetCursorPos(r.L + x, r.T + y);
    mouse_event(2, 0, 0, 0, IntPtr.Zero); mouse_event(4, 0, 0, 0, IntPtr.Zero);
  }
  public static IntPtr Find(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h) && GetWindowTextLength(h) > 0) { found = h; return false; } return true; }, IntPtr.Zero);
    return found;
  }
}
'@

if ($Page) { $env:ADMINOPS_START_PAGE = $Page }
$udf = Join-Path $env:TEMP "adminops-bench-$PID"
$env:WEBVIEW2_USER_DATA_FOLDER = $udf
if ($BrowserArgs) { $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $BrowserArgs }

function Members($appId) {
  $label = @{ [int]$appId = 'adminops' }
  Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object CommandLine -like "*$udf*" | ForEach-Object {
    $label[[int]$_.ProcessId] = if ($_.CommandLine -match '--type=([a-z-]+)') { "webview:$($Matches[1])" } else { 'webview:browser' }
  }
  $label
}

function Cpu($label, $secs) {
  $c0 = @{}; Get-Process -Id @($label.Keys) -ErrorAction SilentlyContinue | ForEach-Object { $c0[$_.Id] = $_.TotalProcessorTime.TotalMilliseconds }
  Start-Sleep -Seconds $secs
  $per = @{}
  Get-Process -Id @($label.Keys) -ErrorAction SilentlyContinue | ForEach-Object {
    $name = $label[$_.Id]; $per[$name] = $per[$name] + ($_.TotalProcessorTime.TotalMilliseconds - $c0[$_.Id]) * 100 / ($secs * 1000)
  }
  $per
}

$sw = [Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process $Exe -PassThru
$hwnd = [IntPtr]::Zero
while ($hwnd -eq [IntPtr]::Zero -and $sw.Elapsed.TotalSeconds -lt 30) { Start-Sleep -Milliseconds 20; $hwnd = [BenchWin]::Find($proc.Id) }
$startup = $sw.ElapsedMilliseconds
Start-Sleep -Seconds 8   # que carguen el webview y el Panel
if ($ClickX -ge 0) { [BenchWin]::Click($hwnd, $ClickX, $ClickY); Start-Sleep -Seconds $AfterClickSeconds }
foreach ($c in ($ThenClicks -split ';' | Where-Object { $_ })) {
  $x, $y, $w = $c -split ','
  [BenchWin]::Click($hwnd, [int]$x, [int]$y); Start-Sleep -Seconds ([int]$w)
}

if ($Screenshot) {
  Add-Type -AssemblyName System.Drawing
  $r = New-Object BenchWin+R; [BenchWin]::GetWindowRect($hwnd, [ref]$r) | Out-Null
  $bmp = New-Object System.Drawing.Bitmap ($r.Ri - $r.L), ($r.B - $r.T)
  $g = [System.Drawing.Graphics]::FromImage($bmp); $dc = $g.GetHdc()
  [BenchWin]::PrintWindow($hwnd, $dc, 2) | Out-Null; $g.ReleaseHdc($dc); $bmp.Save($Screenshot)
}
$label = Members $proc.Id
$procs = Get-Process -Id @($label.Keys)
$appPrivate = (Get-Process -Id $proc.Id).PrivateMemorySize64 / 1MB
$visible = Cpu $label $Seconds
[BenchWin]::ShowWindow($hwnd, 6) | Out-Null   # minimizar
Start-Sleep -Seconds 2
$minimized = Cpu $label $Seconds
Stop-Process -Id $proc.Id -Force
Start-Sleep -Seconds 1
Remove-Item $udf -Recurse -Force -ErrorAction SilentlyContinue

"Arranque hasta ventana: $startup ms"
"Procesos: $($procs.Count) (AdminOps + WebView2)"
"RAM privada: {0:N1} MB total · AdminOps {1:N1} MB" -f (($procs | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB), $appPrivate
"CPU (% de un núcleo)     Panel visible   Minimizada"
foreach ($k in ($visible.Keys | Sort-Object)) { "  {0,-22} {1,10:N1} {2,12:N1}" -f $k, $visible[$k], $minimized[$k] }
"  {0,-22} {1,10:N1} {2,12:N1}" -f 'TOTAL', ($visible.Values | Measure-Object -Sum).Sum, ($minimized.Values | Measure-Object -Sum).Sum
