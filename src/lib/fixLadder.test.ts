import { describe, expect, it } from "vitest";
import { checkKind, LADDERS, nextRung } from "./fixLadder";

describe("escalera de arreglos", () => {
  it("tras SFC, DISM; tras DISM, SFC de nuevo; al final, lo que se hace a mano", () => {
    expect(nextRung("repair.sfc", ["repair.sfc"])).toMatchObject({ kind: "fix", id: "repair.dism" });
    expect(nextRung("repair.sfc", ["repair.sfc", "repair.dism"])).toMatchObject({ kind: "fix", id: "repair.sfc" });
    expect(nextRung("repair.sfc", ["repair.sfc", "repair.dism", "repair.sfc"])).toMatchObject({ kind: "manual" });
  });

  it("un arreglo sin escalera no propone nada", () => {
    expect(nextRung("repair.desconocido", [])).toBeNull();
  });

  it("cada escalón de arreglo existe en alguna escalera o en el catálogo conocido", () => {
    const ids = Object.values(LADDERS).flat().filter((r) => r.kind === "fix").map((r) => (r.kind === "fix" ? r.id : ""));
    for (const id of ids) expect(id).toMatch(/^(repair|cleanup)\./);
  });

  it("lo que mira el análisis rápido se comprueba en segundos; lo demás, con el completo", () => {
    expect(checkKind("Discos")).toBe("quick");
    expect(checkKind("Seguridad")).toBe("full");
  });
});
