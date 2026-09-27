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
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import type { PageId } from "../components/Sidebar";
import { Bar, Card } from "../components/ui";
import {
  diagApi,
  toolsApi,
  type Diagnostics as Diag,
  type Finding,
  type FindingAction,
  type Section,
  type Severity,
  type Tool,
} from "../lib/api";
import { bytes } from "../lib/format";

// El análisis tarda unos segundos: se conserva al cambiar de página, y una
// ejecución en curso se comparte (StrictMode monta los efectos dos veces).
let cached: Diag | null = null;
let inflight: Promise<Diag> | null = null;

function analyze(): Promise<Diag> {
  inflight ??= diagApi.run().finally(() => (inflight = null));
  return inflight;
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

function Unavailable<T>({ section, children }: { section: Section<T>; children: (d: T) => ReactNode }) {
  if (section.data !== null && section.data !== undefined) return <>{children(section.data)}</>;
  return <p className="text-xs break-words text-mute">{section.error ?? "Sin datos."}</p>;
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
  const [d, setD] = useState<Diag | null>(cached);
  const [running, setRunning] = useState(false);
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

  const act = (a: FindingAction) => {
    if (a.kind === "tool") openTool(a.tool);
    else if (a.page === "diagnostics") {
      if (a.focus) goTo(a.focus);
    } else onNavigate(a.page as PageId, a.focus);
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

  const run = useCallback(async () => {
    setRunning(true);
    setError(null);
    try {
      cached = await analyze();
      setD(cached);
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }, []);

  useEffect(() => {
    if (!cached) run();
  }, [run]);

  if (!d)
    return (
      <div className="grid h-full place-items-center p-8 text-center">
        {error ? (
          <p className="text-bad">{error}</p>
        ) : (
          <div>
            <Loader2 size={28} className="mx-auto mb-3 animate-spin text-neon" />
            <p className="text-sm text-dim">Analizando discos, eventos, drivers y seguridad…</p>
            <p className="mt-1 text-xs text-mute">Suele tardar entre 5 y 15 segundos.</p>
          </div>
        )}
      </div>
    );

  const count = (s: Severity) => d.findings.filter((f) => f.severity === s).length;

  return (
    <div className="mx-auto max-w-6xl p-6">
      <div className="mb-4 flex items-center gap-3">
        <p className="text-sm text-dim">
          Análisis del {new Date(d.timestamp * 1000).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" })}
          {!d.admin && <span className="ml-2 text-warn">· sin administrador algunos datos no están disponibles</span>}
        </p>
        <button
          onClick={run}
          disabled={running}
          className="ml-auto flex items-center gap-1.5 rounded-md border border-neon/40 px-3 py-1.5 text-xs font-medium text-neon transition-colors hover:bg-neon/10 disabled:opacity-50"
        >
          <RefreshCw size={13} className={running ? "animate-spin" : ""} />
          {running ? "Analizando…" : "Volver a analizar"}
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
        {d.findings.length === 0 ? (
          <p className="flex items-center gap-2 py-2 text-sm text-ok">
            <CircleCheck size={16} /> No se encontraron problemas.
          </p>
        ) : (
          <ul className="space-y-1">
            {d.findings.map((f: Finding, i) => {
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
                    <p className="text-sm text-ink group-hover:text-neon">{f.title}</p>
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
                            className="flex items-center gap-1 rounded-md border border-line-2 px-2 py-0.5 text-[11px] text-dim transition-colors hover:border-neon/50 hover:text-neon"
                          >
                            {a.label}
                            {a.kind === "tool" ? <ExternalLink size={10} /> : <ArrowRight size={10} />}
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
          <Unavailable section={d.disks}>
            {(disks) => (
              <table className="w-full text-sm">
                <thead>
                  <tr className="text-left text-[10px] tracking-widest text-mute uppercase">
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
          {!d.admin && <p className="mt-2 text-xs text-mute">Temperatura y desgaste requieren administrador.</p>}
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
          <Unavailable section={d.system}>
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
          <Unavailable section={d.stability}>
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
                    <p className="mt-2 mb-1 text-[10px] tracking-widest text-mute uppercase">Apps que fallan</p>
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
                    <p className="mt-2 mb-1 text-[10px] tracking-widest text-mute uppercase">Volcados analizados</p>
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
                <ToolButton tool="deviceManager" label="Dispositivos" onOpen={openTool} />
              </div>
            }
          >
            <TaskStatus task="drivers-backup" active={backingUp} fallback="Copiando drivers…" cancellable={false} className="mb-2" />
            <Unavailable section={d.drivers}>
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
            <Unavailable section={d.battery}>
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
