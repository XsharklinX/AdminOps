// Páginas que agrupan varias vistas relacionadas en pestañas, para que cada
// cosa esté en un solo sitio (Actualizaciones, Mi red, Puestos e inventario,
// Ajustes de Windows).
import { BookCopy, ClipboardList, Cog, EyeOff, Layers, Gauge, Globe, MonitorCheck, Package, Radar, Router as RouterIcon, Search, ShieldCheck, Sparkles, X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { PageTabs, TabPanels, type PageTab } from "../components/PageTabs";
import { Devices } from "./Devices";
import { Inventory } from "./Inventory";
import { Profiles } from "./Profiles";
import { Recipes } from "./Recipes";
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

type UpdatesTab = "programs" | "windows";
const UPDATES: PageTab<UpdatesTab>[] = [
  { id: "programs", label: "Programas", icon: <Package size={14} /> },
  { id: "windows", label: "Windows Update", icon: <ShieldCheck size={14} /> },
];

/** Programas y Windows Update, juntos. */
export function Updates({ isAdmin, focus }: { isAdmin: boolean; focus?: string | null }) {
  return <Tabbed tabs={UPDATES} focus={focus} render={(t) => (t === "programs" ? <Software isAdmin={isAdmin} /> : <WindowsUpdate isAdmin={isAdmin} />)} />;
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

type TweakTab = "cleanup" | "performance" | "privacy" | "services";
const TWEAK_TABS: PageTab<TweakTab>[] = [
  { id: "cleanup", label: "Limpieza", icon: <Sparkles size={14} /> },
  { id: "performance", label: "Rendimiento", icon: <Gauge size={14} /> },
  { id: "privacy", label: "Privacidad", icon: <EyeOff size={14} /> },
  { id: "services", label: "Servicios", icon: <Cog size={14} /> },
];
const TWEAK_CATS = TWEAK_TABS.map((t) => t.id);

/**
 * Limpieza, rendimiento, privacidad y servicios en una sola página. El buscador
 * mira en las cuatro a la vez, así no hace falta saber en cuál está un ajuste.
 */
export function WindowsTweaks({ isAdmin, focus }: { isAdmin: boolean; focus?: string | null }) {
  const [query, setQuery] = useState("");
  // El foco puede ser una pestaña («privacy») o un ajuste concreto («privacy.telemetry»).
  const tabFocus = focus ? (focus.split(".")[0] as TweakTab) : null;
  const tab = tabFocus && TWEAK_CATS.includes(tabFocus) ? tabFocus : null;
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
          render={(t) => <TweaksPage category={t} isAdmin={isAdmin} focus={t === tab ? tweakFocus : null} />}
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
