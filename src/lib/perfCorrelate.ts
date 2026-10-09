// «Va lento desde el jueves»: detecta los saltos del uso (procesador, memoria, disco) en el
// historial de rendimiento y los junta con lo que pasó justo antes (un programa instalado,
// un ajuste aplicado, una actualización), para señalar qué coincide con el cambio.
//
// Coincidir no es causar: se dice «coincide con», no «es culpa de».
import type { Sample, TimelineEvent } from "./api";

export type Metric = "cpu" | "ram" | "disk";

export const METRIC_NAME: Record<Metric, string> = { cpu: "el procesador", ram: "la memoria", disk: "el disco" };

/** Cuánto tiene que subir la media (puntos) para considerarlo un salto. */
const MIN_JUMP: Record<Metric, number> = { cpu: 15, ram: 12, disk: 10 };
/** Muestras (minutos) a cada lado para comparar. */
const WINDOW = 20;
/** Entre dos saltos de lo mismo tiene que haber al menos esto (segundos). */
const APART = 3600;

export interface StepUp {
  metric: Metric;
  at: number;
  before: number;
  after: number;
}

export interface Blame {
  step: StepUp;
  event: TimelineEvent | null;
  /** Minutos entre el suceso y el salto (negativo: el suceso fue después). */
  minutes: number | null;
}

const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / Math.max(xs.length, 1);

/** Los saltos sostenidos hacia arriba de cada métrica, los mayores primero. */
export function findStepUps(samples: Sample[], metrics: Metric[] = ["cpu", "ram", "disk"]): StepUp[] {
  const out: StepUp[] = [];
  for (const metric of metrics) {
    const found: StepUp[] = [];
    for (let i = WINDOW; i + WINDOW <= samples.length; i++) {
      // Sin huecos: las 2×WINDOW muestras tienen que ser casi consecutivas (AdminOps abierta).
      if (samples[i + WINDOW - 1].t - samples[i - WINDOW].t > 2 * WINDOW * 60 * 1.5) continue;
      const before = mean(samples.slice(i - WINDOW, i).map((s) => s[metric]));
      const after = mean(samples.slice(i, i + WINDOW).map((s) => s[metric]));
      if (after - before >= MIN_JUMP[metric]) found.push({ metric, at: samples[i].t, before, after });
    }
    // De cada racha de posiciones seguidas, el salto más grande; y separados entre sí.
    found.sort((a, b) => b.after - b.before - (a.after - a.before));
    const kept: StepUp[] = [];
    for (const s of found) if (kept.every((k) => Math.abs(k.at - s.at) >= APART)) kept.push(s);
    out.push(...kept.slice(0, 3));
  }
  return out.sort((a, b) => b.after - b.before - (a.after - a.before));
}

/** ¿Es un suceso que puede cambiar cómo va el equipo? (no un arranque ni un apagado) */
export function relevant(e: TimelineEvent): boolean {
  if (e.kind !== "change" && e.kind !== "windows") return false;
  if (/arrancó|se apagó|Apagado inesperado/i.test(e.title)) return false;
  return e.level !== "info" || /instalad|desinstalad|Driver|Programa/i.test(e.title);
}

/** Para cada salto, el suceso relevante más cercano justo antes (hasta 90 min) o un poco después (10 min). */
export function blame(steps: StepUp[], events: TimelineEvent[]): Blame[] {
  const rel = events.filter(relevant);
  return steps.map((step) => {
    let best: TimelineEvent | null = null;
    let bestGap = Infinity;
    for (const e of rel) {
      const gap = (step.at - e.time) / 60; // minutos que pasaron desde el suceso hasta el salto
      if (gap < -10 || gap > 90) continue;
      const score = gap < 0 ? 100 + Math.abs(gap) : gap;
      if (score < bestGap) {
        best = e;
        bestGap = score;
      }
    }
    return { step, event: best, minutes: best ? Math.round((step.at - best.time) / 60) : null };
  });
}

/** El texto de un salto en palabras. */
export function describeStep(b: Blame): string {
  const { step } = b;
  const base = `${METRIC_NAME[step.metric][0].toUpperCase()}${METRIC_NAME[step.metric].slice(1)} pasó de ${Math.round(step.before)} % a ${Math.round(step.after)} %`;
  if (!b.event || b.minutes === null) return `${base}. No hay nada registrado justo antes.`;
  const when = b.minutes >= 0 ? `${b.minutes} min después de` : `${-b.minutes} min antes de`;
  return `${base}, ${when} «${b.event.title}»${b.event.detail ? ` (${b.event.detail.split(" · ")[0]})` : ""}.`;
}
