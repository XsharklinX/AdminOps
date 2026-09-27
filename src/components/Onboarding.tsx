import { ArrowRight, Building2, Check, Stethoscope, Ticket, UserRound } from "lucide-react";
import { useEffect, useState } from "react";
import logo from "../assets/logo.svg";
import { Button, inputClass } from "./ui";
import { portalsApi, workApi, type Settings } from "../lib/api";

const STEPS = [
  { icon: UserRound, title: "Tus datos" },
  { icon: Ticket, title: "Tickets" },
  { icon: Building2, title: "Dominio" },
  { icon: Stethoscope, title: "Listo" },
];

/**
 * Asistente de primer arranque: datos del técnico (para los informes), portal
 * de Tickets y dominio habitual. Todo es opcional y se puede cambiar después.
 */
export function Onboarding({ onDone }: { onDone: (goTo: "diagnostics" | "dashboard") => void }) {
  const [step, setStep] = useState(0);
  const [s, setS] = useState<Settings | null>(null);
  const [portal, setPortal] = useState({ name: "", url: "" });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    workApi.settings().then(setS);
  }, []);

  if (!s) return null;

  const finish = async (goTo: "diagnostics" | "dashboard") => {
    setError(null);
    try {
      await workApi.saveSettings({ ...s, onboarded: true });
      onDone(goTo);
    } catch (e) {
      setError(String(e));
    }
  };

  const next = async () => {
    setError(null);
    if (step === 1 && portal.url.trim()) {
      try {
        await portalsApi.save({ id: "", name: portal.name.trim() || "Tickets", url: portal.url, extraDomains: [] });
      } catch (e) {
        setError(String(e));
        return;
      }
    }
    setStep(step + 1);
  };

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-void/95">
      <div className="w-[560px] rounded-2xl border border-line-2 bg-panel p-7 shadow-2xl">
        <div className="mb-6 flex items-center gap-3">
          <img src={logo} alt="" className="size-10" draggable={false} />
          <div>
            <h2 className="text-lg font-semibold">Bienvenido a AdminOps</h2>
            <p className="text-xs text-mute">Tres datos rápidos y listo. Todo es opcional y se cambia en Ajustes.</p>
          </div>
        </div>

        <div className="mb-6 flex items-center gap-2">
          {STEPS.map(({ icon: Icon, title }, i) => (
            <div key={title} className="flex flex-1 items-center gap-2">
              <span
                className={`grid size-7 shrink-0 place-items-center rounded-full border ${
                  i < step ? "border-ok/60 bg-ok/10 text-ok" : i === step ? "border-neon/60 bg-neon/10 text-neon" : "border-line text-mute"
                }`}
              >
                {i < step ? <Check size={13} /> : <Icon size={13} />}
              </span>
              <span className={`text-xs ${i === step ? "text-ink" : "text-mute"}`}>{title}</span>
              {i < STEPS.length - 1 && <span className="h-px flex-1 bg-line" />}
            </div>
          ))}
        </div>

        <div className="min-h-[170px]">
          {step === 0 && (
            <div className="space-y-3">
              <p className="text-sm text-dim">Aparecen en los informes PDF que entregas al cliente.</p>
              <input autoFocus value={s.technician} onChange={(e) => setS({ ...s, technician: e.target.value })} placeholder="Tu nombre" className={inputClass} />
              <input value={s.company} onChange={(e) => setS({ ...s, company: e.target.value })} placeholder="Empresa o departamento (opcional)" className={inputClass} />
              <div className="grid grid-cols-2 gap-2">
                <input value={s.phone} onChange={(e) => setS({ ...s, phone: e.target.value })} placeholder="Teléfono (opcional)" className={inputClass} />
                <input value={s.email} onChange={(e) => setS({ ...s, email: e.target.value })} placeholder="Correo (opcional)" className={inputClass} />
              </div>
            </div>
          )}
          {step === 1 && (
            <div className="space-y-3">
              <p className="text-sm text-dim">
                ¿Usas una web para tus tickets o soportes (intranet, GLPI, osTicket…)? Se abrirá dentro de AdminOps, en <span className="text-ink">Tickets</span>.
              </p>
              <input autoFocus value={portal.name} onChange={(e) => setPortal({ ...portal, name: e.target.value })} placeholder="Nombre (p. ej. Intranet)" className={inputClass} />
              <input
                value={portal.url}
                onChange={(e) => setPortal({ ...portal, url: e.target.value })}
                placeholder="Dirección (p. ej. intranet.empresa.com)"
                className={`${inputClass} font-mono text-xs`}
              />
            </div>
          )}
          {step === 2 && (
            <div className="space-y-3">
              <p className="text-sm text-dim">Si trabajas con un dominio de Active Directory, se propondrá al unir equipos en la página Dominio.</p>
              <input
                autoFocus
                value={s.defaultDomain}
                onChange={(e) => setS({ ...s, defaultDomain: e.target.value })}
                placeholder="Dominio habitual (p. ej. empresa.local)"
                className={`${inputClass} font-mono`}
              />
            </div>
          )}
          {step === 3 && (
            <div className="space-y-3 text-sm text-dim">
              <p>Todo listo. Un par de trucos:</p>
              <ul className="space-y-1.5">
                <li>
                  <kbd className="rounded border border-line px-1 font-mono text-[11px] text-ink">Ctrl K</kbd> busca cualquier página, herramienta, ajuste o
                  reparación.
                </li>
                <li>La barra lateral tiene 7 áreas: al entrar en una se despliegan sus secciones debajo.</li>
                <li>Todo lo que AdminOps cambia queda en el Historial y se puede deshacer.</li>
              </ul>
              <p>¿Empezamos con un diagnóstico de este equipo?</p>
            </div>
          )}
        </div>

        {error && <p className="mt-2 text-xs text-bad">{error}</p>}
        <div className="mt-6 flex items-center justify-between">
          <button onClick={() => finish("dashboard")} className="text-xs text-mute hover:text-ink">
            {step === 3 ? "Ir al Panel" : "Omitir"}
          </button>
          {step < 3 ? (
            <Button onClick={next}>
              Siguiente <ArrowRight size={14} />
            </Button>
          ) : (
            <Button onClick={() => finish("diagnostics")}>
              <Stethoscope size={14} /> Hacer un diagnóstico
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
