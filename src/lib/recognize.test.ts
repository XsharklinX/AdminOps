import { describe, expect, it } from "vitest";
import { recognize } from "./recognize";

describe("reconocer lo que se escribe en Ctrl+K", () => {
  it("equipos", () => {
    expect(recognize("PC-CONTA-03")).toEqual({ kind: "computer", value: "PC-CONTA-03", sure: true });
    expect(recognize("pc-conta-03")?.value).toBe("PC-CONTA-03");
    expect(recognize("LAP_RRHH2")?.kind).toBe("computer");
  });

  it("personas por su usuario: seguro", () => {
    expect(recognize("maria.perez")).toEqual({ kind: "person", value: "maria.perez", sure: true });
    expect(recognize("EMPRESA\\maria.perez")).toEqual({ kind: "person", value: "maria.perez", sure: true });
  });

  it("extensiones, IP y tickets", () => {
    expect(recognize("4512")).toEqual({ kind: "extension", value: "4512", sure: true });
    expect(recognize("192.168.10.45")?.kind).toBe("ip");
    expect(recognize("#4521")?.kind).toBe("ticket");
  });

  it("impresoras", () => {
    expect(recognize("impresora recepción")?.kind).toBe("printer");
    expect(recognize("HP LaserJet")?.kind).toBe("printer");
  });

  /**
   * Dos palabras pueden ser un nombre o una búsqueda normal: se reconoce como
   * persona, pero «dudoso», para ofrecerlo al final y no tapar «Windows Update».
   */
  it("dos palabras sueltas son dudosas", () => {
    expect(recognize("María Pérez")).toEqual({ kind: "person", value: "María Pérez", sure: false });
    expect(recognize("windows update")?.sure).toBe(false);
  });

  it("no adivina de más", () => {
    expect(recognize("red")).toBeNull();
    expect(recognize("espacio")).toBeNull();
    expect(recognize("999.1.1.1")).toBeNull();
    expect(recognize("123456789")).toBeNull();
    expect(recognize("x")).toBeNull();
  });
});
