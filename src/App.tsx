import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AdminBanner } from "./components/AdminBanner";
import { About } from "./components/About";
import { CommandPalette, type PaletteAction } from "./components/CommandPalette";
import { Onboarding } from "./components/Onboarding";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { ToastProvider } from "./components/feedback";
import { NAV, Sidebar, areaOf, type Area, type PageId } from "./components/Sidebar";
import { api, appApi, systemApi, workApi, type AppInfo, type TargetUser } from "./lib/api";
import { Dashboard } from "./pages/Dashboard";

// Solo el Panel se carga al abrir la app; el resto de páginas, al visitarlas.
const lazyPage = <T extends string>(name: T, load: () => Promise<Record<T, React.ComponentType<any>>>) =>
  lazy(() => load().then((m) => ({ default: m[name] })));
const Bloatware = lazyPage("Bloatware", () => import("./pages/Bloatware"));
const Diagnostics = lazyPage("Diagnostics", () => import("./pages/Diagnostics"));
const Report = lazyPage("Report", () => import("./pages/Report"));
const History = lazyPage("History", () => import("./pages/History"));
const Profiles = lazyPage("Profiles", () => import("./pages/Profiles"));
const Startup = lazyPage("Startup", () => import("./pages/Startup"));
const TweaksPage = lazyPage("TweaksPage", () => import("./pages/TweaksPage"));
const Processes = lazyPage("Processes", () => import("./pages/Processes"));
const Hardware = lazyPage("Hardware", () => import("./pages/Hardware"));
const Network = lazyPage("Network", () => import("./pages/Network"));
const Software = lazyPage("Software", () => import("./pages/Software"));
const Space = lazyPage("Space", () => import("./pages/Space"));
const Session = lazyPage("Session", () => import("./pages/Session"));
const Clients = lazyPage("Clients", () => import("./pages/Clients"));
const Tools = lazyPage("Tools", () => import("./pages/Tools"));
const Users = lazyPage("Users", () => import("./pages/Users"));
const Install = lazyPage("Install", () => import("./pages/Install"));
const NetTools = lazyPage("NetTools", () => import("./pages/NetTools"));
const Printers = lazyPage("Printers", () => import("./pages/Printers"));
const Migrate = lazyPage("Migrate", () => import("./pages/Migrate"));
const Tickets = lazyPage("Tickets", () => import("./pages/Tickets"));
const Domain = lazyPage("Domain", () => import("./pages/Domain"));
const Uninstall = lazyPage("Uninstall", () => import("./pages/Uninstall"));
const Security = lazyPage("Security", () => import("./pages/Security"));
const Shortcuts = lazyPage("Shortcuts", () => import("./pages/Shortcuts"));
const WindowsUpdate = lazyPage("WindowsUpdate", () => import("./pages/WindowsUpdate"));
const SettingsPage = lazyPage("SettingsPage", () => import("./pages/SettingsPage"));

/** Páginas que son una lista de ajustes del catálogo, por categoría. */
const TWEAK_PAGES: Partial<Record<PageId, string>> = {
  privacy: "privacy",
  performance: "performance",
  repair: "repair",
  services: "services",
  cleanup: "cleanup",
};

const LAST_PAGE = "adminops.lastPage";

/** Última página visitada. Diagnóstico no: se ejecuta solo al abrirlo. */
function initialPage(): PageId {
  try {
    const p = localStorage.getItem(LAST_PAGE);
    if (p && p !== "diagnostics" && NAV.some((n) => n.id === p)) return p as PageId;
  } catch {
    /* sin almacenamiento */
  }
  return "dashboard";
}

export default function App() {
  const [page, setPage] = useState<PageId>(initialPage);
  // Sección o ajuste a resaltar al llegar desde un hallazgo del diagnóstico o la búsqueda.
  const [focus, setFocus] = useState<string | null>(null);
  // Historial de páginas para Alt+← / Alt+→.
  const back = useRef<PageId[]>([]);
  const forward = useRef<PageId[]>([]);
  const pageRef = useRef(page);
  pageRef.current = page;
  const navigate = useCallback((p: PageId, f: string | null = null) => {
    if (p !== pageRef.current) {
      back.current = [...back.current.slice(-49), pageRef.current];
      forward.current = [];
    }
    setPage(p);
    setFocus(f);
  }, []);
  const goHistory = useCallback((dir: "back" | "forward") => {
    const from = dir === "back" ? back : forward;
    const to = dir === "back" ? forward : back;
    const p = from.current.pop();
    if (!p) return;
    to.current.push(pageRef.current);
    setPage(p);
    setFocus(null);
  }, []);
  // Última pestaña visitada de cada área: la barra lateral vuelve a ella.
  const lastByArea = useRef<Record<string, PageId>>({});
  useEffect(() => {
    const a = areaOf(page);
    if (a) lastByArea.current[a.id] = page;
  }, [page]);
  const openArea = useCallback((a: Area) => navigate(lastByArea.current[a.id] ?? a.pages[0]), [navigate]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [onboarding, setOnboarding] = useState(false);
  const [isAdmin, setIsAdmin] = useState<boolean | null>(null);
  const [targetUser, setTargetUser] = useState<TargetUser | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [sessionActive, setSessionActive] = useState(false);

  useEffect(() => {
    try {
      localStorage.setItem(LAST_PAGE, page);
    } catch {
      /* sin almacenamiento */
    }
  }, [page]);

  // Atajos: Ctrl+K buscar, Ctrl+, ajustes, Alt+←/→ historial.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((o) => !o);
      } else if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        navigate("settings");
      } else if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        e.preventDefault();
        goHistory(e.key === "ArrowLeft" ? "back" : "forward");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate, goHistory]);

  useEffect(() => {
    api.isAdmin().then(setIsAdmin).catch(() => setIsAdmin(false));
    systemApi.targetUser().then(setTargetUser).catch(() => {});
    appApi
      .info()
      .then((info) => {
        setAppInfo(info);
        // Solo para capturas y mediciones (scripts/bench.ps1 -Page): "tickets,dashboard"
        // abre la primera página y pasa a la siguiente cada 5 s.
        const seq = (info.startPage ?? "").split(",").filter((p) => NAV.some((n) => n.id === p)) as PageId[];
        seq.forEach((p, i) => window.setTimeout(() => setPage(p), i * 5000));
        // Primer arranque: asistente (no en las capturas automáticas).
        if (!info.startPage) workApi.settings().then((s) => setOnboarding(!s.onboarded)).catch(() => {});
      })
      .catch(() => {});
    workApi.session().then((s) => setSessionActive(!!s)).catch(() => {});
  }, []);

  const actions = useMemo<PaletteAction[]>(
    () => [
      ...(isAdmin === false ? [{ id: "admin", title: "Reiniciar AdminOps como administrador", run: () => void api.relaunchAsAdmin() }] : []),
      { id: "support", title: "Crear paquete de soporte", subtitle: "Registro y último diagnóstico en un .zip", run: () => void appApi.supportPackage() },
      { id: "diag", title: "Ejecutar un diagnóstico", run: () => navigate("diagnostics") },
      { id: "report", title: "Generar informe PDF", run: () => navigate("report") },
      { id: "shortcut", title: "Añadir un acceso directo propio", run: () => navigate("tools") },
      { id: "join", title: "Unir el equipo a un dominio", run: () => navigate("domain") },
      { id: "newuser", title: "Crear un usuario local", run: () => navigate("users") },
      { id: "setup", title: "Volver a abrir el asistente de inicio", run: () => setOnboarding(true) },
    ],
    [isAdmin, navigate],
  );

  const nav = NAV.find((n) => n.id === page)!;
  const area = areaOf(page);
  const category = TWEAK_PAGES[page];

  let content;
  if (page === "dashboard") content = <Dashboard onNavigate={navigate} />;
  else if (page === "history") content = <History isAdmin={!!isAdmin} />;
  else if (page === "bloatware") content = <Bloatware isAdmin={!!isAdmin} />;
  else if (page === "startup") content = <Startup isAdmin={!!isAdmin} />;
  else if (page === "diagnostics") content = <Diagnostics focus={focus} onNavigate={navigate} />;
  else if (page === "report") content = <Report />;
  else if (page === "profiles") content = <Profiles isAdmin={!!isAdmin} />;
  else if (page === "processes") content = <Processes isAdmin={!!isAdmin} />;
  else if (page === "hardware") content = <Hardware isAdmin={!!isAdmin} focus={focus} onNavigate={navigate} />;
  else if (page === "network") content = <Network isAdmin={!!isAdmin} />;
  else if (page === "software") content = <Software isAdmin={!!isAdmin} />;
  else if (page === "space") content = <Space />;
  else if (page === "session") content = <Session onSessionChange={setSessionActive} />;
  else if (page === "clients") content = <Clients />;
  else if (page === "tools") content = <Tools isAdmin={!!isAdmin} />;
  else if (page === "users") content = <Users isAdmin={!!isAdmin} />;
  else if (page === "install") content = <Install isAdmin={!!isAdmin} />;
  else if (page === "nettools") content = <NetTools isAdmin={!!isAdmin} />;
  else if (page === "printers") content = <Printers isAdmin={!!isAdmin} />;
  else if (page === "migrate") content = <Migrate />;
  else if (page === "tickets") content = <Tickets covered={aboutOpen || paletteOpen || onboarding} />;
  else if (page === "domain") content = <Domain isAdmin={!!isAdmin} />;
  else if (page === "uninstall") content = <Uninstall isAdmin={!!isAdmin} />;
  else if (page === "shortcuts") content = <Shortcuts />;
  else if (page === "security") content = <Security isAdmin={!!isAdmin} focus={focus} onNavigate={navigate} />;
  else if (page === "winupdate") content = <WindowsUpdate isAdmin={!!isAdmin} />;
  else if (page === "settings") content = <SettingsPage appInfo={appInfo} />;
  else if (category) content = <TweaksPage key={category} category={category} isAdmin={!!isAdmin} focus={focus} />;
  else content = null;

  return (
    <ToastProvider>
      <div className="flex h-full">
        <Sidebar active={page} onArea={openArea} onSelect={(p) => navigate(p)} isAdmin={isAdmin} targetUser={targetUser}
          appInfo={appInfo}
          sessionActive={sessionActive}
          onAbout={() => setAboutOpen(true)}
          onSearch={() => setPaletteOpen(true)}
        />
        <main className="flex min-w-0 flex-1 flex-col">
          {isAdmin === false && <AdminBanner />}
          <header className="border-b border-line px-8 pt-5">
            <div className={`flex items-center justify-between ${area && area.pages.length > 1 ? "pb-3" : "pb-4"}`}>
              <h1 className="text-[22px] font-semibold tracking-tight">{area ? area.label : nav.label}</h1>
              {page === "dashboard" && <span className="text-xs text-mute">En vivo · se actualiza cada 2 s</span>}
            </div>
            {area && area.pages.length > 1 && (
              <div role="tablist" className="-mb-px flex gap-1 overflow-x-auto">
                {area.pages.map((p) => {
                  const on = p === page;
                  return (
                    <button
                      key={p}
                      role="tab"
                      aria-selected={on}
                      onClick={() => navigate(p)}
                      className={`h-9 shrink-0 border-b-2 px-3 text-sm transition-colors ${
                        on ? "border-neon font-medium text-ink" : "border-transparent text-dim hover:text-ink"
                      }`}
                    >
                      {NAV.find((n) => n.id === p)!.tab}
                    </button>
                  );
                })}
              </div>
            )}
          </header>
          <div className="flex-1 overflow-y-auto">
            <ErrorBoundary key={page} onHome={() => navigate("dashboard")}>
              <Suspense fallback={<p className="p-8 font-mono text-sm text-mute">Cargando…</p>}>{content}</Suspense>
            </ErrorBoundary>
          </div>
        </main>
      </div>
      <About open={aboutOpen} onClose={() => setAboutOpen(false)} appInfo={appInfo} />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} onNavigate={navigate} actions={actions} />
      {onboarding && (
        <Onboarding
          onDone={(goTo) => {
            setOnboarding(false);
            navigate(goTo);
          }}
        />
      )}
    </ToastProvider>
  );
}
