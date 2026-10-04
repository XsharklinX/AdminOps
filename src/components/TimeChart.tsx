// Gráfica de líneas en el tiempo, sin librerías: una o varias series sobre el
// mismo eje. Los huecos (AdminOps cerrada, un ping sin respuesta) se dejan en
// blanco en vez de unir los puntos con una recta que mentiría.
import { useMemo, useRef, useState } from "react";

export interface Series {
  label: string;
  /** Color CSS (variable del tema). */
  color: string;
  values: (number | null)[];
}

const W = 1000;

/** Marcas del eje de tiempo: cada media hora, cada 3 h o cada día (a medianoche) según el tramo. */
export function ticks(from: number, to: number): { t: number; label: string }[] {
  const span = to - from;
  const every = span <= 3 * 3600 ? 0.5 : span <= 26 * 3600 ? 3 : 24;
  const d = new Date(from * 1000);
  d.setMinutes(every === 0.5 && d.getMinutes() >= 30 ? 30 : 0, 0, 0);
  const out: { t: number; label: string }[] = [];
  for (let t = d.getTime() / 1000; t <= to; t += every === 0.5 ? 1800 : 3600) {
    if (t < from) continue;
    const dt = new Date(t * 1000);
    if (every === 3 && dt.getHours() % 3 !== 0) continue;
    if (every === 24 && dt.getHours() !== 0) continue;
    out.push({
      t,
      label: every === 24 ? dt.toLocaleDateString("es", { weekday: "short", day: "numeric" }) : dt.toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" }),
    });
  }
  return out;
}

export function TimeChart({
  times,
  series,
  max,
  height = 180,
  unit = "%",
  gapSeconds = 180,
  marks = [],
  describe,
}: {
  /** Segundos desde 1970, uno por punto. */
  times: number[];
  series: Series[];
  /** Tope del eje Y (si no, el mayor valor). */
  max?: number;
  height?: number;
  unit?: string;
  /** Más de esto entre dos puntos: se corta la línea. */
  gapSeconds?: number;
  /** Tramos a resaltar (cortes): [desde, hasta] en segundos. */
  marks?: [number, number][];
  /** Texto extra del punto bajo el ratón. */
  describe?: (i: number) => string | null;
}) {
  const [hover, setHover] = useState<number | null>(null);
  const box = useRef<SVGSVGElement>(null);
  const H = height;
  const PAD = { l: 34, r: 8, t: 8, b: 22 };
  const from = times[0] ?? 0;
  const to = times[times.length - 1] ?? 1;
  const top = useMemo(() => max ?? Math.max(1, ...series.flatMap((s) => s.values.filter((v): v is number => v !== null))) * 1.1, [max, series]);
  const x = (t: number) => PAD.l + ((t - from) / Math.max(1, to - from)) * (W - PAD.l - PAD.r);
  const y = (v: number) => PAD.t + (1 - Math.min(v, top) / top) * (H - PAD.t - PAD.b);

  const paths = series.map((s) => {
    let d = "";
    let open = false;
    s.values.forEach((v, i) => {
      if (v === null || (i > 0 && times[i] - times[i - 1] > gapSeconds)) open = false;
      if (v === null) return;
      d += `${open ? "L" : "M"}${x(times[i]).toFixed(1)},${y(v).toFixed(1)}`;
      open = true;
    });
    return d;
  });

  const onMove = (e: React.PointerEvent<SVGSVGElement>) => {
    const r = box.current?.getBoundingClientRect();
    if (!r || !times.length) return;
    const t = from + ((e.clientX - r.left) / r.width * W - PAD.l) / (W - PAD.l - PAD.r) * (to - from);
    let best = 0;
    for (let i = 1; i < times.length; i++) if (Math.abs(times[i] - t) < Math.abs(times[best] - t)) best = i;
    setHover(best);
  };

  const yTicks = [0, top / 2, top].map((v) => Math.round(v));
  const fmt = (v: number) => `${Math.round(v)}${unit === "%" ? " %" : ` ${unit}`}`;

  return (
    <div className="relative">
      <svg
        ref={box}
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        className="block h-auto w-full"
        style={{ height }}
        onPointerMove={onMove}
        onPointerLeave={() => setHover(null)}
        role="img"
        aria-label={series.map((s) => s.label).join(", ")}
      >
        {marks.map(([a, b], i) => (
          <rect key={i} x={x(Math.max(a, from))} y={PAD.t} width={Math.max(2, x(Math.min(b, to)) - x(Math.max(a, from)))} height={H - PAD.t - PAD.b} fill="var(--color-bad)" opacity={0.14} />
        ))}
        {yTicks.map((v) => (
          <g key={v}>
            <line x1={PAD.l} x2={W - PAD.r} y1={y(v)} y2={y(v)} stroke="var(--color-line)" strokeWidth={1} vectorEffect="non-scaling-stroke" />
            <text x={PAD.l - 6} y={y(v) + 3} textAnchor="end" fontSize={10} fill="var(--color-mute)" style={{ fontFamily: "var(--font-mono)" }}>
              {v}
            </text>
          </g>
        ))}
        {ticks(from, to).map((k) => (
          <text key={k.t} x={x(k.t)} y={H - 6} textAnchor="middle" fontSize={10} fill="var(--color-mute)">
            {k.label}
          </text>
        ))}
        {paths.map((d, i) => (
          <path key={series[i].label} d={d} fill="none" stroke={series[i].color} strokeWidth={1.6} vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
        ))}
        {hover !== null && <line x1={x(times[hover])} x2={x(times[hover])} y1={PAD.t} y2={H - PAD.b} stroke="var(--color-line-2)" strokeWidth={1} vectorEffect="non-scaling-stroke" />}
      </svg>
      <div className="mt-1 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11.5px] text-mute">
        {series.map((s) => (
          <span key={s.label} className="flex items-center gap-1.5">
            <span className="h-0.5 w-3 rounded" style={{ background: s.color }} />
            {s.label}
          </span>
        ))}
        {hover !== null && (
          <span className="ml-auto font-mono text-dim">
            {new Date(times[hover] * 1000).toLocaleString("es", { weekday: "short", day: "numeric", hour: "2-digit", minute: "2-digit" })} ·{" "}
            {series.map((s) => `${s.label} ${s.values[hover] === null ? "—" : fmt(s.values[hover]!)}`).join(" · ")}
            {describe?.(hover) ? ` · ${describe(hover)}` : ""}
          </span>
        )}
      </div>
    </div>
  );
}
