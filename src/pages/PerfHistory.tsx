// Rendimiento de los últimos días: procesador, memoria y disco minuto a minuto,
// y qué programa estaba detrás de cada pico. Para «va lento desde el martes».
import { PictureInPicture2, RotateCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { TimeChart } from "../components/TimeChart";
import { Card, EmptyState, ErrorState, iconBtn, Loading, smallBtn, Tile } from "../components/ui";
import { insightApi, logQuietly, workApi, type Sample } from "../lib/api";
import { dateTime } from "../lib/format";
import { usePageActive } from "../lib/pageActive";

const RANGES = [
  { id: "6h", label: "6 horas", secs: 6 * 3600 },
  { id: "24h", label: "24 horas", secs: 24 * 3600 },
  { id: "7d", label: "7 días", secs: 7 * 86400 },
] as const;

/** Momentos con el procesador por encima de esto cuentan como pico. */
const PEAK = 60;

export function PerfHistory() {
  const [all, setAll] = useState<Sample[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [range, setRange] = useState<(typeof RANGES)[number]["id"]>("24h");
  const active = usePageActive();

  const load = useCallback(() => {
    insightApi
      .perfHistory()
      .then((v) => {
        setAll(v);
        setFailed(null);
      })
      .catch((e) => setFailed(String(e)));
  }, []);
  // Una muestra nueva cada minuto: se vuelve a leer mientras se mira.
  useEffect(() => {
    if (!active) return;
    load();
    const t = window.setInterval(load, 60_000);
    return () => window.clearInterval(t);
  }, [active, load]);

  const view = useMemo(() => {
    if (!all) return null;
    const secs = RANGES.find((r) => r.id === range)!.secs;
    const from = Date.now() / 1000 - secs;
    const v = all.filter((s) => s.t >= from);
    const avg = (k: "cpu" | "ram") => (v.length ? v.reduce((a, s) => a + s[k], 0) / v.length : 0);
    // Qué programas aparecen en los picos de procesador, cuántas veces.
    const culprits = new Map<string, number>();
    for (const s of v) if (s.cpu >= PEAK && s.top) culprits.set(s.top, (culprits.get(s.top) ?? 0) + 1);
    const peaks = [...v].filter((s) => s.cpu >= PEAK).sort((a, b) => b.cpu - a.cpu).slice(0, 5);
    return {
      v,
      minutes: v.length,
      cpu: avg("cpu"),
      ram: avg("ram"),
      peakMinutes: v.filter((s) => s.cpu >= PEAK).length,
      culprits: [...culprits.entries()].sort((a, b) => b[1] - a[1]).slice(0, 5),
      peaks,
    };
  }, [all, range]);

  if (!all) return failed ? <ErrorState page message={failed} onRetry={load} /> : <Loading page />;

  return (
    <div className="mx-auto max-w-6xl space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="min-w-60 flex-1 text-sm text-dim">
          Cómo ha ido el equipo, minuto a minuto. Se mide <b className="font-medium text-ink">mientras AdminOps está abierta</b>: los huecos de la gráfica son ratos con AdminOps cerrada.
        </p>
        <div className="inline-flex rounded-lg border border-line bg-void p-0.5" role="group" aria-label="Tramo">
          {RANGES.map((r) => (
            <button key={r.id} onClick={() => setRange(r.id)} aria-pressed={range === r.id} className={`rounded-md px-2.5 py-1 text-xs ${range === r.id ? "bg-panel-2 font-medium text-ink" : "text-dim hover:text-ink"}`}>
              {r.label}
            </button>
          ))}
        </div>
        <button
          onClick={() => void workApi.miniMonitor().catch(logQuietly("PerfHistory"))}
          className={smallBtn}
          title="Una ventanita siempre encima con procesador, memoria, temperatura y red, para vigilar mientras pruebas otra cosa"
        >
          <PictureInPicture2 size={13} /> Mini monitor
        </button>
        <button onClick={load} className={iconBtn} title="Volver a leer" aria-label="Volver a leer">
          <RotateCw size={14} />
        </button>
      </div>

      {view && view.minutes < 2 ? (
        <EmptyState title="Todavía no hay datos de este tramo">
          AdminOps apunta una muestra por minuto desde que se abre. Vuelve en un rato: aquí se verá cómo va el procesador, la memoria y el disco, y qué programa estaba detrás de cada pico.
        </EmptyState>
      ) : (
        view && (
          <>
            <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
              <Tile label="Medido" value={view.minutes >= 120 ? `${Math.round(view.minutes / 60)} h` : `${view.minutes} min`} />
              <Tile label="Procesador de media" value={`${Math.round(view.cpu)} %`} warn={view.cpu >= 50} />
              <Tile label="Memoria de media" value={`${Math.round(view.ram)} %`} warn={view.ram >= 85} />
              <Tile label={`Minutos con el procesador sobre el ${PEAK} %`} value={view.peakMinutes} warn />
            </div>
            <Card title="Procesador, memoria y disco">
              <TimeChart
                times={view.v.map((s) => s.t)}
                max={100}
                height={220}
                series={[
                  { label: "Procesador", color: "var(--color-neon)", values: view.v.map((s) => s.cpu) },
                  { label: "Memoria", color: "var(--color-warn)", values: view.v.map((s) => s.ram) },
                  { label: "Disco del sistema ocupado", color: "var(--color-mute)", values: view.v.map((s) => s.disk) },
                ]}
                describe={(i) => (view.v[i].top ? `más uso: ${view.v[i].top}` : null)}
              />
            </Card>
            <div className="grid gap-4 lg:grid-cols-2">
              <Card title="Qué había detrás de los picos">
                {view.culprits.length === 0 ? (
                  <p className="text-sm text-mute">Ningún programa ha tenido el procesador por encima del {PEAK} % en este tramo.</p>
                ) : (
                  <ul className="space-y-1.5 text-sm">
                    {view.culprits.map(([name, n]) => (
                      <li key={name} className="flex items-center gap-3">
                        <span className="min-w-0 flex-1 truncate font-mono text-[13px] text-ink">{name}</span>
                        <span className="text-xs text-mute">{n} min</span>
                      </li>
                    ))}
                  </ul>
                )}
              </Card>
              <Card title="Momentos de más carga">
                {view.peaks.length === 0 ? (
                  <p className="text-sm text-mute">Sin picos en este tramo.</p>
                ) : (
                  <ul className="space-y-1.5 text-sm">
                    {view.peaks.map((s) => (
                      <li key={s.t} className="flex items-center gap-3">
                        <span className="w-40 shrink-0 text-dim">{dateTime(s.t)}</span>
                        <span className="w-14 shrink-0 font-mono text-warn">{Math.round(s.cpu)} %</span>
                        <span className="min-w-0 flex-1 truncate font-mono text-[12.5px] text-mute">{s.top ?? "—"}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </Card>
            </div>
          </>
        )
      )}
    </div>
  );
}
