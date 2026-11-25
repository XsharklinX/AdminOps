import { describe, expect, it } from "vitest";
import { noteLines, percentOf } from "./UpdateDialog";

describe("UpdateDialog", () => {
  it("convierte las notas de GitHub en líneas legibles", () => {
    const md = "# AdminOps 1.4.1\n\n## Español\n- **Actualizar** sin salir\nTexto con `código` y [enlace](https://x)\n";
    expect(noteLines(md)).toEqual([
      { kind: "h", text: "AdminOps 1.4.1" },
      { kind: "h", text: "Español" },
      { kind: "li", text: "Actualizar sin salir" },
      { kind: "p", text: "Texto con código y enlace" },
    ]);
  });
  it("lee el porcentaje de la descarga", () => {
    expect(percentOf("Descargando la versión 1.4.2… 42 %")).toBe(42);
    expect(percentOf("Guardando la versión 1.4.1 por si hay que volver…")).toBeNull();
    expect(percentOf(null)).toBeNull();
  });
});
