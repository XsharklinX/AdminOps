import { describe, expect, it } from "vitest";
import { didYouMean, editDistance, fuzzyHas, tokens } from "./palette";

describe("Ctrl+K perdona erratas", () => {
  it("mide la distancia entre palabras", () => {
    expect(editDistance("dicsos", "discos")).toBe(1);
    expect(editDistance("impresora", "impresroa")).toBe(1);
    expect(editDistance("red", "red")).toBe(0);
    expect(editDistance("abc", "xyzxyz", 2)).toBe(3);
  });
  it("encuentra palabras con una errata o a medio escribir", () => {
    expect(fuzzyHas(["discos", "salud"], "dicsos")).toBe(true);
    expect(fuzzyHas(["impresora"], "impresro")).toBe(true);
    expect(fuzzyHas(["red"], "rep")).toBe(false);
  });
  it("propone lo que se quiso decir", () => {
    const vocab = new Set(["impresora", "imprime", "pantalla", "azul", "discos"]);
    expect(didYouMean("impresra no imprme", vocab)).toBe("impresora no imprime");
    expect(didYouMean("pantalla azul", vocab)).toBeNull();
    expect(tokens("hola, mundo-x")).toEqual(["hola", "mundo"]);
  });
});
