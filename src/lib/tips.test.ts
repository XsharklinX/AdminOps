import { describe, expect, it } from "vitest";
import { TIPS, tipAt } from "./tips";

describe("consejos de espera", () => {
  it("cambia cada 8 segundos y da la vuelta", () => {
    expect(tipAt(0)).toBe(TIPS[0]);
    expect(tipAt(7)).toBe(TIPS[0]);
    expect(tipAt(8)).toBe(TIPS[1]);
    expect(tipAt(8 * TIPS.length)).toBe(TIPS[0]);
  });
  it("empieza donde se le diga, también con valores raros", () => {
    expect(tipAt(0, 3)).toBe(TIPS[3]);
    expect(tipAt(0, -1)).toBe(TIPS[TIPS.length - 1]);
  });
  it("ninguno está vacío ni es larguísimo", () => {
    for (const t of TIPS) expect(t.length > 20 && t.length < 160).toBe(true);
  });
});
