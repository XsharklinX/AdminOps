// La tendencia de una serie corta (los últimos minutos del Panel): ¿sube, baja o
// se mantiene? Se comparan las medias del principio y del final para no fiarse
// de un pico suelto.

export type Trend = { dir: "up" | "down" | "flat"; delta: number };

/** `min`: cuánto tiene que moverse la media para que cuente como cambio. */
export function trend(data: number[], min: number): Trend {
  if (data.length < 10) return { dir: "flat", delta: 0 };
  const q = Math.max(3, Math.floor(data.length / 5));
  const mean = (a: number[]) => a.reduce((s, v) => s + v, 0) / a.length;
  const delta = mean(data.slice(-q)) - mean(data.slice(0, q));
  return { dir: delta >= min ? "up" : delta <= -min ? "down" : "flat", delta };
}
