import { describe, expect, it } from "vitest";
import { addPhrase, addRecent, nameMatches, planStep, stepsSummary, undoSteps, type QuickEntry } from "./quickActions";
import { parseSteps } from "./recognize";

describe("acciones de Ctrl+K", () => {
  it("encuentra la impresora sin tildes ni mayúsculas", () => {
    expect(nameMatches("HP LaserJet · Recepción", "recepcion")).toBe(true);
    expect(nameMatches("HP LaserJet · Recepción", "recepción hp")).toBe(true);
    expect(nameMatches("Epson Contabilidad", "recepcion")).toBe(false);
    expect(nameMatches("x", "")).toBe(false);
  });

  it("«Hace poco» guarda las últimas cinco sin repetir", () => {
    let l = [] as ReturnType<typeof addRecent>;
    for (let i = 0; i < 7; i++) l = addRecent(l, { label: `A${i}`, query: `q${i}` });
    expect(l).toHaveLength(5);
    expect(l[0].label).toBe("A6");
    l = addRecent(l, { label: "A4", query: "q4" });
    expect(l[0].label).toBe("A4");
    expect(l.filter((r) => r.label === "A4")).toHaveLength(1);
  });

  it("las últimas frases: 20, la más reciente primero, sin repetir ni vacías", () => {
    let l: string[] = [];
    for (let i = 0; i < 25; i++) l = addPhrase(l, `frase ${i}`);
    expect(l).toHaveLength(20);
    expect(l[0]).toBe("frase 24");
    l = addPhrase(l, "frase 10");
    expect(l[0]).toBe("frase 10");
    expect(l.filter((p) => p === "frase 10")).toHaveLength(1);
    expect(addPhrase(l, "   ")).toEqual(l);
  });

  it("un paso solo se hará si se encontró una única cosa", () => {
    const [step] = parseSteps("liberar espacio y vaciar cola") ?? [];
    const e = { key: "k", title: "t" } as QuickEntry;
    expect(planStep(step, [e]).entry).toBe(e);
    expect(planStep(step, []).problem).toMatch(/no encuentro/);
    expect(planStep(step, [e, e]).problem).toMatch(/2 coincidencias/);
  });

  it("resumen y deshacer varios pasos, del último al primero", async () => {
    const order: string[] = [];
    const results = [
      { ok: true, text: "a", undo: async () => (order.push("a"), "a") },
      { ok: false, text: "b" },
      { ok: true, text: "c", undo: async () => (order.push("c"), "c") },
    ];
    expect(stepsSummary(results)).toBe("2 de 3 hechos");
    const u = await undoSteps(results);
    expect(order).toEqual(["c", "a"]);
    expect(u).toEqual({ ok: true, text: "Deshechos 2 pasos" });
  });
});
