// Abre Teams o el Correo como haya elegido el técnico. La primera vez pregunta
// (dentro de AdminOps, en el navegador o en la aplicación de Windows) y lo
// recuerda; se cambia en Ajustes → Portales y correo.
import { AppWindow, Globe, LayoutPanelTop } from "lucide-react";
import { useEffect, useState } from "react";
import { commsApi, type CommApps } from "../lib/api";
import { COMM_MODES, COMM_NAME, onOpenComm, type CommKind } from "../lib/comms";
import { getPrefs, setPrefs, type CommMode } from "../lib/prefs";
import { useToast } from "./feedback";
import { Modal } from "./ui";
import type { PageId } from "./Sidebar";

const ICON = { adminops: LayoutPanelTop, browser: Globe, app: AppWindow };

export function CommOpener({ onAdminOps }: { onAdminOps: (page: PageId) => void }) {
  const [asking, setAsking] = useState<CommKind | null>(null);
  const [apps, setApps] = useState<CommApps | null>(null);
  const toast = useToast();

  const open = (kind: CommKind, mode: Exclude<CommMode, "ask">) => {
    if (mode === "adminops") onAdminOps(kind);
    else commsApi.open(kind, mode).catch((e) => toast("error", String(e)));
  };

  useEffect(
    () =>
      onOpenComm((kind) => {
        const mode = getPrefs().comms[kind];
        if (mode === "ask") {
          setAsking(kind);
          commsApi.apps().then(setApps).catch(() => setApps(null));
        } else open(kind, mode);
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `open` usa lo último de las preferencias al llamarse
    [],
  );

  if (!asking) return null;
  const installed = asking === "teams" ? apps?.teams : !!apps?.mail;
  const pick = (mode: Exclude<CommMode, "ask">) => {
    setPrefs({ comms: { ...getPrefs().comms, [asking]: mode } });
    setAsking(null);
    open(asking, mode);
  };

  return (
    <Modal title={`¿Cómo quieres abrir ${COMM_NAME[asking]}?`} onClose={() => setAsking(null)} width="w-[520px]">
      <div className="space-y-2">
        {COMM_MODES.map((m) => {
          const Icon = ICON[m.id];
          const missing = m.id === "app" && apps !== null && !installed;
          return (
            <button
              key={m.id}
              onClick={() => pick(m.id)}
              disabled={missing}
              className="flex w-full items-start gap-3 rounded-xl border border-line px-3.5 py-3 text-left transition-colors hover:border-neon/60 hover:bg-neon/5 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-panel-2 text-ink">
                <Icon size={17} strokeWidth={1.7} />
              </span>
              <span className="min-w-0">
                <b className="block text-sm font-semibold text-ink">{m.label}</b>
                <span className="block text-xs text-mute">{missing ? `No está instalada en este equipo.` : m.hint}</span>
              </span>
            </button>
          );
        })}
      </div>
      <p className="mt-3 text-[11.5px] text-mute">Se recuerda lo que elijas. Se cambia en Ajustes → Portales y correo.</p>
    </Modal>
  );
}
