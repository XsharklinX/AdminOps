import { Globe, Mail, MessagesSquare, Ticket, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "../../components/feedback";
import { Button, Card, inputClass, Toggle } from "../../components/ui";
import { portalsApi, type Portal, type Settings } from "../../lib/api";
import { setPrefs, usePrefs, type CommMode } from "../../lib/prefs";
import { COMM_MODES } from "../../lib/comms";
import type { PageId } from "../../components/Sidebar";

const KIND = {
  "": { label: "Tickets", icon: Ticket, page: "tickets" as PageId },
  inventory: { label: "Inventario web", icon: Globe, page: "inventory" as PageId },
  mail: { label: "Correo", icon: Mail, page: "mail" as PageId },
  teams: { label: "Teams", icon: MessagesSquare, page: "teams" as PageId },
};

const ZOOMS = [0.8, 0.9, 1, 1.1, 1.25, 1.5];

/** Ajustes de los portales (Tickets, inventario web, Correo y Teams) en un solo sitio. */
export function PortalSettings({
  s,
  set,
  onNavigate,
  Row,
}: {
  s: Settings;
  set: (patch: Partial<Settings>) => void;
  onNavigate: (p: PageId) => void;
  Row: (p: { title: string; sub?: string; children: React.ReactNode }) => React.ReactElement;
}) {
  const prefs = usePrefs();
  const [portals, setPortals] = useState<Portal[] | null>(null);
  const toast = useToast();

  const load = () => portalsApi.list().then(setPortals).catch(() => setPortals([]));
  useEffect(() => {
    void load();
  }, []);

  const patch = async (p: Portal, change: Partial<Portal>) => {
    try {
      await portalsApi.save({ ...p, ...change });
      void load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  const privateOnes = (portals ?? []).filter((p) => p.private);

  return (
    <div className="space-y-4">
      <Card title="Cómo se abren los portales">
        <Row
          title="Precargar el último portal"
          sub="Carga en segundo plano el último portal usado de Tickets, Inventario web, Correo y Teams unos segundos después de abrir AdminOps, para que al entrar ya esté listo. Los de sesión privada nunca se precargan."
        >
          <Toggle checked={prefs.preloadPortals} onChange={(v) => setPrefs({ preloadPortals: v })} />
        </Row>
        <Row
          title="Zoom de los portales"
          sub="Tamaño con el que se abren las webs. Cada portal recuerda el suyo si lo cambias desde su barra; esto es el de los nuevos."
        >
          <select
            value={prefs.portalZoom}
            onChange={(e) => setPrefs({ portalZoom: Number(e.target.value) })}
            className="rounded-md border border-line bg-void/60 px-3 py-1.5 text-sm text-ink outline-none focus:border-neon/50"
          >
            {ZOOMS.map((z) => (
              <option key={z} value={z}>
                {Math.round(z * 100)} %
              </option>
            ))}
          </select>
        </Row>
        <Row title="Cómo se abren Teams y el Correo" sub="Desde sus iconos de la barra de arriba. «Preguntar» lo pregunta la próxima vez que se pulsen.">
          <div className="flex flex-col gap-1.5">
            {(["teams", "mail"] as const).map((k) => (
              <label key={k} className="flex items-center justify-end gap-2 text-xs text-dim">
                {k === "teams" ? "Teams" : "Correo"}
                <select
                  value={prefs.comms[k]}
                  onChange={(e) => setPrefs({ comms: { ...prefs.comms, [k]: e.target.value as CommMode } })}
                  className="rounded-md border border-line bg-void/60 px-2 py-1 text-sm text-ink outline-none focus:border-neon/50"
                >
                  <option value="ask">Preguntar</option>
                  {COMM_MODES.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label}
                    </option>
                  ))}
                </select>
              </label>
            ))}
          </div>
        </Row>
        <Row
          title="Dominio de la empresa"
          sub="Las webs de este dominio entran con tu cuenta de Windows sin pedir contraseña (intranets con dominio, por ejemplo empresa.local). Se aplica al volver a abrir AdminOps."
        >
          <input
            value={s.defaultDomain}
            onChange={(e) => set({ defaultDomain: e.target.value })}
            placeholder="empresa.local"
            className={`${inputClass} w-56 font-mono text-xs`}
          />
        </Row>
      </Card>

      <Card title={`Portales configurados · ${portals?.length ?? 0}`}>
        {portals === null ? (
          <p className="text-sm text-mute">Leyendo…</p>
        ) : portals.length === 0 ? (
          <p className="text-sm text-mute">
            Todavía no hay ninguno. Añádelos desde Soporte → Tickets, Soporte → Inventario, o desde los iconos de Correo y Teams de la barra de arriba.
          </p>
        ) : (
          <ul className="divide-y divide-line/60">
            {portals.map((p) => {
              const k = KIND[(p.kind ?? "") as keyof typeof KIND] ?? KIND[""];
              const Icon = k.icon;
              return (
                <li key={p.id} className="flex flex-wrap items-center gap-3 py-2.5">
                  <Icon size={15} className="shrink-0 text-mute" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-sm text-ink">{p.name}</span>
                      <span className="shrink-0 rounded border border-line px-1 text-[10px] text-mute">{k.label}</span>
                    </div>
                    <div className="truncate font-mono text-[11px] text-mute">{p.url}</div>
                  </div>
                  <label className="flex shrink-0 items-center gap-1.5 text-xs text-dim" title="No guarda nada en este equipo y la sesión se cierra al salir de AdminOps">
                    <input type="checkbox" checked={!!p.private} onChange={(e) => patch(p, { private: e.target.checked })} className="accent-[var(--color-neon)]" />
                    Sesión privada
                  </label>
                  <label className="flex shrink-0 items-center gap-1.5 text-xs text-dim" title="Rellena el inicio de sesión con la cuenta guardada de ese portal">
                    <input type="checkbox" checked={!!p.autofill} onChange={(e) => patch(p, { autofill: e.target.checked })} className="accent-[var(--color-neon)]" />
                    Entrar solo
                  </label>
                  <Button kind="ghost" onClick={() => onNavigate(k.page)}>
                    Abrir
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
        {portals !== null && portals.length > 0 && privateOnes.length === 0 && (
          <p className="mt-3 flex items-start gap-1.5 text-[11px] text-warn">
            <TriangleAlert size={12} className="mt-0.5 shrink-0" />
            Ningún portal usa sesión privada: en el equipo de un cliente quedarían tus sesiones abiertas al cerrar AdminOps.
          </p>
        )}
      </Card>
    </div>
  );
}
