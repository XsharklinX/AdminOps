import { describe, expect, it } from "vitest";
import { bump, diff, parsePack, PACK_FORMAT } from "./pack";

describe("biblioteca compartida", () => {
  it("distingue lo nuevo, lo que cambia y lo igual", () => {
    const mine = [
      { id: "a", title: "A", uses: 5 },
      { id: "b", title: "B" },
    ];
    const theirs = [
      { id: "a", title: "A", uses: 0, updated: 9 },
      { id: "b", title: "B cambiada" },
      { id: "c", title: "C" },
    ];
    const d = diff(mine, theirs);
    expect(d.same.map((x) => x.id)).toEqual(["a"]);
    expect(d.changed.map((x) => x.id)).toEqual(["b"]);
    expect(d.added.map((x) => x.id)).toEqual(["c"]);
  });
  it("solo acepta archivos de biblioteca", () => {
    expect(parsePack("{}")).toBeNull();
    expect(parsePack("no es json")).toBeNull();
    const p = parsePack(JSON.stringify({ format: PACK_FORMAT, name: "Soporte Norte", solutions: [{ id: "x" }] }));
    expect(p?.solutions.length).toBe(1);
    expect(p?.recipes).toEqual([]);
  });
  it("sube la versión", () => {
    expect(bump("3")).toBe("4");
    expect(bump("1.2")).toBe("1.3");
  });
});
