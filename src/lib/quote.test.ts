import { describe, expect, it } from "vitest";
import { bestCatalogMatch, lineFor, margin } from "./quote";
import type { CatalogItem } from "./api";

const cat: CatalogItem[] = [
  { name: "SSD SATA 500 GB", price: 55, part: true, warrantyDays: 730, cost: 38 },
  { name: "Memoria 8 GB DDR4 SO-DIMM", price: 35, part: true, warrantyDays: 730 },
  { name: "Hora de técnico", price: 40, part: false, warrantyDays: 0 },
];

describe("presupuestos", () => {
  it("encuentra el precio en el catálogo", () => {
    expect(bestCatalogMatch("Memoria DDR4 SO-DIMM de 8 GB a 3200", cat)?.price).toBe(35);
    expect(bestCatalogMatch("SSD SATA de 500 GB", cat)?.name).toBe("SSD SATA 500 GB");
    expect(bestCatalogMatch("Batería original", cat)).toBeNull();
  });
  it("monta la línea", () => {
    expect(lineFor("SSD SATA 500 GB 2,5\"", cat)).toMatchObject({ price: 55, part: true, warrantyDays: 730 });
    expect(lineFor("Pantalla 15,6\"", cat, 90)).toMatchObject({ description: "Pantalla 15,6\"", price: 90 });
  });
  it("calcula el margen", () => {
    expect(margin(cat[0])).toBe(31);
    expect(margin(cat[1])).toBeNull();
  });
});
