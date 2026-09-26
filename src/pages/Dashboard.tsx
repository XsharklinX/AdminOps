import { ArrowDown, ArrowUp, Cpu, HardDrive, ListTree, MemoryStick, Monitor, Network } from "lucide-react";
import { useEffect, useState } from "react";
import { Bar, Card, Ring, Sparkline, Stat } from "../components/ui";
import { useLiveMetrics } from "../hooks/useLiveMetrics";
import { api, type SystemInfo } from "../lib/api";
import { bytes, duration, loadColor, pct, rate } from "../lib/format";

export function Dashboard() {
  const { metrics: m, history, error } = useLiveMetrics(2000);
  const [info, setInfo] = useState<SystemInfo | null>(null);

  useEffect(() => {
    api.systemInfo().then(setInfo).catch(() => {});
  }, []);

  if (error && !m) return <p className="p-8 text-bad">Error leyendo métricas: {error}</p>;
  if (!m) return <p className="p-8 font-mono text-sm text-mute">Leyendo sistema…</p>;

  const ramPct = (m.memoryUsed / m.memoryTotal) * 100;
  const swapPct = m.swapTotal ? (m.swapUsed / m.swapTotal) * 100 : 0;
  const disks = m.disks.filter((d) => d.total > 0).sort((a, b) => a.mount.localeCompare(b.mount));

  return (
    <div className="grid grid-cols-12 gap-4 p-6">
      {/* Equipo */}
      <Card title="Equipo" icon={<Monitor size={14} />} className="col-span-12">
        <div className="grid grid-cols-2 gap-x-6 gap-y-3 md:grid-cols-5">
          <Stat label="Host" value={info?.hostName ?? "—"} />
          <Stat label="Sistema" value={info?.osName ?? "—"} sub={info && `Build ${info.kernelVersion}`} />
          <Stat
            label="Procesador"
            value={info?.cpuBrand ?? "—"}
            sub={info && `${info.physicalCores} núcleos · ${info.logicalCores} hilos`}
          />
          <Stat label="Memoria" value={bytes(m.memoryTotal)} sub={m.swapTotal ? `+ ${bytes(m.swapTotal)} de paginación` : undefined} />
          <Stat label="Encendido" value={duration(m.uptime)} sub={`${m.processCount} procesos`} />
        </div>
      </Card>

      {/* CPU */}
      <Card title="CPU" icon={<Cpu size={14} />} className="col-span-12 lg:col-span-6">
        <div className="flex items-center gap-5">
          <Ring value={m.cpuTotal} label="uso" />
          <div className="min-w-0 flex-1">
            <Sparkline data={history.cpu} max={100} color={loadColor(m.cpuTotal)} />
            <div className="mt-3 flex gap-1" title="Uso por hilo">
              {m.cpuPerCore.map((v, i) => (
                <div key={i} className="relative h-7 flex-1 overflow-hidden rounded-sm bg-line" title={`Hilo ${i}: ${pct(v)}`}>
                  <div
                    className="absolute inset-x-0 bottom-0"
                    style={{ height: `${v}%`, background: loadColor(v), opacity: 0.85 }}
                  />
                </div>
              ))}
            </div>
          </div>
        </div>
      </Card>

      {/* RAM */}
      <Card title="Memoria" icon={<MemoryStick size={14} />} className="col-span-12 lg:col-span-6">
        <div className="flex items-center gap-5">
          <Ring value={ramPct} label="ram" />
          <div className="min-w-0 flex-1">
            <Sparkline data={history.ram} max={100} color="var(--color-neon-2)" />
            <div className="mt-3 space-y-2.5">
              <div>
                <div className="mb-1 flex justify-between font-mono text-xs tabular">
                  <span className="text-dim">RAM</span>
                  <span>
                    {bytes(m.memoryUsed)} / {bytes(m.memoryTotal)}
                  </span>
                </div>
                <Bar value={ramPct} />
              </div>
              {m.swapTotal > 0 && (
                <div>
                  <div className="mb-1 flex justify-between font-mono text-xs tabular">
                    <span className="text-dim">Paginación</span>
                    <span>
                      {bytes(m.swapUsed)} / {bytes(m.swapTotal)}
                    </span>
                  </div>
                  <Bar value={swapPct} />
                </div>
              )}
            </div>
          </div>
        </div>
      </Card>

      {/* Discos */}
      <Card title="Almacenamiento" icon={<HardDrive size={14} />} className="col-span-12 lg:col-span-7">
        <div className="space-y-3.5">
          {disks.map((d) => {
            const used = d.total - d.available;
            const p = (used / d.total) * 100;
            return (
              <div key={d.mount}>
                <div className="mb-1.5 flex items-baseline justify-between gap-3">
                  <div className="flex min-w-0 items-baseline gap-2">
                    <span className="font-mono text-sm font-semibold text-ink">{d.mount}</span>
                    <span className="truncate text-xs text-dim">{d.name || "Disco local"}</span>
                    <span className="rounded bg-line px-1.5 font-mono text-[10px] text-mute">
                      {d.kind === "SSD" ? "SSD" : d.kind === "HDD" ? "HDD" : "?"} · {d.fileSystem}
                      {d.removable ? " · USB" : ""}
                    </span>
                  </div>
                  <span className="shrink-0 font-mono text-xs tabular text-dim">
                    <span className={p >= 90 ? "text-bad" : "text-ink"}>{bytes(d.available)}</span> libres de {bytes(d.total)}
                  </span>
                </div>
                <Bar value={p} />
              </div>
            );
          })}
        </div>
      </Card>

      {/* Red */}
      <Card title="Red" icon={<Network size={14} />} className="col-span-12 lg:col-span-5">
        <div className="mb-2 grid grid-cols-2 gap-4">
          <div>
            <div className="flex items-center gap-1 text-[10px] tracking-widest text-mute uppercase">
              <ArrowDown size={11} className="text-neon" /> Bajada
            </div>
            <div className="font-mono text-lg tabular text-neon">{rate(m.netRxPerSec)}</div>
          </div>
          <div>
            <div className="flex items-center gap-1 text-[10px] tracking-widest text-mute uppercase">
              <ArrowUp size={11} className="text-neon-2" /> Subida
            </div>
            <div className="font-mono text-lg tabular text-neon-2">{rate(m.netTxPerSec)}</div>
          </div>
        </div>
        <div className="relative">
          <Sparkline data={history.rx} max={Math.max(...history.rx, ...history.tx, 1)} />
          <div className="absolute inset-0">
            <Sparkline data={history.tx} max={Math.max(...history.rx, ...history.tx, 1)} color="var(--color-neon-2)" />
          </div>
        </div>
      </Card>

      {/* Procesos */}
      <Card title="Procesos con más carga" icon={<ListTree size={14} />} className="col-span-12">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-[10px] tracking-widest text-mute uppercase">
              <th className="pb-2 font-medium">Proceso</th>
              <th className="pb-2 text-right font-medium">PID</th>
              <th className="w-40 pb-2 pl-6 font-medium">CPU</th>
              <th className="pb-2 text-right font-medium">Memoria</th>
            </tr>
          </thead>
          <tbody className="font-mono text-xs tabular">
            {m.topProcesses.map((p) => (
              <tr key={p.pid} className="border-t border-line/70 hover:bg-panel-2">
                <td className="max-w-0 truncate py-1.5 pr-4 font-sans text-[13px] text-ink">{p.name}</td>
                <td className="py-1.5 text-right text-mute">{p.pid}</td>
                <td className="py-1.5 pl-6">
                  <div className="flex items-center gap-2">
                    <div className="flex-1">
                      <Bar value={p.cpu} glow={false} />
                    </div>
                    <span className="w-12 text-right">{p.cpu.toFixed(1)}%</span>
                  </div>
                </td>
                <td className="py-1.5 text-right">{bytes(p.memory)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>
    </div>
  );
}
