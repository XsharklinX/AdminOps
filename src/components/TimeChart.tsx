// Gráfica de líneas en el tiempo, sin librerías: una o varias series sobre el
// mismo eje. Los huecos (AdminOps cerrada, un ping sin respuesta) se dejan en
// blanco en vez de unir los puntos con una recta que mentiría.
import { useId, useMemo, useRef, useState } from "react";
import { motionOk } from "./motion";

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
  events = [],
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
  /** Sucesos a señalar con una línea vertical (instalaciones, cambios…). */
  events?: { t: number; label: string; tone?: "ok" | "info" | "warn" | "bad" }[];
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

  const base = H - PAD.b;
  // Cada serie: su línea y el área bajo ella (un tramo por cada trozo continuo).
  const paths = series.map((s) => {
    let d = "";
    let area = "";
    let open = false;
    let first = 0;
    let last = 0;
    const close = () => {
      if (open) area += `L${last.toFixed(1)},${base}L${first.toFixed(1)},${base}Z`;
      open = false;
    };
    s.values.forEach((v, i) => {
      if (v === null || (i > 0 && times[i] - times[i - 1] > gapSeconds)) close();
      if (v === null) return;
      const px = x(times[i]);
      const seg = `${open ? "L" : "M"}${px.toFixed(1)},${y(v).toFixed(1)}`;
      d += seg;
      area += seg;
      if (!open) first = px;
      last = px;
      open = true;
    });
    close();
    return { d, area };
  });
  const uid = useId().replace(/:/g, "");
  // El punto del último valor de cada serie late: dice «esto es ahora».
  const lastDot = series.map((s) => {
    for (let i = s.values.length - 1; i >= 0; i--) if (s.values[i] !== null) return i;
    return -1;
  });
  const [pulse] = useState(motionOk);
  // Rótulos de lo señalado (sin amontonarse): hasta 4, separados al menos un 14 % del ancho.
  const labels = useMemo(() => {
    const out: { left: number; text: string; tone?: string }[] = [];
    for (const e of events.filter((ev) => ev.t >= from && ev.t <= to).sort((a, b) => a.t - b.t)) {
      const left = (x(e.t) / W) * 100;
      if (out.length < 4 && (!out.length || left - out[out.length - 1].left > 14)) out.push({ left, text: e.label, tone: e.tone });
    }
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps -- x depende de from/to
  }, [events, from, to]);

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
        <defs>
          {series.map((s, i) => (
            <linearGradient key={s.label} id={`${uid}-${i}`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={s.color} stopOpacity={series.length > 2 ? 0.1 : 0.28} />
              <stop offset="100%" stopColor={s.color} stopOpacity={0} />
            </linearGradient>
          ))}
        </defs>
        {marks.map(([a, b], i) => (
          <rect key={i} x={x(Math.max(a, from))} y={PAD.t} width={Math.max(2, x(Math.min(b, to)) - x(Math.max(a, from)))} height={H - PAD.t - PAD.b} fill="var(--color-bad)" opacity={0.14} />
        ))}
        {events
          .filter((e) => e.t >= from && e.t <= to)
          .map((e, i) => (
            <g key={`${e.t}-${i}`}>
              <title>{e.label}</title>
              <line x1={x(e.t)} x2={x(e.t)} y1={PAD.t} y2={H - PAD.b} stroke={e.tone === "bad" ? "var(--color-bad)" : e.tone === "warn" ? "var(--color-warn)" : "var(--color-neon)"} strokeWidth={1} strokeDasharray="3 3" opacity={0.75} vectorEffect="non-scaling-stroke" />
              <circle cx={x(e.t)} cy={PAD.t + 3} r={3} fill={e.tone === "bad" ? "var(--color-bad)" : e.tone === "warn" ? "var(--color-warn)" : "var(--color-neon)"} />
            </g>
          ))}
        {yTicks.map((v) => (
          <g key={v}>
            <line x1={PAD.l} x2={W - PAD.r} y1={y(v)} y2={y(v)} stroke="var(--color-line)" strokeWidth={1} opacity={0.55} vectorEffect="non-scaling-stroke" />
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
        {paths.map((p, i) => (
          <path key={`a-${series[i].label}`} d={p.area} fill={`url(#${uid}-${i})`} stroke="none" />
        ))}
        {paths.map((p, i) => (
          <path key={series[i].label} d={p.d} fill="none" stroke={series[i].color} strokeWidth={1.6} vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
        ))}
        {hover !== null && <line x1={x(times[hover])} x2={x(times[hover])} y1={PAD.t} y2={H - PAD.b} stroke="var(--color-line-2)" strokeWidth={1} vectorEffect="non-scaling-stroke" />}
      </svg>
      {/* Puntos sobre la gráfica (HTML, para que no se deformen con el ancho): el último valor late y, al pasar el ratón, el valor de cada serie. */}
      <div className="pointer-events-none absolute inset-x-0 top-0" style={{ height }} aria-hidden>
        {labels.map((l) => (
          <span key={l.left} className="absolute top-0 -translate-x-1/2 rounded bg-panel/80 px-1 text-[10px] whitespace-nowrap text-dim" style={{ left: `${l.left}%` }}>
            {l.text}
          </span>
        ))}
        {series.map((s, i) => {
          const at = hover ?? lastDot[i];
          const v = at >= 0 ? s.values[at] : null;
          if (v === null || v === undefined || at < 0) return null;
          return (
            <span key={s.label} className="absolute size-2 -translate-x-1/2 -translate-y-1/2" style={{ left: `${(x(times[at]) / W) * 100}%`, top: y(v) }}>
              {hover === null && pulse && <span className="chart-ping absolute inset-0 rounded-full" style={{ background: s.color }} />}
              <span className="absolute inset-0 rounded-full ring-2 ring-panel" style={{ background: s.color }} />
            </span>
          );
        })}
      </div>
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
