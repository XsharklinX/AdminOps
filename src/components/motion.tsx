// Pequeños detalles de movimiento compartidos: cifras que cuentan la primera
// vez que se ven, destello cuando cambian y siluetas de carga. Todo se apaga
// con «Reducir animaciones» (Ajustes → Apariencia) o la preferencia de Windows.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { getPrefs } from "../lib/prefs";

/** ¿Se pueden animar cosas? */
export function motionOk(): boolean {
  if (getPrefs().reduceMotion) return false;
  try {
    return !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return true;
  }
}

/** Valor intermedio de una cuenta que va de 0 a `to` (curva que frena al final). */
export function countFrame(to: number, t: number): number {
  const k = Math.min(1, Math.max(0, t));
  const eased = 1 - Math.pow(1 - k, 3);
  return to * eased;
}

/**
 * Un número que sube desde cero en medio segundo la primera vez que se pinta
 * y que, cuando cambia después, destella un instante en lugar de volver a contar.
 */
export function CountUp({ value, decimals = 0, suffix = "" }: { value: number; decimals?: number; suffix?: string }) {
  const [shown, setShown] = useState(() => (motionOk() ? 0 : value));
  const first = useRef(true);
  const flash = useFlash(value);
  useEffect(() => {
    if (!first.current) {
      setShown(value);
      return;
    }
    first.current = false;
    if (!motionOk() || value === 0) {
      setShown(value);
      return;
    }
    const start = performance.now();
    let raf = 0;
    const step = (now: number) => {
      const t = (now - start) / 500;
      setShown(countFrame(value, t));
      if (t < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [value]);
  const text = shown.toLocaleString("es-ES", { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
  return (
    <span className={flash ? "value-flash" : undefined}>
      {text}
      {suffix}
    </span>
  );
}

/** true durante un momento cada vez que `value` cambia (no al principio). */
export function useFlash(value: unknown): boolean {
  const [on, setOn] = useState(false);
  const prev = useRef(value);
  useEffect(() => {
    if (Object.is(prev.current, value)) return;
    prev.current = value;
    if (!motionOk()) return;
    setOn(true);
    const t = window.setTimeout(() => setOn(false), 900);
    return () => window.clearTimeout(t);
  }, [value]);
  return on;
}

/** Valor de una cifra de resumen: los números cuentan; el resto se pinta tal cual. */
export function AnimatedValue({ value }: { value: ReactNode }) {
  if (typeof value === "number" && Number.isFinite(value)) return <CountUp value={value} decimals={Number.isInteger(value) ? 0 : 1} />;
  return <>{value}</>;
}

/** Silueta de unas filas mientras llegan los datos. */
export function SkeletonRows({ rows = 3, label }: { rows?: number; label?: string }) {
  const widths = [92, 76, 84, 61, 70, 88];
  return (
    <div className="space-y-2 py-2" role="status" aria-live="polite">
      {label && <span className="sr-only">{label}</span>}
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="skeleton h-2.5" style={{ width: `${widths[i % widths.length]}%` }} aria-hidden />
      ))}
    </div>
  );
}

/** Silueta de unas tarjetas de cifras. */
export function SkeletonTiles({ count = 4 }: { count?: number }) {
  return (
    <div className="grid grid-cols-2 gap-3 lg:grid-cols-4" aria-hidden>
      {Array.from({ length: count }, (_, i) => (
        <div key={i} className="rounded-xl border border-line bg-panel px-3 py-3">
          <div className="skeleton h-5 w-16" />
          <div className="skeleton mt-2 h-2.5 w-24" />
        </div>
      ))}
    </div>
  );
}
