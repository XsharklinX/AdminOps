import { describe, expect, it } from "vitest";
import { matchOffsets, plain, step } from "./pageFind";

describe("buscar en la pantalla", () => {
  it("no distingue mayúsculas ni tildes y conserva las posiciones", () => {
    expect(plain("Configuración")).toBe("configuracion");
    expect(plain("Configuración")).toHaveLength("Configuración".length);
    expect(matchOffsets("Cola de impresión · IMPRESORA", "impres")).toEqual([8, 20]);
    expect(matchOffsets("Árbol arbol ÁRBOL", "arbol")).toEqual([0, 6, 12]);
  });

  it("sin texto, nada; y respeta el tope", () => {
    expect(matchOffsets("algo", "  ")).toEqual([]);
    expect(matchOffsets("aaaa", "a", 2)).toEqual([0, 1]);
    expect(matchOffsets("spooler spooler", "spool")).toEqual([0, 8]);
  });

  it("salta de una coincidencia a otra dando la vuelta", () => {
    expect(step(0, 3, 1)).toBe(1);
    expect(step(2, 3, 1)).toBe(0);
    expect(step(0, 3, -1)).toBe(2);
    expect(step(0, 0, 1)).toBe(0);
  });
});
