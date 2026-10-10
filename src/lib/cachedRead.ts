// Memoria de lecturas lentas: lo último leído de una lista que tarda (programas
// instalados, lo que arranca con Windows, los discos…) se enseña al momento al
// volver a la pantalla, con «actualizado hace 3 min», y por detrás se lee de
// nuevo y se sustituye. Desde la 1.2.8 también sobrevive a cerrar AdminOps
// (en el almacenamiento de la ventana, por equipo), así al abrir ya hay algo
// que mirar mientras Windows contesta.

interface Entry {
  at: number;
  data: unknown;
}

const PREFIX = "adminops.cache.";
/** Lo más grande que se guarda entre aperturas (más, solo en memoria). */
const MAX_STORED = 400_000;
/** Lo guardado de hace más de esto ya no se enseña. */
const MAX_AGE = 7 * 24 * 3600 * 1000;

const mem = new Map<string, Entry>();

function fromStorage(key: string): Entry | null {
  try {
    const raw = localStorage.getItem(PREFIX + key);
    if (!raw) return null;
    const e = JSON.parse(raw) as Entry;
    if (!e || typeof e.at !== "number" || Date.now() - e.at > MAX_AGE) return null;
    mem.set(key, e);
    return e;
  } catch {
    return null;
  }
}

function toStorage(key: string, e: Entry) {
  try {
    const raw = JSON.stringify(e);
    if (raw.length <= MAX_STORED) localStorage.setItem(PREFIX + key, raw);
  } catch {
    /* sin sitio o sin almacenamiento: queda en memoria */
  }
}

const get = (key: string) => mem.get(key) ?? fromStorage(key);

/** Lo último leído con esa clave, si lo hay. */
export function cachedValue<T>(key: string): { at: number; data: T } | null {
  const e = get(key);
  return e ? { at: e.at, data: e.data as T } : null;
}

/** Guarda lo que se acaba de leer por otro camino (una recarga tras un cambio). */
export const remember = (key: string, data: unknown) => {
  const e = { at: Date.now(), data };
  mem.set(key, e);
  toStorage(key, e);
};

/** Olvida una lectura (después de un cambio, para no enseñar lo de antes). */
export const forget = (key: string) => {
  mem.delete(key);
  try {
    localStorage.removeItem(PREFIX + key);
  } catch {
    /* nada */
  }
};

/**
 * Lee con memoria: si había algo, `onData` lo recibe enseguida (`fresh` false);
 * luego lee de verdad y lo vuelve a llamar con lo nuevo (`fresh` true). El error
 * de la lectura llega como siempre (rechazo de la promesa).
 */
export async function readCached<T>(key: string, read: () => Promise<T>, onData: (data: T, fresh: boolean, at: number) => void): Promise<void> {
  const c = get(key);
  if (c) onData(c.data as T, false, c.at);
  const data = await read();
  const at = Date.now();
  const e = { at, data };
  mem.set(key, e);
  toStorage(key, e);
  onData(data, true, at);
}

/** ¿Ha cambiado de verdad lo leído? (para avisar solo si algo cambió). */
export const changed = (a: unknown, b: unknown) => JSON.stringify(a) !== JSON.stringify(b);
