import { describe, expect, it, vi } from "vitest";
import type { FaultMark, FaultRecMonitor, FaultRecording } from "./api";
import { areaPath, axisStep, bucketMax, axisTicks, bufferPercent, caseNote, rangeLabel, rateLabel, recSummary, tracksOf, clock, dragRegion, originLabel, watchSince, eraseWhen, getRecOptions, inputLine, inputOverlay, KIND_STYLE, lengthLabel, markLine, markPercent, marksCount, monitorLabel, pickRecording, pillText, regionLabel, remaining, seekTime, setRecOptions, sortedMarks, strokeAlpha, videoBox, whenLabel } from "./faultrec";

const mark = (p: Partial<FaultMark> = {}): FaultMark => ({ t: 24, kind: "hang", title: "Excel no responde", detail: "", ...p });
const rec = (p: Partial<FaultRecording> = {}): FaultRecording => ({ id: "fr-1", started: 1_700_000_000, secs: 72, monitor: "Monitor principal", width: 1920, height: 1080, caseId: null, marks: [], bytes: 1, note: null, ...p });

describe("tiempos", () => {
  it("minutos y segundos", () => {
    expect(clock(0)).toBe("0:00");
    expect(clock(41)).toBe("0:41");
    expect(clock(72)).toBe("1:12");
    expect(clock(120)).toBe("2:00");
    expect(clock(-5)).toBe("0:00");
    expect(clock(Number.NaN)).toBe("0:00");
    expect(clock(41.9)).toBe("0:41");
  });

  it("cuánto dura", () => {
    expect(lengthLabel(45)).toBe("45 s");
    expect(lengthLabel(72)).toBe("1 min 12 s");
    expect(lengthLabel(120)).toBe("2 min");
    expect(lengthLabel(0)).toBe("0 s");
  });

  it("el contador flotante y lo que queda", () => {
    expect(pillText(41)).toBe("Grabando 0:41");
    expect(remaining(41)).toBe(79);
    expect(remaining(500)).toBe(0);
  });
});

describe("marcas", () => {
  it("su posición en la barra queda entre 0 y 100", () => {
    expect(markPercent(36, 72)).toBe(50);
    expect(markPercent(0, 72)).toBe(0);
    expect(markPercent(99, 72)).toBe(100);
    expect(markPercent(-3, 72)).toBe(0);
    expect(markPercent(10, 0)).toBe(0);
  });

  it("saltar a una marca empieza dos segundos antes, sin pasarse del principio", () => {
    expect(seekTime(24)).toBe(22);
    expect(seekTime(1)).toBe(0);
    expect(seekTime(0)).toBe(0);
  });

  it("disco y procesador en ámbar; no responde y errores en rojo", () => {
    expect(KIND_STYLE.disk.dot).toBe("bg-warn");
    expect(KIND_STYLE.cpu.text).toBe("text-warn");
    expect(KIND_STYLE.hang.dot).toBe("bg-bad");
    expect(KIND_STYLE.error.text).toBe("text-bad");
  });

  it("la línea de una marca y el orden", () => {
    expect(markLine(mark({ t: 13, title: "Disco al 100 %" }))).toBe("0:13 · Disco al 100 %");
    const sorted = sortedMarks([mark({ t: 43 }), mark({ t: 13 }), mark({ t: 24 })]);
    expect(sorted.map((m) => m.t)).toEqual([13, 24, 43]);
    const original = [mark({ t: 5 }), mark({ t: 2 })];
    sortedMarks(original);
    expect(original.map((m) => m.t)).toEqual([5, 2]);
  });

  it("cuántas marcas", () => {
    expect(marksCount(0)).toBe("sin marcas");
    expect(marksCount(1)).toBe("1 marca");
    expect(marksCount(3)).toBe("3 marcas");
  });
});

describe("eje", () => {
  it("el paso depende de lo larga que sea", () => {
    expect(axisStep(10)).toBe(5);
    expect(axisStep(30)).toBe(10);
    expect(axisStep(72)).toBe(15);
    expect(axisStep(120)).toBe(30);
  });

  it("marcas del eje de una grabación de 1:12", () => {
    const ticks = axisTicks(72);
    expect(ticks.map((t) => t.label)).toEqual(["0:00", "0:15", "0:30", "0:45", "1:00"]);
    expect(ticks[0].pct).toBe(0);
    expect(ticks.every((t) => t.pct >= 0 && t.pct <= 100)).toBe(true);
  });

  it("una grabación vacía no revienta", () => {
    expect(axisTicks(0)).toEqual([{ t: 0, pct: 0, label: "0:00" }]);
  });
});

describe("textos de la pantalla", () => {
  it("cuándo se borra", () => {
    expect(eraseWhen(true)).toBe("al cerrar el caso");
    expect(eraseWhen(false)).toBe("al cerrar AdminOps");
  });

  const mon = (p: Partial<FaultRecMonitor> = {}): FaultRecMonitor => ({ id: "m1", name: "DELL U2419H", number: 1, primary: true, width: 1920, height: 1080, ...p });
  it("el monitor en el selector", () => {
    expect(monitorLabel(mon(), 1)).toBe("Monitor principal");
    expect(monitorLabel(mon(), 2)).toBe("Monitor principal · DELL U2419H");
    expect(monitorLabel(mon({ primary: false, number: 2 }), 2)).toBe("Monitor 2 · DELL U2419H");
  });

  it("hoy, ayer y otro día", () => {
    const now = new Date(2026, 10, 23, 12, 0, 0);
    const at = (d: Date) => Math.floor(d.getTime() / 1000);
    expect(whenLabel(at(new Date(2026, 10, 23, 10, 42)), now)).toMatch(/^Hoy 10[:.]42/);
    expect(whenLabel(at(new Date(2026, 10, 22, 18, 5)), now)).toMatch(/^Ayer 18[:.]05/);
    expect(whenLabel(at(new Date(2026, 9, 3, 9, 30)), now)).toMatch(/^3 oct\.? 09[:.]30/);
    expect(whenLabel(Number.NaN, now)).toBe("");
  });
});

describe("elegir grabación", () => {
  it("la pedida o, si no está, la más reciente", () => {
    const list = [rec({ id: "fr-1", started: 100 }), rec({ id: "fr-3", started: 300 }), rec({ id: "fr-2", started: 200 })];
    expect(pickRecording(list, "fr-2")?.id).toBe("fr-2");
    expect(pickRecording(list, "fr-9")?.id).toBe("fr-3");
    expect(pickRecording(list)?.id).toBe("fr-3");
    expect(pickRecording([])).toBeNull();
  });
});

describe("G2: zona, clics y teclas", () => {
  it("la zona se dibuja en cualquier sentido, a píxeles físicos, dentro del monitor y par", () => {
    expect(dragRegion({ x: 300, y: 200 }, { x: 100, y: 50 }, 1.5, 3840, 2160)).toEqual({ x: 150, y: 75, w: 300, h: 224 });
    // Fuera del monitor se recorta.
    expect(dragRegion({ x: -20, y: -20 }, { x: 5000, y: 5000 }, 1, 1920, 1080)).toEqual({ x: 0, y: 0, w: 1920, h: 1080 });
    // Demasiado pequeña.
    expect(dragRegion({ x: 0, y: 0 }, { x: 40, y: 400 }, 1, 1920, 1080)).toBeNull();
    // Escala rara: se trata como 1.
    expect(dragRegion({ x: 0, y: 0 }, { x: 100, y: 100 }, 0, 1920, 1080)).toEqual({ x: 0, y: 0, w: 100, h: 100 });
  });

  it("nombres de la zona y de cada clic o tecla", () => {
    expect(regionLabel({ w: 800, h: 600 })).toBe("Zona de 800 × 600");
    expect(inputLine({ t: 18_400, label: "Ctrl + P" })).toBe("0:18 · Ctrl + P");
  });

  it("encima del vídeo: los clics recientes y las últimas teclas", () => {
    const ev = [
      { t: 1000, kind: "click" as const, x: 0.5, y: 0.5, label: "Clic" },
      { t: 1200, kind: "keys" as const, x: 0, y: 0, label: "Ctrl + P" },
      { t: 1900, kind: "keys" as const, x: 0, y: 0, label: "Intro" },
      { t: 5000, kind: "click" as const, x: 0.1, y: 0.2, label: "Clic" },
    ];
    const at = inputOverlay(ev, 1350);
    expect(at.clicks.map((c) => c.e.t)).toEqual([1000]);
    expect(at.clicks[0].age).toBeCloseTo(0.5, 5);
    expect(at.keys?.label).toBe("Ctrl + P");
    expect(inputOverlay(ev, 2000).keys?.label).toBe("Intro");
    expect(inputOverlay(ev, 2000).clicks).toEqual([]);
    // Lo que aún no ha pasado no se ve.
    expect(inputOverlay(ev, 900)).toEqual({ clicks: [], keys: null });
    expect(inputOverlay(ev, 4000).keys).toBeNull();
  });

  it("la imagen dentro del reproductor, con franjas", () => {
    expect(videoBox(1000, 500, 1920, 1080)).toEqual({ left: (1000 - 1920 * (500 / 1080)) / 2, top: 0, width: 1920 * (500 / 1080), height: 500 });
    expect(videoBox(800, 800, 1600, 800)).toEqual({ left: 0, top: 200, width: 800, height: 400 });
    expect(videoBox(0, 0, 0, 0)).toEqual({ left: 0, top: 0, width: 0, height: 0 });
  });

  it("un trazo dibujado se ve entero, se desvanece al final y se borra a los 3 s", () => {
    expect(strokeAlpha(0)).toBe(1);
    expect(strokeAlpha(-5)).toBe(1);
    expect(strokeAlpha(2000)).toBe(1);
    expect(strokeAlpha(2600)).toBe(1);
    expect(strokeAlpha(2800)).toBeCloseTo(0.5, 5);
    expect(strokeAlpha(2999)).toBeGreaterThan(0);
    expect(strokeAlpha(2999)).toBeLessThan(0.01);
    expect(strokeAlpha(3000)).toBe(0);
    expect(strokeAlpha(9000)).toBe(0);
    expect(strokeAlpha(Number.NaN)).toBe(1);
  });

  it("las opciones guardadas se leen con sus valores de serie si faltan o están mal", () => {
    const mem = new Map<string, string>();
    vi.stubGlobal("localStorage", { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v), removeItem: (k: string) => void mem.delete(k) });
    localStorage.removeItem("adminops.faultrec.options");
    expect(getRecOptions()).toEqual({ target: "screen", input: true, mic: false, system: false, camera: false });
    setRecOptions({ target: "region", input: false, mic: true, system: true, camera: true });
    expect(getRecOptions()).toEqual({ target: "region", input: false, mic: true, system: true, camera: true });
    localStorage.setItem("adminops.faultrec.options", JSON.stringify({ target: "x", mic: "sí" }));
    expect(getRecOptions()).toEqual({ target: "screen", input: true, mic: false, system: false, camera: false });
    vi.unstubAllGlobals();
  });
});

describe("G3: vigilar", () => {
  it("cuánto del último minuto hay en memoria", () => {
    expect(bufferPercent(0)).toBe(0);
    expect(bufferPercent(30)).toBe(50);
    expect(bufferPercent(75)).toBe(100);
    expect(bufferPercent(Number.NaN)).toBe(0);
  });

  it("de dónde salió cada grabación", () => {
    expect(originLabel({ origin: "auto", trigger: "Excel no responde" })).toBe("Guardada sola · Excel no responde");
    expect(originLabel({ origin: "auto", trigger: null })).toBe("Guardada sola");
    expect(originLabel({ origin: "replay" })).toBe("El último minuto");
    expect(originLabel({ origin: "record" })).toBe("");
    expect(originLabel({})).toBe("");
  });

  it("desde cuándo se vigila", () => {
    expect(watchSince(null)).toBe("Vigilando");
    expect(watchSince(1_700_000_000)).toMatch(/^Vigilando desde las \d{1,2}:\d{2}$/);
  });
});

describe("G4: pistas, recorte y lista", () => {
  const samples = [
    { t: 0, cpu: 20, disk: 5, mem: 60, net: 1024 * 300 },
    { t: 1, cpu: 95, disk: 100, mem: 61, net: 2 * 1024 * 1024 },
    { t: 3, cpu: 30, disk: null, mem: 61, net: 0 },
  ];

  it("las pistas van alineadas por segundo y la red escala a lo más alto", () => {
    const t = tracksOf(samples, 4);
    expect(t.map((x) => x.key)).toEqual(["cpu", "disk", "mem", "net"]);
    expect(t[0].values).toEqual([20, 95, null, 30]);
    expect(t[3].max).toBe(2 * 1024 * 1024);
    expect(t[3].format(1024 * 1024 * 1.5)).toBe("1,5 MB/s");
    expect(t[0].format(34.4)).toBe("34 %");
    // Sin datos de una pista, no sale.
    expect(tracksOf([{ t: 0, cpu: 10, disk: null, mem: null, net: null }], 2).map((x) => x.key)).toEqual(["cpu"]);
  });

  it("la voz larga se junta en tramos con su valor más alto", () => {
    expect(bucketMax([1, 5, 2, 8, 3, 0], 3)).toEqual([5, 8, 3]);
    expect(bucketMax([1, 2], 5)).toEqual([1, 2]);
    expect(bucketMax(Array.from({ length: 720 }, (_, i) => i % 10), 160)).toHaveLength(160);
  });

  it("el área de una pista", () => {
    expect(areaPath([50, null], 100, 10)).toBe("M0 10 L0 5 L1 5 L1 10 L2 10 L2 10 Z");
    expect(areaPath([], 100)).toBe("");
    expect(rateLabel(340 * 1024)).toBe("340 KB/s");
  });

  it("recorte, resumen de la lista y nota del caso", () => {
    expect(rangeLabel(27, 13)).toBe("0:13 → 0:27 · 14 s");
    const r = {
      started: 1_700_000_000,
      origin: "record" as const,
      trigger: null,
      marks: [mark({ t: 14, kind: "disk", title: "Disco al 95 %", probable: true }), mark({ t: 22, title: "Excel no responde", app: true })],
    };
    expect(recSummary(r)).toEqual({ what: "Excel no responde", probable: "Causa probable: disco al 95 %" });
    expect(recSummary({ marks: [], trigger: "Procesador al 100 % · 38 s", origin: "auto" }).what).toBe("Procesador al 100 % · 38 s");
    expect(recSummary({ marks: [], trigger: null, origin: "record" })).toEqual({ what: "Sin marcas", probable: null });
    const note = caseNote(r, 13, 27);
    expect(note).toMatch(/^Grabación del fallo \(.+, 0:13 → 0:27 · 14 s\): Excel no responde; causa probable: disco al 95 %\.$/);
    expect(caseNote({ ...r, marks: [] }, 0, 5)).toMatch(/^Grabación del fallo \(.+\)\.$/);
  });
});
