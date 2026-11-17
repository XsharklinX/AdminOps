import { describe, expect, it } from "vitest";
import { READ_EVERY_MS, readDue } from "./prefetch";

describe("readDue", () => {
  it("lee si nunca se ha leído", () => {
    expect(readDue(1_000_000, undefined, undefined)).toBe(true);
  });
  it("no repite una lectura adelantada hace menos de 2 minutos", () => {
    const now = 10_000_000;
    expect(readDue(now, now - READ_EVERY_MS + 1000, undefined)).toBe(false);
    expect(readDue(now, now - READ_EVERY_MS, undefined)).toBe(true);
  });
  it("no lee si la memoria de lecturas ya tiene algo reciente", () => {
    const now = 10_000_000;
    expect(readDue(now, undefined, now - 30_000)).toBe(false);
    expect(readDue(now, now - 10 * READ_EVERY_MS, now - 30_000)).toBe(false);
  });
});
