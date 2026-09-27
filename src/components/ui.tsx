import { X } from "lucide-react";
import type { ReactNode } from "react";
import { loadColor } from "../lib/format";

export function Card({
  id,
  title,
  icon,
  right,
  children,
  className = "",
}: {
  id?: string;
  title?: string;
  icon?: ReactNode;
  right?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section id={id} className={`scroll-mt-6 rounded-xl border border-line bg-panel p-4 transition-[border-color,box-shadow] duration-500 [contain:layout_paint] ${className}`}>
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
      {/* Halo con degradado: sin filtros de desenfoque, que son caros de repintar. */}
      <div
        className="absolute -inset-2 rounded-full opacity-30"
        style={{ background: `radial-gradient(circle, transparent 52%, ${color} 58%, transparent 72%)` }}
        aria-hidden
      />
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
          // Sin transición ni filtro: el arco cambia en cada actualización y un
          // drop-shadow obliga a recalcular el desenfoque (medido en la Fase 7).
          // El brillo lo da el halo estático de abajo.
        />
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className="font-mono text-2xl font-semibold tabular" style={{ color }}>
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

export function Bar({ value, color, glow = true }: { value: number; color?: string; glow?: boolean }) {
  const v = Math.min(100, Math.max(0, value));
  const c = color ?? loadColor(v);
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full bg-line">
      <div
        className="h-full rounded-full"
        style={{ width: `${v}%`, background: c, boxShadow: glow ? `0 0 8px ${c}` : undefined }}
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

/** Ventana modal con cabecera y botón de cerrar. Clic fuera o Escape la cierran. */
export function Modal({
  title,
  onClose,
  children,
  footer,
  width = "w-[480px]",
}: {
  title: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: string;
}) {
  return (
    <div
      className="fixed inset-0 z-40 grid place-items-center bg-black/60 backdrop-blur-sm"
      onClick={onClose}
      onKeyDown={(e) => e.key === "Escape" && onClose()}
    >
      <div className={`flex max-h-[88vh] ${width} flex-col rounded-xl border border-line-2 bg-panel shadow-2xl`} onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between border-b border-line px-5 py-3">
          <h3 className="font-semibold">{title}</h3>
          <button onClick={onClose} className="text-mute hover:text-ink" title="Cerrar">
            <X size={16} />
          </button>
        </div>
        <div className="overflow-y-auto px-5 py-4">{children}</div>
        {footer && <div className="flex items-center justify-end gap-2 border-t border-line px-5 py-3">{footer}</div>}
      </div>
    </div>
  );
}

export const inputClass =
  "w-full rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50 disabled:opacity-50";

export function Button({
  children,
  onClick,
  disabled,
  kind = "primary",
  title,
}: {
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  kind?: "primary" | "danger" | "ghost";
  title?: string;
}) {
  const style = {
    primary: "border-neon/50 text-neon hover:bg-neon/10",
    danger: "border-bad/50 text-bad hover:bg-bad/10",
    ghost: "border-transparent text-dim hover:bg-panel-2 hover:text-ink",
  }[kind];
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`flex items-center gap-1.5 rounded-md border px-3.5 py-1.5 text-sm font-medium transition-colors disabled:pointer-events-none disabled:opacity-40 ${style}`}
    >
      {children}
    </button>
  );
}
