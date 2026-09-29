import { Eye, EyeOff } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "./feedback";
import { auditApi } from "../lib/api";

const EVENT = "adminops:audit";

/** Estado del modo auditoría compartido entre el botón y la franja de aviso. */
function useAudit(): [boolean, (on: boolean) => Promise<void>] {
  const [on, setOn] = useState(false);
  useEffect(() => {
    auditApi.get().then(setOn).catch(() => {});
    const f = (e: Event) => setOn((e as CustomEvent<boolean>).detail);
    window.addEventListener(EVENT, f);
    return () => window.removeEventListener(EVENT, f);
  }, []);
  const set = async (v: boolean) => {
    await auditApi.set(v);
    window.dispatchEvent(new CustomEvent(EVENT, { detail: v }));
  };
  return [on, set];
}

/** Botón de la barra superior. */
export function AuditToggle() {
  const [on, set] = useAudit();
  const toast = useToast();
  return (
    <button
      onClick={() =>
        set(!on)
          .then(() => toast("info", on ? "Modo auditoría desactivado: AdminOps vuelve a poder hacer cambios." : "Modo auditoría: AdminOps solo mira, no cambia nada del equipo."))
          .catch((e) => toast("error", String(e)))
      }
      className={`flex items-center gap-1.5 rounded-md px-2 py-1 text-xs transition-colors ${on ? "bg-warn/15 text-warn" : "text-mute hover:bg-panel-2 hover:text-ink"}`}
      title="Modo auditoría: revisar un equipo sin cambiar nada"
    >
      {on ? <Eye size={13} /> : <EyeOff size={13} />} {on ? "Auditoría" : "Solo mirar"}
    </button>
  );
}

/** Franja visible mientras el modo está activo. */
export function AuditBanner() {
  const [on, set] = useAudit();
  if (!on) return null;
  return (
    <div className="flex items-center gap-3 border-b border-warn/40 bg-warn/10 px-8 py-2 text-sm text-warn">
      <Eye size={15} />
      <span className="flex-1">Modo auditoría: AdminOps solo mira. Ninguna acción cambiará este equipo (diagnósticos, informes y tus datos sí funcionan).</span>
      <button onClick={() => set(false)} className="rounded border border-warn/50 px-2 py-0.5 text-xs hover:bg-warn/15">
        Desactivar
      </button>
    </div>
  );
}
