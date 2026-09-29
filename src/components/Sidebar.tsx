import { useState } from "react";
import logo from "../assets/logo.svg";
import { appcareApi, type AppInfo, type TargetUser, type UpdateInfo } from "../lib/api";
import { getPrefs, setSidebar, usePrefs, type NavLayout } from "../lib/prefs";
import {
  Activity,
  Bookmark,
  Box,
  Briefcase,
  Bug,
  Building2,
  ChevronRight,
  Cloud,
  Cpu,
  Database,
  Folder,
  Gauge,
  Globe,
  HardDrive,
  Headset,
  Heart,
  Home,
  Layers,
  Lock,
  Lock as LockIcon,
  Monitor,
  Network,
  Package,
  Printer,
  Rocket,
  Search,
  Settings as SettingsIcon,
  Shield,
  SlidersHorizontal,
  Star,
  Stethoscope,
  Terminal,
  Users,
  Wifi,
  Wrench,
  Zap,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "dashboard"
  | "diagnostics"
  | "troubleshoot"
  | "security"
  | "hardware"
  | "processes"
  | "space"
  | "profiles"
  | "tweaks"
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
  | "router"
  | "devices"
  | "network"
  | "nettools"
  | "session"
  | "tickets"
  | "mail"
  | "agenda"
  | "clients"
  | "contacts"
  | "knowledge"
  | "recipes"
  | "stations"
  | "report"
  | "history"
  | "shortcuts"
  | "tools"
  | "users"
  | "accounts"
  | "domain"
  | "migrate"
  | "printers"
  | "repair"
  | "vault"
  | "wipe"
  | "recover"
  | "family"
  | "inventory"
  | "shares"
  | "remote"
  | "settings";

/** Cada página: su nombre completo (búsqueda, títulos) y el corto de su pestaña. */
interface NavItem {
  id: PageId;
  label: string;
  tab: string;
  /** Qué es y para qué sirve (ayuda junto al título). */
  help: string;
}

export const NAV: NavItem[] = [
  // Inicio
  { id: "dashboard", label: "Panel", tab: "Panel", help: "El equipo en vivo y lo que conviene hacer ahora, en orden de importancia." },
  { id: "troubleshoot", label: "Solucionar problemas", tab: "Solucionar", help: "Eliges el síntoma (no hay Internet, no suena, va lento…) y AdminOps revisa las causas típicas y ofrece la reparación. Abajo, las reparaciones de Windows (SFC, DISM, red, Windows Update…)." },
  { id: "session", label: "Sesión de servicio", tab: "Sesión", help: "Una visita de principio a fin: diagnóstico antes y después, checklist que se marca sola, presupuesto, firma e informe archivado en la ficha del cliente." },
  // Equipo
  { id: "diagnostics", label: "Diagnóstico", tab: "Diagnóstico", help: "Revisa discos, estabilidad, drivers, batería, seguridad y actualizaciones en menos de un minuto, con lo que hay que hacer para cada problema." },
  { id: "hardware", label: "Hardware", tab: "Hardware", help: "Ficha del equipo para el inventario, piezas, temperaturas, salud de los discos (SMART) y memoria." },
  { id: "security", label: "Seguridad", tab: "Seguridad", help: "Nota de seguridad del equipo, antivirus, BitLocker, cuentas, elementos sospechosos que arrancan solos y extensiones del navegador." },
  { id: "processes", label: "Procesos", tab: "Procesos", help: "Qué está usando procesador, memoria y disco ahora mismo, y finalizar lo que se cuelga." },
  { id: "space", label: "Espacio en disco", tab: "Espacio en disco", help: "Qué ocupa el disco, carpeta por carpeta, para liberar espacio con criterio." },
  { id: "history", label: "Historial", tab: "Historial", help: "Línea de tiempo del equipo (cambios, avisos, actualizaciones, drivers, apagados), diario de cambios con «Deshacer» y puntos de restauración." },
  // Mantener
  {
    id: "tweaks",
    label: "Ajustes de Windows",
    tab: "Ajustes",
    help: "Limpieza, rendimiento, privacidad y servicios en un solo sitio, con buscador. Cada ajuste explica qué hace y se puede deshacer.",
  },
  { id: "startup", label: "Inicio de Windows", tab: "Inicio", help: "Programas que arrancan con Windows: desactivar los que sobran acelera el arranque." },
  { id: "bloatware", label: "Bloatware", tab: "Bloatware", help: "Apps preinstaladas de Microsoft Store que se pueden quitar, con recomendación para cada una." },
  // Programas
  { id: "software", label: "Actualizaciones", tab: "Actualizaciones", help: "Programas con versión nueva (winget) y Windows Update, en un solo sitio." },
  { id: "install", label: "Instalar programas", tab: "Instalar", help: "Instala en lote desde un catálogo verificado o buscando en winget. Guarda tus listas para reutilizarlas." },
  { id: "uninstall", label: "Desinstalar programas", tab: "Desinstalar", help: "Desinstala uno o varios, en silencio cuando se puede, y limpia los restos (carpetas, accesos, registro)." },
  {
    id: "recipes",
    label: "Preparar equipos",
    tab: "Preparar",
    help: "Plantillas: todo lo que hay que hacer en un equipo nuevo (quitar bloatware, instalar programas, aplicar ajustes, crear el usuario, unir al dominio…), de una vez y siempre igual. Perfiles de ajustes: grupos de ajustes que se aplican juntos y se pueden deshacer.",
  },
  // Red
  { id: "router", label: "Mi red", tab: "Mi red", help: "La red en la que está el equipo: router y su acceso, Wi-Fi y contraseña, y todos los dispositivos conectados." },
  { id: "network", label: "Velocidad y diagnóstico de red", tab: "Velocidad y diagnóstico", help: "Test de velocidad, estado de la conexión, reparar la red y controlar la Wi-Fi." },
  { id: "nettools", label: "Herramientas de red", tab: "Herramientas", help: "Ping, traceroute, puertos abiertos, DNS y archivo hosts." },
  // Oficina
  { id: "stations", label: "Puestos e inventario", tab: "Puestos", help: "Qué equipos de la oficina responden y cuáles necesitan atención (disco, reinicios, actualizaciones), y su inventario." },
  { id: "users", label: "Usuarios locales", tab: "Usuarios", help: "Crear usuarios, cambiar contraseñas, permisos de administrador y quitar cuentas." },
  { id: "accounts", label: "Cuentas", tab: "Cuentas", help: "Cuentas de Microsoft, profesionales (Entra ID), de Office y credenciales guardadas. Desconectarlas y pasar un equipo a cuenta local sin dar vueltas por Configuración." },
  { id: "domain", label: "Dominio", tab: "Dominio", help: "Unir el equipo al dominio, sacarlo, repararlo o cambiarle el nombre." },
  { id: "shares", label: "Carpetas compartidas", tab: "Compartidas", help: "Compartir carpetas en la red de la oficina y ver quién tiene acceso." },
  { id: "printers", label: "Impresoras", tab: "Impresoras", help: "Impresoras instaladas, cola de impresión, página de prueba y predeterminada." },
  { id: "remote", label: "Acceso remoto", tab: "Acceso remoto", help: "Agenda de conexiones, Escritorio remoto, AnyDesk, RustDesk y TeamViewer." },
  { id: "tools", label: "Herramientas de Windows", tab: "Herramientas", help: "Las herramientas de Windows de siempre, a un clic, y tus accesos directos propios." },
  // Soporte
  { id: "tickets", label: "Tickets", tab: "Tickets", help: "Tu sistema de tickets dentro de AdminOps, sin salir de la app." },
  {
    id: "agenda",
    label: "Agenda",
    tab: "Agenda",
    help: "Visitas de mantenimiento planificadas: qué toca hoy y los próximos días, a qué clientes ya les toca, y aviso de Windows antes de cada visita.",
  },
  {
    id: "mail",
    label: "Correo",
    tab: "Correo",
    help: "Tu Outlook (del trabajo o personal) dentro de AdminOps, sin configurarlo en cada equipo. La cuenta se guarda cifrada y, en sesión privada, no queda nada en el equipo al salir.",
  },
  { id: "clients", label: "Clientes", tab: "Clientes", help: "Fichas de clientes con sus equipos, visitas, garantías, mantenimientos y contactos." },
  { id: "contacts", label: "Contactos", tab: "Contactos", help: "A quién llamar y para qué: extensiones, correos, Teams. Viaja contigo en todos los equipos." },
  { id: "knowledge", label: "Conocimiento", tab: "Conocimiento", help: "Soluciones que funcionaron, plantillas de texto y notas de cada equipo y red." },
  { id: "report", label: "Informe", tab: "Informe", help: "Informe PDF profesional con la comparación antes/después; «Entrega rápida» resume lo hecho hoy." },
  { id: "shortcuts", label: "Atajos de teclado", tab: "Atajos", help: "Atajos de Windows y de programas habituales, con prueba en vivo." },
  // Datos
  { id: "vault", label: "Caja fuerte y carpetas cifradas", tab: "Caja fuerte", help: "Unidades cifradas con BitLocker y carpetas en .zip con contraseña." },
  { id: "wipe", label: "Borrado seguro", tab: "Borrado seguro", help: "Borrar archivos o el espacio libre para que no se puedan recuperar." },
  { id: "recover", label: "Recuperar archivos borrados", tab: "Recuperar archivos", help: "Recupera archivos borrados con Windows File Recovery." },
  { id: "migrate", label: "Copia de datos", tab: "Copia de datos", help: "Copia los datos de un usuario (escritorio, documentos, navegadores, Wi-Fi) y los restaura en otro equipo." },
  { id: "family", label: "Control parental", tab: "Control parental", help: "Filtro de webs para adultos, sitios bloqueados y horario de uso." },
  { id: "settings", label: "Ajustes", tab: "Ajustes", help: "Tus datos, apariencia, navegación, seguridad, informes y copias de seguridad." },
];

/**
 * Páginas que se unieron a otras: los enlaces antiguos (avisos, búsqueda, atajos)
 * llevan a la página nueva y a su pestaña.
 */
export const PAGE_ALIAS: Partial<Record<PageId, [PageId, string]>> = {
  repair: ["troubleshoot", "repairs"],
  winupdate: ["software", "windows"],
  devices: ["router", "devices"],
  inventory: ["stations", "inventory"],
  cleanup: ["tweaks", "cleanup"],
  performance: ["tweaks", "performance"],
  privacy: ["tweaks", "privacy"],
  services: ["tweaks", "services"],
  profiles: ["recipes", "profiles"],
};

/** Página y foco reales (resuelve las páginas unidas). */
export function resolvePage(page: PageId, focus: string | null = null): [PageId, string | null] {
  const alias = PAGE_ALIAS[page];
  return alias ? [alias[0], focus ?? alias[1]] : [page, focus];
}

/** ¿Se puede navegar a esta página (también las unidas a otra)? */
export const isPageId = (p: string | null | undefined): p is PageId => !!p && (NAV.some((n) => n.id === p) || p in PAGE_ALIAS);

export interface Area {
  id: string;
  label: string;
  icon: LucideIcon;
  /** Nombre del icono (para guardar la estructura personalizada). */
  iconName: string;
  pages: PageId[];
}

/** Iconos disponibles para las secciones (también para las que cree el usuario). */
export const AREA_ICONS: Record<string, LucideIcon> = {
  Gauge,
  Monitor,
  SlidersHorizontal,
  Package,
  Network,
  Headset,
  Lock,
  Wrench,
  Star,
  Folder,
  Briefcase,
  Shield,
  Home,
  Cpu,
  HardDrive,
  Wifi,
  Users,
  Terminal,
  Globe,
  Zap,
  Heart,
  Bookmark,
  Layers,
  Box,
  Database,
  Cloud,
  Printer,
  Bug,
  Rocket,
  Activity,
  Building2,
  Stethoscope,
};

/** Las áreas de la barra lateral; sus páginas son pestañas. */
const DEFAULT_AREAS: Omit<Area, "icon">[] = [
  // Los id se conservan: las navegaciones personalizadas siguen funcionando.
  { id: "panel", label: "Inicio", iconName: "Home", pages: ["dashboard", "troubleshoot", "session"] },
  { id: "equipo", label: "Equipo", iconName: "Monitor", pages: ["diagnostics", "hardware", "security", "processes", "space", "history"] },
  { id: "optimizar", label: "Mantener", iconName: "SlidersHorizontal", pages: ["tweaks", "startup", "bloatware"] },
  { id: "programas", label: "Programas", iconName: "Package", pages: ["software", "install", "uninstall", "recipes"] },
  { id: "red", label: "Red", iconName: "Network", pages: ["router", "network", "nettools"] },
  { id: "admin", label: "Oficina", iconName: "Building2", pages: ["stations", "users", "accounts", "domain", "shares", "printers", "remote", "tools"] },
  { id: "soporte", label: "Soporte", iconName: "Headset", pages: ["tickets", "mail", "agenda", "clients", "contacts", "knowledge", "report", "shortcuts"] },
  { id: "datos", label: "Datos", iconName: "Lock", pages: ["vault", "wipe", "recover", "migrate", "family"] },
];

const withIcon = (a: Omit<Area, "icon">): Area => ({ ...a, icon: AREA_ICONS[a.iconName] ?? Folder });

/** Estructura de fábrica. */
export const AREAS: Area[] = DEFAULT_AREAS.map(withIcon);

const VALID = new Set<string>(NAV.map((n) => n.id));

/** Estructura efectiva: la personalizada, con las páginas nuevas (de versiones futuras) en su sección de fábrica. */
export function effectiveAreas(layout: NavLayout | null): Area[] {
  if (!layout?.areas?.length) return AREAS;
  const seen = new Set<string>();
  const areas: Omit<Area, "icon">[] = layout.areas.map((a) => ({
    id: String(a.id),
    label: String(a.label || "Sección").slice(0, 30),
    iconName: AREA_ICONS[a.icon] ? a.icon : "Folder",
    pages: (a.pages ?? []).filter((p) => VALID.has(p) && p !== "settings" && !seen.has(p) && (seen.add(p), true)),
  }));
  for (const def of DEFAULT_AREAS) {
    const missing = def.pages.filter((p) => !seen.has(p));
    if (!missing.length) continue;
    const target = areas.find((a) => a.id === def.id);
    if (target) target.pages.push(...missing);
    else areas.push({ ...def, pages: missing });
    missing.forEach((p) => seen.add(p));
  }
  return areas.map(withIcon);
}

export const getAreas = () => effectiveAreas(getPrefs().layout);

export const areaOf = (page: PageId): Area | null => getAreas().find((a) => a.pages.includes(page)) ?? null;

/** Nombre de una página: el propio que le haya puesto el usuario o el de fábrica. */
export const pageLabel = (id: PageId): string => {
  const real = resolvePage(id)[0];
  return getPrefs().pageLabels[real]?.trim() || NAV.find((n) => n.id === real)?.label || real;
};

/** Páginas de la sección actual que se muestran (sin las ocultas, salvo la activa). */
export function visibleAreas(active: PageId): Area[] {
  const p = getPrefs();
  const hidden = new Set<string>(p.layout?.hidden ?? []);
  return effectiveAreas(p.layout)
    .map((a) => ({ ...a, pages: a.pages.filter((x) => !hidden.has(x) || x === active) }))
    .filter((a) => a.pages.length > 0);
}

const OPEN_KEY = "adminops.navOpen";

function readOpen(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(OPEN_KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
  } catch {
    return [];
  }
}

const WIDTH = { narrow: "w-52", normal: "w-60", wide: "w-72" } as const;
const ROW = { compact: "h-8", normal: "h-9", comfortable: "h-10" } as const;
const SUBROW = { compact: "h-7", normal: "h-8", comfortable: "h-9" } as const;

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
  onLock,
  update,
  recent = [],
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
  onLock?: () => void;
  update?: UpdateInfo | null;
  recent?: PageId[];
}) {
  const prefs = usePrefs();
  const sb = prefs.sidebar;
  const areas = visibleAreas(active);
  const current = areas.find((a) => a.pages.includes(active)) ?? null;
  // Áreas que el usuario dejó abiertas con la flecha (además de la actual).
  const [pinned, setPinned] = useState<string[]>(readOpen);
  const [closedCurrent, setClosedCurrent] = useState<string | null>(null);
  const isOpen = (a: Area) => {
    if (a.pages.length < 2) return false;
    if (sb.expand === "all") return true;
    const isCurrent = current?.id === a.id && closedCurrent !== a.id;
    return sb.expand === "current" ? isCurrent : pinned.includes(a.id) || isCurrent;
  };
  const toggle = (a: Area) => {
    const open = isOpen(a);
    if (sb.expand === "current") {
      setClosedCurrent(open ? a.id : null);
      if (!open) onArea(a);
      return;
    }
    let next = pinned.filter((id) => id !== a.id);
    if (!open) next = [...next, a.id];
    setPinned(next);
    setClosedCurrent(open && current?.id === a.id ? a.id : null);
    try {
      localStorage.setItem(OPEN_KEY, JSON.stringify(next));
    } catch {
      /* sin almacenamiento */
    }
  };
  const toggleFavorite = (p: PageId) => {
    const favs = sb.favorites.includes(p) ? sb.favorites.filter((x) => x !== p) : [...sb.favorites, p];
    setSidebar({ favorites: favs });
  };

  const border = sb.position === "right" ? "border-l" : "border-r";
  const mini = sb.mode === "mini";

  // ---------- Solo iconos ----------
  if (mini) {
    return (
      <aside className={`flex w-16 shrink-0 flex-col items-center border-line bg-panel ${border}`}>
        <button onClick={onAbout} className="pt-5 pb-4" title={`AdminOps ${appInfo?.version ?? ""}`}>
          <img src={logo} alt="" className="size-8" draggable={false} />
        </button>
        {sb.showSearch && (
          <button onClick={onSearch} className="mb-2 grid size-10 place-items-center rounded-lg text-mute hover:bg-panel-2 hover:text-ink" title="Buscar o ejecutar (Ctrl+K)">
            <Search size={18} strokeWidth={1.6} />
          </button>
        )}
        <nav aria-label="Navegación principal" className="flex flex-1 flex-col items-center gap-1 overflow-y-auto">
          {sb.favorites.length > 0 && (
            <button
              onClick={() => onSelect(sb.favorites[0])}
              className={`grid size-10 place-items-center rounded-lg ${sb.favorites.includes(active) ? "bg-panel-2 text-ink" : "text-mute hover:bg-panel-2 hover:text-ink"}`}
              title={`Favoritos: ${sb.favorites.map(pageLabel).join(", ")}`}
            >
              <Star size={18} strokeWidth={1.6} />
            </button>
          )}
          {areas.map((a) => {
            const Icon = a.icon;
            const on = current?.id === a.id;
            return (
              <button
                key={a.id}
                onClick={() => onArea(a)}
                aria-current={on ? "page" : undefined}
                title={a.label}
                className={`relative grid size-10 place-items-center rounded-lg transition-colors ${on ? "bg-panel-2 text-ink" : "text-mute hover:bg-panel-2 hover:text-ink"}`}
              >
                {on && <span className="absolute top-2 bottom-2 left-0 w-0.5 rounded-full bg-neon" />}
                <Icon size={19} strokeWidth={1.6} />
              </button>
            );
          })}
        </nav>
        {sessionActive && sb.showSession && (
          <button onClick={() => onSelect("session")} className="mb-2 grid size-10 place-items-center rounded-lg text-ok hover:bg-panel-2" title="Sesión de servicio en curso">
            <span className="size-2 rounded-full bg-ok" />
          </button>
        )}
        {update && (
          <button onClick={() => appcareApi.openRelease(update.url).catch(() => {})} className="mb-2 grid size-10 place-items-center rounded-lg text-neon hover:bg-panel-2" title={`Versión ${update.latest} disponible`}>
            <Rocket size={17} />
          </button>
        )}
        <div className="flex flex-col items-center gap-1 border-t border-line py-3">
          {onLock && (
            <button onClick={onLock} className="grid size-9 place-items-center rounded-md text-mute hover:bg-panel-2 hover:text-ink" title="Bloquear (Ctrl+L)">
              <LockIcon size={16} strokeWidth={1.6} />
            </button>
          )}
          <button
            onClick={() => onSelect("settings")}
            className={`grid size-9 place-items-center rounded-md hover:bg-panel-2 ${active === "settings" ? "text-ink" : "text-mute hover:text-ink"}`}
            title={`Ajustes (Ctrl+,) · ${isAdmin ? "Administrador" : "Usuario estándar"}`}
          >
            <SettingsIcon size={17} strokeWidth={1.6} />
          </button>
        </div>
      </aside>
    );
  }

  // ---------- Completa ----------
  const pageRow = (p: PageId, indent: boolean, key: string) => {
    const sel = p === active;
    const fav = sb.favorites.includes(p);
    return (
      <div key={key} className="group/page relative">
        <button
          onClick={() => onSelect(p)}
          aria-current={sel ? "page" : undefined}
          className={`relative flex w-full items-center rounded-md pr-7 text-left text-[13px] transition-colors ${SUBROW[sb.density]} ${indent ? (sb.showAreaIcons ? "pl-10" : "pl-5") : "pl-3"} ${
            sel ? "bg-panel-2 font-medium text-ink" : "text-dim hover:bg-panel-2/60 hover:text-ink"
          }`}
        >
          {sel && indent && <span className={`absolute top-2 bottom-2 w-0.5 rounded-full bg-neon ${sb.showAreaIcons ? "left-[21px]" : "left-2"}`} />}
          <span className="truncate">{pageLabel(p)}</span>
        </button>
        <button
          onClick={() => toggleFavorite(p)}
          className={`absolute top-1/2 right-1.5 -translate-y-1/2 rounded p-0.5 ${fav ? "text-neon" : "text-mute opacity-0 group-hover/page:opacity-100 hover:text-ink"}`}
          title={fav ? "Quitar de favoritos" : "Añadir a favoritos"}
          aria-label={fav ? "Quitar de favoritos" : "Añadir a favoritos"}
        >
          <Star size={12} fill={fav ? "currentColor" : "none"} />
        </button>
      </div>
    );
  };

  const smallHeader = (text: string) => <div className="px-3 pt-2 pb-1 text-[11px] font-medium text-mute">{text}</div>;
  const recentPages = sb.recents > 0 ? recent.filter((p) => p !== "settings").slice(0, sb.recents) : [];

  return (
    <aside className={`flex ${WIDTH[sb.width]} shrink-0 flex-col border-line bg-panel ${border}`}>
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

      {sb.showSearch && (
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
      )}

      <nav aria-label="Navegación principal" className="flex flex-1 flex-col gap-0.5 overflow-y-auto px-2.5">
        {sb.favorites.length > 0 && (
          <div className="mb-2">
            {smallHeader("Favoritos")}
            {sb.favorites.filter((p) => NAV.some((n) => n.id === p)).map((p) => pageRow(p, false, `fav-${p}`))}
          </div>
        )}
        {recentPages.length > 0 && (
          <div className="mb-2">
            {smallHeader("Recientes")}
            {recentPages.map((p) => pageRow(p, false, `rec-${p}`))}
          </div>
        )}
        {(sb.favorites.length > 0 || recentPages.length > 0) && smallHeader("Secciones")}
        {areas.map((a) => {
          const on = current?.id === a.id;
          const open = isOpen(a);
          const single = a.pages.length === 1;
          const Icon = a.icon;
          return (
            <div key={a.id}>
              <div className={`group flex ${ROW[sb.density]} items-center rounded-lg transition-colors ${on && (single || !open) ? "bg-panel-2" : "hover:bg-panel-2/60"}`}>
                <button
                  onClick={() => {
                    setClosedCurrent(null);
                    onArea(a);
                  }}
                  aria-current={on && single ? "page" : undefined}
                  className={`flex h-full min-w-0 flex-1 items-center gap-3 pl-3 text-left text-sm ${on ? "font-medium text-ink" : "text-dim group-hover:text-ink"}`}
                >
                  {sb.showAreaIcons && <Icon size={18} strokeWidth={1.6} className={on ? "text-ink" : "text-mute"} />}
                  <span className="flex-1 truncate">{a.label}</span>
                </button>
                {!single && sb.expand !== "all" && (
                  <button
                    onClick={() => toggle(a)}
                    aria-expanded={open}
                    aria-label={open ? `Plegar ${a.label}` : `Desplegar ${a.label}`}
                    className="grid h-full w-8 shrink-0 place-items-center rounded-r-lg text-mute hover:text-ink"
                  >
                    <ChevronRight size={14} className={`transition-transform ${open ? "rotate-90" : ""}`} />
                  </button>
                )}
              </div>
              {open && <div className="mt-0.5 mb-1 flex flex-col gap-px">{a.pages.map((p) => pageRow(p, true, p))}</div>}
            </div>
          );
        })}
      </nav>

      {update && (
        <button
          onClick={() => appcareApi.openRelease(update.url).catch(() => {})}
          className="mx-3.5 mb-2.5 flex flex-col gap-0.5 rounded-lg border border-neon/40 bg-neon/5 px-3 py-2.5 text-left transition-colors hover:border-neon"
          title={update.notes || "Ver la versión nueva en GitHub"}
        >
          <span className="text-xs text-neon">Versión {update.latest} disponible</span>
          <span className="text-[13px] text-ink">Ver novedades y descargar</span>
        </button>
      )}

      {sessionActive && sb.showSession && (
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
        {sb.showFooter ? (
          <>
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
          </>
        ) : (
          <span className="flex-1" />
        )}
        {onLock && (
          <button onClick={onLock} className="grid size-8 place-items-center rounded-md text-mute transition-colors hover:bg-panel-2 hover:text-ink" title="Bloquear AdminOps (Ctrl+L)" aria-label="Bloquear">
            <LockIcon size={16} strokeWidth={1.6} />
          </button>
        )}
        <button
          onClick={() => onSelect("settings")}
          className={`grid size-8 place-items-center rounded-md transition-colors hover:bg-panel-2 ${active === "settings" ? "text-ink" : "text-mute hover:text-ink"}`}
          title="Ajustes (Ctrl+,)"
          aria-label="Ajustes"
        >
          <SettingsIcon size={17} strokeWidth={1.6} />
        </button>
      </div>
      {sb.showFooter && (
        <button onClick={onAbout} className="pb-3 text-center text-[11px] text-mute transition-colors hover:text-dim">
          por David Bonilla
        </button>
      )}
    </aside>
  );
}
