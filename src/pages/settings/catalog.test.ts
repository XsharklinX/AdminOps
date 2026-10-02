import { describe, expect, it } from "vitest";
import { findSettings, SECTIONS, SETTINGS_INDEX } from "./catalog";

describe("buscador de ajustes", () => {
  it("cada ajuste del índice apunta a una sección que existe", () => {
    const ids = new Set(SECTIONS.map((s) => s.id));
    for (const e of SETTINGS_INDEX) expect(ids.has(e.section), e.title).toBe(true);
  });

  it("encuentra sin tildes y por palabras sueltas", () => {
    expect(findSettings("diagnostico").some((e) => e.title === "Diagnosticar al abrir AdminOps")).toBe(true);
    expect(findSettings("contraseña").some((e) => e.section === "security")).toBe(true);
    expect(findSettings("zoom portales").some((e) => e.title === "Zoom de los portales")).toBe(true);
  });

  it("sin texto no devuelve nada y lo que no existe tampoco", () => {
    expect(findSettings("")).toEqual([]);
    expect(findSettings("   ")).toEqual([]);
    expect(findSettings("xyzquenoexiste")).toEqual([]);
  });

  // Si a un ajuste se le cambia el nombre y el índice se queda con el viejo, el
  // buscador lleva a la sección pero no encuentra a dónde ir.
  it("cada ajuste del índice existe con ese título en la pantalla", () => {
    const files = import.meta.glob<string>(["../SettingsPage.tsx", "./*.tsx", "../../components/CompanyConfig.tsx", "../../components/M365.tsx", "../../components/PerfPanel.tsx"], {
      query: "?raw",
      import: "default",
      eager: true,
    });
    const source = Object.values(files).join("\n");
    expect(Object.keys(files).length).toBeGreaterThan(8);
    // title="…", title={`…`} y los títulos que se eligen entre dos («a» : «b»).
    const missing = SETTINGS_INDEX.filter((e) => !source.includes(`"${e.title}"`) && !source.includes(`\`${e.title}`)).map((e) => e.title);
    expect(missing).toEqual([]);
  });

  it("no hay ajustes repetidos en el índice", () => {
    const keys = SETTINGS_INDEX.map((e) => `${e.section}|${e.title}`);
    expect(new Set(keys).size).toBe(keys.length);
  });
});
