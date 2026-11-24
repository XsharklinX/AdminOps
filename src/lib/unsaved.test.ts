import { describe, expect, it } from "vitest";
import type { UnsavedFile } from "./api";
import { agoText, canOpenCopy, rowLine, sortUnsaved, versionsSentence, whenText } from "./unsaved";

const file = (p: Partial<UnsavedFile> = {}): UnsavedFile => ({
  path: "C:\\x\\a.xlsb",
  name: "Presupuesto 2027.xlsx",
  app: "Excel",
  origin: "Autorrecuperación",
  size: 1000,
  modified: 0,
  ext: "xlsb",
  copyExt: "xlsx",
  ...p,
});

describe("lo que no se guardó de Office", () => {
  // 21 nov 2026, 12:10 (hora local de la máquina de pruebas).
  const now = new Date(2026, 10, 21, 12, 10, 0).getTime();
  const at = (d: Date) => Math.floor(d.getTime() / 1000);

  it("la hora se dice como «hoy 11:52», «ayer 18:03» o con la fecha", () => {
    expect(whenText(at(new Date(2026, 10, 21, 11, 52)), now)).toMatch(/^hoy 11:52/);
    expect(whenText(at(new Date(2026, 10, 20, 18, 3)), now)).toMatch(/^ayer 18:03/);
    expect(whenText(at(new Date(2026, 10, 12, 10, 20)), now)).toMatch(/12.*10:20/);
    expect(whenText(0, now)).toBe("fecha desconocida");
  });

  it("«Hace 18 min» con la primera en mayúscula", () => {
    expect(agoText(at(new Date(2026, 10, 21, 11, 52)), now)).toBe("Hace 18 min");
    expect(agoText(0, now)).toBe("");
  });

  it("la fila dice nombre, origen, hora y cuánto hace", () => {
    const f = file({ modified: at(new Date(2026, 10, 21, 11, 52)) });
    expect(rowLine(f, now)).toMatch(/^Presupuesto 2027\.xlsx · Autorrecuperación · hoy 11:52 · Hace 18 min$/);
    // Sin fecha, la fila no lleva huecos.
    expect(rowLine(file(), now)).toBe("Presupuesto 2027.xlsx · Autorrecuperación · fecha desconocida");
  });

  it("lo más reciente primero, y a igual hora por nombre", () => {
    const list = [file({ name: "b", modified: 10 }), file({ name: "c", modified: 30 }), file({ name: "a", modified: 10 })];
    expect(sortUnsaved(list).map((f) => f.name)).toEqual(["c", "a", "b"]);
    // No toca la lista original.
    expect(list[0].name).toBe("b");
  });

  it("las versiones anteriores dicen por qué no hay ninguna", () => {
    expect(versionsSentence(0, 0)).toBe("Windows no tiene copias de versiones anteriores en esa unidad.");
    expect(versionsSentence(3, 0)).toBe("El archivo no está en ninguna de las 3 copias de esa unidad.");
    expect(versionsSentence(1, 0)).toBe("El archivo no está en la única copia de esa unidad.");
    expect(versionsSentence(3, 2)).toBeNull();
  });

  it("se abre con su programa solo si se sabe qué tipo es", () => {
    expect(canOpenCopy(file())).toBe(true);
    expect(canOpenCopy(file({ copyExt: null }))).toBe(false);
  });
});
