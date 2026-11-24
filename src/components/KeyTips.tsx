// Mantén Alt y cada botón dice su letra (como en Word y Excel).
//
// - Alt solo, mantenido 250 ms: salen las letras; se pulsa una (con Alt aún
//   pulsado) y se hace. Soltar Alt sin pulsar nada lo cancela.
// - Alt pulsado y soltado enseguida, sin otra tecla: las letras se quedan (modo
//   Office) hasta pulsar una, Esc, un clic o unos segundos.
// No toca Alt+←/→, Alt+↑/↓, Alt+F4, Alt+Tab ni AltGr (Ctrl+Alt en teclados
// españoles): cualquier otra tecla con Alt, o Ctrl pulsado, no lo activa.
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { assignCodes, pressCode } from "../lib/keytips";
import { isTypingPlace } from "../lib/typeFilter";

const HOLD_MS = 250;
const OFFICE_MS = 4000;
const MAX_TIPS = 120;
const CLICKABLE = 'button, [role="tab"], a[href], [role="menuitem"], summary';

interface Tip {
  el: HTMLElement;
  code: string;
}

/** Los elementos pulsables que se ven ahora (si hay un diálogo abierto, solo los suyos), con su letra. */
function collect(): Tip[] {
  const dialogs = document.querySelectorAll('[role="dialog"],[aria-modal="true"]');
  const root: ParentNode = dialogs.length ? dialogs[dialogs.length - 1] : document;
  const els = [...root.querySelectorAll<HTMLElement>(CLICKABLE)].filter((el) => {
    if ((el as HTMLButtonElement).disabled || el.getAttribute("aria-disabled") === "true") return false;
    if (el.closest('[aria-hidden="true"],[data-keytips]')) return false;
    if (el.getClientRects().length === 0) return false;
    const r = el.getBoundingClientRect();
    if (r.width < 6 || r.height < 6 || r.bottom <= 0 || r.right <= 0 || r.top >= window.innerHeight || r.left >= window.innerWidth) return false;
    return getComputedStyle(el).visibility !== "hidden";
  });
  const shown = els.slice(0, MAX_TIPS);
  const codes = assignCodes(
    shown.map((el) => ({
      kt: el.dataset.kt,
      text: el.getAttribute("aria-label") || el.textContent || el.title || "",
      side: !!el.closest("[data-kt-side]"),
    })),
  );
  const out: Tip[] = [];
  shown.forEach((el, i) => {
    const code = codes[i];
    if (code) out.push({ el, code });
  });
  return out;
}

/**
 * La letra o cifra de una tecla, según su posición (vale con cualquier distribución del teclado).
 * El teclado numérico no cuenta: con Alt se usa para escribir códigos Alt (Alt+64 → @).
 */
function letterOf(ev: KeyboardEvent): string | null {
  const m = /^Key([A-Z])$/.exec(ev.code) ?? /^Digit(\d)$/.exec(ev.code);
  return m ? m[1] : null;
}

/** Ahora otra cosa usa las teclas: se está grabando un atajo o se prueba el teclado. */
function keysTaken(): boolean {
  return !!document.body.dataset.recordingShortcut || !!document.querySelector("[data-keytest]");
}

export function KeyTips({ disabled }: { disabled: boolean }) {
  const [mode, setMode] = useState<{ kind: "hold" | "office"; tips: Tip[]; typed: string } | null>(null);
  const [, redraw] = useState(0);
  const modeRef = useRef(mode);
  modeRef.current = mode;
  const disabledRef = useRef(disabled);
  disabledRef.current = disabled;
  const alt = useRef<{ down: boolean; other: boolean; timer: number }>({ down: false, other: false, timer: 0 });
  const officeTimer = useRef(0);

  const cancel = useCallback(() => {
    window.clearTimeout(officeTimer.current);
    setMode(null);
  }, []);

  useEffect(() => {
    const a = alt.current;
    const start = (kind: "hold" | "office") => {
      const tips = collect();
      if (!tips.length) return;
      setMode({ kind, tips, typed: "" });
      window.clearTimeout(officeTimer.current);
      if (kind === "office") officeTimer.current = window.setTimeout(cancel, OFFICE_MS);
    };
    const press = (letter: string): boolean => {
      const m = modeRef.current;
      if (!m) return false;
      const typed = m.typed + letter;
      const r = pressCode(
        m.tips.map((t) => t.code),
        typed,
      );
      if (!r) return false;
      if ("pick" in r) {
        const el = m.tips[r.pick].el;
        cancel();
        el.click();
      } else setMode({ ...m, typed: r.typed });
      return true;
    };
    const onKeyDown = (ev: KeyboardEvent) => {
      if (disabledRef.current) return;
      if (keysTaken()) {
        // Grabando un atajo o probando el teclado: las letras con Alt son de ellos.
        a.down = false;
        window.clearTimeout(a.timer);
        if (modeRef.current) cancel();
        return;
      }
      if (ev.key === "Alt") {
        if (ev.repeat) {
          if (modeRef.current) ev.preventDefault();
          return;
        }
        // AltGr en teclados españoles es Ctrl+Alt: no es esto.
        if (ev.ctrlKey || ev.shiftKey || ev.metaKey || ev.getModifierState("AltGraph")) return;
        if (modeRef.current) {
          cancel();
          return;
        }
        a.down = true;
        a.other = false;
        window.clearTimeout(a.timer);
        a.timer = window.setTimeout(() => {
          if (!a.down || a.other) return;
          a.other = true;
          // Escribiendo en un campo, Alt+letra no se toca (tampoco en el modo mantener).
          if (isTypingPlace(document.activeElement)) return;
          start("hold");
        }, HOLD_MS);
        return;
      }
      const m = modeRef.current;
      if (!m) {
        // Otra tecla con Alt (Alt+←, Alt+Tab…): era un atajo, no el modo.
        if (a.down) {
          a.other = true;
          window.clearTimeout(a.timer);
        }
        return;
      }
      a.other = true;
      if (ev.key === "Escape") {
        ev.preventDefault();
        ev.stopPropagation();
        cancel();
        return;
      }
      const letter = ev.ctrlKey || ev.metaKey ? null : letterOf(ev);
      if (letter && press(letter)) {
        ev.preventDefault();
        ev.stopPropagation();
        return;
      }
      // Una tecla que no es de las letras (flechas, Tab…): se cancela y sigue su camino.
      if (!["Shift", "Control", "Meta"].includes(ev.key)) cancel();
    };
    const onKeyUp = (ev: KeyboardEvent) => {
      if (ev.key !== "Alt") return;
      const quick = a.down && !a.other && !modeRef.current;
      a.down = false;
      window.clearTimeout(a.timer);
      const m = modeRef.current;
      if (m?.kind === "hold") {
        // Soltar Alt sin haber pulsado nada cancela; con la primera letra de dos, se sigue.
        ev.preventDefault();
        if (m.typed) {
          setMode({ ...m, kind: "office" });
          window.clearTimeout(officeTimer.current);
          officeTimer.current = window.setTimeout(cancel, OFFICE_MS);
        } else cancel();
      } else if (quick && !disabledRef.current && !keysTaken() && !isTypingPlace(document.activeElement)) {
        ev.preventDefault();
        start("office");
      }
    };
    const stop = () => {
      a.down = false;
      window.clearTimeout(a.timer);
      if (modeRef.current) cancel();
    };
    const onScroll = () => modeRef.current && redraw((n) => n + 1);
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    window.addEventListener("blur", stop);
    window.addEventListener("mousedown", stop, true);
    window.addEventListener("resize", onScroll);
    window.addEventListener("scroll", onScroll, true);
    return () => {
      window.clearTimeout(a.timer);
      window.clearTimeout(officeTimer.current);
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
      window.removeEventListener("blur", stop);
      window.removeEventListener("mousedown", stop, true);
      window.removeEventListener("resize", onScroll);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [cancel]);

  if (!mode) return null;
  return createPortal(
    <div data-keytips aria-hidden className="pointer-events-none fixed inset-0 z-[95]">
      {mode.tips
        .filter((t) => t.code.startsWith(mode.typed))
        .map((t) => {
          const r = t.el.getBoundingClientRect();
          return (
            <span
              key={t.code}
              className="absolute rounded-[3px] bg-[#f5d76e] px-1 py-0.5 font-mono text-[11px] leading-none font-bold text-[#1b1b1b] shadow-md"
              style={{ left: Math.max(0, r.left - 2), top: Math.max(0, r.top - 2) }}
            >
              {mode.typed ? <span className="opacity-50">{mode.typed}</span> : null}
              {t.code.slice(mode.typed.length)}
            </span>
          );
        })}
      <div className="absolute bottom-6 left-1/2 -translate-x-1/2 rounded-full border border-line-2 bg-panel px-3.5 py-1.5 text-xs text-ink shadow-xl">
        {mode.kind === "hold" ? "Alt pulsado · suelta Alt para cancelar" : "Pulsa una letra · Esc cancela"}
      </div>
    </div>,
    document.body,
  );
}
