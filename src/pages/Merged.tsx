// Páginas que agrupan varias vistas relacionadas en pestañas, para que cada
// cosa esté en un solo sitio (Actualizaciones, Mi red, Puestos e inventario,
// Ajustes de Windows).
import {
  BookCopy,
  ClipboardList,
  Cog,
  Download,
  EyeOff,
  FileText,
  Gauge,
  Globe,
  Activity,
  Cpu,
  FolderKey,
  HardDrive,
  History as HistoryIcon,
  Keyboard,
  Layers,
  Network as NetworkIcon,
  Printer,
  UserRound,
  Lock,
  MonitorCheck,
  Package,
  PackageMinus,
  PlayCircle,
  Power,
  Radar,
  Router as RouterIcon,
  Search,
  ShieldCheck,
  Sparkles,
  Stethoscope,
  Store,
  Trash2,
  Undo2,
  Wrench,
  X,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { PageTabs, TabPanels, type PageTab } from "../components/PageTabs";
import type { PageId } from "../components/Sidebar";
import { Devices } from "./Devices";
import { Accounts } from "./Accounts";
import { Diagnostics } from "./Diagnostics";
import { Hardware } from "./Hardware";
import { History } from "./History";
import { Security } from "./Security";
import { Bloatware } from "./Bloatware";
import { Domain } from "./Domain";
import { Family } from "./Family";
import { Install } from "./Install";
import { Printers } from "./Printers";
import { Shares } from "./Shares";
import { Inventory } from "./Inventory";
import { Migrate } from "./Migrate";
import { Profiles } from "./Profiles";
import { Recipes } from "./Recipes";
import { Recover } from "./Recover";
import { Report } from "./Report";
import { Session } from "./Session";
import { Disks } from "./Disks";
import { Space } from "./Space";
import { Shortcuts } from "./Shortcuts";
import { Startup } from "./Startup";
import { Tools } from "./Tools";
import { Uninstall } from "./Uninstall";
import { Vault } from "./Vault";
import { Wipe } from "./Wipe";
import { Router } from "./Router";
import { Software } from "./Software";
import { Stations } from "./Stations";
import { Tickets } from "./Tickets";
import { TweaksPage } from "./TweaksPage";
import { WindowsUpdate } from "./WindowsUpdate";

/** Pestaña elegida (o la que pide un enlace) y las ya visitadas, que se mantienen montadas. */
function useTabs<T extends string>(
  tabs: PageTab<T>[],
  focus: string | null | undefined,
) {
  const first = tabs[0].id;
  const [tab, setTab] = useState<T>(() =>
    tabs.some((t) => t.id === focus) ? (focus as T) : first,
  );
  const [visited, setVisited] = useState<T[]>([tab]);
  const choose = (t: T) => {
    setTab(t);
    setVisited((v) => (v.includes(t) ? v : [...v, t]));
  };
  useEffect(() => {
    if (focus && tabs.some((t) => t.id === focus)) choose(focus as T);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- solo cuando llega un foco nuevo; `choose` y `tabs` cambian en cada render
  }, [focus]);
  return { tab, visited, choose };
}

function Tabbed<T extends string>({
  tabs,
  focus,
  render,
}: {
  tabs: PageTab<T>[];
  focus?: string | null;
  render: (t: T, visible: boolean) => ReactNode;
}) {
  const { tab, visited, choose } = useTabs(tabs, focus);
  return (
    <div className="flex h-full flex-col">
      <PageTabs tabs={tabs} value={tab} onChange={choose} />
      <TabPanels
        value={tab}
        visited={visited}
        render={(t) => render(t, t === tab)}
      />
    </div>
  );
}

type AppsTab = "update" | "winupdate" | "install" | "uninstall" | "bloatware";
const APPS: PageTab<AppsTab>[] = [
  {
    id: "update",
    label: "Actualizar",
    icon: <Package size={14} />,
    help: "Programas con versión nueva según winget. Puedes actualizarlos en lote o ignorar los que no quieras tocar.",
  },
  {
    id: "winupdate",
    label: "Windows Update",
    icon: <ShieldCheck size={14} />,
    help: "Actualizaciones de Windows: cuáles faltan, instalarlas, pausarlas y ocultar la que dé problemas.",
  },
  {
    id: "install",
    label: "Instalar",
    icon: <Download size={14} />,
    help: "Instala programas en lote desde un catálogo comprobado (113 programas, incluidas las utilidades de driver de cada marca) o buscando en winget. Tus listas se guardan para reutilizarlas.",
  },
  {
    id: "uninstall",
    label: "Desinstalar",
    icon: <PackageMinus size={14} />,
    help: "Desinstala uno o varios programas, en silencio cuando se puede, y limpia lo que dejan atrás: carpetas, accesos directos y entradas del registro.",
  },
  {
    id: "bloatware",
    label: "Bloatware",
    icon: <Store size={14} />,
    help: "Apps preinstaladas de Microsoft Store que se pueden quitar, con una recomendación para cada una.",
  },
];

/** Todo lo que se hace con los programas del equipo, en un solo sitio. */
export function Apps({
  isAdmin,
  focus,
}: {
  isAdmin: boolean;
  focus?: string | null;
}) {
  return (
    <Tabbed
      tabs={APPS}
      focus={focus}
      render={(t) =>
        t === "update" ? (
          <Software isAdmin={isAdmin} />
        ) : t === "winupdate" ? (
          <WindowsUpdate isAdmin={isAdmin} />
        ) : t === "install" ? (
          <Install isAdmin={isAdmin} />
        ) : t === "uninstall" ? (
          <Uninstall isAdmin={isAdmin} />
        ) : (
          <Bloatware isAdmin={isAdmin} />
        )
      }
    />
  );
}

type NetTab = "router" | "devices";
const NET: PageTab<NetTab>[] = [
  {
    id: "router",
    label: "Red y router",
    icon: <RouterIcon size={14} />,
    help: "El router de esta red: su panel, la Wi-Fi y su contraseña.",
  },
  {
    id: "devices",
    label: "Dispositivos",
    icon: <Radar size={14} />,
    help: "Todo lo que está conectado a la red, con su función y su responsable; puedes vigilar los que importan.",
  },
];

/** La red en la que está el equipo: router, Wi-Fi y lo que hay conectado. */
export function MyNetwork({
  covered,
  focus,
}: {
  covered: boolean;
  focus?: string | null;
}) {
  // El portal del router es una ventana nativa: se oculta si su pestaña no está a la vista.
  return (
    <Tabbed
      tabs={NET}
      focus={focus}
      render={(t, visible) =>
        t === "router" ? <Router covered={covered || !visible} /> : <Devices />
      }
    />
  );
}

type OfficeTab = "stations" | "inventory" | "webinventory";
const OFFICE: PageTab<OfficeTab>[] = [
  {
    id: "stations",
    label: "Comprobar puestos",
    icon: <MonitorCheck size={14} />,
    help: "Comprueba qué equipos responden y cuáles necesitan atención, y actúa sobre varios a la vez.",
  },
  {
    id: "inventory",
    label: "Inventario",
    icon: <ClipboardList size={14} />,
    help: "Tu inventario de equipos, con la ficha de cada uno.",
  },
  {
    id: "webinventory",
    label: "Inventario web",
    icon: <Globe size={14} />,
    help: "La web de inventario de tu empresa, dentro de AdminOps, con los datos de este equipo a mano para rellenar el formulario.",
  },
];

/** Los equipos de la oficina: cuáles responden, su inventario y la web de inventario de la empresa. */
export function Workstations({
  covered,
  focus,
}: {
  covered: boolean;
  focus?: string | null;
}) {
  return (
    <Tabbed
      tabs={OFFICE}
      focus={focus}
      render={(t, visible) =>
        t === "stations" ? (
          <Stations />
        ) : t === "inventory" ? (
          <Inventory />
        ) : (
          <Tickets kind="inventory" covered={covered || !visible} />
        )
      }
    />
  );
}

type TweakTab = "cleanup" | "performance" | "privacy" | "services" | "startup";
const TWEAK_TABS: PageTab<TweakTab>[] = [
  {
    id: "cleanup",
    label: "Limpieza",
    icon: <Sparkles size={14} />,
    help: "Borrar lo que sobra: temporales, cachés y restos de Windows Update. Se puede revisar antes y deshacer después.",
  },
  {
    id: "performance",
    label: "Rendimiento",
    icon: <Gauge size={14} />,
    help: "Ajustes que aligeran Windows: efectos, apps en segundo plano, inicio rápido y plan de energía.",
  },
  {
    id: "privacy",
    label: "Privacidad",
    icon: <EyeOff size={14} />,
    help: "Telemetría, publicidad, historial de actividad y las novedades que envían datos a Microsoft.",
  },
  {
    id: "services",
    label: "Servicios",
    icon: <Cog size={14} />,
    help: "Servicios de Windows que se pueden desactivar sin romper nada. AdminOps guarda cómo estaban.",
  },
  {
    id: "startup",
    label: "Inicio de Windows",
    icon: <Power size={14} />,
    help: "Programas que arrancan con Windows. Desactivar los que sobran es lo que más acelera el arranque.",
  },
];
/** Las del catálogo de ajustes (Inicio de Windows es una página aparte). */
const TWEAK_CATS: TweakTab[] = [
  "cleanup",
  "performance",
  "privacy",
  "services",
];

/**
 * Limpieza, rendimiento, privacidad y servicios en una sola página. El buscador
 * mira en las cuatro a la vez, así no hace falta saber en cuál está un ajuste.
 */
export function WindowsTweaks({
  isAdmin,
  focus,
}: {
  isAdmin: boolean;
  focus?: string | null;
}) {
  const [query, setQuery] = useState("");
  // El foco puede ser una pestaña («privacy») o un ajuste concreto («privacy.telemetry»).
  const tabFocus = focus ? (focus.split(".")[0] as TweakTab) : null;
  const tab =
    tabFocus && TWEAK_TABS.some((t) => t.id === tabFocus) ? tabFocus : null;
  const tweakFocus = focus?.includes(".") ? focus : null;
  // Si llega un enlace a un ajuste, se limpia la búsqueda para que se vea.
  useEffect(() => {
    if (focus) setQuery("");
  }, [focus]);

  return (
    <div className="flex h-full flex-col">
      <div className="mx-auto w-full max-w-4xl shrink-0 px-6 pt-4">
        <label className="flex items-center gap-2 rounded-lg border border-line bg-panel px-3 py-2 focus-within:border-neon/50">
          <Search size={14} className="text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar un ajuste: telemetría, temporales, Cortana, indexación…"
            className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-mute"
          />
          {query && (
            <button
              onClick={() => setQuery("")}
              title="Borrar búsqueda"
              className="text-mute hover:text-ink"
            >
              <X size={14} />
            </button>
          )}
        </label>
      </div>
      {query.trim() ? (
        <div className="min-h-0 flex-1 overflow-y-auto">
          <TweaksPage category={TWEAK_CATS} isAdmin={isAdmin} query={query} />
        </div>
      ) : (
        <Tabbed
          tabs={TWEAK_TABS}
          focus={tab}
          render={(t) =>
            t === "startup" ? (
              <Startup isAdmin={isAdmin} />
            ) : (
              <TweaksPage
                category={t}
                isAdmin={isAdmin}
                focus={t === tab ? tweakFocus : null}
              />
            )
          }
        />
      )}
    </div>
  );
}

type PrepTab = "recipes" | "profiles";
const PREP: PageTab<PrepTab>[] = [
  {
    id: "recipes",
    label: "Plantillas",
    icon: <BookCopy size={14} />,
    help: "Plantillas con todo lo que haces en un equipo nuevo, para dejarlo listo de una vez y siempre igual.",
  },
  {
    id: "profiles",
    label: "Perfiles de ajustes",
    icon: <Layers size={14} />,
    help: "Grupos de ajustes que se aplican juntos (oficina, equipo antiguo, privacidad) y se pueden deshacer.",
  },
];

/** Preparar equipos: las plantillas de preparación y los perfiles de ajustes que usan, juntos. */
export function RecipesAndProfiles({
  isAdmin,
  focus,
}: {
  isAdmin: boolean;
  focus?: string | null;
}) {
  return (
    <Tabbed
      tabs={PREP}
      focus={focus}
      render={(t) =>
        t === "recipes" ? (
          <Recipes isAdmin={isAdmin} />
        ) : (
          <Profiles isAdmin={isAdmin} />
        )
      }
    />
  );
}

type ToolboxTab = "tools" | "shortcuts";
const TOOLBOX: PageTab<ToolboxTab>[] = [
  {
    id: "tools",
    label: "Herramientas",
    icon: <Wrench size={14} />,
    help: "Las utilidades de Windows de siempre a un clic, y tus accesos directos propios.",
  },
  {
    id: "shortcuts",
    label: "Atajos de teclado",
    icon: <Keyboard size={14} />,
    help: "Atajos de teclado de Windows y de los programas habituales, con prueba en vivo.",
  },
];

/** Las utilidades de Windows y los atajos de teclado, juntos. */
export function WindowsToolbox({
  isAdmin,
  focus,
}: {
  isAdmin: boolean;
  focus?: string | null;
}) {
  return (
    <Tabbed
      tabs={TOOLBOX}
      focus={focus}
      render={(t) =>
        t === "tools" ? <Tools isAdmin={isAdmin} /> : <Shortcuts />
      }
    />
  );
}

type SessionTab = "session" | "report";
const SESSION: PageTab<SessionTab>[] = [
  {
    id: "session",
    label: "Sesión",
    icon: <PlayCircle size={14} />,
    help: "La visita en curso: diagnóstico antes y después, checklist, presupuesto y firma.",
  },
  {
    id: "report",
    label: "Informe",
    icon: <FileText size={14} />,
    help: "El informe PDF con tu marca y la comparación antes/después, listo para entregar o enviar.",
  },
];

/** La visita de principio a fin y el informe que se entrega al cliente. */
export function ServiceSession({
  onSessionChange,
  focus,
}: {
  onSessionChange: (active: boolean) => void;
  focus?: string | null;
}) {
  // El foco puede ser una pestaña o el id de un cliente (viene de la Agenda: «Empezar»).
  const tab = SESSION.some((t) => t.id === focus) ? focus : null;
  return (
    <Tabbed
      tabs={SESSION}
      focus={tab}
      render={(t) =>
        t === "session" ? (
          <Session
            onSessionChange={onSessionChange}
            focus={tab ? null : focus}
          />
        ) : (
          <Report />
        )
      }
    />
  );
}

type DataTab = "migrate" | "vault" | "wipe" | "recover" | "family";
const DATA: PageTab<DataTab>[] = [
  {
    id: "migrate",
    label: "Copia de datos",
    icon: <Undo2 size={14} />,
    help: "Copia los datos de un usuario (escritorio, documentos, navegadores, Wi-Fi) y los restaura en otro equipo.",
  },
  {
    id: "vault",
    label: "Caja fuerte",
    icon: <Lock size={14} />,
    help: "Unidades cifradas con BitLocker y carpetas en .zip con contraseña.",
  },
  {
    id: "wipe",
    label: "Borrado seguro",
    icon: <Trash2 size={14} />,
    help: "Borra archivos o el espacio libre para que no se puedan recuperar.",
  },
  {
    id: "recover",
    label: "Recuperar archivos",
    icon: <Undo2 size={14} />,
    help: "Recupera archivos borrados con Windows File Recovery.",
  },
  {
    id: "family",
    label: "Control parental",
    icon: <ShieldCheck size={14} />,
    help: "Filtro de webs para adultos, sitios bloqueados y horario de uso.",
  },
];

/** Los datos del equipo: llevarlos a otro, cifrarlos, borrarlos o recuperarlos. */
export function DataTools({
  focus,
  onNavigate,
}: {
  focus?: string | null;
  onNavigate: (p: PageId) => void;
}) {
  return (
    <Tabbed
      tabs={DATA}
      focus={focus}
      render={(t) =>
        t === "migrate" ? (
          <Migrate />
        ) : t === "vault" ? (
          <Vault />
        ) : t === "wipe" ? (
          <Wipe onNavigate={onNavigate} />
        ) : t === "recover" ? (
          <Recover />
        ) : (
          <Family />
        )
      }
    />
  );
}

type IdentityTab = "accounts" | "domain";
const IDENTITY: PageTab<IdentityTab>[] = [
  {
    id: "accounts",
    label: "Cuentas",
    icon: <UserRound size={14} />,
    help: "Con qué cuenta entra este equipo: Microsoft, profesional (Entra ID), Office y credenciales guardadas. Incluye pasar el equipo a una cuenta local.",
  },
  {
    id: "domain",
    label: "Dominio",
    icon: <NetworkIcon size={14} />,
    help: "Unir el equipo al dominio, sacarlo, reparar la relación de confianza o cambiarle el nombre.",
  },
];

/** Con qué cuenta entra este equipo: cuentas conectadas y el dominio de la empresa. */
export function AccountsAndDomain({
  isAdmin,
  focus,
}: {
  isAdmin: boolean;
  focus?: string | null;
}) {
  return (
    <Tabbed
      tabs={IDENTITY}
      focus={focus}
      render={(t) =>
        t === "accounts" ? (
          <Accounts isAdmin={isAdmin} />
        ) : (
          <Domain isAdmin={isAdmin} />
        )
      }
    />
  );
}

type DiskTab = "space" | "health";
const DISK_TABS: PageTab<DiskTab>[] = [
  {
    id: "space",
    label: "Espacio",
    icon: <HardDrive size={14} />,
    help: "Dónde se ha ido el espacio y qué se puede recuperar: temporales, papelera, Windows.old e hibernación, el disco carpeta por carpeta y los archivos grandes que nadie abre.",
  },
  {
    id: "health",
    label: "Salud y reparación",
    icon: <Stethoscope size={14} />,
    help: "Qué le pasa a cada disco (superficie, conexión o sistema de archivos), repararlo cuando se puede y rescatar los archivos de un disco que falla.",
  },
];

/** Discos: el espacio y su salud (con reparación y rescate de archivos). */
export function DiskTools({
  isAdmin,
  focus,
  onNavigate,
}: {
  isAdmin: boolean;
  focus?: string | null;
  onNavigate?: (p: PageId, focus?: string | null) => void;
}) {
  return <Tabbed tabs={DISK_TABS} focus={focus} render={(t) => (t === "space" ? <Space onNavigate={onNavigate} /> : <Disks isAdmin={isAdmin} />)} />;
}

type SharedTab = "printers" | "shares";
const SHARED: PageTab<SharedTab>[] = [
  {
    id: "printers",
    label: "Impresoras",
    icon: <Printer size={14} />,
    help: "Impresoras instaladas, cola de impresión, página de prueba y cuál es la predeterminada.",
  },
  {
    id: "shares",
    label: "Carpetas compartidas",
    icon: <FolderKey size={14} />,
    help: "Carpetas compartidas en la red de la oficina y quién tiene acceso a cada una.",
  },
];

/** Lo que la oficina comparte: impresoras y carpetas de red. */
export function PrintersAndShares({
  isAdmin,
  focus,
}: {
  isAdmin: boolean;
  focus?: string | null;
}) {
  return (
    <Tabbed
      tabs={SHARED}
      focus={focus}
      render={(t) =>
        t === "printers" ? (
          <Printers isAdmin={isAdmin} />
        ) : (
          <Shares isAdmin={isAdmin} />
        )
      }
    />
  );
}

type MachineTab = "diagnostics" | "hardware" | "security" | "history";
const MACHINE: PageTab<MachineTab>[] = [
  {
    id: "diagnostics",
    label: "Diagnóstico",
    icon: <Activity size={14} />,
    help: "Analiza discos, estabilidad, drivers, batería, seguridad y actualizaciones, y dice qué hacer con cada problema. Lo que está bien se pliega para no estorbar.",
  },
  {
    id: "hardware",
    label: "Hardware",
    icon: <Cpu size={14} />,
    help: "Qué piezas lleva el equipo (para el inventario), temperaturas en vivo, salud de los discos (SMART) y prueba de memoria.",
  },
  {
    id: "security",
    label: "Seguridad",
    icon: <ShieldCheck size={14} />,
    help: "Nota de seguridad, antivirus, BitLocker, cuentas, lo que arranca solo y las extensiones del navegador.",
  },
  {
    id: "history",
    label: "Historial",
    icon: <HistoryIcon size={14} />,
    help: "Todo lo que ha pasado en este equipo: cambios, avisos, actualizaciones, drivers y apagados, con «Deshacer» y los puntos de restauración.",
  },
];

/**
 * Qué es este equipo y cómo está: el análisis, sus piezas, su seguridad y lo
 * que ha pasado en él. Cada pestaña se pausa cuando no se ve (ver TabPanels).
 */
export function MachineState({
  isAdmin,
  focus,
  onNavigate,
}: {
  isAdmin: boolean;
  focus?: string | null;
  onNavigate: (p: PageId, f?: string | null) => void;
}) {
  // El foco puede ser una pestaña o una sección de dentro (p. ej. «disks»).
  const tab = MACHINE.some((t) => t.id === focus)
    ? (focus as MachineTab)
    : null;
  const inner = tab ? null : (focus ?? null);
  return (
    <Tabbed
      tabs={MACHINE}
      focus={tab ?? sectionTab(inner)}
      render={(t) => {
        const f = t === (tab ?? sectionTab(inner)) ? inner : null;
        if (t === "diagnostics")
          return <Diagnostics focus={f} onNavigate={onNavigate} />;
        if (t === "hardware")
          return (
            <Hardware isAdmin={isAdmin} focus={f} onNavigate={onNavigate} />
          );
        if (t === "security")
          return (
            <Security isAdmin={isAdmin} focus={f} onNavigate={onNavigate} />
          );
        return <History isAdmin={isAdmin} onNavigate={onNavigate} />;
      }}
    />
  );
}

/** Secciones que viven dentro de una pestaña concreta (enlaces de los hallazgos). */
const SECTION_TAB: Record<string, MachineTab> = {
  disks: "diagnostics",
  stability: "diagnostics",
  drivers: "diagnostics",
  battery: "diagnostics",
  sensors: "hardware",
  smart: "hardware",
  memory: "hardware",
  sheet: "hardware",
};

const sectionTab = (focus: string | null): MachineTab | null =>
  focus ? (SECTION_TAB[focus] ?? "security") : null;
