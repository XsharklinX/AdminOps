import { describe, expect, it } from "vitest";
import type { Sample, TimelineEvent } from "./api";
import { blame, describeStep, findStepUps, relevant } from "./perfCorrelate";

const T0 = 1_760_000_000;
/** Un minuto por muestra: procesador bajo hasta `jump`, alto después. */
function samples(n: number, jump: number, lo = 10, hi = 60): Sample[] {
  return Array.from({ length: n }, (_, i) => ({ t: T0 + i * 60, cpu: i < jump ? lo : hi, ram: 40, disk: 30 }));
}
const ev = (minutes: number, title: string, kind: TimelineEvent["kind"] = "windows", level: TimelineEvent["level"] = "info", detail = ""): TimelineEvent => ({ time: T0 + minutes * 60, kind, level, title, detail, page: null });

describe("saltos del rendimiento", () => {
  it("encuentra el salto del procesador donde ocurre", () => {
    const steps = findStepUps(samples(180, 90));
    expect(steps).toHaveLength(1);
    expect(steps[0].metric).toBe("cpu");
    // El salto más claro es el que deja 20 min bajos a un lado y altos al otro.
    expect(Math.abs(steps[0].at - (T0 + 90 * 60))).toBeLessThanOrEqual(60);
    expect(steps[0].after - steps[0].before).toBeGreaterThanOrEqual(45);
  });

  it("no inventa saltos en un equipo estable ni con huecos", () => {
    expect(findStepUps(samples(180, 0, 30, 30))).toEqual([]);
    // El salto cae en un hueco de AdminOps cerrada: no se puede saber cuándo empezó.
    const gap = samples(180, 90).map((s, i) => (i >= 90 ? { ...s, t: s.t + 6 * 3600 } : s));
    expect(findStepUps(gap)).toEqual([]);
  });

  it("junta el salto con el suceso de justo antes", () => {
    const steps = findStepUps(samples(180, 90));
    const events = [ev(10, "Actualización instalada", "windows", "ok"), ev(80, "Programa instalado", "windows", "info", "Updater Pro 2.1"), ev(85, "Windows arrancó")];
    const b = blame(steps, events)[0];
    expect(b.event?.title).toBe("Programa instalado");
    expect(b.minutes).toBeGreaterThan(0);
    expect(describeStep(b)).toMatch(/procesador pasó de 10 % a 60 %/i);
    expect(describeStep(b)).toContain("Updater Pro 2.1");
  });

  it("dice que no hay nada si no hay suceso cerca", () => {
    const steps = findStepUps(samples(180, 90));
    const b = blame(steps, [ev(-300, "Programa instalado")])[0];
    expect(b.event).toBeNull();
    expect(describeStep(b)).toContain("No hay nada registrado");
  });

  it("los arranques y apagados no cuentan como causa", () => {
    expect(relevant(ev(0, "Windows arrancó"))).toBe(false);
    expect(relevant(ev(0, "Apagado inesperado", "windows", "warn"))).toBe(false);
    expect(relevant(ev(0, "Aplicado: desactivar telemetría", "change", "ok"))).toBe(true);
    expect(relevant(ev(0, "Diagnóstico", "scan"))).toBe(false);
  });
});
