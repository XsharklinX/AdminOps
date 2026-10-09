// Vigilante de la conexión: se deja en marcha y apunta cada corte, cuánto duró
// y de quién era la culpa (router o Internet). El resumen se copia tal cual para
// el proveedor de Internet.
import { Copy, Play, Square, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { TimeChart } from "../components/TimeChart";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, ErrorState, Loading, Tile } from "../components/ui";
import { netwatchApi, type Outage, type WatchStatus } from "../lib/api";
import { dateTime } from "../lib/format";
import { usePageActive } from "../lib/pageActive";

const BLAME = {
  router: { label: "El router", hint: "No contestaba ni el router: cable, Wi-Fi, el propio router o la corriente." },
  internet: { label: "Internet", hint: "El router contestaba, pero fuera no: la línea o el proveedor." },
  network: { label: "Sin red", hint: "El equipo no estaba conectado a ninguna red." },
} as const;

export const lasted = (o: Outage, now = Date.now() / 1000) => Math.max(0, (o.end ?? now) - o.start);

export function durationText(secs: number): string {
  if (secs < 60) return `${Math.round(secs)} s`;
  if (secs < 3600) return `${Math.floor(secs / 60)} min ${Math.round(secs % 60)} s`;
  return `${Math.floor(secs / 3600)} h ${Math.round((secs % 3600) / 60)} min`;
}

/** El resumen para el proveedor: desde cuándo se vigila, cuántos cortes y los peores. */
export function summaryText(s: WatchStatus, now = Date.now() / 1000): string {
  const list = [...s.outages].sort((a, b) => a.start - b.start);
  const since = s.startedAt ?? list[0]?.start ?? now;
  const total = list.reduce((a, o) => a + lasted(o, now), 0);
  const longest = list.reduce<Outage | null>((m, o) => (!m || lasted(o, now) > lasted(m, now) ? o : m), null);
  const lines = [
    `Vigilancia de la conexión desde ${dateTime(since)} hasta ${dateTime(now)}.`,
    list.length
      ? `${list.length} ${list.length === 1 ? "corte" : "cortes"}, ${durationText(total)} sin conexión en total; el más largo, ${durationText(lasted(longest!, now))} (${dateTime(longest!.start)}).`
      : "Ningún corte en ese tiempo.",
    ...(s.checks ? [`Comprobaciones: ${s.checks}, fallidas: ${s.failed} (${((100 * s.failed) / s.checks).toFixed(1)} %).`] : []),
    ...list.map((o) => `- ${dateTime(o.start)}: ${durationText(lasted(o, now))} sin conexión. Fallaba: ${BLAME[o.kind].label.toLowerCase()}.`),
  ];
  return lines.join("\n");
}

export function NetWatch() {
  const [s, setS] = useState<WatchStatus | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const active = usePageActive();
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => {
    netwatchApi
      .status()
      .then((x) => {
        setS(x);
        setFailed(null);
      })
      .catch((e) => setFailed(String(e)));
  }, []);
  // Mientras se mira y está en marcha, se refresca al ritmo de las comprobaciones.
  useEffect(() => {
    if (!active) return;
    load();
    const t = window.setInterval(load, 5000);
    return () => window.clearInterval(t);
  }, [active, load]);

  const stats = useMemo(() => {
    if (!s) return null;
    const last = s.points[s.points.length - 1];
    const okMs = s.points.filter((p) => p.ms !== null).map((p) => p.ms!);
    return {
      now: last ? (last.ms === null ? "sin respuesta" : `${last.ms} ms`) : "—",
      avg: okMs.length ? Math.round(okMs.reduce((a, b) => a + b, 0) / okMs.length) : null,
      loss: s.checks ? (100 * s.failed) / s.checks : 0,
      open: s.outages.some((o) => o.end === null),
    };
  }, [s]);

  if (!s || !stats) return failed ? <ErrorState page message={failed} onRetry={load} /> : <Loading page />;

  const toggle = () =>
    (s.running ? netwatchApi.stop().then(() => window.setTimeout(load, 600)) : netwatchApi.start().then(setS)).catch((e) => toast("error", String(e)));
  const copy = () =>
    navigator.clipboard.writeText(summaryText(s)).then(
      () => toast("ok", "Resumen copiado: pégalo en el correo o el chat del proveedor."),
      () => toast("error", "No se pudo copiar."),
    );
  const clear = async () => {
    if (await confirm({ title: "Borrar los cortes apuntados", body: "Se borra la lista de cortes y la gráfica. No cambia nada del equipo.", confirmLabel: "Borrar" })) {
      await netwatchApi.clear().catch((e) => toast("error", String(e)));
      load();
    }
  };
  const outages = [...s.outages].sort((a, b) => b.start - a.start);

  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="min-w-60 flex-1 text-sm text-dim">
          Déjalo en marcha mientras pasa lo de «se me corta a ratos»: cada 5 segundos hace un ping al router y a Internet, y apunta cada corte con su hora, cuánto duró y de quién era la culpa. Solo funciona con AdminOps abierta.
        </p>
        <Button kind={s.running ? "danger" : "primary"} onClick={() => void toggle()}>
          {s.running ? <Square size={13} /> : <Play size={14} />} {s.running ? "Detener" : "Poner en marcha"}
        </Button>
      </div>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <Tile label={s.running ? `En marcha desde ${s.startedAt ? dateTime(s.startedAt) : "—"}` : "Detenido"} value={s.running ? (stats.open ? "Cortado" : "Vigilando") : "—"} warn={stats.open} />
        <Tile label="Internet ahora" value={stats.now} />
        <Tile label="Respuesta media" value={stats.avg === null ? "—" : `${stats.avg} ms`} />
        <Tile label="Comprobaciones perdidas" value={`${stats.loss.toFixed(1)} %`} warn={stats.loss >= 2} />
      </div>

      {s.points.length > 1 && (
        <Card title={`Respuesta de Internet${s.gateway ? ` · router ${s.gateway}` : ""}`}>
          <TimeChart
            times={s.points.map((p) => p.t)}
            series={[{ label: "Internet (ms)", color: "var(--color-neon)", values: s.points.map((p) => p.ms) }]}
            unit="ms"
            gapSeconds={30}
            height={160}
            marks={s.outages.map((o) => [o.start, o.end ?? Date.now() / 1000])}
            describe={(i) => (s.points[i].ms === null ? (s.points[i].router ? "el router contesta; Internet no" : "no contesta ni el router") : null)}
          />
          <p className="mt-1 text-[11.5px] text-mute">La última hora y media. En rojo, los cortes.</p>
        </Card>
      )}

      <Card
        title={`Cortes · ${outages.length}`}
        right={
          <div className="flex gap-1.5">
            <Button kind="secondary" size="sm" onClick={() => void copy()}>
              <Copy size={13} /> Copiar resumen
            </Button>
            {outages.length > 0 && (
              <Button kind="ghost" size="sm" onClick={() => void clear()}>
                <Trash2 size={13} /> Borrar
              </Button>
            )}
          </div>
        }
      >
        {outages.length === 0 ? (
          <p className="text-sm text-mute">{s.running ? "Ningún corte por ahora." : "Ponlo en marcha para empezar a apuntar."}</p>
        ) : (
          <ul className="divide-y divide-line text-[13px]">
            {outages.map((o) => (
              <li key={o.start} className="flex flex-wrap items-baseline gap-x-4 gap-y-0.5 py-2">
                <span className="w-44 shrink-0 text-ink">{dateTime(o.start)}</span>
                <span className={`w-28 shrink-0 font-mono ${o.end === null ? "text-bad" : "text-dim"}`}>{o.end === null ? "sigue cortado" : durationText(lasted(o))}</span>
                <span className="font-medium text-ink">{BLAME[o.kind].label}</span>
                <span className="min-w-0 flex-1 text-xs text-mute">{BLAME[o.kind].hint}</span>
              </li>
            ))}
          </ul>
        )}
      </Card>
      {dialog}
    </div>
  );
}
