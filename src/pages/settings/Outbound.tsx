// Ajustes → Seguridad → Qué sale de este equipo.
import { Globe } from "lucide-react";
import { useEffect, useState } from "react";
import { Card, Toggle } from "../../components/ui";
import { outboundApi, type OutboundService } from "../../lib/api";
import { ago } from "../../lib/format";
import { type SettingsProps } from "./shared";

export function Outbound({ s, set }: SettingsProps) {
  const [list, setList] = useState<OutboundService[] | null>(null);
  const off = s.netOff ?? [];
  const offKey = off.join();
  useEffect(() => {
    outboundApi.list().then(setList, () => setList([]));
    // Al abrir y cada vez que se apaga o enciende algo: lo último que se usó puede haber cambiado.
  }, [offKey]);
  const toggle = (id: string, on: boolean) => set({ netOff: on ? off.filter((x) => x !== id) : [...off, id] });
  return (
    <Card title="Qué sale de este equipo" icon={<Globe size={14} />}>
      <p className="mb-3 text-xs text-dim">
        Los pocos sitios con los que AdminOps habla por su cuenta, qué se les envía y cuándo. Todo lo demás está bloqueado: la ventana solo puede cargar lo suyo. Apaga lo que no quieras y AdminOps sigue funcionando sin Internet.
      </p>
      {list === null ? null : (
        <ul className="divide-y divide-line/60 rounded-lg border border-line">
          {list.map((x) => {
            const enabled = x.switchable ? !off.includes(x.id) : true;
            return (
              <li key={x.id} className={`flex items-start gap-3 px-3 py-2.5 ${enabled ? "" : "opacity-60"}`}>
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium text-ink">{x.what}</span>
                  <span className="block font-mono text-[11px] text-mute">{x.host}</span>
                  <span className="mt-1 block text-xs text-dim">
                    <b className="font-medium text-ink">Se envía:</b> {x.sends}
                  </span>
                  <span className="block text-xs text-dim">
                    <b className="font-medium text-ink">Cuándo:</b> {x.when}
                    {x.lastUsed > 0 && <span className="text-mute"> · última vez {ago(x.lastUsed)}</span>}
                  </span>
                </span>
                {x.switchable ? (
                  <Toggle checked={enabled} onChange={(v) => toggle(x.id, v)} label={x.what} />
                ) : (
                  <span className="shrink-0 rounded border border-line-2 px-1.5 py-px text-[11px] text-mute" title="No se puede apagar: es una comprobación o la hace Windows">
                    Fijo
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
      <p className="mt-3 text-[11px] text-mute">Los portales que configures (tickets, correo, Teams) son webs que abres tú dentro de AdminOps; no se cuentan aquí. En ningún caso se envían datos de clientes, contactos ni contraseñas.</p>
    </Card>
  );
}
