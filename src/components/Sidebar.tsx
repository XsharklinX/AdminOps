import { useEffect, useRef, useState, type ReactNode } from "react";
import { type UpdateInfo } from "../lib/api";
import { getPrefs, setSidebar, usePrefs, type NavLayout } from "../lib/prefs";
import { goToPage } from "../lib/navigate";
import type { Badge } from "../lib/machineState";
import { navKey, parseNavKey, sectionLabel, sectionsOf } from "../lib/sections";
import { markSeen, useNewMarks } from "../lib/whatsNew";
import { useCurrentSections } from "../lib/sectionState";
import {
  Activity,
  Bookmark,
  PanelLeftClose,
  PanelLeftOpen,
  Box,
  Briefcase,
  Bug,
  Building2,
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
  MessagesSquare,
  Monitor,
  Network,
  Package,
  Pin,
  Printer,
  Rocket,
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
  | "machine"
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
  | "apps"
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
  | "people"
  | "mail"
  | "teams"
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
  | "data"
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
  {
    id: "troubleshoot",
    label: "Solucionar problemas",
    tab: "Solucionar",
    help: "Eliges el síntoma (no hay Internet, no suena, va lento…) y AdminOps revisa las causas típicas y ofrece la reparación. Abajo, las reparaciones de Windows (SFC, DISM, red, Windows Update…). Si buscas los pasos explicados, están en Soluciones.",
  },
  { id: "session", label: "Sesión de servicio", tab: "Sesión", help: "Una visita de principio a fin: diagnóstico antes y después, checklist que se marca sola, presupuesto y firma. En la pestaña Informe generas el PDF que entregas al cliente." },
  // Equipo
  {
    id: "machine",
    label: "Estado del equipo",
    tab: "Estado",
    help: "Qué es este equipo y cómo está: el diagnóstico con lo que hay que arreglar, sus piezas y temperaturas, su seguridad, y el historial de todo lo que ha pasado en él.",
  },
  { id: "processes", label: "Procesos", tab: "Procesos", help: "Qué está usando procesador, memoria y disco ahora mismo, y finalizar lo que se cuelga." },
  {
    id: "space",
    label: "Discos",
    tab: "Discos",
    help: "El espacio (qué ocupa y qué se puede liberar) y la salud de cada disco: qué le pasa, repararlo cuando se puede («Reparar disco» de Windows, sectores dañados) y rescatar los archivos de un disco que falla.",
  },
  // Optimizar Windows
  {
    id: "tweaks",
    label: "Optimizar Windows",
    tab: "Optimizar",
    help: "Limpieza, rendimiento, privacidad y servicios en un solo sitio, con buscador. Cada ajuste explica qué hace y se puede deshacer.",
  },
  // Aplicaciones
  {
    id: "apps",
    label: "Aplicaciones",
    tab: "Aplicaciones",
    help: "Todo lo que se hace con los programas del equipo: actualizarlos (winget y Windows Update), instalar en lote, desinstalar limpiando los restos y quitar el bloatware preinstalado.",
  },
  {
    id: "recipes",
    label: "Preparar equipos",
    tab: "Preparar",
    help: "Plantillas: todo lo que hay que hacer en un equipo nuevo (quitar bloatware, instalar programas, aplicar ajustes, crear el usuario, unir al dominio…), de una vez y siempre igual. Perfiles de ajustes: grupos de ajustes que se aplican juntos y se pueden deshacer.",
  },
  // Red
  { id: "router", label: "Red", tab: "Red", help: "Todo lo de la red en un sitio: el router y la Wi-Fi, los dispositivos conectados, el test de velocidad con reparar la red, y las herramientas (ping, traceroute, puertos, DNS, hosts)." },
  // Administración
  {
    id: "stations",
    label: "Puestos",
    tab: "Puestos",
    help: "Qué equipos de la oficina responden y cuáles necesitan atención (disco, reinicios, actualizaciones), a partir de tus listas de equipos.",
  },
  {
    id: "inventory",
    label: "Inventario",
    tab: "Inventario",
    help: "Todo el inventario en un sitio: el tuyo, con la ficha y el veredicto de cada equipo, y la web de inventario de la empresa dentro de AdminOps, con los datos de este equipo a mano para rellenarla.",
  },
  {
    id: "users",
    label: "Usuarios y cuentas",
    tab: "Usuarios",
    help: "Las cuentas de este equipo en un solo sitio: los usuarios locales (crear, contraseñas, administrador), las cuentas de Microsoft y Office con las credenciales guardadas, y el dominio de la empresa.",
  },
  {
    id: "printers",
    label: "Impresoras y carpetas",
    tab: "Impresoras",
    help: "Lo que la oficina comparte: impresoras instaladas con su cola y página de prueba, y las carpetas compartidas en la red con quién tiene acceso.",
  },
  { id: "remote", label: "Acceso remoto", tab: "Acceso remoto", help: "Agenda de conexiones, Escritorio remoto, AnyDesk, RustDesk y TeamViewer." },
  {
    id: "tools",
    label: "Herramientas de Windows",
    tab: "Herramientas",
    help: "Las herramientas de Windows de siempre a un clic, tus accesos directos propios y los atajos de teclado de Windows y de los programas habituales.",
  },
  // Soporte
  { id: "tickets", label: "Tickets", tab: "Tickets", help: "Tu sistema de tickets dentro de AdminOps, sin salir de la app." },
  {
    id: "people",
    label: "Personas y clientes",
    tab: "Personas y clientes",
    help: "A quién atiendes. Personas: la ficha de alguien del dominio (cuenta bloqueada o caducada, desbloquear, contraseña temporal, su equipo y sus claves), con tus propios permisos. Clientes: sus fichas con equipos, visitas, garantías y mantenimientos.",
  },
  {
    id: "agenda",
    label: "Agenda",
    tab: "Agenda",
    help: "Tareas, llamadas, reuniones y visitas, con cliente o sin él: lo de hoy, lo de mañana y la semana, los seguimientos, a qué clientes les toca mantenimiento y aviso de Windows 30 minutos antes.",
  },
  {
    id: "mail",
    label: "Correo",
    tab: "Correo",
    help: "Tu Outlook (del trabajo o personal) dentro de AdminOps, sin configurarlo en cada equipo. La cuenta se guarda cifrada y, en sesión privada, no queda nada en el equipo al salir.",
  },
  {
    id: "teams",
    label: "Teams",
    tab: "Teams",
    help: "Tu Teams (del trabajo o personal) dentro de AdminOps: chats, equipos y reuniones sin instalarlo en el equipo del cliente. La cuenta se guarda cifrada y, en sesión privada, no queda nada en el equipo al salir.",
  },
  { id: "contacts", label: "Contactos", tab: "Contactos", help: "A quién llamar y para qué: extensiones, correos, Teams. Viaja contigo en todos los equipos." },
  {
    id: "knowledge",
    label: "Soluciones",
    tab: "Soluciones",
    help: "Qué hacer ante cada problema: soluciones probadas paso a paso (las que trae AdminOps y las tuyas), plantillas de texto para el cliente y notas de cada equipo y red.",
  },
  // Datos
  {
    id: "data",
    label: "Datos del equipo",
    tab: "Datos",
    help: "Los datos del usuario: llevarlos a otro equipo, guardarlos cifrados, borrarlos sin que se puedan recuperar, recuperar los borrados y el control parental.",
  },
  {
    id: "settings",
    label: "Ajustes",
    tab: "Ajustes",
    help: "Todo lo que se puede personalizar, con buscador: para quién es AdminOps (técnico o usuario), cómo se comporta al abrirse, apariencia, barra lateral y atajos, portales y correo, bloqueo con PIN, tu marca en los informes, y cuánto tarda la app en abrirse.",
  },
];

/**
 * Páginas que se unieron a otras: los enlaces antiguos (avisos, búsqueda, atajos)
 * llevan a la página nueva y a su pestaña.
 */
export const PAGE_ALIAS: Partial<Record<PageId, [PageId, string]>> = {
  repair: ["troubleshoot", "repairs"],
  devices: ["router", "devices"],
  network: ["router", "speed"],
  nettools: ["router", "tools"],
  // Optimizar Windows
  cleanup: ["tweaks", "cleanup"],
  performance: ["tweaks", "performance"],
  privacy: ["tweaks", "privacy"],
  services: ["tweaks", "services"],
  startup: ["tweaks", "startup"],
  profiles: ["recipes", "profiles"],
  // Aplicaciones
  software: ["apps", "update"],
  winupdate: ["apps", "winupdate"],
  install: ["apps", "install"],
  uninstall: ["apps", "uninstall"],
  bloatware: ["apps", "bloatware"],
  // Herramientas de Windows
  shortcuts: ["tools", "shortcuts"],
  // Estado del equipo
  diagnostics: ["machine", "diagnostics"],
  hardware: ["machine", "hardware"],
  security: ["machine", "security"],
  history: ["machine", "history"],
  // Personas y clientes
  clients: ["people", "clients"],
  // Administración
  accounts: ["users", "accounts"],
  domain: ["users", "domain"],
  shares: ["printers", "shares"],
  // Sesión de servicio
  report: ["session", "report"],
  // Datos del equipo
  migrate: ["data", "migrate"],
  vault: ["data", "vault"],
  wipe: ["data", "wipe"],
  recover: ["data", "recover"],
  family: ["data", "family"],
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
  MessagesSquare,
};

/**
 * Las áreas de la barra lateral (1.2). Cada una enseña sus pantallas y, debajo de
 * cada pantalla, sus secciones (lib/sections.ts): nada queda detrás de una pestaña.
 * Teams y Correo no están: se abren desde la barra de arriba. Herramientas de
 * Windows y Ajustes van abajo, en la columna de áreas.
 */
const DEFAULT_AREAS: Omit<Area, "icon">[] = [
  // Los id se conservan: las navegaciones personalizadas siguen funcionando.
  { id: "panel", label: "Inicio", iconName: "Home", pages: ["dashboard", "troubleshoot", "session"] },
  { id: "equipo", label: "Este equipo", iconName: "Monitor", pages: ["machine", "tweaks", "processes", "space", "data"] },
  { id: "red", label: "Red", iconName: "Wifi", pages: ["router"] },
  { id: "programas", label: "Programas", iconName: "Package", pages: ["apps"] },
  { id: "admin", label: "Administración", iconName: "Building2", pages: ["stations", "users", "printers", "remote", "recipes"] },
  { id: "soporte", label: "Soporte", iconName: "Headset", pages: ["agenda", "tickets", "inventory", "people", "contacts", "knowledge"] },
];

/** Pantallas que no van en ningún área: tienen su sitio fijo (barra de arriba o pie de la columna). */
export const OUTSIDE_AREAS: PageId[] = ["teams", "mail", "tools", "settings"];

const withIcon = (a: Omit<Area, "icon">): Area => ({ ...a, icon: AREA_ICONS[a.iconName] ?? Folder });

/**
 * Lo que se ve en modo usuario: cómo está el equipo, resolver lo típico y dejar
 * entrar al técnico. Fuera queda lo que puede romper algo y todo lo que son
 * datos del técnico (clientes, contactos, tickets, correo, agenda).
 */
export const USER_MODE_PAGES: PageId[] = ["dashboard", "machine", "space", "troubleshoot", "remote", "settings"];

export const allowedInMode = (page: PageId, mode: "admin" | "user") => mode === "admin" || USER_MODE_PAGES.includes(page);

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
    .map((a) => ({ ...a, pages: a.pages.filter((x) => allowedInMode(x, p.mode) && (!hidden.has(x) || x === active)) }))
    .filter((a) => a.pages.length > 0);
}

/** Nombre de una línea fijada: la pantalla, o «Sección» de una pantalla. */
export function navLabel(key: string): string {
  const { page, section } = parseNavKey(key);
  return sectionLabel(page, section) ?? pageLabel(page);
}

/** Ancho del árbol (la columna de áreas va aparte). */
const WIDTH_PX = { narrow: 216, normal: 248, wide: 296 } as const;
const RAIL_W = 84;
const ROW = { compact: "h-7", normal: "h-8", comfortable: "h-9" } as const;

/** Hasta dónde se puede estrechar o ensanchar el árbol arrastrando. */
const MIN_W = 180;
const MAX_W = 440;

/**
 * Ajustar el ancho arrastrando el borde. Mientras se arrastra el ancho vive en
 * memoria (mover el ratón no debe escribir en el almacenamiento); al soltar se
 * guarda. Doble clic en el borde vuelve al ancho de los Ajustes.
 */
function useSidebarResize(position: "left" | "right") {
  const [dragW, setDragW] = useState<number | null>(null);
  const latest = useRef<number | null>(null);
  // Solo importa si se está arrastrando, no el ancho: el efecto no debe volver a
  // montarse con cada píxel que se mueve el ratón.
  const dragging = dragW !== null;

  useEffect(() => {
    if (!dragging) return;
    const move = (e: PointerEvent) => {
      const raw = (position === "right" ? window.innerWidth - e.clientX : e.clientX) - RAIL_W;
      const w = Math.round(Math.min(MAX_W, Math.max(MIN_W, raw)));
      latest.current = w;
      setDragW(w);
    };
    const stop = () => {
      if (latest.current !== null) setSidebar({ widthPx: latest.current });
      setDragW(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
    // Durante el arrastre el cursor no cambia aunque salga de la barra.
    const prev = [document.body.style.cursor, document.body.style.userSelect];
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
      [document.body.style.cursor, document.body.style.userSelect] = prev;
    };
  }, [dragging, position]);

  return {
    dragging,
    width: dragW,
    start: (from: number) => {
      latest.current = from;
      setDragW(from);
    },
  };
}

const TONE = {
  ok: "bg-ok/12 text-ok",
  warn: "bg-warn/15 text-warn",
  bad: "bg-bad/15 text-bad",
  neutral: "bg-panel-2 text-dim",
} as const;

function BadgeTag({ b }: { b: Badge }) {
  return (
    <span title={b.title} className={`shrink-0 rounded px-1.5 font-mono text-[10.5px] leading-[17px] ${TONE[b.tone]}`}>
      {b.text}
    </span>
  );
}

/**
 * La barra lateral de 1.2: una columna con las áreas y, a su lado, el árbol del
 * área: cada pantalla y, debajo, sus secciones como líneas. Lo que antes estaba
 * en una pestaña (Dominio, Inicio de Windows, Carpetas compartidas…) se ve aquí.
 */
export function Sidebar({
  active,
  onArea,
  onNavigate,
  isAdmin,
  badges,
  sessionActive,
  onLock,
  update,
  recent = [],
}: {
  active: PageId;
  onArea: (area: Area) => void;
  onNavigate: (page: PageId, section?: string | null) => void;
  isAdmin: boolean | null;
  badges: Record<string, Badge>;
  sessionActive: boolean;
  /** Abre «Todo AdminOps» (el mapa y el buscador). */
  onLock?: () => void;
  update?: UpdateInfo | null;
  recent?: PageId[];
}) {
  const prefs = usePrefs();
  const sb = prefs.sidebar;
  const resize = useSidebarResize(sb.position);
  const sections = useCurrentSections();
  const areas = visibleAreas(active);
  const activeArea = areas.find((a) => a.pages.includes(active)) ?? null;
  // El área que enseña el árbol: la de la pantalla actual; en Ajustes, Teams o
  // Herramientas (que no son de ningún área) se queda la última.
  const activeAreaId = activeArea?.id ?? null;
  const [shownId, setShownId] = useState<string | null>(activeAreaId);
  useEffect(() => {
    if (activeAreaId) setShownId(activeAreaId);
  }, [activeAreaId]);
  const shown = areas.find((a) => a.id === shownId) ?? activeArea ?? areas[0] ?? null;
  const mini = sb.mode === "mini";
  const badgeOf = (k: string) => (sb.showBadges ? badges[k] : undefined);
  const areaAlert = (a: Area) =>
    a.pages.some((p) => [navKey(p), ...sectionsOf(p).map((s) => navKey(p, s.id))].some((k) => {
      const b = badgeOf(k);
      return b && (b.tone === "warn" || b.tone === "bad");
    }));
  const pinned = sb.favorites.filter((k) => isPageId(parseNavKey(k).page) && allowedInMode(parseNavKey(k).page, prefs.mode));
  const togglePin = (k: string) => setSidebar({ favorites: sb.favorites.includes(k) ? sb.favorites.filter((x) => x !== k) : [...sb.favorites, k] });

  // Marca «Nuevo» tras actualizar: se va al entrar en la pantalla o en la sección.
  const fresh = useNewMarks();
  const shownSection = sections[active] ?? sectionsOf(active)[0]?.id ?? null;
  useEffect(() => {
    markSeen(shownSection ? [navKey(active), navKey(active, shownSection)] : [navKey(active)]);
  }, [active, shownSection]);

  /** La línea está a la vista: la pantalla actual y, si tiene secciones, la sección actual. */
  const isCurrent = (page: PageId, section: string | null) => {
    if (page !== active) return false;
    const list = sectionsOf(page);
    if (!list.length) return section === null;
    return section === (sections[page] ?? list[0].id);
  };

  const line = (k: string, label: string, opts: { sub?: boolean; current: boolean; onClick: () => void; badge?: Badge; title?: string }) => {
    const on = sb.favorites.includes(k);
    return (
      <div key={k} className="group/line relative">
        <button
          onClick={opts.onClick}
          aria-current={opts.current ? "page" : undefined}
          title={opts.title}
          className={`relative flex w-full items-center gap-2 rounded-md pr-8 text-left transition-colors ${ROW[sb.density]} ${
            opts.sub ? "pl-7 text-[12.5px]" : "pl-2.5 text-[13px] font-medium"
          } ${opts.current ? "bg-neon/10 text-ink" : opts.sub ? "text-dim hover:bg-panel-2 hover:text-ink" : "text-ink hover:bg-panel-2"}`}
        >
          {opts.sub && <span className={`absolute top-0 bottom-0 left-3.5 ${opts.current ? "w-0.5 bg-neon" : "w-px bg-line-2"}`} />}
          <span className="min-w-0 flex-1 truncate">{label}</span>
          {opts.badge ? (
            <BadgeTag b={opts.badge} />
          ) : (
            fresh.includes(k) && <BadgeTag b={{ text: "Nuevo", tone: "ok", title: "Ha cambiado en esta versión. La marca se va al entrar." }} />
          )}
        </button>
        <button
          onClick={() => togglePin(k)}
          className={`absolute top-1/2 right-1 grid size-6 -translate-y-1/2 place-items-center rounded ${on ? "text-neon" : "text-mute opacity-0 group-hover/line:opacity-100 hover:bg-panel-2 hover:text-ink focus-visible:opacity-100"}`}
          title={on ? "Quitar de fijados" : "Fijar arriba"}
          aria-label={on ? `Quitar ${label} de fijados` : `Fijar ${label} arriba`}
        >
          <Pin size={12} fill={on ? "currentColor" : "none"} />
        </button>
      </div>
    );
  };

  /** Una pantalla y sus secciones. */
  const pageLines = (p: PageId) => {
    const list = sectionsOf(p);
    return (
      <div key={p} className="flex flex-col gap-px">
        {line(navKey(p), pageLabel(p), {
          current: isCurrent(p, null),
          onClick: () => onNavigate(p),
          badge: badgeOf(navKey(p)),
          title: NAV.find((n) => n.id === p)?.help,
        })}
        {list.map((s) => line(navKey(p, s.id), s.label, { sub: true, current: isCurrent(p, s.id), onClick: () => onNavigate(p, s.id), badge: badgeOf(navKey(p, s.id)) }))}
      </div>
    );
  };

  const fixedButton = (label: string, Icon: LucideIcon, on: boolean, onClick: () => void, title?: string) => (
    <button
      onClick={onClick}
      aria-current={on ? "page" : undefined}
      title={title ?? label}
      className={`relative flex w-[76px] flex-col items-center gap-1 rounded-lg px-1 pt-2 pb-1.5 text-[10.5px] leading-tight transition-colors ${
        on ? "bg-neon/10 text-ink" : "text-mute hover:bg-panel-2 hover:text-ink"
      }`}
    >
      {on && <span className="absolute top-2.5 bottom-2.5 -left-1 w-[3px] rounded-full bg-neon" />}
      <Icon size={19} strokeWidth={1.6} />
      <span className="max-w-full text-center break-words">{label}</span>
    </button>
  );

  const rail = (
    <nav aria-label="Áreas" className={`flex w-[84px] shrink-0 flex-col items-center gap-0.5 bg-panel py-2 ${mini ? "" : sb.position === "right" ? "border-l border-line" : "border-r border-line"}`}>
      {areas.map((a) => {
        const on = !mini ? shown?.id === a.id && (activeArea?.id === a.id || !activeArea) : activeArea?.id === a.id;
        return (
          <div key={a.id} className="relative">
            {fixedButton(
              a.label,
              a.icon,
              on,
              () => {
                setShownId(a.id);
                onArea(a);
              },
            )}
            {areaAlert(a) && <span className="pointer-events-none absolute top-2 right-5 size-2 rounded-full bg-warn" title="Hay algo que atender en esta área" />}
          </div>
        );
      })}
      <div className="flex-1" />
      {allowedInMode("tools", prefs.mode) && fixedButton("Herramientas", Wrench, active === "tools", () => onNavigate("tools"), "Herramientas de Windows y atajos de teclado")}
      {onLock && fixedButton("Bloquear", LockIcon, false, onLock, "Bloquear AdminOps (Ctrl+L)")}
      {fixedButton("Ajustes", SettingsIcon, active === "settings", () => onNavigate("settings"), `Ajustes (Ctrl+,) · ${isAdmin ? "Administrador" : "Usuario estándar"}`)}
      <button
        onClick={() => setSidebar({ mode: mini ? "full" : "mini" })}
        className="mt-1 grid size-8 place-items-center rounded-md text-mute hover:bg-panel-2 hover:text-ink"
        title={mini ? "Mostrar las pantallas de cada área" : "Ocultar las pantallas: solo las áreas"}
        aria-label={mini ? "Mostrar las pantallas de cada área" : "Ocultar las pantallas"}
      >
        {mini === (sb.position === "right") ? <PanelLeftOpen size={16} strokeWidth={1.6} /> : <PanelLeftClose size={16} strokeWidth={1.6} />}
      </button>
    </nav>
  );

  if (mini) return <aside className={`flex shrink-0 border-line ${sb.position === "right" ? "border-l" : "border-r"}`}>{rail}</aside>;

  const recentPages = sb.recents > 0 ? recent.filter((p) => !OUTSIDE_AREAS.includes(p)).slice(0, sb.recents) : [];
  const widthPx = resize.width ?? sb.widthPx ?? WIDTH_PX[sb.width];
  const header = (text: string, right?: ReactNode) => (
    <div className="flex items-baseline justify-between px-2.5 pt-1 pb-1.5 font-mono text-[10.5px] tracking-[0.08em] text-mute uppercase">
      <span>{text}</span>
      {right}
    </div>
  );

  return (
    <aside className={`relative flex shrink-0 border-line ${sb.position === "right" ? "flex-row-reverse border-l" : "border-r"}`}>
      {rail}
      <div style={{ width: `${widthPx}px` }} className="relative flex flex-col bg-panel">
        {/* Borde que se arrastra para cambiar el ancho; doble clic vuelve al de Ajustes. */}
        <div
          onPointerDown={(e) => {
            e.preventDefault();
            resize.start(widthPx);
          }}
          onDoubleClick={() => setSidebar({ widthPx: null })}
          role="separator"
          aria-orientation="vertical"
          aria-label="Ajustar el ancho de la barra lateral"
          title="Arrastra para ajustar el ancho · doble clic para el ancho de siempre"
          className={`absolute inset-y-0 z-20 w-1.5 cursor-col-resize transition-colors hover:bg-neon/40 ${resize.dragging ? "bg-neon/60" : ""} ${
            sb.position === "right" ? "left-0" : "right-0"
          }`}
        />
        <nav aria-label="Pantallas y secciones" className="flex flex-1 flex-col gap-3 overflow-y-auto px-2 pt-3 pb-3">
          {pinned.length > 0 && (
            <div className="flex flex-col gap-px">
              {header("Fijados")}
              {pinned.map((k) => {
                const { page, section } = parseNavKey(k);
                return line(k, navLabel(k), {
                  current: section ? isCurrent(page, section) : page === active && !sectionsOf(page).length,
                  onClick: () => onNavigate(page, section),
                  badge: badgeOf(k),
                  title: section ? `${pageLabel(page)} › ${navLabel(k)}` : undefined,
                });
              })}
            </div>
          )}
          {recentPages.length > 0 && (
            <div className="flex flex-col gap-px">
              {header("Recientes")}
              {recentPages.map((p) => line(navKey(p), pageLabel(p), { current: p === active, onClick: () => onNavigate(p) }))}
            </div>
          )}
          {shown && (
            <div className="flex flex-col gap-2">
              {header(shown.label, <span>{shown.pages.length}</span>)}
              {shown.pages.map(pageLines)}
            </div>
          )}
        </nav>

        {update && (
          <button
            onClick={() => goToPage("settings", "about")}
            className="mx-2.5 mb-2 flex flex-col gap-0.5 rounded-lg border border-neon/40 bg-neon/5 px-3 py-2.5 text-left transition-colors hover:border-neon"
            title={update.notes || "Ver la versión nueva en Ajustes → Acerca de"}
          >
            <span className="text-xs text-neon">Versión {update.latest} disponible</span>
            <span className="text-[13px] text-ink">Ver novedades y descargar</span>
          </button>
        )}
        {sessionActive && sb.showSession && (
          <button
            onClick={() => onNavigate("session")}
            className="mx-2.5 mb-2.5 flex flex-col gap-0.5 rounded-lg border border-line px-3 py-2.5 text-left transition-colors hover:border-line-2"
          >
            <span className="flex items-center gap-1.5 text-xs text-mute">
              <span className="size-1.5 rounded-full bg-ok" /> Sesión de servicio
            </span>
            <span className="text-[13px] text-ink">En curso · ver checklist</span>
          </button>
        )}
      </div>
    </aside>
  );
}
