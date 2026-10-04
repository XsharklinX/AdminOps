// Memoria de lecturas lentas: lo último leído de una lista que tarda (programas
// instalados, lo que arranca con Windows…) se enseña al momento al volver a la
// pantalla, y por detrás se lee de nuevo y se sustituye. Solo en memoria,
// mientras AdminOps está abierta: al abrirla, todo se lee fresco.

interface Entry {
  at: number;
  data: unknown;
}

const mem = new Map<string, Entry>();

/** Lo último leído con esa clave, si lo hay. */
export function cachedValue<T>(key: string): { at: number; data: T } | null {
  const e = mem.get(key);
  return e ? { at: e.at, data: e.data as T } : null;
}

/** Guarda lo que se acaba de leer por otro camino (una recarga tras un cambio). */
export const remember = (key: string, data: unknown) => void mem.set(key, { at: Date.now(), data });

/** Olvida una lectura (después de un cambio, para no enseñar lo de antes). */
export const forget = (key: string) => void mem.delete(key);

/**
 * Lee con memoria: si había algo, `onData` lo recibe enseguida (`fresh` false);
 * luego lee de verdad y lo vuelve a llamar con lo nuevo (`fresh` true). El error
 * de la lectura llega como siempre (rechazo de la promesa).
 */
export async function readCached<T>(key: string, read: () => Promise<T>, onData: (data: T, fresh: boolean, at: number) => void): Promise<void> {
  const c = mem.get(key);
  if (c) onData(c.data as T, false, c.at);
  const data = await read();
  const at = Date.now();
  mem.set(key, { at, data });
  onData(data, true, at);
}
