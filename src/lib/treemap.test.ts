import { describe, expect, it } from "vitest";
import { squarify } from "./treemap";

const box = { x: 0, y: 0, w: 600, h: 400 };

describe("mapa del espacio", () => {
  it("cada rectángulo ocupa en proporción a su tamaño y llenan la caja", () => {
    const items = [500, 300, 100, 60, 40];
    const r = squarify(items, (v) => v, box);
    expect(r).toHaveLength(5);
    const total = items.reduce((a, b) => a + b, 0);
    for (const p of r) expect(p.w * p.h).toBeCloseTo((p.item / total) * 600 * 400, 3);
    expect(r.reduce((a, p) => a + p.w * p.h, 0)).toBeCloseTo(600 * 400, 3);
    // Nada se sale de la caja.
    for (const p of r) {
      expect(p.x).toBeGreaterThanOrEqual(-1e-6);
      expect(p.y).toBeGreaterThanOrEqual(-1e-6);
      expect(p.x + p.w).toBeLessThanOrEqual(600 + 1e-6);
      expect(p.y + p.h).toBeLessThanOrEqual(400 + 1e-6);
    }
  });

  it("sin tamaño, o sin sitio, no hay nada que pintar", () => {
    expect(squarify([0, 0], (v) => v, box)).toEqual([]);
    expect(squarify([5], (v) => v, { x: 0, y: 0, w: 0, h: 10 })).toEqual([]);
    expect(squarify([10, 0, 5], (v) => v, box)).toHaveLength(2);
  });
});
