// Aparatos RICOH o EPSON conectados a los que les falta el driver: aviso arriba
// de la lista y «Instalarla» (el mejor driver de la biblioteca o de Windows Update).
import { CheckCircle2, CircleAlert, Loader2, Printer } from "lucide-react";
import { useState } from "react";
import { useToast } from "../feedback";
import { Button } from "../ui";
import { vendorApi, type VendorDetected } from "../../lib/api";
import { ActionError, BrandBadge, StepList, useVendorSteps } from "./Parts";
import { bestCandidate, INSTALL_STEPS } from "./logic";

function Row({ d, isAdmin, onChanged, onOptions }: { d: VendorDetected; isAdmin: boolean; onChanged: () => void; onOptions: (d: VendorDetected) => void }) {
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const [steps, clearSteps] = useVendorSteps(d.name, true, INSTALL_STEPS);
  const how = d.usbId ? "por USB" : "conectada";

  const install = async () => {
    setError(null);
    setNote(null);
    clearSteps();
    setBusy(true);
    try {
      const c = await vendorApi.candidates(d.device, false);
      const best = bestCandidate(c.candidates);
      if (!best) {
        setNote(`No hay un driver para ${d.model || d.name} en tu biblioteca ni en lo último de Windows Update.`);
        return;
      }
      const r = await vendorApi.install(d.device, best.id);
      setDone(r.message);
      toast("ok", r.message);
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <li className="px-4 py-3">
      <div className="flex flex-wrap items-center gap-3">
        <CircleAlert size={16} className="shrink-0 text-warn" />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5 text-sm text-ink">
            Detectada una impresora sin instalar:
            <BrandBadge vendor={d.vendor} />
            <span className="font-medium">{d.model || d.name}</span>
            <span className="text-mute">{how}</span>
          </div>
          <div className="text-[11px] text-mute">Windows la ve pero no tiene driver, así que no se puede imprimir con ella.</div>
        </div>
        {done ? (
          <span className="flex items-center gap-1 text-xs text-ok">
            <CheckCircle2 size={13} /> Instalada
          </span>
        ) : (
          <>
            <button type="button" onClick={() => onOptions(d)} disabled={busy} className="text-xs text-neon hover:underline disabled:opacity-40">
              Ver opciones
            </button>
            <Button onClick={() => void install()} disabled={!isAdmin || busy} title={isAdmin ? undefined : "Requiere administrador"} size="sm">
              {busy ? <Loader2 size={13} className="animate-spin" /> : <Printer size={13} />} {busy ? "Instalando…" : "Instalarla"}
            </Button>
          </>
        )}
      </div>
      {(busy || steps.length > 0) && !done && (
        <div className="mt-3 ml-7">
          <StepList steps={steps} table={INSTALL_STEPS} />
        </div>
      )}
      {note && <p className="mt-2 ml-7 text-xs text-dim">{note}</p>}
      {error && <ActionError className="mt-2 ml-7" message={error} needs="Instalar una impresora necesita administrador." />}
    </li>
  );
}

export function NotInstalled({ devices, isAdmin, onChanged, onOptions }: { devices: VendorDetected[]; isAdmin: boolean; onChanged: () => void; onOptions: (d: VendorDetected) => void }) {
  if (devices.length === 0) return null;
  return (
    <ul className="mb-4 divide-y divide-warn/20 overflow-hidden rounded-2xl border border-warn/40 bg-warn/5 shadow-elev-1">
      {devices.map((d) => (
        <Row key={d.instanceId || d.name} d={d} isAdmin={isAdmin} onChanged={onChanged} onOptions={onOptions} />
      ))}
    </ul>
  );
}
