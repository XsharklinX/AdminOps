import { describe, expect, it } from "vitest";
import type { DiskReport, JournalEntry } from "./api";
import { assess, effects, type DiskErrors } from "./diskAssist";

const disk = (over: Partial<DiskReport> = {}): DiskReport =>
  ({
    number: 1,
    model: "ST3500410SV",
    bus: "SATA",
    media: "HDD",
    size: 500e9,
    health: "Healthy",
    system: false,
    temperature: 41,
    hours: 50000,
    readErrors: 0,
    writeErrors: 0,
    wear: -1,
    volumes: [{ letter: "D:", label: "", fs: "NTFS", size: 500e9, free: 1e9, health: "Healthy", dirty: false, system: false, bitlocker: "", bitlockerPercent: 0 }],
    predictFailure: false,
    reallocated: 0,
    pending: 0,
    uncorrectable: 0,
    crcErrors: 0,
    mediaErrors: 0,
    verdict: { level: "ok", title: "", text: "Sin problemas.", advice: [] },
    trend: [],
    rising: [],
    ...over,
  }) as unknown as DiskReport;

const errs = (byKind: DiskErrors["byKind"], days: [string, number][] = []): DiskErrors => ({ disk: 1, total: Object.values(byKind).reduce((a, b) => a + (b ?? 0), 0), byKind, days, lastDay: days.length ? days[days.length - 1][0] : "" });

describe("asistente de disco", () => {
  it("superficie: primero los datos, y cambiar si va a peor", () => {
    const a = assess(disk({ pending: 44, uncorrectable: 44, reallocated: 429, rising: ["Sectores apartados"] }), errs({ surface: 948 }));
    expect(a.cause).toBe("surface");
    expect(a.plan.map((s) => s.id)).toEqual(["rescue", "clone", "scan", "replace"]);
    expect(a.verdict.title).toBe("Cambiar el disco");
    expect(a.evidence.find((e) => e.label.startsWith("Sectores pendientes"))?.tone).toBe("bad");
  });

  it("bloques dañados quietos: vigilar, no cambiar todavía", () => {
    const a = assess(disk({ reallocated: 8 }), errs({ surface: 3 }));
    expect(a.cause).toBe("surface");
    expect(a.verdict.level).toBe("warn");
  });

  it("errores de transmisión sin superficie: es el cable, el disco puede estar bien", () => {
    const a = assess(disk({ crcErrors: 12, rising: ["Errores de conexión"] }), errs({ connection: 5 }));
    expect(a.cause).toBe("connection");
    expect(a.plan[0].id).toBe("cable");
    expect(a.plan.find((s) => s.id === "power")?.action).toEqual({ kind: "tweak", tweak: "performance.disk-link-power" });
  });

  it("en un disco USB, el arreglo es no apagar los USB", () => {
    const a = assess(disk({ bus: "USB" }), errs({ retry: 30 }));
    expect(a.cause).toBe("power");
    expect(a.plan[0].action).toEqual({ kind: "tweak", tweak: "performance.usb-suspend" });
  });

  it("solo reintentos o reinicios de la controladora: energía o driver", () => {
    expect(assess(disk(), errs({ retry: 120 })).cause).toBe("power");
    expect(assess(disk(), undefined, 6).cause).toBe("power");
  });

  it("sistema de archivos dañado: comprobar y reparar esa unidad", () => {
    const a = assess(disk(), errs({ fileSystem: 2 }));
    expect(a.cause).toBe("filesystem");
    expect(a.plan.map((s) => s.action)).toEqual([
      { kind: "check", letter: "D:" },
      { kind: "repair", letter: "D:" },
    ]);
  });

  it("sin señales, sano y sin plan", () => {
    const a = assess(disk(), undefined);
    expect(a.cause).toBe("none");
    expect(a.plan).toEqual([]);
  });
});

describe("¿funcionó el arreglo?", () => {
  const DAY = 86400;
  const at = Date.UTC(2026, 9, 10) / 1000;
  const entry = (over: Partial<JournalEntry> = {}): JournalEntry => ({ id: 1, timestamp: at, op: "apply", tweakId: "performance.disk-link-power", title: "Sin ahorro en el enlace", ok: true, message: null, reverted: false, undoable: true, ...over });
  const days: [string, number][] = [
    ["2026-10-05", 30],
    ["2026-10-06", 28],
    ["2026-10-08", 35],
    ["2026-10-09", 31],
    ["2026-10-11", 1],
  ];

  it("los errores caen tras el arreglo: funcionó", () => {
    const [e] = effects(errs({ retry: 125 }, days), [entry()], at + 6 * DAY, ["performance.disk-link-power"]);
    expect(e.beforePerDay).toBeCloseTo(124 / 7);
    expect(e.afterPerDay).toBeCloseTo(1 / 6);
    expect(e.verdict).toBe("works");
  });

  it("los errores siguen igual: no cambió nada", () => {
    const same: [string, number][] = [...days, ["2026-10-12", 30], ["2026-10-13", 29]];
    expect(effects(errs({ retry: 1 }, same), [entry()], at + 4 * DAY, ["performance.disk-link-power"])[0].verdict).toBe("no-change");
  });

  it("menos de un día: aún no se sabe; y lo deshecho no cuenta", () => {
    expect(effects(errs({}, days), [entry()], at + 3600, ["performance.disk-link-power"])[0].verdict).toBe("too-soon");
    expect(effects(errs({}, days), [entry({ reverted: true })], at + 6 * DAY, ["performance.disk-link-power"])).toEqual([]);
    expect(effects(errs({}, days), [entry({ tweakId: "performance.otro" })], at + 6 * DAY, ["performance.disk-link-power"])).toEqual([]);
  });
});
