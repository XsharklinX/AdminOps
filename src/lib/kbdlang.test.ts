import { describe, expect, it } from "vitest";
import type { KbdRow, KbdState } from "./api";
import { analyzeKeyTest, canKeepOnlySpanish, hasLeftovers, keyTestSentence, markRows, OTHER_USER_SENTENCE, shortcutLine } from "./kbdlang";

const row = (tag: string, layout = "Español", active = false): KbdRow => ({ tag, language: tag, layout, klid: null, langId: 0, active });
const state = (p: Partial<KbdState> = {}): KbdState => ({
  rows: [row("es-DO"), row("en-US", "Inglés (EE. UU.)")],
  languageCount: 2,
  region: "DO",
  shortcut: { code: "1", label: "Alt+Mayús" },
  hasSpanish: true,
  otherUser: false,
  kept: null,
  warning: null,
  note: null,
  ...p,
});

describe("cuál es «el suyo» y cuál «de más»", () => {
  it("sin país que coincida, el primero de la lista", () => {
    const m = markRows([row("en-US"), row("es-ES")], "");
    expect(m.map((r) => r.kind)).toEqual(["mine", "extra"]);
  });

  it("el idioma que coincide con el país de Windows, aunque no sea el primero", () => {
    const m = markRows([row("en-US"), row("es-DO")], "do");
    expect(m.map((r) => r.kind)).toEqual(["extra", "mine"]);
  });

  it("de un idioma con dos distribuciones solo la primera es «la suya»", () => {
    const rows = [row("es-DO", "Latinoamericano"), row("es-DO", "Español"), row("en-US")];
    expect(markRows(rows, "DO").map((r) => r.kind)).toEqual(["mine", "extra", "extra"]);
  });

  it("una lista vacía no rompe nada", () => {
    expect(markRows([], "DO")).toEqual([]);
  });
});

describe("la fila del atajo", () => {
  it("con varios idiomas cambia sin avisar", () => {
    expect(shortcutLine(state())).toEqual({ text: "Alt+Mayús", note: "Cambia de idioma sin avisar", on: true, risky: true });
  });

  it("con un solo idioma no cambia nada", () => {
    const s = state({ rows: [row("es-DO")], languageCount: 1 });
    expect(shortcutLine(s)).toMatchObject({ on: true, risky: false, note: "Solo hay un idioma: no cambia nada" });
  });

  it("sin atajo (3) está quitado", () => {
    expect(shortcutLine(state({ shortcut: { code: "3", label: "Sin atajo" } }))).toMatchObject({ on: false, risky: false });
  });
});

describe("cuándo se ofrece dejar solo Español", () => {
  it("con español y algo que quitar", () => {
    expect(canKeepOnlySpanish(state())).toBe(true);
    // Un solo idioma pero con atajo puesto: el atajo también se quita.
    expect(canKeepOnlySpanish(state({ rows: [row("es-DO")], languageCount: 1 }))).toBe(true);
  });

  it("sin español, no", () => {
    expect(canKeepOnlySpanish(state({ hasSpanish: false }))).toBe(false);
  });

  it("si ya está solo en español y sin atajo, no hay nada que quitar", () => {
    const s = state({ rows: [row("es-DO")], languageCount: 1, shortcut: { code: "3", label: "Sin atajo" } });
    expect(hasLeftovers(s)).toBe(false);
    expect(canKeepOnlySpanish(s)).toBe(false);
  });
});

describe("distribuciones de más dentro del español", () => {
  const sinAtajo = { code: "3", label: "Sin atajo" };

  it("un solo idioma con dos distribuciones sí tiene algo que quitar", () => {
    const s = state({ rows: [row("es-DO", "Latinoamericano"), row("es-DO", "Inglés (EE. UU.)")], languageCount: 1, shortcut: sinAtajo });
    expect(hasLeftovers(s)).toBe(true);
    expect(canKeepOnlySpanish(s)).toBe(true);
  });

  it("el idioma de Windows que se queda sin teclado no cuenta como algo que quitar", () => {
    // es-DO con una sola distribución y en-US (el idioma de Windows) ya sin teclado.
    const s = state({ rows: [row("es-DO")], languageCount: 2, kept: "en-US", shortcut: sinAtajo });
    expect(hasLeftovers(s)).toBe(false);
    expect(canKeepOnlySpanish(s)).toBe(false);
    // Pero con otro idioma más, sí.
    expect(hasLeftovers({ ...s, languageCount: 3 })).toBe(true);
  });

  it("con el atajo puesto y el idioma de Windows sin teclado, el atajo no cambia nada", () => {
    const s = state({ rows: [row("es-DO")], languageCount: 2, kept: "en-US" });
    expect(shortcutLine(s)).toMatchObject({ on: true, risky: false });
  });

  it("con AdminOps abierto como otra cuenta no se ofrece cambiar nada", () => {
    expect(canKeepOnlySpanish(state({ otherUser: true }))).toBe(false);
    expect(OTHER_USER_SENTENCE).toBe("Con AdminOps abierto como otra cuenta no se puede cambiar el idioma de este usuario: ábrelo con su cuenta.");
  });
});

describe("la prueba de «ñ @ ´»", () => {
  it("vacío: pide pulsar", () => {
    expect(analyzeKeyTest("")).toEqual({ state: "empty" });
    expect(analyzeKeyTest("  ")).toEqual({ state: "empty" });
    expect(keyTestSentence({ state: "empty" })).toBe("Pulsa: ñ @ ´");
  });

  it("con la ñ y la @ sale bien, la ´ no se exige", () => {
    expect(analyzeKeyTest("ñ@")).toEqual({ state: "ok" });
    expect(analyzeKeyTest("ñ @ ´")).toEqual({ state: "ok" });
    expect(analyzeKeyTest("Ñ@")).toEqual({ state: "ok" });
    expect(keyTestSentence({ state: "ok" })).toBe("Sale bien.");
  });

  it("teclado en inglés: enseña lo que salió", () => {
    const t = analyzeKeyTest(";2[");
    expect(t).toEqual({ state: "english", seen: "; 2 [" });
    expect(keyTestSentence(t)).toBe("Sale «; 2 [»: el teclado está en inglés en este programa.");
  });

  it("en inglés aunque la @ salga (Mayús+2)", () => {
    expect(analyzeKeyTest(";@'")).toEqual({ state: "english", seen: "; '" });
  });

  it("los caracteres repetidos salen una vez", () => {
    expect(analyzeKeyTest(";;2;")).toEqual({ state: "english", seen: "; 2" });
  });

  it("ñ sin @ y @ sin ñ", () => {
    expect(analyzeKeyTest("ñ")).toEqual({ state: "noAt" });
    expect(analyzeKeyTest("@")).toEqual({ state: "noN" });
    expect(keyTestSentence({ state: "noAt" })).toContain("AltGr+2");
    expect(keyTestSentence({ state: "noN" })).toBe("No sale la ñ.");
  });

  it("emojis y letras sueltas no rompen la comparación", () => {
    expect(analyzeKeyTest("😀 abc")).toEqual({ state: "noN" });
  });
});
