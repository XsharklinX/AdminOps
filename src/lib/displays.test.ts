import { describe, expect, it } from "vitest";
import type { DisplayMonitor } from "./api";
import { isNative, layoutBoxes, monitorLine, monitorState, nativeFixLabel, nativeSize, problemCount, secondsLeft, stateSentence, summary } from "./displays";

const mon = (p: Partial<DisplayMonitor> = {}): DisplayMonitor => ({
  id: "m1",
  name: "LG 24MK430",
  maker: "LG",
  model: "24MK430",
  number: 1,
  width: 1920,
  height: 1080,
  x: 0,
  y: 0,
  hz: 60,
  maxHz: 75,
  nativeWidth: 1920,
  nativeHeight: 1080,
  connection: "HDMI",
  scale: 100,
  primary: true,
  rotation: 0,
  ...p,
});

describe("estado de cada monitor", () => {
  it("a su resolución propia, con escala normal y Hz buenos: bien", () => {
    expect(monitorState(mon())).toBe("ok");
    expect(stateSentence(mon())).toBeNull();
  });

  it("en otra resolución: borroso, con la frase de siempre", () => {
    const m = mon({ width: 1280, height: 1024 });
    expect(monitorState(m)).toBe("blurry");
    expect(stateSentence(m)).toBe("Va a una resolución que no es la suya: por eso se ve borroso y estirado.");
  });

  it("girado en vertical sigue siendo su resolución", () => {
    expect(isNative(mon({ width: 1080, height: 1920, rotation: 90 }))).toBe(true);
    expect(monitorState(mon({ width: 1080, height: 1920, rotation: 90 }))).toBe("ok");
  });

  it("sin saber la resolución propia no se acusa a nadie", () => {
    expect(monitorState(mon({ width: 1280, height: 1024, nativeWidth: null, nativeHeight: null }))).toBe("ok");
  });

  it("escala rara: la que Windows no ofrece", () => {
    expect(monitorState(mon({ scale: 137 }))).toBe("oddScale");
    expect(monitorState(mon({ scale: 125 }))).toBe("ok");
    expect(monitorState(mon({ scale: 175 }))).toBe("ok");
    expect(stateSentence(mon({ scale: 137 }))).toContain("137 %");
  });

  it("Hz bajos: por debajo de 50 solo si admite más", () => {
    expect(monitorState(mon({ hz: 30, maxHz: 60 }))).toBe("lowHz");
    expect(monitorState(mon({ hz: 30, maxHz: 30 }))).toBe("ok");
    expect(monitorState(mon({ hz: 30, maxHz: null }))).toBe("ok");
    expect(monitorState(mon({ hz: 50, maxHz: 144 }))).toBe("ok");
    expect(monitorState(mon({ hz: 0, maxHz: 60 }))).toBe("ok");
    expect(stateSentence(mon({ hz: 30, maxHz: 60 }))).toContain("30 Hz");
  });

  it("si hay varias cosas, se dice la más clara: borroso antes que escala o Hz", () => {
    expect(monitorState(mon({ width: 1280, height: 1024, scale: 137, hz: 30, maxHz: 60 }))).toBe("blurry");
    expect(monitorState(mon({ scale: 137, hz: 30, maxHz: 60 }))).toBe("oddScale");
  });
});

describe("la línea de cada monitor", () => {
  it("dice su nativa solo cuando no la usa", () => {
    expect(monitorLine(mon({ width: 1280, height: 1024 }))).toBe("LG 24MK430 · 1280×1024 · su nativa es 1920×1080 · HDMI · 60 Hz");
    expect(monitorLine(mon())).toBe("LG 24MK430 · 1920×1080 · HDMI · 60 Hz");
    expect(monitorLine(mon({ hz: 0 }))).toBe("LG 24MK430 · 1920×1080 · HDMI");
  });

  it("el botón de arreglo pone la resolución propia, y solo si hace falta", () => {
    expect(nativeFixLabel(mon({ width: 1280, height: 1024 }))).toBe("Poner 1920×1080");
    expect(nativeFixLabel(mon())).toBeNull();
    expect(nativeFixLabel(mon({ width: 1280, height: 1024, nativeWidth: null, nativeHeight: null }))).toBeNull();
    expect(nativeSize(mon())).toBe("1920x1080");
    expect(nativeSize(mon({ nativeWidth: null }))).toBeNull();
  });

  it("el resumen cuenta monitores y problemas", () => {
    expect(summary([mon()], false)).toBe("1 monitor");
    expect(summary([mon(), mon({ id: "m2", width: 1280, height: 1024 })], false)).toBe("2 monitores · 1 con algo raro");
    expect(summary([mon(), mon({ id: "m2" })], true)).toBe("2 monitores · duplicados");
    expect(problemCount([mon({ scale: 137 }), mon()])).toBe(1);
  });
});

describe("el dibujo de la disposición", () => {
  const two = [mon({ id: "a", number: 1 }), mon({ id: "b", number: 2, x: 1920, primary: false })];

  it("cada monitor en su sitio y a la misma escala", () => {
    const boxes = layoutBoxes(two, 400, 100, 0);
    expect(boxes).toHaveLength(2);
    const [a, b] = boxes;
    // Caben dos monitores de 1920×1080 en 400×100: manda la altura (100/1080).
    expect(a.height).toBeCloseTo(100);
    expect(b.height).toBeCloseTo(100);
    expect(a.width).toBeCloseTo(b.width);
    expect(b.left).toBeCloseTo(a.left + a.width);
    expect(a.primary).toBe(true);
    expect(b.primary).toBe(false);
  });

  it("queda centrado y dentro del espacio", () => {
    for (const b of layoutBoxes(two, 400, 100, 8)) {
      expect(b.left).toBeGreaterThanOrEqual(8 - 1e-6);
      expect(b.top).toBeGreaterThanOrEqual(8 - 1e-6);
      expect(b.left + b.width).toBeLessThanOrEqual(392 + 1e-6);
      expect(b.top + b.height).toBeLessThanOrEqual(92 + 1e-6);
    }
  });

  it("uno encima de otro y posiciones negativas (un monitor a la izquierda del principal)", () => {
    const boxes = layoutBoxes([mon({ id: "a", x: 0 }), mon({ id: "b", x: -1920, primary: false }), mon({ id: "c", x: 0, y: -1080, primary: false })], 300, 300, 0);
    const byId = Object.fromEntries(boxes.map((b) => [b.id, b]));
    expect(byId.b.left).toBeLessThan(byId.a.left);
    expect(byId.c.top).toBeLessThan(byId.a.top);
  });

  it("sin espacio o sin monitores no dibuja nada, y marca el estado de cada uno", () => {
    expect(layoutBoxes([], 400, 100)).toEqual([]);
    expect(layoutBoxes(two, 0, 100)).toEqual([]);
    expect(layoutBoxes([mon({ width: 0 })], 400, 100)).toEqual([]);
    expect(layoutBoxes([mon({ width: 1280, height: 1024 })], 400, 100)[0].state).toBe("blurry");
  });
});

describe("la cuenta atrás", () => {
  it("sube al segundo siguiente y no baja de cero", () => {
    expect(secondsLeft(15_000, 0)).toBe(15);
    expect(secondsLeft(15_000, 100)).toBe(15);
    expect(secondsLeft(15_000, 14_001)).toBe(1);
    expect(secondsLeft(15_000, 15_000)).toBe(0);
    expect(secondsLeft(15_000, 20_000)).toBe(0);
  });
});
