import { describe, expect, it } from "vitest";
import type { ProgramUsage, UsageSnapshot } from "./api";
import { buildView, destinationsText, freeUpText, lastSpeed, mbit, mbitShort, pausedText, rateText, saturates, spanText, todayText, totalText, uploadedText } from "./netusage";

const prog = (id: string, name: string, p: Partial<ProgramUsage> = {}): ProgramUsage => ({
  key: id,
  name,
  path: null,
  upMbps: 0,
  downMbps: 0,
  totalUp: 0,
  totalDown: 0,
  todayUp: 0,
  todayDown: 0,
  watchedSecs: 0,
  destinations: [],
  active: true,
  ...p,
});

const snap = (programs: ProgramUsage[], extra: Partial<UsageSnapshot> = {}): UsageSnapshot => ({
  running: true,
  method: "etw",
  tcpOnly: false,
  error: null,
  programs,
  totalUpMbps: 0,
  totalDownMbps: 0,
  startedMs: 0,
  nowMs: 1,
  onedrivePause: null,
  lostEvents: 0,
  ...extra,
});

const SPEED = { upMbps: 20, downMbps: 200 };

describe("textos de cada fila", () => {
  it("las cifras llevan coma y una decimal", () => {
    expect(mbit(18.4)).toBe("18,4");
    expect(mbit(2.05)).toMatch(/^2,[01]$/);
    expect(mbit(0)).toBe("0,0");
    expect(mbit(Number.NaN)).toBe("0,0");
    expect(mbitShort(20)).toBe("20");
    expect(mbitShort(1.6)).toBe("1,6");
    expect(mbitShort(1.64)).toBe("1,6");
  });

  it("↑, ↓ y ↑↓ según lo que pase de 0,1", () => {
    expect(rateText({ upMbps: 18.4, downMbps: 0 })).toBe("↑ 18,4 Mbit/s");
    expect(rateText({ upMbps: 0, downMbps: 2.1 })).toBe("↓ 2,1 Mbit/s");
    expect(rateText({ upMbps: 0.6, downMbps: 0.6 })).toBe("↑↓ 1,2 Mbit/s");
    // Una de las dos por debajo de 0,1 no cuenta.
    expect(rateText({ upMbps: 2, downMbps: 0.05 })).toBe("↑ 2,0 Mbit/s");
    expect(rateText({ upMbps: 0.05, downMbps: 0.02 })).toBe("menos de 0,1 Mbit/s");
    expect(rateText({ upMbps: 0, downMbps: 0 })).toBe("—");
  });

  it("lo subido se cuenta como se vio, sin prometer lo que falta", () => {
    expect(uploadedText({ upMbps: 18.4, totalUp: 38 * 1024 * 1024, watchedSecs: 240 })).toBe("38 MB subidos en 4 min");
    expect(uploadedText({ upMbps: 18.4, totalUp: 5 * 1024 * 1024, watchedSecs: 30 })).toBe("5 MB subidos en 30 s");
    // Sin subida o con muy poco, nada.
    expect(uploadedText({ upMbps: 0.01, totalUp: 90_000_000, watchedSecs: 240 })).toBeNull();
    expect(uploadedText({ upMbps: 5, totalUp: 200, watchedSecs: 240 })).toBeNull();
    expect(spanText(3900)).toBe("1 h 5 min");
    expect(spanText(7200)).toBe("2 h");
    expect(spanText(-4)).toBe("0 s");
  });

  it("lo de hoy, los destinos y la pausa", () => {
    expect(todayText({ todayUp: 0, todayDown: 0 })).toBe("—");
    expect(todayText({ todayUp: 1024 * 1024 * 1024 * 2, todayDown: 340 * 1024 * 1024 })).toBe("↑ 2 GB · ↓ 340 MB");
    expect(todayText({ todayUp: 1.4 * 1024 * 1024 * 1024, todayDown: 1536 })).toBe("↑ 1.4 GB · ↓ 1.5 KB");
    expect(destinationsText({ destinations: [{ host: "onedrive.live.com", ip: "1.1.1.1", bytes: 1 }, { host: "52.1.1.1", ip: "52.1.1.1", bytes: 1 }] })).toBe("onedrive.live.com · 52.1.1.1");
    expect(destinationsText({ destinations: [] })).toBe("");
    const t = new Date(2026, 10, 22, 13, 10).getTime();
    expect(pausedText(t)).toMatch(/^En pausa hasta las 13[:.]10$/);
    expect(totalText({ totalUpMbps: 21.5, totalDownMbps: 3.9 })).toBe("↑ 21,5 · ↓ 3,9 Mbit/s en total");
  });
});

describe("la última prueba de velocidad", () => {
  it("la más reciente con cifras", () => {
    expect(lastSpeed([])).toBeNull();
    expect(lastSpeed([{ timestamp: 5, uploadMbps: 0, downloadMbps: 0 }])).toBeNull();
    expect(
      lastSpeed([
        { timestamp: 1, uploadMbps: 5, downloadMbps: 50 },
        { timestamp: 9, uploadMbps: 20, downloadMbps: 200 },
        { timestamp: 4, uploadMbps: 7, downloadMbps: 70 },
      ]),
    ).toEqual({ upMbps: 20, downMbps: 200 });
    // Si la más reciente salió a cero, se usa la anterior con datos.
    expect(lastSpeed([{ timestamp: 9, uploadMbps: 0, downloadMbps: 0 }, { timestamp: 4, uploadMbps: 7, downloadMbps: 70 }])).toEqual({ upMbps: 7, downMbps: 70 });
  });
});

describe("estados: Satura, Fluida y Pausado", () => {
  it("satura pasando del 70 % de la prueba, la subida o la bajada", () => {
    expect(saturates({ upMbps: 18.4, downMbps: 0 }, SPEED)).toBe("up");
    expect(saturates({ upMbps: 14, downMbps: 0 }, SPEED)).toBeNull();
    expect(saturates({ upMbps: 14.1, downMbps: 0 }, SPEED)).toBe("up");
    expect(saturates({ upMbps: 1, downMbps: 150 }, SPEED)).toBe("down");
    // Las dos: la que más pase.
    expect(saturates({ upMbps: 19, downMbps: 150 }, SPEED)).toBe("up");
    // Sin prueba de velocidad guardada, nunca.
    expect(saturates({ upMbps: 999, downMbps: 999 }, null)).toBeNull();
    expect(saturates({ upMbps: 5, downMbps: 5 }, { upMbps: 0, downMbps: 0 })).toBeNull();
  });

  it("la maqueta: OneDrive satura la subida con la barra al 92 %, Teams no tiene estado mientras algo sature", () => {
    const v = buildView(
      snap([
        prog("onedrive.exe", "OneDrive", { upMbps: 18.4, totalUp: 38 * 1024 * 1024, watchedSecs: 240 }),
        prog("chrome.exe", "Chrome", { downMbps: 2.1 }),
        prog("ms-teams.exe", "Teams", { upMbps: 0.6, downMbps: 0.6 }),
        prog("svchost:wu", "Windows Update", { downMbps: 0.3 }),
      ]),
      SPEED,
    );
    expect(v.rows.map((r) => r.p.name)).toEqual(["OneDrive", "Chrome", "Teams", "Windows Update"]);
    const one = v.rows[0];
    expect(one.state).toEqual({ kind: "saturaSubida", label: "Satura la subida", tone: "bad" });
    expect(one.bar).toEqual({ pct: 92, dir: "up" });
    expect(v.rows[1].state).toBeNull();
    // Con algo saturando, Teams no se dice «Fluida».
    expect(v.rows[2].state).toBeNull();
    expect(v.hasSpeed).toBe(true);
  });

  it("tras pausar OneDrive: «Pausado», Teams «Fluida» y la subida libre", () => {
    const until = new Date(2026, 10, 22, 13, 10).getTime();
    const v = buildView(
      snap([prog("ms-teams.exe", "Teams", { upMbps: 0.6, downMbps: 0.6 }), prog("chrome.exe", "Chrome", { downMbps: 2.1 })], { onedrivePause: { untilMs: until, entryId: 7 } }),
      SPEED,
    );
    // OneDrive no mueve nada pero su fila se ve.
    const one = v.rows.find((r) => r.p.key === "onedrive.exe")!;
    expect(one.paused).toEqual({ untilMs: until, entryId: 7 });
    expect(one.state).toEqual({ kind: "pausado", label: "Pausado", tone: "mute" });
    expect(one.bar).toBeNull();
    expect(v.rows.find((r) => r.p.name === "Teams")!.state).toEqual({ kind: "fluida", label: "Fluida", tone: "ok" });
    // 20 de línea menos 0,6 que sube Teams.
    expect(v.freeUpMbps).toBeCloseTo(19.4, 5);
    expect(freeUpText(v.freeUpMbps!, SPEED)).toBe("Subida libre: 19,4 de 20 Mbit/s");
    expect(freeUpText(1.6, SPEED)).toBe("Subida libre: 1,6 de 20 Mbit/s");
  });

  it("«Fluida» solo para videollamadas", () => {
    const v = buildView(snap([prog("zoom.exe", "Zoom", { upMbps: 1, downMbps: 1 }), prog("chrome.exe", "Chrome", { downMbps: 3 })]), SPEED);
    expect(v.rows[0].state?.kind).toBe("fluida");
    expect(v.rows[1].state).toBeNull();
  });

  it("sin prueba de velocidad no hay estados de saturación ni subida libre", () => {
    const v = buildView(snap([prog("onedrive.exe", "OneDrive", { upMbps: 500 }), prog("ms-teams.exe", "Teams", { upMbps: 1 })]), null);
    expect(v.rows.every((r) => r.state === null && r.bar === null)).toBe(true);
    expect(v.hasSpeed).toBe(false);
    expect(v.freeUpMbps).toBeNull();
  });

  it("los que no mueven nada ahora van a «Sin tráfico ahora»", () => {
    const v = buildView(snap([prog("a.exe", "A", { upMbps: 1 }), prog("b.exe", "B", { active: false, todayDown: 5 })]), SPEED);
    expect(v.rows.map((r) => r.p.key)).toEqual(["a.exe"]);
    expect(v.quiet.map((p) => p.key)).toEqual(["b.exe"]);
    // Una lista vacía no rompe nada.
    const vacio = buildView(snap([]), SPEED);
    expect(vacio.rows).toEqual([]);
    expect(vacio.quiet).toEqual([]);
    expect(vacio.freeUpMbps).toBe(20);
  });

  it("la barra no pasa del 100 %", () => {
    const v = buildView(snap([prog("onedrive.exe", "OneDrive", { upMbps: 35 })]), SPEED);
    expect(v.rows[0].bar).toEqual({ pct: 100, dir: "up" });
    expect(v.freeUpMbps).toBe(0);
  });
});
