// El Panel a medida: qué tarjetas hay, en qué orden y cuáles están ocultas.
// La elección se guarda en las preferencias (por técnico).

/** El Panel es una rejilla de seis columnas: así caben tercios y mitades. */
export const PANEL_COLUMNS = 6;

/** Los anchos que se pueden elegir, en columnas de la rejilla. */
export const WIDTHS = [
  { span: 2, label: "⅓", title: "Un tercio" },
  { span: 3, label: "½", title: "La mitad" },
  { span: 4, label: "⅔", title: "Dos tercios" },
  { span: 6, label: "Todo", title: "Todo el ancho" },
] as const;

export type Span = (typeof WIDTHS)[number]["span"];

export const PANEL_BLOCKS = {
  today: { label: "Pendiente de hoy", span: 6 },
  notes: { label: "Notas de este equipo y esta red", span: 6 },
  live: { label: "En vivo: procesador, memoria, disco y red", span: 6 },
  plan: { label: "Qué hacer ahora", span: 4 },
  quick: { label: "Acciones rápidas", span: 2 },
  slow: { label: "Qué frena el equipo ahora", span: 6 },
  machine: { label: "Este equipo", span: 2 },
  disks: { label: "Discos", span: 2 },
  recent: { label: "Actividad reciente", span: 2 },
} as const satisfies Record<string, { label: string; span: Span }>;

export type PanelBlock = keyof typeof PANEL_BLOCKS;
export const DEFAULT_ORDER = Object.keys(PANEL_BLOCKS) as PanelBlock[];

export interface PanelPrefs {
  order: string[];
  hidden: string[];
  /** Ancho elegido por tarjeta (columnas de seis); lo que no está, el de fábrica. */
  widths: Record<string, number>;
}

/** El ancho de una tarjeta: el elegido si es uno de los válidos, si no el de fábrica. */
export function spanOf(id: PanelBlock, widths: Record<string, number>): Span {
  const chosen = widths[id];
  return WIDTHS.some((w) => w.span === chosen) ? (chosen as Span) : PANEL_BLOCKS[id].span;
}

/**
 * El orden guardado, puesto al día: fuera lo que ya no existe, y lo que se
 * añadió después (una versión nueva trae una tarjeta) entra en su sitio de fábrica.
 */
export function arrange(saved: string[]): PanelBlock[] {
  const known = saved.filter((id, i): id is PanelBlock => id in PANEL_BLOCKS && saved.indexOf(id) === i);
  for (const id of DEFAULT_ORDER) {
    if (known.includes(id)) continue;
    // Detrás de la tarjeta que la precede de fábrica, si está; si no, al principio.
    const before = DEFAULT_ORDER.slice(0, DEFAULT_ORDER.indexOf(id)).reverse().find((b) => known.includes(b));
    known.splice(before ? known.indexOf(before) + 1 : 0, 0, id);
  }
  return known;
}

/** Mueve una tarjeta un puesto hacia delante (-1) o hacia atrás (+1). */
export function shift(order: PanelBlock[], id: PanelBlock, by: -1 | 1): PanelBlock[] {
  const i = order.indexOf(id);
  const j = i + by;
  if (i < 0 || j < 0 || j >= order.length) return order;
  const next = [...order];
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

/** Suelta una tarjeta en el sitio de otra (arrastrando). */
export function dropOn(order: PanelBlock[], id: PanelBlock, target: PanelBlock): PanelBlock[] {
  if (id === target || !order.includes(id) || !order.includes(target)) return order;
  const from = order.indexOf(id);
  const to = order.indexOf(target);
  const next = order.filter((b) => b !== id);
  // Hacia delante cae detrás de la otra; hacia atrás, delante.
  next.splice(from < to ? next.indexOf(target) + 1 : next.indexOf(target), 0, id);
  return next;
}
