import logo from "../assets/logo.svg";
import type { AppInfo, TargetUser } from "../lib/api";
import {
  Activity,
  Brush,
  CircuitBoard,
  ClipboardCheck,
  Cog,
  Cpu,
  Download,
  FileText,
  Gauge,
  HardDrive,
  HardDriveDownload,
  History,
  Layers,
  Package,
  PackagePlus,
  Power,
  Printer,
  Settings as SettingsIcon,
  ShieldHalf,
  Stethoscope,
  Toolbox,
  UserCog,
  Waypoints,
  Users,
  Wifi,
  Wrench,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "dashboard"
  | "processes"
  | "diagnostics"
  | "hardware"
  | "network"
  | "nettools"
  | "space"
  | "profiles"
  | "cleanup"
  | "privacy"
  | "performance"
  | "bloatware"
  | "services"
  | "startup"
  | "software"
  | "install"
  | "session"
  | "tools"
  | "users"
  | "migrate"
  | "printers"
  | "clients"
  | "repair"
  | "report"
  | "history"
  | "settings";

interface NavItem {
  id: PageId;
  label: string;
  icon: LucideIcon;
}

export const NAV_GROUPS: { title: string; items: NavItem[] }[] = [
  {
    title: "Sistema",
    items: [
      { id: "dashboard", label: "Panel", icon: Activity },
      { id: "processes", label: "Procesos", icon: Cpu },
      { id: "diagnostics", label: "Diagnóstico", icon: Stethoscope },
      { id: "hardware", label: "Hardware", icon: CircuitBoard },
      { id: "network", label: "Red y velocidad", icon: Wifi },
      { id: "nettools", label: "Herramientas de red", icon: Waypoints },
      { id: "space", label: "Espacio en disco", icon: HardDrive },
    ],
  },
  {
    title: "Optimizar",
    items: [
      { id: "profiles", label: "Perfiles", icon: Layers },
      { id: "cleanup", label: "Limpieza", icon: Brush },
      { id: "performance", label: "Rendimiento", icon: Gauge },
      { id: "privacy", label: "Privacidad", icon: ShieldHalf },
      { id: "bloatware", label: "Bloatware", icon: Package },
      { id: "services", label: "Servicios", icon: Cog },
      { id: "startup", label: "Inicio", icon: Power },
      { id: "software", label: "Actualizar software", icon: Download },
      { id: "install", label: "Instalar programas", icon: PackagePlus },
    ],
  },
  {
    title: "Servicio técnico",
    items: [
      { id: "session", label: "Sesión de servicio", icon: ClipboardCheck },
      { id: "clients", label: "Clientes", icon: Users },
      { id: "tools", label: "Herramientas", icon: Toolbox },
      { id: "users", label: "Usuarios locales", icon: UserCog },
      { id: "migrate", label: "Copia de datos", icon: HardDriveDownload },
      { id: "printers", label: "Impresoras", icon: Printer },
      { id: "repair", label: "Reparaciones", icon: Wrench },
      { id: "report", label: "Informe", icon: FileText },
      { id: "history", label: "Historial", icon: History },
      { id: "settings", label: "Ajustes", icon: SettingsIcon },
    ],
  },
];

export const NAV: NavItem[] = NAV_GROUPS.flatMap((g) => g.items);

export function Sidebar({
  active,
  onSelect,
  isAdmin,
  targetUser,
  appInfo,
  sessionActive,
  onAbout,
}: {
  active: PageId;
  onSelect: (id: PageId) => void;
  isAdmin: boolean | null;
  targetUser: TargetUser | null;
  appInfo: AppInfo | null;
  sessionActive: boolean;
  onAbout: () => void;
}) {
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-line bg-panel/60">
      <button onClick={onAbout} className="flex items-center gap-2.5 px-5 pt-4 pb-3 text-left" title="Acerca de AdminOps">
        <img src={logo} alt="" className="size-9 drop-shadow-[0_0_10px_rgba(34,225,255,0.35)]" draggable={false} />
        <div>
          <div className="text-[15px] font-semibold tracking-tight">AdminOps</div>
          <div className="flex items-center gap-1.5 text-[10px] tracking-widest text-mute uppercase">
            {appInfo ? `v${appInfo.version}` : "…"}
            {appInfo?.portable && (
              <span
                className="rounded border border-neon/40 px-1 tracking-normal text-neon normal-case"
                title="Modo portable: los datos se guardan junto a AdminOps.exe, no en este equipo"
              >
                portable
              </span>
            )}
          </div>
        </div>
      </button>

      <nav className="flex flex-1 flex-col overflow-y-auto px-3 pb-2">
        {NAV_GROUPS.map((g) => (
          <div key={g.title} className="mb-1">
            <div className="px-3 pt-1 pb-px text-[10px] font-semibold tracking-[0.14em] text-mute uppercase">{g.title}</div>
            {g.items.map(({ id, label, icon: Icon }) => {
              const on = id === active;
              return (
                <button
                  key={id}
                  ref={on ? (el) => el?.scrollIntoView({ block: "nearest" }) : undefined}
                  onClick={() => onSelect(id)}
                  className={`group relative flex w-full items-center gap-3 rounded-lg px-3 py-[2px] text-left text-[13px] transition-colors ${
                    on ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2 hover:text-ink"
                  }`}
                >
                  {on && <span className="absolute top-1.5 bottom-1.5 left-0 w-0.5 rounded-full bg-neon shadow-[0_0_8px_var(--color-neon)]" />}
                  <Icon size={15} strokeWidth={1.8} />
                  <span className="flex-1">{label}</span>
                  {id === "session" && sessionActive && (
                    <span className="size-2 animate-pulse rounded-full bg-ok" title="Sesión en curso" />
                  )}
                </button>
              );
            })}
          </div>
        ))}
      </nav>

      <div className="mx-3 mb-1.5 rounded-lg border border-line bg-void/60 px-3 py-2">
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
      <button onClick={onAbout} className="mb-2 text-center text-[10px] tracking-wide text-mute transition-colors hover:text-neon">
        by <span className="font-semibold">David Bonilla</span>
      </button>
    </aside>
  );
}
