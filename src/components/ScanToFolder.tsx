// Escanear a carpeta: el asistente de Impresoras → Escáneres. Deja este equipo listo para que una
// RICOH o una EPSON mande los escaneos a una carpeta compartida, enseña lo que se escribe en el
// panel de la impresora, espera un escaneo de prueba y, si no llega, dice por qué.
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, CircleAlert, FolderOpen, Loader2, ScanLine, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { logQuietly, scanFolderApi, tweaksApi, vendorApi, type ScanDiagnosis, type ScanSetup, type ScanStatus, type ScanStep, type ScanWatch } from "../lib/api";
import { arrivedText, collapseSteps, dictationText, headerText, ipPathText, isIpv4, OLD_PRINTER_HINT, offersSigning, panelFields, pickPrinters, setupWorked, waitedText, type PrinterChoice } from "../lib/scanfolder";
import { usePageActive } from "../lib/pageActive";
import { AUDIT_TITLE, useAuditOn } from "../lib/useAudit";
import { ViewCommand } from "./ActionCommand";
import { NeedsAdmin } from "./AdminBanner";
import { useConfirm, useToast } from "./feedback";
import { BrandBadge } from "./printers/Parts";
import { Button, Card, inputClass, Notice } from "./ui";

const MANUAL = "manual";

function StepIcon({ state }: { state: ScanStep["state"] }) {
  if (state === "running") return <Loader2 size={14} className="shrink-0 animate-spin text-neon" />;
  if (state === "ok") return <CheckCircle2 size={14} className="shrink-0 text-ok" />;
  if (state === "warn") return <CircleAlert size={14} className="shrink-0 text-warn" />;
  return <XCircle size={14} className="shrink-0 text-bad" />;
}

function Steps({ steps }: { steps: ScanStep[] }) {
  return (
    <ul className="space-y-1.5" aria-live="polite">
      {steps.map((s) => (
        <li key={s.id} className="flex items-start gap-2 text-sm">
          <span className="mt-0.5">
            <StepIcon state={s.state} />
          </span>
          <span className="min-w-0">
            <span className={s.state === "error" ? "text-bad" : "text-ink"}>{s.label}</span>
            {s.detail && <span className="block text-xs text-dim">{s.detail}</span>}
          </span>
        </li>
      ))}
    </ul>
  );
}

/** Un campo del panel de la impresora, en letra grande y con su «Copiar». */
function Field({ label, value, mono = true }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="min-w-0">
      <div className="mb-1 flex items-center gap-2">
        <span className="text-[11px] font-semibold text-dim">{label}</span>
        <button type="button" data-copy={value} className="text-[11px] text-neon hover:underline" aria-label={`Copiar ${label.toLowerCase()}`}>
          Copiar
        </button>
      </div>
      <div className={`break-all text-2xl text-ink select-text ${mono ? "font-mono" : ""}`}>{value}</div>
    </div>
  );
}

export function ScanToFolder({ isAdmin }: { isAdmin: boolean }) {
  const [open, setOpen] = useState(false);
  const [choices, setChoices] = useState<PrinterChoice[] | null>(null);
  const [pick, setPick] = useState<string>("");
  const [manual, setManual] = useState("");
  const [status, setStatus] = useState<ScanStatus | null>(null);
  const [live, setLive] = useState<ScanStep[]>([]);
  const [setup, setSetup] = useState<ScanSetup | null>(null);
  const [running, setRunning] = useState(false);
  const [watch, setWatch] = useState<ScanWatch | null>(null);
  const [diagnosis, setDiagnosis] = useState<ScanDiagnosis | null>(null);
  const [signingOn, setSigningOn] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const active = usePageActive();
  const audit = useAuditOn();
  const watching = useRef(false);

  const ip = pick === MANUAL ? manual.trim() : pick;
  const ipOk = isIpv4(ip);
  const choice = choices?.find((c) => c.ip === ip) ?? null;
  const lock = audit ? AUDIT_TITLE : undefined;

  // Las impresoras que AdminOps conoce, la primera vez que se abre el asistente.
  useEffect(() => {
    if (!open || choices) return;
    let alive = true;
    vendorApi
      .printers()
      .then((inv) => {
        if (!alive) return;
        const list = pickPrinters(inv.printers);
        setChoices(list);
        setPick((p) => p || (list[0]?.ip ?? MANUAL));
      })
      .catch((e) => {
        logQuietly("ScanToFolder")(e);
        if (alive) {
          setChoices([]);
          setPick(MANUAL);
        }
      });
    return () => {
      alive = false;
    };
  }, [open, choices]);

  // Cómo está este equipo para esa impresora (lectura).
  // Cada lectura lleva un número: si mientras tanto se cambió de impresora o se volvió a leer, la respuesta vieja se descarta.
  const statusSeq = useRef(0);
  const readStatus = useCallback(async () => {
    const mine = ++statusSeq.current;
    if (!ipOk) return setStatus(null);
    try {
      const s = await scanFolderApi.status(ip);
      if (mine === statusSeq.current) setStatus(s);
    } catch (e) {
      logQuietly("ScanToFolder")(e);
    }
  }, [ip, ipOk]);
  useEffect(() => {
    if (!open || !active) return;
    const t = window.setTimeout(() => void readStatus(), pick === MANUAL ? 500 : 0);
    return () => window.clearTimeout(t);
  }, [open, active, readStatus, pick]);

  // Cada paso del asistente llega según se hace.
  useEffect(() => {
    if (!open) return;
    const off = listen<ScanStep>("scanfolder-step", (e) => setLive((l) => [...l.filter((s) => s.id !== e.payload.id), e.payload]));
    return () => {
      void off.then((f) => f());
    };
  }, [open]);

  const stopWatch = useCallback(() => {
    if (!watching.current) return;
    watching.current = false;
    scanFolderApi.watchStop().catch(logQuietly("ScanToFolder"));
  }, []);

  // La espera se para al salir de la pantalla y al cerrar el asistente.
  useEffect(() => {
    if (!active || !open) stopWatch();
  }, [active, open, stopWatch]);
  useEffect(() => stopWatch, [stopWatch]);

  // Mientras se espera: se mira la carpeta cada segundo y corre el «hace n s».
  const waiting = watch?.state === "waiting";
  const arrived = watch?.state === "arrived";
  useEffect(() => {
    if (!active || !open || (!waiting && !arrived)) return;
    const t = window.setInterval(() => {
      setNow(Date.now());
      if (!waiting) return;
      scanFolderApi
        .watchStatus()
        .then((w) => {
          setWatch(w);
          if (w.diagnosis) setDiagnosis(w.diagnosis);
          if (w.state === "arrived") watching.current = false;
          if (w.state === "timeout") {
            watching.current = false;
            void readStatus();
          }
        })
        .catch(logQuietly("ScanToFolder"));
    }, 1000);
    return () => window.clearInterval(t);
  }, [active, open, waiting, arrived, readStatus]);

  const startWatch = useCallback(async () => {
    if (!ipOk) return;
    setDiagnosis(null);
    try {
      watching.current = true;
      setWatch(await scanFolderApi.watchStart(ip));
      setNow(Date.now());
    } catch (e) {
      watching.current = false;
      toast("error", String(e));
    }
  }, [ip, ipOk, toast]);

  const prepare = async () => {
    if (running || !ipOk) return;
    setRunning(true);
    setLive([]);
    setSetup(null);
    setWatch(null);
    setDiagnosis(null);
    try {
      const r = await scanFolderApi.setup(ip);
      setSetup(r);
      setLive([]);
      void readStatus();
      // Con la red pública el cortafuegos sigue cerrado: no tiene sentido esperar todavía.
      if (setupWorked(r.steps) && !r.publicNetwork && !audit) await startWatch();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(false);
    }
  };

  const run = async (key: string, what: () => Promise<void>) => {
    if (busy) return;
    setBusy(key);
    try {
      await what();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const newPassword = () =>
    run("password", async () => {
      const ok = await confirm({ title: "Contraseña nueva", body: "El usuario «escaner» tendrá otra contraseña: hay que escribirla de nuevo en la impresora. La contraseña de ahora deja de valer.", confirmLabel: "Cambiarla" });
      if (!ok) return;
      const r = await scanFolderApi.newPassword();
      setSetup((s) => (s ? { ...s, password: r.password, speech: r.speech } : { path: `\\\\${status?.computer ?? ""}\\Escaneos`, ipPath: status?.localIp ? `\\\\${status.localIp}\\Escaneos` : null, user: r.user, password: r.password, speech: r.speech, entryId: 0, steps: [], publicNetwork: false, folderHadFiles: false }));
      toast("ok", "Contraseña cambiada: escríbela en la impresora.");
    });

  const undoAll = () =>
    run("undo", async () => {
      const entry = setup?.entryId;
      if (!entry) {
        toast("info", "Se deshace desde Estado del equipo → Historial del equipo.");
        return;
      }
      const ok = await confirm({
        title: "Deshacer «Escanear a carpeta»",
        body: "Se quita el recurso compartido, el usuario «escaner» y lo que se abrió del cortafuegos. La carpeta Escaneos no se borra si tiene archivos.",
        confirmLabel: "Deshacer",
        danger: true,
      });
      if (!ok) return;
      stopWatch();
      const r = await tweaksApi.revertEntry(entry);
      toast("ok", r.message);
      setSetup(null);
      setWatch(null);
      setDiagnosis(null);
      await readStatus();
    });

  const makePrivate = () =>
    run("private", async () => {
      toast("ok", await scanFolderApi.privateNetwork(ip));
      await readStatus();
    });

  const applySigning = () =>
    run("signing", async () => {
      const ok = await confirm({
        title: "Quitar la firma SMB obligatoria",
        body: "Este equipo dejará de exigir la firma en las conexiones de archivos. Es lo que necesitan las impresoras viejas, pero baja la seguridad de este equipo en la red. Se puede deshacer desde el historial.",
        confirmLabel: "Quitar la firma",
        danger: true,
      });
      if (!ok) return;
      toast("ok", await scanFolderApi.disableSigning());
      setSigningOn(false);
      await readStatus();
      await startWatch();
    });

  const explain = () =>
    run("diagnose", async () => {
      setDiagnosis(await scanFolderApi.diagnose(ip, watch?.startedMs || undefined));
    });

  const shortcut = () =>
    run("shortcut", async () => {
      toast("ok", await scanFolderApi.desktopShortcut());
    });

  // Los pasos que se enseñan: los de ahora mientras corre, y al acabar los del resultado.
  const steps = useMemo(() => (setup ? collapseSteps(setup.steps) : collapseSteps(live)), [setup, live]);
  const configured = !!status && status.userExists && status.shareOk;
  const panel = setup ?? (configured && status ? { path: `\\\\${status.computer}\\Escaneos`, ipPath: status.localIp ? `\\\\${status.localIp}\\Escaneos` : null, user: "escaner", password: "", speech: "", entryId: 0 } : null);
  const publicNet = status?.networkCategory === "Public";
  // Solo si la directiva pide más de lo que puede tener la contraseña que genera AdminOps.
  const tooShort = status?.passwordMin != null && status.passwordMax > 0 && status.passwordMin > status.passwordMax;

  return (
    <Card
      title="Escanear a carpeta"
      icon={<ScanLine size={14} />}
      fold={{ collapsed: !open, note: "Para que la RICOH o la EPSON mande los escaneos a una carpeta de este equipo", onToggle: () => setOpen((v) => !v) }}
    >
      {dialog}
      <div className="space-y-4">
        {/* La impresora */}
        <div className="flex flex-wrap items-end gap-3">
          <label className="min-w-64 flex-1 text-xs text-dim">
            Impresora
            <select value={pick} onChange={(e) => setPick(e.target.value)} disabled={!choices || running} className={`${inputClass} mt-1`}>
              {!choices && <option value="">Buscando impresoras…</option>}
              {choices?.map((c) => (
                <option key={c.ip} value={c.ip}>
                  {c.label}
                </option>
              ))}
              {choices && <option value={MANUAL}>Otra: escribir la IP…</option>}
            </select>
          </label>
          {pick === MANUAL && (
            <label className="w-48 text-xs text-dim">
              IP de la impresora
              <input value={manual} onChange={(e) => setManual(e.target.value)} placeholder="10.0.0.41" inputMode="decimal" className={`${inputClass} mt-1 font-mono`} aria-invalid={manual !== "" && !ipOk} />
            </label>
          )}
        </div>

        {ipOk ? (
          <p className="text-sm font-medium text-ink">{headerText(choice?.model ?? null, ip)}</p>
        ) : (
          pick === MANUAL && manual !== "" && <Notice tone="warn">Escribe una IP completa, por ejemplo 10.0.0.41.</Notice>
        )}

        {!isAdmin && (
          <NeedsAdmin>«Escanear a carpeta» crea un usuario, un recurso compartido y reglas del cortafuegos en este equipo: necesita administrador. Sin él solo se puede mirar.</NeedsAdmin>
        )}
        {isAdmin && publicNet && (
          <div className="flex flex-wrap items-center gap-3 rounded-lg border border-warn/40 bg-warn/5 px-3 py-2 text-xs text-warn" role="note">
            <CircleAlert size={14} className="shrink-0" />
            <p className="min-w-48 flex-1">
              La red de este equipo{status?.networkName ? ` («${status.networkName}»)` : ""} es pública y Windows no deja compartir archivos por ella. Hay que ponerla como privada: es un paso aparte y se puede deshacer.
            </p>
            <span className="flex items-center gap-1">
              <Button kt="V" kind="secondary" size="sm" disabled={audit || busy !== null} title={lock} onClick={makePrivate}>
                Poner la red como privada
              </Button>
              <ViewCommand id="scan.private" icon />
            </span>
          </div>
        )}
        {tooShort && (
          <p className="flex items-center gap-2 rounded-lg border border-warn/40 bg-warn/5 px-3 py-2 text-xs text-warn" role="note">
            <CircleAlert size={14} className="shrink-0" /> La directiva de contraseñas de este equipo pide al menos {status?.passwordMin} caracteres y la contraseña que genera AdminOps no pasa de {status?.passwordMax}.
          </p>
        )}
        {configured && !setup && (
          <p className="text-xs text-dim">Ya está preparado en este equipo: recurso «Escaneos» y usuario «escaner». Si no tienes la contraseña, «Contraseña nueva» pone otra.</p>
        )}

        {/* Preparar */}
        <div className="flex flex-wrap items-center gap-2">
          <Button kt="P" disabled={!isAdmin || audit || running || !ipOk || tooShort} title={lock} onClick={prepare}>
            {running ? <Loader2 size={13} className="animate-spin" /> : <ScanLine size={13} />} {running ? "Preparando…" : configured || setup ? "Preparar otra vez" : "Preparar este equipo"}
          </Button>
          <ViewCommand id="scan.setup" context={ip} icon />
          {(setup || configured) && (
            <>
              <Button kt="N" kind="secondary" size="sm" disabled={!isAdmin || audit || busy !== null} title={lock} onClick={newPassword}>
                {busy === "password" ? <Loader2 size={13} className="animate-spin" /> : null} Contraseña nueva
              </Button>
              <ViewCommand id="scan.password" icon />
              {setup && setup.entryId > 0 && (
                <Button kt="H" kind="ghost" size="sm" disabled={!isAdmin || audit || busy !== null} title={lock} onClick={undoAll}>
                  Deshacer todo
                </Button>
              )}
            </>
          )}
        </div>

        {steps.length > 0 && !(setup && watch && watch.state !== "idle") && <Steps steps={steps} />}

        {/* El panel de la impresora */}
        {panel && (
          <section className="rounded-2xl border border-line-2 bg-panel-2 p-5" aria-label="Panel de la impresora">
            <div className="mb-4 flex items-center gap-3">
              <h3 className="text-sm font-semibold text-ink">Panel de la impresora · Libreta de direcciones → Carpeta</h3>
              <span className="ml-auto">{choice && <BrandBadge vendor={choice.vendor} />}</span>
            </div>
            <div className="grid gap-5 md:grid-cols-3">
              {setup ? (
                panelFields(setup).map((f) => <Field key={f.id} label={f.label} value={f.value} mono />)
              ) : (
                <>
                  <Field label="Ruta" value={panel.path} />
                  <Field label="Usuario" value={panel.user} />
                  <div className="min-w-0">
                    <span className="text-[11px] font-semibold text-dim">Contraseña</span>
                    <div className="text-sm text-dim">La de la última vez. AdminOps no la guarda: si se perdió, «Contraseña nueva».</div>
                  </div>
                </>
              )}
            </div>
            <div className="mt-4 space-y-1 text-sm text-dim">
              {setup?.speech && <p>{dictationText(setup.speech)}</p>}
              {ipPathText(panel.ipPath) && (
                <p>
                  <span data-copy={panel.ipPath ?? ""} className="font-mono text-ink">
                    {ipPathText(panel.ipPath)}
                  </span>
                </p>
              )}
              <p className="text-xs text-mute">{OLD_PRINTER_HINT}</p>
              {setup?.folderHadFiles && <p className="text-xs text-mute">La carpeta ya existía: lo que había dentro no se ha tocado.</p>}
            </div>
          </section>
        )}

        {/* Esperar un escaneo de prueba */}
        {(setup || configured) && ipOk && (
          <div className="space-y-3">
            {watch && watch.state !== "idle" ? (
              <>
                {setup && setupWorked(setup.steps) && (
                  <ul className="space-y-1.5 text-sm">
                    {steps
                      .filter((s) => s.id === "setup" || s.id === "reach")
                      .map((s) => (
                        <li key={s.id} className="flex items-center gap-2">
                          <StepIcon state={s.state} /> <span className="text-ink">{s.label}</span>
                        </li>
                      ))}
                  </ul>
                )}
                <div className={`flex flex-wrap items-center gap-3 rounded-lg border px-3 py-2 text-sm ${watch.state === "arrived" ? "border-ok/40 bg-ok/5" : "border-line bg-panel"}`} role="status" aria-live="polite">
                  {watch.state === "waiting" && (
                    <>
                      <Loader2 size={14} className="animate-spin text-neon" />
                      <span className="text-ink">Esperando un escaneo de prueba…</span>
                      <span className="font-mono text-xs text-mute">{waitedText(watch.startedMs, now)}</span>
                    </>
                  )}
                  {watch.state === "arrived" && (
                    <>
                      <CheckCircle2 size={14} className="text-ok" />
                      <span className="font-medium text-ink">{arrivedText(watch.file ?? "un archivo", watch.pages, watch.arrivedMs ?? now, now)}</span>
                    </>
                  )}
                  {watch.state === "timeout" && (
                    <>
                      <CircleAlert size={14} className="text-warn" />
                      <span className="text-ink">Pasaron 10 minutos y no llegó ningún escaneo.</span>
                    </>
                  )}
                </div>
              </>
            ) : null}

            {watch?.state === "arrived" && (
              <div className="space-y-2">
                <p className="flex items-center gap-2 text-sm text-ok" role="status">
                  <CheckCircle2 size={14} /> Escanear a carpeta funcionando
                </p>
                <div className="flex flex-wrap items-center gap-2">
                  <Button kt="A" onClick={() => run("open", () => scanFolderApi.openFolder())}>
                    <FolderOpen size={13} /> Abrir la carpeta
                  </Button>
                  <Button kt="S" kind="secondary" disabled={audit || busy !== null} title={lock} onClick={shortcut}>
                    Crear acceso en el Escritorio
                  </Button>
                  <ViewCommand id="scan.shortcut" icon />
                </div>
              </div>
            )}

            <div className="flex flex-wrap items-center gap-2">
              {(!watch || watch.state !== "waiting") && !publicNet && (
                <Button kt="T" kind="secondary" size="sm" disabled={running} onClick={startWatch}>
                  {watch ? "Esperar otro escaneo" : "Esperar un escaneo de prueba"}
                </Button>
              )}
              {watch?.state === "waiting" && (
                <Button kt="Q" kind="secondary" size="sm" disabled={busy !== null} onClick={explain}>
                  {busy === "diagnose" ? <Loader2 size={13} className="animate-spin" /> : null} ¿Por qué no llega?
                </Button>
              )}
              {watch?.state === "timeout" && !diagnosis && (
                <Button kt="Q" kind="secondary" size="sm" disabled={busy !== null} onClick={explain}>
                  ¿Por qué no llegó?
                </Button>
              )}
            </div>

            {diagnosis && (
              <div className="space-y-2 rounded-lg border border-warn/40 bg-warn/5 px-3 py-3 text-sm" role="alert">
                <p className="flex items-start gap-2 font-medium text-ink">
                  <CircleAlert size={14} className="mt-0.5 shrink-0 text-warn" /> {diagnosis.sentence}
                </p>
                {diagnosis.details.length > 0 && (
                  <ul className="ml-6 list-disc space-y-1 text-xs text-dim">
                    {diagnosis.details.map((d) => (
                      <li key={d}>{d}</li>
                    ))}
                  </ul>
                )}
                {offersSigning(diagnosis) && (
                  <div className="ml-6 space-y-2 border-t border-line/60 pt-2">
                    <label className="flex items-start gap-2 text-xs text-dim">
                      <input type="checkbox" checked={signingOn} onChange={(e) => setSigningOn(e.target.checked)} disabled={!isAdmin || audit} className="mt-0.5 size-3.5 accent-[var(--color-neon)]" />
                      <span>
                        <span className="text-ink">Quitar la firma obligatoria</span> · riesgo alto: baja la seguridad de este equipo en la red. Pide confirmación y se puede deshacer.
                      </span>
                    </label>
                    <div className="flex items-center gap-1">
                      <Button kt="F" kind="danger" size="sm" disabled={!signingOn || !isAdmin || audit || busy !== null} title={lock} onClick={applySigning}>
                        Aplicar lo marcado
                      </Button>
                      <ViewCommand id="scan.signing" icon />
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </Card>
  );
}
