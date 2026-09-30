import { BarChart3, TriangleAlert } from "lucide-react";
import { useState } from "react";
import { workApi, type Comparison } from "../lib/api";
import { Card } from "./ui";
import { useLiveEffect } from "../lib/useLiveEffect";

/**
 * Cómo queda cada equipo frente a los demás del mismo cliente. Solo cifras
 * técnicas del propio equipo (arranque, memoria, disco, seguridad): sirve para
 * decir con datos cuál toca renovar, sin mirar nada de lo que hay dentro.
 */
export function MachineCompare({ clientId, refresh = 0 }: { clientId: string; refresh?: number }) {
  const [rows, setRows] = useState<Comparison[] | null>(null);

  useLiveEffect(
    (vigente) => {
      workApi
        .compareMachines(clientId)
        .then((r) => vigente() && setRows(r))
        .catch(() => vigente() && setRows([]));
    },
    [clientId, refresh],
  );

  // Con menos de tres equipos no hay con qué comparar: el backend no devuelve nada.
  if (!rows?.length) return null;

  return (
    <Card title="Este equipo frente al resto de la oficina" icon={<BarChart3 size={14} />}>
      <ul className="space-y-3">
        {rows.map((c) => {
          const worst = c.machines.find((m) => m.worse);
          return (
            <li key={c.label}>
              <div className="flex items-start gap-2">
                {worst ? <TriangleAlert size={13} className="mt-0.5 shrink-0 text-warn" /> : <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-ok" />}
                <p className={`text-sm ${worst ? "text-ink" : "text-dim"}`}>{c.summary}</p>
              </div>
              <ul className="mt-1.5 ml-5 space-y-0.5">
                {c.machines.map((m) => (
                  <li key={m.host} className="flex items-center gap-2 text-xs">
                    <span className={`min-w-0 flex-1 truncate font-mono ${m.worse ? "text-warn" : "text-mute"}`}>{m.host}</span>
                    <span className={m.worse ? "text-warn" : "text-dim"}>{m.value}</span>
                    {m.factor !== null && m.factor >= 1.2 && <span className="w-12 text-right text-[11px] text-mute">×{m.factor.toFixed(1)}</span>}
                  </li>
                ))}
              </ul>
            </li>
          );
        })}
      </ul>
      <p className="mt-3 text-[11px] text-mute">
        Se compara con la mediana de los equipos de este cliente, a partir de las cifras de su última visita. Solo datos técnicos del equipo.
      </p>
    </Card>
  );
}
