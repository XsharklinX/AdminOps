import { X, Loader2, ChevronDown} from "lucide-react";
import type { ReactNode } from "react";
import { loadColor } from "../lib/format";

export function Card({
  id,
  title,
  icon,
  right,
  children,
  className = "",
  fold,
}: {
  id?: string;
  title?: string;
  icon?: ReactNode;
  right?: ReactNode;
  children: ReactNode;
  className?: string;
  /** Tarjeta plegada: se ve el título y una nota, y el detalle a un clic. */
  fold?: { collapsed: boolean; note: string; onToggle: () => void };
}) {
  const collapsed = !!fold?.collapsed;
  return (
    <section
      id={id}
      className={`scroll-mt-6 rounded-xl border border-line bg-panel p-4 transition-[border-color,box-shadow] duration-500 [contain:layout_paint] ${
        collapsed ? "py-3" : ""
      } ${className}`}
    >
      {title && (
        <header className={`flex items-center justify-between ${collapsed ? "" : "mb-3"}`}>
          <h2 className="flex items-center gap-2 text-[13px] font-semibold text-ink">
            {icon && <span className="text-mute">{icon}</span>}
            {title}
          </h2>
          {fold ? (
            <button onClick={fold.onToggle} className="flex items-center gap-1.5 text-[11px] text-mute transition-colors hover:text-ink">
              {collapsed && <span className="text-ok">{fold.note}</span>}
              {collapsed ? "Ver detalles" : "Ocultar"}
              <ChevronDown size={11} className={`transition-transform ${collapsed ? "" : "rotate-180"}`} />
            </button>
          ) : (
            right
          )}
        </header>
      )}
      {!collapsed && children}
    </section>
  );
}

/** Anillo de porcentaje. */
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
          // Sin transición ni filtro: el arco cambia en cada actualización (medido en la Fase 7).
        />
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className="text-2xl font-semibold tabular" style={{ color: v >= 70 ? color : "var(--color-ink)" }}>
          {Math.round(v)}
          <span className="text-sm">%</span>
        </span>
        <span className="text-[11px] text-mute">{label}</span>
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
    <svg viewBox={`0 0 ${w} ${height}`} preserveAspectRatio="none" className="w-full" style={{ height }}>
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.12} />
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

/** Barra de progreso fina. `glow` se conserva por compatibilidad y no hace nada. */
export function Bar({ value, color }: { value: number; color?: string; glow?: boolean }) {
  const v = Math.min(100, Math.max(0, value));
  const c = color ?? loadColor(v);
  return (
    <div className="h-1 w-full overflow-hidden rounded-full bg-line">
      <div className="h-full rounded-full" style={{ width: `${v}%`, background: c }} />
    </div>
  );
}

export function Stat({ label, value, sub }: { label: string; value: ReactNode; sub?: ReactNode }) {
  return (
    <div className="min-w-0">
      <div className="text-[11px] text-mute">{label}</div>
      <div className="truncate text-sm font-medium tabular text-ink">{value}</div>
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
      className="fixed inset-0 z-40 grid place-items-center bg-black/55"
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
    primary: "border-neon bg-neon text-on-neon hover:brightness-110",
    danger: "border-bad/50 text-bad hover:bg-bad/10",
    ghost: "border-transparent text-dim hover:bg-panel-2 hover:text-ink",
  }[kind];
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`flex h-9 items-center gap-1.5 rounded-lg border px-3.5 text-[13px] font-medium transition-colors disabled:pointer-events-none disabled:opacity-40 ${style}`}
    >
      {children}
    </button>
  );
}

/** Estado de carga igual en toda la app. `page`: ocupa el sitio de una página entera. */
export function Loading({ text = "Cargando…", page = false }: { text?: string; page?: boolean }) {
  return (
    <p className={`flex items-center gap-2 text-sm text-mute ${page ? "p-8" : "py-2"}`}>
      <Loader2 size={14} className="animate-spin" /> {text}
    </p>
  );
}

/** Lista vacía igual en toda la app: qué pasa y, si procede, qué hacer. */
export function EmptyState({ icon, title, children, action }: { icon?: ReactNode; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-2 rounded-xl border border-dashed border-line px-6 py-10 text-center">
      {icon && <span className="text-mute">{icon}</span>}
      <p className="text-sm text-ink">{title}</p>
      {children && <p className="max-w-md text-xs text-mute">{children}</p>}
      {action && <div className="mt-1">{action}</div>}
    </div>
  );
}
