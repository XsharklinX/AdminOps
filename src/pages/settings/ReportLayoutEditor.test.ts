import { describe, expect, it } from "vitest";
import workflow from "../../../src-tauri/src/workflow.rs?raw";
import { getAreas, isPageId, NAV, OUTSIDE_AREAS, resolvePage } from "../../components/Sidebar";
import { DEFAULT_LAYOUT, layoutOf, moveSection, REPORT_SECTIONS } from "./ReportLayoutEditor";

describe("plantilla de informe propia", () => {
  it("las secciones son las mismas, y en el mismo orden, que conoce el programa", () => {
    const m = /REPORT_SECTIONS: \[&str; \d+\] = \[([^\]]+)\]/.exec(workflow);
    const rust = m![1].split(",").map((s) => s.trim().replace(/"/g, ""));
    expect(REPORT_SECTIONS.map((s) => s.id)).toEqual(rust);
  });

  it("subir y bajar una sección no pierde ninguna ni se sale de la lista", () => {
    const list = ["a", "b", "c"];
    expect(moveSection(list, "b", -1)).toEqual(["b", "a", "c"]);
    expect(moveSection(list, "b", 1)).toEqual(["a", "c", "b"]);
    expect(moveSection(list, "a", -1)).toBe(list);
    expect(moveSection(list, "c", 1)).toBe(list);
    expect(moveSection(list, "x", 1)).toBe(list);
  });

  it("con ajustes de una versión anterior se usa la de fábrica", () => {
    expect(layoutOf(null)).toBe(DEFAULT_LAYOUT);
    expect(layoutOf({ reportLayout: { name: "Corta", technical: true, sections: ["notes"] } }).name).toBe("Corta");
  });
});

describe("barra lateral", () => {
  it("las áreas de 1.2, en su orden; Teams, Correo, Herramientas y Ajustes van fuera", () => {
    const areas = getAreas();
    expect(areas.map((a) => [a.id, a.label])).toEqual([
      ["panel", "Inicio"],
      ["equipo", "Este equipo"],
      ["red", "Red"],
      ["programas", "Programas"],
      ["admin", "Administración"],
      ["soporte", "Soporte"],
    ]);
    expect(areas.find((a) => a.id === "soporte")!.pages).toEqual(["agenda", "tickets", "inventory", "people", "contacts", "knowledge"]);
    expect(areas.find((a) => a.id === "admin")!.pages).toEqual(["stations", "users", "printers", "remote", "recipes"]);
    const inAreas = new Set(areas.flatMap((a) => a.pages));
    for (const p of OUTSIDE_AREAS) expect(inAreas.has(p), p).toBe(false);
    // Ninguna pantalla se queda sin sitio: o en un área, o fuera a propósito.
    for (const n of NAV) expect(inAreas.has(n.id) || OUTSIDE_AREAS.includes(n.id), n.id).toBe(true);
  });

  it("cada página de la barra existe, y los enlaces antiguos siguen llevando a algún sitio", () => {
    for (const a of getAreas()) for (const p of a.pages) expect(isPageId(p), p).toBe(true);
    expect(resolvePage("inventory")).toEqual(["inventory", null]);
    expect(resolvePage("devices")).toEqual(["router", "devices"]);
    expect(resolvePage("nettools")).toEqual(["router", "tools"]);
  });
});
