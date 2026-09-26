import type { ReactNode } from "react";
import { loadColor } from "../lib/format";

export function Card({
  title,
  icon,
  right,
  children,
  className = "",
}: {
  title?: string;
  icon?: ReactNode;
  right?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rounded-xl border border-line bg-panel p-4 ${className}`}>
      {title && (
        <header className="mb-3 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-xs font-semibold tracking-[0.14em] text-dim uppercase">
            {icon && <span className="text-neon">{icon}</span>}
            {title}
          </h2>
          {right}
        </header>
      )}
      {children}
    </section>
  );
}

/** Anillo de porcentaje con brillo neón. */
export function Ring({ value, size = 112, label }: { value: number; size?: number; label: string }) {
  const stroke = 8;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const v = Math.min(100, Math.max(0, value));
  const color = loadColor(v);
  return (
    <div className="relative shrink-0" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90">
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--color-line)" strokeWidth={stroke} />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke={color}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c - (v / 100) * c}
          style={{ transition: "stroke-dashoffset .6s ease, stroke .3s", filter: `drop-shadow(0 0 6px ${color})` }}
        />
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className="font-mono text-2xl font-semibold tabular text-glow" style={{ color }}>
          {Math.round(v)}
          <span className="text-sm">%</span>
        </span>
        <span className="text-[10px] tracking-widest text-mute uppercase">{label}</span>
      </div>
    </div>
  );
}

/** Mini gráfico de línea con relleno degradado. `max` fija la escala (p. ej. 100 para %). */
export function Sparkline({
  data,
  max,
  color = "var(--color-neon)",
  height = 48,
}: {
  data: number[];
  max?: number;
  color?: string;
  height?: number;
}) {
  const w = 240;
  const top = Math.max(max ?? Math.max(...data, 1), 1);
  const n = Math.max(data.length, 2);
  const pts = data.map((v, i) => `${(i / (n - 1)) * w},${height - (v / top) * (height - 2) - 1}`);
  const id = `g${color.replace(/[^a-z0-9]/gi, "")}`;
  return (
    <svg viewBox={`0 0 ${w} ${height}`} preserveAspectRatio="none" className="h-12 w-full">
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.35} />
          <stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      {data.length > 1 && (
        <>
          <polygon points={`0,${height} ${pts.join(" ")} ${((data.length - 1) / (n - 1)) * w},${height}`} fill={`url(#${id})`} />
          <polyline points={pts.join(" ")} fill="none" stroke={color} strokeWidth={1.5} vectorEffect="non-scaling-stroke" />
        </>
      )}
    </svg>
  );
}

export function Bar({ value, color }: { value: number; color?: string }) {
  const v = Math.min(100, Math.max(0, value));
  const c = color ?? loadColor(v);
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full bg-line">
      <div
        className="h-full rounded-full"
        style={{ width: `${v}%`, background: c, boxShadow: `0 0 8px ${c}`, transition: "width .6s ease" }}
      />
    </div>
  );
}

export function Stat({ label, value, sub }: { label: string; value: ReactNode; sub?: ReactNode }) {
  return (
    <div className="min-w-0">
      <div className="text-[10px] tracking-widest text-mute uppercase">{label}</div>
      <div className="truncate font-mono text-sm tabular text-ink">{value}</div>
      {sub && <div className="truncate text-xs text-dim">{sub}</div>}
    </div>
  );
}
