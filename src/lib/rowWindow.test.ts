import { describe, expect, it } from "vitest";
import { rowRange } from "./rowWindow";

describe("rowRange", () => {
  it("al principio pinta las primeras filas y un margen", () => {
    expect(rowRange(0, 340, 34, 5000, 12)).toEqual({ start: 0, end: 22 });
  });
  it("a mitad de lista pinta lo que se ve con margen a los dos lados", () => {
    const r = rowRange(34 * 1000, 340, 34, 5000, 12);
    expect(r).toEqual({ start: 988, end: 1022 });
  });
  it("al final no pasa de la última fila", () => {
    const r = rowRange(34 * 4990, 340, 34, 5000, 12);
    expect(r.end).toBe(5000);
    expect(r.start).toBeLessThan(5000);
  });
  it("una lista vacía no pinta nada y un desplazamiento negativo cuenta como cero", () => {
    expect(rowRange(0, 340, 34, 0)).toEqual({ start: 0, end: 0 });
    expect(rowRange(-50, 340, 34, 100, 0).start).toBe(0);
  });
  it("siempre hay al menos una fila si la lista no está vacía", () => {
    const r = rowRange(1e9, 340, 34, 10);
    expect(r.end - r.start).toBeGreaterThanOrEqual(1);
    expect(r.end).toBeLessThanOrEqual(10);
  });
});
