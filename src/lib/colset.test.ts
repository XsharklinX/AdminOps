import { describe, expect, it } from "vitest";
import { fullOrder, moved, toggled, visibleIds, type ColInfo } from "./colset";

const cols: ColInfo[] = [{ id: "name", locked: true }, { id: "publisher" }, { id: "version" }, { id: "size", hidden: true }, { id: "date", hidden: true }];

describe("columnas a tu gusto", () => {
  it("sin nada guardado salen las de fábrica, con las ocultas fuera", () => {
    expect(visibleIds(cols, null)).toEqual(["name", "publisher", "version"]);
  });

  it("marcar una columna oculta la enseña y se recuerda; la primera no se puede quitar", () => {
    const a = toggled(cols, null, "size");
    expect(visibleIds(cols, a)).toEqual(["name", "publisher", "version", "size"]);
    const b = toggled(cols, a, "publisher");
    expect(visibleIds(cols, b)).toEqual(["name", "version", "size"]);
    expect(visibleIds(cols, toggled(cols, b, "name"))).toContain("name");
  });

  it("arrastrar cambia el orden, antes o después de otra", () => {
    const a = moved(cols, null, "version", "publisher", false);
    expect(visibleIds(cols, a)).toEqual(["name", "version", "publisher"]);
    const b = moved(cols, a, "name", "publisher", true);
    expect(visibleIds(cols, b)).toEqual(["version", "publisher", "name"]);
  });

  it("una columna nueva en otra versión aparece al final con sus valores de fábrica", () => {
    const saved = { order: ["name", "publisher"], shown: ["name"] };
    expect(visibleIds(cols, saved)).toEqual(["name", "version"]);
    expect(fullOrder(cols, saved)).toEqual(["name", "publisher", "version", "size", "date"]);
  });

  it("si una columna guardada ya no existe, se ignora", () => {
    expect(visibleIds(cols, { order: ["gone", "name"], shown: ["gone", "name"] })).toEqual(["name", "publisher", "version"]);
  });
});
