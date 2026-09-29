import { Copy, Gauge, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { appApi, type StartupTiming } from "../lib/api";
import { appStartMs, commandStats, onPerfChange, pageLoads, type PageLoad } from "../lib/perf";
import { pageLabel, type PageId } from "./Sidebar";
import { useToast } from "./feedback";
import { Card } from "./ui";

const ms = (v: number | null | undefined) => (v == null ? "—" : v >= 1000 ? `${(v / 1000).toFixed(1)} s` : `${v} ms`);
const tone = (v: number | null, warn: number, bad: number) => (v == null ? "text-mute" : v >= bad ? "text-bad" : v >= warn ? "text-warn" : "text-ink");

/** Tiempos del backend con el reloj de la ventana en el momento de la respuesta. */
type Timing = StartupTiming & { perfAt: number };

/** Pide los tiempos y anota a la vez el reloj de la ventana, para poder cruzarlos. */
export const fetchTiming = (): Promise<Timing> => appApi.startupTiming().then((t) => ({ ...t, perfAt: performance.now() }));

/** Partes del arranque, de principio a fin, en ms desde que arrancó el proceso. */
export function startupParts(t: Timing, first: PageLoad | undefined) {
  // Diferencia entre el reloj del proceso y el de la ventana: cuándo empezó a cargarse la interfaz.
  const windowAt = Math.max(0, t.nowMs - Math.round(t.perfAt));
  const app = appStartMs();
  return {
    steps: t.steps,
    windowAt,
    reactAt: app == null ? null : windowAt + app,
    paintAt: first?.paintMs == null ? null : windowAt + first.paintMs,
    readyAt: first?.readyMs == null ? null : windowAt + first.readyMs,
    firstPage: first?.page ?? null,
  };
}

export function startupSummary(t: Timing, first: PageLoad | undefined): string {
  const p = startupParts(t, first);
  const slow = t.steps.filter((s) => s.ms >= 20).map((s) => `${s.name} ${s.ms} ms`);
  return [
    `ventana ${p.windowAt} ms`,
    p.reactAt != null && `interfaz ${p.reactAt} ms`,
    p.paintAt != null && `primera página pintada ${p.paintAt} ms`,
    p.readyAt != null && `lista ${p.readyAt} ms (${p.firstPage})`,
    slow.length > 0 && `pasos lentos: ${slow.join(", ")}`,
  ]
    .filter(Boolean)
    .join(" · ");
}

/** Ajustes → Rendimiento: cuánto tarda en abrirse AdminOps y cada página, con datos. */
export function PerfPanel() {
  const [timing, setTiming] = useState<Timing | null>(null);
  const [, setTick] = useState(0);
  const toast = useToast();
  useEffect(() => {
    fetchTiming().then(setTiming).catch(() => {});
    return onPerfChange(() => setTick((n) => n + 1));
  }, []);

  const loads = pageLoads();
  // La primera medida (la de más atrás) es la de la página de inicio.
  const first = loads.find((l) => l.at === 0);
  const parts = timing ? startupParts(timing, first) : null;
  const calls = commandStats().slice(0, 15);

  const copy = () => {
    const lines = [
      timing ? `Arranque: ${startupSummary(timing, first)}` : "",
      "",
      "Páginas (primera visita): código / pintada / lista",
      ...loads.map((l) => `${l.page}: ${ms(l.codeMs)} / ${ms(l.paintMs)} / ${ms(l.readyMs)}${l.calls[0] ? ` · más lenta: ${l.calls[0].cmd} ${ms(l.calls[0].ms)}` : ""}`),
      "",
      "Llamadas al backend: veces / media / máximo",
      ...calls.map((c) => `${c.cmd}: ${c.count} / ${ms(Math.round(c.totalMs / c.count))} / ${ms(c.maxMs)}`),
    ];
    navigator.clipboard.writeText(lines.join("\n")).then(
      () => toast("ok", "Tiempos copiados."),
      () => toast("error", "No se pudo copiar."),
    );
  };

  return (
    <div className="space-y-4">
      <Card
        title="Arranque de AdminOps"
        icon={<Gauge size={14} />}
        right={
          <div className="flex items-center gap-3 text-[11px]">
            <button onClick={() => fetchTiming().then(setTiming)} className="flex items-center gap-1 text-mute hover:text-ink">
              <RefreshCw size={11} /> Actualizar
            </button>
            <button onClick={copy} className="flex items-center gap-1 text-mute hover:text-ink">
              <Copy size={11} /> Copiar todo
            </button>
          </div>
        }
      >
        {!parts ? (
          <p className="text-sm text-mute">Midiendo…</p>
        ) : (
          <>
            <div className="mb-4 grid grid-cols-2 gap-3 md:grid-cols-4">
              {[
                ["Ventana abierta", parts.windowAt, 800, 2000],
                ["Interfaz en marcha", parts.reactAt, 1200, 3000],
                ["Primera página pintada", parts.paintAt, 1500, 3500],
                ["Lista para usar", parts.readyAt, 2500, 6000],
              ].map(([label, v, w, b]) => (
                <div key={label as string} className="rounded-lg border border-line bg-void/40 px-3 py-2">
                  <div className="text-[11px] text-mute">{label}</div>
                  <div className={`font-mono text-lg ${tone(v as number | null, w as number, b as number)}`}>{ms(v as number | null)}</div>
                </div>
              ))}
            </div>
            <table className="w-full text-xs">
              <thead>
                <tr className="text-left text-[11px] text-mute">
                  <th className="pb-1 font-medium">Paso del programa</th>
                  <th className="pb-1 text-right font-medium">Empieza</th>
                  <th className="pb-1 text-right font-medium">Dura</th>
                </tr>
              </thead>
              <tbody>
                {parts.steps.map((s) => (
                  <tr key={s.name} className="border-t border-line/60">
                    <td className="py-1 text-dim">{s.name}</td>
                    <td className="py-1 text-right font-mono text-mute">{ms(s.atMs)}</td>
                    <td className={`py-1 text-right font-mono ${tone(s.ms, 50, 250)}`}>{ms(s.ms)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="mt-2 text-[11px] text-mute">
              Tiempos desde que arranca el programa. El resumen queda también en el registro técnico para comparar entre versiones.
            </p>
          </>
        )}
      </Card>

      <Card title={`Carga de páginas · ${loads.length}`}>
        {loads.length === 0 ? (
          <p className="text-sm text-mute">Abre algunas páginas: aquí aparece cuánto tarda cada una la primera vez.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="text-left text-[11px] text-mute">
                  <th className="pb-1 font-medium">Página</th>
                  <th className="pb-1 text-right font-medium" title="Descargar y preparar el código de la página">
                    Código
                  </th>
                  <th className="pb-1 text-right font-medium" title="Hasta que se ve en pantalla">
                    Pintada
                  </th>
                  <th className="pb-1 text-right font-medium" title="Hasta que terminan las consultas que hace al abrirse">
                    Lista
                  </th>
                  <th className="pb-1 pl-4 font-medium">Consulta más lenta</th>
                </tr>
              </thead>
              <tbody>
                {loads.map((l, i) => (
                  <tr key={`${l.page}-${l.at}-${i}`} className="border-t border-line/60">
                    <td className="py-1 text-ink">
                      {pageLabel(l.page as PageId)}
                      {l.at === 0 && <span className="ml-1.5 text-[10px] text-mute">(al abrir)</span>}
                    </td>
                    <td className={`py-1 text-right font-mono ${tone(l.codeMs, 300, 1000)}`}>{ms(l.codeMs)}</td>
                    <td className={`py-1 text-right font-mono ${tone(l.paintMs, 400, 1200)}`}>{ms(l.paintMs)}</td>
                    <td className={`py-1 text-right font-mono ${tone(l.readyMs, 1500, 4000)}`}>{ms(l.readyMs)}</td>
                    <td className="py-1 pl-4 text-dim">{l.calls[0] ? `${l.calls[0].cmd} · ${ms(l.calls[0].ms)}` : "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      {calls.length > 0 && (
        <Card title="Consultas al backend más lentas">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-left text-[11px] text-mute">
                <th className="pb-1 font-medium">Consulta</th>
                <th className="pb-1 text-right font-medium">Veces</th>
                <th className="pb-1 text-right font-medium">Media</th>
                <th className="pb-1 text-right font-medium">Máximo</th>
              </tr>
            </thead>
            <tbody>
              {calls.map((c) => (
                <tr key={c.cmd} className="border-t border-line/60">
                  <td className="py-1 font-mono text-dim">{c.cmd}</td>
                  <td className="py-1 text-right font-mono text-mute">{c.count}</td>
                  <td className="py-1 text-right font-mono text-ink">{ms(Math.round(c.totalMs / c.count))}</td>
                  <td className={`py-1 text-right font-mono ${tone(c.maxMs, 800, 3000)}`}>{ms(c.maxMs)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="mt-2 text-[11px] text-mute">Desde que se abrió AdminOps. Las tareas largas a propósito (diagnóstico, análisis) aparecen aquí también.</p>
        </Card>
      )}
    </div>
  );
}
