// «Lo de siempre» de Ctrl+K: recuento por franja, poda de 31 días, orden y límite de 5.
import { afterEach, describe, expect, it, vi } from "vitest";
import { altIndex, habitAllowed, loadUses, noteUse, recordUse, slotOf, slotTitle, timesThisMonth, topFor, trackableKey, type Use } from "./ctrlkHabits";

const DAY = 24 * 60 * 60 * 1000;
// 15 de noviembre de 2026, hora local.
const at = (day: number, hour: number) => new Date(2026, 10, day, hour, 0, 0).getTime();
const NOW = at(20, 10);
const all = () => true;

const use = (k: string, t: number): Use => ({ k, t, s: slotOf(new Date(t)) });

describe("franja del día", () => {
  it("mañana antes de las 14:00 y tarde desde las 14:00", () => {
    expect(slotOf(new Date(2026, 10, 20, 0, 0))).toBe("morning");
    expect(slotOf(new Date(2026, 10, 20, 13, 59))).toBe("morning");
    expect(slotOf(new Date(2026, 10, 20, 14, 0))).toBe("afternoon");
    expect(slotOf(new Date(2026, 10, 20, 23, 59))).toBe("afternoon");
  });
  it("las cabeceras", () => {
    expect(slotTitle("morning")).toBe("Lo de siempre · por la mañana");
    expect(slotTitle("afternoon")).toBe("Lo de siempre · por la tarde");
  });
  it("«N veces este mes»", () => {
    expect(timesThisMonth(7)).toBe("7 veces este mes");
    expect(timesThisMonth(1)).toBe("1 vez este mes");
  });
});

describe("recuento por franja", () => {
  it("solo cuenta la franja pedida", () => {
    const uses = [use("page:a", at(19, 9)), use("page:a", at(18, 10)), use("page:a", at(18, 16)), use("page:b", at(19, 17))];
    expect(topFor(uses, "morning", NOW, all)).toEqual([{ key: "page:a", count: 2 }]);
    expect(topFor(uses, "afternoon", NOW, all)).toEqual([
      { key: "page:b", count: 1 },
      { key: "page:a", count: 1 },
    ]);
  });
  it("sin usos en la franja, no hay nada (la paleta se queda como hoy)", () => {
    expect(topFor([use("page:a", at(19, 17))], "morning", NOW, all)).toEqual([]);
    expect(topFor([], "morning", NOW, all)).toEqual([]);
  });
  it("cuenta solo los últimos 30 días", () => {
    const uses = [use("page:a", NOW - 29 * DAY), use("page:a", NOW - 31 * DAY), use("page:b", NOW - 30 * DAY - 1000)];
    expect(topFor(uses, "morning", NOW, all)).toEqual([{ key: "page:a", count: 1 }]);
  });
});

describe("poda de 31 días", () => {
  it("al apuntar una acción se borra lo de más de 31 días", () => {
    const old = [use("page:a", NOW - 32 * DAY), use("page:b", NOW - 31 * DAY + 1000), use("page:c", NOW - DAY)];
    const next = recordUse(old, "page:d", NOW);
    expect(next.map((u) => u.k)).toEqual(["page:b", "page:c", "page:d"]);
    expect(next[2]).toEqual({ k: "page:d", t: NOW, s: "morning" });
  });
  it("una clave que no se cuenta no se apunta, pero la poda se hace igual", () => {
    const next = recordUse([use("page:a", NOW - 40 * DAY), use("page:b", NOW)], "contact:123", NOW);
    expect(next.map((u) => u.k)).toEqual(["page:b"]);
  });
});

describe("orden y límite", () => {
  it("más usadas primero; a igualdad, la más reciente", () => {
    const uses = [use("page:a", at(10, 9)), use("page:b", at(11, 9)), use("page:b", at(12, 9)), use("page:c", at(15, 9))];
    expect(topFor(uses, "morning", NOW, all).map((h) => h.key)).toEqual(["page:b", "page:c", "page:a"]);
  });
  it("como mucho 5", () => {
    const uses = ["a", "b", "c", "d", "e", "f", "g"].map((x, i) => use(`page:${x}`, at(10 + i, 9)));
    const top = topFor(uses, "morning", NOW, all);
    expect(top).toHaveLength(5);
    expect(top[0].key).toBe("page:g");
  });
});

describe("acciones que ya no existen", () => {
  it("no salen, y el límite se llena con las que sí están", () => {
    const keys = ["a", "b", "c", "d", "e", "f"];
    const uses = keys.flatMap((x, i) => Array.from({ length: 10 - i }, (_, j) => use(`page:${x}`, at(1 + j, 9))));
    const top = topFor(uses, "morning", NOW, (k) => k !== "page:a");
    expect(top.map((h) => h.key)).toEqual(["page:b", "page:c", "page:d", "page:e", "page:f"]);
  });
});

describe("Alt+1…5", () => {
  it("sin texto lanza la de esa posición; con texto no hace nada", () => {
    expect(altIndex("Digit1", "", 3)).toBe(0);
    expect(altIndex("Digit3", "  ", 3)).toBe(2);
    expect(altIndex("Digit4", "", 3)).toBeNull();
    expect(altIndex("Digit2", "dominio", 3)).toBeNull();
    expect(altIndex("Digit6", "", 5)).toBeNull();
    expect(altIndex("KeyA", "", 5)).toBeNull();
  });
  it("el teclado numérico no lanza nada (Alt+número son códigos de carácter)", () => {
    expect(altIndex("Numpad1", "", 3)).toBeNull();
    expect(altIndex("Numpad5", "", 5)).toBeNull();
  });
});

describe("«Lo de siempre» según el modo", () => {
  const pageOk = (mode: "admin" | "user") => (p: string) => mode === "admin" || p === "updates";
  const expert = (id: string) => (id === "whynot" ? "processes" : undefined);
  const ok = (key: string, mode: "admin" | "user") => habitAllowed(key, mode, pageOk(mode), expert);
  it("las pantallas y secciones, solo si están permitidas en el modo", () => {
    expect(ok("page:updates", "user")).toBe(true);
    expect(ok("page:domain", "user")).toBe(false);
    expect(ok("section:domain:users", "user")).toBe(false);
    expect(ok("section:updates:history", "user")).toBe(true);
    expect(ok("page:domain", "admin")).toBe(true);
  });
  it("las entradas expertas, según su pantalla", () => {
    expect(ok("expert:whynot", "user")).toBe(false);
    expect(ok("expert:whynot", "admin")).toBe(true);
    expect(ok("expert:desconocida", "admin")).toBe(false);
  });
  it("las que se lanzan directamente, solo en modo admin", () => {
    for (const k of ["action:do:restore-point", "action:diag", "tool:regedit", "tweak:telemetry"]) {
      expect(ok(k, "user")).toBe(false);
      expect(ok(k, "admin")).toBe(true);
    }
  });
});

describe("nada personal", () => {
  it("solo se cuentan páginas, secciones, acciones, ajustes y herramientas", () => {
    expect(trackableKey("page:updates")).toBe("page:updates");
    expect(trackableKey("section:machine:displays")).toBe("section:machine:displays");
    expect(trackableKey("action:do:restore-point")).toBe("action:do:restore-point");
    expect(trackableKey("contact:abc")).toBeNull();
    expect(trackableKey("smart:person")).toBeNull();
    expect(trackableKey("quick:unlock:maria")).toBeNull();
    expect(trackableKey(`action:${"x".repeat(100)}`)).toBeNull();
  });
});

describe("guardado en el equipo", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("guarda ids, fecha y franja; poda al escribir; ignora basura", () => {
    const mem = new Map<string, string>();
    vi.stubGlobal("localStorage", { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v), removeItem: (k: string) => void mem.delete(k) });
    noteUse("page:a", NOW - 40 * DAY);
    noteUse("page:b", NOW);
    noteUse("contact:zzz", NOW);
    expect(loadUses()).toEqual([{ k: "page:b", t: NOW, s: "morning" }]);
    mem.set("adminops.ctrlk.uses", JSON.stringify([{ k: "page:b", t: NOW, s: "morning" }, { k: 3 }, null, "x"]));
    expect(loadUses()).toHaveLength(1);
    mem.set("adminops.ctrlk.uses", "no es json");
    expect(loadUses()).toEqual([]);
  });
});
