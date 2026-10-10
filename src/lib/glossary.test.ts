import { describe, expect, it } from "vitest";
import { GLOSSARY, lookupTerm, termIn } from "./glossary";

describe("glosario", () => {
  it("encuentra términos y sus otros nombres", () => {
    expect(lookupTerm("smart")?.term).toBe("SMART");
    expect(lookupTerm("Arranque seguro")?.term).toBe("Secure Boot");
    expect(lookupTerm("nada")).toBeNull();
  });
  it("encuentra el término dentro de un texto", () => {
    expect(termIn("Errores CRC del cable")?.term).toBe("CRC");
    expect(termIn("Sectores reasignados")?.term).toBe("Sectores reasignados");
    expect(termIn("Temperatura")).toBeNull();
  });
  it("cada entrada dice qué es, si preocupa y qué hacer", () => {
    for (const g of GLOSSARY) expect(g.what && g.worry && g.todo).toBeTruthy();
  });
});
