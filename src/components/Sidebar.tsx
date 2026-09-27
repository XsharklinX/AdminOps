import { useState } from "react";
import logo from "../assets/logo.svg";
import type { AppInfo, TargetUser } from "../lib/api";
import {
  Activity,
  Brush,
  ChevronRight,
  Building2,
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
  Search,
  ShieldHalf,
  Star,
  Stethoscope,
  Ticket,
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
  | "tickets"
  | "domain"
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
    title: "Soporte",
    items: [
      { id: "session", label: "Sesión de servicio", icon: ClipboardCheck },
      { id: "tickets", label: "Tickets", icon: Ticket },
      { id: "clients", label: "Clientes", icon: Users },
      { id: "report", label: "Informe", icon: FileText },
      { id: "history", label: "Historial", icon: History },
    ],
  },
  {
    title: "Administración",
    items: [
      { id: "tools", label: "Herramientas", icon: Toolbox },
      { id: "users", label: "Usuarios locales", icon: UserCog },
      { id: "domain", label: "Dominio", icon: Building2 },
      { id: "migrate", label: "Copia de datos", icon: HardDriveDownload },
      { id: "printers", label: "Impresoras", icon: Printer },
      { id: "repair", label: "Reparaciones", icon: Wrench },
    ],
  },
];

/** Páginas fuera de los grupos (Ajustes va en el pie de la barra). */
const EXTRA: NavItem[] = [{ id: "settings", label: "Ajustes", icon: SettingsIcon }];

export const NAV: NavItem[] = [...NAV_GROUPS.flatMap((g) => g.items), ...EXTRA];

const FAVS = "adminops.navFavorites";
const COLLAPSED = "adminops.navCollapsed";

function readList(key: string): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(key) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
  } catch {
    return [];
  }
}

function writeList(key: string, list: string[]) {
  try {
    localStorage.setItem(key, JSON.stringify(list));
  } catch {
    /* sin almacenamiento: solo dura esta sesión */
  }
}

export function Sidebar({
  active,
  onSelect,
  isAdmin,
  targetUser,
  appInfo,
  sessionActive,
  onAbout,
  onSearch,
}: {
  active: PageId;
  onSelect: (id: PageId) => void;
  isAdmin: boolean | null;
  targetUser: TargetUser | null;
  appInfo: AppInfo | null;
  sessionActive: boolean;
  onAbout: () => void;
  onSearch: () => void;
}) {
  const [favorites, setFavorites] = useState<PageId[]>(() => readList(FAVS).filter((id) => NAV.some((n) => n.id === id)) as PageId[]);
  const [collapsed, setCollapsed] = useState<string[]>(() => readList(COLLAPSED));

  const toggleFavorite = (id: PageId) => {
    const next = favorites.includes(id) ? favorites.filter((f) => f !== id) : [...favorites, id];
    setFavorites(next);
    writeList(FAVS, next);
  };
  const toggleGroup = (title: string) => {
    const next = collapsed.includes(title) ? collapsed.filter((c) => c !== title) : [...collapsed, title];
    setCollapsed(next);
    writeList(COLLAPSED, next);
  };

  const item = ({ id, label, icon: Icon }: NavItem, group: string) => {
    const on = id === active;
    const fav = favorites.includes(id);
    return (
      <div key={`${group}-${id}`} className="group relative">
        <button
          ref={
            on
              ? (el) => {
                  el?.scrollIntoView({ block: "nearest" });
                }
              : undefined
          }
          onClick={() => onSelect(id)}
          className={`relative flex w-full items-center gap-3 rounded-lg px-3 py-[2px] pr-7 text-left text-[13px] transition-colors ${
            on ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2 hover:text-ink"
          }`}
        >
          {on && <span className="absolute top-1.5 bottom-1.5 left-0 w-0.5 rounded-full bg-neon shadow-[0_0_8px_var(--color-neon)]" />}
          <Icon size={15} strokeWidth={1.8} />
          <span className="flex-1 truncate">{label}</span>
          {id === "session" && sessionActive && <span className="size-2 animate-pulse rounded-full bg-ok" title="Sesión en curso" />}
        </button>
        <button
          onClick={() => toggleFavorite(id)}
          title={fav ? "Quitar de favoritos" : "Fijar en favoritos"}
          className={`absolute top-1/2 right-1.5 -translate-y-1/2 rounded p-0.5 transition-opacity ${
            fav && group === "Favoritos"
              ? "text-warn opacity-0 group-hover:opacity-100"
              : fav
                ? "text-warn opacity-60"
                : "text-mute opacity-0 group-hover:opacity-100 hover:text-ink"
          }`}
        >
          <Star size={11} fill={fav ? "currentColor" : "none"} />
        </button>
      </div>
    );
  };

  const favItems = favorites.map((id) => NAV.find((n) => n.id === id)).filter((n): n is NavItem => !!n);

  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-line bg-panel/60">
      <button onClick={onAbout} className="flex items-center gap-2.5 px-5 pt-4 pb-2 text-left" title="Acerca de AdminOps">
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

      <button
        onClick={onSearch}
        className="mx-3 mb-2 flex items-center gap-2 rounded-lg border border-line bg-void/60 px-3 py-1.5 text-left text-xs text-mute transition-colors hover:border-neon/40 hover:text-ink"
        title="Buscar páginas, herramientas, ajustes y reparaciones"
      >
        <Search size={13} />
        <span className="flex-1">Buscar…</span>
        <kbd className="rounded border border-line px-1 font-mono text-[10px]">Ctrl K</kbd>
      </button>

      <nav className="flex flex-1 flex-col overflow-y-auto px-3 pb-2">
        {favItems.length > 0 && (
          <div className="mb-1">
            <div className="flex items-center gap-1 px-3 pt-1 pb-px text-[10px] font-semibold tracking-[0.14em] text-warn/80 uppercase">
              <Star size={9} fill="currentColor" /> Favoritos
            </div>
            {favItems.map((n) => item(n, "Favoritos"))}
          </div>
        )}
        {NAV_GROUPS.map((g) => {
          const closed = collapsed.includes(g.title);
          // Plegado, pero la página abierta sigue visible.
          const shown = closed ? g.items.filter((n) => n.id === active) : g.items;
          return (
            <div key={g.title} className="mb-1">
              <button
                onClick={() => toggleGroup(g.title)}
                className="flex w-full items-center gap-1 px-3 pt-1 pb-px text-left text-[10px] font-semibold tracking-[0.14em] text-mute uppercase transition-colors hover:text-ink"
                title={closed ? "Mostrar" : "Plegar"}
              >
                <ChevronRight size={10} className={`transition-transform ${closed ? "" : "rotate-90"}`} />
                <span className="flex-1">{g.title}</span>
                {closed && <span className="font-mono tracking-normal normal-case">{g.items.length}</span>}
              </button>
              {shown.map((n) => item(n, g.title))}
            </div>
          );
        })}
      </nav>

      <div className="mx-3 mb-1.5 flex items-start gap-2 rounded-lg border border-line bg-void/60 px-3 py-2">
        <div className="min-w-0 flex-1">
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
        <button
          onClick={() => onSelect("settings")}
          className={`rounded-md p-1 transition-colors ${active === "settings" ? "text-neon" : "text-mute hover:text-ink"}`}
          title="Ajustes (Ctrl+,)"
        >
          <SettingsIcon size={15} />
        </button>
      </div>
      <button onClick={onAbout} className="mb-2 text-center text-[10px] tracking-wide text-mute transition-colors hover:text-neon">
        by <span className="font-semibold">David Bonilla</span>
      </button>
    </aside>
  );
}
