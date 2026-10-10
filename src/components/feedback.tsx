import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, Check, CheckCircle2, ClipboardCopy, Info, Loader2, Undo2, X, XCircle } from "lucide-react";
import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { tweaksApi } from "../lib/api";
import { Overlay } from "./ui";
import { notifyJournalChange } from "../lib/journalEvents";
import { supportDetails } from "../lib/errors";
import { playSound } from "../lib/sounds";

// ---------- Toasts ----------

type ToastKind = "ok" | "error" | "info";
interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
  /** Cambio del diario que se puede deshacer desde el propio aviso. */
  undo?: { entry: number; title: string };
  /** Cuánto dura a la vista (ms). */
  ms: number;
}

/** Lo que dura un aviso con «Deshacer»: lo que tarda en vaciarse su barrita. */
export const UNDO_MS = 8000;

const ToastCtx = createContext<(kind: ToastKind, text: string) => void>(() => {});

export const useToast = () => useContext(ToastCtx);

/** Cambio que se puede deshacer, a la espera del aviso «Aplicado» de la página que lo hizo. */
interface PendingUndo {
  entry: number;
  title: string;
  count: number;
  timer: number;
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [undoing, setUndoing] = useState<number | null>(null);
  const pending = useRef<PendingUndo | null>(null);
  // Temporizador de cada aviso: se para con el ratón encima y sigue al salir.
  const timers = useRef(new Map<number, { timer: number; left: number; since: number }>());
  const [paused, setPaused] = useState<number | null>(null);
  const [copied, setCopied] = useState<number | null>(null);

  const drop = useCallback((id: number) => {
    const t = timers.current.get(id);
    if (t) window.clearTimeout(t.timer);
    timers.current.delete(id);
    setToasts((x) => x.filter((y) => y.id !== id));
  }, []);

  const arm = useCallback(
    (id: number, ms: number) => {
      const timer = window.setTimeout(() => drop(id), ms);
      timers.current.set(id, { timer, left: ms, since: Date.now() });
    },
    [drop],
  );

  const add = useCallback(
    (kind: ToastKind, text: string, undo?: Toast["undo"]) => {
      const id = Date.now() + Math.random();
      const ms = undo ? UNDO_MS : kind === "error" ? 8000 : 3000;
      setToasts((t) => [...t.slice(-3), { id, kind, text, undo, ms }]);
      arm(id, ms);
      if (kind === "error") void playSound("error");
    },
    [arm],
  );

  const hold = (id: number) => {
    const t = timers.current.get(id);
    if (!t) return;
    window.clearTimeout(t.timer);
    t.left = Math.max(800, t.left - (Date.now() - t.since));
    setPaused(id);
  };
  const resume = (id: number) => {
    const t = timers.current.get(id);
    setPaused(null);
    if (t) arm(id, t.left);
  };

  // Solo un cambio suelto lleva «Deshacer»: si una acción aplica varios a la vez
  // (un perfil, por ejemplo), deshacer uno solo confundiría: eso va al Historial.
  const takeUndo = () => {
    const p = pending.current;
    if (!p) return undefined;
    window.clearTimeout(p.timer);
    pending.current = null;
    return p.count === 1 ? { entry: p.entry, title: p.title } : undefined;
  };

  const push = useCallback(
    (kind: ToastKind, text: string) => add(kind, text, kind === "ok" ? takeUndo() : undefined),
    [add],
  );

  useEffect(() => {
    const off = listen<{ id: number; title: string }>("undoable-change", ({ payload }) => {
      const p = pending.current;
      if (p) window.clearTimeout(p.timer);
      const count = (p?.count ?? 0) + 1;
      // La página suele avisar justo después; si no lo hace, el aviso lo pone esto.
      const timer = window.setTimeout(() => {
        const u = takeUndo();
        if (u) add("ok", `Cambio aplicado: ${u.title}`, u);
      }, 2500);
      pending.current = { entry: payload.id, title: payload.title, count, timer };
    });
    return () => {
      void off.then((f) => f());
    };
  }, [add]);

  const undo = async (t: Toast) => {
    if (!t.undo) return;
    setUndoing(t.id);
    try {
      const r = await tweaksApi.revertEntry(t.undo.entry);
      drop(t.id);
      add("ok", `${t.undo.title}: ${r.message}`);
      notifyJournalChange();
    } catch (e) {
      add("error", `No se pudo deshacer: ${e}`);
    } finally {
      setUndoing(null);
    }
  };

  const icon = { ok: CheckCircle2, error: XCircle, info: Info };
  const color = { ok: "text-ok", error: "text-bad", info: "text-neon" };

  return (
    <ToastCtx.Provider value={push}>
      {children}
      {/* Abajo en el centro y «atravesables»: un aviso encima de un botón no lo bloquea (solo responden su X y «Deshacer»). */}
      <div className="pointer-events-none fixed bottom-5 left-1/2 z-50 flex w-96 max-w-[90vw] -translate-x-1/2 flex-col gap-2">
        {toasts.map((t) => {
          const Icon = icon[t.kind];
          return (
            <div
              key={t.id}
              data-toast={t.kind}
              onMouseEnter={() => hold(t.id)}
              onMouseLeave={() => resume(t.id)}
              className={`${t.undo ? "pointer-events-auto" : ""} relative flex items-start gap-2.5 overflow-hidden rounded-lg border border-line-2 bg-panel-2/95 px-3.5 py-3 text-sm shadow-2xl ${paused === t.id ? "toast-paused" : ""}`}
            >
              <Icon size={16} className={`mt-0.5 shrink-0 ${color[t.kind]}`} />
              <p className="max-h-32 min-w-0 flex-1 overflow-y-auto text-ink [overflow-wrap:anywhere]">{t.text}</p>
              {t.kind === "error" && (
                <button
                  onClick={() =>
                    void navigator.clipboard?.writeText(supportDetails(t.text)).then(() => {
                      setCopied(t.id);
                      window.setTimeout(() => setCopied(null), 1500);
                    })
                  }
                  className="pointer-events-auto shrink-0 rounded p-0.5 text-mute hover:text-ink"
                  title="Copiar detalles para soporte (sin rutas ni nombres)"
                  aria-label="Copiar detalles para soporte"
                >
                  {copied === t.id ? <Check size={14} className="text-ok" /> : <ClipboardCopy size={14} />}
                </button>
              )}
              {t.undo && (
                <button
                  onClick={() => undo(t)}
                  disabled={undoing !== null}
                  className="flex shrink-0 items-center gap-1 rounded-md border border-line-2 px-2 py-0.5 text-xs text-dim transition-colors hover:border-neon/40 hover:text-neon disabled:opacity-40"
                >
                  {undoing === t.id ? <Loader2 size={11} className="animate-spin" /> : <Undo2 size={11} />} Deshacer
                </button>
              )}
              <button onClick={() => drop(t.id)} className="pointer-events-auto text-mute hover:text-ink" aria-label="Cerrar aviso">
                <X size={14} />
              </button>
              {t.undo && <span className="toast-countdown absolute bottom-0 left-0 h-0.5 w-full bg-neon/70" style={{ animationDuration: `${t.ms}ms` }} aria-hidden />}
            </div>
          );
        })}
      </div>
    </ToastCtx.Provider>
  );
}

// ---------- Confirmación ----------

export interface ConfirmOptions {
  title: string;
  body: ReactNode;
  confirmLabel: string;
  danger?: boolean;
}

export function ConfirmDialog({
  options,
  onClose,
}: {
  options: ConfirmOptions | null;
  onClose: (confirmed: boolean) => void;
}) {
  if (!options) return null;
  return (
    <Overlay onClose={() => onClose(false)} z="z-50" label={typeof options.title === "string" ? options.title : undefined}>
      <div
        className="w-[440px] max-w-full rounded-xl border border-line-2 bg-panel p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-3 flex items-center gap-2.5">
          <AlertTriangle size={18} className={options.danger ? "text-bad" : "text-warn"} />
          <h3 className="font-semibold">{options.title}</h3>
        </div>
        <div className="text-sm text-dim">{options.body}</div>
        <div className="mt-5 flex justify-end gap-2">
          <button
            onClick={() => onClose(false)}
            className="rounded-md px-3.5 py-1.5 text-sm text-dim transition-colors hover:bg-panel-2 hover:text-ink"
          >
            Cancelar
          </button>
          <button
            autoFocus
            onClick={() => onClose(true)}
            className={`rounded-md border px-3.5 py-1.5 text-sm font-medium transition-colors ${
              options.danger
                ? "border-bad/50 text-bad hover:bg-bad/10"
                : "border-neon/50 text-neon hover:bg-neon/10"
            }`}
          >
            {options.confirmLabel}
          </button>
        </div>
      </div>
    </Overlay>
  );
}

/** Hook que expone `confirm()` como promesa y el diálogo a renderizar. */
export function useConfirm() {
  const [state, setState] = useState<{ options: ConfirmOptions; resolve: (v: boolean) => void } | null>(null);
  const confirm = useCallback(
    (options: ConfirmOptions) => new Promise<boolean>((resolve) => setState({ options, resolve })),
    [],
  );
  const dialog = (
    <ConfirmDialog
      options={state?.options ?? null}
      onClose={(v) => {
        state?.resolve(v);
        setState(null);
      }}
    />
  );
  return { confirm, dialog };
}
