// Prueba sobre el programa compilado (G2, Fase 30 del ROADMAP).
//
// La prueba de tests/ui mira la interfaz en un navegador con el sistema
// simulado. Esta arranca el AdminOps.exe de verdad, en una carpeta portable
// limpia (como un equipo donde se abre por primera vez), y se conecta a su
// WebView2 por el puerto de depuración para:
//   1. comprobar que arranca y pinta en menos de 15 s;
//   2. recorrer todas las pantallas en los dos temas, sin «Algo falló», sin
//      pantallas vacías y sin errores en la consola (capturas en tests/exe/out);
//   3. abrir un portal de prueba (un servidor que levanta la propia prueba) y
//      comprobar que carga, que se oculta al salir de Tickets y que al volver
//      conserva la sesión;
//   4. «¿Se puede pulsar?»: tras el portal, ir a Ajustes y hacer un clic de
//      ratón de verdad (no simulado dentro de la página): si algo invisible
//      tapa la ventana, el clic no llega y la prueba falla;
//   5. cerrar la ventana y comprobar que no quedan procesos.
// Son los fallos que llegaron en la 1.1.6: la capa invisible, la ventana negra
// del portable y el modo claro con el fondo negro.
//
//   node tests/exe/exe.mjs                    (el portable de release/v<versión>)
//   node tests/exe/exe.mjs --exe ruta.exe     (un .exe suelto; se copia con lhm/)
//   node tests/exe/exe.mjs --quick            (solo arranque, portal, clic y cierre)
//
// Solo consulta: no pulsa nada que cambie el equipo. El .exe de release pide
// administrador; si esta terminal no lo es, Windows pregunta una vez (UAC).
import { spawn, execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const out = join(root, "tests", "exe", "out");
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
const args = process.argv.slice(2);
const opt = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const quick = args.includes("--quick");
const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const failures = [];
const fail = (what, detail) => failures.push(`${what}${detail ? `: ${detail}` : ""}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (s) => console.log(s);

// ── 1. Preparar una carpeta portable limpia ────────────────────────────────
const work = mkdtempSync(join(tmpdir(), "adminops-exe-"));
let exe;
const given = opt("--exe");
if (given) {
  exe = join(work, "AdminOps.exe");
  cpSync(given, exe);
  const lhm = join(root, "src-tauri", "resources", "lhm");
  if (existsSync(lhm)) cpSync(lhm, join(work, "lhm"), { recursive: true });
  writeFileSync(join(work, "AdminOps.portable"), "Prueba sobre el programa compilado.\n");
} else {
  const zip = join(root, "release", `v${version}`, `AdminOps-${version}-portable.zip`);
  if (!existsSync(zip)) {
    console.error(`No está ${zip}. Compila la versión o pasa --exe <ruta>.`);
    process.exit(2);
  }
  execFileSync("tar", ["-xf", zip, "-C", work]);
  const inner = readdirSync(work).find((d) => statSync(join(work, d)).isDirectory());
  exe = join(work, inner, "AdminOps.exe");
}
const dataDir = join(dirname(exe), "AdminOps-data");
mkdirSync(dataDir, { recursive: true });
// Ajustes vacíos (todo por defecto): sin ellos, el portable «vacío» se trae los
// datos de AdminOps instalado en este equipo (storage::migrate_if_needed), con
// sus portales y ajustes como «Al cerrar, minimizar», y la prueba deja de ser limpia.
// «onboarded»: sin él sale el asistente de inicio, que tapa la ventana y deja el portal escondido.
writeFileSync(join(dataDir, "settings.json"), JSON.stringify({ onboarded: true }));

// ── 2. Portal de prueba: una página con sesión (cookie) y un contador ──────
let visits = 0;
const portal = createServer((req, res) => {
  if (req.url === "/favicon.ico") return res.writeHead(404).end();
  visits++;
  const cookie = req.headers.cookie ?? "";
  const session = /sesion=(\w+)/.exec(cookie)?.[1];
  const headers = { "content-type": "text/html; charset=utf-8" };
  if (!session) headers["set-cookie"] = `sesion=${Math.random().toString(36).slice(2, 10)}; Path=/; Max-Age=3600`;
  res.writeHead(200, headers).end(`<!doctype html><title>Portal de prueba</title>
<body style="font:16px system-ui;padding:2rem"><h1 id="estado">${session ? "Sesión: " + session : "Sin sesión"}</h1>
<p>Página del portal de prueba de AdminOps.</p></body>`);
});
await new Promise((r) => portal.listen(0, "127.0.0.1", r));
const portalUrl = `http://127.0.0.1:${portal.address().port}/`;
writeFileSync(join(dataDir, "portals.json"), JSON.stringify([{ id: "prueba", name: "Portal de prueba", url: portalUrl, extraDomains: [] }], null, 2));

// ── 3. Arrancar el .exe con el puerto de depuración de WebView2 ────────────
const conf = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const baseArgs = conf.app?.windows?.[0]?.additionalBrowserArgs ?? "";
const cdpPort = 9300 + Math.floor(Math.random() * 500);
const webviewData = join(work, "webview2");
const env = { ...process.env, WEBVIEW2_USER_DATA_FOLDER: webviewData, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `${baseArgs} --remote-debugging-port=${cdpPort}`.trim() };

const started = Date.now();
let elevatedLaunch = false;
let pid = await launch();
async function launch() {
  try {
    const child = spawn(exe, [], { env, detached: true, stdio: "ignore" });
    await new Promise((ok, ko) => {
      child.once("spawn", ok);
      child.once("error", ko);
    });
    child.unref();
    return child.pid;
  } catch (e) {
    // Pide administrador (código 740): se abre elevado con las mismas variables.
    log(`El .exe pide administrador (${e.code}); Windows va a preguntar.`);
    elevatedLaunch = true;
    const ps = `$env:WEBVIEW2_USER_DATA_FOLDER='${webviewData}'; $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='${env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS}'; (Start-Process -FilePath '${exe}' -PassThru).Id | Out-File -Encoding ascii '${join(work, "pid.txt")}'`;
    execFileSync("powershell", ["-NoProfile", "-Command", `Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '-NoProfile','-Command',"${ps.replace(/"/g, '`"')}"`]);
    return Number(readFileSync(join(work, "pid.txt"), "utf8").trim());
  }
}

// Si algo revienta a medias, se cierra igual el programa y cuenta como fallo.
process.on("uncaughtException", async (e) => {
  fail("prueba", `se cortó: ${String(e?.message ?? e).split(/\r?\n/)[0]}`);
  await finish();
});

let browser;
for (let i = 0; i < 60 && !browser; i++) {
  try {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`, { timeout: 2000 });
  } catch {
    await sleep(500);
  }
}
if (!browser) {
  fail("arranque", "la ventana de AdminOps no abrió su WebView2 en 30 s");
  await finish();
}
const findMain = () => browser.contexts().flatMap((c) => c.pages()).find((p) => /tauri\.localhost|^tauri:/.test(p.url()));
let page;
for (let i = 0; i < 40 && !page; i++) {
  page = findMain();
  if (!page) await sleep(250);
}
if (!page) {
  fail("arranque", "no aparece la página de AdminOps");
  await finish();
}
const errors = [];
page.on("pageerror", (e) => errors.push(`[excepción] ${e.message}`));
page.on("console", (m) => m.type() === "error" && errors.push(`[consola] ${m.text().split("\n")[0].slice(0, 300)}`));
try {
  await page.waitForSelector("main", { timeout: 15000 });
  const ms = Date.now() - started;
  log(`Arranque: ${(ms / 1000).toFixed(1)} s hasta pintar.`);
  // Para la prueba de rapidez (tests/perf/budget.mjs); con UAC por medio no vale.
  if (!elevatedLaunch) writeFileSync(join(out, "startup.json"), JSON.stringify({ ms }));
} catch {
  fail("arranque", "no pintó en 15 s");
}
await page.screenshot({ path: join(out, "00-arranque.png") });

// Que no salgan las novedades ni el recorrido encima de todo.
await page.evaluate((v) => {
  localStorage.setItem("adminops-seen-version", v);
}, version);
// Las novedades ya se abrieron al arrancar (versión «nueva» en un perfil limpio):
// se recarga para empezar sin ellas encima, que tapan los portales.
await page.reload({ waitUntil: "domcontentloaded" });
await page.waitForSelector("main", { timeout: 15000 }).catch(() => fail("arranque", "no volvió a pintar tras recargar"));
await sleep(500);

// ── 4. Todas las pantallas, en los dos temas ───────────────────────────────
function pages() {
  const src = readFileSync(join(root, "src", "components", "Sidebar.tsx"), "utf8");
  const block = src.slice(src.indexOf("export type PageId"), src.indexOf(";", src.indexOf("export type PageId")));
  return [...block.matchAll(/"([a-z]+)"/g)].map((m) => m[1]);
}
// Pantallas que viven como pestaña de otra (PAGE_ALIAS en Sidebar.tsx): «vault» se ve dentro de «data».
const alias = (() => {
  const src = readFileSync(join(root, "src", "components", "Sidebar.tsx"), "utf8");
  const start = src.indexOf("export const PAGE_ALIAS");
  const block = src.slice(start, src.indexOf("};", start));
  return Object.fromEntries([...block.matchAll(/^\s*([a-z]+):\s*\["([a-z]+)"/gm)].map((m) => [m[1], m[2]]));
})();
const go = (id) => page.evaluate((p) => window.dispatchEvent(new CustomEvent("adminops:navigate", { detail: { page: p, focus: null } })), id);
const shown = () =>
  page.evaluate(() => {
    const el = [...document.querySelectorAll("[data-page]")].find((e) => !e.hidden);
    const text = el?.innerText ?? "";
    const bg = getComputedStyle(document.body).backgroundColor;
    return { id: el?.getAttribute("data-page") ?? "", empty: text.trim().length < 20, crashed: /Algo falló|Something went wrong/.test(text), bg, theme: document.documentElement.dataset.theme };
  });
const luminance = (rgb) => {
  const [r, g, b] = (rgb.match(/\d+/g) ?? ["0", "0", "0"]).map(Number);
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
};

if (!quick) {
  for (const theme of ["light", "dark"]) {
    await page.evaluate((t) => localStorage.setItem("adminops.theme", t), theme);
    await page.reload({ waitUntil: "domcontentloaded" });
    await page.waitForSelector("main", { timeout: 15000 }).catch(() => fail(`tema ${theme}`, "no volvió a pintar al recargar"));
    for (const p of pages()) {
      if (["tickets", "mail", "teams"].includes(p)) continue; // portales: se prueban aparte
      const before = errors.length;
      await go(p);
      await sleep(900);
      const s = await shown();
      try {
        await page.screenshot({ path: join(out, `${theme}-${p}.png`), timeout: 10000 });
      } catch {
        fail(`${theme} › ${p}`, "la captura no termina en 10 s (la ventana no pinta)");
      }
      const lum = luminance(s.bg);
      const fresh = errors.slice(before);
      if (s.id !== (alias[p] ?? p)) fail(`${theme} › ${p}`, `se ve «${s.id}»`);
      if (s.empty) fail(`${theme} › ${p}`, "vacía");
      if (s.crashed) fail(`${theme} › ${p}`, "«Algo falló»");
      if (s.theme !== theme) fail(`${theme} › ${p}`, `el tema aplicado es «${s.theme}»`);
      if (theme === "light" && lum < 0.5) fail(`${theme} › ${p}`, `fondo oscuro en modo claro (${s.bg})`);
      if (fresh.length) fail(`${theme} › ${p}`, fresh.slice(0, 3).join(" · "));
    }
  }
}

// ── 5. Portal: carga, se oculta y conserva la sesión ───────────────────────
const portalPage = async () => {
  for (let i = 0; i < 40; i++) {
    const p = browser.contexts().flatMap((c) => c.pages()).find((x) => x.url().startsWith(portalUrl));
    if (p) return p;
    await sleep(250);
  }
  return null;
};
await go("tickets");
let pp = await portalPage();
if (!pp) {
  fail("portal", "Tickets no abrió el portal de prueba en 10 s (ver portal-sin-abrir.png)");
  await page.screenshot({ path: join(out, "portal-sin-abrir.png"), timeout: 10000 }).catch(() => {});
}
else {
  await pp.waitForSelector("#estado", { timeout: 10000 }).catch(() => fail("portal", "el portal no terminó de cargar («Abriendo…» colgado)"));
  await page.screenshot({ path: join(out, "portal.png") });
  // Que el portal no tape nada fuera de Tickets lo comprueba el clic de verdad
  // del paso 6: document.visibilityState no sirve, porque al ocultar la vista
  // nativa la página de dentro sigue diciendo «visible».
  const loads = visits;
  await go("settings");
  await sleep(800);
  await go("tickets");
  await sleep(800);
  pp = await portalPage();
  if (!pp) fail("portal", "al volver a Tickets el portal ya no está");
  else {
    // Al volver no se recarga: se enseña la misma vista, viva.
    if (visits !== loads) fail("portal", `se recargó al volver a Tickets (${visits - loads} cargas de más)`);
    // Y la sesión (la cookie) sobrevive: al recargar, el portal reconoce al usuario.
    await pp.reload({ waitUntil: "domcontentloaded" }).catch(() => {});
    const again = await pp.textContent("#estado").catch(() => "");
    if (!/Sesión/.test(again)) fail("portal", `perdió la sesión: al recargar dice «${again}»`);
  }
}

// ── 6. ¿Se puede pulsar? Un clic de ratón de verdad tras el portal ─────────
await go("settings");
await sleep(800);
const target = await page.evaluate(() => {
  const b = document.createElement("button");
  b.id = "prueba-clic";
  b.textContent = "Prueba de clic";
  Object.assign(b.style, { position: "fixed", left: "50%", top: "50%", width: "160px", height: "48px", zIndex: "2147483647" });
  b.onclick = () => (b.dataset.pulsado = "1");
  document.body.appendChild(b);
  const r = b.getBoundingClientRect();
  return { x: window.screenX + r.left + r.width / 2, y: window.screenY + r.top + r.height / 2, dpr: window.devicePixelRatio };
});
// El portal escucha también: si el clic le llega a él, es que sigue encima (el fallo de la 1.1.6).
const portalNow = await portalPage();
await portalNow?.evaluate(() => document.addEventListener("mousedown", () => (window.__clicado = true), true)).catch(() => {});
const clickAt = () => realClick(Math.round(target.x * target.dpr), Math.round(target.y * target.dpr));
const pressed = () => page.evaluate(() => document.getElementById("prueba-clic")?.dataset.pulsado === "1");
let under = clickAt();
await sleep(700);
let clicked = await pressed();
const portalGot = async () => (await portalNow?.evaluate(() => !!window.__clicado).catch(() => false)) ?? false;
if (!clicked && !(await portalGot())) {
  // A veces Windows se queda el primer clic para activar la ventana: uno más.
  under = clickAt();
  await sleep(700);
  clicked = await pressed();
}
if (!clicked) {
  if (await portalGot()) fail("¿se puede pulsar?", "el clic en Ajustes se lo llevó el portal: sigue encima aunque no se vea");
  else if (under.pid !== pid) fail("¿se puede pulsar?", `el clic cayó en otro programa (pid ${under.pid}, clase «${under.cls}»): AdminOps no estaba delante; repite sin tocar nada`);
  else fail("¿se puede pulsar?", `el clic no llegó a la página (bajo el ratón: «${under.cls}» de AdminOps)`);
}
await page.evaluate(() => document.getElementById("prueba-clic")?.remove());

function realClick(x, y) {
  // Proceso consciente de los PPP para que las coordenadas sean píxeles reales.
  const ps = `Add-Type @'
using System; using System.Runtime.InteropServices;
public class C { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, int d, IntPtr e);
[StructLayout(LayoutKind.Sequential)] public struct P { public int X; public int Y; }
[DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(P p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint f);
[DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, IntPtr e);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c); }
'@
[C]::SetProcessDPIAware() | Out-Null
$w = (Get-Process -Id ${pid} -ErrorAction SilentlyContinue).MainWindowHandle
# Windows solo deja traer una ventana al frente a quien acaba de usar el teclado: se pulsa Alt antes.
if ($w -and $w -ne [IntPtr]::Zero) { [C]::keybd_event(0x12,0,0,[IntPtr]::Zero); [C]::keybd_event(0x12,0,2,[IntPtr]::Zero); [C]::ShowWindow($w, 9) | Out-Null; [C]::SetForegroundWindow($w) | Out-Null; Start-Sleep -Milliseconds 400 }
[C]::SetCursorPos(${x}, ${y}) | Out-Null; Start-Sleep -Milliseconds 150
$p = New-Object C+P; $p.X = ${x}; $p.Y = ${y}
$h = [C]::WindowFromPoint($p); $r = [C]::GetAncestor($h, 2); $o = 0; [C]::GetWindowThreadProcessId($r, [ref]$o) | Out-Null
$sb = New-Object System.Text.StringBuilder 256; [C]::GetClassName($h, $sb, 256) | Out-Null
[C]::mouse_event(2,0,0,0,[IntPtr]::Zero); [C]::mouse_event(4,0,0,0,[IntPtr]::Zero)
"$o|$($sb.ToString())"`;
  const [p, cls] = execFileSync("powershell", ["-NoProfile", "-Command", ps], { encoding: "utf8" }).trim().split("|");
  return { pid: Number(p), cls: cls ?? "" };
}

await finish();

// ── 7. Cerrar y comprobar que no queda nada ────────────────────────────────
async function finish() {
  try {
    await browser?.close();
  } catch {
    /* ya cerrado */
  }
  if (pid) {
    try {
      execFileSync("taskkill", ["/PID", String(pid)], { stdio: "ignore" });
    } catch {
      /* sin ventana o ya cerrado */
    }
    let alive = true;
    for (let i = 0; i < 20 && alive; i++) {
      await sleep(500);
      alive = execFileSync("tasklist", ["/FI", `PID eq ${pid}`, "/NH"], { encoding: "utf8" }).includes(String(pid));
    }
    if (alive) {
      fail("cierre", "la ventana no se cerró en 10 s");
      execFileSync("taskkill", ["/F", "/T", "/PID", String(pid)], { stdio: "ignore" });
    }
    // WebView2 cierra sus procesos poco a poco (los de los portales tardan más):
    // se espera hasta 10 s antes de dar por hecho que se quedaron colgados.
    const count = () => execFileSync("powershell", ["-NoProfile", "-Command", `@(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object CommandLine -like '*${work.replace(/'/g, "''")}*').Count`], { encoding: "utf8" }).trim();
    let left = count();
    for (let i = 0; i < 10 && left !== "0"; i++) {
      await sleep(1000);
      left = count();
    }
    if (left !== "0") fail("cierre", `quedaron ${left} procesos de WebView2 abiertos 10 s después de cerrar`);
  }
  portal.close();
  log(`Portal de prueba: ${visits} visitas.`);
  try {
    rmSync(work, { recursive: true, force: true });
  } catch {
    /* algún archivo aún en uso; se queda en %TEMP% */
  }
  log(failures.length ? `\n✗ ${failures.length} fallos:\n  ${failures.join("\n  ")}` : "\n✓ Todo bien. Capturas en tests/exe/out.");
  process.exit(failures.length ? 1 : 0);
}
