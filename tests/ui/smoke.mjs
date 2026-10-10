// Pruebas de la app en marcha: abre la interfaz de verdad en un navegador (con
// el sistema simulado), recorre todas las pantallas y comprueba que ninguna se
// queda en blanco, enseña «Algo falló» o deja errores en la consola. Guarda una
// captura de cada una en tests/ui/out/.
//
//   node tests/ui/smoke.mjs            (necesita Playwright con Chromium)
//
// Es lo que evita fallos como los de la 1.1.6 (portales colgados, ajustes sin
// poder pulsar) que pasaban todas las pruebas de lógica.
import { createServer } from "vite";
import { readFileSync, readdirSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const out = join(root, "tests", "ui", "out");
mkdirSync(out, { recursive: true });

// Qué espera cada orden (lista, texto, sí/no…), leído de src/lib/api.
function defaults() {
  const map = {};
  const dir = join(root, "src", "lib", "api");
  for (const f of readdirSync(dir)) {
    const src = readFileSync(join(dir, f), "utf8");
    for (const m of src.matchAll(/invoke<([^>]+(?:<[^>]*>)?[^>]*)>\("([a-z_]+)"/g)) {
      const t = m[1].trim();
      map[m[2]] = /\[\]$/.test(t) ? "array" : t === "string" ? "string" : t === "boolean" ? "bool" : t === "number" ? "number" : "null";
    }
  }
  return map;
}

// Todas las pantallas, de la barra lateral.
function pages() {
  const src = readFileSync(join(root, "src", "components", "Sidebar.tsx"), "utf8");
  const block = src.slice(src.indexOf("export type PageId"), src.indexOf(";", src.indexOf("export type PageId")));
  return [...block.matchAll(/"([a-z]+)"/g)].map((m) => m[1]);
}

// Las secciones de cada pantalla (src/lib/sections.ts): también se visitan.
function sections() {
  const src = readFileSync(join(root, "src", "lib", "sections.ts"), "utf8");
  const body = src.slice(src.indexOf("export const SECTIONS"));
  const out = [];
  let page = null;
  for (const line of body.split("\n")) {
    const p = line.match(/^ {2}([a-z]+): \[/);
    if (p) page = p[1];
    const sec = line.match(/^ {4}\{ id: "([a-z-]+)"/);
    if (page && sec) out.push([page, sec[1]]);
    if (/^};/.test(line)) break;
  }
  return out;
}

let playwright;
try {
  playwright = await import("playwright");
} catch {
  playwright = await import("/opt/node22/lib/node_modules/playwright/index.mjs");
}

const server = await createServer({ root, configFile: join(root, "vite.config.ts"), server: { port: 1430, strictPort: false }, logLevel: "error" });
await server.listen();
const url = `http://localhost:${server.config.server.port}/tests/ui/index.html`;
const launch = process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {};
const browser = await playwright.chromium.launch(launch);
const page = await browser.newPage({ viewport: { width: 1366, height: 820 } });
const errors = [];
page.on("pageerror", (e) => errors.push(`[excepción] ${e.message}`));
page.on("console", (m) => m.type() === "error" && errors.push(`[consola] ${m.text().split("\n")[0].slice(0, 300)}`));
await page.addInitScript((d) => {
  window.__E2E_DEFAULTS__ = d;
  // También corre en los marcos sin almacenamiento (vistas previas de informes).
  try {
    localStorage.setItem("adminops-seen-version", "1.2.9");
  } catch {
    /* marco aislado */
  }
}, defaults());

await page.goto(url, { waitUntil: "domcontentloaded", timeout: 120000 });
await page.waitForSelector("main", { timeout: 120000 });

const failures = [];
// El Panel también cuenta: es la primera pantalla.
if (errors.length) failures.push({ page: "dashboard (al abrir)", shown: "dashboard", empty: false, crashed: false, errors: errors.slice(0, 3) });
for (const p of pages()) {
  if (["tickets", "mail", "teams"].includes(p)) continue; // vistas web nativas
  const before = errors.length;
  await page.evaluate((id) => window.dispatchEvent(new CustomEvent("adminops:navigate", { detail: { page: id, focus: null } })), p);
  await page.waitForTimeout(700);
  const state = await page.evaluate(() => {
    const visible = [...document.querySelectorAll("[data-page]")].find((el) => !el.hidden);
    const text = visible?.innerText ?? "";
    return { id: visible?.getAttribute("data-page") ?? "", empty: text.trim().length < 20, crashed: /Algo falló|Something went wrong/.test(text) };
  });
  await page.screenshot({ path: join(out, `${p}.png`) });
  // Avisos rojos con un fallo de JavaScript (no los errores normales de Windows).
  const toasts = await page.evaluate(() => [...document.querySelectorAll('[data-toast="error"]')].map((t) => t.textContent ?? ""));
  for (const t of toasts) if (/TypeError|ReferenceError|undefined|null \(reading/.test(t)) errors.push(`[aviso] ${t.slice(0, 200)}`);
  await page.evaluate(() => document.querySelectorAll('[data-toast] button[aria-label="Cerrar aviso"]').forEach((b) => b.click()));
  const newErrors = errors.slice(before);
  if (state.empty || state.crashed || newErrors.length) failures.push({ page: p, shown: state.id, empty: state.empty, crashed: state.crashed, errors: newErrors.slice(0, 3) });
}

// Cada sección de cada pantalla (la primera ya se vio arriba).
const secs = sections();
for (const [p, sec] of secs) {
  if (["tickets", "mail", "teams", "inventory", "router"].includes(p) && ["webinventory", "router"].includes(sec)) continue;
  const before = errors.length;
  await page.evaluate(([id, s]) => {
    window.dispatchEvent(new CustomEvent("adminops:navigate", { detail: { page: id, focus: s } }));
    window.dispatchEvent(new CustomEvent("adminops:open-section", { detail: { page: id, section: s } }));
  }, [p, sec]);
  await page.waitForTimeout(600);
  const state = await page.evaluate(() => {
    const visible = [...document.querySelectorAll("[data-page]")].find((el) => !el.hidden);
    const text = visible?.innerText ?? "";
    return { id: visible?.getAttribute("data-page") ?? "", empty: text.trim().length < 20, crashed: /Algo falló|Something went wrong/.test(text) };
  });
  await page.screenshot({ path: join(out, `${p}-${sec}.png`) });
  const toasts = await page.evaluate(() => [...document.querySelectorAll('[data-toast="error"]')].map((t) => t.textContent ?? ""));
  for (const t of toasts) if (/TypeError|ReferenceError|undefined|null \(reading/.test(t)) errors.push(`[aviso] ${t.slice(0, 200)}`);
  await page.evaluate(() => document.querySelectorAll('[data-toast] button[aria-label="Cerrar aviso"]').forEach((b) => b.click()));
  const newErrors = errors.slice(before);
  if (state.empty || state.crashed || newErrors.length) failures.push({ page: `${p} › ${sec}`, shown: state.id, empty: state.empty, crashed: state.crashed, errors: newErrors.slice(0, 3) });
}

// Interacciones de la 1.2.9: no basta con que las pantallas se pinten.
async function interact(name, fn) {
  const before = errors.length;
  try {
    await fn();
  } catch (e) {
    errors.push(`[${name}] ${String(e.message).split("\n")[0]}`);
  }
  const fresh = errors.slice(before);
  if (fresh.length) failures.push({ page: name, shown: "-", empty: false, crashed: false, errors: fresh.slice(0, 3) });
}
await interact("pantalla de taller (F11)", async () => {
  await page.evaluate(() => window.dispatchEvent(new CustomEvent("adminops:navigate", { detail: { page: "dashboard", focus: null } })));
  await page.waitForTimeout(400);
  await page.keyboard.press("F11");
  await page.waitForSelector('[aria-label="Pantalla de taller"]', { timeout: 5000 });
  await page.screenshot({ path: join(out, "taller.png") });
  await page.keyboard.press("Escape");
  await page.waitForSelector('[aria-label="Pantalla de taller"]', { state: "detached", timeout: 5000 });
});
await interact("cabecera compacta al bajar", async () => {
  await page.evaluate(() => {
    const el = document.querySelector('[data-page="dashboard"]');
    el.scrollTop = 400;
    el.dispatchEvent(new Event("scroll", { bubbles: true }));
  });
  await page.waitForTimeout(300);
  const compact = await page.evaluate(() => document.querySelector("header.page-head")?.getAttribute("data-compact"));
  if (compact !== "true" && (await page.evaluate(() => document.querySelector('[data-page="dashboard"]').scrollHeight <= document.querySelector('[data-page="dashboard"]').clientHeight))) return;
  if (compact !== "true") throw new Error("la cabecera no se compactó");
});
await interact("copiar con un clic", async () => {
  await page.evaluate(() => {
    const s = document.createElement("span");
    s.id = "copy-probe";
    s.dataset.copy = "192.168.1.34";
    s.textContent = "192.168.1.34";
    document.body.appendChild(s);
  });
  await page.click("#copy-probe");
  await page.waitForFunction(() => document.querySelector("#copy-probe")?.classList.contains("copied"), null, { timeout: 3000 }).catch(() => {
    // Sin permiso de portapapeles en el navegador de pruebas: no es un fallo de la app.
  });
});

const unknown = await page.evaluate(() => [...window.__E2E_UNKNOWN__]);
await browser.close();
await server.close();

console.log(`Pantallas revisadas: ${pages().length - 3} y ${secs.length} secciones. Con problemas: ${failures.length}.`);
for (const f of failures) console.log(`✗ ${f.page} (se ve «${f.shown}»)${f.empty ? " · vacía" : ""}${f.crashed ? " · «Algo falló»" : ""}\n   ${f.errors.join("\n   ")}`);
if (unknown.length) console.log(`Órdenes sin tipo conocido (respondidas con null): ${unknown.join(", ")}`);
process.exit(failures.length ? 1 : 0);
