import { describe, expect, it } from "vitest";
import { arrange, DEFAULT_ORDER, dropOn, shift } from "./panelLayout";
import { slowdown } from "./slowdown";
import { resultPage, took, withFinished } from "./activity";

describe("Panel a medida", () => {
  it("sin nada guardado sale el orden de fábrica", () => {
    expect(arrange([])).toEqual(DEFAULT_ORDER);
  });

  it("respeta el orden guardado, quita lo que no existe y cuela lo nuevo en su sitio", () => {
    // Guardado antes de que existiera «slow», y con una tarjeta que ya no está.
    const saved = ["quick", "plan", "ya-no-existe", "live", "today", "notes", "machine", "disks", "recent", "quick"];
    const order = arrange(saved);
    expect(order[0]).toBe("quick");
    // Lo guardado mantiene su orden relativo.
    expect(order.filter((b) => saved.includes(b))).toEqual(["quick", "plan", "live", "today", "notes", "machine", "disks", "recent"]);
    expect(new Set(order)).toEqual(new Set(DEFAULT_ORDER));
    expect(order).toHaveLength(DEFAULT_ORDER.length);
    // «slow» va de fábrica detrás de «quick».
    expect(order.indexOf("slow")).toBe(order.indexOf("quick") + 1);
  });

  it("mueve y suelta tarjetas", () => {
    const o = arrange([]);
    expect(shift(o, "today", -1)).toBe(o);
    expect(shift(o, "notes", -1).slice(0, 2)).toEqual(["notes", "today"]);
    expect(dropOn(o, "recent", "today")[0]).toBe("recent");
    const down = dropOn(o, "today", "live");
    expect(down.indexOf("today")).toBe(down.indexOf("live") + 1);
    expect(dropOn(o, "today", "today")).toBe(o);
  });
});

describe("qué frena el equipo", () => {
  const GB = 1024 ** 3;
  const base = { memoryTotal: 16 * GB, memoryUsed: 8 * GB, cpuTotal: 20 };
  const p = (name: string, cpu: number, gb = 0.2) => ({ pid: name.length, name, cpu, memory: gb * GB });

  it("holgado: lo dice, y no señala procesos ligeros", () => {
    const s = slowdown({ ...base, topProcesses: [p("chrome.exe", 3), p("notepad.exe", 1)] });
    expect(s.level).toBe("ok");
    expect(s.hogs).toEqual([]);
    expect(s.headline).toContain("Nada");
  });

  it("procesador alto: dice cuánto y quién, y solo ofrece cerrar lo que es del usuario", () => {
    const s = slowdown({ ...base, cpuTotal: 91, topProcesses: [p("chrome.exe", 48, 2.9), p("MsMpEng.exe", 27), p("notepad.exe", 1)] });
    expect(s.level).toBe("warn");
    expect(s.headline).toBe("Procesador al 91 %");
    expect(s.hogs.map((h) => [h.name, h.closable])).toEqual([
      ["chrome.exe", true],
      ["MsMpEng.exe", false],
    ]);
    expect(s.hogs[1].about).toContain("antivirus");
  });

  it("memoria casi llena es grave, y cuenta quien la ocupa aunque no use procesador", () => {
    const s = slowdown({ ...base, memoryUsed: 15.5 * GB, topProcesses: [p("Teams.exe", 1, 3)] });
    expect(s.level).toBe("bad");
    expect(s.headline).toBe("Memoria al 97 %");
    expect(s.hogs).toHaveLength(1);
  });
});

describe("actividad de la campana", () => {
  it("apunta lo que duró algo o se canceló, lo último primero", () => {
    let list = withFinished([], { task: "diagnostics", name: "Diagnóstico", seconds: 134, cancelled: false, at: 1 });
    list = withFinished(list, { task: "lan-scan", name: "Buscar en la red", seconds: 1, cancelled: false, at: 2 });
    expect(list).toHaveLength(1);
    list = withFinished(list, { task: "update", name: "Actualizar", seconds: 1, cancelled: true, at: 3 });
    expect(list.map((a) => a.task)).toEqual(["update", "diagnostics"]);
    expect(list[0].seen).toBe(false);
  });

  it("sabe dónde ver el resultado y decir cuánto tardó", () => {
    expect(resultPage("diagnostics")).toEqual({ page: "machine", section: "diagnostics" });
    expect(resultPage("disk-speed:E")).toEqual({ page: "space", section: "health" });
    expect(resultPage("uninstall:7zip")).toEqual({ page: "apps", section: "uninstall" });
    // «update» no es «updater-algo»: las claves exactas no casan por prefijo.
    expect(resultPage("updates-raro")).toBeNull();
    expect(resultPage("vault")).toBeNull();
    expect(took(134)).toBe("2 min 14 s");
    expect(took(48)).toBe("48 s");
  });
});
