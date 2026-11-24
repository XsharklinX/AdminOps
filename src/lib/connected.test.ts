import { describe, expect, it } from "vitest";
import type { ConnectedDevice, RecentChange } from "./api";
import { actionsFor, ago, buildRows, HIGHLIGHT_MS, REMOVED_MS, summary } from "./connected";

const dev = (id: string, status: ConnectedDevice["status"] = "ok", code = 0): ConnectedDevice => ({
  id,
  name: id,
  class: "USB",
  manufacturer: "",
  status,
  code,
  problem: "",
  driverVersion: null,
  driverDate: null,
  system: false,
});
const change = (kind: "arrived" | "removed", at: number, d: ConnectedDevice): RecentChange => ({ kind, at, device: d });

describe("lista de dispositivos conectados", () => {
  const now = 1_000_000;

  it("lo recién conectado sube arriba, lo último primero, y el resto queda como venía", () => {
    const [a, b, c, d] = [dev("A", "error"), dev("B"), dev("C"), dev("D", "noDriver")];
    const rows = buildRows([a, b, c, d], [change("arrived", now - 50_000, c), change("arrived", now - 4_000, d)], now);
    expect(rows.map((r) => r.device.id)).toEqual(["D", "C", "A", "B"]);
    expect(rows[0].fresh).toEqual({ kind: "arrived", at: now - 4_000 });
    expect(rows[2].fresh).toBeUndefined();
  });

  it("el resaltado dura dos minutos", () => {
    const a = dev("A");
    expect(buildRows([a], [change("arrived", now - HIGHLIGHT_MS, a)], now)[0].fresh).toBeDefined();
    expect(buildRows([a], [change("arrived", now - HIGHLIGHT_MS - 1, a)], now)[0].fresh).toBeUndefined();
  });

  it("lo recién quitado sale atenuado 30 s, tras lo nuevo", () => {
    const [a, b, gone] = [dev("A"), dev("B"), dev("G")];
    const rows = buildRows([a, b], [change("removed", now - 10_000, gone), change("arrived", now - 1_000, b)], now);
    expect(rows.map((r) => [r.device.id, r.fresh?.kind])).toEqual([
      ["B", "arrived"],
      ["G", "removed"],
      ["A", undefined],
    ]);
    expect(buildRows([a], [change("removed", now - REMOVED_MS - 1, gone)], now)).toHaveLength(1);
  });

  it("si lo quitado vuelve a estar, no se duplica", () => {
    const a = dev("A");
    expect(buildRows([a], [change("removed", now - 5_000, a)], now).map((r) => r.fresh)).toEqual([undefined]);
  });

  it("el resumen no cuenta ceros", () => {
    expect(summary([])).toBe("0 conectados");
    expect(summary([dev("A")])).toBe("1 conectado");
    expect(summary([dev("A"), dev("B", "noDriver"), dev("C", "error"), dev("D", "error")])).toBe("4 conectados · 1 sin driver · 2 con error");
  });

  it("cuánto hace", () => {
    expect(ago(4_200)).toBe("hace 4 s");
    expect(ago(-5)).toBe("hace 0 s");
    expect(ago(61_000)).toBe("hace 1 min");
  });

  it("cada estado tiene su acción", () => {
    expect(actionsFor(dev("A"))).toEqual([]);
    expect(actionsFor(dev("A", "noDriver", 28))).toEqual(["scan"]);
    expect(actionsFor(dev("A", "error", 22))).toEqual(["enable"]);
    expect(actionsFor(dev("A", "error", 43))).toEqual(["restart", "scan"]);
  });
});
