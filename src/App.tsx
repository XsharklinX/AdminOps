import { RotateCw } from "lucide-react";
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AdminBanner } from "./components/AdminBanner";
import type { PaletteAction } from "./components/CommandPalette";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { ToastProvider } from "./components/feedback";
import { AlertCenter } from "./components/AlertCenter";
import { LockScreen } from "./components/LockScreen";
import { NAV, Sidebar, areaOf, pageLabel, visibleAreas, type Area, type PageId } from "./components/Sidebar";
import { api, appApi, appcareApi, lockApi, systemApi, troubleshootApi, workApi, type AppInfo, type LockStatus, type TargetUser, type UpdateInfo } from "./lib/api";
import { PageActiveContext } from "./lib/pageActive";
import { comboOf, getPrefs, usePrefs } from "./lib/prefs";
import { Dashboard } from "./pages/Dashboard";
import { SYMPTOMS, Troubleshoot } from "./pages/Troubleshoot";
import { Contacts } from "./pages/Contacts";

// Solo el Panel se carga al abrir la app; el resto de páginas, al visitarlas.
const lazyPage = <T extends string>(name: T, load: () => Promise<Record<T, React.ComponentType<any>>>) =>
  lazy(() => load().then((m) => ({ default: m[name] })));
// Ventanas que se abren poco: se cargan al usarlas.
const About = lazyPage("About", () => import("./components/About"));
const CommandPalette = lazyPage("CommandPalette", () => import("./components/CommandPalette"));
const Onboarding = lazyPage("Onboarding", () => import("./components/Onboarding"));
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
const Router = lazyPage("Router", () => import("./pages/Router"));
const Devices = lazyPage("Devices", () => import("./pages/Devices"));
const Vault = lazyPage("Vault", () => import("./pages/Vault"));
const Wipe = lazyPage("Wipe", () => import("./pages/Wipe"));
const Recover = lazyPage("Recover", () => import("./pages/Recover"));
const Family = lazyPage("Family", () => import("./pages/Family"));
const Inventory = lazyPage("Inventory", () => import("./pages/Inventory"));
const Shares = lazyPage("Shares", () => import("./pages/Shares"));
const Remote = lazyPage("Remote", () => import("./pages/Remote"));
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
/** Páginas que se mantienen vivas a la vez; la menos usada se descarta al pasar de aquí. */
const MAX_ALIVE = 12;

/** Página de inicio elegida en Ajustes, o la última visitada (Diagnóstico no: se ejecuta solo al abrirlo). */
function initialPage(): PageId {
  const start = getPrefs().startPage;
  if (start !== "last" && NAV.some((n) => n.id === start)) return start;
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
  const openArea = useCallback(
    (a: Area) => navigate(getPrefs().sidebar.areaClick === "first" ? a.pages[0] : (lastByArea.current[a.id] ?? a.pages[0])),
    [navigate],
  );
  // Páginas recientes (para la sección «Recientes» de la barra lateral).
  const [recent, setRecent] = useState<PageId[]>([]);
  useEffect(() => {
    setRecent((r) => [page, ...r.filter((x) => x !== page)].slice(0, 10));
  }, [page]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [onboarding, setOnboarding] = useState(false);
  const [isAdmin, setIsAdmin] = useState<boolean | null>(null);
  const [targetUser, setTargetUser] = useState<TargetUser | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [alertsOpen, setAlertsOpen] = useState(false);
  const [sessionActive, setSessionActive] = useState(false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  // Aviso de versión nueva (Ajustes → General), unos segundos después de abrir.
  useEffect(() => {
    const t = window.setTimeout(() => {
      workApi
        .settings()
        .then((s) => (s.checkUpdates ? appcareApi.checkUpdate() : null))
        .then((u) => u?.newer && setUpdate(u))
        .catch(() => {});
    }, 8000);
    return () => window.clearTimeout(t);
  }, []);
  const prefs = usePrefs();
  // Páginas vivas (las más recientes primero) y cuántas veces se ha recargado cada una.
  const [alive, setAlive] = useState<PageId[]>([page]);
  const [reloads, setReloads] = useState<Partial<Record<PageId, number>>>({});
  useEffect(() => {
    setAlive((a) => (a[0] === page ? a : [page, ...a.filter((x) => x !== page)].slice(0, MAX_ALIVE)));
  }, [page]);
  // Bloqueo con PIN o contraseña: null mientras se consulta (no se enseña nada hasta saberlo).
  const [lock, setLock] = useState<LockStatus | null>(null);
  const [locked, setLocked] = useState<boolean | null>(null);
  const lastActivity = useRef(Date.now());
  const loadLock = useCallback(() => {
    lockApi
      .status()
      .then((s) => {
        setLock(s);
        setLocked((l) => (l === null ? s.enabled : l && s.enabled));
      })
      .catch(() => setLocked(false));
  }, []);
  useEffect(() => {
    loadLock();
    window.addEventListener("adminops-lock", loadLock);
    return () => window.removeEventListener("adminops-lock", loadLock);
  }, [loadLock]);
  // Bloqueo por inactividad.
  useEffect(() => {
    if (!lock?.enabled || !lock.idleMinutes) return;
    const touch = () => (lastActivity.current = Date.now());
    const events = ["pointerdown", "pointermove", "keydown", "wheel"] as const;
    events.forEach((ev) => window.addEventListener(ev, touch, { passive: true }));
    const t = window.setInterval(() => {
      if (Date.now() - lastActivity.current > lock.idleMinutes * 60_000) setLocked(true);
    }, 15_000);
    return () => {
      events.forEach((ev) => window.removeEventListener(ev, touch));
      window.clearInterval(t);
    };
  }, [lock]);

  useEffect(() => {
    try {
      localStorage.setItem(LAST_PAGE, page);
    } catch {
      /* sin almacenamiento */
    }
  }, [page]);

  const lockRef = useRef(false);
  lockRef.current = !!lock?.enabled;
  const lockedRef = useRef(false);
  lockedRef.current = locked !== false;

  // Atajos: Ctrl+K buscar, Ctrl+, ajustes, Ctrl+L bloquear, Alt+←/→ historial.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Bloqueada: ningún atajo actúa por debajo de la pantalla de bloqueo.
      if (lockedRef.current) return;
      // Atajos propios (Ajustes → Navegación). Mientras se asigna uno, no actúan.
      const combo = comboOf(e);
      const target = combo && !document.body.dataset.recordingShortcut ? getPrefs().shortcuts[combo] : undefined;
      if (target && NAV.some((n) => n.id === target)) {
        e.preventDefault();
        navigate(target);
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((o) => !o);
      } else if (e.ctrlKey && e.key.toLowerCase() === "l" && lockRef.current) {
        e.preventDefault();
        setLocked(true);
      } else if (e.key === "F5" && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        const p = pageRef.current;
        if (p !== "dashboard" && p !== "settings") setReloads((r) => ({ ...r, [p]: (r[p] ?? 0) + 1 }));
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
      // Síntomas: abren «Solucionar problemas» y lo comprueban.
      ...SYMPTOMS.map((s) => ({ id: `trouble:${s.id}`, title: `Solucionar: ${s.title}`, subtitle: s.hint, keywords: `${s.keywords} problema arreglar no funciona`, run: () => navigate("troubleshoot", s.id) })),
      { id: "netrepair", title: "Reparar la red", subtitle: "DNS, IP y adaptadores, con antes y después", keywords: "internet conexion winsock tcp ip renovar", run: () => navigate("network") },
      { id: "wifistate", title: "Estado de la Wi-Fi", subtitle: "Reiniciar la tarjeta, ahorro de energía, adaptadores fantasma", keywords: "wifi tarjeta adaptador driver codigo 10", run: () => navigate("network") },
      { id: "timeline", title: "Línea de tiempo del equipo", subtitle: "Qué cambió y qué pasó, por días", keywords: "historial eventos cambios arranques actualizaciones drivers", run: () => navigate("history") },
      { id: "wifion", title: "Encender la Wi-Fi", keywords: "wifi radio activar", run: () => troubleshootApi.fix("wifi.radio.on") },
      { id: "wifioff", title: "Apagar la Wi-Fi", keywords: "wifi radio desactivar", run: () => troubleshootApi.fix("wifi.radio.off") },
      { id: "bton", title: "Encender el Bluetooth", keywords: "bluetooth radio activar", run: () => troubleshootApi.fix("bt.radio.on") },
      { id: "btoff", title: "Apagar el Bluetooth", keywords: "bluetooth radio desactivar", run: () => troubleshootApi.fix("bt.radio.off") },
      { id: "gpureset", title: "Reiniciar el driver de la gráfica", subtitle: "Win+Ctrl+Shift+B: la pantalla parpadea un segundo", keywords: "pantalla negra monitor video grafica", run: () => troubleshootApi.fix("display.reset") },
      { id: "project", title: "Elegir cómo usar las pantallas", subtitle: "Duplicar, extender, solo una (Win+P)", keywords: "monitor proyector duplicar extender", run: () => troubleshootApi.fix("display.project") },
      ...[
        ["sound", "Configuración de sonido", "audio altavoces salida"],
        ["apps-volume", "Mezclador de volumen", "audio volumen aplicaciones"],
        ["display", "Configuración de pantalla", "monitor resolucion escala"],
        ["bluetooth", "Configuración de Bluetooth", "emparejar dispositivos"],
        ["network-wifi", "Redes Wi-Fi de Windows", "wifi conectar"],
        ["printers", "Impresoras y escáneres", "impresora anadir"],
      ].map(([uri, title, kw]) => ({ id: `open:${uri}`, title, subtitle: "Abre Configuración de Windows", keywords: kw, run: () => troubleshootApi.fix(`open:ms-settings:${uri}`) })),
    ],
    [isAdmin, navigate],
  );

  const nav = NAV.find((n) => n.id === page)!;
  const area = areaOf(page);
  // Pestañas de la sección actual: con la barra de solo iconos, o si se activan en Ajustes.
  const showTabs = prefs.sidebar.mode === "mini" || prefs.sidebar.headerTabs;
  const tabs = showTabs ? (visibleAreas(page).find((a) => a.pages.includes(page))?.pages ?? []) : [];

  // Cada página se construye una vez y se mantiene viva al cambiar de página (conserva sus datos,
  // su scroll y lo que esté haciendo). Se vuelve a cargar con «Recargar» o al cerrar la app.
  const renderPage = (p: PageId) => {
    const category = TWEAK_PAGES[p];
    if (p === "dashboard") return <Dashboard onNavigate={navigate} />;
    if (p === "history") return <History isAdmin={!!isAdmin} onNavigate={(x: PageId) => navigate(x)} />;
    if (p === "bloatware") return <Bloatware isAdmin={!!isAdmin} />;
    if (p === "startup") return <Startup isAdmin={!!isAdmin} />;
    if (p === "diagnostics") return <Diagnostics focus={p === page ? focus : null} onNavigate={navigate} />;
    if (p === "report") return <Report />;
    if (p === "profiles") return <Profiles isAdmin={!!isAdmin} />;
    if (p === "processes") return <Processes isAdmin={!!isAdmin} />;
    if (p === "hardware") return <Hardware isAdmin={!!isAdmin} focus={p === page ? focus : null} onNavigate={navigate} />;
    if (p === "network") return <Network isAdmin={!!isAdmin} />;
    if (p === "troubleshoot") return <Troubleshoot isAdmin={!!isAdmin} focus={p === page ? focus : null} onNavigate={(x: PageId) => navigate(x)} />;
    if (p === "software") return <Software isAdmin={!!isAdmin} />;
    if (p === "space") return <Space />;
    if (p === "session") return <Session onSessionChange={setSessionActive} />;
    if (p === "clients") return <Clients />;
    if (p === "contacts") return <Contacts focus={p === page ? focus : null} />;
    if (p === "tools") return <Tools isAdmin={!!isAdmin} />;
    if (p === "users") return <Users isAdmin={!!isAdmin} />;
    if (p === "install") return <Install isAdmin={!!isAdmin} />;
    if (p === "nettools") return <NetTools isAdmin={!!isAdmin} />;
    if (p === "printers") return <Printers isAdmin={!!isAdmin} />;
    if (p === "migrate") return <Migrate />;
    if (p === "tickets") return <Tickets covered={aboutOpen || paletteOpen || onboarding || alertsOpen || locked !== false || p !== page} />;
    if (p === "router") return <Router covered={aboutOpen || paletteOpen || onboarding || alertsOpen || locked !== false || p !== page} />;
    if (p === "devices") return <Devices />;
    if (p === "vault") return <Vault />;
    if (p === "wipe") return <Wipe onNavigate={(p: PageId) => navigate(p)} />;
    if (p === "recover") return <Recover />;
    if (p === "family") return <Family />;
    if (p === "inventory") return <Inventory />;
    if (p === "shares") return <Shares isAdmin={!!isAdmin} />;
    if (p === "remote") return <Remote isAdmin={!!isAdmin} />;
    if (p === "domain") return <Domain isAdmin={!!isAdmin} />;
    if (p === "uninstall") return <Uninstall isAdmin={!!isAdmin} />;
    if (p === "shortcuts") return <Shortcuts />;
    if (p === "security") return <Security isAdmin={!!isAdmin} focus={p === page ? focus : null} onNavigate={navigate} />;
    if (p === "winupdate") return <WindowsUpdate isAdmin={!!isAdmin} />;
    if (p === "settings") return <SettingsPage appInfo={appInfo} />;
    if (category) return <TweaksPage category={category} isAdmin={!!isAdmin} focus={p === page ? focus : null} />;
    return null;
  };

  return (
    <ToastProvider>
      {locked !== false && (locked === null ? <div className="fixed inset-0 z-[100] bg-void" /> : <LockScreen kind={lock?.kind ?? "password"} onUnlock={() => {
        lastActivity.current = Date.now();
        setLocked(false);
      }} />)}
      <div className={`flex h-full ${prefs.sidebar.position === "right" ? "flex-row-reverse" : ""}`}>
        <Sidebar active={page} onArea={openArea} onSelect={(p) => navigate(p)} isAdmin={isAdmin} targetUser={targetUser}
          appInfo={appInfo}
          sessionActive={sessionActive}
          onAbout={() => setAboutOpen(true)}
          onSearch={() => setPaletteOpen(true)}
          onLock={lock?.enabled ? () => setLocked(true) : undefined}
          update={update}
          recent={recent}
        />
        <main className="flex min-w-0 flex-1 flex-col">
          {isAdmin === false && <AdminBanner />}
          <header className="flex items-end justify-between border-b border-line px-8 pt-5 pb-4">
            <div className="min-w-0">
              {area && area.pages.length > 1 && !showTabs && <div className="mb-0.5 text-xs text-mute">{area.label}</div>}
              <h1 className="truncate text-[22px] font-semibold tracking-tight">{pageLabel(nav.id)}</h1>
              {showTabs && tabs.length > 1 && (
                <div className="mt-3 -mb-4 flex gap-1 overflow-x-auto">
                  {tabs.map((t) => (
                    <button
                      key={t}
                      onClick={() => navigate(t)}
                      className={`-mb-px shrink-0 border-b-2 px-3 py-1.5 text-[13px] transition-colors ${t === page ? "border-neon font-medium text-ink" : "border-transparent text-dim hover:text-ink"}`}
                    >
                      {pageLabel(t)}
                    </button>
                  ))}
                </div>
              )}
            </div>
            <div className="flex items-center gap-2">
            {page === "dashboard" ? (
              <span className="text-xs text-mute">En vivo · se actualiza cada {prefs.refreshMs / 1000} s</span>
            ) : (
              page !== "settings" && (
                <button
                  onClick={() => setReloads((r) => ({ ...r, [page]: (r[page] ?? 0) + 1 }))}
                  className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-mute transition-colors hover:bg-panel-2 hover:text-ink"
                  title="Vuelve a cargar esta página desde cero (F5)"
                >
                  <RotateCw size={13} /> Recargar
                </button>
              )
            )}
              <AlertCenter onNavigate={(p) => navigate(p)} onOpenChange={setAlertsOpen} />
            </div>
          </header>
          <div className="relative min-h-0 flex-1">
            {alive.map((p) => (
              <div key={`${p}-${reloads[p] ?? 0}`} hidden={p !== page} className="absolute inset-0 overflow-y-auto">
                <PageActiveContext.Provider value={p === page}>
                  <ErrorBoundary onHome={() => navigate("dashboard")}>
                    <Suspense fallback={<p className="p-8 font-mono text-sm text-mute">Cargando…</p>}>{renderPage(p)}</Suspense>
                  </ErrorBoundary>
                </PageActiveContext.Provider>
              </div>
            ))}
          </div>
        </main>
      </div>
      <Suspense fallback={null}>
        {aboutOpen && <About open onClose={() => setAboutOpen(false)} appInfo={appInfo} />}
        {paletteOpen && <CommandPalette open onClose={() => setPaletteOpen(false)} onNavigate={navigate} actions={actions} />}
      </Suspense>
      {onboarding && (
        <Suspense fallback={null}>
          <Onboarding
            onDone={(goTo: "diagnostics" | "dashboard") => {
              setOnboarding(false);
              navigate(goTo);
            }}
          />
        </Suspense>
      )}
    </ToastProvider>
  );
}
