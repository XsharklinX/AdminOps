import logo from "../assets/logo.svg";
import type { AppInfo, TargetUser } from "../lib/api";
import {
  Gauge,
  Headset,
  Monitor,
  Network,
  Package,
  Search,
  Settings as SettingsIcon,
  SlidersHorizontal,
  Wrench,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "dashboard"
  | "diagnostics"
  | "security"
  | "hardware"
  | "processes"
  | "space"
  | "profiles"
  | "cleanup"
  | "performance"
  | "privacy"
  | "services"
  | "startup"
  | "bloatware"
  | "software"
  | "install"
  | "uninstall"
  | "winupdate"
  | "network"
  | "nettools"
  | "session"
  | "tickets"
  | "clients"
  | "report"
  | "history"
  | "shortcuts"
  | "tools"
  | "users"
  | "domain"
  | "migrate"
  | "printers"
  | "repair"
  | "settings";

/** Cada página: su nombre completo (búsqueda, títulos) y el corto de su pestaña. */
interface NavItem {
  id: PageId;
  label: string;
  tab: string;
}

export const NAV: NavItem[] = [
  { id: "dashboard", label: "Panel", tab: "Panel" },
  { id: "diagnostics", label: "Diagnóstico", tab: "Diagnóstico" },
  { id: "security", label: "Seguridad", tab: "Seguridad" },
  { id: "hardware", label: "Hardware", tab: "Hardware" },
  { id: "processes", label: "Procesos", tab: "Procesos" },
  { id: "space", label: "Espacio en disco", tab: "Espacio en disco" },
  { id: "profiles", label: "Perfiles", tab: "Perfiles" },
  { id: "cleanup", label: "Limpieza", tab: "Limpieza" },
  { id: "performance", label: "Rendimiento", tab: "Rendimiento" },
  { id: "privacy", label: "Privacidad", tab: "Privacidad" },
  { id: "services", label: "Servicios", tab: "Servicios" },
  { id: "startup", label: "Inicio de Windows", tab: "Inicio" },
  { id: "bloatware", label: "Bloatware", tab: "Bloatware" },
  { id: "software", label: "Actualizar programas", tab: "Actualizar" },
  { id: "install", label: "Instalar programas", tab: "Instalar" },
  { id: "uninstall", label: "Desinstalar programas", tab: "Desinstalar" },
  { id: "winupdate", label: "Windows Update", tab: "Windows Update" },
  { id: "network", label: "Velocidad y diagnóstico de red", tab: "Velocidad y diagnóstico" },
  { id: "nettools", label: "Herramientas de red", tab: "Herramientas" },
  { id: "session", label: "Sesión de servicio", tab: "Sesión" },
  { id: "tickets", label: "Tickets", tab: "Tickets" },
  { id: "clients", label: "Clientes", tab: "Clientes" },
  { id: "report", label: "Informe", tab: "Informe" },
  { id: "history", label: "Historial", tab: "Historial" },
  { id: "shortcuts", label: "Atajos de teclado", tab: "Atajos" },
  { id: "tools", label: "Herramientas de Windows", tab: "Herramientas" },
  { id: "users", label: "Usuarios locales", tab: "Usuarios" },
  { id: "domain", label: "Dominio", tab: "Dominio" },
  { id: "migrate", label: "Copia de datos", tab: "Copia de datos" },
  { id: "printers", label: "Impresoras", tab: "Impresoras" },
  { id: "repair", label: "Reparaciones", tab: "Reparaciones" },
  { id: "settings", label: "Ajustes", tab: "Ajustes" },
];

export interface Area {
  id: string;
  label: string;
  icon: LucideIcon;
  pages: PageId[];
}

/** Las 7 áreas de la barra lateral; sus páginas son pestañas. */
export const AREAS: Area[] = [
  { id: "panel", label: "Panel", icon: Gauge, pages: ["dashboard"] },
  { id: "equipo", label: "Equipo", icon: Monitor, pages: ["diagnostics", "security", "hardware", "processes", "space"] },
  { id: "optimizar", label: "Optimizar", icon: SlidersHorizontal, pages: ["profiles", "cleanup", "performance", "privacy", "services", "startup", "bloatware"] },
  { id: "programas", label: "Programas", icon: Package, pages: ["software", "install", "uninstall", "winupdate"] },
  { id: "red", label: "Red", icon: Network, pages: ["network", "nettools"] },
  { id: "soporte", label: "Soporte", icon: Headset, pages: ["session", "tickets", "clients", "report", "history", "shortcuts"] },
  { id: "admin", label: "Administración", icon: Wrench, pages: ["tools", "users", "domain", "migrate", "printers", "repair"] },
];

export const areaOf = (page: PageId): Area | null => AREAS.find((a) => a.pages.includes(page)) ?? null;

export function Sidebar({
  active,
  onArea,
  onSelect,
  isAdmin,
  targetUser,
  appInfo,
  sessionActive,
  onAbout,
  onSearch,
}: {
  active: PageId;
  onArea: (area: Area) => void;
  onSelect: (id: PageId) => void;
  isAdmin: boolean | null;
  targetUser: TargetUser | null;
  appInfo: AppInfo | null;
  sessionActive: boolean;
  onAbout: () => void;
  onSearch: () => void;
}) {
  const current = areaOf(active);
  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-line bg-panel">
      <button onClick={onAbout} className="flex items-center gap-2.5 px-5 pt-5 pb-4 text-left" title="Acerca de AdminOps">
        <img src={logo} alt="" className="size-8" draggable={false} />
        <div className="min-w-0">
          <div className="text-[15px] font-semibold tracking-tight text-ink">AdminOps</div>
          <div className="flex items-center gap-1.5 text-xs text-mute">
            {appInfo ? `Versión ${appInfo.version}` : "…"}
            {appInfo?.portable && (
              <span className="rounded border border-line-2 px-1 text-[11px] text-dim" title="Modo portable: los datos se guardan junto a AdminOps.exe, no en este equipo">
                portable
              </span>
            )}
          </div>
        </div>
      </button>

      <div className="px-3.5 pb-3">
        <button
          onClick={onSearch}
          className="flex h-9 w-full items-center gap-2 rounded-lg border border-line bg-void px-2.5 text-left text-[13px] text-mute transition-colors hover:border-line-2 hover:text-dim"
          title="Buscar páginas, herramientas, ajustes y reparaciones"
        >
          <Search size={15} strokeWidth={1.6} />
          <span className="flex-1">Buscar o ejecutar</span>
          <kbd className="rounded border border-line-2 px-1.5 font-sans text-[11px]">Ctrl K</kbd>
        </button>
      </div>

      <nav aria-label="Navegación principal" className="flex flex-1 flex-col gap-0.5 overflow-y-auto px-2.5">
        {AREAS.map((a) => {
          const on = current?.id === a.id;
          const Icon = a.icon;
          return (
            <button
              key={a.id}
              onClick={() => onArea(a)}
              aria-current={on ? "page" : undefined}
              className={`flex h-9 w-full items-center gap-3 rounded-lg px-3 text-left text-sm transition-colors ${
                on ? "bg-panel-2 font-medium text-ink shadow-[inset_2px_0_0_var(--color-neon)]" : "text-dim hover:bg-panel-2/60 hover:text-ink"
              }`}
            >
              <Icon size={18} strokeWidth={1.6} className={on ? "text-ink" : "text-mute"} />
              <span className="flex-1">{a.label}</span>
            </button>
          );
        })}
      </nav>

      {sessionActive && (
        <button
          onClick={() => onSelect("session")}
          className="mx-3.5 mb-2.5 flex flex-col gap-0.5 rounded-lg border border-line px-3 py-2.5 text-left transition-colors hover:border-line-2"
        >
          <span className="flex items-center gap-1.5 text-xs text-mute">
            <span className="size-1.5 rounded-full bg-ok" /> Sesión de servicio
          </span>
          <span className="text-[13px] text-ink">En curso · ver checklist</span>
        </button>
      )}

      <div className="flex items-center gap-2.5 border-t border-line px-5 py-3.5">
        <span className={`size-2 shrink-0 rounded-full ${isAdmin ? "bg-ok" : "bg-warn"}`} />
        <div className="min-w-0 flex-1">
          <div className="text-[13px] text-ink">{isAdmin === null ? "Comprobando…" : isAdmin ? "Administrador" : "Usuario estándar"}</div>
          {targetUser && (
            <div className="truncate text-[11px] text-mute" title={`Los ajustes de usuario (HKCU) se aplican a ${targetUser.name} (${targetUser.sid})`}>
              {targetUser.name}
              {targetUser.redirected && " · sesión activa"}
            </div>
          )}
        </div>
        <button
          onClick={() => onSelect("settings")}
          className={`grid size-8 place-items-center rounded-md transition-colors hover:bg-panel-2 ${active === "settings" ? "text-ink" : "text-mute hover:text-ink"}`}
          title="Ajustes (Ctrl+,)"
          aria-label="Ajustes"
        >
          <SettingsIcon size={17} strokeWidth={1.6} />
        </button>
      </div>
      <button onClick={onAbout} className="pb-3 text-center text-[11px] text-mute transition-colors hover:text-dim">
        por David Bonilla
      </button>
    </aside>
  );
}
