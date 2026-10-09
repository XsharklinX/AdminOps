// Arranques y cuelgues de los últimos 60 días: cuántas veces arrancó, cuántas
// se apagó mal (corte de luz, botón, cuelgue), los pantallazos azules con su
// explicación, y cuánto tarda en arrancar (esto último, con administrador).
import { RotateCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { NeedsAdmin } from "../components/AdminBanner";
import { Card, ErrorState, iconBtn, Loading, Tile } from "../components/ui";
import { insightApi, type BootLog } from "../lib/api";
import { bytes, dateTime, shortDate } from "../lib/format";

const KIND = {
  boot: { label: "Arranque", tone: "text-dim" },
  shutdown: { label: "Apagado normal", tone: "text-mute" },
  unexpected: { label: "Se apagó mal", tone: "text-warn" },
  bsod: { label: "Pantallazo azul", tone: "text-bad" },
} as const;

/** «0x0000009F» → «0x9F», más fácil de leer y de buscar. */
const shortCode = (code: string) => `0x${(Number.parseInt(code.replace(/^0x/i, ""), 16) || 0).toString(16).toUpperCase()}`;

const secs = (ms: number) => `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)} s`;

export function Boots({ isAdmin }: { isAdmin: boolean }) {
  const [log, setLog] = useState<BootLog | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    setBusy(true);
    insightApi
      .bootHistory()
      .then((l) => {
        setLog(l);
        setFailed(null);
      })
      .catch((e) => setFailed(String(e)))
      .finally(() => setBusy(false));
  }, []);
  useEffect(load, [load]);

  const sum = useMemo(() => {
    if (!log) return null;
    const count = (k: string) => log.events.filter((e) => e.kind === k).length;
    // Un pantallazo deja dos huellas (el apagón y el informe): se cuentan los informes,
    // y los apagones con código que no tengan informe a su lado.
    const bsods = log.events.filter((e) => e.kind === "bsod" || (e.kind === "unexpected" && e.code && !log.events.some((b) => b.kind === "bsod" && Math.abs(b.t - e.t) < 600)));
    const boots = log.boots ?? [];
    const recent = boots.slice(-10);
    return {
      boots: count("boot"),
      unexpected: log.events.filter((e) => e.kind === "unexpected" && !e.code).length,
      bsods,
      avg: recent.length ? recent.reduce((a, b) => a + b.ms, 0) / recent.length : null,
      maxMs: Math.max(1, ...boots.map((b) => b.ms)),
    };
  }, [log]);

  if (!log || !sum) return failed ? <ErrorState page message={failed} onRetry={load} /> : <Loading page text="Leyendo el registro de Windows…" />;

  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="min-w-60 flex-1 text-sm text-dim">Lo que dice el registro de Windows de los últimos 60 días: cuándo arrancó, cuándo se apagó mal y los pantallazos azules, con qué suele haber detrás.</p>
        <button onClick={load} disabled={busy} className={iconBtn} title="Volver a leer" aria-label="Volver a leer">
          <RotateCw size={14} className={busy ? "animate-spin" : ""} />
        </button>
      </div>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <Tile label="Arranques" value={sum.boots} />
        <Tile label="Se apagó mal (luz, botón, cuelgue)" value={sum.unexpected} warn />
        <Tile label="Pantallazos azules" value={sum.bsods.length} warn />
        <Tile label="Arranque medio (últimos 10)" value={sum.avg === null ? "—" : secs(sum.avg)} warn={sum.avg !== null && sum.avg > 90_000} />
      </div>

      <Card title="Cuánto tarda en arrancar">
        {log.boots === null ? (
          isAdmin ? (
            <p className="text-sm text-mute">Windows no ha apuntado la duración de ningún arranque.</p>
          ) : (
            <NeedsAdmin>Windows solo deja leer cuánto tarda cada arranque como administrador.</NeedsAdmin>
          )
        ) : log.boots.length === 0 ? (
          <p className="text-sm text-mute">Windows no ha apuntado la duración de ningún arranque en estos 60 días.</p>
        ) : (
          <>
            <div className="flex h-36 items-end gap-1" role="img" aria-label="Duración de cada arranque">
              {log.boots.slice(-40).map((b) => (
                <div key={b.t} className="group relative flex min-w-1 flex-1 flex-col justify-end" title={`${dateTime(b.t)} · ${secs(b.ms)} (escritorio a los ${secs(b.mainMs)})`}>
                  <div className={`rounded-t ${b.ms > 90_000 ? "bg-warn/70" : "bg-neon/60"} group-hover:opacity-80`} style={{ height: `${Math.max(3, (b.ms / sum.maxMs) * 100)}%` }} />
                </div>
              ))}
            </div>
            <p className="mt-2 text-[11.5px] text-mute">Cada barra es un arranque, del más antiguo al más reciente (los últimos 40). En ámbar, los de más de minuto y medio: lo que arranca con Windows suele ser la causa (Optimizar Windows → Inicio de Windows).</p>
          </>
        )}
      </Card>

      <Card title="Pantallazos azules">
        {sum.bsods.length === 0 ? (
          <p className="text-sm text-mute">Ninguno en estos 60 días.</p>
        ) : (
          <ul className="divide-y divide-line">
            {sum.bsods.map((e) => {
              // El volcado de ese pantallazo (Windows lo escribe en el mismo arranque) dice el driver.
              const dump = log.dumps.find((d) => d.analysis?.culprit && Math.abs(d.t - e.t) < 1800);
              return (
                <li key={`${e.t}-${e.kind}`} className="py-2.5">
                  <div className="flex flex-wrap items-baseline gap-x-3">
                    <span className="text-sm font-medium text-ink">{dateTime(e.t)}</span>
                    {e.code && <span className="font-mono text-xs text-bad">{shortCode(e.code)}</span>}
                    {e.name && <span className="font-mono text-[11.5px] text-mute">{e.name}</span>}
                    {dump && (
                      <span className="rounded bg-warn/15 px-1.5 py-px font-mono text-[11.5px] text-warn" title={dump.analysis!.stackDrivers.length ? `En la pila: ${dump.analysis!.stackDrivers.join(", ")}` : undefined}>
                        driver probable: {dump.analysis!.culprit}
                      </span>
                    )}
                  </div>
                  {dump?.analysis?.culpritHint && <p className="mt-0.5 text-[13px] text-dim">{dump.analysis.culpritHint}</p>}
                  <p className="mt-0.5 text-[13px] text-mute">{e.hint ?? (e.code ? "Código poco habitual: búscalo por su número junto al modelo del equipo." : "Windows no guardó el código.")}</p>
                </li>
              );
            })}
          </ul>
        )}
        {log.dumps.length > 0 && (
          <p className="mt-3 text-[11.5px] text-mute">
            Hay {log.dumps.length} {log.dumps.length === 1 ? "volcado" : "volcados"} en la carpeta Minidump de Windows (el último, {shortDate(log.dumps[0].t)}, {bytes(log.dumps[0].size)}). El driver probable sale de ahí: es una pista para empezar, no un veredicto.
          </p>
        )}
        {sum.bsods.length > 0 && log.dumps.length === 0 && !isAdmin && (
          <NeedsAdmin className="mt-3">Para ver qué driver estaba detrás de cada pantallazo hay que leer los volcados de Windows, y eso requiere administrador.</NeedsAdmin>
        )}
      </Card>

      <Card title="Últimos sucesos">
        <ul className="divide-y divide-line text-[13px]">
          {log.events.slice(0, 40).map((e) => (
            <li key={`${e.t}-${e.kind}`} className="flex items-center gap-3 py-1.5">
              <span className="w-44 shrink-0 text-dim">{dateTime(e.t)}</span>
              <span className={KIND[e.kind].tone}>{KIND[e.kind].label}</span>
              {e.code && <span className="font-mono text-xs text-mute">{shortCode(e.code)}</span>}
            </li>
          ))}
          {log.events.length === 0 && <li className="py-2 text-mute">Sin sucesos en estos 60 días.</li>}
        </ul>
      </Card>
    </div>
  );
}
