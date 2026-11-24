// La cola de instalación: una fila por programa con su estado, su progreso y su motivo.
// El estado viene del programa (eventos `install-item`); aquí solo se dibuja y se decide.
import { CheckCircle2, CircleSlash, Loader2, RotateCcw, X, XCircle } from "lucide-react";
import { useState } from "react";
import { installQueueApi, type InstallItem } from "../lib/api";
import { bytes } from "../lib/format";
import { canCloseAndGo, initial, isFinished, needsDecision, statusText, summaryText } from "../lib/installQueue";
import { useConfirm, useToast } from "./feedback";
import { Bar, Button } from "./ui";

const STATE_COLOR: Record<InstallItem["state"], string> = {
  queued: "text-mute",
  starting: "text-dim",
  downloading: "text-neon",
  installing: "text-neon",
  done: "text-ok",
  skipped: "text-mute",
  failed: "text-bad",
};

function Row({ item }: { item: InstallItem }) {
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const act = async (op: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await op();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  // Cerrar el programa y seguir: siempre con confirmación si hay algo que cerrar, enseñando qué.
  const closeAndGo = () =>
    act(async () => {
      const b = await installQueueApi.blockers(item.id);
      if (b.windows > 0 || b.processes.length > 0) {
        const windows = b.windows === 0 ? "" : b.windows === 1 ? " Tiene una ventana abierta." : ` Tiene ${b.windows} ventanas abiertas.`;
        const ok = await confirm({
          title: `Cerrar ${item.name}`,
          body: `Se cerrará: ${b.processes.length > 0 ? b.processes.join(", ") : item.name}.${windows} Si lo cierras ahora, se puede perder lo que no esté guardado.`,
          confirmLabel: "Cerrar y seguir",
          danger: true,
        });
        if (!ok) return;
      }
      await installQueueApi.closeAndRetry(item.id);
    });

  const active = item.state === "starting" || item.state === "downloading" || item.state === "installing";
  const Icon = item.state === "done" ? CheckCircle2 : item.state === "failed" ? XCircle : item.state === "skipped" ? CircleSlash : null;
  const why = item.state === "failed" || item.state === "skipped" || (item.state === "done" && item.reboot) ? item.message : "";

  return (
    <li className="py-2.5">
      <div className="flex items-center gap-3">
        <span aria-hidden className="flex size-8 shrink-0 items-center justify-center rounded-md border border-line bg-panel-2 text-sm font-semibold text-ink">
          {initial(item.name)}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <span className="truncate text-sm text-ink" title={item.id}>
              {item.name}
            </span>
            {item.size ? <span className="shrink-0 text-[11px] text-mute">{bytes(item.size)}</span> : null}
          </div>
          {item.state === "downloading" && item.percent !== null && (
            <div className="mt-1.5">
              <Bar value={item.percent} color="var(--color-neon)" />
            </div>
          )}
        </div>
        <span className={`flex shrink-0 items-center gap-1.5 text-xs ${STATE_COLOR[item.state]}`}>
          {active && item.state !== "downloading" && <Loader2 size={13} className="animate-spin" />}
          {Icon && <Icon size={13} />}
          {statusText(item)}
        </span>
      </div>
      {(why || needsDecision(item)) && (
        <div className="mt-1.5 ml-11 flex flex-wrap items-center gap-x-3 gap-y-1.5">
          {why && (
            <p className={`min-w-0 flex-1 text-xs ${item.state === "failed" ? "text-bad" : "text-dim"}`}>
              {why}
              {item.code && <span className="ml-1.5 font-mono text-mute">{item.code}</span>}
            </p>
          )}
          {needsDecision(item) && (
            <span className="flex shrink-0 gap-1.5">
              {canCloseAndGo(item) && (
                <Button size="sm" onClick={closeAndGo} disabled={busy}>
                  Cerrar y seguir
                </Button>
              )}
              <Button size="sm" kind="secondary" onClick={() => act(() => installQueueApi.retry(item.id))} disabled={busy}>
                <RotateCcw size={12} /> Reintentar
              </Button>
              <Button size="sm" kind="ghost" onClick={() => act(() => installQueueApi.skip(item.id))} disabled={busy}>
                Saltar
              </Button>
            </span>
          )}
        </div>
      )}
      {dialog}
    </li>
  );
}

/** Todas las filas de la tanda y, cuando termina, el resumen de arriba. */
export function InstallQueue({ items, onClose }: { items: InstallItem[]; onClose: () => void }) {
  if (items.length === 0) return null;
  const summary = summaryText(items);
  return (
    <div className="mb-4 rounded-2xl border border-line/70 bg-panel p-4 shadow-elev-1">
      <div className="mb-1 flex items-center justify-between gap-3">
        <h3 className="text-sm font-medium">{summary ?? `Instalando ${items.length} ${items.length === 1 ? "programa" : "programas"}`}</h3>
        {items.every(isFinished) && (
          <button onClick={onClose} className="text-mute hover:text-ink" title="Quitar la lista" aria-label="Quitar la lista">
            <X size={14} />
          </button>
        )}
      </div>
      <ul className="divide-y divide-line/60">
        {items.map((i) => (
          <Row key={i.id} item={i} />
        ))}
      </ul>
    </div>
  );
}
