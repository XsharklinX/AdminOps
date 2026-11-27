// La ficha del driver de una impresora (o de una conectada sin instalar): qué
// tiene ahora, qué hay para ponerle (biblioteca y Windows Update), actualizarlo
// con los pasos en vivo y deshacerlo si la página de prueba no salió bien.
import { CheckCircle2, Download, Loader2, Search, Undo2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Drawer } from "../Drawer";
import { useKeepAlive } from "../../lib/alive";
import { useToast } from "../feedback";
import { Button, ErrorState, softBtn, Notice } from "../ui";
import {
  appApi,
  vendorApi,
  type DriverCandidate,
  type DriverCandidates,
  type DriverUpdateResult,
  type PrinterInstallResult,
  type VendorDetected,
  type VendorPrinter,
} from "../../lib/api";
import { ActionError, BrandBadge, DriverChip, StepList, useVendorSteps } from "./Parts";
import { bestCandidate, connectionText, dayText, describeDriver, INSTALL_STEPS, UPDATE_STEPS, VENDOR_NAME } from "./logic";

export type DriverTarget = { kind: "printer"; printer: VendorPrinter } | { kind: "detected"; device: VendorDetected };

const MATCH_TEXT: Record<string, string> = {
  hardwareId: "Coincide con el identificador del aparato",
  modelo: "Coincide con el modelo",
  mismoDriver: "Es el mismo driver que ya tiene",
  universal: "Driver universal de la marca",
};

function CandidateRow({ c, chosen, disabled, onPick }: { c: DriverCandidate; chosen: boolean; disabled: boolean; onPick: () => void }) {
  const newer = c.newer === true ? { t: "Más nuevo que el actual", cls: "text-ok" } : c.newer === false ? { t: "Más viejo que el actual", cls: "text-warn" } : { t: c.comparable ? "Misma versión" : "No se puede comparar", cls: "text-mute" };
  return (
    <label className={`flex cursor-pointer items-start gap-2.5 rounded-lg border px-3 py-2 text-xs transition-colors ${chosen ? "border-neon/50 bg-neon/5" : "border-line hover:border-line-2"} ${disabled ? "pointer-events-none opacity-60" : ""}`}>
      <input type="radio" name="candidate" checked={chosen} onChange={onPick} disabled={disabled} className="mt-0.5 accent-(--color-neon)" />
      <span className="min-w-0 flex-1">
        <span className="block font-medium text-ink">{c.model || c.title}</span>
        <span className="block text-mute">
          {[c.version && `versión ${c.version}`, c.date, c.source === "windowsUpdate" ? "Windows Update" : "tu biblioteca"].filter(Boolean).join(" · ")}
        </span>
        <span className="block text-mute">
          {MATCH_TEXT[c.matchedBy] ?? c.matchedBy} · <span className={newer.cls}>{newer.t}</span>
        </span>
      </span>
    </label>
  );
}

export function DriverDrawer({
  target,
  isAdmin,
  onClose,
  onChanged,
  onGoDrivers,
}: {
  target: DriverTarget;
  isAdmin: boolean;
  onClose: () => void;
  /** Algo cambió en el equipo: que la lista se vuelva a leer. */
  onChanged: () => void;
  /** Llevar a la pestaña Drivers (para bajar el paquete de la web oficial). */
  onGoDrivers?: () => void;
}) {
  const toast = useToast();
  const isPrinter = target.kind === "printer";
  const device = isPrinter ? target.printer.device : target.device.device;
  const name = isPrinter ? target.printer.name : target.device.name;
  const vendor = isPrinter ? target.printer.vendor : target.device.vendor;
  const table = isPrinter ? UPDATE_STEPS : INSTALL_STEPS;

  const [cands, setCands] = useState<DriverCandidates | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [searching, setSearching] = useState(false);
  const [chosen, setChosen] = useState<string | null>(null);
  const [phase, setPhase] = useState<"idle" | "running" | "done" | "failed">("idle");
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<DriverUpdateResult | null>(null);
  const [installed, setInstalled] = useState<PrinterInstallResult | null>(null);
  const [verdict, setVerdict] = useState<"ok" | "undone" | null>(null);
  const [undoing, setUndoing] = useState(false);
  // Su «Deshacer» solo vive aquí: la pantalla no se descarta hasta que se responda.
  useKeepAlive("printers", phase === "running" || undoing || (!!result && verdict === null));
  const changed = useRef(false);
  const [steps, clearSteps] = useVendorSteps(name, true, table);

  const load = useCallback(
    async (searchUpdate: boolean) => {
      if (searchUpdate) setSearching(true);
      try {
        const c = await vendorApi.candidates(device, searchUpdate);
        setCands(c);
        setFailed(null);
        setChosen((cur) => (cur && c.candidates.some((x) => x.id === cur) ? cur : (bestCandidate(c.candidates)?.id ?? null)));
      } catch (e) {
        if (!searchUpdate) setFailed(String(e));
        else toast("error", String(e));
      } finally {
        setSearching(false);
      }
    },
    [device, toast],
  );

  useEffect(() => {
    void load(false);
  }, [load]);

  const close = () => {
    onClose();
  };

  const go = async () => {
    const cand = cands?.candidates.find((c) => c.id === chosen);
    if (!cand) return;
    clearSteps();
    setError(null);
    setPhase("running");
    try {
      if (target.kind === "printer") {
        const r = await vendorApi.update(target.printer.name, cand.id);
        setResult(r);
      } else {
        const r = await vendorApi.install(target.device.device, cand.id);
        setInstalled(r);
      }
      changed.current = true;
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("failed");
    } finally {
      onChanged();
    }
  };

  const undo = async () => {
    if (!result) return;
    setUndoing(true);
    try {
      const msg = await vendorApi.undo(result.undoId);
      toast("ok", msg);
      setVerdict("undone");
      changed.current = true;
      onChanged();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setUndoing(false);
    }
  };

  const busy = phase === "running";
  const list = cands?.candidates ?? [];
  const chosenCand = list.find((c) => c.id === chosen) ?? null;
  const goLabel = isPrinter ? "Actualizar driver" : "Instalar la impresora";

  return (
    <Drawer
      width={480}
      onClose={close}
      closeDisabled={busy}
      closeLabel="Espera a que termine"
      title={
        <span className="flex items-center gap-2">
          <BrandBadge vendor={vendor} />
          <span className="truncate">{name}</span>
        </span>
      }
      subtitle={isPrinter ? [target.printer.model, connectionText(target.printer.connection)].filter(Boolean).join(" · ") : `Conectada por ${target.device.usbId ? "USB" : "cable"} · sin instalar`}
    >
      <div className="space-y-5">
        {/* Ahora */}
        <section>
          <h4 className="mb-1.5 text-[11px] font-semibold text-dim">Ahora</h4>
          {isPrinter ? (
            <div className="rounded-lg border border-line bg-panel-2/40 px-3 py-2.5">
              <div className="flex flex-wrap items-center gap-2">
                <span className="min-w-0 flex-1 text-sm font-medium text-ink select-text">{describeDriver(target.printer.driver.name, "")}</span>
                <DriverChip {...target.printer.state} />
              </div>
              <p className="mt-0.5 text-xs text-mute">
                {[target.printer.driver.version && `versión ${target.printer.driver.version}`, target.printer.driver.date, target.printer.driver.manufacturer].filter(Boolean).join(" · ") || "Sin datos del driver"}
              </p>
              <p className="mt-1.5 text-xs leading-relaxed text-dim">{target.printer.state.reason}</p>
            </div>
          ) : (
            <div className="rounded-lg border border-warn/30 bg-warn/5 px-3 py-2.5 text-xs text-warn">
              Conectada y sin driver: Windows no sabe imprimir con ella. {target.device.model && <span className="text-dim">Modelo: {target.device.model}.</span>}
            </div>
          )}
        </section>

        {/* Candidatos */}
        <section>
          <div className="mb-1.5 flex items-center gap-2">
            <h4 className="text-[11px] font-semibold text-dim">Drivers disponibles</h4>
            <button
              type="button"
              onClick={() => void load(true)}
              disabled={searching || busy}
              className="ml-auto inline-flex items-center gap-1 text-xs text-neon hover:underline disabled:pointer-events-none disabled:opacity-40"
            >
              {searching ? <Loader2 size={12} className="animate-spin" /> : <Search size={12} />} Buscar en Windows Update
            </button>
          </div>
          {searching && (
            <div className="mb-2 flex items-center gap-2 rounded-lg border border-line bg-panel-2/40 px-3 py-2 text-xs text-dim">
              <Loader2 size={13} className="shrink-0 animate-spin text-neon" />
              <p className="min-w-0 flex-1">Buscando en Windows Update. Puede tardar unos minutos; puedes seguir trabajando.</p>
              <button type="button" onClick={() => void appApi.cancelTask("printvendor-wu").catch((e) => toast("error", String(e)))} className="shrink-0 text-neon hover:underline">
                Cancelar
              </button>
            </div>
          )}
          {failed ? (
            <ErrorState message={failed} onRetry={() => void load(false)} where="Drivers de la impresora" />
          ) : !cands ? (
            <p className="flex items-center gap-2 text-xs text-mute">
              <Loader2 size={13} className="animate-spin" /> Mirando la biblioteca…
            </p>
          ) : list.length === 0 ? (
            <div className="rounded-lg border border-line px-3 py-2.5 text-xs text-dim">
              <p>{device.model || name}: no está en tu biblioteca ni en lo último que ofreció Windows Update.</p>
              {VENDOR_NAME[vendor] && (
                <p className="mt-1 text-mute">
                  Prueba «Buscar en Windows Update» o baja el driver de la página oficial de {VENDOR_NAME[vendor]} desde la pestaña Drivers.
                  {onGoDrivers && (
                    <button type="button" onClick={() => { close(); onGoDrivers(); }} className="ml-1 text-neon hover:underline">
                      Ir a Drivers
                    </button>
                  )}
                </p>
              )}
            </div>
          ) : (
            <div className="space-y-1.5">
              {list.map((c) => (
                <CandidateRow key={c.id} c={c} chosen={c.id === chosen} disabled={busy || phase === "done"} onPick={() => setChosen(c.id)} />
              ))}
            </div>
          )}
          {cands?.windowsUpdate.error && <p className="mt-1.5 text-[11px] text-warn">Windows Update no respondió: {cands.windowsUpdate.error} Se usa la biblioteca.</p>}
          {cands && !cands.windowsUpdate.error && (
            <p className="mt-1.5 text-[11px] text-mute">
              {cands.windowsUpdate.searchedAt ? `Última búsqueda en Windows Update: ${dayText(cands.windowsUpdate.searchedAt)}.` : "Aún no se ha buscado en Windows Update."}
            </p>
          )}
        </section>

        {/* Acción */}
        {phase !== "done" && (
          <section className="space-y-2.5">
            {!isAdmin && <ActionError message="Requiere ejecutar AdminOps como administrador." needs={isPrinter ? "Cambiar el driver necesita administrador." : "Instalar una impresora necesita administrador."} />}
            {error && <ActionError message={error} needs="Cambiar el driver necesita administrador." />}
            <div className="flex flex-wrap items-center gap-2">
              <Button onClick={() => void go()} disabled={!chosenCand || !isAdmin || busy || searching} title={isAdmin ? undefined : "Requiere administrador"}>
                {busy ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />} {busy ? "Trabajando…" : goLabel}
              </Button>
              {chosenCand && !busy && (
                <span className="text-[11px] text-mute">
                  {isPrinter ? "Se guarda una copia del driver actual y se puede deshacer." : "Se instala el driver y se comprueba que Windows la cree."}
                </span>
              )}
            </div>
          </section>
        )}

        {(busy || steps.length > 0) && (
          <section>
            <h4 className="mb-1.5 text-[11px] font-semibold text-dim">{isPrinter ? "Actualizando" : "Instalando"}</h4>
            <StepList steps={steps} table={table} />
          </section>
        )}

        {/* Resultado */}
        {phase === "done" && result && (
          <section className="space-y-3 rounded-lg border border-line bg-panel-2/40 p-3">
            <p className="flex items-start gap-2 text-xs text-ink">
              <CheckCircle2 size={14} className="mt-px shrink-0 text-ok" />
              <span className="min-w-0 flex-1">
                {result.message}
                {result.rebootRequired && <span className="block text-warn">Windows pide reiniciar para terminar.</span>}
              </span>
            </p>
            {verdict === null && (
              <>
                <p className="text-sm font-medium text-ink">¿Salió bien la página de prueba?</p>
                <p className="text-[11px] text-mute">
                  {result.testPageSent ? "Windows la ha enviado a la impresora." : "No se pudo enviar la página de prueba: imprime una tú antes de decidir."} Si no sale bien, «deshacer» deja la impresora como estaba.
                </p>
                <div className="flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      setVerdict("ok");
                      onChanged();
                      changed.current = false;
                    }}
                    className={softBtn}
                  >
                    Sí, listo
                  </button>
                  <Button kind="danger" size="sm" onClick={() => void undo()} disabled={undoing || !isAdmin}>
                    {undoing ? <Loader2 size={13} className="animate-spin" /> : <Undo2 size={13} />} No: deshacer
                  </Button>
                </div>
              </>
            )}
            {verdict === "ok" && (
              <p className="text-xs text-ok">
                {target.kind === "printer" ? target.printer.name : name} con el driver {describeDriver(result.newDriver, result.newVersion)}. Queda anotado en el diario del equipo.
              </p>
            )}
            {verdict === "undone" && <Notice tone="warn">Deshecho: vuelve a {describeDriver(result.previousDriver, result.previousVersion)}.</Notice>}
          </section>
        )}
        {phase === "done" && installed && (
          <section className="space-y-1.5 rounded-lg border border-line bg-panel-2/40 p-3">
            <p className="flex items-start gap-2 text-xs text-ink">
              <CheckCircle2 size={14} className="mt-px shrink-0 text-ok" />
              <span className="min-w-0 flex-1">
                {installed.message}
                {installed.rebootRequired && <span className="block text-warn">Windows pide reiniciar para terminar.</span>}
              </span>
            </p>
            <p className="text-[11px] text-mute">Prueba con «Imprimir página de prueba» en la lista.</p>
          </section>
        )}
      </div>
    </Drawer>
  );
}
