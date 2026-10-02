import { describe, expect, it } from "vitest";
import type { JournalEntry } from "./api";
import { groupByDay, matchesJournal } from "./journalDays";

const NOW = new Date(2026, 9, 1, 15, 0, 0).getTime();
const at = (daysAgo: number, hour = 10) => Math.floor(new Date(2026, 9, 1 - daysAgo, hour, 0, 0).getTime() / 1000);

const entry = (over: Partial<JournalEntry>): JournalEntry => ({ id: 1, timestamp: at(0), op: "apply", tweakId: null, title: "Ajuste", ok: true, message: null, reverted: false, undoable: true, ...over });

describe("groupByDay", () => {
  it("agrupa por día, lo más reciente primero", () => {
    const days = groupByDay([entry({ id: 1, timestamp: at(1) }), entry({ id: 2, timestamp: at(0, 9) }), entry({ id: 3, timestamp: at(0, 12) }), entry({ id: 4, timestamp: at(5) })], NOW);
    expect(days.map((d) => d.label)).toEqual(["Hoy", "Ayer", "Sábado, 26 de septiembre"]);
    expect(days[0].entries.map((e) => e.id)).toEqual([3, 2]);
  });

  it("de otro año, la fecha lleva el año", () => {
    const [d] = groupByDay([entry({ timestamp: Math.floor(new Date(2025, 2, 3, 10).getTime() / 1000) })], NOW);
    expect(d.label).toContain("2025");
  });
});

describe("matchesJournal", () => {
  it("filtra por tipo, por error y por lo que aún se puede deshacer", () => {
    const failed = entry({ ok: false });
    const reverted = entry({ reverted: true });
    const run = entry({ op: "run", undoable: false });
    expect(matchesJournal(failed, "failed", "")).toBe(true);
    expect(matchesJournal(failed, "undoable", "")).toBe(false);
    expect(matchesJournal(reverted, "undoable", "")).toBe(false);
    expect(matchesJournal(entry({}), "undoable", "")).toBe(true);
    expect(matchesJournal(run, "run", "")).toBe(true);
    expect(matchesJournal(run, "apply", "")).toBe(false);
  });

  it("busca en el título y en el mensaje", () => {
    const e = entry({ title: "Limpiar temporales", message: "Liberados 2 GB" });
    expect(matchesJournal(e, "all", "TEMPORALES")).toBe(true);
    expect(matchesJournal(e, "all", "2 gb")).toBe(true);
    expect(matchesJournal(e, "all", "impresora")).toBe(false);
  });
});
