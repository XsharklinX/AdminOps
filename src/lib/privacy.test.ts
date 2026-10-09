import { describe, expect, it } from "vitest";
import { findSensitive } from "./privacy";

const hidden = (text: string) => findSensitive(text).map(([a, b]) => text.slice(a, b));

describe("modo privacidad", () => {
  it("tapa correos, IP, MAC, rutas de red y carpetas de usuario", () => {
    expect(hidden("Escribe a maria.perez@empresa.com o llama")).toEqual(["maria.perez@empresa.com"]);
    expect(hidden("IP 192.168.10.42 y 10.0.0.1")).toEqual(["192.168.10.42", "10.0.0.1"]);
    expect(hidden("MAC 00:1A:2B:3C:4D:5E")).toEqual(["00:1A:2B:3C:4D:5E"]);
    expect(hidden(String.raw`Carpeta \\SRV-ARCHIVO\Contabilidad\2024 lista`)).toEqual([String.raw`\\SRV-ARCHIVO\Contabilidad\2024`]);
    expect(hidden(String.raw`En C:\Users\maria.perez\Desktop hay`)).toEqual([String.raw`C:\Users\maria.perez`]);
  });

  it("tapa las claves de recuperación de BitLocker", () => {
    expect(hidden("123456-234567-345678-456789-567890-678901-789012-890123")).toHaveLength(1);
  });

  it("deja el texto normal en paz", () => {
    expect(findSensitive("Windows 11 Pro versión 23H2, 16 GB de memoria")).toEqual([]);
    expect(findSensitive("")).toEqual([]);
  });

  it("une los solapes", () => {
    // Un correo cuyo dominio parece una IP: se tapa una sola vez.
    const r = findSensitive("a@10.0.0.1");
    expect(r).toEqual([[0, 10]]);
  });
});
