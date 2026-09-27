import { Brush, ClipboardCheck, FileText, Gauge, HardDrive, LifeBuoy, Loader2, RefreshCw, RotateCcw, Star, Stethoscope, Wifi } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useToast } from "../components/feedback";
import type { PageId } from "../components/Sidebar";
import { Bar, Sparkline } from "../components/ui";
import { useLiveMetrics } from "../hooks/useLiveMetrics";
import { tempColor, useSensors } from "../hooks/useSensors";
import { api, diagApi, toolboxApi, tweaksApi, type FindingAction, type JournalEntry, type SystemInfo } from "../lib/api";
import { getPrefs } from "../lib/prefs";
import { bytes, duration, loadColor, rate } from "../lib/format";

type Latest = Awaited<ReturnType<typeof diagApi.latest>>;

const DOT = { bad: "bg-bad", warn: "bg-warn", info: "bg-mute", ok: "bg-ok" };

function ago(ts: number) {
  const m = Math.round((Date.now() / 1000 - ts) / 60);
  if (m < 60) return `hace ${Math.max(1, m)} min`;
  const h = Math.round(m / 60);
  if (h < 48) return `hace ${h} h`;
  return `hace ${Math.round(h / 24)} días`;
}

function Section({ title, action, children, className = "" }: { title: string; action?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={`flex min-w-0 flex-col gap-2 ${className}`}>
      <div className="flex items-baseline justify-between gap-3">
        <h2 className="text-[15px] font-semibold">{title}</h2>
        {action}
      </div>
      <div className="border-t border-line">{children}</div>
    </section>
  );
}

function LinkButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button onClick={onClick} className="shrink-0 text-[13px] whitespace-nowrap text-neon hover:underline">
      {children}
    </button>
  );
}

function Kpi({ label, value, unit, sub, children }: { label: string; value: string; unit?: string; sub?: ReactNode; children?: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-2 px-5 py-4">
      <div className="text-[13px] text-dim">{label}</div>
      <div className="flex items-baseline gap-1.5">
        <span className="text-[26px] leading-none font-semibold tracking-tight tabular">{value}</span>
        {unit && <span className="truncate text-[13px] text-dim">{unit}</span>}
      </div>
      {children}
      {sub && <div className="truncate text-xs text-mute">{sub}</div>}
    </div>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex gap-4 border-b border-line py-2 text-[13px]">
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
  const toast = useToast();

  useEffect(() => {
    api.systemInfo().then(setInfo).catch(() => {});
    diagApi
      .latest()
      .then(setLatest)
      .catch(() => {})
      .finally(() => setLoaded(true));
    tweaksApi.journal().then((j) => setJournal([...j].sort((a, b) => b.timestamp - a.timestamp).slice(0, 5)));
  }, []);

  if (error && !m) return <p className="p-8 text-bad">Error leyendo métricas: {error}</p>;
  if (!m) return <p className="p-8 text-sm text-mute">Leyendo el equipo…</p>;

  const act = (a: FindingAction) => (a.kind === "tool" ? diagApi.openTool(a.tool) : onNavigate(a.page as PageId, a.focus));

  const quick = async (key: string, label: string, run: () => Promise<string | void>) => {
    setBusy(key);
    try {
      const msg = await run();
      toast("ok", msg ? `${label}: ${msg}` : `${label}: hecho.`);
      tweaksApi.journal().then((j) => setJournal([...j].sort((a, b) => b.timestamp - a.timestamp).slice(0, 5)));
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
  const attention = findings.filter((f) => f.severity !== "info").slice(0, 6);
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

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-7 px-8 py-6">
      {/* Veredicto */}
      <section className="flex items-center gap-5 rounded-xl border border-line bg-panel px-6 py-5">
        <span className={`size-3 shrink-0 rounded-full ${verdict.dot}`} />
        <div className="min-w-0 flex-1">
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
        <button onClick={() => onNavigate("session")} className="flex h-9 items-center gap-1.5 rounded-lg border border-line-2 px-3.5 text-[13px] text-ink hover:bg-panel-2">
          <ClipboardCheck size={15} strokeWidth={1.6} /> Sesión de servicio
        </button>
        <button onClick={() => onNavigate("report")} className="flex h-9 items-center gap-1.5 rounded-lg border border-line-2 px-3.5 text-[13px] text-ink hover:bg-panel-2">
          <FileText size={15} strokeWidth={1.6} /> Informe
        </button>
        <button onClick={() => onNavigate("diagnostics")} className="flex h-9 items-center gap-1.5 rounded-lg bg-neon px-4 text-[13px] font-medium text-on-neon hover:brightness-110">
          <Stethoscope size={15} strokeWidth={1.8} /> {latest ? "Volver a diagnosticar" : "Diagnosticar"}
        </button>
      </section>

      {/* En vivo */}
      <section className="grid grid-cols-4 divide-x divide-line rounded-xl border border-line bg-panel">
        <Kpi
          label="Procesador"
          value={`${Math.round(m.cpuTotal)}`}
          unit="%"
          sub={
            <>
              {cpuTemp != null ? <span style={{ color: tempColor(cpuTemp) }}>{cpuTemp.toFixed(0)} °C</span> : "Temperatura: requiere administrador"}
              {gpuTemp != null && <span style={{ color: tempColor(gpuTemp) }}> · GPU {gpuTemp.toFixed(0)} °C</span>}
            </>
          }
        >
          <Sparkline data={history.cpu} max={100} color={loadColor(m.cpuTotal)} height={26} />
        </Kpi>
        <Kpi label="Memoria" value={bytes(m.memoryUsed).replace(/ GB$/, "")} unit={`de ${bytes(m.memoryTotal)}`} sub={`${Math.round(ramPct)} % en uso · ${m.processCount} procesos`}>
          <Sparkline data={history.ram} max={100} color={loadColor(ramPct)} height={26} />
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
          <Sparkline data={history.rx} max={Math.max(...history.rx, ...history.tx, 1)} color="var(--color-dim)" height={26} />
        </Kpi>
      </section>

      <div className="grid grid-cols-3 gap-8">
        <Section
          className="col-span-2"
          title="Requiere atención"
          action={latest && <LinkButton onClick={() => onNavigate("diagnostics")}>Ver diagnóstico</LinkButton>}
        >
          {!loaded ? null : !latest ? (
            <p className="py-5 text-sm text-dim">Haz un diagnóstico para ver aquí lo que hay que resolver en este equipo.</p>
          ) : attention.length === 0 ? (
            <p className="flex items-center gap-2.5 py-5 text-sm text-dim">
              <span className="size-2 rounded-full bg-ok" /> Nada pendiente en el último diagnóstico.
            </p>
          ) : (
            attention.map((f, i) => (
              <div key={i} className="flex items-center gap-3.5 border-b border-line py-3">
                <span className={`size-2 shrink-0 rounded-full ${DOT[f.severity]}`} />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm">{f.title}</div>
                  {f.detail && <div className="truncate text-xs text-mute">{f.detail}</div>}
                </div>
                {f.actions[0] && <LinkButton onClick={() => act(f.actions[0])}>{f.actions[0].label}</LinkButton>}
              </div>
            ))
          )}
        </Section>

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
      </div>

      <div className="grid grid-cols-3 gap-8">
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
      </div>

      <Section title="Procesos con más carga" action={<LinkButton onClick={() => onNavigate("processes")}>Ver todos</LinkButton>}>
        <div className="grid grid-cols-2 gap-x-10">
          {m.topProcesses.slice(0, 8).map((p) => (
            <div key={p.pid} className="flex items-center gap-3 border-b border-line py-2 text-[13px]">
              <span className="min-w-0 flex-1 truncate">{p.name}</span>
              <span className="w-14 text-right text-dim tabular" style={p.cpu >= 50 ? { color: loadColor(p.cpu) } : undefined}>
                {p.cpu.toFixed(1)} %
              </span>
              <span className="w-16 text-right text-dim tabular">{bytes(p.memory)}</span>
            </div>
          ))}
        </div>
      </Section>

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
      .catch(() => setItems([]));
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
