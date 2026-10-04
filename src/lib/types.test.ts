/// <reference types="vite/client" />
// Los datos que manda el programa (structs de Rust con Serialize) y cómo los
// describe la interfaz (interfaces de lib/api) tienen que coincidir campo a campo.
// Si un campo cambia de nombre en un lado y no en el otro, nada falla al compilar:
// la pantalla enseña «undefined» o deja una casilla vacía. Esta prueba lo caza.
//
// Se comparan las que se llaman igual en los dos lados. La interfaz puede no usar
// todos los campos que manda el programa, pero no puede esperar uno que no existe.
import { describe, expect, it } from "vitest";

const RUST = import.meta.glob("../../src-tauri/src/**/*.rs", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const TS = import.meta.glob("./api/*.ts", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

const camel = (s: string) => s.replace(/_([a-z0-9])/g, (_, c: string) => c.toUpperCase());

/** El cuerpo entre la llave que abre en `from` y la que la cierra. */
function block(text: string, from: number): string | null {
  const open = text.indexOf("{", from);
  if (open < 0) return null;
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    if (text[i] === "{") depth++;
    else if (text[i] === "}" && --depth === 0) return text.slice(open + 1, i);
  }
  return null;
}

/** Las líneas del nivel superior de un cuerpo (sin lo que hay dentro de otras llaves o corchetes). */
function topLevel(body: string): string[] {
  const out: string[] = [];
  let depth = 0;
  let cur = "";
  for (const ch of body) {
    if ("{[(<".includes(ch)) depth++;
    else if ("}])>".includes(ch)) depth--;
    if (ch === "\n" && depth === 0) {
      out.push(cur);
      cur = "";
    } else cur += ch;
  }
  out.push(cur);
  return out.map((l) => l.trim()).filter(Boolean);
}

interface RustStruct {
  file: string;
  fields: Set<string>;
}

/** Structs que se mandan a la interfaz, con el nombre de cada campo tal como llega. */
function rustStructs(): Map<string, RustStruct[]> {
  const found = new Map<string, RustStruct[]>();
  for (const [path, raw] of Object.entries(RUST)) {
    const text = raw.replace(/\r\n/g, "\n").replace(/^\s*\/\/[^\n]*$/gm, "");
    for (const m of text.matchAll(/((?:#\[[^\]]*\]\s*)+)pub(?:\([a-z]+\))? struct (\w+)\s*\{/g)) {
      const attrs = m[1];
      if (!/derive\([^)]*\bSerialize\b/.test(attrs)) continue;
      const body = block(text, m.index! + m[0].length - 1);
      if (body === null) continue;
      const camelCase = /rename_all\s*=\s*"camelCase"/.test(attrs);
      const fields = new Set<string>();
      let flatten = false;
      let pending = "";
      for (const line of body.split("\n").map((l) => l.trim())) {
        if (line.startsWith("#[")) {
          pending += line;
          continue;
        }
        const f = line.match(/^(?:pub(?:\([a-z]+\))?\s+)?([a-z_][a-z0-9_]*)\s*:/);
        if (f) {
          if (/flatten/.test(pending)) flatten = true;
          if (!/serde\([^)]*\bskip(?:_serializing)?\b(?!_)/.test(pending)) {
            const renamed = pending.match(/serde\([^)]*\brename\s*=\s*"([^"]+)"/);
            fields.add(renamed ? renamed[1] : camelCase ? camel(f[1]) : f[1]);
          }
        }
        if (line) pending = "";
      }
      // Con `flatten` los campos de otro struct llegan mezclados: no se puede comparar así.
      if (flatten) continue;
      const list = found.get(m[2]) ?? [];
      list.push({ file: path.replace(/^(\.\.\/)+/, ""), fields });
      found.set(m[2], list);
    }
  }
  return found;
}

/** Interfaces de lib/api con sus campos obligatorios y los opcionales. */
function tsInterfaces(): Map<string, { file: string; required: string[]; all: string[] }> {
  const found = new Map<string, { file: string; required: string[]; all: string[] }>();
  for (const [path, raw] of Object.entries(TS)) {
    const text = raw.replace(/\r\n/g, "\n").replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/[^\n]*$/gm, "");
    for (const m of text.matchAll(/export interface (\w+)(?:<[^>]*>)?(?:\s+extends\s+[^{]+)?\s*\{/g)) {
      // Las que heredan de otra traen campos que no se ven aquí: se comparan solo los suyos.
      const body = block(text, m.index! + m[0].length - 1);
      if (body === null) continue;
      const required: string[] = [];
      const all: string[] = [];
      for (const line of topLevel(body)) {
        const f = line.match(/^(?:readonly\s+)?["']?([A-Za-z_$][\w$]*)["']?(\?)?\s*:/);
        if (!f) continue;
        all.push(f[1]);
        if (!f[2]) required.push(f[1]);
      }
      found.set(m[1], { file: path.replace(/^\.\//, "src/lib/"), required, all });
    }
  }
  return found;
}

describe("tipos entre el programa y la interfaz", () => {
  const rust = rustStructs();
  const ts = tsInterfaces();
  const shared = [...ts.keys()].filter((n) => rust.has(n));

  it("encuentra los dos lados (si esto falla, cambió cómo se escriben)", () => {
    expect(rust.size).toBeGreaterThan(100);
    expect(ts.size).toBeGreaterThan(100);
    expect(shared.length).toBeGreaterThan(80);
  });

  it("la interfaz no espera ningún campo que el programa no mande", () => {
    const problems: string[] = [];
    for (const name of shared) {
      const t = ts.get(name)!;
      const candidates = rust.get(name)!;
      // Si hay varios structs con ese nombre, vale con que uno cuadre.
      const best = candidates
        .map((r) => ({ r, missing: t.required.filter((f) => !r.fields.has(f)) }))
        .sort((a, b) => a.missing.length - b.missing.length)[0];
      if (best.missing.length) problems.push(`${name} (${t.file} ↔ ${best.r.file}): ${best.missing.join(", ")}`);
    }
    expect(problems).toEqual([]);
  });
});
