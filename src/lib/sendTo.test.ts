import { describe, expect, it } from "vitest";
import { appendNote, tableToTsv } from "./sendTo";

describe("Enviar a…", () => {
  it("convierte una tabla para Excel", () => {
    expect(tableToTsv([["Equipo", "Disco"], ["PC-01", "SSD\t256"]])).toBe("Equipo\tDisco\nPC-01\tSSD 256");
  });
  it("añade a las notas del caso", () => {
    expect(appendNote("", "Discos", "Bien")).toBe("Discos\nBien");
    expect(appendNote("Llamó María\n", "Discos", "Bien")).toBe("Llamó María\n\nDiscos\nBien");
  });
});
