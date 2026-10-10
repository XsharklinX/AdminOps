import { describe, expect, it } from "vitest";
import { trend } from "./trend";

describe("trend", () => {
  it("sube, baja o se mantiene", () => {
    expect(trend(Array.from({ length: 30 }, (_, i) => 20 + i), 5).dir).toBe("up");
    expect(trend(Array.from({ length: 30 }, (_, i) => 80 - i), 5).dir).toBe("down");
    expect(trend(Array.from({ length: 30 }, () => 40), 5).dir).toBe("flat");
  });
  it("no se fía de un pico suelto", () => {
    const v = Array.from({ length: 30 }, () => 30);
    v[15] = 100;
    expect(trend(v, 5).dir).toBe("flat");
  });
  it("con pocos datos no dice nada", () => {
    expect(trend([1, 50, 90], 5).dir).toBe("flat");
  });
});
