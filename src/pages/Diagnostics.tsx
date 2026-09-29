import {
  AlertOctagon,
  ArrowRight,
  ExternalLink,
  BatteryMedium,
  CircleCheck,
  Cpu,
  HardDrive,
  Info,
  Loader2,
  RefreshCw,
  ShieldCheck,
  TriangleAlert,
  Zap,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { analyze, lastDiagnostics } from "../lib/diagRun";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import type { PageId } from "../components/Sidebar";
import { Bar, Card } from "../components/ui";
import {
  diagApi,
  tweaksApi,
  toolsApi,
  type Diagnostics as Diag,
  type DiagChanges,
  type Finding,
  type FindingAction,
  type Section,
  type Severity,
  type Tool,
} from "../lib/api";
import { bytes } from "../lib/format";
import { DriverRestoreButton } from "../components/Maintenance";


/** Mientras se analiza, cada sección se pinta en cuanto el backend la termina. */
const EMPTY_SECTION = { data: null, error: null } as unknown as Section<never>;

function skeleton(): Diag {
  const s = EMPTY_SECTION;
  return {
    timestamp: 0,
    host: "",
    os: "",
    cpu: "",
    ramTotal: 0,
    admin: false,
    volumes: [],
    disks: s,
    stability: s,
    drivers: s,
    battery: s,
    system: s,
    startupEnabled: s,
    bloatInstalled: s,
    softwareUpdates: s,
    hardware: s,
    smart: s,
    memoryTest: s,
    temperatures: s,
    security: s,
    tweaksApplied: 0,
    findings: [],
  } as unknown as Diag;
}

const SEV: Record<Severity, { icon: typeof Info; cls: string; label: string }> = {
  bad: { icon: AlertOctagon, cls: "text-bad", label: "Críticos" },
  warn: { icon: TriangleAlert, cls: "text-warn", label: "Advertencias" },
  info: { icon: Info, cls: "text-neon", label: "Informativos" },
};

const date = (iso: string) => {
  const d = new Date(iso);
  return isNaN(+d) ? iso : d.toLocaleString("es", { dateStyle: "medium", timeStyle: "short" });
};

/** Qué cambió desde el análisis anterior: problemas nuevos y resueltos. */
function ChangesStrip({ c }: { c: DiagChanges }) {
  const [open, setOpen] = useState(false);
  const when = new Date(c.since * 1000).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" });
  if (!c.new.length && !c.resolved.length)
    return <p className="mb-3 text-xs text-mute">Sin cambios desde el análisis del {when}.</p>;
  return (
    <div className="mb-3 rounded-lg border border-line bg-void/40 px-3 py-2 text-xs">
      <button onClick={() => setOpen(!open)} className="flex w-full flex-wrap items-center gap-x-3 gap-y-1 text-left">
        <span className="text-dim">Desde el análisis del {when}:</span>
        {c.new.length > 0 && <span className="text-warn">{c.new.length} {c.new.length === 1 ? "problema nuevo" : "problemas nuevos"}</span>}
        {c.resolved.length > 0 && <span className="text-ok">{c.resolved.length} {c.resolved.length === 1 ? "resuelto" : "resueltos"}</span>}
        {c.resolved.length > 0 && <span className="ml-auto text-neon">{open ? "Ocultar" : "Ver resueltos"}</span>}
      </button>
      {open && (
        <ul className="mt-2 space-y-0.5">
          {c.resolved.map((f) => (
            <li key={f.title} className="flex items-center gap-2 text-dim">
              <CircleCheck size={12} className="shrink-0 text-ok" />
              <span className="line-through decoration-mute/60">{f.title}</span>
              <span className="text-mute">· {f.area}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function Unavailable<T>({ section, children }: { section: Section<T>; children: (d: T) => ReactNode }) {
  if (section.data !== null && section.data !== undefined) return <>{children(section.data)}</>;
  // Sin datos y sin error: el backend todavía está con esa sección.
  if (!section.error)
    return (
      <p className="flex items-center gap-2 text-xs text-mute">
        <Loader2 size={12} className="animate-spin" /> Analizando…
      </p>
    );
  return <p className="text-xs break-words text-mute">{section.error}</p>;
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex justify-between gap-4 border-t border-line/70 py-1.5 text-sm first:border-t-0">
      <span className="text-dim">{label}</span>
      <span className="text-right text-ink">{children}</span>
    </div>
  );
}

const flag = (v: boolean | null, yes: string, no: string, goodWhenTrue = true) =>
  v === null ? (
    <span className="text-mute">Requiere admin</span>
  ) : (
    <span className={v === goodWhenTrue ? "text-ok" : "text-warn"}>{v ? yes : no}</span>
  );

function ToolButton({ tool, label, onOpen }: { tool: Tool; label: string; onOpen: (t: Tool) => void }) {
  return (
    <button onClick={() => onOpen(tool)} className="flex items-center gap-1 text-[11px] text-mute hover:text-neon">
      {label} <ExternalLink size={10} />
    </button>
  );
}

export function Diagnostics({
  focus,
  onNavigate,
}: {
  focus?: string | null;
  onNavigate: (page: PageId, focus?: string | null) => void;
}) {
  const [d, setD] = useState<Diag | null>(lastDiagnostics);
  const [running, setRunning] = useState(false);
  // Secciones que ya ha terminado el backend mientras el análisis sigue en curso.
  const [live, setLive] = useState<Diag | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hl, setHl] = useState<string | null>(null);
  const toast = useToast();

  // Resaltar y llevar a una sección de esta misma página.
  const goTo = useCallback((section: string) => {
    const el = document.getElementById(`focus-${section}`);
    if (!el) return;
    el.scrollIntoView({ behavior: "smooth", block: "start" });
    setHl(section);
    window.setTimeout(() => setHl((h) => (h === section ? null : h)), 2500);
  }, []);

  useEffect(() => {
    if (focus && d) goTo(focus);
  }, [focus, d, goTo]);

  const openTool = (tool: Tool) => diagApi.openTool(tool).catch((e) => toast("error", String(e)));

  // `fixing`: id del arreglo en curso (o "all" para el lote).
  const [fixing, setFixing] = useState<string | null>(null);

  const applyFix = useCallback(
    async (id: string, label: string) => {
      setFixing(id);
      try {
        toast("ok", `${label}: ${await tweaksApi.fixFinding(id)}`);
      } catch (e) {
        toast("error", `${label}: ${e}`);
      } finally {
        setFixing(null);
      }
    },
    [toast],
  );

  const act = (a: FindingAction) => {
    if (a.kind === "fix") applyFix(a.id, a.label);
    else if (a.kind === "tool") openTool(a.tool);
    else if (a.page === "diagnostics") {
      if (a.focus) goTo(a.focus);
    } else onNavigate(a.page as PageId, a.focus);
  };

  // Todo lo que se puede arreglar sin riesgo, de una vez.
  const safeFixes = (d?.findings ?? []).flatMap((f) => f.actions.filter((a) => a.kind === "fix" && a.safe));
  const fixAllSafe = async () => {
    setFixing("all");
    let done = 0;
    const failed: string[] = [];
    for (const a of safeFixes) {
      if (a.kind !== "fix") continue;
      try {
        await tweaksApi.fixFinding(a.id);
        done++;
      } catch {
        failed.push(a.label);
      }
    }
    setFixing(null);
    toast(failed.length ? "info" : "ok", failed.length ? `${done} arreglados; fallaron: ${failed.join(", ")}` : `${done} arreglados. Vuelve a analizar para comprobarlo.`);
  };

  const ring = (section: string) => (hl === section ? "border-neon! glow-neon" : "");

  const [backingUp, setBackingUp] = useState(false);
  const backupDrivers = async () => {
    setBackingUp(true);
    try {
      toast("ok", await toolsApi.backupDrivers());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBackingUp(false);
    }
  };

  const run = useCallback(async (force = false) => {
    setRunning(true);
    setError(null);
    setLive(skeleton());
    try {
      setD(await analyze(force));
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
      setLive(null);
    }
  }, []);

  useEffect(() => {
    if (!lastDiagnostics()) run();
  }, [run]);

  // El backend avisa de cada sección en cuanto la termina: se pintan una a una
  // en vez de esperar con la pantalla en blanco a que acaben todas.
  useEffect(() => {
    const un = listen<{ key: string; section: unknown }>("diagnostics-progress", ({ payload }) => {
      setLive((prev) => {
        if (!prev) return prev;
        if (payload.key === "meta") {
          const meta = payload.section as Partial<Diag>;
          return { ...prev, ...meta, findings: prev.findings };
        }
        return { ...prev, [payload.key]: payload.section } as Diag;
      });
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  if (error && !d)
    return (
      <div className="grid h-full place-items-center p-8 text-center">
        <p className="text-bad">{error}</p>
      </div>
    );

  // Sin resultado todavía: se va pintando lo que ya ha llegado.
  const shown = d ?? live;
  if (!shown)
    return (
      <div className="grid h-full place-items-center p-8 text-center">
        <div>
          <Loader2 size={28} className="mx-auto mb-3 animate-spin text-neon" />
          <p className="text-sm text-dim">Analizando discos, eventos, drivers y seguridad…</p>
        </div>
      </div>
    );

  const count = (s: Severity) => shown.findings.filter((f) => f.severity === s).length;

  return (
    <div className="mx-auto max-w-6xl p-6">
      <div className="mb-4 flex items-center gap-3">
        <p className="text-sm text-dim">
          {shown.timestamp ? `Análisis del ${new Date(shown.timestamp * 1000).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" })}` : "Analizando el equipo…"}
          {!shown.admin && <span className="ml-2 text-warn">· sin administrador algunos datos no están disponibles</span>}
        </p>
        <button
          onClick={() => run(false)}
          disabled={running}
          className="ml-auto flex items-center gap-1.5 rounded-md border border-neon/40 px-3 py-1.5 text-xs font-medium text-neon transition-colors hover:bg-neon/10 disabled:opacity-50"
        >
          <RefreshCw size={13} className={running ? "animate-spin" : ""} />
          {running ? "Analizando…" : "Volver a analizar"}
        </button>
        <button
          onClick={() => run(true)}
          disabled={running}
          title="Vuelve a leer también el hardware y las actualizaciones (winget) en vez de reutilizar lo de hace unos minutos"
          className="rounded-md px-2 py-1.5 text-xs text-mute transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-50"
        >
          A fondo
        </button>
      </div>

      {/* Hallazgos */}
      <section className="mb-4 rounded-xl border border-line bg-panel p-4">
        <div className="mb-3 flex gap-3">
          {(["bad", "warn", "info"] as Severity[]).map((s) => {
            const S = SEV[s];
            return (
              <div key={s} className="flex items-center gap-2 rounded-lg border border-line bg-void/40 px-3 py-2">
                <S.icon size={16} className={S.cls} />
                <span className={`font-mono text-lg ${S.cls}`}>{count(s)}</span>
                <span className="text-xs text-dim">{S.label}</span>
              </div>
            );
          })}
        </div>
        {shown.changes && <ChangesStrip c={shown.changes} />}
        {safeFixes.length > 0 && (
          <button
            onClick={fixAllSafe}
            disabled={fixing !== null}
            className="mb-3 flex items-center gap-1.5 rounded-md border border-neon/50 px-3 py-1.5 text-xs font-medium text-neon transition-colors hover:bg-neon/10 disabled:opacity-50"
            title="Aplica solo lo que no cambia el comportamiento de Windows ni borra archivos tuyos"
          >
            {fixing === "all" ? <Loader2 size={13} className="animate-spin" /> : <Zap size={13} />}
            Arreglar todo lo seguro ({safeFixes.length})
          </button>
        )}
        {shown.findings.length === 0 ? (
          <p className="flex items-center gap-2 py-2 text-sm text-ok">
            <CircleCheck size={16} /> No se encontraron problemas.
          </p>
        ) : (
          <ul className="space-y-1">
            {shown.findings.map((f: Finding, i) => {
              const S = SEV[f.severity];
              return (
                <li
                  key={i}
                  onClick={() => f.actions[0] && act(f.actions[0])}
                  className={`group flex gap-3 rounded-lg px-2 py-2 hover:bg-panel-2 ${f.actions.length ? "cursor-pointer" : ""}`}
                  title={f.actions[0]?.label}
                >
                  <S.icon size={15} className={`mt-0.5 shrink-0 ${S.cls}`} />
                  <div className="min-w-0 flex-1">
                    <p className="text-sm text-ink group-hover:text-neon">
                      {f.title}
                      {shown.changes?.new.includes(f.title) && (
                        <span className="ml-2 rounded bg-warn/15 px-1.5 py-px align-middle text-[10px] font-medium text-warn">Nuevo</span>
                      )}
                    </p>
                    <p className="text-xs break-words text-dim">
                      <span className="text-mute">{f.area}</span>
                      {f.detail && <> · {f.detail}</>}
                    </p>
                    {f.actions.length > 0 && (
                      <div className="mt-1.5 flex flex-wrap gap-1.5">
                        {f.actions.map((a) => (
                          <button
                            key={a.label}
                            onClick={(e) => {
                              e.stopPropagation();
                              act(a);
                            }}
                            disabled={a.kind === "fix" && fixing !== null}
                            className={`flex items-center gap-1 rounded-md border px-2 py-0.5 text-[11px] transition-colors disabled:opacity-40 ${
                              a.kind === "fix" ? "border-neon/50 text-neon hover:bg-neon/10" : "border-line-2 text-dim hover:border-neon/50 hover:text-neon"
                            }`}
                          >
                            {a.kind === "fix" && fixing === a.id ? <Loader2 size={10} className="animate-spin" /> : null}
                            {a.label}
                            {a.kind === "tool" ? <ExternalLink size={10} /> : a.kind === "page" ? <ArrowRight size={10} /> : null}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <div className="grid grid-cols-12 gap-4">
        {/* Discos */}
        <Card
          id="focus-disks"
          title="Salud de discos"
          icon={<HardDrive size={14} />}
          className={`col-span-12 lg:col-span-7 ${ring("disks")}`}
          right={
            <button onClick={() => onNavigate("cleanup")} className="flex items-center gap-1 text-[11px] text-mute hover:text-neon">
              Liberar espacio <ArrowRight size={10} />
            </button>
          }
        >
          <Unavailable section={shown.disks}>
            {(disks) => (
              <table className="w-full text-sm">
                <thead>
                  <tr className="text-left text-[11px] text-mute">
                    <th className="pb-2 font-medium">Disco</th>
                    <th className="pb-2 font-medium">Estado</th>
                    <th className="pb-2 text-right font-medium">Temp.</th>
                    <th className="pb-2 text-right font-medium">Desgaste</th>
                  </tr>
                </thead>
                <tbody>
                  {disks.map((k) => (
                    <tr key={k.name + k.size} className="border-t border-line/70">
                      <td className="py-1.5 pr-3">
                        <div className="text-ink">{k.name}</div>
                        <div className="font-mono text-[11px] text-mute">
                          {k.mediaType} · {k.busType} · {bytes(k.size)}
                        </div>
                      </td>
                      <td className={k.health === "Healthy" ? "text-ok" : "text-bad"}>
                        {k.health === "Healthy" ? "Saludable" : k.health}
                      </td>
                      <td className="text-right font-mono text-xs">{k.temperature ? `${k.temperature} °C` : "—"}</td>
                      <td className="text-right font-mono text-xs">{k.wear !== null ? `${k.wear}%` : "—"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Unavailable>
          {!shown.admin && <p className="mt-2 text-xs text-mute">Temperatura y desgaste requieren administrador.</p>}
        </Card>

        {/* Seguridad */}
        <Card
          id="focus-security"
          title="Sistema y seguridad"
          icon={<ShieldCheck size={14} />}
          className={`col-span-12 lg:col-span-5 ${ring("security")}`}
          right={
            <div className="flex gap-3">
              <ToolButton tool="windowsSecurity" label="Seguridad" onOpen={openTool} />
              <ToolButton tool="windowsUpdate" label="Update" onOpen={openTool} />
            </div>
          }
        >
          <Unavailable section={shown.system}>
            {(s) => (
              <>
                <Row label="Antivirus">{s.antivirus.join(", ") || <span className="text-bad">No detectado</span>}</Row>
                <Row label="Tiempo real (Defender)">{flag(s.defenderRealtime, "Activo", "Desactivado")}</Row>
                <Row label="Última actualización">
                  {s.lastUpdate ? `${date(s.lastUpdate).split(",")[0]} · ${s.lastUpdateId ?? ""}` : "—"}
                </Row>
                <Row label="Reinicio pendiente">{flag(s.pendingReboot, "Sí", "No", false)}</Row>
                <Row label="Windows activado">{flag(s.activated, "Sí", "No")}</Row>
                <Row label="Arranque seguro">{flag(s.secureBoot, "Activado", "Desactivado")}</Row>
                <Row label="TPM">{flag(s.tpmReady, "Listo", "No listo")}</Row>
                <Row label="Último arranque">{date(s.lastBoot)}</Row>
              </>
            )}
          </Unavailable>
        </Card>

        {/* Estabilidad */}
        <Card
          id="focus-stability"
          title="Estabilidad"
          icon={<Zap size={14} />}
          className={`col-span-12 lg:col-span-7 ${ring("stability")}`}
          right={
            <div className="flex gap-3">
              <ToolButton tool="reliability" label="Confiabilidad" onOpen={openTool} />
              <ToolButton tool="eventViewer" label="Eventos" onOpen={openTool} />
            </div>
          }
        >
          <Unavailable section={shown.stability}>
            {(s) => (
              <>
                <div className="mb-3 grid grid-cols-3 gap-3">
                  {[
                    { n: s.bugchecks.length, label: "Pantallazos azules", bad: s.bugchecks.length > 0 },
                    { n: s.unexpectedShutdowns.length, label: "Apagados inesperados", bad: s.unexpectedShutdowns.length >= 2 },
                    { n: s.crashes.reduce((a, c) => a + c.count, 0), label: "Cierres de apps", bad: false },
                  ].map((x) => (
                    <div key={x.label} className="rounded-lg border border-line bg-void/40 px-3 py-2">
                      <div className={`font-mono text-xl ${x.bad ? "text-bad" : "text-ink"}`}>{x.n}</div>
                      <div className="text-[11px] text-dim">
                        {x.label} · {s.days} días
                      </div>
                    </div>
                  ))}
                </div>
                {s.bugchecks.map((b) => (
                  <div key={b.time} className="mb-2 rounded-lg border border-bad/30 bg-bad/5 px-3 py-2 text-sm">
                    <div className="flex justify-between">
                      <span className="font-mono text-bad">
                        {b.code} {b.name}
                      </span>
                      <span className="text-xs text-mute">{date(b.time)}</span>
                    </div>
                    {b.hint && <p className="text-xs text-dim">{b.hint}</p>}
                  </div>
                ))}
                {s.crashes.length > 0 && (
                  <>
                    <p className="mt-2 mb-1 text-[11px] text-mute">Apps que fallan</p>
                    {s.crashes.map((c) => (
                      <div key={c.app} className="flex items-center gap-3 py-1 text-sm">
                        <span className="min-w-0 flex-1 truncate text-ink">{c.app}</span>
                        <span className="text-xs text-mute">{date(c.last)}</span>
                        <span className="w-8 text-right font-mono text-xs text-warn">×{c.count}</span>
                      </div>
                    ))}
                  </>
                )}
                {s.minidumps?.some((m) => m.analysis?.culprit) && (
                  <>
                    <p className="mt-2 mb-1 text-[11px] text-mute">Volcados analizados</p>
                    {s.minidumps
                      .filter((m) => m.analysis)
                      .map((m) => (
                        <div key={m.name} className="py-1 text-sm" title={m.analysis!.stackDrivers.length ? `En la pila: ${m.analysis!.stackDrivers.join(", ")}` : undefined}>
                          <div className="flex items-center gap-3">
                            <span className="text-xs text-mute">{date(m.time)}</span>
                            <span className="font-mono text-xs text-dim">{m.analysis!.bugcheck}</span>
                            <span className="min-w-0 flex-1 truncate text-right">
                              {m.analysis!.culprit ? (
                                <span className="font-mono text-xs text-warn">{m.analysis!.culprit}</span>
                              ) : (
                                <span className="text-xs text-mute">sin driver concreto</span>
                              )}
                            </span>
                          </div>
                          {m.analysis!.culpritHint && <p className="text-xs text-dim">{m.analysis!.culpritHint}</p>}
                        </div>
                      ))}
                    <p className="mt-1 text-[11px] text-mute">Driver probable según la pila del fallo: es una pista para empezar, no un veredicto.</p>
                  </>
                )}
                <p className="mt-3 text-xs text-mute">
                  Tiempo de arranque:{" "}
                  {s.bootTimes === null ? (
                    "requiere administrador"
                  ) : s.bootTimes.length === 0 ? (
                    "sin registros"
                  ) : (
                    <span className="font-mono text-dim">{(s.bootTimes[0].ms / 1000).toFixed(1)} s</span>
                  )}
                  {" · "}Volcados de memoria:{" "}
                  {s.minidumps === null ? "requiere administrador" : `${s.minidumps.length}`}
                </p>
              </>
            )}
          </Unavailable>
        </Card>

        <div className="col-span-12 flex flex-col gap-4 lg:col-span-5">
          {/* Drivers */}
          <Card
            id="focus-drivers"
            title="Drivers"
            icon={<Cpu size={14} />}
            className={ring("drivers")}
            right={
              <div className="flex gap-3">
                <button
                  onClick={backupDrivers}
                  disabled={backingUp}
                  className="flex items-center gap-1 text-[11px] text-mute hover:text-neon disabled:opacity-50"
                  title="Exporta los drivers de terceros para reinstalarlos tras formatear"
                >
                  Copia de drivers <ArrowRight size={10} />
                </button>
                <DriverRestoreButton />
                <ToolButton tool="deviceManager" label="Dispositivos" onOpen={openTool} />
              </div>
            }
          >
            <TaskStatus task="drivers-backup" active={backingUp} fallback="Copiando drivers…" cancellable={false} className="mb-2" />
            <Unavailable section={shown.drivers}>
              {(drivers) =>
                drivers.length === 0 ? (
                  <p className="flex items-center gap-2 text-sm text-ok">
                    <CircleCheck size={15} /> Todos los dispositivos funcionan.
                  </p>
                ) : (
                  <ul className="space-y-2">
                    {drivers.map((x) => (
                      <li key={x.deviceId} className="text-sm">
                        <div className="text-ink">{x.name}</div>
                        <div className="text-xs text-warn">
                          {x.problem} <span className="font-mono text-mute">(código {x.code})</span>
                        </div>
                      </li>
                    ))}
                  </ul>
                )
              }
            </Unavailable>
          </Card>

          {/* Batería */}
          <Card id="focus-battery" title="Batería" icon={<BatteryMedium size={14} />} className={ring("battery")}>
            <Unavailable section={shown.battery}>
              {(b) => {
                if (!b) return <p className="text-sm text-mute">Equipo de sobremesa: sin batería.</p>;
                const health = b.design ? (b.full / b.design) * 100 : 0;
                return (
                  <>
                    <div className="mb-1 flex justify-between text-sm">
                      <span className="text-dim">Capacidad respecto a la original</span>
                      <span className="font-mono">{health.toFixed(0)}%</span>
                    </div>
                    <Bar value={health} color={health < 60 ? "var(--color-bad)" : health < 80 ? "var(--color-warn)" : "var(--color-ok)"} />
                    <p className="mt-2 text-xs text-mute">
                      {b.full.toLocaleString("es")} de {b.design.toLocaleString("es")} mWh
                      {b.cycles !== null && ` · ${b.cycles} ciclos`} · {b.manufacturer} {b.name}
                    </p>
                  </>
                );
              }}
            </Unavailable>
          </Card>
        </div>
      </div>
    </div>
  );
}
