import { BookOpen, Bug, Scale, Sparkles, X } from "lucide-react";
import logo from "../assets/logo.svg";
import type { AppInfo } from "../lib/api";
import { openHelp, type HelpTab } from "../lib/help";

const LINKS: { tab: HelpTab; label: string; hint: string; icon: typeof BookOpen }[] = [
  { tab: "guide", label: "Guía", hint: "Qué hace cada pantalla, cómo se usa, glosario y preguntas frecuentes", icon: BookOpen },
  { tab: "news", label: "Novedades", hint: "Lo que trae cada versión", icon: Sparkles },
  { tab: "terms", label: "Términos de uso", hint: "Licencia, responsabilidad y tus datos", icon: Scale },
  { tab: "report", label: "Reportar un problema", hint: "Prepara el correo para el autor con el diagnóstico adjunto", icon: Bug },
];

export function About({ open, onClose, appInfo }: { open: boolean; onClose: () => void; appInfo: AppInfo | null }) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/55" onClick={onClose}>
      <div className="relative w-[440px] max-w-[95vw] rounded-2xl border border-line-2 bg-panel p-7 text-center shadow-2xl" onClick={(e) => e.stopPropagation()}>
        <button onClick={onClose} className="absolute top-4 right-4 text-mute hover:text-ink" title="Cerrar">
          <X size={16} />
        </button>
        <img src={logo} alt="" className="mx-auto mb-4 size-20" />
        <h2 className="text-2xl font-semibold tracking-tight">AdminOps</h2>
        <p className="mt-1 font-mono text-xs text-mute">
          v{appInfo?.version ?? "…"}
          {appInfo?.portable && " · portable"}
        </p>
        <p className="mt-4 text-sm text-dim">Diagnóstico, optimización y servicio técnico para Windows.</p>

        <div className="mt-5 grid grid-cols-2 gap-2">
          {LINKS.map((l) => (
            <button
              key={l.tab}
              onClick={() => {
                onClose();
                openHelp(l.tab);
              }}
              title={l.hint}
              className="flex items-center gap-2 rounded-lg border border-line px-3 py-2.5 text-left text-[13px] text-dim transition-colors hover:border-neon/40 hover:text-ink"
            >
              <l.icon size={15} strokeWidth={1.7} className="shrink-0 text-neon" /> {l.label}
            </button>
          ))}
        </div>

        <div className="mt-5 rounded-xl border border-neon/25 bg-neon/5 px-4 py-3">
          <div className="text-[11px] text-mute">Creado por</div>
          <div className="mt-0.5 text-lg font-semibold text-neon">David Bonilla</div>
        </div>
        <p className="mt-5 text-[11px] text-mute">© {new Date().getFullYear()} David Bonilla. Todos los derechos reservados.</p>
      </div>
    </div>
  );
}
