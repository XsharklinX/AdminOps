import { ChevronDown, Loader2, Play, ShieldCheck, TriangleAlert } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { TaskStatus } from "./TaskStatus";
import { ChangePreview } from "./ChangePreview";
import type { Risk, TweakStatus, TweakView } from "../lib/api";

const RISK: Record<Risk, { label: string; cls: string }> = {
  low: { label: "Riesgo bajo", cls: "text-ok border-ok/30 bg-ok/5" },
  medium: { label: "Riesgo medio", cls: "text-warn border-warn/30 bg-warn/5" },
  high: { label: "Riesgo alto", cls: "text-bad border-bad/30 bg-bad/5" },
};

const STATUS: Partial<Record<TweakStatus, { label: string; cls: string }>> = {
  applied: { label: "Aplicado", cls: "text-neon" },
  notApplied: { label: "Por defecto", cls: "text-mute" },
  partial: { label: "Parcial", cls: "text-warn" },
  unavailable: { label: "No disponible en este equipo", cls: "text-mute" },
  unknown: { label: "Estado desconocido", cls: "text-warn" },
};

/**
 * Interruptor de un ajuste de Windows. Cambia en cuanto se pulsa (con un pequeño
 * rebote) y Windows lo confirma por detrás: `busy` mientras tanto. Si al terminar
 * el ajuste no cambió, vuelve a su sitio y se sacude.
 */
export function Switch({ on, disabled, onClick, busy = false }: { on: boolean; disabled?: boolean; onClick: () => void; busy?: boolean }) {
  const [want, setWant] = useState<boolean | null>(null);
  const [shake, setShake] = useState(false);
  const wasBusy = useRef(false);
  useEffect(() => {
    if (wasBusy.current && !busy) {
      // Terminó: lo que se ve vuelve a ser lo que dice Windows.
      if (want !== null && want !== on) {
        setShake(true);
        window.setTimeout(() => setShake(false), 320);
      }
      setWant(null);
    }
    wasBusy.current = busy;
  }, [busy, on, want]);
  const shown = want ?? on;
  return (
    <button
      role="switch"
      aria-checked={shown}
      aria-busy={busy || undefined}
      disabled={disabled}
      onClick={() => {
        if (busy) return;
        setWant(!on);
        onClick();
      }}
      className={`relative h-6 w-11 shrink-0 rounded-full border transition-all disabled:cursor-not-allowed disabled:opacity-40 ${
        shown ? "border-neon/60 bg-neon/20" : "border-line-2 bg-void"
      } ${busy && want !== null ? "opacity-80" : ""} ${shake ? "shake" : ""}`}
    >
      <span
        className={`absolute top-0.5 size-4.5 rounded-full transition-[left,background-color] duration-300 ease-[cubic-bezier(0.34,1.56,0.64,1)] ${
          shown ? "left-[22px] bg-neon" : "left-0.5 bg-mute"
        }`}
      />
    </button>
  );
}

export function TweakCard({
  tweak: t,
  busy,
  busyLabel,
  lastMessage,
  isAdmin,
  onToggle,
  onRun,
  highlighted = false,
}: {
  tweak: TweakView;
  busy: boolean;
  busyLabel?: string;
  lastMessage?: string;
  isAdmin: boolean;
  onToggle: () => void;
  onRun: () => void;
  /** Resaltado al llegar desde un hallazgo del diagnóstico. */
  highlighted?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const blocked = (t.needsAdmin && !isAdmin) || !t.supported || t.status === "unavailable";
  // "Parcial" se muestra apagado: al pulsarlo se completa el ajuste.
  const on = t.status === "applied";
  const st = STATUS[t.status];

  return (
    <article
      id={`focus-${t.id}`}
      className={`scroll-mt-6 rounded-xl border bg-panel transition-all duration-500 ${
        t.status === "applied" ? "border-neon/25" : "border-line"
      } ${blocked ? "opacity-70" : ""} ${highlighted ? "border-neon! glow-neon" : ""}`}
    >
      <div className="flex items-start gap-4 p-4">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="font-medium text-ink">{t.name}</h3>
            <span className={`rounded border px-1.5 py-px text-[11px] font-medium ${RISK[t.risk].cls}`}>
              {RISK[t.risk].label}
            </span>
            {t.reboot && (
              <span className="rounded border border-line-2 px-1.5 py-px text-[11px] text-dim">Requiere reinicio</span>
            )}
            {t.needsAdmin && !isAdmin && (
              <span className="rounded border border-warn/30 px-1.5 py-px text-[11px] text-warn">Requiere admin</span>
            )}
          </div>
          <p className="mt-1 text-sm text-dim">{t.description}</p>
          {t.note && (
            <p className="mt-2 flex gap-1.5 text-xs text-warn/90">
              <TriangleAlert size={13} className="mt-px shrink-0" />
              {t.note}
            </p>
          )}
          <div className="mt-2.5 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
            {busy ? (
              <TaskStatus task={`tweak:${t.id}`} active={busy} fallback={busyLabel} />
            ) : (
              <>
                {st && <span className={`font-medium ${st.cls}`}>● {st.label}</span>}
                {t.hasBackup && (
                  <span className="flex items-center gap-1 text-dim" title="AdminOps guardó el estado anterior">
                    <ShieldCheck size={12} className="text-ok" /> Copia guardada
                  </span>
                )}
                {lastMessage && <span className="text-dim">{lastMessage}</span>}
              </>
            )}
            {t.changes.length > 0 && (
              <button onClick={() => setOpen(!open)} className="flex items-center gap-1 text-mute hover:text-ink">
                Qué cambia
                <ChevronDown size={12} className={`transition-transform ${open ? "rotate-180" : ""}`} />
              </button>
            )}
          </div>
        </div>

        <div className="pt-0.5">
          {t.kind === "action" ? (
            <button
              onClick={onRun}
              disabled={busy || blocked}
              className="flex items-center gap-1.5 rounded-md border border-neon/40 px-3 py-1.5 text-xs font-medium text-neon transition-colors hover:bg-neon/10 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {busy ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} />}
              Ejecutar
            </button>
          ) : (
            <Switch on={on} disabled={blocked} busy={busy} onClick={onToggle} />
          )}
        </div>
      </div>

      {open && (
        <div className="space-y-2 border-t border-line bg-void/40 px-4 py-3">
          {/* Antes → después con los valores de este equipo, ahora mismo. */}
          <ChangePreview ids={[t.id]} compact />
          <ul className="space-y-1 font-mono text-[11px] text-mute">
            {t.changes.map((c) => (
              <li key={c} className="break-all select-text">
                <span className="text-neon/70">›</span> {c}
              </li>
            ))}
          </ul>
        </div>
      )}
    </article>
  );
}
