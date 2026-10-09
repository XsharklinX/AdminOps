import { EyeOff, ShieldCheck } from "lucide-react";
import { useEffect } from "react";
import { useToast } from "./feedback";
import { setPrivacy, usePrivacy } from "../lib/privacy";

/** Botón de la barra superior. Ctrl+Alt+P lo activa y lo desactiva desde cualquier pantalla. */
export function PrivacyToggle() {
  const on = usePrivacy();
  const toast = useToast();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.altKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        setPrivacy(!on);
        toast("info", on ? "Modo privacidad desactivado." : "Modo privacidad: correos, IP y claves quedan difuminados en pantalla.");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [on, toast]);
  return (
    <button
      type="button"
      onClick={() => setPrivacy(!on)}
      className={`flex items-center gap-1.5 rounded-md px-2 py-1 text-xs transition-colors ${on ? "bg-neon/15 text-neon" : "text-mute hover:bg-panel-2 hover:text-ink"}`}
      title="Modo privacidad (Ctrl+Alt+P): difumina correos, IP, usuarios y claves para compartir la pantalla"
      aria-pressed={on}
    >
      {on ? <ShieldCheck size={13} /> : <EyeOff size={13} />} {on ? "Privado" : "Privacidad"}
    </button>
  );
}

/** Franja visible mientras el modo está activo: así no se comparte la pantalla creyendo que no lo está. */
export function PrivacyBanner() {
  const on = usePrivacy();
  if (!on) return null;
  return (
    <div className="flex items-center gap-3 border-b border-neon/40 bg-neon/10 px-8 py-1.5 text-xs text-neon">
      <ShieldCheck size={14} />
      <span className="flex-1">Modo privacidad: correos, IP, MAC, rutas de red, carpetas de usuario y claves de recuperación se ven difuminados. Los datos no cambian.</span>
      <button type="button" onClick={() => setPrivacy(false)} className="rounded border border-neon/50 px-2 py-0.5 hover:bg-neon/15">
        Desactivar
      </button>
    </div>
  );
}
