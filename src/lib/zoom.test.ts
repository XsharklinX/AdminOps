import { describe, expect, it } from "vitest";
import { nextZoom, zoomFor, zoomKeyDir, zoomPercent, ZOOM_STEPS } from "./zoom";

describe("tamaño de la interfaz", () => {
  it("sube y baja por los pasos 80, 90, 100, 110, 125, 140 y 150 %", () => {
    expect(ZOOM_STEPS).toEqual([0.8, 0.9, 1, 1.1, 1.25, 1.4, 1.5]);
    expect(nextZoom(1, 1)).toBe(1.1);
    expect(nextZoom(1.1, 1)).toBe(1.25);
    expect(nextZoom(1, -1)).toBe(0.9);
    expect(nextZoom(0.9, -1)).toBe(0.8);
  });

  it("no pasa de los extremos y Ctrl+0 vuelve al 100 %", () => {
    expect(nextZoom(1.5, 1)).toBe(1.5);
    expect(nextZoom(0.8, -1)).toBe(0.8);
    expect(nextZoom(1.4, 0)).toBe(1);
  });

  it("un valor raro se acerca al paso más próximo", () => {
    expect(nextZoom(1.2, 1)).toBe(1.4);
    expect(nextZoom(1.07, -1)).toBe(1);
  });

  it("cada monitor tiene el suyo y, si no, el general", () => {
    const by = { DISPLAY1: 1.25 };
    expect(zoomFor(1, by, "DISPLAY1")).toBe(1.25);
    expect(zoomFor(1.1, by, "DISPLAY2")).toBe(1.1);
    expect(zoomFor(1.1, by, null)).toBe(1.1);
    expect(zoomFor(1, { x: 3 }, "x")).toBe(1);
  });

  it("el porcentaje sale redondeado", () => {
    expect(zoomPercent(1.25)).toBe("125 %");
    expect(zoomPercent(0.9)).toBe("90 %");
  });

  it("las teclas: Ctrl +, Ctrl =, el + del teclado numérico, Ctrl −, Ctrl 0; con Alt (AltGr) o sin Ctrl, nada", () => {
    const k = (key: string, o: Partial<KeyboardEvent> = {}) => zoomKeyDir({ key, code: "", ctrlKey: true, altKey: false, metaKey: false, ...o });
    expect(k("+")).toBe(1);
    expect(k("=")).toBe(1);
    expect(k("+", { code: "NumpadAdd" })).toBe(1);
    expect(k("-")).toBe(-1);
    expect(k("-", { code: "NumpadSubtract" })).toBe(-1);
    expect(k("0")).toBe(0);
    expect(k("0", { code: "Numpad0" })).toBe(0);
    expect(k("+", { ctrlKey: false })).toBeNull();
    expect(k("+", { altKey: true })).toBeNull();
    expect(k("a")).toBeNull();
  });
});
