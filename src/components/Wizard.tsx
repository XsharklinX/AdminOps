// Asistente de pocos pasos para lo delicado: qué quieres hacer, qué se va a
// tocar y qué no, una comprobación de seguridad y la confirmación. En cada paso
// se ve cómo salir sin cambiar nada.
import { Check, ChevronLeft, ChevronRight, ShieldAlert } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Modal } from "./ui";

export interface WizardStep {
  title: string;
  body: ReactNode;
  /** Se puede pasar al siguiente (casillas marcadas, palabra escrita…). */
  ready?: boolean;
}

export function Wizard({ title, steps, finishLabel, danger = true, onClose, onFinish }: { title: string; steps: WizardStep[]; finishLabel: string; danger?: boolean; onClose: () => void; onFinish: () => void }) {
  const [i, setI] = useState(0);
  const step = steps[i];
  const last = i === steps.length - 1;
  const ready = step.ready !== false;
  return (
    <Modal
      title={title}
      onClose={onClose}
      width="w-[560px]"
      footer={
        <>
          <button onClick={onClose} className="mr-auto text-xs text-mute hover:text-ink">
            Salir sin cambiar nada
          </button>
          {i > 0 && (
            <Button kind="ghost" onClick={() => setI(i - 1)}>
              <ChevronLeft size={14} /> Atrás
            </Button>
          )}
          {last ? (
            <Button kind={danger ? "danger" : "primary"} disabled={!ready} onClick={onFinish}>
              {finishLabel}
            </Button>
          ) : (
            <Button disabled={!ready} onClick={() => setI(i + 1)}>
              Siguiente <ChevronRight size={14} />
            </Button>
          )}
        </>
      }
    >
      <ol className="mb-4 flex items-center gap-2" aria-label="Pasos">
        {steps.map((s, k) => (
          <li key={s.title} className="flex min-w-0 flex-1 items-center gap-2">
            <span className={`grid size-6 shrink-0 place-items-center rounded-full border text-[11px] font-semibold ${k < i ? "border-ok bg-ok/15 text-ok" : k === i ? "border-neon bg-neon/15 text-neon" : "border-line-2 text-mute"}`}>
              {k < i ? <Check size={12} /> : k + 1}
            </span>
            <span className={`truncate text-[11px] ${k === i ? "text-ink" : "text-mute"}`}>{s.title}</span>
            {k < steps.length - 1 && <span className="h-px min-w-3 flex-1 bg-line-2" />}
          </li>
        ))}
      </ol>
      <div className="text-sm text-dim">{step.body}</div>
    </Modal>
  );
}

/** Lista de comprobación: todas marcadas para seguir. */
export function Checks({ items, value, onChange }: { items: string[]; value: boolean[]; onChange: (v: boolean[]) => void }) {
  return (
    <div className="space-y-2">
      <p className="flex items-center gap-1.5 text-xs text-warn">
        <ShieldAlert size={13} /> Antes de seguir, confirma que:
      </p>
      {items.map((it, k) => (
        <label key={it} className="flex cursor-pointer items-start gap-2 rounded-md border border-line px-3 py-2 text-[13px] text-ink hover:border-line-2">
          <input type="checkbox" checked={!!value[k]} onChange={(e) => onChange(items.map((_, j) => (j === k ? e.target.checked : !!value[j])))} className="mt-0.5 accent-[var(--color-neon)]" />
          {it}
        </label>
      ))}
    </div>
  );
}
