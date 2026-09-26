import logo from "../assets/logo.svg";
import type { AppInfo, TargetUser } from "../lib/api";
import {
  Activity,
  Brush,
  Cog,
  FileText,
  Gauge,
  History,
  Layers,
  Package,
  Power,
  ShieldHalf,
  Stethoscope,
  Wrench,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "dashboard"
  | "profiles"
  | "cleanup"
  | "privacy"
  | "performance"
  | "bloatware"
  | "services"
  | "startup"
  | "diagnostics"
  | "repair"
  | "report"
  | "history";

export const NAV: { id: PageId; label: string; icon: LucideIcon }[] = [
  { id: "dashboard", label: "Panel", icon: Activity },
  { id: "profiles", label: "Perfiles", icon: Layers },
  { id: "cleanup", label: "Limpieza", icon: Brush },
  { id: "performance", label: "Rendimiento", icon: Gauge },
  { id: "privacy", label: "Privacidad", icon: ShieldHalf },
  { id: "bloatware", label: "Bloatware", icon: Package },
  { id: "services", label: "Servicios", icon: Cog },
  { id: "startup", label: "Inicio", icon: Power },
  { id: "diagnostics", label: "Diagnóstico", icon: Stethoscope },
  { id: "repair", label: "Reparaciones", icon: Wrench },
  { id: "report", label: "Informe", icon: FileText },
  { id: "history", label: "Historial", icon: History },
];

export function Sidebar({
  active,
  onSelect,
  isAdmin,
  targetUser,
  appInfo,
}: {
  active: PageId;
  onSelect: (id: PageId) => void;
  isAdmin: boolean | null;
  targetUser: TargetUser | null;
  appInfo: AppInfo | null;
}) {
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-line bg-panel/60">
      <div className="flex items-center gap-2.5 px-5 pt-5 pb-6">
        <img src={logo} alt="" className="size-9 drop-shadow-[0_0_10px_rgba(34,225,255,0.35)]" draggable={false} />
        <div>
          <div className="text-[15px] font-semibold tracking-tight">AdminOps</div>
          <div className="flex items-center gap-1.5 text-[10px] tracking-widest text-mute uppercase">
            {appInfo ? `v${appInfo.version}` : "…"}
            {appInfo?.portable && (
              <span
                className="rounded border border-neon/40 px-1 tracking-normal text-neon normal-case"
                title={`Datos en ${appInfo.dataDir}`}
              >
                portable
              </span>
            )}
          </div>
        </div>
      </div>

      <nav className="flex flex-1 flex-col gap-0.5 px-3">
        {NAV.map(({ id, label, icon: Icon }) => {
          const on = id === active;
          return (
            <button
              key={id}
              onClick={() => onSelect(id)}
              className={`group relative flex items-center gap-3 rounded-lg px-3 py-2 text-left text-sm transition-colors ${
                on ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2 hover:text-ink"
              }`}
            >
              {on && <span className="absolute top-1.5 bottom-1.5 left-0 w-0.5 rounded-full bg-neon shadow-[0_0_8px_var(--color-neon)]" />}
              <Icon size={16} strokeWidth={1.8} />
              <span className="flex-1">{label}</span>
            </button>
          );
        })}
      </nav>

      <div className="m-3 rounded-lg border border-line bg-void/60 px-3 py-2.5">
        <div className="flex items-center gap-2 text-xs">
          <span
            className={`size-2 rounded-full ${isAdmin ? "bg-ok shadow-[0_0_8px_var(--color-ok)]" : "bg-warn shadow-[0_0_8px_var(--color-warn)]"}`}
          />
          <span className={isAdmin ? "text-ok" : "text-warn"}>
            {isAdmin === null ? "Comprobando…" : isAdmin ? "Administrador" : "Usuario estándar"}
          </span>
        </div>
        {targetUser && (
          <div
            className="mt-1.5 truncate text-[11px] text-mute"
            title={`Los ajustes de usuario (HKCU) se aplican a ${targetUser.name} (${targetUser.sid})`}
          >
            Usuario: <span className={targetUser.redirected ? "text-neon" : "text-dim"}>{targetUser.name}</span>
            {targetUser.redirected && <span className="text-neon"> · sesión activa</span>}
          </div>
        )}
      </div>
    </aside>
  );
}
