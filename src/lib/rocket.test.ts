import { describe, expect, it } from "vitest";
import { memLevel, rocketSummary, share, shouldNudge } from "./rocket";

const base = { ramFreed: 0, ramUsedBefore: 0, ramUsedAfter: 0, ramTotal: 0, trimmed: 0, standby: false, tempFreed: 0, tempFiles: 0, tempSkipped: 0, dns: true, ms: 2300, apps: [], folders: [] };
const MB = 1024 * 1024;

describe("rocketSummary", () => {
  it("cuenta memoria y temporales", () => {
    const s = rocketSummary({ ...base, ramFreed: 1200 * MB, tempFreed: 340 * MB, tempFiles: 812 });
    expect(s.line).toBe("1.2 GB de memoria liberados y 340.0 MB de temporales borrados");
    expect(s.sub).toBe("Sin cerrar nada · 2 s");
  });

  it("no presume de unos pocos MB de memoria", () => {
    const s = rocketSummary({ ...base, ramFreed: 20 * MB });
    expect(s.line).toBe("El equipo ya estaba ligero");
  });

  it("temporales pequeños se cuentan por número y los abiertos se dicen", () => {
    const s = rocketSummary({ ...base, tempFreed: 2000, tempFiles: 1, tempSkipped: 3, ms: 400 });
    expect(s.line).toBe("1 temporal borrado");
    expect(s.sub).toBe("Sin cerrar nada · 1 s · 3 en uso, se quedan");
  });
});

describe("la burbuja avisa", () => {
  it("colores del anillo", () => {
    expect(memLevel(60)).toBe("ok");
    expect(memLevel(80)).toBe("warn");
    expect(memLevel(92)).toBe("bad");
  });

  it("late tras dos minutos altos", () => {
    expect(shouldNudge(null, 1e9)).toBe(false);
    expect(shouldNudge(0, 119_000)).toBe(false);
    expect(shouldNudge(0, 120_000)).toBe(true);
  });

  it("barras del desglose", () => {
    expect(share(640, 640)).toBe(100);
    expect(share(1, 640)).toBe(4);
    expect(share(5, 0)).toBe(0);
  });
});
