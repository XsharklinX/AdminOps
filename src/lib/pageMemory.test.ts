import { beforeEach, describe, expect, it } from "vitest";
import { memClear, memGet, memLabel, memRestore, memSet, memSnapshot } from "./pageMemory";

beforeEach(memClear);

describe("memoria de cada pantalla", () => {
  it("guarda por pantalla y nombre, sin mezclar pantallas", () => {
    memSet("processes", "q", "chrome");
    memSet("startup", "q", "teams");
    expect(memGet("processes", "q", "")).toBe("chrome");
    expect(memGet("startup", "q", "")).toBe("teams");
    expect(memGet("devices", "q", "nada")).toBe("nada");
  });

  it("la instantánea solo trae lo de esa pantalla y se puede devolver", () => {
    memSet("processes", "q", "chrome");
    memSet("processes", "sort", { key: "cpu", desc: true });
    memSet("startup", "q", "teams");
    const snap = memSnapshot("processes");
    expect(snap).toEqual({ q: "chrome", sort: { key: "cpu", desc: true } });
    memSet("processes", "q", "otra cosa");
    memSet("processes", "extra", 1);
    memRestore("processes", snap);
    expect(memGet("processes", "q", "")).toBe("chrome");
    expect(memGet("processes", "extra", "sin")).toBe("sin");
    expect(memGet("startup", "q", "")).toBe("teams");
  });

  it("el filtro del botón de volver es el texto corto, no «all» ni lo largo", () => {
    expect(memLabel({ filter: "Lento" })).toBe("Lento");
    expect(memLabel({ filter: "all", q: "chrome" })).toBe("chrome");
    expect(memLabel({ q: "un texto de búsqueda demasiado largo" })).toBeNull();
    expect(memLabel({ q: "  " })).toBeNull();
    expect(memLabel({})).toBeNull();
  });
});
