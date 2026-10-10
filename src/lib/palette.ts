// Ctrl+K desde cualquier sitio (el clic derecho, un error, una ayuda) y la
// búsqueda que perdona erratas: «dicsos» encuentra «Discos».

export const PALETTE_EVENT = "adminops-palette";

/** Abre «Todo AdminOps» con algo ya escrito. */
export function openPalette(query = "") {
  window.dispatchEvent(new CustomEvent<string>(PALETTE_EVENT, { detail: query }));
}

/** Distancia entre dos palabras contando cambios, huecos y letras cruzadas («dicsos» → «discos» es 1). */
export function editDistance(a: string, b: string, limit = 3): number {
  if (a === b) return 0;
  if (Math.abs(a.length - b.length) > limit) return limit + 1;
  const d: number[][] = Array.from({ length: a.length + 1 }, (_, i) => [i, ...Array(b.length).fill(0)]);
  for (let j = 1; j <= b.length; j++) d[0][j] = j;
  for (let i = 1; i <= a.length; i++) {
    let best = limit + 1;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
      best = Math.min(best, d[i][j]);
    }
    if (best > limit) return limit + 1;
  }
  return d[a.length][b.length];
}

/** Erratas que se perdonan según lo larga que sea la palabra. */
export const tolerance = (word: string) => (word.length <= 3 ? 0 : word.length <= 6 ? 1 : 2);

/** ¿Alguna palabra de `haystack` se parece a `word` (o empieza igual, con erratas)? */
export function fuzzyHas(haystack: string[], word: string): boolean {
  const tol = tolerance(word);
  if (!tol) return false;
  return haystack.some((h) => editDistance(word, h, tol) <= tol || (h.length > word.length && editDistance(word, h.slice(0, word.length), tol) <= tol));
}

/** Palabras sueltas de un texto de búsqueda (ya normalizado). */
export const tokens = (s: string) => s.split(/[^a-z0-9ñ]+/).filter((w) => w.length > 1);

/**
 * La palabra conocida más parecida a cada una de las que se han escrito, para
 * proponer «¿Quisiste decir…?». Devuelve null si no hay nada mejor que lo escrito.
 */
export function didYouMean(query: string, vocabulary: Set<string>): string | null {
  let changed = false;
  const out = tokens(query).map((w) => {
    if (vocabulary.has(w)) return w;
    let best = w;
    let bestD = tolerance(w) + 1;
    for (const v of vocabulary) {
      const d = editDistance(w, v, bestD - 1);
      if (d < bestD) {
        bestD = d;
        best = v;
      }
    }
    if (best !== w) changed = true;
    return best;
  });
  return changed ? out.join(" ") : null;
}
