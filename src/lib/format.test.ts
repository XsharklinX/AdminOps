import { describe, expect, it } from "vitest";
import { ago, dateTime, fullDate, shortDate } from "./format";

const NOW = new Date(2026, 9, 2, 12, 0, 0).getTime();
const secs = (msAgo: number) => (NOW - msAgo) / 1000;
const MIN = 60_000;
const DAY = 24 * 60 * MIN;

describe("ago", () => {
  it("dice el tiempo de forma natural", () => {
    expect(ago(secs(10_000), NOW)).toBe("hace un momento");
    expect(ago(secs(5 * MIN), NOW)).toBe("hace 5 min");
    expect(ago(secs(3 * 60 * MIN), NOW)).toBe("hace 3 h");
    expect(ago(secs(DAY + MIN), NOW)).toBe("ayer");
    expect(ago(secs(4 * DAY), NOW)).toBe("hace 4 días");
    expect(ago(secs(90 * DAY), NOW)).toBe("hace 3 meses");
    expect(ago(secs(800 * DAY), NOW)).toBe("hace 2 años");
  });

  it("acepta segundos y fechas ISO por igual", () => {
    const iso = new Date(NOW - 4 * DAY).toISOString();
    expect(ago(iso, NOW)).toBe(ago(secs(4 * DAY), NOW));
  });

  it("una fecha que no se entiende se devuelve tal cual", () => {
    expect(ago("no es fecha", NOW)).toBe("no es fecha");
    expect(dateTime("no es fecha")).toBe("no es fecha");
  });
});

describe("fechas", () => {
  it("la fecha completa lleva el año y la corta solo si no es el actual", () => {
    expect(fullDate(new Date(2024, 0, 15))).toMatch(/2024/);
    expect(shortDate(new Date(2024, 0, 15))).toMatch(/2024/);
    expect(shortDate(new Date(new Date().getFullYear(), 0, 15))).not.toMatch(/\d{4}/);
  });
});
