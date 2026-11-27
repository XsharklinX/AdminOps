// Piezas comunes de las pantallas de impresoras, escáneres y drivers.
import { CheckCircle2, Circle, CircleAlert, Loader2, MinusCircle, XCircle } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { NeedsAdmin } from "../AdminBanner";
import { HoverCard } from "../HoverCard";
import { onVendorEvent, type DriverStatus, type VendorKey, type VendorStepEvent } from "../../lib/api";
import { applyStep, chipText, isAdminError, isAuditError, STATUS_TONE, UPDATE_STEPS, VENDOR_NAME, type StepView, type Tone } from "./logic";
import { Notice } from "../ui";

const TONE_CLASS: Record<Tone, string> = {
  ok: "border-ok/40 bg-ok/10 text-ok",
  warn: "border-warn/40 bg-warn/10 text-warn",
  bad: "border-bad/40 bg-bad/10 text-bad",
  mute: "border-line text-mute",
};

/** La marca, discreta: RICOH en rojo, EPSON en azul. Las demás no llevan insignia. */
export function BrandBadge({ vendor }: { vendor: VendorKey }) {
  if (vendor === "otra") return null;
  const tone = vendor === "ricoh" ? "border-bad/40 text-bad" : "border-neon/40 text-neon";
  return <span className={`shrink-0 rounded border px-1.5 py-px font-mono text-[10px] font-semibold tracking-wide ${tone}`}>{VENDOR_NAME[vendor]}</span>;
}

/** El estado del driver en una palabra; el motivo sale al pasar el ratón. */
export function DriverChip({
  status,
  label,
  reason,
  newVersion,
  source,
  onClick,
  disabledReason,
}: {
  status: DriverStatus;
  label: string;
  reason: string;
  newVersion: string | null;
  source: string | null;
  onClick?: () => void;
  disabledReason?: string;
}) {
  const text = chipText(status, label, newVersion);
  const cls = `inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-[11px] font-medium whitespace-nowrap ${TONE_CLASS[STATUS_TONE[status]]}`;
  const disabled = !!disabledReason;
  const card = (
    <div className="space-y-1.5">
      <div className="font-medium text-ink">{label || text}</div>
      {reason && <p className="leading-relaxed text-dim">{reason}</p>}
      {newVersion && (
        <p className="text-mute">
          Hay la versión {newVersion}
          {source ? ` en ${source}` : ""}.
        </p>
      )}
      {disabledReason && <Notice tone="warn">{disabledReason}</Notice>}
      {onClick && !disabled && <p className="text-mute">Clic para ver el driver y cambiarlo.</p>}
    </div>
  );
  return (
    <HoverCard card={card}>
      {onClick && !disabled ? (
        <button type="button" onClick={onClick} className={`${cls} cursor-pointer hover:brightness-110`}>
          {text}
        </button>
      ) : (
        <span className={`${cls} ${disabled ? "pointer-events-none opacity-60" : ""}`}>{text}</span>
      )}
    </HoverCard>
  );
}

/** Los pasos de una operación, con el estado de cada uno. Los que aún no han empezado salen apagados. */
export function StepList({ steps, table = UPDATE_STEPS }: { steps: StepView[]; table?: { step: string; label: string }[] }) {
  return (
    <ul className="space-y-1.5" aria-live="polite">
      {table.map((t) => {
        const s = steps.find((x) => x.step === t.step);
        const state = s?.state;
        const Icon = state === "ok" ? CheckCircle2 : state === "error" ? XCircle : state === "skipped" ? MinusCircle : state === "running" ? Loader2 : Circle;
        const color = state === "ok" ? "text-ok" : state === "error" ? "text-bad" : state === "running" ? "text-neon" : "text-mute";
        return (
          <li key={t.step} className={`flex items-start gap-2 text-xs ${state ? "text-ink" : "text-mute"}`}>
            <Icon size={14} className={`mt-px shrink-0 ${color} ${state === "running" ? "animate-spin" : ""}`} />
            <span className="min-w-0 flex-1">
              {t.label}
              {s?.message && <span className={`block text-[11px] ${state === "error" ? "text-bad" : "text-mute"}`}>{s.message}</span>}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

/** Los pasos en vivo de una impresora (evento `printvendor:step`) mientras `active`. */
export function useVendorSteps(printer: string | null, active: boolean, table = UPDATE_STEPS): [StepView[], () => void] {
  const [steps, setSteps] = useState<StepView[]>([]);
  useEffect(() => {
    if (!printer || !active) return;
    return onVendorEvent<VendorStepEvent>("printvendor:step", (ev) => setSteps((s) => applyStep(s, ev, printer, table)));
  }, [printer, active, table]);
  return [steps, () => setSteps([])];
}

/**
 * Un fallo al cambiar algo del equipo, dicho claro: sin administrador (con el botón
 * para reabrir así), con el modo auditoría activo (el mensaje es el del programa) o
 * el error tal cual.
 */
export function ActionError({ message, needs = "Esto cambia el equipo y necesita administrador.", className = "" }: { message: string; needs?: ReactNode; className?: string }) {
  if (isAdminError(message)) return <NeedsAdmin className={className}>{needs}</NeedsAdmin>;
  const audit = isAuditError(message);
  return (
    <div role="alert" className={`flex items-start gap-2 rounded-lg border px-3 py-2 text-xs ${audit ? "border-warn/30 bg-warn/5 text-warn" : "border-bad/40 bg-bad/10 text-bad"} ${className}`}>
      <CircleAlert size={14} className="mt-px shrink-0" />
      <p className="min-w-0 flex-1 leading-relaxed select-text">{message}</p>
    </div>
  );
}
