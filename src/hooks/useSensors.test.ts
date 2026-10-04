import { describe, expect, it } from "vitest";
import { nextSensorWait } from "./useSensors";

describe("ritmo de lectura de los sensores", () => {
  it("una lectura rápida mantiene el ritmo", () => {
    expect(nextSensorWait(5000, 40)).toBe(5000);
    expect(nextSensorWait(5000, 1400)).toBe(5000);
  });

  it("una lenta espacia la siguiente, sin pasar de un minuto", () => {
    expect(nextSensorWait(5000, 3600)).toBe(18_000);
    expect(nextSensorWait(5000, 50_000)).toBe(60_000);
    // Nunca más rápido que el ritmo pedido.
    expect(nextSensorWait(30_000, 2000)).toBe(30_000);
  });
});
