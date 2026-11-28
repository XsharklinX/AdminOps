// «Actualizar las RICOH» / «las EPSON»: la misma secuencia que en una sola
// impresora, una detrás de otra (el servicio de impresión no admite dos cambios
// de driver a la vez). Si una falla, las demás siguen. Al final, un resumen para
// pegar en el ticket.
import { CheckCircle2, ClipboardCopy, Loader2, RotateCcw, Undo2, X, XCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useToast } from "../feedback";
import { useKeepAlive } from "../../lib/alive";
import { Button, Notice, smallBtn } from "../ui";
import { onVendorEvent, vendorApi, type VendorKey, type VendorPrinter, type VendorStepEvent } from "../../lib/api";
import { ActionError, BrandBadge } from "./Parts";
import { bestCandidate, describeDriver, isAdminError, isAuditError, ticketSummary, VENDOR_NAME, type BatchResult } from "./logic";

type RowState = { phase: "wait" | "running" | "ok" | "failed" | "undone"; result?: BatchResult };

export function BatchPanel({
  vendor,
  targets,
  isAdmin,
  host,
  onClose,
  onDone,
}: {
  vendor: VendorKey;
  targets: VendorPrinter[];
  isAdmin: boolean;
  host?: string;
  onClose: () => void;
  /** Se cambió algo: que la lista se vuelva a leer. */
  onDone: () => void;
}) {
  const toast = useToast();
  const [rows, setRows] = useState<Record<string, RowState>>({});
  const [running, setRunning] = useState(false);
  const [finished, setFinished] = useState(false);
  useKeepAlive("printers", running || finished);
  const [stopNote, setStopNote] = useState<string | null>(null);
  const [live, setLive] = useState<{ printer: string; text: string } | null>(null);
  const stop = useRef(false);
  const [frozenTargets] = useState(() => targets);
  const brand = VENDOR_NAME[vendor];

  useEffect(() => {
    return () => {
      stop.current = true;
    };
  }, []);

  useEffect(() => {
    if (!running) return;
    return onVendorEvent<VendorStepEvent>("printvendor:step", (ev) => {
      if (ev.state === "running") setLive({ printer: ev.printer, text: `Paso ${ev.index} de ${ev.total}: ${ev.message}` });
    });
  }, [running]);

  const setRow = (name: string, s: RowState) => setRows((r) => ({ ...r, [name]: s }));

  /** Actualiza una impresora; nunca lanza: el resultado dice qué pasó. */
  const one = async (p: VendorPrinter): Promise<BatchResult> => {
    const from = describeDriver(p.driver.name, p.driver.version);
    try {
      const c = await vendorApi.candidates(p.device, false);
      const best = bestCandidate(c.candidates);
      if (!best) return { printer: p.name, ok: false, from, to: from, message: "No hay un driver mejor en la biblioteca ni en lo último de Windows Update." };
      const r = await vendorApi.update(p.name, best.id);
      return { printer: p.name, ok: true, from, to: describeDriver(r.newDriver, r.newVersion), message: r.message, undoId: r.undoId };
    } catch (e) {
      return { printer: p.name, ok: false, from, to: from, message: String(e) };
    }
  };

  const start = async (only?: VendorPrinter[]) => {
    stop.current = false;
    setStopNote(null);
    setRunning(true);
    setFinished(false);
    const list = only ?? frozenTargets;
    let changed = false;
    for (const p of list) {
      if (stop.current) break;
      setRow(p.name, { phase: "running" });
      const r = await one(p);
      setRow(p.name, { phase: r.ok ? "ok" : "failed", result: r });
      if (r.ok) changed = true;
      // Sin administrador o con el modo auditoría, todas fallarían igual: se para.
      if (!r.ok && (isAdminError(r.message) || isAuditError(r.message))) {
        setStopNote(r.message);
        break;
      }
    }
    setRunning(false);
    setLive(null);
    setFinished(true);
    if (changed) onDone();
  };

  const undo = async (name: string) => {
    const r = rows[name]?.result;
    if (!r?.undoId) return;
    try {
      await vendorApi.undo(r.undoId);
      setRow(name, { phase: "undone", result: { ...r, ok: false, message: "Deshecho: vuelve al driver anterior.", undoId: undefined } });
      onDone();
    } catch (e) {
      toast("error", String(e));
    }
  };

  // Lo que se enseña en el resumen: lo que pasó con cada una, en el orden de la lista.
  const results = frozenTargets.map((p) => rows[p.name]?.result).filter((r): r is BatchResult => !!r);
  const summary = ticketSummary(vendor, results, host);
  const pending = frozenTargets.filter((p) => !rows[p.name]);
  const failed = frozenTargets.filter((p) => rows[p.name]?.phase === "failed");

  return (
    <section className="mb-4 rounded-2xl border border-line/70 bg-panel shadow-elev-1">
      <div className="flex items-center gap-2 border-b border-line/70 px-4 py-3">
        <BrandBadge vendor={vendor} />
        <h3 className="text-sm font-medium text-ink">
          Actualizar {frozenTargets.length === 1 ? "la" : `las ${frozenTargets.length}`} {brand}
        </h3>
        <button type="button" onClick={onClose} disabled={running} className="ml-auto text-mute hover:text-ink disabled:opacity-30" title="Cerrar" aria-label="Cerrar">
          <X size={14} />
        </button>
      </div>
      <ul className="divide-y divide-line/60">
        {frozenTargets.map((p) => {
          const st = rows[p.name];
          const r = st?.result;
          return (
            <li key={p.name} className="flex items-start gap-3 px-4 py-2.5 text-xs">
              <span className="mt-0.5 shrink-0">
                {st?.phase === "running" ? (
                  <Loader2 size={14} className="animate-spin text-neon" />
                ) : st?.phase === "ok" ? (
                  <CheckCircle2 size={14} className="text-ok" />
                ) : st?.phase === "failed" ? (
                  <XCircle size={14} className="text-bad" />
                ) : st?.phase === "undone" ? (
                  <Undo2 size={14} className="text-warn" />
                ) : (
                  <span className="block h-3.5 w-3.5 rounded-full border border-line-2" />
                )}
              </span>
              <div className="min-w-0 flex-1">
                <div className="font-medium text-ink">{p.name}</div>
                <div className="text-mute">
                  {st?.phase === "ok" && r ? `${r.from} → ${r.to}` : `${describeDriver(p.driver.name, p.driver.version)} · ${p.state.label}`}
                </div>
                {st?.phase === "running" && live?.printer === p.name && <div className="text-dim">{live.text}</div>}
                {(st?.phase === "failed" || st?.phase === "undone") && r && <Notice tone={st.phase === "failed" ? "error" : "warn"} where="Impresoras" className="mt-1">{r.message}</Notice>}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {st?.phase === "ok" && r?.undoId && (
                  <button type="button" onClick={() => void undo(p.name)} className="text-neon hover:underline">
                    Deshacer
                  </button>
                )}
                {st?.phase === "failed" && !running && (
                  <button type="button" onClick={() => void start([p])} className="inline-flex items-center gap-1 text-neon hover:underline">
                    <RotateCcw size={11} /> Reintentar
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>
      <div className="space-y-2.5 border-t border-line/70 px-4 py-3">
        {!isAdmin && <ActionError message="Requiere ejecutar AdminOps como administrador." needs="Cambiar drivers necesita administrador." />}
        {stopNote && <ActionError message={stopNote} needs="Cambiar drivers necesita administrador." />}
        {!finished && !running && <p className="text-[11px] text-mute">Una detrás de otra, con su copia del driver anterior. Si una falla, se sigue con las demás. Usa la biblioteca y lo último que ofreció Windows Update.</p>}
        <div className="flex flex-wrap items-center gap-2">
          {pending.length > 0 && (
            <Button onClick={() => void start(pending)} disabled={!isAdmin || running} title={isAdmin ? undefined : "Requiere administrador"}>
              {running ? <Loader2 size={14} className="animate-spin" /> : null} {running ? "Actualizando…" : `Actualizar ${pending.length === 1 ? "la impresora" : `las ${pending.length} impresoras`}`}
            </Button>
          )}
          {running && (
            <button type="button" onClick={() => {
                stop.current = true;
              }} className={smallBtn}>
              Parar después de esta
            </button>
          )}
          {failed.length > 1 && !running && (
            <button type="button" onClick={() => void start(failed)} className={smallBtn}>
              <RotateCcw size={12} /> Reintentar las {failed.length} que fallaron
            </button>
          )}
        </div>
        {finished && results.length > 0 && (
          <div className="rounded-lg border border-line bg-panel-2/40 p-3">
            <pre className="font-sans text-xs leading-relaxed whitespace-pre-wrap text-dim select-text">{summary}</pre>
            <div className="mt-2">
              <button
                type="button"
                onClick={() => void navigator.clipboard.writeText(summary).then(() => toast("ok", "Resumen copiado: pégalo en el ticket."))}
                className={smallBtn}
              >
                <ClipboardCopy size={12} /> Copiar para el ticket
              </button>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
