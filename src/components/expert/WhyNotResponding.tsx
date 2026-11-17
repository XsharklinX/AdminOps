// ¿Por qué no responde? Panel de Procesos: a qué espera el programa, dicho en
// palabras, y qué se puede hacer. Mirar no cambia nada; reconectar una unidad o
// cerrar el programa sí, y piden confirmación.
import { HelpCircle, Loader2, Network, RefreshCw, TriangleAlert, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../feedback";
import { diagnoseApi, officeApi, type WaitReport } from "../../lib/api";
import { smallBtn } from "../ui";

const KIND_LABEL: Record<string, string> = {
  network: "La red",
  deadlock: "Bloqueado por dentro",
  antivirus: "El antivirus",
  program: "Otro programa",
  service: "Un servicio de Windows",
  lock: "Un bloqueo interno",
  busy: "Ocupado",
  hidden: "No se ve",
};

export function WhyNotResponding({ pid, name, onClose, canClose, onDismiss }: { pid: number; name: string; /** Cerrar el programa (el que llama pide confirmación). */ onClose: () => void; canClose: boolean; onDismiss: () => void }) {
  const toast = useToast();
  const [report, setReport] = useState<WaitReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [looking, setLooking] = useState(true);
  const [reconnecting, setReconnecting] = useState(false);

  const look = useCallback(() => {
    setLooking(true);
    setError(null);
    diagnoseApi
      .whyNotResponding(pid, name)
      .then(setReport, (e) => {
        setReport(null);
        setError(String(e));
      })
      .finally(() => setLooking(false));
  }, [pid, name]);

  useEffect(() => {
    look();
  }, [look]);

  const reconnect = async (letter: string) => {
    setReconnecting(true);
    try {
      toast("ok", await officeApi.reconnectDrive(letter.replace(":", "")));
      look();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setReconnecting(false);
    }
  };

  return (
    <div className="mt-3 rounded-xl border border-line-2 bg-panel px-4 py-3" role="region" aria-label={`Por qué ${name} no responde`}>
      <div className="mb-2 flex items-center gap-2">
        <HelpCircle size={14} className="text-neon" />
        <span className="text-sm font-medium text-ink">¿Por qué {name.replace(/\.exe$/i, "")} no responde?</span>
        {report?.hung && <span className="rounded-full border border-warn/50 px-2 text-[11px] text-warn">Windows lo marca «No responde»</span>}
        <button onClick={onDismiss} className="ml-auto text-[11px] text-mute hover:text-ink">
          Ocultar
        </button>
      </div>

      {looking && !report ? (
        <p className="flex items-center gap-2 py-2 text-sm text-mute">
          <Loader2 size={14} className="animate-spin" /> Mirando a qué espera…
        </p>
      ) : error ? (
        <p className="text-sm text-bad">{error}</p>
      ) : report ? (
        <>
          <p className={`text-sm ${report.verdict.kind === "busy" || report.verdict.kind === "hidden" ? "text-dim" : "text-ink"}`}>{report.verdict.text}</p>

          {report.reasons.length > 0 && (
            <ul className="mt-2 space-y-1">
              {report.reasons.map((r) => (
                <li key={r.kind + r.text} className="flex items-start gap-2 text-xs text-dim">
                  <span className="mt-0.5 shrink-0 rounded border border-line-2 px-1.5 text-[10px] text-mute">{KIND_LABEL[r.kind] ?? r.kind}</span>
                  <span className="min-w-0 break-words">
                    {r.text}
                    {r.threads > 1 && <span className="text-mute"> ({r.threads} partes del programa)</span>}
                  </span>
                </li>
              ))}
            </ul>
          )}

          {report.nets.length > 0 && (
            <ul className="mt-2 space-y-1">
              {report.nets.map((n) => (
                <li key={n.unc} className="flex items-center gap-2 text-xs text-dim">
                  <Network size={12} className={n.reachable ? "text-ok" : "text-bad"} />
                  <span className="font-mono text-ink">{n.drive}</span>
                  <span className="min-w-0 truncate font-mono text-mute select-text">{n.unc}</span>
                  <span className={n.reachable ? "text-ok" : "text-bad"}>{n.reachable ? "contesta" : "no contesta"}</span>
                  {n.used && <span className="text-mute">· el programa la usa</span>}
                </li>
              ))}
            </ul>
          )}

          {report.connections.length > 0 && (
            <p className="mt-2 text-[11px] text-mute">
              Conexiones con otros equipos: {report.connections.slice(0, 5).map((c) => `${c.remote}:${c.port} (${c.state})`).join(" · ")}
              {report.connections.length > 5 && ` y ${report.connections.length - 5} más`}.
            </p>
          )}
          {report.readable === 0 && report.threads > 0 && (
            <p className="mt-2 flex items-center gap-1.5 text-[11px] text-warn">
              <TriangleAlert size={12} /> Windows no dejó leer las esperas de este programa (¿es de otro usuario o está protegido?). Con administrador se ve más.
            </p>
          )}

          <div className="mt-3 flex flex-wrap items-center gap-2">
            {report.reconnect && (
              <button onClick={() => void reconnect(report.reconnect!)} disabled={reconnecting} className={smallBtn} title="Vuelve a conectar la unidad de red; el programa suele seguir solo">
                {reconnecting ? <Loader2 size={12} className="animate-spin" /> : <Network size={12} />} Reconectar {report.reconnect}
              </button>
            )}
            <button onClick={look} disabled={looking} className={smallBtn} title="Esperar un poco y volver a mirar">
              <RefreshCw size={12} className={looking ? "animate-spin" : ""} /> Esperar y volver a mirar
            </button>
            {canClose && (
            <button onClick={onClose} className="inline-flex items-center gap-1.5 rounded-md border border-bad/60 bg-bad/10 px-2.5 py-1.5 text-xs font-medium text-bad hover:bg-bad/20" title="Se pierde lo que no esté guardado">
              <XCircle size={12} /> Cerrar (se pierde lo no guardado)
            </button>
            )}
          </div>
        </>
      ) : null}
    </div>
  );
}
