import { Brush, ClipboardCheck, FileText, Gauge, HardDrive, LifeBuoy, Loader2, RefreshCw, RotateCcw, Star, Stethoscope, Wifi } from "lucide-react";
import { PlaceNotes } from "../components/PlaceNotes";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { ActionPlan } from "../components/ActionPlan";
import { useToast } from "../components/feedback";
import type { PageId } from "../components/Sidebar";
import { Bar, Button, Card, Sparkline, Loading } from "../components/ui";
import { useLiveMetrics } from "../hooks/useLiveMetrics";
import { tempColor, useSensors } from "../hooks/useSensors";
import { logQuietly, api, diagApi, toolboxApi, tweaksApi, type JournalEntry, type SystemInfo } from "../lib/api";
import { getPrefs, usePrefs } from "../lib/prefs";
import { UserHome } from "../components/UserHome";
import { summarize as summarizeForText, summaryText, type UserInput } from "../lib/userHome";
import { bytes, duration, loadColor, rate, ago } from "../lib/format";
import { FirstSteps } from "../components/FirstSteps";
import { PanelGrid } from "../components/PanelGrid";
import { Slowdown } from "../components/Slowdown";
import type { PanelBlock } from "../lib/panelLayout";
import { TodayCard } from "../components/TodayCard";
import { DIAG_COUNT_CHANGED, DIAG_UPDATED } from "../lib/diagRun";
import { trend } from "../lib/trend";

type Latest = Awaited<ReturnType<typeof diagApi.latest>>;

const DOT = { bad: "bg-bad", warn: "bg-warn", info: "bg-mute", ok: "bg-ok" };
/** Color del resplandor según el veredicto. */
const AURA: Record<string, string> = { "bg-ok": "var(--color-ok)", "bg-warn": "var(--color-warn)", "bg-bad": "var(--color-bad)" };

function Section({ title, action, children, className = "" }: { title: string; action?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <Card title={title} right={action} className={`min-w-0 ${className}`}>
      {children}
    </Card>
  );
}

function LinkButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button onClick={onClick} className="shrink-0 text-xs whitespace-nowrap text-neon hover:underline">
      {children}
    </button>
  );
}

function Kpi({ label, value, unit, sub, children }: { label: string; value: string; unit?: string; sub?: ReactNode; children?: ReactNode }) {
  return (
    <Card className="flex min-w-0 flex-col gap-2">
      <div className="text-[13px] text-dim">{label}</div>
      <div className="flex items-baseline gap-1.5">
        <span className="text-[30px] leading-none font-semibold tracking-tight tabular">{value}</span>
        {unit && <span className="truncate text-[13px] text-dim">{unit}</span>}
      </div>
      {children}
      {sub && <div className="truncate text-xs text-mute">{sub}</div>}
    </Card>
  );
}

/** Flecha con la tendencia de los últimos minutos. `worse`: hacia dónde es malo (más carga es peor). */
function Trend({ data, min, unit, worse = "up" }: { data: number[]; min: number; unit: string; worse?: "up" | "down" | null }) {
  const t = trend(data, min);
  if (t.dir === "flat") return <span className="text-mute">estable</span>;
  const bad = worse === t.dir;
  return (
    <span className={worse === null ? "text-dim" : bad ? "text-warn" : "text-ok"}>
      {t.dir === "up" ? "▲" : "▼"} {t.dir === "up" ? "sube" : "baja"} {Math.round(Math.abs(t.delta))} {unit}
    </span>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex gap-4 border-b border-line py-2 text-[13px] last:border-0">
      <span className="w-24 shrink-0 text-mute">{label}</span>
      <span className="min-w-0 flex-1 truncate text-ink">{children}</span>
    </div>
  );
}

export function Dashboard({ onNavigate }: { onNavigate: (page: PageId, focus?: string | null) => void }) {
  const { metrics: m, history, error } = useLiveMetrics(getPrefs().refreshMs);
  const { sensors } = useSensors(5000);
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [latest, setLatest] = useState<Latest>(null);
  const [loaded, setLoaded] = useState(false);
  const [journal, setJournal] = useState<JournalEntry[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [reviewing, setReviewing] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const toast = useToast();
  const prefs = usePrefs();
  const userMode = prefs.mode === "user";
  const navigate = useCallback((p: PageId, f?: string | null) => onNavigate(p, f), [onNavigate]);

  const loadLatest = useCallback(
    () =>
      diagApi
        .latest()
        .then(setLatest)
        .catch(logQuietly("Dashboard"))
        .finally(() => setLoaded(true)),
    [],
  );

  // Revisión completa: diagnóstico (discos, estabilidad, drivers, seguridad y
  // actualizaciones) y la lista de «Qué hacer ahora» al día.
  const review = async () => {
    setReviewing(true);
    try {
      // Revisión completa: se vuelve a leer todo, sin reutilizar lo de hace unos minutos.
      const d = await diagApi.run(true);
      const n = d.findings.filter((f) => f.severity !== "info").length;
      toast(n ? "info" : "ok", n ? `Revisión terminada: ${n} cosa(s) que atender.` : "Revisión terminada: todo en orden.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setReviewing(false);
      await loadLatest();
      setRefresh((r) => r + 1);
    }
  };

  useEffect(() => {
    api.systemInfo().then(setInfo).catch(logQuietly("Dashboard"));
    void loadLatest();
    void tweaksApi
        .journal()
        .then((j) => setJournal([...j].sort((a, b) => b.timestamp - a.timestamp).slice(0, 5)))
        .catch(logQuietly("Dashboard"));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- solo al abrir el Panel; después se recarga desde los botones
  }, []);
  // Un análisis nuevo o completado, o un hallazgo aceptado en Diagnóstico: el Panel se pone al día.
  useEffect(() => {
    const reload = () => void loadLatest();
    window.addEventListener(DIAG_COUNT_CHANGED, reload);
    window.addEventListener(DIAG_UPDATED, reload);
    return () => {
      window.removeEventListener(DIAG_COUNT_CHANGED, reload);
      window.removeEventListener(DIAG_UPDATED, reload);
    };
  }, [loadLatest]);

  if (error && !m) return <p className="p-8 text-bad">Error leyendo métricas: {error}</p>;
  if (!m) return <Loading page text="Leyendo el equipo…" />;


  const quick = async (key: string, label: string, run: () => Promise<string | void>) => {
    setBusy(key);
    try {
      const msg = await run();
      toast("ok", msg ? `${label}: ${msg}` : `${label}: hecho.`);
      void tweaksApi
        .journal()
        .then((j) => setJournal([...j].sort((a, b) => b.timestamp - a.timestamp).slice(0, 5)))
        .catch(logQuietly("Dashboard"));
    } catch (e) {
      toast("error", `${label}: ${e}`);
    } finally {
      setBusy(null);
    }
  };
  const runTweak = (id: string) => () => tweaksApi.run(id).then((r) => r.message);

  const findings = latest?.findings ?? [];
  const bad = findings.filter((f) => f.severity === "bad").length;
  const warn = findings.filter((f) => f.severity === "warn").length;
  const ramPct = (m.memoryUsed / m.memoryTotal) * 100;
  const volumes = m.disks.filter((d) => d.total > 0).sort((a, b) => a.mount.localeCompare(b.mount));
  const system = volumes.find((d) => d.mount.toUpperCase().startsWith("C:")) ?? volumes[0];
  const cpuTemp = sensors?.cpuTemp;
  const gpuTemp = sensors?.gpus.find((g) => g.temperature != null)?.temperature ?? null;

  const verdict = !latest
    ? { dot: "bg-mute", text: "Este equipo aún no se ha analizado", sub: "Un diagnóstico revisa discos, estabilidad, drivers, seguridad y actualizaciones en menos de un minuto." }
    : bad + warn === 0
      ? { dot: "bg-ok", text: "Todo en orden", sub: `Diagnóstico ${ago(latest.timestamp)} sin problemas.` }
      : {
          dot: bad ? "bg-bad" : "bg-warn",
          text: [bad && `${bad} ${bad === 1 ? "problema" : "problemas"}`, warn && `${warn} ${warn === 1 ? "aviso" : "avisos"}`].filter(Boolean).join(" y "),
          sub: `Según el diagnóstico ${ago(latest.timestamp)}.`,
        };

  const quickActions: { key: string; label: string; icon: typeof Brush; run: () => void }[] = [
    { key: "temp", label: "Limpiar temporales", icon: Brush, run: () => quick("temp", "Temporales", runTweak("cleanup.user-temp")) },
    { key: "dns", label: "Vaciar caché DNS", icon: Wifi, run: () => quick("dns", "Caché DNS", runTweak("cleanup.dns-cache")) },
    { key: "explorer", label: "Reiniciar Explorador", icon: RefreshCw, run: () => quick("explorer", "Explorador", runTweak("repair.explorer")) },
    { key: "rp", label: "Crear punto de restauración", icon: RotateCcw, run: () => quick("rp", "Punto de restauración", () => tweaksApi.createRestorePoint()) },
    { key: "speed", label: "Test de velocidad", icon: Gauge, run: () => onNavigate("network") },
    { key: "space", label: "Ver qué ocupa el disco", icon: HardDrive, run: () => onNavigate("space") },
    { key: "sfc", label: "Reparar archivos de Windows", icon: LifeBuoy, run: () => onNavigate("repair", "repair.sfc") },
  ];

  // Cada tarjeta del Panel por su nombre: el orden y cuáles se ven lo decide el técnico (PanelGrid).
  const blocks: Record<PanelBlock, ReactNode> = {
    // Lo pendiente del técnico: casos, seguimientos, visitas y avisos (no en modo usuario).
    today: (
      <>
        <FirstSteps />
        <TodayCard />
      </>
    ),
    // Lo que se apuntó de este equipo o de esta red la última vez.
    notes: <PlaceNotes compact />,
    live: (
      <section className="grid grid-cols-2 gap-3 @3xl:grid-cols-4">
        <Kpi
          label="Procesador"
          value={`${Math.round(m.cpuTotal)}`}
          unit="%"
          sub={
            <>
              {cpuTemp != null ? <span style={{ color: tempColor(cpuTemp) }}>{cpuTemp.toFixed(0)} °C</span> : "Temperatura: requiere administrador"}
              {gpuTemp != null && <span style={{ color: tempColor(gpuTemp) }}> · GPU {gpuTemp.toFixed(0)} °C</span>}
              {" · "}
              <Trend data={history.cpu} min={8} unit="pts" />
            </>
          }
        >
          <Sparkline data={history.cpu} max={100} color={loadColor(m.cpuTotal)} height={32} />
        </Kpi>
        <Kpi label="Memoria" value={bytes(m.memoryUsed).replace(/ GB$/, "")} unit={`de ${bytes(m.memoryTotal)}`} sub={
          <>
            {Math.round(ramPct)} % en uso · <Trend data={history.ram} min={3} unit="pts" />
          </>
        }>
          <Sparkline data={history.ram} max={100} color={loadColor(ramPct)} height={32} />
        </Kpi>
        <Kpi
          label={`Disco del sistema${system ? ` (${system.mount.replace(/\\$/, "")})` : ""}`}
          value={system ? bytes(system.available).replace(/ GB$/, "") : "—"}
          unit="GB libres"
          sub={system ? `de ${bytes(system.total)}` : undefined}
        >
          <div className="py-2.5">
            <Bar value={system ? ((system.total - system.available) / system.total) * 100 : 0} />
          </div>
        </Kpi>
        <Kpi label="Red" value={rate(m.netRxPerSec)} sub={<>Subida {rate(m.netTxPerSec)} · encendido hace {duration(m.uptime)}</>}>
          <Sparkline data={history.rx} max={Math.max(...history.rx, ...history.tx, 1)} color="var(--color-dim)" height={32} />
        </Kpi>
      </section>
    ),
    plan: (
        <Section
          title="Qué hacer ahora"
          action={latest && <LinkButton onClick={() => onNavigate("diagnostics")}>Ver diagnóstico</LinkButton>}
        >
          {!loaded ? null : (
            <>
              {!latest && <p className="border-b border-line py-3 text-sm text-dim">Pulsa «Revisar el equipo» para completar esta lista con discos, drivers, seguridad y estabilidad.</p>}
              <ActionPlan
                findings={latest?.findings ?? []}
                diagnosedAt={latest?.timestamp ?? null}
                systemDisk={system ? { free: system.available, total: system.total, mount: system.mount } : null}
                refresh={refresh}
                onNavigate={navigate}
              />
            </>
          )}
        </Section>

    ),
    quick: (
        <Section title="Acciones rápidas" action={<LinkButton onClick={() => onNavigate("tools")}>Herramientas</LinkButton>}>
          <div className="flex flex-col py-1">
            {quickActions.map((q) => (
              <button
                key={q.key}
                onClick={q.run}
                disabled={busy !== null}
                className="-mx-2 flex h-9 items-center gap-3 rounded-lg px-2 text-left text-[13px] text-ink transition-colors hover:bg-panel-2 disabled:opacity-50"
              >
                {busy === q.key ? <Loader2 size={16} className="animate-spin text-neon" /> : <q.icon size={16} strokeWidth={1.6} className="text-mute" />}
                {q.label}
              </button>
            ))}
          </div>
          <FavoriteTools onNavigate={onNavigate} />
        </Section>
    ),
    slow: <Slowdown metrics={m} onProcesses={() => onNavigate("processes")} />,
    machine: (
        <Section title="Este equipo" action={<LinkButton onClick={() => onNavigate("hardware")}>Hardware</LinkButton>}>
          <Row label="Modelo">{latest?.model ?? info?.hostName ?? "—"}</Row>
          <Row label="Procesador">{info?.cpuBrand ?? "—"}</Row>
          <Row label="Memoria">{bytes(m.memoryTotal)}</Row>
          <Row label="Gráfica">{latest?.gpus.join(" + ") || "—"}</Row>
          <Row label="Windows">
            {latest?.windows ?? info?.osName ?? "—"}
            {latest?.activated === false && <span className="text-warn"> · sin activar</span>}
          </Row>
        </Section>

    ),
    disks: (
        <Section title="Discos" action={<LinkButton onClick={() => onNavigate("space")}>Espacio</LinkButton>}>
          {latest && latest.disks.length > 0
            ? latest.disks.map((d) => (
                <div key={d.name} className="flex items-start gap-3 border-b border-line py-2">
                  <span className={`mt-1.5 size-2 shrink-0 rounded-full ${DOT[d.status]}`} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px]">{d.name}</div>
                    <div className={`truncate text-xs ${d.status === "ok" ? "text-mute" : d.status === "bad" ? "text-bad" : "text-warn"}`}>
                      {d.kind} · {bytes(d.size)} · {d.detail}
                    </div>
                  </div>
                </div>
              ))
            : volumes.map((d) => (
                <div key={d.mount} className="flex flex-col gap-1.5 border-b border-line py-2.5">
                  <div className="flex justify-between text-[13px]">
                    <span>{d.mount}</span>
                    <span className="text-dim tabular">{bytes(d.available)} libres</span>
                  </div>
                  <Bar value={((d.total - d.available) / d.total) * 100} />
                </div>
              ))}
          {latest && <p className="pt-2 text-xs text-mute">Salud según el diagnóstico {ago(latest.timestamp)}.</p>}
        </Section>

    ),
    recent: (
        <Section title="Actividad reciente" action={<LinkButton onClick={() => onNavigate("history")}>Historial</LinkButton>}>
          {journal.length === 0 ? (
            <p className="py-4 text-sm text-mute">Todavía no se ha cambiado nada en este equipo.</p>
          ) : (
            journal.map((j) => (
              <div key={j.id} className="flex items-start gap-3 border-b border-line py-2">
                <span className={`mt-1.5 size-2 shrink-0 rounded-full ${j.ok ? "bg-ok" : "bg-bad"}`} />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px]">{j.title}</div>
                  <div className="text-xs text-mute">{ago(j.timestamp)}</div>
                </div>
              </div>
            ))
          )}
        </Section>
    ),
  };

  const userInput: UserInput = {
    ramPct,
    cpuPct: m.cpuTotal,
    securityScore: latest?.securityScore ?? null,
    securityIssues: findings.filter((f) => f.severity !== "info" && /segur|antivirus|firewall|cortafuegos|defender|bitlocker|actualiz/i.test(`${f.area} ${f.title}`)).map((f) => f.title),
    freePct: system && system.total > 0 ? (system.available / system.total) * 100 : null,
    disks: (latest?.disks ?? []).filter((k) => k.status !== "ok").map((k): [string, "ok" | "warn" | "bad"] => [k.name, k.status]),
    startupCount: null,
    analyzed: !!latest,
  };
  const userFix = () =>
    quick("fix", "Arreglar", async () => {
      const r = await tweaksApi.run("cleanup.user-temp");
      void loadLatest();
      return r.message;
    });
  const tellTechnician = () =>
    void navigator.clipboard.writeText(summaryText(summarizeForText(userInput), info?.hostName ?? "mi equipo")).then(
      () => toast("ok", "Resumen copiado: pégalo en un mensaje o un correo a tu técnico."),
      () => toast("error", "No se pudo copiar."),
    );

  return (
    <div className="mx-auto flex max-w-(--page-max) flex-col gap-5 px-8 py-6">
      {userMode && <UserHome input={userInput} busy={busy === "fix" ? "fix" : reviewing ? "review" : null} onFix={userFix} onTellTechnician={tellTechnician} onReview={() => void review()} />}
      {/* Veredicto */}
      <Card className="relative isolate flex flex-wrap items-center gap-x-5 gap-y-3 overflow-hidden px-6 py-5">
        {prefs.aura && <span aria-hidden className="aura" style={{ background: `radial-gradient(ellipse 60% 140% at 4% 0%, color-mix(in srgb, ${AURA[verdict.dot] ?? "var(--color-mute)"} 24%, transparent), transparent 70%)` }} />}
        <span className={`size-3 shrink-0 rounded-full ${verdict.dot}`} />
        <div className="min-w-60 flex-1">
          <div className="text-xl font-semibold tracking-tight">{verdict.text}</div>
          <div className="mt-0.5 truncate text-[13px] text-dim">
            {verdict.sub}
            {latest?.securityScore != null && (
              <>
                {" "}
                Seguridad{" "}
                <button onClick={() => onNavigate("security")} className="font-medium text-ink hover:underline">
                  {latest.securityScore}/100
                </button>
                .
              </>
            )}
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button kind="secondary" onClick={() => onNavigate("session")}>
            <ClipboardCheck size={15} strokeWidth={1.6} /> Sesión de servicio
          </Button>
          <Button kind="secondary" onClick={() => onNavigate("report")}>
            <FileText size={15} strokeWidth={1.6} /> Informe
          </Button>
          <Button
            onClick={() => void review()}
            disabled={reviewing}
            title="Diagnóstico completo (discos, estabilidad, drivers, seguridad y actualizaciones) y plan de acción. Tarda alrededor de un minuto."
          >
            {reviewing ? <Loader2 size={15} className="animate-spin" /> : <Stethoscope size={15} strokeWidth={1.8} />}
            {reviewing ? "Revisando…" : latest ? "Revisión completa" : "Revisar el equipo"}
          </Button>
        </div>
      </Card>

      <PanelGrid blocks={blocks} />
    </div>
  );
}

/** Herramientas marcadas con estrella en Herramientas, a un clic desde el Panel. */
function FavoriteTools({ onNavigate }: { onNavigate: (p: PageId) => void }) {
  const [items, setItems] = useState<{ id: string; name: string; confirm: boolean }[] | null>(null);
  const toast = useToast();
  useEffect(() => {
    toolboxApi
      .list()
      .then((v) => {
        const names = new Map<string, { name: string; confirm: boolean }>([
          ...v.tools.filter((t) => !t.unavailable).map((t) => [t.id, { name: t.name, confirm: !!t.confirm }] as const),
          ...v.custom.map((c) => [c.id, { name: c.name, confirm: false }] as const),
        ]);
        setItems(v.favorites.flatMap((id) => (names.has(id) ? [{ id, ...names.get(id)! }] : [])));
      })
      .catch((e) => {
        setItems([]);
        logQuietly("Dashboard")(e);
      });
  }, []);
  if (items === null) return null;
  return (
    <div className="border-t border-line/60 py-1">
      {items.length === 0 ? (
        <button onClick={() => onNavigate("tools")} className="py-2 text-left text-xs text-mute hover:text-ink">
          Marca herramientas con ★ en Herramientas para tenerlas aquí.
        </button>
      ) : (
        items.slice(0, 8).map((t) => (
          <button
            key={t.id}
            // Las que piden confirmación (reinicios) se abren en Herramientas, con su aviso.
            onClick={() => (t.confirm ? onNavigate("tools") : toolboxApi.launch(t.id).catch((e) => toast("error", String(e))))}
            className="-mx-2 flex h-9 w-full items-center gap-3 rounded-lg px-2 text-left text-[13px] text-ink transition-colors hover:bg-panel-2"
          >
            <Star size={16} strokeWidth={1.6} className="text-mute" />
            <span className="truncate">{t.name}</span>
          </button>
        ))
      )}
    </div>
  );
}
