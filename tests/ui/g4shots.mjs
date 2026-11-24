// Capturas del reproductor con pistas y de la lista de Grabar el fallo (1.4, G4), con la demo: se graba (la demo
// simula 5 s), se ve la grabación y luego la lista. Uso: CHROMIUM_PATH=... node tests/ui/g4shots.mjs
import { createServer } from "vite";
import { mkdirSync, readdirSync, readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const out = join(root, "tests", "ui", "out");
mkdirSync(out, { recursive: true });
// Qué espera cada orden (lo mismo que smoke.mjs).
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
const server = await createServer({ root, configFile: join(root, "vite.config.ts"), server: { port: 1431, strictPort: false }, logLevel: "error" });
await server.listen();
const browser = await chromium.launch(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {});
const page = await browser.newPage({ viewport: { width: 1366, height: 900 } });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
await page.addInitScript((d) => {
  window.__E2E_DEFAULTS__ = d;
  localStorage.setItem("adminops-seen-version", "1.2.9");
  localStorage.setItem("adminops.prefs", JSON.stringify({ demo: true }));
}, defaults());
await page.goto(`http://localhost:${server.config.server.port}/tests/ui/index.html`, { waitUntil: "domcontentloaded", timeout: 120000 });
await page.waitForSelector("main", { timeout: 120000 });
const go = () => page.evaluate(() => window.dispatchEvent(new CustomEvent("adminops:navigate", { detail: { page: "machine", focus: "recorder" } })));
await go();
await page.waitForTimeout(1500);
await page.screenshot({ path: join(out, "g4-antes.png") });
await page.locator("button:visible", { hasText: "Empezar a grabar" }).first().click({ timeout: 8000 });
await page.waitForTimeout(7500);
await go();
await page.waitForTimeout(1500);
await page.screenshot({ path: join(out, "g4-lista.png"), fullPage: false });
const row = page.locator("button:visible", { hasText: "Excel no responde" }).first();
if (await row.count()) await row.click();
await page.waitForTimeout(1500);
await page.screenshot({ path: join(out, "g4-reproductor.png"), fullPage: false });
// Recortar arrastrando sobre las pistas.
const area = page.locator(".cursor-crosshair").first();
await area.scrollIntoViewIfNeeded();
await page.waitForTimeout(300);
const box = await area.boundingBox();
if (box) {
  await page.mouse.move(box.x + box.width * 0.18, box.y + 10);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.4, box.y + 10, { steps: 8 });
  await page.mouse.up();
  await page.waitForTimeout(300);
}
await area.scrollIntoViewIfNeeded();
await page.screenshot({ path: join(out, "g4-recorte.png"), fullPage: false });
await browser.close();
await server.close();
console.log(errors.length ? `Errores: ${errors.join(" | ")}` : "Sin errores");
