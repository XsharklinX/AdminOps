// Lo que se carga por detrás al abrir AdminOps (permisos, ajustes, sensores…).
// La ventana aparece enseguida con el marco; mientras esto termina se ve una
// barrita fina arriba y, si algo tarda de verdad, se dice qué es.

type Listener = () => void;

const pending = new Map<number, { label: string; since: number }>();
const listeners = new Set<Listener>();
let next = 1;

const emit = () => listeners.forEach((l) => l());

/** Apunta una carga de fondo; se quita sola al terminar (bien o mal). */
export function trackBoot<T>(label: string, p: Promise<T>): Promise<T> {
  const id = next++;
  pending.set(id, { label, since: Date.now() });
  emit();
  const end = () => {
    pending.delete(id);
    emit();
  };
  p.then(end, end);
  return p;
}

/** Lo que sigue cargando y desde cuándo. */
export function bootPending(): { label: string; since: number }[] {
  return [...pending.values()];
}

export function onBootChange(l: Listener): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Lo que lleva más de `ms` cargando, para decirlo en palabras. */
export function slowOnes(list: { label: string; since: number }[], now: number, ms = 4000): string[] {
  return list.filter((x) => now - x.since >= ms).map((x) => x.label);
}
