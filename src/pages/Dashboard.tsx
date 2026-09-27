import { ArrowDown, ArrowUp } from "lucide-react";
import { useEffect, useState } from "react";
import type { PageId } from "../components/Sidebar";
import { Bar, Sparkline } from "../components/ui";
import { useLiveMetrics } from "../hooks/useLiveMetrics";
import { tempColor, useSensors } from "../hooks/useSensors";
import { api, diagApi, type Finding, type FindingAction, type SystemInfo } from "../lib/api";
import { bytes, duration, loadColor, rate } from "../lib/format";

const SEVERITY_DOT = { bad: "bg-bad", warn: "bg-warn", info: "bg-mute" };

function ago(ts: number) {
  const m = Math.round((Date.now() / 1000 - ts) / 60);
  if (m < 60) return `hace ${Math.max(1, m)} min`;
  const h = Math.round(m / 60);
  if (h < 48) return `hace ${h} h`;
  return `hace ${Math.round(h / 24)} días`;
}

/** Una cifra clave de la banda superior. */
function Kpi({ label, value, unit, sub, children }: { label: string; value: string; unit?: string; sub?: React.ReactNode; children?: React.ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-2 px-5 py-4">
      <div className="text-[13px] text-dim">{label}</div>
      <div className="flex items-baseline gap-1.5">
        <span className="text-[28px] leading-none font-semibold tracking-tight tabular">{value}</span>
        {unit && <span className="truncate text-[13px] text-dim">{unit}</span>}
      </div>
      {children}
      {sub && <div className="truncate text-xs text-mute">{sub}</div>}
    </div>
  );
}

export function Dashboard({ onNavigate }: { onNavigate: (page: PageId, focus?: string | null) => void }) {
  const { metrics: m, history, error } = useLiveMetrics(2000);
  const { sensors } = useSensors(5000);
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [latest, setLatest] = useState<Awaited<ReturnType<typeof diagApi.latest>>>(null);
  const [latestLoaded, setLatestLoaded] = useState(false);

  useEffect(() => {
    api.systemInfo().then(setInfo).catch(() => {});
    diagApi
      .latest()
      .then(setLatest)
      .catch(() => {})
      .finally(() => setLatestLoaded(true));
  }, []);

  if (error && !m) return <p className="p-8 text-bad">Error leyendo métricas: {error}</p>;
  if (!m) return <p className="p-8 text-sm text-mute">Leyendo el equipo…</p>;

  const ramPct = (m.memoryUsed / m.memoryTotal) * 100;
  const disks = m.disks.filter((d) => d.total > 0).sort((a, b) => a.mount.localeCompare(b.mount));
  const system = disks.find((d) => d.mount.toUpperCase().startsWith("C:")) ?? disks[0];
  const sysUsed = system ? ((system.total - system.available) / system.total) * 100 : 0;
  const cpuTemp = sensors?.cpuTemp;
  const gpu = sensors?.gpus.find((g) => g.temperature != null);

  const act = (a: FindingAction) => {
    if (a.kind === "tool") diagApi.openTool(a.tool);
    else onNavigate(a.page as PageId, a.focus);
  };
  const attention = (latest?.findings ?? []).filter((f) => f.severity !== "info").slice(0, 6);

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-7 px-8 py-6">
      <div className="flex items-center gap-4">
        <p className="min-w-0 flex-1 truncate text-[13px] text-dim">
          {info ? `${info.hostName} · ${info.osName} · encendido hace ${duration(m.uptime)} · ${m.processCount} procesos` : "…"}
        </p>
        <button onClick={() => onNavigate("report")} className="h-9 rounded-lg border border-line-2 px-3.5 text-[13px] text-ink transition-colors hover:bg-panel-2">
          Informe PDF
        </button>
        <button onClick={() => onNavigate("diagnostics")} className="h-9 rounded-lg bg-neon px-4 text-[13px] font-medium text-on-neon transition-[filter] hover:brightness-110">
          Diagnosticar equipo
        </button>
      </div>

      {/* Cifras clave */}
      <section className="grid grid-cols-4 divide-x divide-line rounded-xl border border-line bg-panel">
        <Kpi
          label="Procesador"
          value={`${Math.round(m.cpuTotal)}`}
          unit="%"
          sub={
            <>
              {info?.cpuBrand ?? "…"}
              {cpuTemp != null && (
                <span style={{ color: tempColor(cpuTemp) }}> · {cpuTemp.toFixed(0)} °C</span>
              )}
              {cpuTemp == null && gpu && <span style={{ color: tempColor(gpu.temperature) }}> · GPU {gpu.temperature!.toFixed(0)} °C</span>}
            </>
          }
        >
          <Sparkline data={history.cpu} max={100} color={loadColor(m.cpuTotal)} height={28} />
        </Kpi>
        <Kpi label="Memoria" value={bytes(m.memoryUsed).replace(/ GB$/, "")} unit={`de ${bytes(m.memoryTotal)}`} sub={`${Math.round(ramPct)} % en uso`}>
          <Bar value={ramPct} />
        </Kpi>
        <Kpi
          label="Disco del sistema"
          value={system ? bytes(system.available).replace(/ GB$/, "") : "—"}
          unit={system ? "GB libres" : undefined}
          sub={system ? `${system.mount} · ${bytes(system.total)} · ${system.kind === "SSD" ? "SSD" : system.kind === "HDD" ? "HDD" : "disco"}` : undefined}
        >
          <Bar value={sysUsed} />
        </Kpi>
        <Kpi label="Red" value={rate(m.netRxPerSec)} unit="bajada" sub={`Subida ${rate(m.netTxPerSec)}`}>
          <Sparkline data={history.rx} max={Math.max(...history.rx, ...history.tx, 1)} color="var(--color-dim)" height={28} />
        </Kpi>
      </section>

      <div className="grid grid-cols-5 gap-8">
        {/* Requiere atención */}
        <section className="col-span-3 flex flex-col gap-2">
          <div className="flex items-baseline justify-between">
            <h2 className="text-[15px] font-semibold">Requiere atención</h2>
            {latest && (
              <button onClick={() => onNavigate("diagnostics")} className="text-[13px] text-neon hover:underline">
                Diagnóstico de {ago(latest.timestamp)}
              </button>
            )}
          </div>
          <div className="border-t border-line">
            {!latestLoaded ? null : !latest ? (
              <div className="py-6 text-sm text-dim">
                Aún no hay ningún diagnóstico de este equipo.{" "}
                <button onClick={() => onNavigate("diagnostics")} className="text-neon hover:underline">
                  Hacer el primero
                </button>
              </div>
            ) : attention.length === 0 ? (
              <div className="flex items-center gap-3 py-5 text-sm text-dim">
                <span className="size-2 rounded-full bg-ok" /> El último diagnóstico no encontró nada que requiera atención.
              </div>
            ) : (
              attention.map((f: Finding, i) => (
                <div key={i} className="flex items-center gap-3.5 border-b border-line py-3">
                  <span className={`size-2 shrink-0 rounded-full ${SEVERITY_DOT[f.severity]}`} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm text-ink">{f.title}</div>
                    {f.detail && <div className="truncate text-xs text-mute">{f.detail}</div>}
                  </div>
                  {f.actions[0] && (
                    <button onClick={() => act(f.actions[0])} className="shrink-0 text-[13px] whitespace-nowrap text-neon hover:underline">
                      {f.actions[0].label}
                    </button>
                  )}
                </div>
              ))
            )}
          </div>
        </section>

        {/* Procesos */}
        <section className="col-span-2 flex flex-col gap-2">
          <div className="flex items-baseline justify-between">
            <h2 className="text-[15px] font-semibold">Procesos con más carga</h2>
            <button onClick={() => onNavigate("processes")} className="text-[13px] text-neon hover:underline">
              Ver todos
            </button>
          </div>
          <div className="border-t border-line">
            {m.topProcesses.slice(0, 7).map((p) => (
              <div key={p.pid} className="flex items-center gap-3 border-b border-line py-2 text-[13px]">
                <span className="min-w-0 flex-1 truncate">{p.name}</span>
                <span className="w-14 text-right text-dim tabular" style={p.cpu >= 50 ? { color: loadColor(p.cpu) } : undefined}>
                  {p.cpu.toFixed(1)} %
                </span>
                <span className="w-16 text-right text-dim tabular">{bytes(p.memory)}</span>
              </div>
            ))}
          </div>
        </section>
      </div>

      {/* Almacenamiento */}
      <section className="flex flex-col gap-2">
        <h2 className="text-[15px] font-semibold">Almacenamiento</h2>
        <div className="grid grid-cols-2 gap-x-10 border-t border-line pt-1">
          {disks.map((d) => {
            const p = ((d.total - d.available) / d.total) * 100;
            return (
              <div key={d.mount} className="flex flex-col gap-1.5 border-b border-line py-3">
                <div className="flex items-baseline justify-between gap-3 text-[13px]">
                  <span className="min-w-0 truncate">
                    <span className="font-medium text-ink">{d.mount}</span>
                    <span className="text-mute">
                      {" "}
                      {d.name || "Disco local"} · {d.kind === "SSD" ? "SSD" : d.kind === "HDD" ? "HDD" : "disco"}
                      {d.removable ? " · USB" : ""}
                    </span>
                  </span>
                  <span className="shrink-0 text-dim tabular">
                    <span className={p >= 90 ? "text-bad" : "text-ink"}>{bytes(d.available)}</span> libres de {bytes(d.total)}
                  </span>
                </div>
                <Bar value={p} />
              </div>
            );
          })}
        </div>
      </section>

      <div className="flex gap-6 text-xs text-mute">
        <span className="flex items-center gap-1">
          <ArrowDown size={12} /> {rate(m.netRxPerSec)}
        </span>
        <span className="flex items-center gap-1">
          <ArrowUp size={12} /> {rate(m.netTxPerSec)}
        </span>
        <span>Memoria de paginación {m.swapTotal ? `${bytes(m.swapUsed)} de ${bytes(m.swapTotal)}` : "—"}</span>
      </div>
    </div>
  );
}
