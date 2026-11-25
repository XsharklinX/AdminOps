// «Lo de siempre» de Ctrl+K: qué acciones se lanzan desde la caja a cada hora del día.
// Se cuenta en este equipo (localStorage) y no sale de él. Solo se guarda el id de la
// acción, el momento y la franja: nada de nombres de personas ni de equipos.

export type Slot = "morning" | "afternoon";

export interface Use {
  /** Id de la entrada de Ctrl+K («page:updates», «action:diag»…). */
  k: string;
  /** Cuándo, en milisegundos. */
  t: number;
  /** Franja del día en que se lanzó. */
  s: Slot;
}

const STORE_KEY = "adminops.ctrlk.uses";
const DAY = 24 * 60 * 60 * 1000;
/** Lo que se guarda: más allá se poda al escribir. */
export const KEEP_DAYS = 31;
/** Lo que cuenta para «Lo de siempre». */
export const COUNT_DAYS = 30;
export const MAX_SHOWN = 5;
const MAX_KEY = 80;

/** «Mañana» antes de las 14:00; «tarde» desde las 14:00. */
export function slotOf(d: Date): Slot {
  return d.getHours() < 14 ? "morning" : "afternoon";
}

/** El título de la cabecera de «Lo de siempre». */
export const slotTitle = (s: Slot) => (s === "morning" ? "Lo de siempre · por la mañana" : "Lo de siempre · por la tarde");

/** Los tipos de entrada que se cuentan. Contactos, frases con nombre (equipos, personas) y demás se quedan fuera. */
const TRACKED = ["page:", "section:", "action:", "expert:", "tweak:", "tool:"];

/** La clave tal cual si es de una acción que se puede contar; null si no (o si lleva algo que no es un id). */
export function trackableKey(key: string): string | null {
  if (key.length > MAX_KEY || !TRACKED.some((p) => key.startsWith(p))) return null;
  return key;
}

const valid = (u: unknown): u is Use => {
  const x = u as Use | null;
  return !!x && typeof x.k === "string" && typeof x.t === "number" && Number.isFinite(x.t) && (x.s === "morning" || x.s === "afternoon");
};

/** Quita lo que tiene más de 31 días. */
export function prune(uses: Use[], now: number): Use[] {
  return uses.filter((u) => now - u.t <= KEEP_DAYS * DAY);
}

/** Apunta una acción lanzada ahora. Devuelve la lista nueva, ya podada. */
export function recordUse(uses: Use[], key: string, now: number): Use[] {
  const k = trackableKey(key);
  if (!k) return prune(uses, now);
  return [...prune(uses, now), { k, t: now, s: slotOf(new Date(now)) }];
}

export interface Habit {
  key: string;
  count: number;
}

/**
 * Las acciones más usadas en una franja en los últimos 30 días. Solo cuentan las que
 * `exists` dice que Ctrl+K ofrece ahora. Empates: la usada más recientemente, y luego la clave.
 */
export function topFor(uses: Use[], slot: Slot, now: number, exists: (key: string) => boolean, limit = MAX_SHOWN): Habit[] {
  const acc = new Map<string, { count: number; last: number }>();
  for (const u of uses) {
    if (u.s !== slot || now - u.t > COUNT_DAYS * DAY || u.t > now + DAY) continue;
    const a = acc.get(u.k) ?? { count: 0, last: 0 };
    a.count++;
    a.last = Math.max(a.last, u.t);
    acc.set(u.k, a);
  }
  return [...acc.entries()]
    .filter(([key]) => exists(key))
    .sort((a, b) => b[1].count - a[1].count || b[1].last - a[1].last || a[0].localeCompare(b[0]))
    .slice(0, limit)
    .map(([key, a]) => ({ key, count: a.count }));
}

/**
 * ¿Sale esta acción en «Lo de siempre» en este modo? Las que llevan a una pantalla (page:, section:,
 * expert:) solo si esa pantalla está permitida en el modo; todo lo demás (acciones, ajustes y
 * herramientas, que se lanzan o cuyo destino no se conoce desde aquí) solo en modo admin.
 * `pageAllowed` dice si una pantalla está permitida en el modo actual; `expertPage`, a qué pantalla lleva una entrada experta.
 */
export function habitAllowed(key: string, mode: "admin" | "user", pageAllowed: (page: string) => boolean, expertPage: (id: string) => string | undefined): boolean {
  if (key.startsWith("page:")) return pageAllowed(key.slice(5));
  if (key.startsWith("section:")) return pageAllowed(key.split(":")[1] ?? "");
  if (key.startsWith("expert:")) {
    const page = expertPage(key.slice(7));
    return page !== undefined && pageAllowed(page);
  }
  return mode === "admin";
}

/** Qué lanza Alt+1…5: la posición de «Lo de siempre», y solo con la caja vacía (con texto, Ctrl+K como siempre). Usa `e.code` (`Digit1`…`Digit5`): el teclado numérico con Alt son códigos de carácter y no cuenta. */
export function altIndex(code: string, query: string, shown: number): number | null {
  if (query.trim() || !/^Digit[1-5]$/.test(code)) return null;
  const i = Number(code.slice(5)) - 1;
  return i < shown ? i : null;
}

/** «5 veces este mes» (la primera de la lista). */
export const timesThisMonth = (n: number) => (n === 1 ? "1 vez este mes" : `${n} veces este mes`);

export function loadUses(): Use[] {
  try {
    const raw = JSON.parse(localStorage.getItem(STORE_KEY) ?? "[]");
    return Array.isArray(raw) ? raw.filter(valid) : [];
  } catch {
    return [];
  }
}

/** Cuenta una acción lanzada desde Ctrl+K (y poda lo viejo). Sin almacenamiento, no hace nada. */
export function noteUse(key: string, now = Date.now()) {
  if (!trackableKey(key)) return;
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(recordUse(loadUses(), key, now)));
  } catch {
    /* sin almacenamiento: no se cuenta */
  }
}
