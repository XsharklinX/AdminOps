// Diagnóstico de experto (1.2.8): lo que haría un técnico senior con
// Sysinternals, el Visor de eventos y paciencia, hecho en minutos y explicado.
import { Activity, Bug, Flame, Gauge, HardDrive, ListTree, Loader2, Play, RotateCcw, ScrollText, ShieldAlert, Square } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "./feedback";
import { DataTable, type Column } from "./DataTable";
import { FindingList } from "./FindingList";
import { TimeChart } from "./TimeChart";
import { TypedConfirm } from "./DiskPartitions";
import { Button, Card, EmptyState, ErrorState, Loading, smallBtn } from "./ui";
import type { PageId } from "./Sidebar";
import { appApi, expertApi, troubleshootApi, tweaksApi, type Autorun, type CrashReport, type EventGroup, type PerfInsights, type StressResult, type TroubleFinding, type TroubleFix } from "../lib/api";
import { fullDate, shortDate } from "../lib/format";
import { CodeLinks } from "./CodeLinks";

const CAT: Record<Autorun["category"], string> = {
  run: "Registro",
  startup: "Carpeta Inicio",
  task: "Tarea programada",
  service: "Servicio",
  driver: "Driver",
  winlogon: "Inicio de sesión",
  ifeo: "Depurador (IFEO)",
  appinit: "AppInit",
  explorer: "Explorador",
  wmi: "WMI",
};

// ---------- Todo lo que arranca con Windows ----------

export function AutorunsCard({ isAdmin }: { isAdmin: boolean }) {
  const toast = useToast();
  const [list, setList] = useState<Autorun[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<string>("concern");
  const [busy, setBusy] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const load = useCallback(() => {
    setError(null);
    setLoading(true);
    expertApi
      .autoruns()
      .then(setList, (e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);
  const counts = useMemo(() => {
    const c: Record<string, number> = { all: list?.length ?? 0, concern: list?.filter((a) => a.concern !== "none").length ?? 0, unsigned: list?.filter((a) => a.exists && !a.signed).length ?? 0 };
    for (const a of list ?? []) c[a.category] = (c[a.category] ?? 0) + 1;
    return c;
  }, [list]);
  const rows = (list ?? []).filter((a) => (filter === "all" ? true : filter === "concern" ? a.concern !== "none" : filter === "unsigned" ? a.exists && !a.signed : a.category === filter));
  const disable = async (a: Autorun) => {
    setBusy(a.id);
    try {
      toast("ok", await expertApi.disableAutorun(a.id));
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };
  const cols: Column<Autorun>[] = [
    { id: "name", header: "Entrada", sortBy: (a) => a.name, cell: (a) => <span className="font-medium text-ink" data-ctx-path={a.image || undefined}>{a.name}</span> },
    { id: "cat", header: "Dónde", sortBy: (a) => a.category, cell: (a) => <span className="text-dim">{CAT[a.category]}</span> },
    { id: "image", header: "Archivo", cell: (a) => <span className="font-mono text-[11px] break-all text-mute" title={a.command}>{a.image || a.command}</span> },
    { id: "pub", header: "Editor", sortBy: (a) => a.publisher, cell: (a) => (a.signed ? <span className="text-dim">{a.publisher}</span> : <span className="text-warn">{a.exists ? "Sin firma" : "—"}</span>) },
    { id: "flags", header: "Llama la atención", cell: (a) => <span className={a.concern === "high" ? "text-bad" : a.concern === "low" ? "text-warn" : "text-mute"}>{a.flags.join(" · ") || "—"}</span> },
    {
      id: "act",
      header: "",
      align: "right",
      stopClick: true,
      cell: (a) =>
        a.canDisable ? (
          <button className={smallBtn} disabled={!!busy || (!isAdmin && !a.location.startsWith("HKCU"))} onClick={() => void disable(a)}>
            {busy === a.id ? <Loader2 size={11} className="animate-spin" /> : null} Desactivar
          </button>
        ) : null,
    },
  ];
  const chip = (id: string, label: string) => (
    <button key={id} onClick={() => setFilter(id)} className={`rounded-full border px-2.5 py-0.5 text-xs ${filter === id ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`}>
      {label} <span className="text-mute">{counts[id] ?? 0}</span>
    </button>
  );
  return (
    <Card title="Todo lo que arranca con Windows" icon={<ListTree size={14} />} right={list && <button className={smallBtn} onClick={load}>Volver a leer</button>}>
      <p className="mb-3 text-xs text-dim">
        Todos los sitios por donde algo puede arrancar solo, como Autoruns: registro, carpetas de Inicio, tareas, servicios y drivers que no son de Microsoft, inicio de sesión, «depuradores», AppInit,
        Explorador y WMI. Desactivar no borra nada y se puede deshacer desde el Historial.
      </p>
      {!list && !error && !loading && (
        <Button onClick={load}>
          <Play size={14} /> Revisarlo todo (un minuto)
        </Button>
      )}
      {loading && <Loading text="Leyendo registro, tareas, servicios y drivers, y comprobando firmas…" />}
      {error && <ErrorState message={error} onRetry={load} where="Todo lo que arranca con Windows" />}
      {list && (
        <>
          <div className="mb-3 flex flex-wrap gap-1.5">
            {chip("concern", "Llaman la atención")}
            {chip("unsigned", "Sin firma")}
            {chip("all", "Todo")}
            {(Object.keys(CAT) as Autorun["category"][]).filter((c) => counts[c]).map((c) => chip(c, CAT[c]))}
          </div>
          {rows.length === 0 ? (
            <EmptyState title={filter === "concern" ? "Nada llama la atención" : "Nada en este grupo"}>Todo lo que arranca está firmado, existe y no sale de carpetas temporales.</EmptyState>
          ) : (
            <div className="pane-lg overflow-auto">
              <DataTable columns={cols} rows={rows} rowKey={(a) => a.id} size="xs" sticky alignTop />
            </div>
          )}
        </>
      )}
    </Card>
  );
}

// ---------- Pantallazos azules ----------

export function CrashCulprits({ isAdmin }: { isAdmin: boolean }) {
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [r, setR] = useState<CrashReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rollback, setRollback] = useState<{ inf: string; device: string; to: string } | null>(null);
  const load = useCallback(() => {
    setError(null);
    expertApi.crashes().then(setR, (e) => setError(String(e)));
  }, []);
  useEffect(load, [load]);
  const doRollback = async () => {
    if (!rollback) return;
    const target = rollback;
    setRollback(null);
    try {
      await tweaksApi.createRestorePoint().catch(() => undefined);
      toast("ok", await expertApi.rollback(target.inf));
      load();
    } catch (e) {
      toast("error", String(e));
    }
  };
  const verifier = async (drivers: string[]) => {
    const ok = await confirm({
      title: "Driver Verifier",
      danger: true,
      confirmLabel: "Activar y reiniciar después",
      body: (
        <div className="space-y-2">
          <p>Windows someterá a {drivers.join(", ")} a pruebas duras. Si el driver es el culpable, el próximo pantallazo lo dirá con nombre.</p>
          <p className="text-warn">Puede provocar pantallazos a propósito. Si el equipo no arranca: arranque avanzado → modo seguro → «verifier /reset», o desactívalo aquí.</p>
        </div>
      ),
    });
    if (!ok) return;
    try {
      toast("ok", await expertApi.verifierOn(drivers));
      load();
    } catch (e) {
      toast("error", String(e));
    }
  };
  if (error) return <ErrorState message={error} onRetry={load} where="Pantallazos azules" />;
  if (!r) return <Loading text="Agrupando los pantallazos por causa…" />;
  if (!r.groups.length && !r.verifier.length) return null;
  return (
    <Card title="Pantallazos: del driver culpable a la solución" icon={<Bug size={14} />}>
      {r.verifier.length > 0 && (
        <div className="mb-3 flex flex-wrap items-center gap-2 rounded-lg border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
          <ShieldAlert size={14} /> Driver Verifier está vigilando: {r.verifier.join(", ")}.
          <Button size="sm" kind="danger" onClick={() => expertApi.verifierOff().then((m) => (toast("ok", m), load()), (e) => toast("error", String(e)))} disabled={!isAdmin}>
            Desactivar Driver Verifier
          </Button>
        </div>
      )}
      <div className="space-y-3">
        {r.groups.map((g) => (
          <div key={g.driver || "?"} className="rounded-lg border border-line bg-void/30 p-3">
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <b className="text-sm text-ink">
                {g.count} {g.count === 1 ? "pantallazo" : "pantallazos"} · {g.driver ? <span className="font-mono text-bad">{g.driver}</span> : "sin culpable claro"}
              </b>
              <span className="text-[11px] text-mute">
                {shortDate(g.first)} – {shortDate(g.last)} · <CodeLinks text={g.codes.join(", ")} />
              </span>
            </div>
            {g.device && (
              <p className="mt-1 text-xs text-dim">
                {g.device} · versión {g.version || "?"}
                {g.driverDate && ` del ${g.driverDate}`}
                {g.installed > 0 && ` · instalado el ${fullDate(g.installed)}`}
              </p>
            )}
            {g.hint && <p className="mt-1 text-xs text-dim">{g.hint}</p>}
            {g.sinceInstall && <p className="mt-1 text-xs text-warn">Todos los pantallazos son posteriores a la instalación de esta versión: lo más probable es que sea ella.</p>}
            <div className="mt-2 flex flex-wrap gap-2">
              {g.older.length > 0 && g.inf && (
                <button className={smallBtn} disabled={!isAdmin} onClick={() => setRollback({ inf: g.inf, device: g.device || g.driver, to: g.older[0].version })}>
                  <RotateCcw size={11} /> Volver a la versión {g.older[0].version}
                </button>
              )}
              <button className={smallBtn} onClick={() => void troubleshootApi.fix("open:ms-settings:windowsupdate-optionalupdates")}>
                Buscar una versión más nueva
              </button>
              {g.driver && (
                <button className={smallBtn} disabled={!isAdmin || r.verifier.length > 0} onClick={() => void verifier([g.driver])}>
                  Driver Verifier sobre {g.driver}
                </button>
              )}
            </div>
          </div>
        ))}
        {r.withoutDump > 0 && <p className="text-[11px] text-mute">{r.withoutDump} pantallazo(s) más sin volcado que leer: Windows no guardó el detalle (revisa que haya espacio y archivo de paginación).</p>}
      </div>
      {rollback && (
        <TypedConfirm
          title="Volver al driver anterior"
          body={`Se quitará el driver en uso de ${rollback.device} y Windows instalará la versión ${rollback.to}, que sigue en el equipo. Antes se crea un punto de restauración.`}
          word="VOLVER"
          confirmLabel="Volver al anterior"
          touches={[`El driver de ${rollback.device}`]}
          keeps={["Tus archivos y programas", "La versión anterior del driver (se reinstala)"]}
          checks={["He guardado lo que tengo abierto (puede parpadear la pantalla o cortarse la red un momento)."]}
          onClose={() => setRollback(null)}
          onConfirm={() => void doRollback()}
        />
      )}
      {dialog}
    </Card>
  );
}

// ---------- Prueba de estrés ----------

export function StressCard() {
  const toast = useToast();
  const [secs, setSecs] = useState(300);
  const [memory, setMemory] = useState(false);
  const [running, setRunning] = useState(false);
  const [live, setLive] = useState<StressResult | null>(null);
  const [history, setHistory] = useState<StressResult[]>([]);
  useEffect(() => void expertApi.stressHistory().then(setHistory, () => undefined), []);
  useEffect(() => {
    if (!running) return;
    const t = window.setInterval(() => void expertApi.stressLive().then((r) => r && setLive(r), () => undefined), 2000);
    return () => window.clearInterval(t);
  }, [running]);
  const start = async () => {
    setRunning(true);
    setLive(null);
    try {
      const r = await expertApi.stress(secs, memory);
      setLive(r);
      if (!r.cancelled) setHistory((h) => [...h, r].slice(-10));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(false);
    }
  };
  const r = live;
  const prev = history.length > 1 ? history[history.length - 2] : null;
  const times = r?.samples.map((s) => r.at + s.t) ?? [];
  return (
    <Card title="Prueba de estrés con temperatura" icon={<Flame size={14} />}>
      <p className="mb-3 text-xs text-dim">Carga el procesador al 100 % unos minutos y mide temperatura, frecuencia y ventilador. Se para sola si pasa de 97 °C. Sirve para ver si baja la velocidad por calor o si toca limpiar y cambiar la pasta térmica.</p>
      <div className="flex flex-wrap items-center gap-3 text-xs text-dim">
        <select value={secs} onChange={(e) => setSecs(Number(e.target.value))} disabled={running} className="rounded-md border border-line bg-void/60 px-2 py-1.5 text-sm text-ink" aria-label="Duración">
          <option value={120}>2 minutos</option>
          <option value={300}>5 minutos</option>
          <option value={600}>10 minutos</option>
        </select>
        <label className="flex items-center gap-1.5">
          <input type="checkbox" checked={memory} onChange={(e) => setMemory(e.target.checked)} disabled={running} className="accent-[var(--color-neon)]" /> También la memoria
        </label>
        {running ? (
          <Button kind="danger" onClick={() => void appApi.cancelTask("stress")}>
            <Square size={13} /> Parar
          </Button>
        ) : (
          <Button onClick={start}>
            <Play size={14} /> Empezar
          </Button>
        )}
      </div>
      {r && r.samples.length > 1 && (
        <div className="mt-4">
          <TimeChart
            times={times}
            unit=""
            height={160}
            gapSeconds={30}
            series={[
              { label: "°C", color: "var(--color-bad)", values: r.samples.map((s) => s.temp) },
              { label: "GHz ×20", color: "var(--color-neon)", values: r.samples.map((s) => (s.mhz ? s.mhz / 50 : null)) },
            ]}
            describe={(i) => {
              const s = r.samples[i];
              return `${s.temp != null ? `${s.temp.toFixed(0)} °C` : "sin temperatura"} · ${s.mhz ? `${(s.mhz / 1000).toFixed(2)} GHz` : ""}${s.fan ? ` · ventilador ${s.fan.toFixed(0)} rpm` : ""}`;
            }}
          />
        </div>
      )}
      {r && r.verdict.length > 0 && !running && (
        <div className={`mt-3 rounded-lg border px-3 py-2 text-xs ${r.level === "bad" ? "border-bad/40 text-bad" : r.level === "warn" ? "border-warn/40 text-warn" : "border-ok/40 text-ok"}`}>
          {r.verdict.map((v) => (
            <p key={v}>{v}</p>
          ))}
          {prev && prev.maxTemp != null && r.maxTemp != null && (
            <p className="mt-1 text-dim">
              La vez anterior ({shortDate(prev.at)}): máximo {prev.maxTemp.toFixed(0)} °C → ahora {r.maxTemp.toFixed(0)} °C.
            </p>
          )}
        </div>
      )}
    </Card>
  );
}

// ---------- Fugas de memoria y disco al 100 % ----------

export function PerfInsightsCard() {
  const [d, setD] = useState<PerfInsights | null>(null);
  useEffect(() => void expertApi.insights().then(setD, () => undefined), []);
  if (!d) return null;
  if (d.hours < 2)
    return (
      <Card title="Lo que crece y lo que satura el disco" icon={<Activity size={14} />}>
        <p className="text-xs text-mute">Hacen falta al menos un par de horas de historial con AdminOps abierta para ver fugas de memoria y quién satura el disco. Hay {d.hours.toFixed(1)} h.</p>
      </Card>
    );
  return (
    <Card title="Lo que crece y lo que satura el disco" icon={<Activity size={14} />}>
      <div className="space-y-3 text-xs">
        {d.leaks.length === 0 ? (
          <p className="text-ok">Ningún programa crece sin parar en memoria ({d.hours.toFixed(0)} h de historial).</p>
        ) : (
          d.leaks.map((l) => (
            <div key={l.name} className="rounded-lg border border-warn/40 bg-warn/5 px-3 py-2">
              <b className="text-ink">{l.name}</b>{" "}
              <span className="text-dim">
                {(l.fromMb / 1024).toFixed(1)} GB → {(l.toMb / 1024).toFixed(1)} GB en {l.hours.toFixed(1)} h
              </span>
              <p className="text-warn">Sube sin parar: es una fuga de memoria. Reiniciarlo la libera; si se repite, actualízalo.</p>
            </div>
          ))
        )}
        {d.saturatedMinutes > 0 ? (
          <div>
            <p className="text-dim">
              <HardDrive size={12} className="mr-1 inline" />
              El disco estuvo al 85 % o más durante {d.saturatedMinutes} minuto(s). Los que más lo usaban entonces:
            </p>
            <ul className="mt-1 space-y-0.5">
              {d.diskHogs.map((h) => (
                <li key={h.name} className="flex justify-between gap-3">
                  <span className="text-ink">{h.name}</span>
                  <span className="font-mono text-mute">
                    {h.minutes} min · {(h.avgKbs / 1024).toFixed(1)} MB/s
                  </span>
                </li>
              ))}
            </ul>
          </div>
        ) : (
          <p className="text-ok">El disco no se ha quedado saturado en el historial.</p>
        )}
        {d.peak && (
          <p className="text-dim">
            <Gauge size={12} className="mr-1 inline" />
            Hacia las {d.peak.hour} h va más cargado (procesador {d.peak.cpu.toFixed(0)} %, disco {d.peak.busy.toFixed(0)} %){d.peak.names.length > 0 && `; coincide con ${d.peak.names.join(", ")}`}.
          </p>
        )}
      </div>
    </Card>
  );
}

// ---------- Visor de eventos agrupado ----------

const WEIGHT = { matter: { label: "Importa", cls: "border-bad/40 text-bad" }, watch: { label: "Vigilar", cls: "border-warn/40 text-warn" }, noise: { label: "Ruido", cls: "border-line text-mute" } };

export function EventDigestCard({ onNavigate }: { onNavigate: (p: PageId) => void }) {
  const [days, setDays] = useState(7);
  const [list, setList] = useState<EventGroup[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showNoise, setShowNoise] = useState(false);
  const load = useCallback(() => {
    setList(null);
    setError(null);
    expertApi.events(days).then(setList, (e) => setError(String(e)));
  }, [days]);
  useEffect(load, [load]);
  const shown = (list ?? []).filter((g) => showNoise || g.weight !== "noise");
  const noise = (list ?? []).filter((g) => g.weight === "noise");
  return (
    <Card
      title="El Visor de eventos, traducido"
      icon={<ScrollText size={14} />}
      right={
        <select value={days} onChange={(e) => setDays(Number(e.target.value))} className="rounded-md border border-line bg-void/60 px-2 py-1 text-xs text-ink" aria-label="Días">
          <option value={1}>Último día</option>
          <option value={7}>Últimos 7 días</option>
          <option value={30}>Últimos 30 días</option>
        </select>
      }
    >
      {error ? (
        <ErrorState message={error} onRetry={load} where="Visor de eventos" />
      ) : !list ? (
        <Loading text="Leyendo y agrupando los errores…" />
      ) : shown.length === 0 ? (
        <EmptyState title="Ningún error que importe">Solo hay avisos rutinarios de Windows.</EmptyState>
      ) : (
        <ul className="space-y-2">
          {shown.map((g) => (
            <li key={`${g.provider}-${g.id}`} className="rounded-lg border border-line bg-void/30 px-3 py-2">
              <div className="flex flex-wrap items-center gap-2">
                <span className={`rounded-full border px-2 py-px text-[10px] ${WEIGHT[g.weight].cls}`}>{WEIGHT[g.weight].label}</span>
                <b className="text-sm text-ink">{g.title}</b>
                <span className="ml-auto font-mono text-[11px] text-mute">
                  ×{g.count} · último {shortDate(g.last)}
                </span>
              </div>
              <p className="mt-1 text-xs text-dim">{g.meaning}</p>
              <p className="text-xs text-dim">
                <b className="font-medium text-ink">Qué hacer:</b> {g.todo}
                {g.page && (
                  <button onClick={() => onNavigate(g.page as PageId)} className="ml-2 text-neon hover:underline">
                    Ir
                  </button>
                )}
              </p>
              <p className="mt-0.5 truncate text-[11px] text-mute" title={g.sample}>
                {g.provider} · evento {g.id} · {g.sample}
              </p>
            </li>
          ))}
        </ul>
      )}
      {noise.length > 0 && (
        <button onClick={() => setShowNoise((v) => !v)} className="mt-2 text-[11px] text-mute hover:text-ink">
          {showNoise ? "Ocultar el ruido" : `Ver también ${noise.length} grupo(s) de ruido conocido (${noise.reduce((t, g) => t + g.count, 0)} eventos)`}
        </button>
      )}
    </Card>
  );
}

// ---------- Seguridad a fondo ----------

export function SecurityDeepCard({ isAdmin, onNavigate }: { isAdmin: boolean; onNavigate: (p: PageId) => void }) {
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [findings, setFindings] = useState<TroubleFinding[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [fixing, setFixing] = useState<string | null>(null);
  const load = useCallback(() => {
    setError(null);
    troubleshootApi.check("security").then((r) => setFindings(r.findings), (e) => setError(String(e)));
  }, []);
  useEffect(load, [load]);
  const run = async (f: TroubleFix) => {
    if (f.confirm && !(await confirm({ title: f.label, body: f.confirm, confirmLabel: "Continuar" }))) return;
    setFixing(f.id);
    try {
      const m = await troubleshootApi.fix(f.id);
      if (m) toast("ok", m);
    } catch (e) {
      toast("error", `${f.label}: ${e}`);
    } finally {
      setFixing(null);
      load();
    }
  };
  return (
    <Card title="Seguridad a fondo: lo que no se ve" icon={<ShieldAlert size={14} />} right={<button className={smallBtn} onClick={load}>Volver a mirar</button>}>
      <p className="mb-3 text-xs text-dim">Lo que usan los atacantes y casi nadie mira: exclusiones de Defender, SMBv1, Escritorio remoto, cuentas de administrador, contraseñas guardadas en claro, UAC y macros de Office.</p>
      {error ? <ErrorState message={error} onRetry={load} where="Seguridad a fondo" /> : !findings ? <Loading text="Revisando…" /> : <FindingList findings={findings} isAdmin={isAdmin} fixing={fixing} onFix={run} onNavigate={onNavigate} />}
      {dialog}
    </Card>
  );
}
