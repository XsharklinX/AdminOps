import { describe, expect, it } from "vitest";
import type { ChangeView } from "./api";
import { addDays, dayEs, describeChange, groupByKind, isoDay, lastMonday, weekdayOf } from "./whatChanged";

const c = (o: Partial<ChangeView>): ChangeView => ({ kind: "program", change: "changed", label: "X", old: "1", new: "2", day: "2026-11-10", fix: null, ...o });

describe("¿Qué cambió? · ayudas", () => {
  it("fechas", () => {
    expect(isoDay(new Date(2026, 10, 5))).toBe("2026-11-05");
    expect(isoDay(lastMonday(new Date(2026, 10, 18)))).toBe("2026-11-16");
    // Si hoy es lunes, el anterior.
    expect(isoDay(lastMonday(new Date(2026, 10, 16)))).toBe("2026-11-09");
    expect(isoDay(addDays(new Date(2026, 10, 1), -1))).toBe("2026-10-31");
    expect(weekdayOf("2026-11-16")).toBe("lunes");
    expect(dayEs("2026-11-16")).toBe("16/11/2026");
    expect(weekdayOf("raro")).toBe("raro");
  });

  it("frases", () => {
    expect(describeChange(c({ change: "added", old: null }))).toBe("Nuevo · versión 2");
    expect(describeChange(c({}))).toBe("Versión 1 → 2");
    expect(describeChange(c({ change: "removed", new: null }))).toBe("Ya no está");
    expect(describeChange(c({ kind: "service", old: "Auto", new: "Disabled" }))).toBe("Inicio automático → desactivado");
  });

  it("agrupa por tipo en orden y omite lo vacío", () => {
    const g = groupByKind([c({ kind: "service" }), c({ kind: "program" }), c({ kind: "program", label: "Y" })]);
    expect(g.map((x) => x.kind)).toEqual(["program", "service"]);
    expect(g[0].items).toHaveLength(2);
  });
});
