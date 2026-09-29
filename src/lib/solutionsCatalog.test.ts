import { describe, expect, it } from "vitest";
import { BUILTIN_PREFIX, BUILTIN_SOLUTIONS, duplicateForEditing, isBuiltin } from "./solutionsCatalog";

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
