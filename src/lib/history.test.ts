import { describe, expect, it } from "vitest";
import { MAX_HISTORY, pushHistory, stepHistory } from "./history";

describe("historial de pantallas", () => {
  it("guarda como mucho 50 entradas y se queda con las últimas", () => {
    let list: number[] = [];
    for (let i = 0; i < 60; i++) list = pushHistory(list, i);
    expect(list.length).toBe(MAX_HISTORY);
    expect(list[0]).toBe(10);
    expect(list[list.length - 1]).toBe(59);
  });

  it("atrás lleva a la última y deja la pantalla actual para «adelante»", () => {
    const s = stepHistory(["a", "b", "c"], [], "d", 0, () => true)!;
    expect(s.target).toBe("c");
    expect(s.from).toEqual(["a", "b"]);
    expect(s.to).toEqual(["d"]);
  });

  it("saltar con el menú deja las pantallas saltadas en su orden para ir adelante", () => {
    const s = stepHistory(["a", "b", "c"], [], "d", 2, () => true)!;
    expect(s.target).toBe("a");
    expect(s.from).toEqual([]);
    // Adelante saca del final: primero b, luego c, luego d.
    expect(s.to).toEqual(["d", "c", "b"]);
  });

  it("no abre lo que el modo usuario no permite y sigue con la anterior", () => {
    const s = stepHistory(["a", "b", "c"], [], "d", 0, (e) => e !== "c")!;
    expect(s.target).toBe("b");
    expect(s.from).toEqual(["a"]);
  });

  it("el salto del menú cuenta sobre la lista sin lo que el modo usuario no permite", () => {
    // Historial: a, X, b, X, c (las X no se enseñan). El menú muestra c, b, a: el tercero (índice 2) es «a».
    const prohibido = (e: string) => e !== "X";
    const lista = ["a", "X", "b", "X", "c"];
    expect(stepHistory(lista, [], "d", 0, prohibido)!.target).toBe("c");
    expect(stepHistory(lista, [], "d", 1, prohibido)!.target).toBe("b");
    const s = stepHistory(lista, [], "d", 2, prohibido)!;
    expect(s.target).toBe("a");
    expect(s.from).toEqual([]);
    // Pasados el final de lo permitido, no hay a dónde ir.
    expect(stepHistory(lista, [], "d", 3, prohibido)).toBeNull();
  });

  it("sin nada a donde volver, no hace nada", () => {
    expect(stepHistory<string>([], [], "x", 0, () => true)).toBeNull();
    expect(stepHistory(["a"], [], "x", 0, () => false)).toBeNull();
    expect(stepHistory(["a"], [], "x", 5, () => true)).toBeNull();
  });
});
