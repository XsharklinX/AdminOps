/// <reference types="vite/client" />
// El registro de secciones (lib/sections.ts) tiene que decir lo mismo que las
// pestañas de verdad: si una pestaña cambia de id o de nombre y el registro no,
// la barra lateral llevaría a una sección que no existe.
import { describe, expect, it } from "vitest";
import { isPageId } from "../components/Sidebar";
import { parseNavKey, navKey, SECTIONS } from "./sections";

const MERGED = Object.values(import.meta.glob("../pages/Merged.tsx", { query: "?raw", import: "default", eager: true }) as Record<string, string>)[0];
const KNOWLEDGE = Object.values(import.meta.glob("../pages/Knowledge.tsx", { query: "?raw", import: "default", eager: true }) as Record<string, string>)[0];

/** Pestañas de cada lista de Merged.tsx: [id, nombre] en orden. */
function tabsOf(text: string, constName: string): [string, string][] {
  const start = text.indexOf(`const ${constName}`);
  if (start < 0) throw new Error(`No está ${constName}`);
  const body = text.slice(start, text.indexOf("\n];", start));
  return [...body.matchAll(/id:\s*"([^"]+)",\s*\n?\s*label:\s*"([^"]+)"/g)].map((m) => [m[1], m[2]]);
}

const PAGE_OF_CONST: Record<string, string> = {
  APPS: "apps",
  NET: "router",
  INVENTORY: "inventory",
  TWEAK_TABS: "tweaks",
  PREP: "recipes",
  TOOLBOX: "tools",
  SESSION: "session",
  DATA: "data",
  IDENTITY: "users",
  DISK_TABS: "space",
  WHO: "people",
  SHARED: "printers",
  MACHINE: "machine",
};

describe("secciones de cada pantalla", () => {
  it("cada lista de pestañas de Merged.tsx está en el registro, igual", () => {
    for (const [c, page] of Object.entries(PAGE_OF_CONST)) {
      const real = tabsOf(MERGED, c);
      expect(real.length, c).toBeGreaterThan(1);
      const reg = (SECTIONS as Record<string, { id: string; label: string }[]>)[page]?.map((s) => [s.id, s.label]);
      expect(reg, page).toEqual(real);
    }
  });

  it("las de Soluciones existen en Knowledge.tsx", () => {
    for (const s of SECTIONS.knowledge ?? []) expect(KNOWLEDGE).toContain(`id: "${s.id}"`);
  });

  it("todas son de pantallas que existen y sin repetir", () => {
    for (const [page, list] of Object.entries(SECTIONS)) {
      expect(isPageId(page), page).toBe(true);
      const ids = list!.map((s) => s.id);
      expect(new Set(ids).size, page).toBe(ids.length);
    }
  });

  it("la clave de una línea va y vuelve", () => {
    expect(parseNavKey(navKey("users", "domain"))).toEqual({ page: "users", section: "domain" });
    expect(parseNavKey(navKey("processes"))).toEqual({ page: "processes", section: null });
  });
});
