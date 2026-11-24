// «Vigilar» en Grabar el fallo (1.4, ideas 9 y 10): el último minuto en memoria, sin escribir nada en el disco,
// y «¡Acaba de pasar!» para guardarlo (o Ctrl+Alt+G desde cualquier sitio). Con la grabación automática
// encendida (apagada de serie), se guarda sola cuando un programa se cuelga, sale un error de aplicación o
// el procesador o el disco se quedan al 100 %.
import { Eye, EyeOff, History, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { faultRecApi, type FaultAutoSettings, type FaultWatchStatus } from "../lib/api";
import { logQuietly } from "../lib/api/core";
import { bufferPercent, REPLAY_SECS, watchSince } from "../lib/faultrec";
import { useToast } from "./feedback";
import { Button, Card, Toggle } from "./ui";

const TRIGGERS: { key: "hang" | "appError" | "busy"; label: string }[] = [
  { key: "hang", label: "Un programa deja de responder" },
  { key: "appError", label: "Error de aplicación" },
  { key: "busy", label: "Procesador o disco al 100 % más de 20 s" },
];

export function FaultWatch({ monitorId, input, onSaved }: { monitorId: string; input: boolean; onSaved: (id: string) => void }) {
  const [status, setStatus] = useState<FaultWatchStatus | null>(null);
  const [busy, setBusy] = useState<"start" | "stop" | "save" | null>(null);
  const toast = useToast();

  const load = useCallback(() => {
    faultRecApi.watchStatus().then((s) => {
      setStatus(s);
      // Vigilar se apagó solo (p. ej. se desconectó el monitor): llega una vez.
      if (s?.error) toast("error", s.error);
    }, logQuietly("FaultWatch.status"));
  }, [toast]);
  useEffect(load, [load]);
  // Encendido: el contador del último minuto se llena; se pregunta cada 2 s.
  useEffect(() => {
    if (!status?.on) return;
    const t = window.setInterval(load, 2000);
    return () => window.clearInterval(t);
  }, [status?.on, load]);

  const run = async (kind: "start" | "stop" | "save", fn: () => Promise<void>) => {
    setBusy(kind);
    try {
      await fn();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const start = () => run("start", async () => setStatus(await faultRecApi.watchStart(monitorId || null, input)));
  const stop = () => run("stop", async () => setStatus(await faultRecApi.watchStop()));
  const save = async () => {
    setBusy("save");
    try {
      const rec = await faultRecApi.justHappened();
      toast("ok", "Guardado el último minuto.");
      onSaved(rec.id);
    } catch {
      // El fallo ya lo enseña Grabar el fallo con el aviso de «terminó» (faultrec://done): aquí no se repite.
    } finally {
      setBusy(null);
    }
  };

  const setAuto = (next: FaultAutoSettings) => {
    // Se ve al momento; si falla, vuelve como estaba.
    const before = status;
    setStatus((s) => (s ? { ...s, auto: next } : s));
    faultRecApi.autoSet(next).then(
      (saved) => setStatus((s) => (s ? { ...s, auto: saved } : s)),
      (e) => {
        setStatus(before);
        toast("error", String(e));
      },
    );
  };
  const unignore = (key: string) => {
    faultRecApi.autoUnignore(key).then(load, (e) => toast("error", String(e)));
  };

  if (!status) return null;
  const auto = status.auto;
  const pct = bufferPercent(status.bufferedSecs);

  return (
    <Card id="fault-watch" title="Vigilar" icon={<Eye size={14} />}>
      {!status.on ? (
        <>
          <p className="-mt-2 mb-3 text-xs text-mute">El último minuto, siempre a mano</p>
          <p className="max-w-[80ch] text-[13px] text-dim">
            Para el fallo que nunca sale cuando grabas: AdminOps guarda en memoria el último minuto de la pantalla, sin escribir nada en el disco. Cuando el usuario diga «¡ya pasó!», un clic lo guarda con sus marcas.
          </p>
          <div className="mt-3">
            <Button kt="V" onClick={start} disabled={busy !== null}>
              <Eye size={13} /> {busy === "start" ? "Empezando…" : "Vigilar"}
            </Button>
          </div>
        </>
      ) : (
        <>
          <p className="-mt-2 mb-3 flex items-center gap-2 text-xs text-mute">
            <span className={`size-2 rounded-full ${status.pausedForRecording ? "bg-warn" : "animate-pulse bg-ok"}`} aria-hidden />
            {status.pausedForRecording ? "En pausa mientras grabas a mano · vuelve sola al terminar" : `${watchSince(status.since)} · último minuto en memoria`}
            {status.monitor ? ` · ${status.monitor}` : ""}
          </p>

          {/* El último minuto: se llena al empezar y luego va corriendo. */}
          <div className="max-w-xl" aria-label={`${Math.round(Math.min(status.bufferedSecs, REPLAY_SECS))} de ${REPLAY_SECS} segundos en memoria`}>
            <div className="relative h-2.5 overflow-hidden rounded-full bg-panel-2">
              <div className="absolute inset-y-0 right-0 rounded-full bg-ok/50 transition-[width] duration-700" style={{ width: `${pct}%` }} />
            </div>
            <div className="mt-1 flex justify-between text-[10.5px] text-mute tabular">
              <span>−{REPLAY_SECS} s</span>
              <span>ahora</span>
            </div>
          </div>

          <div className="mt-3 flex flex-wrap items-center gap-3">
            <Button kt="A" onClick={save} disabled={busy !== null || status.bufferedSecs < 1 || status.pausedForRecording}>
              <History size={14} /> {busy === "save" ? "Guardando…" : "¡Acaba de pasar!"}
            </Button>
            <span className="text-xs text-dim">
              {status.hotkey === false ? "Ctrl+Alt+G lo usa otro programa: usa este botón." : (
                <>
                  Guarda el último minuto · o <kbd className="rounded border border-line-2 px-1 font-mono text-[11px]">Ctrl+Alt+G</kbd> desde cualquier sitio
                </>
              )}
            </span>
            <Button kind="ghost" size="sm" onClick={stop} disabled={busy !== null}>
              <EyeOff size={13} /> Dejar de vigilar
            </Button>
          </div>
        </>
      )}

      {/* La automática: apagada de serie, y solo actúa mientras se vigila. */}
      <div className="mt-4 border-t border-line/60 pt-3">
        <div className="flex items-start gap-3">
          <Toggle checked={auto.enabled} onChange={(v) => setAuto({ ...auto, enabled: v })} label="Guardar sola cuando algo falla" />
          <div className="min-w-0 text-[13px]">
            <p className="text-ink">Guardar sola cuando algo falla</p>
            <p className="text-xs text-dim">
              Mientras vigila: guarda los 30 s de antes y los 15 de después, y avisa una sola vez. Como mucho 2 por programa y hora y 10 al día.
            </p>
          </div>
        </div>
        {auto.enabled && (
          <div className="mt-2.5 ml-12 space-y-1.5 text-[13px]">
            {TRIGGERS.map((t) => (
              <label key={t.key} className="flex items-center gap-2 text-ink">
                <input type="checkbox" className="accent-[var(--color-neon)]" checked={auto[t.key]} onChange={(e) => setAuto({ ...auto, [t.key]: e.target.checked })} />
                {t.label}
              </label>
            ))}
            {auto.ignored.length > 0 && (
              <div className="pt-1.5">
                <p className="text-xs text-mute">No se graba sola por:</p>
                <ul className="mt-1 flex flex-wrap gap-1.5">
                  {auto.ignored.map((i) => (
                    <li key={i.key} className="flex items-center gap-1 rounded-full border border-line px-2.5 py-0.5 text-xs text-dim">
                      {i.label}
                      <button type="button" onClick={() => unignore(i.key)} className="text-mute hover:text-ink" title="Volver a grabar por esto" aria-label={`Volver a grabar por ${i.label}`}>
                        <X size={11} />
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {(status.skippedToday ?? 0) > 0 && <p className="text-xs text-mute">Hoy no se guardaron {status.skippedToday} más por los límites.</p>}
            {!status.on && <p className="text-xs text-warn">Solo funciona mientras se vigila.</p>}
          </div>
        )}
      </div>
    </Card>
  );
}
