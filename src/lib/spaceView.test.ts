import { describe, expect, it } from "vitest";
import type { SpaceEntry, SpaceFolder } from "./api";
import { crumbsOf, kindShares, looseSize, matchesText, parentOf, percent, sortEntries } from "./spaceView";

const entry = (name: string, size: number, files = 1, modified?: number): SpaceEntry => ({ name, path: `C:\\Datos\\${name}`, size, files, hasChildren: false, modified });

describe("pantalla de Espacio", () => {
  it("reparte los tipos de mayor a menor", () => {
    const shares = kindShares([
      { id: "doc", size: 25 },
      { id: "video", size: 75 },
    ]);
    expect(shares.map((k) => k.id)).toEqual(["video", "doc"]);
    expect(shares[0].share).toBeCloseTo(0.75);
    expect(kindShares([])).toEqual([]);
  });

  it("cuenta lo que no es una subcarpeta con nombre", () => {
    const folder: SpaceFolder = { path: "C:\\Datos", size: 1000, files: 9, kinds: [], children: [entry("a", 600), entry("b", 250)] };
    expect(looseSize(folder)).toBe(150);
    expect(looseSize({ ...folder, size: 800 })).toBe(0);
  });

  it("ordena las carpetas", () => {
    const list = [entry("beta", 10, 5, 300), entry("Álbum", 30, 1, 100), entry("gamma", 20, 9)];
    expect(sortEntries(list, "size").map((e) => e.name)).toEqual(["Álbum", "gamma", "beta"]);
    expect(sortEntries(list, "name").map((e) => e.name)).toEqual(["Álbum", "beta", "gamma"]);
    expect(sortEntries(list, "files").map((e) => e.name)).toEqual(["gamma", "beta", "Álbum"]);
    // Lo más antiguo primero; lo que no tiene fecha, al final.
    expect(sortEntries(list, "old").map((e) => e.name)).toEqual(["Álbum", "beta", "gamma"]);
  });

  it("busca sin tildes ni mayúsculas", () => {
    expect(matchesText("Presentación FINAL.pptx", "presentacion final")).toBe(true);
    expect(matchesText("boda.mp4", "  ")).toBe(true);
    expect(matchesText("boda.mp4", "iso")).toBe(false);
  });

  it("saca las migas y la carpeta de arriba", () => {
    expect(crumbsOf("C:\\", "C:\\Users\\ana\\Videos").map((c) => c.path)).toEqual(["C:\\", "C:\\Users", "C:\\Users\\ana", "C:\\Users\\ana\\Videos"]);
    expect(crumbsOf("D:\\Copias", "D:\\Copias\\2019").map((c) => c.name)).toEqual(["D:\\Copias", "2019"]);
    expect(crumbsOf("C:\\", "C:\\")).toHaveLength(1);
    expect(parentOf("C:\\Users\\ana")).toBe("C:\\Users");
    expect(parentOf("C:\\Users")).toBe("C:\\");
  });

  it("no dice 0 % de algo que existe", () => {
    expect(percent(1, 1000)).toBe("<1 %");
    expect(percent(500, 1000)).toBe("50 %");
    expect(percent(0, 1000)).toBe("0 %");
  });
});
