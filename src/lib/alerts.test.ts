import { describe, expect, it } from "vitest";
import type { WindowsAlert } from "./api";
import { countByLevel, dayLabel, filterAlerts, groupByDay, worstUnread } from "./alerts";

const NOW = new Date(2026, 9, 4, 15, 0, 0).getTime();
const at = (daysAgo: number, hour = 10) => Math.floor(new Date(2026, 9, 4 - daysAgo, hour, 0, 0).getTime() / 1000);
const alert = (id: string, level: WindowsAlert["level"], time: number, read = false): WindowsAlert => ({
  id,
  time,
  key: id,
  level,
  title: id,
  detail: "",
  explanation: "",
  advice: "",
  page: null,
  count: 1,
  read,
});

describe("avisos de la campana", () => {
  const list = [alert("a", "bad", at(0)), alert("b", "warn", at(0, 9), true), alert("c", "info", at(1)), alert("d", "warn", at(3)), alert("e", "bad", at(20), true)];

  it("cuenta por nivel", () => {
    expect(countByLevel(list)).toEqual({ all: 5, bad: 2, warn: 2, info: 1 });
  });

  it("filtra por nivel y por sin leer", () => {
    expect(filterAlerts(list, "bad", false).map((a) => a.id)).toEqual(["a", "e"]);
    expect(filterAlerts(list, "bad", true).map((a) => a.id)).toEqual(["a"]);
    expect(filterAlerts(list, "all", true).map((a) => a.id)).toEqual(["a", "c", "d"]);
  });

  it("pone nombre al día", () => {
    expect(dayLabel(at(0, 1), NOW)).toBe("Hoy");
    // Ayer a última hora sigue siendo ayer, aunque hayan pasado pocas horas.
    expect(dayLabel(at(1, 23), new Date(2026, 9, 4, 0, 30).getTime())).toBe("Ayer");
    expect(dayLabel(at(3), NOW)).toMatch(/^[A-ZÁÉÍÓÚ]/);
    expect(dayLabel(at(20), NOW)).toContain("septiembre");
  });

  it("agrupa por días, lo más reciente primero", () => {
    const groups = groupByDay([...list].reverse(), NOW);
    expect(groups.map((g) => g.items.map((a) => a.id))).toEqual([["a", "b"], ["c"], ["d"], ["e"]]);
    expect(groups[0].label).toBe("Hoy");
    expect(groups[1].label).toBe("Ayer");
  });

  it("el contador es rojo solo con algo grave sin leer", () => {
    expect(worstUnread(list)).toBe("bad");
    expect(worstUnread(list.filter((a) => a.id !== "a"))).toBe("warn");
    expect(worstUnread(list.filter((a) => a.read))).toBeNull();
  });
});
