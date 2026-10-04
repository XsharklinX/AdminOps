// Mapa del espacio: cada carpeta es un rectángulo proporcional a lo que ocupa.
// Algoritmo «squarified» (Bruls, Huizing y van Wijk): filas de rectángulos lo
// más cuadrados posible, que son los que mejor se leen y se pulsan.

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Placed<T> extends Rect {
  item: T;
}

/** Peor proporción de una fila de áreas colocada a lo largo de un lado `side`. */
function worst(row: number[], side: number): number {
  const sum = row.reduce((a, b) => a + b, 0);
  const max = Math.max(...row);
  const min = Math.min(...row);
  const s2 = side * side;
  const sum2 = sum * sum;
  return Math.max((s2 * max) / sum2, sum2 / (s2 * min));
}

/** Coloca `items` (con tamaño > 0) dentro de `box`. Los de tamaño 0 se omiten. */
export function squarify<T>(items: T[], size: (t: T) => number, box: Rect): Placed<T>[] {
  const list = items.map((item) => ({ item, v: Math.max(0, size(item)) })).filter((x) => x.v > 0).sort((a, b) => b.v - a.v);
  const total = list.reduce((a, b) => a + b.v, 0);
  if (!total || box.w <= 0 || box.h <= 0) return [];
  const scale = (box.w * box.h) / total;
  const areas = list.map((x) => ({ item: x.item, a: x.v * scale }));
  const out: Placed<T>[] = [];
  let free = { ...box };
  let i = 0;
  while (i < areas.length) {
    const side = Math.min(free.w, free.h);
    const row: typeof areas = [areas[i]];
    i++;
    while (i < areas.length && worst([...row, areas[i]].map((r) => r.a), side) <= worst(row.map((r) => r.a), side)) {
      row.push(areas[i]);
      i++;
    }
    const sum = row.reduce((a, r) => a + r.a, 0);
    if (free.w >= free.h) {
      // Columna a la izquierda.
      const w = sum / free.h;
      let y = free.y;
      for (const r of row) {
        const h = r.a / w;
        out.push({ item: r.item, x: free.x, y, w, h });
        y += h;
      }
      free = { x: free.x + w, y: free.y, w: free.w - w, h: free.h };
    } else {
      // Fila arriba.
      const h = sum / free.w;
      let x = free.x;
      for (const r of row) {
        const w = r.a / h;
        out.push({ item: r.item, x, y: free.y, w, h });
        x += w;
      }
      free = { x: free.x, y: free.y + h, w: free.w, h: free.h - h };
    }
  }
  return out;
}
