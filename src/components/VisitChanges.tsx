import { ArrowRight, History } from "lucide-react";
import { useState } from "react";
import { workApi, type MachineChanges } from "../lib/api";
import { Card } from "./ui";
import { useLiveEffect } from "../lib/useLiveEffect";

const date = (ts: number) => new Date(ts * 1000).toLocaleDateString("es", { day: "numeric", month: "short", year: "numeric" });
const daysAgo = (ts: number) => Math.max(0, Math.round((Date.now() / 1000 - ts) / 86_400));

/**
 * Qué cambió en los equipos de un cliente desde la última visita. En este equipo
 * se compara con cómo está ahora; en los demás, entre sus dos últimas visitas.
 * `host`: solo ese equipo (al empezar una sesión en él).
 */
export function VisitChanges({ clientId, host, refresh = 0 }: { clientId: string; host?: string; refresh?: number }) {
  const [list, setList] = useState<MachineChanges[] | null>(null);
  useLiveEffect(
    (vigente) => {
      workApi
        .visitChanges(clientId)
        .then((l) => vigente() && setList(host ? l.filter((m) => m.host.toLowerCase() === host.toLowerCase()) : l))
        .catch(() => vigente() && setList([]));
    },
    [clientId, host, refresh],
  );

  if (!list?.length) return null;
  return (
    <Card title={host ? "Desde la última visita" : "Qué cambió entre visitas"} icon={<History size={14} />}>
      <ul className="space-y-3">
        {list.map((m) => (
          <li key={m.host}>
            <div className="mb-1 flex flex-wrap items-baseline gap-x-2 text-xs">
              {!host && <span className="font-mono text-sm text-ink">{m.host}</span>}
              <span className="text-mute">
                {m.live ? `última visita el ${date(m.since)} (hace ${daysAgo(m.since)} días) → ahora` : `visita del ${date(m.since)} → ${date(m.until)}`}
              </span>
            </div>
            {m.changes.length === 0 ? (
              <p className="text-xs text-dim">Sin cambios relevantes.</p>
            ) : (
              <ul className="space-y-0.5 text-xs">
                {m.changes.map((c) => (
                  <li key={c.label} className="flex flex-wrap items-center gap-x-2">
                    <span className={`size-1.5 shrink-0 rounded-full ${c.better === null ? "bg-mute" : c.better ? "bg-ok" : "bg-bad"}`} />
                    <span className="text-dim">{c.label}:</span>
                    <span className="text-mute">{c.before || "—"}</span>
                    <ArrowRight size={10} className="text-mute" />
                    <span className={c.better === null ? "text-ink" : c.better ? "text-ok" : "text-bad"}>{c.after}</span>
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
    </Card>
  );
}
