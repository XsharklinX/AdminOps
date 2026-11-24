// Tamaño de toda la interfaz (Ctrl +, Ctrl −, Ctrl+0): pasos y qué tamaño
// corresponde a cada monitor. Sin dependencias, para poder probarlo.

/** Los tamaños a los que se puede ir, de menor a mayor. */
export const ZOOM_STEPS = [0.8, 0.9, 1, 1.1, 1.25, 1.4, 1.5];

/** Del tamaño actual al siguiente (1) o al anterior (-1); 0 vuelve al 100 %. Se queda en los extremos. */
export function nextZoom(current: number, dir: 1 | -1 | 0): number {
  if (dir === 0) return 1;
  let at = 0;
  ZOOM_STEPS.forEach((z, i) => {
    if (Math.abs(z - current) < Math.abs(ZOOM_STEPS[at] - current)) at = i;
  });
  return ZOOM_STEPS[Math.min(ZOOM_STEPS.length - 1, Math.max(0, at + dir))];
}

/** El tamaño de un monitor, o el general si no se ha elegido ninguno para él. */
export function zoomFor(general: number, byMonitor: Record<string, number> | undefined, monitor: string | null): number {
  const z = monitor ? byMonitor?.[monitor] : undefined;
  return typeof z === "number" && ZOOM_STEPS.includes(z) ? z : general;
}

/** «125 %» */
export const zoomPercent = (z: number) => `${Math.round(z * 100)} %`;

/** Qué hace una tecla con Ctrl: 1 (más grande), -1 (más pequeño), 0 (100 %) o null (nada). */
export function zoomKeyDir(ev: Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "altKey" | "metaKey">): 1 | -1 | 0 | null {
  if (!ev.ctrlKey || ev.altKey || ev.metaKey) return null;
  if (ev.key === "+" || ev.key === "=" || ev.code === "NumpadAdd") return 1;
  if (ev.key === "-" || ev.key === "_" || ev.code === "NumpadSubtract") return -1;
  if (ev.key === "0" || ev.code === "Numpad0") return 0;
  return null;
}
