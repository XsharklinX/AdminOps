// «Mantén Alt y cada botón dice su letra»: reparto de letras. Es una función pura:
// el mismo DOM (los mismos textos en el mismo orden) da siempre las mismas
// letras. Los códigos nunca son prefijo unos de otros, así que en cuanto se
// escribe uno completo se sabe a qué botón es.

export interface TipInput {
  /** Lo que dice el botón (para sacar su letra). */
  text: string;
  /** Letra fija (`data-kt`): una o dos letras o cifras. */
  kt?: string;
  /** Entrada de la barra lateral: son muchas, llevan dos letras. */
  side?: boolean;
}

const ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/** Mayúsculas, sin tildes y solo A-Z, 0-9 y espacios. */
export function plain(s: string): string {
  return s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toUpperCase()
    .replace(/[^A-Z0-9 ]+/g, " ");
}

/** Letras a probar para un texto: la inicial de cada palabra, luego todas sus letras, luego el resto. */
export function candidates(text: string): string[] {
  const t = plain(text);
  const words = t.split(/\s+/).filter(Boolean);
  const out: string[] = [];
  const add = (c: string) => {
    if (c && !out.includes(c)) out.push(c);
  };
  words.forEach((w) => add(w[0]));
  for (const c of t.replace(/ /g, "")) add(c);
  for (const c of ALPHABET) add(c);
  return out;
}

const validKt = (k: string | undefined): k is string => !!k && /^[A-Za-z0-9]{1,2}$/.test(k);

/** ¿Choca el código con alguno ya puesto (igual, o uno prefijo del otro)? */
function clashes(code: string, taken: Iterable<string>): boolean {
  for (const t of taken) if (t.startsWith(code) || code.startsWith(t)) return true;
  return false;
}

/**
 * Letra de cada elemento (en el orden en que vienen), o null si no queda ninguna.
 * Primero las fijas (`kt`), luego los botones sueltos con una letra y por último
 * la barra lateral con dos.
 */
export function assignCodes(items: TipInput[]): (string | null)[] {
  const codes: (string | null)[] = items.map(() => null);
  const taken = new Set<string>();
  // 1. Las fijas, por orden; si dos piden la misma, la primera se queda.
  items.forEach((it, i) => {
    if (!validKt(it.kt)) return;
    const c = it.kt.toUpperCase();
    if (clashes(c, taken)) return;
    codes[i] = c;
    taken.add(c);
  });
  // 2. Los botones de la pantalla: una letra.
  items.forEach((it, i) => {
    if (codes[i] || it.side) return;
    const c = candidates(it.text).find((x) => !clashes(x, taken));
    if (!c) return;
    codes[i] = c;
    taken.add(c);
  });
  // 3. La barra lateral: dos letras, con una primera letra que no sea ya un botón suelto.
  items.forEach((it, i) => {
    if (codes[i] || !it.side) return;
    const cand = candidates(it.text);
    for (const a of cand) {
      if (taken.has(a)) continue;
      const b = cand.find((x) => !taken.has(a + x));
      if (!b) continue;
      const c = a + b;
      if (clashes(c, [...taken].filter((t) => t.length === 1))) continue;
      codes[i] = c;
      taken.add(c);
      return;
    }
  });
  return codes;
}

/** Qué hacer al pulsar una letra: elegir uno, quedarse con los que empiezan por ella o nada. */
export function pressCode(codes: (string | null)[], typed: string): { pick: number } | { typed: string } | null {
  const exact = codes.findIndex((c) => c === typed);
  if (exact >= 0) return { pick: exact };
  if (codes.some((c) => c && c.startsWith(typed))) return { typed };
  return null;
}

/** Letras para las pestañas de una página: la inicial de cada una si no se repite. */
export function tabLetters(labels: string[]): (string | undefined)[] {
  const used = new Set<string>();
  return labels.map((l) => {
    const c = plain(l).trim()[0];
    if (!c || used.has(c)) return undefined;
    used.add(c);
    return c;
  });
}
