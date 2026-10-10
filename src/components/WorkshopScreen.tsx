// Pantalla de taller (1.2.9): cifras enormes del equipo y de lo que se está
// haciendo, para dejarla en un monitor mientras se trabaja en otra cosa o para
// que el cliente vea el avance desde lejos. Sin menús. F11 o Esc la cierran.
import { listen } from "@tauri-apps/api/event";
import { X } from "lucide-react";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useLiveMetrics } from "../hooks/useLiveMetrics";
import { tempColor, useSensors } from "../hooks/useSensors";
import { bytes, loadColor, rate } from "../lib/format";
import { humanDuration, percentOf, remainingSeconds, type Sample } from "../lib/taskEta";
import { Sparkline } from "./ui";

interface Job {
  task: string;
  name: string;
  message: string;
  started: number;
  samples: Sample[];
}

/** Lo que se está haciendo ahora, con el porcentaje que traigan los mensajes. */
function useJobs() {
  const [jobs, setJobs] = useState<Record<string, Job>>({});
  useEffect(() => {
    const offs = [
      listen<{ task: string; name: string }>("task-started", ({ payload }) => setJobs((j) => ({ ...j, [payload.task]: j[payload.task] ?? { task: payload.task, name: payload.name, message: "", started: Date.now(), samples: [] } }))),
      listen<{ task: string; message: string }>("task-progress", ({ payload }) =>
        setJobs((j) => {
          const cur = j[payload.task];
          if (!cur) return j;
          const pct = percentOf(payload.message);
          const samples = pct === null ? cur.samples : [...cur.samples.filter((x) => x.pct <= pct), { at: Date.now(), pct }].slice(-60);
          return { ...j, [payload.task]: { ...cur, message: payload.message, samples } };
        }),
      ),
      listen<{ task: string }>("task-finished", ({ payload }) =>
        setJobs((j) => {
          const n = { ...j };
          delete n[payload.task];
          return n;
        }),
      ),
    ];
    return () => offs.forEach((o) => void o.then((f) => f()));
  }, []);
  return Object.values(jobs).sort((a, b) => a.started - b.started);
}

function Big({ label, value, unit, color, children }: { label: string; value: string; unit?: string; color?: string; children?: React.ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-2 rounded-2xl border border-line bg-panel p-5 shadow-elev-1">
      <div className="text-sm text-dim">{label}</div>
      <div className="flex items-baseline gap-2">
        <span className="truncate text-[56px] leading-none font-semibold tracking-tight tabular" style={{ color: color ?? "var(--color-ink)" }}>
          {value}
        </span>
        {unit && <span className="text-xl text-mute">{unit}</span>}
      </div>
      {children}
    </div>
  );
}

export function WorkshopScreen({ onClose, host }: { onClose: () => void; host: string }) {
  const { metrics: m, history } = useLiveMetrics(1000);
  const { sensors } = useSensors(3000);
  const jobs = useJobs();
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const t = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(t);
  }, []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "F11") {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  const cpuTemp = sensors?.cpuTemp ?? null;
  const gpuTemp = sensors?.gpus.find((g) => g.temperature != null)?.temperature ?? null;
  const hot = Math.max(cpuTemp ?? 0, gpuTemp ?? 0);
  const ram = m ? (m.memoryUsed / m.memoryTotal) * 100 : 0;
  const sys = m?.disks.filter((d) => d.total > 0).find((d) => d.mount.toUpperCase().startsWith("C:")) ?? m?.disks.find((d) => d.total > 0);

  return createPortal(
    <div role="dialog" aria-modal="true" aria-label="Pantalla de taller" className="fixed inset-0 z-[60] flex flex-col gap-5 overflow-y-auto bg-void p-8">
      <header className="flex items-center gap-4">
        <div>
          <div className="text-3xl font-semibold tracking-tight text-ink">{host || "Este equipo"}</div>
          <div className="text-sm text-mute">Pantalla de taller · se actualiza cada segundo</div>
        </div>
        <div className="ml-auto text-right">
          <div className="font-mono text-4xl tabular text-ink">{now.toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}</div>
        </div>
        <button onClick={onClose} className="grid size-10 place-items-center rounded-lg border border-line-2 text-dim hover:text-ink" title="Cerrar (Esc o F11)" aria-label="Cerrar la pantalla de taller">
          <X size={18} />
        </button>
      </header>

      {hot >= 88 && (
        <div className="rounded-2xl border border-bad/50 bg-bad/10 px-6 py-4 text-2xl font-semibold text-bad" role="alert">
          ■ Temperatura crítica: {Math.round(hot)} °C
        </div>
      )}

      {jobs.length > 0 && (
        <section className="space-y-3">
          {jobs.map((j) => {
            const pct = j.samples.length ? j.samples[j.samples.length - 1].pct : null;
            const left = remainingSeconds(j.samples);
            return (
              <div key={j.task} className="rounded-2xl border border-neon/40 bg-neon/5 p-5">
                <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
                  <span className="text-2xl font-semibold text-ink">{j.name}</span>
                  <span className="min-w-0 flex-1 truncate text-lg text-dim">{j.message || "Trabajando…"}</span>
                  <span className="font-mono text-4xl tabular text-neon">{pct !== null ? `${Math.round(pct)} %` : humanDuration(Math.floor((Date.now() - j.started) / 1000))}</span>
                </div>
                <div className="mt-3 h-3 overflow-hidden rounded-full bg-line">
                  <div className="bar-stripes h-full rounded-full bg-neon transition-[width] duration-500" style={{ width: pct !== null ? `${pct}%` : "35%" }} />
                </div>
                <div className="mt-1.5 flex justify-between font-mono text-sm text-mute">
                  <span>lleva {humanDuration(Math.floor((Date.now() - j.started) / 1000))}</span>
                  {pct !== null && <span>{left !== null ? `quedan ~${humanDuration(left)}` : "calculando…"}</span>}
                </div>
              </div>
            );
          })}
        </section>
      )}

      {m ? (
        <section className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          <Big label="Procesador" value={`${Math.round(m.cpuTotal)}`} unit="%" color={loadColor(m.cpuTotal)}>
            <Sparkline data={history.cpu} max={100} color={loadColor(m.cpuTotal)} height={56} />
          </Big>
          <Big label="Memoria" value={`${Math.round(ram)}`} unit={`% de ${bytes(m.memoryTotal)}`} color={loadColor(ram)}>
            <Sparkline data={history.ram} max={100} color={loadColor(ram)} height={56} />
          </Big>
          <Big label="Temperatura" value={hot > 0 ? `${Math.round(hot)}` : "—"} unit={hot > 0 ? "°C" : ""} color={tempColor(hot || null)}>
            <div className="text-sm text-mute">
              {cpuTemp != null ? `Procesador ${Math.round(cpuTemp)} °C` : "Procesador: requiere administrador"}
              {gpuTemp != null && ` · Gráfica ${Math.round(gpuTemp)} °C`}
            </div>
          </Big>
          <Big label={`Disco ${sys ? sys.mount.replace(/\\$/, "") : ""}`} value={sys ? bytes(sys.available).replace(/ GB$/, "") : "—"} unit="GB libres">
            <div className="text-sm text-mute">{sys ? `de ${bytes(sys.total)}` : ""}</div>
          </Big>
          <Big label="Red · bajada" value={rate(m.netRxPerSec)}>
            <Sparkline data={history.rx} max={Math.max(...history.rx, ...history.tx, 1)} color="var(--color-dim)" height={56} />
          </Big>
          <Big label="Red · subida" value={rate(m.netTxPerSec)}>
            <div className="text-sm text-mute">{m.processCount} procesos en marcha</div>
          </Big>
        </section>
      ) : (
        <p className="text-xl text-mute">Leyendo el equipo…</p>
      )}
    </div>,
    document.body,
  );
}
