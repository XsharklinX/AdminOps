// «Todo en orden»: un ✓ grande y breve cuando se resuelve lo último pendiente
// de una revisión. Nada de confeti: da sensación de trabajo cerrado y se va solo.
// Se apaga en Ajustes → Apariencia.
import { useEffect, useState } from "react";
import { getPrefs } from "../lib/prefs";
import { playSound } from "../lib/sounds";

const EVENT = "adminops-celebrate";

/** Enseña el ✓. `force`: aunque esté apagado (para verlo desde Ajustes). */
export function celebrate(text: string, force = false) {
  if (!force && !getPrefs().celebrate) return;
  window.dispatchEvent(new CustomEvent<string>(EVENT, { detail: text }));
}

/** Frase de cierre según cuántas cosas se arreglaron hoy. */
export function celebrationText(fixedToday: number): string {
  if (fixedToday <= 0) return "Todo en orden";
  return `Todo en orden · ${fixedToday} ${fixedToday === 1 ? "cosa arreglada" : "cosas arregladas"} hoy`;
}

export function CelebrateLayer() {
  const [text, setText] = useState<string | null>(null);
  const [n, setN] = useState(0);
  useEffect(() => {
    const on = (e: Event) => {
      setText((e as CustomEvent<string>).detail);
      setN((x) => x + 1);
      void playSound("done");
    };
    window.addEventListener(EVENT, on);
    return () => window.removeEventListener(EVENT, on);
  }, []);
  useEffect(() => {
    if (!text) return;
    const t = window.setTimeout(() => setText(null), 2300);
    return () => window.clearTimeout(t);
  }, [text, n]);
  if (!text) return null;
  return (
    <div className="pointer-events-none fixed inset-0 z-[60] grid place-items-center" role="status" aria-live="polite">
      <div key={n} className="celebrate flex flex-col items-center gap-3 rounded-2xl border border-ok/30 bg-panel/95 px-10 py-7 shadow-2xl">
        <svg width="76" height="76" viewBox="0 0 76 76" aria-hidden>
          <circle cx="38" cy="38" r="34" fill="color-mix(in srgb, var(--color-ok) 14%, transparent)" stroke="var(--color-ok)" strokeWidth="3" />
          <path d="M24 39 L34 49 L53 29" fill="none" stroke="var(--color-ok)" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        <p className="text-base font-semibold text-ink">{text}</p>
      </div>
    </div>
  );
}
