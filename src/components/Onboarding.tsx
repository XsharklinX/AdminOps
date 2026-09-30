import { ArrowLeft, ArrowRight, Building2, Check, Mail, ShieldCheck, Stethoscope, Ticket, TriangleAlert, UserRound, Wrench } from "lucide-react";
import { useEffect, useState } from "react";
import logo from "../assets/logo.svg";
import { Button, inputClass } from "./ui";
import { portalsApi, workApi, type Settings } from "../lib/api";
import { setPrefs, type AppMode } from "../lib/prefs";

type Step = "mode" | "tech" | "tickets" | "mail" | "domain" | "done";

const TECH_STEPS: Step[] = ["mode", "tech", "tickets", "mail", "domain", "done"];
const USER_STEPS: Step[] = ["mode", "done"];

const TITLE: Record<Step, string> = {
  mode: "¿Quién va a usar AdminOps en este equipo?",
  tech: "Tus datos",
  tickets: "Tus tickets",
  mail: "Tu correo",
  domain: "El dominio de tu empresa",
  done: "Listo",
};

const ICON: Record<Step, typeof UserRound> = {
  mode: ShieldCheck,
  tech: UserRound,
  tickets: Ticket,
  mail: Mail,
  domain: Building2,
  done: Stethoscope,
};

/**
 * Bienvenida: primero para quién es esta instalación (el técnico o la persona
 * que usa el equipo) y, si es del técnico, sus datos, sus portales y su dominio.
 * Todo es opcional y se cambia luego en Ajustes.
 */
export function Onboarding({ onDone }: { onDone: (goTo: "diagnostics" | "dashboard") => void }) {
  const [mode, setMode] = useState<AppMode>("admin");
  const [i, setI] = useState(0);
  const [s, setS] = useState<Settings | null>(null);
  const [portal, setPortal] = useState({ name: "", url: "" });
  const [mail, setMail] = useState<"" | "work" | "personal">("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void workApi.settings().then(setS);
  }, []);

  if (!s) return null;

  const steps = mode === "user" ? USER_STEPS : TECH_STEPS;
  const step = steps[Math.min(i, steps.length - 1)];

  const finish = async (goTo: "diagnostics" | "dashboard") => {
    setError(null);
    setSaving(true);
    try {
      if (mode === "admin" && mail) {
        const preset = mail === "work" ? "https://outlook.office.com/mail/" : "https://outlook.live.com/mail/";
        await portalsApi.save({ id: "", name: "Correo", url: preset, extraDomains: [], kind: "mail", private: true, autofill: true });
      }
      setPrefs({ mode });
      await workApi.saveSettings({ ...s, onboarded: true });
      onDone(mode === "user" ? "dashboard" : goTo);
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  };

  const next = async () => {
    setError(null);
    if (step === "tickets" && portal.url.trim()) {
      try {
        await portalsApi.save({ id: "", name: portal.name.trim() || "Tickets", url: portal.url, extraDomains: [] });
      } catch (e) {
        return setError(String(e));
      }
    }
    setI((n) => n + 1);
  };

  const Icon = ICON[step];

  return (
    <div className="fixed inset-0 z-[90] grid place-items-center bg-void/95 p-6">
      <div className="w-[640px] max-w-full rounded-2xl border border-line-2 bg-panel p-7 shadow-2xl">
        <div className="mb-5 flex items-center gap-3">
          <img src={logo} alt="" className="size-9" draggable={false} />
          <div className="min-w-0 flex-1">
            <h1 className="text-lg font-semibold tracking-tight text-ink">{TITLE[step]}</h1>
            <p className="text-xs text-mute">
              Paso {steps.indexOf(step) + 1} de {steps.length} · todo se puede cambiar luego en Ajustes
            </p>
          </div>
          <Icon size={22} className="shrink-0 text-neon" />
        </div>

        {step === "mode" && <ModeStep mode={mode} onChange={setMode} />}

        {step === "tech" && (
          <div className="space-y-3">
            <p className="text-sm text-dim">Aparecen en la cabecera de los informes que entregas al cliente.</p>
            <div className="grid grid-cols-2 gap-3">
              <label className="block">
                <span className="mb-1 block text-xs text-dim">Tu nombre</span>
                <input autoFocus value={s.technician} onChange={(e) => setS({ ...s, technician: e.target.value })} placeholder="David Bonilla" className={inputClass} />
              </label>
              <label className="block">
                <span className="mb-1 block text-xs text-dim">Empresa (opcional)</span>
                <input value={s.company} onChange={(e) => setS({ ...s, company: e.target.value })} placeholder="Soporte Técnico" className={inputClass} />
              </label>
              <label className="block">
                <span className="mb-1 block text-xs text-dim">Teléfono</span>
                <input value={s.phone} onChange={(e) => setS({ ...s, phone: e.target.value })} className={inputClass} />
              </label>
              <label className="block">
                <span className="mb-1 block text-xs text-dim">Correo</span>
                <input value={s.email} onChange={(e) => setS({ ...s, email: e.target.value })} className={inputClass} />
              </label>
            </div>
          </div>
        )}

        {step === "tickets" && (
          <div className="space-y-3">
            <p className="text-sm text-dim">La web donde llevas los tickets o el inventario (intranet, GLPI, osTicket…). Se abrirá dentro de AdminOps.</p>
            <div className="grid grid-cols-3 gap-3">
              <label className="block">
                <span className="mb-1 block text-xs text-dim">Nombre</span>
                <input autoFocus value={portal.name} onChange={(e) => setPortal({ ...portal, name: e.target.value })} placeholder="Intranet" className={inputClass} />
              </label>
              <label className="col-span-2 block">
                <span className="mb-1 block text-xs text-dim">Dirección</span>
                <input
                  value={portal.url}
                  onChange={(e) => setPortal({ ...portal, url: e.target.value })}
                  placeholder="https://intranet.empresa.com"
                  className={`${inputClass} font-mono text-xs`}
                />
              </label>
            </div>
            <p className="text-[11px] text-mute">Déjalo vacío si no usas ninguna; se añade después desde Soporte → Tickets.</p>
          </div>
        )}

        {step === "mail" && (
          <div className="space-y-3">
            <p className="text-sm text-dim">Tu Outlook dentro de AdminOps, sin configurarlo en cada equipo. La sesión se cierra al salir.</p>
            <div className="grid gap-2 sm:grid-cols-3">
              {(
                [
                  ["work", "Del trabajo", "Microsoft 365"],
                  ["personal", "Personal", "Outlook.com"],
                  ["", "Ahora no", "Se añade luego"],
                ] as const
              ).map(([id, title, sub]) => (
                <button
                  key={id || "none"}
                  onClick={() => setMail(id)}
                  className={`rounded-lg border px-3 py-2.5 text-left transition-colors ${mail === id ? "border-neon/60 bg-neon/10" : "border-line hover:border-line-2"}`}
                >
                  <div className={`text-sm font-medium ${mail === id ? "text-neon" : "text-ink"}`}>{title}</div>
                  <div className="text-[11px] text-mute">{sub}</div>
                </button>
              ))}
            </div>
          </div>
        )}

        {step === "domain" && (
          <div className="space-y-3">
            <p className="text-sm text-dim">
              Si trabajas con intranets de empresa (por ejemplo <span className="font-mono text-xs">pgr.gob.do</span>), AdminOps entrará en ellas con tu cuenta de
              Windows, sin pedirte contraseña.
            </p>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Dominio habitual (opcional)</span>
              <input
                autoFocus
                value={s.defaultDomain}
                onChange={(e) => setS({ ...s, defaultDomain: e.target.value })}
                placeholder="empresa.local"
                className={`${inputClass} font-mono text-xs`}
              />
            </label>
          </div>
        )}

        {step === "done" && <DoneStep mode={mode} />}

        {error && <p className="mt-4 text-xs text-bad">{error}</p>}

        <div className="mt-6 flex items-center gap-2">
          {i > 0 && (
            <Button kind="ghost" onClick={() => setI((n) => n - 1)}>
              <ArrowLeft size={14} /> Atrás
            </Button>
          )}
          {step !== "done" ? (
            <div className="ml-auto flex gap-2">
              {step !== "mode" && (
                <Button kind="ghost" onClick={() => setI(steps.length - 1)}>
                  Saltar el resto
                </Button>
              )}
              <Button onClick={next}>
                Continuar <ArrowRight size={14} />
              </Button>
            </div>
          ) : (
            <div className="ml-auto flex gap-2">
              {mode === "admin" && (
                <Button kind="ghost" onClick={() => finish("dashboard")} disabled={saving}>
                  Ir al Panel
                </Button>
              )}
              <Button onClick={() => finish(mode === "admin" ? "diagnostics" : "dashboard")} disabled={saving}>
                <Check size={14} /> {mode === "admin" ? "Empezar con un diagnóstico" : "Empezar"}
              </Button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function ModeStep({ mode, onChange }: { mode: AppMode; onChange: (m: AppMode) => void }) {
  const card = (id: AppMode, icon: React.ReactNode, title: string, sub: string, points: string[]) => (
    <button
      onClick={() => onChange(id)}
      className={`flex flex-col gap-2 rounded-xl border p-4 text-left transition-colors ${mode === id ? "border-neon/60 bg-neon/5" : "border-line hover:border-line-2"}`}
    >
      <span className={`flex items-center gap-2 text-sm font-semibold ${mode === id ? "text-neon" : "text-ink"}`}>
        {icon} {title}
      </span>
      <span className="text-xs text-dim">{sub}</span>
      <ul className="mt-1 space-y-1">
        {points.map((p) => (
          <li key={p} className="flex items-start gap-1.5 text-[11px] text-mute">
            <Check size={11} className="mt-0.5 shrink-0" /> {p}
          </li>
        ))}
      </ul>
    </button>
  );

  return (
    <div className="space-y-3">
      <div className="grid gap-3 sm:grid-cols-2">
        {card("admin", <Wrench size={15} />, "Modo técnico", "Para ti: AdminOps completo.", [
          "Diagnóstico, ajustes de Windows, usuarios y cuentas",
          "Clientes, informes, tickets y correo",
          "Para cambiar el equipo hay que abrirlo como administrador",
        ])}
        {card("user", <UserRound size={15} />, "Modo usuario", "Para quien usa el equipo a diario.", [
          "Ver cómo está el equipo y liberar espacio",
          "Resolver lo típico con pasos guiados",
          "Dejarte entrar por acceso remoto",
        ])}
      </div>
      {mode === "admin" ? (
        <p className="flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-[11px] text-warn">
          <TriangleAlert size={13} className="mt-0.5 shrink-0" />
          El modo técnico toca el registro, los servicios, los usuarios y el arranque de Windows. Está pensado para trabajar, no para curiosear: aplica solo lo que
          entiendas, y usa los puntos de restauración y el «Deshacer» del Historial.
        </p>
      ) : (
        <p className="flex items-start gap-2 rounded-lg border border-line bg-void/40 px-3 py-2 text-[11px] text-mute">
          <ShieldCheck size={13} className="mt-0.5 shrink-0 text-neon" />
          No se verán tus clientes, contactos, tickets ni correo, ni lo que puede romper el equipo. Es un modo de la interfaz para evitar accidentes: para que no
          se pueda volver al modo técnico sin ti, pon un PIN en Ajustes → Seguridad.
        </p>
      )}
    </div>
  );
}

function DoneStep({ mode }: { mode: AppMode }) {
  return (
    <div className="space-y-3 text-sm text-dim">
      {mode === "admin" ? (
        <>
          <p>Todo listo. Un par de cosas que te ahorrarán tiempo:</p>
          <ul className="space-y-1.5 text-[13px]">
            <li className="flex gap-2">
              <span className="text-neon">·</span> <span className="text-ink">Ctrl+K</span> busca páginas, ajustes, herramientas, contactos y soluciones.
            </li>
            <li className="flex gap-2">
              <span className="text-neon">·</span> El <span className="text-ink">?</span> junto a cada título explica para qué sirve esa página.
            </li>
            <li className="flex gap-2">
              <span className="text-neon">·</span> AdminOps trae <span className="text-ink">24 soluciones probadas</span> con botones para aplicarlas.
            </li>
            <li className="flex gap-2">
              <span className="text-neon">·</span> El <span className="text-ink">Historial</span> deshace lo que cambies.
            </li>
          </ul>
        </>
      ) : (
        <>
          <p>Listo. Desde aquí puedes:</p>
          <ul className="space-y-1.5 text-[13px]">
            <li className="flex gap-2">
              <span className="text-neon">·</span> Ver en el <span className="text-ink">Panel</span> cómo está tu equipo.
            </li>
            <li className="flex gap-2">
              <span className="text-neon">·</span> Usar <span className="text-ink">Solucionar problemas</span> si algo no va.
            </li>
            <li className="flex gap-2">
              <span className="text-neon">·</span> Dar acceso a tu técnico desde <span className="text-ink">Acceso remoto</span>.
            </li>
          </ul>
          <p className="text-xs text-mute">Si eres el técnico y quieres AdminOps completo, cámbialo en Ajustes → General.</p>
        </>
      )}
    </div>
  );
}
