import { describe, expect, it } from "vitest";
import { assignCodes, pressCode, tabLetters, type TipInput } from "./keytips";

const btn = (text: string, kt?: string, side = false): TipInput => ({ text, kt, side });

describe("letras de Alt", () => {
  it("el mismo DOM da siempre las mismas letras", () => {
    const dom = [btn("Liberar espacio"), btn("Lo más grande"), btn("Analizar a fondo"), btn("Papelera")];
    expect(assignCodes(dom)).toEqual(assignCodes(dom));
    expect(assignCodes(dom)).toEqual(["L", "M", "A", "P"]);
  });

  it("las letras fijas (data-kt) mandan aunque otro botón venga antes", () => {
    const codes = assignCodes([btn("Lo más grande"), btn("Liberar espacio", "L")]);
    expect(codes[1]).toBe("L");
    expect(codes[0]).not.toBe("L");
  });

  it("si dos piden la misma letra fija, la primera se queda y la otra recibe otra", () => {
    const codes = assignCodes([btn("Renovar la IP", "R"), btn("Reiniciar", "R")]);
    expect(codes[0]).toBe("R");
    expect(codes[1]).not.toBeNull();
    expect(codes[1]).not.toBe("R");
  });

  it("acentos y eñes: «Ñandú» no rompe nada", () => {
    expect(assignCodes([btn("Árbol")])).toEqual(["A"]);
    expect(assignCodes([btn("¿?")])[0]).toMatch(/^[A-Z0-9]$/);
  });

  it("con muchos elementos de la barra lateral, dos letras y ninguna es prefijo de otra", () => {
    const names = ["Panel", "Procesos", "Programas", "Espacio", "Red", "Impresoras", "Puestos", "Dispositivos", "Historial", "Inventario", "Inicio", "Servicios", "Seguridad", "Soporte", "Ajustes"];
    const items = [btn("Liberar espacio"), btn("Papelera"), ...names.map((n) => btn(n, undefined, true))];
    const codes = assignCodes(items);
    const side = codes.slice(2);
    expect(side.every((c) => c && c.length === 2)).toBe(true);
    const all = codes.filter(Boolean) as string[];
    expect(new Set(all).size).toBe(all.length);
    for (const a of all) for (const b of all) if (a !== b) expect(b.startsWith(a)).toBe(false);
  });

  it("pulsar una letra: elige, filtra o no hace nada", () => {
    const codes = ["L", "PA", "PR", null];
    expect(pressCode(codes, "L")).toEqual({ pick: 0 });
    expect(pressCode(codes, "P")).toEqual({ typed: "P" });
    expect(pressCode(codes, "PR")).toEqual({ pick: 2 });
    expect(pressCode(codes, "Z")).toBeNull();
  });

  it("las pestañas usan su inicial si no se repite", () => {
    expect(tabLetters(["Actualizar", "Windows Update", "Instalar", "Desinstalar", "Bloatware", "Inicio"])).toEqual(["A", "W", "I", "D", "B", undefined]);
  });
});
