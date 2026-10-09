import { describe, expect, it } from "vitest";
import { RELEASES } from "./changelog";
import { NEW_IN, newerThan, pendingMarks } from "./whatsNew";
import { parseNavKey, sectionsOf } from "./sections";
import { NAV } from "../components/Sidebar";

describe("marca «Nuevo»", () => {
  it("compara versiones por números, no por letras", () => {
    expect(newerThan("1.2.10", "1.2.9")).toBe(true);
    expect(newerThan("1.2.3", "1.2.3")).toBe(false);
    expect(newerThan("1.2.3", "1.3")).toBe(false);
    expect(newerThan("1.2.3", "0")).toBe(true);
  });

  it("solo señala lo posterior a la versión con la que se empezó, y lo no visto", () => {
    const recent = ["1.2.7", "1.2.6", "1.2.5"];
    const all = Object.keys(NEW_IN);
    expect(pendingMarks("0", [], recent).sort()).toEqual(all.filter((k) => recent.includes(NEW_IN[k])).sort());
    // Instalación nueva: nada es «nuevo».
    expect(pendingMarks("1.2.7", [], recent)).toEqual([]);
    // Venía de la 1.2.5: solo lo de después.
    expect(pendingMarks("1.2.5", [], recent)).toEqual(all.filter((k) => newerThan(NEW_IN[k], "1.2.5") && recent.includes(NEW_IN[k])));
    expect(pendingMarks("0", ["dashboard"], recent)).not.toContain("dashboard");
    // Lo de versiones que ya no son recientes deja de señalarse.
    expect(pendingMarks("0", [], ["9.9.9"])).toEqual([]);
  });

  it("cada marca apunta a una pantalla o sección que existe y a una versión publicada", () => {
    const versions = RELEASES.map((r) => r.version);
    for (const [key, version] of Object.entries(NEW_IN)) {
      const { page, section } = parseNavKey(key);
      expect(NAV.some((n) => n.id === page), key).toBe(true);
      if (section) expect(sectionsOf(page).some((s) => s.id === section), key).toBe(true);
      expect(versions, key).toContain(version);
    }
  });
});
