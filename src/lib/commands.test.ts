/// <reference types="vite/client" />
// Cada `invoke("x")` de la interfaz tiene que existir en `generate_handler![…]`
// de src-tauri/src/lib.rs. Si no, el fallo no aparece al compilar: aparece el día
// que un técnico pulsa ese botón y recibe «command x not found». Ya pasó dos
// veces; por eso esto es una prueba y no una comprobación a mano.
//
// Los archivos se leen con el propio empaquetador (`?raw`), sin depender de que
// la prueba corra en Node ni de la ruta desde la que se lance.
import { describe, expect, it } from "vitest";

/** Todo el código de la interfaz, por ruta. */
const SOURCES = import.meta.glob("../**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
/** El registro de comandos del programa. */
const LIB = Object.values(import.meta.glob("../../src-tauri/src/lib.rs", { query: "?raw", import: "default", eager: true }) as Record<string, string>)[0];

/** Comandos que la interfaz llama, con el archivo donde se llaman. */
function invoked(): Map<string, string> {
  const found = new Map<string, string>();
  for (const [path, text] of Object.entries(SOURCES)) {
    if (path.endsWith(".test.ts")) continue;
    for (const m of text.matchAll(/\binvoke(?:<[^>]*>)?\(\s*["'`]([a-z0-9_]+)["'`]/g)) {
      if (!found.has(m[1])) found.set(m[1], path.replace(/^\.\.\//, "src/"));
    }
  }
  return found;
}

/** Comandos que el programa registra, tal y como los recibe el despachador. */
function registered(): string[] {
  const block = LIB?.split("generate_handler![")[1]?.split("])")[0];
  if (!block) throw new Error("No se encontró generate_handler![…] en lib.rs");
  return block
    .split(",")
    .map((x) => x.trim())
    .filter((x) => x && !x.startsWith("//"))
    .map((x) => x.split("::").pop() as string);
}

describe("comandos entre la interfaz y el programa", () => {
  it("lee las dos listas (si esto falla, el formato de lib.rs cambió)", () => {
    expect(registered().length).toBeGreaterThan(100);
    expect(invoked().size).toBeGreaterThan(100);
  });

  it("todo comando que llama la interfaz está registrado en lib.rs", () => {
    const reg = new Set(registered());
    const faltan = [...invoked()].filter(([cmd]) => !reg.has(cmd)).map(([cmd, file]) => `${cmd} (${file})`);
    expect(faltan, "Comandos que la interfaz llama y el programa no registra").toEqual([]);
  });

  it("no hay comandos registrados dos veces", () => {
    const reg = registered();
    const repetidos = reg.filter((c, i) => reg.indexOf(c) !== i);
    expect(repetidos, "Comandos repetidos en generate_handler!").toEqual([]);
  });
});
