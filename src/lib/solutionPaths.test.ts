import { describe, expect, it } from "vitest";
import { getAreas, NAV } from "../components/Sidebar";
import { BUILTIN_SOLUTIONS as SOLUTIONS } from "./solutionsCatalog";
import guide from "./guide.ts?raw";

// Las soluciones y la guía dicen «AdminOps → Página → Pestaña → Botón». Cuando una
// página cambia de nombre o de sitio, esas rutas se quedaban apuntando a la nada.
const sources = Object.values(import.meta.glob<string>(["../pages/**/*.tsx", "../components/**/*.tsx"], { query: "?raw", import: "default", eager: true })).join("\n");
const pages = new Set([...NAV.map((n) => n.label), ...getAreas().map((a) => a.label)]);

function paths(text: string): string[][] {
  return [...text.matchAll(/AdminOps((?: → [^→)\n.,:;(`]+)+)/g)].map((m) => m[1].split(" → ").map((s) => s.trim()).filter(Boolean));
}

describe("rutas escritas en los textos", () => {
  const all = [...SOLUTIONS.flatMap((s) => paths(`${s.solution} ${s.problem ?? ""}`)), ...paths(guide)];

  it("hay rutas que comprobar", () => {
    expect(all.length).toBeGreaterThan(20);
  });

  it("empiezan por una página o un área que existe", () => {
    const bad = all.filter((p) => !pages.has(p[0]) && ![...pages].some((name) => p[0].startsWith(`${name} `)));
    expect(bad.map((p) => p.join(" → "))).toEqual([]);
  });

  it("lo que nombran dentro de la página existe en la interfaz", () => {
    // Si la ruta sigue más allá del segundo paso, el segundo es una pestaña o una sección real.
    const bad = all.filter((p) => p.length >= 3 && !sources.includes(`"${p[1]}"`) && !sources.includes(`${p[1]}\``));
    expect(bad.map((p) => p.join(" → "))).toEqual([]);
  });
});
