import { describe, expect, it } from "vitest";
import { addRecent, nameMatches } from "./quickActions";

describe("acciones de Ctrl+K", () => {
  it("encuentra la impresora sin tildes ni mayúsculas", () => {
    expect(nameMatches("HP LaserJet · Recepción", "recepcion")).toBe(true);
    expect(nameMatches("HP LaserJet · Recepción", "recepción hp")).toBe(true);
    expect(nameMatches("Epson Contabilidad", "recepcion")).toBe(false);
    expect(nameMatches("x", "")).toBe(false);
  });

  it("«Hace poco» guarda las últimas cinco sin repetir", () => {
    let l = [] as ReturnType<typeof addRecent>;
    for (let i = 0; i < 7; i++) l = addRecent(l, { label: `A${i}`, query: `q${i}` });
    expect(l).toHaveLength(5);
    expect(l[0].label).toBe("A6");
    l = addRecent(l, { label: "A4", query: "q4" });
    expect(l[0].label).toBe("A4");
    expect(l.filter((r) => r.label === "A4")).toHaveLength(1);
  });
});
