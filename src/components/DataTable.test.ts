import { describe, expect, it } from "vitest";
import { compareValues, sortRows } from "./DataTable";

describe("orden de las tablas", () => {
  it("las IP y los textos con números se ordenan como los lee una persona", () => {
    const ips = ["192.168.1.20", "192.168.1.3", "192.168.1.100"].map((ip) => ({ ip }));
    expect(sortRows(ips, (r) => r.ip, false).map((r) => r.ip)).toEqual(["192.168.1.3", "192.168.1.20", "192.168.1.100"]);
    expect(sortRows([{ n: "PC-10" }, { n: "pc-2" }], (r) => r.n, false).map((r) => r.n)).toEqual(["pc-2", "PC-10"]);
  });

  it("lo vacío va al final, se ordene hacia donde se ordene", () => {
    const rows = [{ ms: null }, { ms: 30 }, { ms: 5 }] as { ms: number | null }[];
    expect(sortRows(rows, (r) => r.ms, false).map((r) => r.ms)).toEqual([5, 30, null]);
    expect(sortRows(rows, (r) => r.ms, true).map((r) => r.ms)).toEqual([30, 5, null]);
    expect(compareValues("", "a", true)).toBe(1);
  });

  it("sin columna, las filas salen como llegan; y a igualdad, en su orden", () => {
    const rows = [{ k: 1, t: "b" }, { k: 1, t: "a" }];
    expect(sortRows(rows, undefined, false)).toBe(rows);
    expect(sortRows(rows, (r) => r.k, true).map((r) => r.t)).toEqual(["b", "a"]);
  });
});
