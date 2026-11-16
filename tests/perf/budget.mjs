// Prueba de rapidez (P1): que ninguna versión salga más pesada sin querer.
//
// Mira el tamaño de cada trozo de la interfaz en dist/assets (después de
// `npm run build`) y el arranque medido por tests/exe/exe.mjs, y los compara
// con tests/perf/baseline.json. Falla si un trozo o el total crece más de un
// 15 %, o si el arranque tarda más de un 15 % (con 300 ms de margen, que el
// arranque varía de una vez a otra).
//
//   node tests/perf/budget.mjs            comparar
//   node tests/perf/budget.mjs --update   guardar lo de ahora como referencia
//                                         (cuando el aumento es a propósito)
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const assets = join(root, "dist", "assets");
const baselinePath = join(root, "tests", "perf", "baseline.json");
const startupPath = join(root, "tests", "exe", "out", "startup.json");
const LIMIT = 1.15;
const KB = (b) => `${(b / 1024).toFixed(0)} KB`;

if (!existsSync(assets)) {
  console.error("No hay dist/assets: ejecuta antes `npm run build`.");
  process.exit(2);
}

// «Merged-B2x9aQ.js» → «Merged.js»: el nombre sin la huella, para comparar versiones.
const chunks = {};
for (const f of readdirSync(assets)) {
  if (!/\.(js|css)$/.test(f)) continue;
  const key = f.replace(/-[A-Za-z0-9_-]{8}(?=\.(js|css)$)/, "");
  chunks[key] = (chunks[key] ?? 0) + statSync(join(assets, f)).size;
}
const total = Object.values(chunks).reduce((a, b) => a + b, 0);
const startupMs = existsSync(startupPath) ? JSON.parse(readFileSync(startupPath, "utf8")).ms : null;
const now = { total, chunks, ...(startupMs ? { startupMs } : {}) };

if (process.argv.includes("--update") || !existsSync(baselinePath)) {
  const prev = existsSync(baselinePath) ? JSON.parse(readFileSync(baselinePath, "utf8")) : {};
  const saved = { ...now, startupMs: startupMs ?? prev.startupMs ?? null, date: new Date().toISOString().slice(0, 10) };
  writeFileSync(baselinePath, JSON.stringify(saved, null, 2) + "\n");
  console.log(`Referencia guardada: ${Object.keys(chunks).length} trozos, ${KB(total)} en total${saved.startupMs ? `, arranque ${saved.startupMs} ms` : ""}.`);
  process.exit(0);
}

const base = JSON.parse(readFileSync(baselinePath, "utf8"));
const problems = [];
if (total > base.total * LIMIT) problems.push(`total: ${KB(base.total)} → ${KB(total)} (+${((total / base.total - 1) * 100).toFixed(0)} %)`);
for (const [name, size] of Object.entries(chunks)) {
  const was = base.chunks[name];
  // Los trozos pequeños nuevos no cuentan (una pantalla partida en dos); los grandes nuevos sí.
  if (was === undefined) {
    if (size > 100 * 1024) problems.push(`${name}: trozo nuevo de ${KB(size)}`);
    continue;
  }
  if (size > was * LIMIT && size - was > 5 * 1024) problems.push(`${name}: ${KB(was)} → ${KB(size)} (+${((size / was - 1) * 100).toFixed(0)} %)`);
}
if (startupMs && base.startupMs && startupMs > base.startupMs * LIMIT + 300) problems.push(`arranque: ${base.startupMs} ms → ${startupMs} ms`);

const biggest = Object.entries(chunks).sort((a, b) => b[1] - a[1]).slice(0, 5);
console.log(`Interfaz: ${KB(total)} (referencia ${KB(base.total)} del ${base.date}). Los más grandes: ${biggest.map(([n, s]) => `${n} ${KB(s)}`).join(", ")}.`);
if (startupMs) console.log(`Arranque: ${startupMs} ms (referencia ${base.startupMs ?? "sin medir"}).`);
if (problems.length) {
  console.log(`\n✗ Crece más de un 15 %:\n  ${problems.join("\n  ")}\nSi es a propósito: node tests/perf/budget.mjs --update`);
  process.exit(1);
}
console.log("✓ Dentro del presupuesto.");
