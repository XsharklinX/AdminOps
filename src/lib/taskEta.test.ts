import { describe, expect, it } from "vitest";
import { humanDuration, pageOfTask, percentOf, remainingSeconds } from "./taskEta";

describe("centro de tareas", () => {
  it("saca el porcentaje del mensaje", () => {
    expect(percentOf("Copiando… 45 %")).toBe(45);
    expect(percentOf("Leyendo 12,5%")).toBe(12.5);
    expect(percentOf("Bloque 100 de 400")).toBe(25);
    expect(percentOf("Archivo 3/4")).toBe(75);
    expect(percentOf("Buscando actualizaciones")).toBeNull();
  });
  it("calcula lo que falta con la velocidad real", () => {
    const s = [
      { at: 0, pct: 10 },
      { at: 10_000, pct: 20 },
    ];
    expect(remainingSeconds(s)).toBe(80);
    expect(remainingSeconds([{ at: 0, pct: 10 }])).toBeNull();
    expect(remainingSeconds([{ at: 0, pct: 10 }, { at: 1000, pct: 30 }])).toBeNull();
    expect(remainingSeconds([{ at: 0, pct: 30 }, { at: 10_000, pct: 20 }])).toBeNull();
  });
  it("usa solo los dos últimos minutos", () => {
    const s = [
      { at: 0, pct: 0 },
      { at: 600_000, pct: 50 },
      { at: 660_000, pct: 60 },
    ];
    expect(remainingSeconds(s)).toBe(240);
  });
  it("escribe la duración", () => {
    expect(humanDuration(45)).toBe("45 s");
    expect(humanDuration(130)).toBe("2 min 10 s");
    expect(humanDuration(3900)).toBe("1 h 5 min");
  });
  it("lleva a la pantalla de cada tarea", () => {
    expect(pageOfTask("disk-clone:1")).toEqual(["space", null]);
    expect(pageOfTask("uninstall:vlc")).toEqual(["apps", "uninstall"]);
    expect(pageOfTask("algo-raro")).toBeNull();
  });
});
