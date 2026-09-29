import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Cada prueba con el módulo recién cargado (su estado es global).
async function fresh() {
  vi.resetModules();
  return import("./perf");
}

describe("medición de páginas", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("da la página por lista cuando terminan sus consultas", async () => {
    const perf = await fresh();
    perf.pageOpened("hardware");
    let resolve!: (v: number) => void;
    const call = perf.trackCall("hardware_inventory", new Promise<number>((r) => (resolve = r)));
    vi.advanceTimersByTime(50);
    perf.pagePainted("hardware");
    vi.advanceTimersByTime(500);
    // La consulta sigue en marcha: aún no está lista.
    expect(perf.pageLoads()).toHaveLength(0);
    vi.advanceTimersByTime(700);
    resolve(1);
    await call;
    vi.advanceTimersByTime(250);
    const [load] = perf.pageLoads();
    expect(load.page).toBe("hardware");
    expect(load.paintMs).toBe(50);
    expect(load.readyMs).toBeGreaterThanOrEqual(1250);
    expect(load.calls[0].cmd).toBe("hardware_inventory");
    expect(perf.commandStats()[0]).toMatchObject({ cmd: "hardware_inventory", count: 1 });
  });

  it("sin consultas, está lista al pintarse", async () => {
    const perf = await fresh();
    perf.pageOpened("shortcuts");
    vi.advanceTimersByTime(30);
    perf.pagePainted("shortcuts");
    vi.advanceTimersByTime(250);
    expect(perf.pageLoads()[0]).toMatchObject({ page: "shortcuts", paintMs: 30, readyMs: 30 });
  });

  it("no duplica la misma apertura (StrictMode) ni mide el pintado de otra página", async () => {
    const perf = await fresh();
    perf.pageOpened("dashboard", 0);
    perf.pageOpened("dashboard", 0);
    perf.pagePainted("otra");
    vi.advanceTimersByTime(300);
    expect(perf.pageLoads()).toHaveLength(0);
    perf.pagePainted("dashboard");
    vi.advanceTimersByTime(300);
    expect(perf.pageLoads()).toHaveLength(1);
  });
});
