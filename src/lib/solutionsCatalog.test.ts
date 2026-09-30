import { describe, expect, it } from "vitest";
import { BUILTIN_PREFIX, BUILTIN_SOLUTIONS, duplicateForEditing, isBuiltin, solutionForFinding } from "./solutionsCatalog";

describe("soluciones que trae AdminOps", () => {
  it("todas tienen id propio, título, problema, pasos y etiquetas", () => {
    expect(BUILTIN_SOLUTIONS.length).toBeGreaterThanOrEqual(20);
    for (const s of BUILTIN_SOLUTIONS) {
      expect(s.id.startsWith(BUILTIN_PREFIX), s.id).toBe(true);
      expect(s.title.length, s.id).toBeGreaterThan(8);
      expect(s.problem.length, s.id).toBeGreaterThan(30);
      // Pasos numerados y con sustancia.
      expect(s.solution, s.id).toMatch(/^1\./m);
      expect(s.solution.length, s.id).toBeGreaterThan(120);
      expect(s.tags.length, s.id).toBeGreaterThan(0);
    }
  });

  it("no hay ids ni títulos repetidos", () => {
    const ids = BUILTIN_SOLUTIONS.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
    const titles = BUILTIN_SOLUTIONS.map((s) => s.title.toLowerCase());
    expect(new Set(titles).size).toBe(titles.length);
  });

  it("distingue las de AdminOps de las del técnico", () => {
    expect(isBuiltin(BUILTIN_SOLUTIONS[0].id)).toBe(true);
    expect(isBuiltin("s17324")).toBe(false);
  });

  it("al duplicar queda una copia editable del técnico", () => {
    const copy = duplicateForEditing(BUILTIN_SOLUTIONS[0]);
    expect(copy.id).toBe("");
    expect(isBuiltin(copy.id)).toBe(false);
    expect(copy.solution).toBe(BUILTIN_SOLUTIONS[0].solution);
    // Las etiquetas se copian, no se comparten.
    expect(copy.tags).not.toBe(BUILTIN_SOLUTIONS[0].tags);
  });
});

describe("del hallazgo a la solución", () => {
  const find = (area: string, title: string) => solutionForFinding({ area, title });

  it("lleva cada problema típico a su solución", () => {
    expect(find("Almacenamiento", "C: con poco espacio libre (8%)")).toBe(`${BUILTIN_PREFIX}disco-c-lleno`);
    expect(find("Estabilidad", "3 pantallazo(s) azul(es) en 7 días (último: MEMORY_MANAGEMENT)")).toBe(`${BUILTIN_PREFIX}pantallazo-azul`);
    expect(find("Sistema", "Windows no está activado")).toBe(`${BUILTIN_PREFIX}activacion-windows`);
    expect(find("Rendimiento", "14 programas arrancan con Windows")).toBe(`${BUILTIN_PREFIX}equipo-lento`);
    expect(find("Hardware", "Batería al 48% de su capacidad")).toBe(`${BUILTIN_PREFIX}bateria-dura-poco`);
    expect(find("Drivers", "Realtek Audio: el dispositivo no funciona")).toBe(`${BUILTIN_PREFIX}pantallazo-azul`);
  });

  it("toda solución enlazada existe en el catálogo", () => {
    const ids = new Set(BUILTIN_SOLUTIONS.map((s) => s.id));
    const samples: [string, string][] = [
      ["Almacenamiento", "poco espacio libre"],
      ["Estabilidad", "2 pantallazos azules"],
      ["Sistema", "Windows no está activado"],
      ["Seguridad", "Firmas del antivirus con 30 días de antigüedad"],
      ["Memoria", "La prueba de memoria de Windows encontró errores"],
      ["Discos", "Samsung SSD: el disco anuncia un fallo inminente (SMART)"],
      ["", "Windows Update lleva 40 días sin instalar nada"],
    ];
    for (const [area, title] of samples) {
      const id = solutionForFinding({ area, title });
      expect(id, `${area} · ${title}`).not.toBeNull();
      expect(ids.has(id!), `${id} no está en el catálogo`).toBe(true);
    }
  });

  it("un hallazgo sin solución conocida no inventa ninguna", () => {
    expect(find("Otra cosa", "algo que nadie ha visto")).toBeNull();
  });
});
