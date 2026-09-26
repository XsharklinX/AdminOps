import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import { createContext, useCallback, useContext, useState, type ReactNode } from "react";

// ---------- Toasts ----------

type ToastKind = "ok" | "error" | "info";
interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
}

const ToastCtx = createContext<(kind: ToastKind, text: string) => void>(() => {});

export const useToast = () => useContext(ToastCtx);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const push = useCallback((kind: ToastKind, text: string) => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t.slice(-3), { id, kind, text }]);
    window.setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), kind === "error" ? 8000 : 4000);
  }, []);

  const icon = { ok: CheckCircle2, error: XCircle, info: Info };
  const color = { ok: "text-ok", error: "text-bad", info: "text-neon" };

  return (
    <ToastCtx.Provider value={push}>
      {children}
      <div className="pointer-events-none fixed right-5 bottom-5 z-50 flex w-96 flex-col gap-2">
        {toasts.map((t) => {
          const Icon = icon[t.kind];
          return (
            <div
              key={t.id}
              className="pointer-events-auto flex items-start gap-2.5 rounded-lg border border-line-2 bg-panel-2/95 px-3.5 py-3 text-sm shadow-2xl backdrop-blur"
            >
              <Icon size={16} className={`mt-0.5 shrink-0 ${color[t.kind]}`} />
              <p className="max-h-32 min-w-0 flex-1 overflow-y-auto text-ink [overflow-wrap:anywhere]">{t.text}</p>
              <button onClick={() => setToasts((x) => x.filter((y) => y.id !== t.id))} className="text-mute hover:text-ink">
                <X size={14} />
              </button>
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
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/60 backdrop-blur-sm" onClick={() => onClose(false)}>
      <div
        className="w-[440px] rounded-xl border border-line-2 bg-panel p-5 shadow-2xl"
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
    </div>
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
