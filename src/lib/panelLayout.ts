// El Panel a medida: qué tarjetas hay, en qué orden y cuáles están ocultas.
// La elección se guarda en las preferencias (por técnico).

export const PANEL_BLOCKS = {
  today: { label: "Pendiente de hoy", span: 3 },
  notes: { label: "Notas de este equipo y esta red", span: 3 },
  live: { label: "En vivo: procesador, memoria, disco y red", span: 3 },
  plan: { label: "Qué hacer ahora", span: 2 },
  quick: { label: "Acciones rápidas", span: 1 },
  slow: { label: "Qué frena el equipo ahora", span: 3 },
  machine: { label: "Este equipo", span: 1 },
  disks: { label: "Discos", span: 1 },
  recent: { label: "Actividad reciente", span: 1 },
} as const;

export type PanelBlock = keyof typeof PANEL_BLOCKS;
export const DEFAULT_ORDER = Object.keys(PANEL_BLOCKS) as PanelBlock[];

export interface PanelPrefs {
  order: string[];
  hidden: string[];
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
