// Estado del equipo → Teclado e idioma: qué distribución es «la suya» y cuál sobra, cómo se dice el
// atajo de cambio y la prueba de «ñ @ ´». Lo que lee Windows (idiomas, atajo) está en Rust (kbdlang.rs).
import type { KbdRow, KbdState } from "./api";

export type RowKind = "mine" | "extra";

export interface MarkedRow extends KbdRow {
  kind: RowKind;
}

/**
 * La distribución «de la persona»: la primera del idioma que coincide con el país de Windows
 * (es-DO en República Dominicana) y, si ninguno coincide, la primera de la lista. Las demás sobran.
 */
export function markRows(rows: KbdRow[], region: string): MarkedRow[] {
  const country = region.trim().toUpperCase();
  const byRegion = country ? rows.find((r) => r.tag.toUpperCase().endsWith(`-${country}`)) : undefined;
  const mine = byRegion ?? rows[0];
  return rows.map((r) => ({ ...r, kind: r === mine ? "mine" : "extra" }));
}

export const KIND_LABEL: Record<RowKind, string> = { mine: "El suyo", extra: "De más" };

export interface ShortcutLine {
  /** «Alt+Mayús». */
  text: string;
  /** Lo que hace en este equipo. */
  note: string;
  /** El atajo está puesto. */
  on: boolean;
  /** Cambia de idioma sin avisar: hay atajo y más de un idioma. */
  risky: boolean;
}

/** La fila del atajo: «Alt+Mayús · Cambia de idioma sin avisar · Activo». */
export function shortcutLine(s: KbdState): ShortcutLine {
  const on = s.shortcut.code !== "3";
  const several = otherLanguages(s) > 0 || s.rows.length > 1;
  if (!on) return { text: s.shortcut.label, note: "No cambia de idioma", on, risky: false };
  return { text: s.shortcut.label, note: several ? "Cambia de idioma sin avisar" : "Solo hay un idioma: no cambia nada", on, risky: several };
}

/** Idiomas de la lista aparte del español y del que se queda sin teclado por ser el de Windows. */
export function otherLanguages(s: KbdState): number {
  return Math.max(0, s.languageCount - 1 - (s.kept ? 1 : 0));
}

/** ¿Hay algo que quitar? Otro idioma, una distribución de más (también dentro del español) o el atajo puesto. */
export function hasLeftovers(s: KbdState): boolean {
  return s.shortcut.code !== "3" || otherLanguages(s) > 0 || s.rows.length > 1;
}

/** El botón «Dejar solo Español…» solo aparece si hay un español y algo que quitar. Con otra cuenta no se puede cambiar. */
export function canKeepOnlySpanish(s: KbdState): boolean {
  return s.hasSpanish && !s.otherUser && hasLeftovers(s);
}

/** Con AdminOps abierto como otra cuenta, el idioma de este usuario no se cambia desde aquí. */
export const OTHER_USER_SENTENCE = "Con AdminOps abierto como otra cuenta no se puede cambiar el idioma de este usuario: ábrelo con su cuenta.";

// ---------- Probar el teclado ----------

export type KeyTest =
  | { state: "empty" }
  | { state: "ok" }
  /** Sale lo de un teclado en inglés: `seen` son esos caracteres. */
  | { state: "english"; seen: string }
  | { state: "noN" }
  | { state: "noAt" };

/** Lo que sale al pulsar las teclas de la ñ, la @ y la ´ de un teclado español con una distribución inglesa. */
const ENGLISH_SIGNS = new Set([";", "2", "[", "'", "q", "Q"]);

/**
 * Lo escrito al pulsar «ñ @ ´». La @ vale como salga (AltGr+2 en España, AltGr+Q en Latinoamérica);
 * la ´ no se exige porque sola no escribe nada hasta la letra siguiente.
 */
export function analyzeKeyTest(text: string): KeyTest {
  const chars = [...text].filter((c) => !/\s/.test(c));
  if (!chars.length) return { state: "empty" };
  const hasN = chars.some((c) => c === "ñ" || c === "Ñ");
  const hasAt = chars.includes("@");
  if (hasN && hasAt) return { state: "ok" };
  if (hasN) return { state: "noAt" };
  const seen = [...new Set(chars.filter((c) => ENGLISH_SIGNS.has(c)))];
  return seen.length ? { state: "english", seen: seen.join(" ") } : { state: "noN" };
}

/** La frase que se enseña con el resultado de la prueba. */
export function keyTestSentence(t: KeyTest): string {
  switch (t.state) {
    case "empty":
      return "Pulsa: ñ @ ´";
    case "ok":
      return "Sale bien.";
    case "english":
      return `Sale «${t.seen}»: el teclado está en inglés en este programa.`;
    case "noN":
      return "No sale la ñ.";
    case "noAt":
      return "Falta la @: en un teclado español va en AltGr+2 o en AltGr+Q.";
  }
}
