// Busca claves, tokens y contraseñas pegados por error antes de que lleguen al
// repositorio. Sin dependencias: corre igual en el equipo del desarrollador
// (antes de cada commit) y en la CI (sobre todo lo que está en git).
//
//   node scripts/scan-secrets.mjs            lo que está en git (git ls-files)
//   node scripts/scan-secrets.mjs --staged   solo lo que se va a subir con este commit
//   node scripts/scan-secrets.mjs --history  todo lo que se añadió alguna vez, en cualquier rama
//
// Una línea con «secrets:ignore» se salta (por ejemplo, un ejemplo en la documentación).
import { execFileSync, spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** [nombre, patrón]. Pensados para que casi no den falsos positivos. */
export const RULES = [
  ["token de GitHub", /\bgh[pousr]_[A-Za-z0-9]{36,}\b/],
  ["token de GitHub (fine-grained)", /\bgithub_pat_[A-Za-z0-9_]{50,}\b/],
  ["clave de acceso de AWS", /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/],
  ["clave privada", /-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP |ENCRYPTED )?PRIVATE KEY(?: BLOCK)?-----/],
  ["token de Slack", /\bxox[abprs]-[A-Za-z0-9-]{20,}\b/],
  ["clave de API de Google", /\bAIza[0-9A-Za-z_-]{35}\b/],
  ["clave secreta de Stripe", /\b[sr]k_live_[0-9A-Za-z]{20,}\b/],
  ["token de Anthropic", /\bsk-ant-[A-Za-z0-9_-]{20,}\b/],
  ["clave de OpenAI", /\bsk-(?:proj-)?[A-Za-z0-9_-]{40,}\b/],
  ["JWT", /\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b/],
  ["dirección con usuario y contraseña", /\b[a-z][a-z0-9+.-]*:\/\/[^\s/:@'"]{2,}:[^\s/@'"]{4,}@[^\s/'"]+/i],
  // nombre = "valor largo y sin espacios", sin parecer una variable o un marcador.
  ["contraseña o clave escrita en el código", /\b(?:api[_-]?key|secret|passw(?:or)?d|token|auth)\w*["']?\s*[:=]\s*["'](?![^"']*(?:\$\{|\{\{|<|%|\*{3}|xxx|example|ejemplo|changeme|tu_|your|dummy|placeholder|test))[A-Za-z0-9/+=_.-]{20,}["']/i],
];

/** Archivos que no se miran: binarios, dependencias y los propios ejemplos de las pruebas. */
const SKIP = [/(^|\/)package-lock\.json$/, /(^|\/)Cargo\.lock$/, /\.(png|jpe?g|gif|webp|ico|woff2?|pdf|zip|exe|dll|wasm|icns|ttf)$/i, /(^|\/)node_modules\//, /(^|\/)target\//, /(^|\/)scripts\/scan-secrets\.(mjs|test\.ts)$/, /(^|\/)src\/lib\/safeSvg\.test\.ts$/];

export function skipped(path) {
  return SKIP.some((r) => r.test(path.replaceAll("\\", "/")));
}

/** Los hallazgos de un texto: [{ line, rule }]. Nunca devuelve el valor encontrado. */
export function scan(text) {
  const found = [];
  text.split(/\r?\n/).forEach((line, i) => {
    if (line.length > 4000 || line.includes("secrets:ignore")) return;
    for (const [rule, re] of RULES) if (re.test(line)) found.push({ line: i + 1, rule });
  });
  return found;
}

function files(staged) {
  const args = staged ? ["diff", "--cached", "--name-only", "--diff-filter=ACMR", "-z"] : ["ls-files", "-z"];
  return execFileSync("git", args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }).split("\0").filter(Boolean);
}

/** Lo que se añadió alguna vez: recorre `git log -p` y mira cada línea nueva. */
async function history() {
  const git = spawn("git", ["log", "--all", "--no-color", "-U0", "-p", "--pretty=format:commit %H"], { stdio: ["ignore", "pipe", "inherit"] });
  let commit = "";
  let path = "";
  let problems = 0;
  let n = 0;
  for await (const line of createInterface({ input: git.stdout })) {
    if (line.startsWith("commit ")) {
      commit = line.slice(7, 14);
      n++;
    } else if (line.startsWith("+++ b/")) path = line.slice(6);
    else if (line.startsWith("+") && !line.startsWith("+++") && !skipped(path)) {
      for (const f of scan(line.slice(1))) {
        problems++;
        console.error(`✕ ${commit} ${path}  posible ${f.rule}`);
      }
    }
  }
  if (problems) {
    console.error(`\n${problems} hallazgo(s) en el historial. Una clave subida alguna vez ya no es secreta: cámbiala en su servicio.`);
    process.exit(1);
  }
  console.log(`Sin claves ni secretos en el historial (${n} cambios revisados).`);
}

function main() {
  if (process.argv.includes("--history")) return history();
  const staged = process.argv.includes("--staged");
  let problems = 0;
  for (const path of files(staged)) {
    if (skipped(path)) continue;
    let text;
    try {
      // Lo que se va a subir es lo del índice, no lo que haya ahora en el disco.
      text = staged ? execFileSync("git", ["show", `:${path}`], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }) : readFileSync(path, "utf8");
    } catch {
      continue;
    }
    if (text.includes("\0")) continue;
    for (const f of scan(text)) {
      problems++;
      console.error(`✕ ${path}:${f.line}  posible ${f.rule}`);
    }
  }
  if (problems) {
    console.error(`\n${problems} hallazgo(s). Si es una clave de verdad, quítala del archivo, cámbiala en su servicio (la anterior ya no es secreta) y no la subas.\nSi es un ejemplo, añade «secrets:ignore» a esa línea.`);
    process.exit(1);
  }
  console.log(`Sin claves ni secretos en ${staged ? "lo que se va a subir" : "el repositorio"}.`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
