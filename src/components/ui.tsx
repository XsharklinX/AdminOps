import { Check, ChevronDown, ClipboardCopy, Images, Loader2, RotateCw, ShieldCheck, TriangleAlert, X } from "lucide-react";
import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { copyAsImage, NO_CAPTURE } from "../lib/copyImage";
import { logQuietly } from "../lib/api/core";
import { createPortal } from "react-dom";
import { loadColor } from "../lib/format";
import { explainError, supportDetails } from "../lib/errors";
import { AnimatedValue, SkeletonRows, motionOk } from "./motion";
import { SendTo } from "./SendTo";
import { CodeLinks } from "./CodeLinks";

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
  const ref = useRef<HTMLElement>(null);
  // Copiar la tarjeta como imagen: «idle», dibujando, hecho, o el motivo del fallo.
  const [copy, setCopy] = useState<"idle" | "busy" | "done" | string>("idle");
  const copyCard = () => {
    if (!ref.current || copy === "busy") return;
    setCopy("busy");
    copyAsImage(ref.current).then(
      () => setCopy("done"),
      (e) => {
        logQuietly("Card")(e);
        setCopy(e instanceof Error ? e.message : "No se pudo copiar la imagen.");
      },
    );
    window.setTimeout(() => setCopy("idle"), 4000);
  };
  return (
    <section
      ref={ref}
      id={id}
      data-setting={title}
      className={`group/card scroll-mt-6 rounded-xl border border-line bg-panel p-4 shadow-elev-1 transition-[border-color,box-shadow] duration-500 ${
        collapsed ? "py-3" : ""
      } ${className}`}
    >
      {title && (
        <header className={`flex items-center justify-between ${collapsed ? "" : "mb-3"}`}>
          <h2 className="flex items-center gap-2 text-[13px] font-semibold text-ink">
            {icon && <span className="text-mute">{icon}</span>}
            {title}
          </h2>
          <span className="flex items-center gap-2">
          {!collapsed && (
            <button
              {...NO_CAPTURE}
              onClick={copyCard}
              title={copy === "done" ? "Copiada: pégala en Teams o en el correo" : copy === "idle" || copy === "busy" ? "Copiar esta tarjeta como imagen" : copy}
              aria-label={`Copiar la tarjeta «${title}» como imagen`}
              className={`rounded p-1 transition-opacity focus:opacity-100 ${
                copy === "idle" ? "text-mute opacity-0 group-hover/card:opacity-100 hover:text-ink" : copy === "done" ? "text-ok" : copy === "busy" ? "text-neon" : "text-bad"
              }`}
            >
              {copy === "busy" ? <Loader2 size={13} className="animate-spin" /> : copy === "done" ? <Check size={13} /> : <Images size={13} />}
            </button>
          )}
          {!collapsed && <SendTo target={() => ref.current} title={title} />}
          {fold ? (
            <button onClick={fold.onToggle} className="flex items-center gap-1.5 text-[11px] text-mute transition-colors hover:text-ink">
              {collapsed && <span className="text-ok">{fold.note}</span>}
              {collapsed ? "Ver detalles" : "Ocultar"}
              <ChevronDown size={11} className={`transition-transform ${collapsed ? "" : "rotate-180"}`} />
            </button>
          ) : (
            right
          )}
          </span>
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
  // Se dibuja la primera vez que aparece; después cambia sin animación (va en vivo).
  const [draw] = useState(motionOk);
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
          className={draw ? "ring-draw" : undefined}
          style={{ ["--ring-c" as string]: c }}
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

/** Capas abiertas, de abajo arriba. */
const OPEN: object[] = [];

/**
 * Capa que cubre la ventana entera, con su contenido centrado. Se pinta en el
 * `body`, fuera de quien la abre: dentro de una tarjeta o de una página con
 * `@container`, «fixed» deja de referirse a la ventana y el diálogo salía
 * cortado o descolocado. Clic fuera o Escape la cierran.
 */
export function Overlay({ onClose, children, z = "z-40", label }: { onClose: () => void; children: ReactNode; z?: string; /** Nombre del diálogo para los lectores de pantalla. */ label?: string }) {
  // `onClose` suele ser una función nueva en cada pintado: se guarda la última.
  const close = useRef(onClose);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    close.current = onClose;
  });
  useEffect(() => {
    // Con dos capas abiertas (una confirmación sobre un diálogo), Escape cierra solo la de arriba.
    const me = {};
    OPEN.push(me);
    // El foco entra en el diálogo (salvo que algo de dentro ya lo haya pedido con autoFocus)
    // y, al cerrarlo, vuelve a donde estaba.
    const before = document.activeElement as HTMLElement | null;
    const t = window.setTimeout(() => {
      const el = box.current;
      if (el && !el.contains(document.activeElement)) (focusables(el)[0] ?? el).focus();
    }, 0);
    const onKey = (e: KeyboardEvent) => {
      if (OPEN[OPEN.length - 1] !== me) return;
      if (e.key === "Escape") {
        e.stopPropagation();
        close.current();
      } else if (e.key === "Tab" && box.current) {
        // Tab y Mayús+Tab dan la vuelta dentro del diálogo, sin salir a la página de detrás.
        const items = focusables(box.current);
        if (!items.length) {
          e.preventDefault();
          return;
        }
        const first = items[0];
        const last = items[items.length - 1];
        const inside = box.current.contains(document.activeElement);
        if (e.shiftKey && (document.activeElement === first || !inside)) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && (document.activeElement === last || !inside)) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.clearTimeout(t);
      window.removeEventListener("keydown", onKey);
      OPEN.splice(OPEN.indexOf(me), 1);
      if (before && document.contains(before)) before.focus();
    };
  }, []);
  return createPortal(
    <div ref={box} role="dialog" aria-modal="true" aria-label={label} tabIndex={-1} className={`fixed inset-0 ${z} grid place-items-center bg-black/55 p-4 outline-none`} onClick={onClose}>
      {children}
    </div>,
    document.body,
  );
}

/** Lo que se puede alcanzar con Tab dentro de un elemento, en orden. */
function focusables(root: HTMLElement): HTMLElement[] {
  const sel = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
  return Array.from(root.querySelectorAll<HTMLElement>(sel)).filter((el) => el.offsetParent !== null || el === document.activeElement);
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
    <Overlay onClose={onClose} label={typeof title === "string" ? title : undefined}>
      <div className={`flex max-h-[88vh] ${width} max-w-full flex-col rounded-xl border border-line-2 bg-panel shadow-elev-3`} onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between border-b border-line px-5 py-3">
          <h3 className="font-semibold">{title}</h3>
          <button onClick={onClose} className="text-mute hover:text-ink" title="Cerrar">
            <X size={16} />
          </button>
        </div>
        <div className="overflow-y-auto px-5 py-4">{children}</div>
        {footer && <div className="flex items-center justify-end gap-2 border-t border-line px-5 py-3">{footer}</div>}
      </div>
    </Overlay>
  );
}

/** Botón de solo icono (editar, borrar, copiar…). Lleva siempre `title` o `aria-label`. */
export const iconBtn = "rounded-md p-1.5 text-mute transition-colors hover:bg-panel-2 hover:text-ink disabled:pointer-events-none disabled:opacity-30";
/** Botón pequeño con borde, para acciones secundarias dentro de una tarjeta o una fila. */
export const smallBtn =
  "inline-flex items-center justify-center gap-1.5 rounded-md border border-line-2 px-2.5 py-1.5 text-xs text-dim transition-colors hover:border-neon/40 hover:text-ink disabled:pointer-events-none disabled:opacity-40";
/** Botón pequeño con el acento suave: la acción principal de una fila o un panel. */
export const softBtn =
  "inline-flex items-center justify-center gap-1.5 rounded-md border border-neon/50 bg-neon/10 px-3 py-1.5 text-xs font-medium text-neon transition-colors hover:bg-neon/20 disabled:pointer-events-none disabled:opacity-40";

/** El nombre del ajuste de la fila (para los lectores de pantalla del interruptor). */
export const ToggleLabelCtx = createContext<string | undefined>(undefined);

/** Interruptor: para lo que se enciende o se apaga. Para elegir entre varias cosas, un desplegable. */
export function Toggle({ checked, onChange, disabled, label }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; label?: string }) {
  const fromRow = useContext(ToggleLabelCtx);
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label ?? fromRow}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors focus-visible:ring-2 focus-visible:ring-neon/60 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40 ${
        checked ? "bg-neon" : "bg-line-2"
      }`}
    >
      <span className={`absolute size-4 rounded-full shadow transition-[left] ${checked ? "left-[18px] bg-white" : "left-0.5 bg-dim"}`} />
    </button>
  );
}

/** Botón de solo icono, con su nombre para el ratón y para los lectores de pantalla. */
export function IconButton({
  label,
  onClick,
  disabled,
  danger,
  children,
}: {
  label: string;
  onClick?: () => void;
  disabled?: boolean;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button type="button" onClick={onClick} disabled={disabled} title={label} aria-label={label} className={`${iconBtn} ${danger ? "ico-lid hover:text-bad" : "ico-pop"}`}>
      {children}
    </button>
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
  size = "md",
}: {
  children: ReactNode;
  /** Si devuelve una promesa, el botón enseña que trabaja y termina en «Hecho». */
  onClick?: () => unknown;
  disabled?: boolean;
  kind?: "primary" | "danger" | "ghost" | "secondary";
  title?: string;
  /** "sm": la altura de los botones dentro de una fila o una tarjeta. */
  size?: "md" | "sm";
}) {
  const [state, setState] = useState<"idle" | "busy" | "done">("idle");
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const style = {
    primary: "border-neon bg-neon text-on-neon hover:brightness-110",
    danger: "border-bad/50 text-bad hover:bg-bad/10",
    ghost: "border-transparent text-dim hover:bg-panel-2 hover:text-ink",
    secondary: "border-line-2 text-dim hover:border-neon/40 hover:text-ink",
  }[kind];
  const click = () => {
    if (state === "busy") return;
    const r = onClick?.();
    if (!(r instanceof Promise)) return;
    setState("busy");
    r.then(
      () => {
        if (!alive.current) return;
        setState("done");
        window.setTimeout(() => alive.current && setState("idle"), 1200);
      },
      () => alive.current && setState("idle"),
    );
  };
  return (
    <button
      onClick={click}
      disabled={disabled || state === "busy"}
      title={title}
      aria-busy={state === "busy" || undefined}
      className={`relative flex items-center gap-1.5 rounded-lg border font-medium transition-colors disabled:pointer-events-none disabled:opacity-40 ${size === "sm" ? "h-8 px-2.5 text-xs" : "h-9 px-3.5 text-[13px]"} ${style} ${state === "busy" ? "!opacity-80" : ""}`}
    >
      <span className={`flex items-center gap-1.5 ${state === "idle" ? "" : "invisible"}`}>{children}</span>
      {state !== "idle" && (
        <span className="absolute inset-0 flex items-center justify-center gap-1.5">
          {state === "busy" ? <Loader2 size={14} className="animate-spin" /> : <><Check size={14} /> Hecho</>}
        </span>
      )}
    </button>
  );
}

/** Estado de carga igual en toda la app. `page`: ocupa el sitio de una página entera. */
export function Loading({ text = "Cargando…", page = false }: { text?: string; page?: boolean }) {
  if (page) return <PageSkeleton text={text} />;
  // La forma de lo que va a llegar, con lo que se está haciendo en letra pequeña.
  return (
    <div className="max-w-xl">
      <p className="flex items-center gap-1.5 pt-1 text-[11px] text-mute" aria-hidden>
        <Loader2 size={11} className="animate-spin" /> {text}
      </p>
      <SkeletonRows rows={3} label={text} />
    </div>
  );
}

/**
 * Silueta de una pantalla mientras carga: la forma de lo que viene (cifras arriba,
 * tarjetas debajo) en vez de un círculo girando. Se nota menos la espera y la
 * página no salta al llegar los datos.
 */
function PageSkeleton({ text }: { text: string }) {
  const bar = "skeleton";
  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6" role="status" aria-live="polite">
      <span className="sr-only">{text}</span>
      <p className="flex items-center gap-2 text-xs text-mute" aria-hidden>
        <Loader2 size={12} className="animate-spin" /> {text}
      </p>
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4" aria-hidden>
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="rounded-xl border border-line bg-panel px-3 py-3">
            <div className={`h-5 w-16 ${bar}`} />
            <div className={`mt-2 h-2.5 w-24 ${bar}`} />
          </div>
        ))}
      </div>
      {[0, 1].map((c) => (
        <div key={c} className="space-y-2.5 rounded-xl border border-line bg-panel p-4" aria-hidden>
          <div className={`h-3 w-40 ${bar}`} />
          {[92, 78, 85, 64].map((w, i) => (
            <div key={i} className={`h-2.5 ${bar}`} style={{ width: `${w - c * 10}%` }} />
          ))}
        </div>
      ))}
    </div>
  );
}

/**
 * La lectura falló: se dice qué pasó y se ofrece reintentar. Sin esto, una
 * página cuya primera lectura fallaba se quedaba en «Cargando…» para siempre.
 */
export function ErrorState({ message, onRetry, page = false, where }: { message: string; onRetry?: () => void; page?: boolean; /** Pantalla o tarjeta, para los detalles de soporte. */ where?: string }) {
  const e = explainError(message);
  const [copied, setCopied] = useState(false);
  const copy = () =>
    void navigator.clipboard?.writeText(supportDetails(message, where)).then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    });
  const admin = () => void import("../lib/api").then(({ api }) => api.relaunchAsAdmin());
  return (
    <div className={page ? "p-8" : ""}>
      <div className="rounded-xl border border-bad/30 bg-bad/5 px-4 py-3">
        <div className="flex flex-wrap items-start gap-3">
          <TriangleAlert size={16} className="mt-0.5 shrink-0 text-bad" />
          <div className="min-w-0 flex-1 text-sm text-ink">
            <p className="break-words">
              <CodeLinks text={e.what} />
            </p>
            <p className="mt-1 text-xs text-dim">{e.why}</p>
            {e.tries.length > 0 && (
              <ul className="mt-1.5 list-disc space-y-0.5 pl-4 text-xs text-dim">
                {e.tries.map((t) => (
                  <li key={t}>{t}</li>
                ))}
              </ul>
            )}
          </div>
        </div>
        <div className="mt-2.5 flex flex-wrap items-center justify-end gap-2">
          <button onClick={copy} className={smallBtn} title="Texto técnico sin rutas ni nombres de usuario">
            {copied ? <Check size={12} /> : <ClipboardCopy size={12} />} {copied ? "Copiado" : "Copiar detalles para soporte"}
          </button>
          {e.fix === "admin" && (
            <button onClick={admin} className={smallBtn}>
              <ShieldCheck size={12} /> Reiniciar como administrador
            </button>
          )}
          {onRetry && (
            <Button kind="ghost" size="sm" onClick={onRetry}>
              <RotateCw size={14} /> Reintentar
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

/** Lista sin nada que enseñar (o búsqueda sin resultados), dentro de una tarjeta. */
export function EmptyLine({ children }: { children: ReactNode }) {
  return <p className="px-4 py-8 text-center text-sm text-mute">{children}</p>;
}

/** Cifra de resumen arriba de una página. Con `onClick` filtra la lista. */
export function Tile({
  label,
  value,
  warn = false,
  active = false,
  onClick,
}: {
  label: string;
  value: ReactNode;
  /** En ámbar cuando no es cero. */
  warn?: boolean;
  active?: boolean;
  onClick?: () => void;
}) {
  const cls = `rounded-xl border px-3 py-2.5 text-left ${active ? "border-neon/50 bg-neon/10" : "border-line bg-panel"} ${onClick ? "lift glow hover:border-line-2" : ""}`;
  const body = (
    <>
      <div className={`font-mono text-xl leading-none font-semibold ${warn && value !== 0 && value !== "0" ? "text-warn" : "text-ink"}`}>
        <AnimatedValue value={value} />
      </div>
      <div className="mt-1 truncate text-[11px] text-mute">{label}</div>
    </>
  );
  return onClick ? (
    <button onClick={onClick} className={cls}>
      {body}
    </button>
  ) : (
    <div className={cls}>{body}</div>
  );
}

/** Lista vacía igual en toda la app: qué pasa y, si procede, qué hacer. */
export function EmptyState({ icon, title, children, action }: { icon?: ReactNode; title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-2 rounded-xl border border-dashed border-line px-6 py-10 text-center">
      {icon ? <span className="text-mute">{icon}</span> : <EmptyArt />}
      <p className="text-sm text-ink">{title}</p>
      {children && <p className="max-w-md text-xs text-mute">{children}</p>}
      {action && <div className="mt-1">{action}</div>}
    </div>
  );
}

/** Ilustración sencilla de «aquí todavía no hay nada»: una caja abierta. */
export function EmptyArt({ size = 72 }: { size?: number }) {
  return (
    <svg width={size} height={size * 0.75} viewBox="0 0 96 72" fill="none" aria-hidden className="text-mute">
      <ellipse cx="48" cy="64" rx="30" ry="4" fill="currentColor" opacity="0.12" />
      <path d="M20 30 L48 40 L76 30 L76 56 L48 66 L20 56 Z" fill="var(--color-panel-2)" stroke="currentColor" strokeOpacity="0.5" strokeWidth="1.5" strokeLinejoin="round" />
      <path d="M48 40 V66" stroke="currentColor" strokeOpacity="0.4" strokeWidth="1.5" />
      <path d="M20 30 L10 20 L38 10 L48 20 Z" fill="var(--color-panel)" stroke="currentColor" strokeOpacity="0.5" strokeWidth="1.5" strokeLinejoin="round" />
      <path d="M76 30 L86 20 L58 10 L48 20 Z" fill="var(--color-panel)" stroke="currentColor" strokeOpacity="0.5" strokeWidth="1.5" strokeLinejoin="round" />
      <circle cx="66" cy="6" r="2" fill="var(--color-neon)" opacity="0.7" />
      <circle cx="28" cy="4" r="1.5" fill="var(--color-neon)" opacity="0.5" />
    </svg>
  );
}
