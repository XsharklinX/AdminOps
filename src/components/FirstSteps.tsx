// Primeros pasos: lo que un técnico nuevo tiene que dejar listo, con su
// casilla que se marca sola al hacerlo y el botón que lleva a cada sitio.
// Desaparece al completarlo o al ocultarlo. Solo en modo administrador (en
// modo usuario el Panel lo ve el cliente).
import { Check, ChevronRight, Rocket, Upload, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useCompanyImport } from "./CompanyConfig";
import { autoBackupApi, graphApi, lockApi, portalsApi, workApi } from "../lib/api";
import { goToPage } from "../lib/navigate";
import { usePrefs } from "../lib/prefs";

const HIDDEN = "adminops.firstSteps.hidden";

interface Step {
  id: string;
  title: string;
  why: string;
  done: boolean;
  go: () => void;
}

export function FirstSteps() {
  const prefs = usePrefs();
  const [steps, setSteps] = useState<Step[] | null>(null);
  const [hidden, setHidden] = useState(() => {
    try {
      return localStorage.getItem(HIDDEN) === "1";
    } catch {
      return false;
    }
  });

  const load = useCallback(async () => {
    const [s, portals, graph, lock, backup] = await Promise.all([
      workApi.settings().catch(() => null),
      portalsApi.list().catch(() => []),
      graphApi.status().catch(() => null),
      lockApi.status().catch(() => null),
      autoBackupApi.info().catch(() => null),
    ]);
    setSteps([
      { id: "company", title: "Tus datos y los de la empresa", why: "Salen en los informes y presupuestos.", done: !!s?.company.trim() && !!s?.technician.trim(), go: () => goToPage("settings", "general") },
      { id: "portals", title: "Tickets, Correo y Teams", why: "Los portales de la empresa dentro de AdminOps.", done: portals.length > 0, go: () => goToPage("settings", "portals") },
      { id: "m365", title: "Conectar Microsoft 365", why: "Inicios de sesión, MFA, presencia de Teams y el calendario.", done: !!graph?.connected, go: () => goToPage("settings", "portals") },
      { id: "lock", title: "Bloqueo con PIN", why: "Protege la app y, en el pendrive, tus contraseñas guardadas.", done: !!lock?.enabled, go: () => goToPage("settings", "security") },
      { id: "backup", title: "Copia automática", why: "Si pierdes el pendrive o el equipo, no pierdes nada.", done: !!backup?.enabled, go: () => goToPage("settings", "general") },
    ]);
  }, []);
  const imp = useCompanyImport(() => void load());

  useEffect(() => {
    if (hidden || prefs.mode === "user") return;
    void load();
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, [hidden, prefs.mode, load]);

  if (hidden || prefs.mode === "user" || !steps) return null;
  const done = steps.filter((s) => s.done).length;
  if (done === steps.length) return null;

  const hide = () => {
    setHidden(true);
    try {
      localStorage.setItem(HIDDEN, "1");
    } catch {
      /* sin almacenamiento */
    }
  };

  return (
    <section className="rounded-xl border border-neon/30 bg-panel">
      <header className="flex flex-wrap items-center gap-2 px-5 pt-4 pb-2">
        <Rocket size={15} className="text-neon" />
        <h2 className="text-sm font-semibold text-ink">Primeros pasos</h2>
        <span className="text-xs text-mute">
          {done} de {steps.length}
        </span>
        <span className="mx-2 h-1.5 w-28 overflow-hidden rounded-full bg-line">
          <span className="block h-full bg-neon" style={{ width: `${(done / steps.length) * 100}%` }} />
        </span>
        <button onClick={() => void imp.start()} className="ml-auto flex items-center gap-1 rounded-md px-2 py-1 text-xs text-neon hover:bg-neon/10" title="Si te pasaron el archivo de configuración de la empresa">
          <Upload size={12} /> Importar la configuración de la empresa
        </button>
        <button onClick={hide} className="rounded-md p-1 text-mute hover:text-ink" title="Ocultar (vuelve desde Ajustes → General)">
          <X size={14} />
        </button>
      </header>
      <ul className="grid gap-1 px-3 pb-3 sm:grid-cols-2 lg:grid-cols-5">
        {steps.map((s) => (
          <li key={s.id}>
            <button
              onClick={s.go}
              className={`flex h-full w-full items-start gap-2 rounded-lg px-2 py-2 text-left transition-colors hover:bg-panel-2 ${s.done ? "opacity-60" : ""}`}
            >
              <span className={`mt-0.5 grid size-4 shrink-0 place-items-center rounded-full border ${s.done ? "border-ok bg-ok text-void" : "border-line-2"}`}>{s.done && <Check size={10} />}</span>
              <span className="min-w-0 flex-1">
                <span className={`block text-sm ${s.done ? "text-dim line-through" : "text-ink"}`}>{s.title}</span>
                <span className="block text-[11px] text-mute">{s.why}</span>
              </span>
              {!s.done && <ChevronRight size={13} className="mt-1 shrink-0 text-mute" />}
            </button>
          </li>
        ))}
      </ul>
      {imp.dialog}
    </section>
  );
}

/** Volver a enseñar Primeros pasos (Ajustes → General). */
export function showFirstStepsAgain() {
  try {
    localStorage.removeItem(HIDDEN);
  } catch {
    /* sin almacenamiento */
  }
}
