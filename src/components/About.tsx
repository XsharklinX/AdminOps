import { X } from "lucide-react";
import logo from "../assets/logo.svg";
import type { AppInfo } from "../lib/api";

export function About({ open, onClose, appInfo }: { open: boolean; onClose: () => void; appInfo: AppInfo | null }) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div className="relative w-[420px] rounded-2xl border border-line-2 bg-panel p-7 text-center shadow-2xl" onClick={(e) => e.stopPropagation()}>
        <button onClick={onClose} className="absolute top-4 right-4 text-mute hover:text-ink">
          <X size={16} />
        </button>
        <img src={logo} alt="" className="mx-auto mb-4 size-20 drop-shadow-[0_0_24px_rgba(34,225,255,0.35)]" />
        <h2 className="text-2xl font-semibold tracking-tight">AdminOps</h2>
        <p className="mt-1 font-mono text-xs text-mute">
          v{appInfo?.version ?? "…"}
          {appInfo?.portable && " · portable"}
        </p>
        <p className="mt-4 text-sm text-dim">Diagnóstico, optimización y servicio técnico para Windows.</p>
        <div className="mt-6 rounded-xl border border-neon/25 bg-neon/5 px-4 py-3">
          <div className="text-[10px] tracking-[0.2em] text-mute uppercase">Creado por</div>
          <div className="mt-0.5 text-lg font-semibold text-neon">David Bonilla</div>
        </div>
        <p className="mt-5 text-[11px] text-mute">© {new Date().getFullYear()} David Bonilla. Todos los derechos reservados.</p>
        {appInfo && (
          <p className="mt-3 truncate font-mono text-[10px] text-mute" title={appInfo.dataDir}>
            Datos: {appInfo.dataDir}
          </p>
        )}
      </div>
    </div>
  );
}
