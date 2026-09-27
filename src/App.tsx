import { lazy, Suspense, useEffect, useState } from "react";
import { AdminBanner } from "./components/AdminBanner";
import { About } from "./components/About";
import { ToastProvider } from "./components/feedback";
import { NAV, Sidebar, type PageId } from "./components/Sidebar";
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
const SettingsPage = lazyPage("SettingsPage", () => import("./pages/SettingsPage"));

/** Páginas que son una lista de ajustes del catálogo, por categoría. */
const TWEAK_PAGES: Partial<Record<PageId, string>> = {
  privacy: "privacy",
  performance: "performance",
  repair: "repair",
  services: "services",
  cleanup: "cleanup",
};

export default function App() {
  const [page, setPage] = useState<PageId>("dashboard");
  // Sección o ajuste a resaltar al llegar desde un hallazgo del diagnóstico.
  const [focus, setFocus] = useState<string | null>(null);
  const navigate = (p: PageId, f: string | null = null) => {
    setPage(p);
    setFocus(f);
  };
  const [isAdmin, setIsAdmin] = useState<boolean | null>(null);
  const [targetUser, setTargetUser] = useState<TargetUser | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [sessionActive, setSessionActive] = useState(false);

  useEffect(() => {
    api.isAdmin().then(setIsAdmin).catch(() => setIsAdmin(false));
    systemApi.targetUser().then(setTargetUser).catch(() => {});
    appApi.info().then(setAppInfo).catch(() => {});
    workApi.session().then((s) => setSessionActive(!!s)).catch(() => {});
  }, []);

  const nav = NAV.find((n) => n.id === page)!;
  const category = TWEAK_PAGES[page];

  let content;
  if (page === "dashboard") content = <Dashboard />;
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
  else if (page === "settings") content = <SettingsPage appInfo={appInfo} />;
  else if (category) content = <TweaksPage key={category} category={category} isAdmin={!!isAdmin} focus={focus} />;
  else content = null;

  return (
    <ToastProvider>
      <div className="flex h-full">
        <Sidebar active={page} onSelect={(p) => navigate(p)} isAdmin={isAdmin} targetUser={targetUser}
          appInfo={appInfo}
          sessionActive={sessionActive}
          onAbout={() => setAboutOpen(true)}
        />
        <main className="flex min-w-0 flex-1 flex-col">
          {isAdmin === false && <AdminBanner />}
          <header className="flex items-center justify-between border-b border-line px-6 py-4">
            <h1 className="text-xl font-semibold tracking-tight">{nav.label}</h1>
            {page === "dashboard" && <span className="font-mono text-[11px] text-mute">en vivo · 2 s</span>}
          </header>
          <div className="flex-1 overflow-y-auto">
            <Suspense fallback={<p className="p-8 font-mono text-sm text-mute">Cargando…</p>}>{content}</Suspense>
          </div>
        </main>
      </div>
      <About open={aboutOpen} onClose={() => setAboutOpen(false)} appInfo={appInfo} />
    </ToastProvider>
  );
}
