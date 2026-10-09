import { describe, expect, it } from "vitest";
import { dayNumber, floorOf, healthOf, monthsText, monthTicks } from "./batteryChart";

describe("la batería en el tiempo", () => {
  it("calcula la capacidad sobre la de fábrica", () => {
    expect(healthOf({ full: 35_500, design: 50_000 })).toBeCloseTo(71);
    expect(healthOf({ full: 1, design: 0 })).toBe(0);
  });

  it("cuenta los días sin depender de la zona horaria", () => {
    expect(dayNumber("1970-01-02")).toBe(1);
    expect(dayNumber("2026-03-01") - dayNumber("2026-02-01")).toBe(28);
  });

  it("empieza la gráfica por debajo del peor dato, sin pasar del 50 %", () => {
    expect(floorOf([96, 92, 88])).toBe(50);
    expect(floorOf([71, 64])).toBe(50);
    expect(floorOf([46, 52])).toBe(30);
    expect(floorOf([])).toBe(0);
  });

  it("pone los meses en el eje, y el año en enero", () => {
    const t = monthTicks("2025-11-10", "2026-03-20");
    expect(t.map((x) => x.label)).toEqual(["dic", "ene 26", "feb", "mar"]);
    // Muchos meses: uno de cada tres.
    expect(monthTicks("2024-01-15", "2026-01-15").length).toBeLessThanOrEqual(9);
  });

  it("dice el plazo en palabras", () => {
    expect(monthsText(0.4)).toBe("menos de un mes");
    expect(monthsText(1)).toBe("1 mes");
    expect(monthsText(8.2)).toBe("8 meses");
    expect(monthsText(27)).toBe("2 años y 3 meses");
    expect(monthsText(36)).toBe("3 años");
    expect(monthsText(80)).toBe("más de 5 años");
  });
});
