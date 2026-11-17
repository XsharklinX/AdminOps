import { Monitor, MoreHorizontal, PictureInPicture } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { auditApi, logQuietly, workApi } from "../lib/api";
import { usePrivacy } from "../lib/privacy";
import { AuditToggle } from "./AuditMode";
import { PrivacyToggle } from "./PrivacyMode";

/**
 * El menú «⋯» de la barra de arriba: lo que se usa de vez en cuando (privacidad,
 * solo mirar, pantalla de taller). Antes eran botones de texto que pesaban lo
 * mismo que «Nuevo caso». Con algún modo activo, el botón lleva un punto.
 */
export function MoreMenu({ onWorkshop }: { onWorkshop: () => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const privacy = usePrivacy();
  const [audit, setAudit] = useState(false);

  useEffect(() => {
    auditApi.get().then(setAudit).catch(logQuietly("MoreMenu"));
    const f = (e: Event) => setAudit(!!(e as CustomEvent<boolean>).detail);
    window.addEventListener("adminops:audit", f);
    return () => window.removeEventListener("adminops:audit", f);
  }, []);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  const active = privacy || audit;
  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="menu"
        aria-expanded={open}
        title={active ? "Más opciones · hay un modo activo" : "Más opciones: privacidad, solo mirar, pantalla de taller"}
        aria-label="Más opciones"
        className="relative grid size-8 place-items-center rounded-md text-mute transition-colors hover:bg-panel-2 hover:text-ink"
      >
        <MoreHorizontal size={17} strokeWidth={1.8} />
        {active && <span className="absolute top-1 right-1 size-2 rounded-full bg-warn ring-2 ring-panel" />}
      </button>
      {/* Montado siempre (oculto): así el atajo de privacidad funciona aunque el menú esté cerrado. */}
      <div role="menu" hidden={!open} className="absolute top-full right-0 z-40 mt-2 flex w-72 flex-col gap-0.5 rounded-xl border border-line-2 bg-panel p-1.5 shadow-elev-3">
        <PrivacyToggle row />
        <AuditToggle row />
        <button
          type="button"
          role="menuitem"
          className="menu-row"
          onClick={() => {
            setOpen(false);
            onWorkshop();
          }}
        >
          <Monitor size={14} />
          <span className="flex-1">Pantalla de taller</span>
          <kbd className="font-mono text-[10.5px] text-mute">F11</kbd>
        </button>
        <button
          type="button"
          role="menuitem"
          className="menu-row"
          onClick={() => {
            setOpen(false);
            void workApi.miniAdminOps().catch(logQuietly("MoreMenu"));
          }}
        >
          <PictureInPicture size={14} />
          <span className="flex-1">AdminOps en miniatura</span>
          <kbd className="font-mono text-[10.5px] text-mute">Ctrl+Alt+A</kbd>
        </button>
      </div>
    </div>
  );
}
