// Ctrl+F: buscar en la pantalla actual. Se marca cada coincidencia (CSS Custom
// Highlight API, sin tocar el contenido de la página) y se salta de una a otra.

/** Una letra sin tilde y en minúscula; conserva la longitud (para las posiciones). */
function plainChar(c: string): string {
  return c.normalize("NFD").charAt(0).toLowerCase();
}

export function plain(s: string): string {
  let out = "";
  for (const c of s) out += c.length === 1 ? plainChar(c) : c;
  return out;
}

/** Dónde empieza cada coincidencia de `query` en `text`, sin mayúsculas ni tildes. */
export function matchOffsets(text: string, query: string, limit = Infinity): number[] {
  const q = plain(query.trim());
  if (!q) return [];
  const t = plain(text);
  const out: number[] = [];
  for (let i = t.indexOf(q); i >= 0 && out.length < limit; i = t.indexOf(q, i + q.length)) out.push(i);
  return out;
}

/** El índice siguiente o anterior, dando la vuelta. */
export function step(index: number, total: number, dir: 1 | -1): number {
  if (!total) return 0;
  return (index + dir + total) % total;
}

const MAX = 2000;

/** Las coincidencias de `query` en el texto visible de `root`, en orden. */
export function findRanges(root: Element, query: string): Range[] {
  const q = query.trim();
  if (!q) return [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (n) => {
      const el = n.parentElement;
      if (!el || !n.nodeValue?.trim()) return NodeFilter.FILTER_REJECT;
      if (el.closest("script,style,[hidden],[aria-hidden='true'],[data-no-find]")) return NodeFilter.FILTER_REJECT;
      // Lo plegado o fuera de la vista no cuenta: no se podría enseñar.
      if (typeof el.checkVisibility === "function" && !el.checkVisibility()) return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  const ranges: Range[] = [];
  for (let n = walker.nextNode(); n && ranges.length < MAX; n = walker.nextNode()) {
    const text = n.nodeValue ?? "";
    for (const at of matchOffsets(text, q, MAX - ranges.length)) {
      const r = document.createRange();
      r.setStart(n, at);
      r.setEnd(n, at + q.length);
      ranges.push(r);
    }
  }
  return ranges;
}
