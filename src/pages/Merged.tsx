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
  FolderKey,
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
function useTabs<T extends string>(tabs: PageTab<T>[], focus: string | null | undefined) {
  const first = tabs[0].id;
  const [tab, setTab] = useState<T>(() => (tabs.some((t) => t.id === focus) ? (focus as T) : first));
  const [visited, setVisited] = useState<T[]>([tab]);
  const choose = (t: T) => {
    setTab(t);
    setVisited((v) => (v.includes(t) ? v : [...v, t]));
  };
  useEffect(() => {
    if (focus && tabs.some((t) => t.id === focus)) choose(focus as T);
  }, [focus]);
  return { tab, visited, choose };
}

function Tabbed<T extends string>({ tabs, focus, render }: { tabs: PageTab<T>[]; focus?: string | null; render: (t: T, visible: boolean) => ReactNode }) {
  const { tab, visited, choose } = useTabs(tabs, focus);
  return (
    <div className="flex h-full flex-col">
      <PageTabs tabs={tabs} value={tab} onChange={choose} />
      <TabPanels value={tab} visited={visited} render={(t) => render(t, t === tab)} />
    </div>
  );
}

type AppsTab = "update" | "winupdate" | "install" | "uninstall" | "bloatware";
const APPS: PageTab<AppsTab>[] = [
  { id: "update", label: "Actualizar", icon: <Package size={14} /> },
  { id: "winupdate", label: "Windows Update", icon: <ShieldCheck size={14} /> },
  { id: "install", label: "Instalar", icon: <Download size={14} /> },
  { id: "uninstall", label: "Desinstalar", icon: <PackageMinus size={14} /> },
  { id: "bloatware", label: "Bloatware", icon: <Store size={14} /> },
];

/** Todo lo que se hace con los programas del equipo, en un solo sitio. */
export function Apps({ isAdmin, focus }: { isAdmin: boolean; focus?: string | null }) {
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
  { id: "router", label: "Red y router", icon: <RouterIcon size={14} /> },
  { id: "devices", label: "Dispositivos", icon: <Radar size={14} /> },
];

/** La red en la que está el equipo: router, Wi-Fi y lo que hay conectado. */
export function MyNetwork({ covered, focus }: { covered: boolean; focus?: string | null }) {
  // El portal del router es una ventana nativa: se oculta si su pestaña no está a la vista.
  return <Tabbed tabs={NET} focus={focus} render={(t, visible) => (t === "router" ? <Router covered={covered || !visible} /> : <Devices />)} />;
}

type OfficeTab = "stations" | "inventory" | "webinventory";
const OFFICE: PageTab<OfficeTab>[] = [
  { id: "stations", label: "Comprobar puestos", icon: <MonitorCheck size={14} /> },
  { id: "inventory", label: "Inventario", icon: <ClipboardList size={14} /> },
  { id: "webinventory", label: "Inventario web", icon: <Globe size={14} /> },
];

/** Los equipos de la oficina: cuáles responden, su inventario y la web de inventario de la empresa. */
export function Workstations({ covered, focus }: { covered: boolean; focus?: string | null }) {
  return (
    <Tabbed
      tabs={OFFICE}
      focus={focus}
      render={(t, visible) =>
        t === "stations" ? <Stations /> : t === "inventory" ? <Inventory /> : <Tickets kind="inventory" covered={covered || !visible} />
      }
    />
  );
}

type TweakTab = "cleanup" | "performance" | "privacy" | "services" | "startup";
const TWEAK_TABS: PageTab<TweakTab>[] = [
  { id: "cleanup", label: "Limpieza", icon: <Sparkles size={14} /> },
  { id: "performance", label: "Rendimiento", icon: <Gauge size={14} /> },
  { id: "privacy", label: "Privacidad", icon: <EyeOff size={14} /> },
  { id: "services", label: "Servicios", icon: <Cog size={14} /> },
  { id: "startup", label: "Inicio de Windows", icon: <Power size={14} /> },
];
/** Las del catálogo de ajustes (Inicio de Windows es una página aparte). */
const TWEAK_CATS: TweakTab[] = ["cleanup", "performance", "privacy", "services"];

/**
 * Limpieza, rendimiento, privacidad y servicios en una sola página. El buscador
 * mira en las cuatro a la vez, así no hace falta saber en cuál está un ajuste.
 */
export function WindowsTweaks({ isAdmin, focus }: { isAdmin: boolean; focus?: string | null }) {
  const [query, setQuery] = useState("");
  // El foco puede ser una pestaña («privacy») o un ajuste concreto («privacy.telemetry»).
  const tabFocus = focus ? (focus.split(".")[0] as TweakTab) : null;
  const tab = tabFocus && TWEAK_TABS.some((t) => t.id === tabFocus) ? tabFocus : null;
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
            <button onClick={() => setQuery("")} title="Borrar búsqueda" className="text-mute hover:text-ink">
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
          render={(t) => (t === "startup" ? <Startup isAdmin={isAdmin} /> : <TweaksPage category={t} isAdmin={isAdmin} focus={t === tab ? tweakFocus : null} />)}
        />
      )}
    </div>
  );
}

type PrepTab = "recipes" | "profiles";
const PREP: PageTab<PrepTab>[] = [
  { id: "recipes", label: "Plantillas", icon: <BookCopy size={14} /> },
  { id: "profiles", label: "Perfiles de ajustes", icon: <Layers size={14} /> },
];

/** Preparar equipos: las plantillas de preparación y los perfiles de ajustes que usan, juntos. */
export function RecipesAndProfiles({ isAdmin, focus }: { isAdmin: boolean; focus?: string | null }) {
  return <Tabbed tabs={PREP} focus={focus} render={(t) => (t === "recipes" ? <Recipes isAdmin={isAdmin} /> : <Profiles isAdmin={isAdmin} />)} />;
}

type ToolboxTab = "tools" | "shortcuts";
const TOOLBOX: PageTab<ToolboxTab>[] = [
  { id: "tools", label: "Herramientas", icon: <Wrench size={14} /> },
  { id: "shortcuts", label: "Atajos de teclado", icon: <Keyboard size={14} /> },
];

/** Las utilidades de Windows y los atajos de teclado, juntos. */
export function WindowsToolbox({ isAdmin, focus }: { isAdmin: boolean; focus?: string | null }) {
  return <Tabbed tabs={TOOLBOX} focus={focus} render={(t) => (t === "tools" ? <Tools isAdmin={isAdmin} /> : <Shortcuts />)} />;
}

type SessionTab = "session" | "report";
const SESSION: PageTab<SessionTab>[] = [
  { id: "session", label: "Sesión", icon: <PlayCircle size={14} /> },
  { id: "report", label: "Informe", icon: <FileText size={14} /> },
];

/** La visita de principio a fin y el informe que se entrega al cliente. */
export function ServiceSession({ onSessionChange, focus }: { onSessionChange: (active: boolean) => void; focus?: string | null }) {
  // El foco puede ser una pestaña o el id de un cliente (viene de la Agenda: «Empezar»).
  const tab = SESSION.some((t) => t.id === focus) ? focus : null;
  return (
    <Tabbed
      tabs={SESSION}
      focus={tab}
      render={(t) => (t === "session" ? <Session onSessionChange={onSessionChange} focus={tab ? null : focus} /> : <Report />)}
    />
  );
}

type DataTab = "migrate" | "vault" | "wipe" | "recover" | "family";
const DATA: PageTab<DataTab>[] = [
  { id: "migrate", label: "Copia de datos", icon: <Undo2 size={14} /> },
  { id: "vault", label: "Caja fuerte", icon: <Lock size={14} /> },
  { id: "wipe", label: "Borrado seguro", icon: <Trash2 size={14} /> },
  { id: "recover", label: "Recuperar archivos", icon: <Undo2 size={14} /> },
  { id: "family", label: "Control parental", icon: <ShieldCheck size={14} /> },
];

/** Los datos del equipo: llevarlos a otro, cifrarlos, borrarlos o recuperarlos. */
export function DataTools({ focus, onNavigate }: { focus?: string | null; onNavigate: (p: PageId) => void }) {
  return (
    <Tabbed
      tabs={DATA}
      focus={focus}
      render={(t) =>
        t === "migrate" ? <Migrate /> : t === "vault" ? <Vault /> : t === "wipe" ? <Wipe onNavigate={onNavigate} /> : t === "recover" ? <Recover /> : <Family />
      }
    />
  );
}

type IdentityTab = "accounts" | "domain";
const IDENTITY: PageTab<IdentityTab>[] = [
  { id: "accounts", label: "Cuentas", icon: <UserRound size={14} /> },
  { id: "domain", label: "Dominio", icon: <NetworkIcon size={14} /> },
];

/** Con qué cuenta entra este equipo: cuentas conectadas y el dominio de la empresa. */
export function AccountsAndDomain({ isAdmin, focus }: { isAdmin: boolean; focus?: string | null }) {
  return <Tabbed tabs={IDENTITY} focus={focus} render={(t) => (t === "accounts" ? <Accounts isAdmin={isAdmin} /> : <Domain isAdmin={isAdmin} />)} />;
}

type SharedTab = "printers" | "shares";
const SHARED: PageTab<SharedTab>[] = [
  { id: "printers", label: "Impresoras", icon: <Printer size={14} /> },
  { id: "shares", label: "Carpetas compartidas", icon: <FolderKey size={14} /> },
];

/** Lo que la oficina comparte: impresoras y carpetas de red. */
export function PrintersAndShares({ isAdmin, focus }: { isAdmin: boolean; focus?: string | null }) {
  return <Tabbed tabs={SHARED} focus={focus} render={(t) => (t === "printers" ? <Printers isAdmin={isAdmin} /> : <Shares isAdmin={isAdmin} />)} />;
}
