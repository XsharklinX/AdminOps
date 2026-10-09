import { RotateCw } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { PaletteAction } from "./components/CommandPalette";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { ToastProvider } from "./components/feedback";
import { AlertCenter } from "./components/AlertCenter";
import { QuitButton } from "./components/QuitButton";
import { OpenTabs } from "./components/OpenTabs";
import { PageFind } from "./components/PageFind";
import { LockScreen } from "./components/LockScreen";
import { NAV, Sidebar, allowedInMode, areaOf, isPageId, pageLabel, resolvePage, visibleAreas, type Area, type PageId } from "./components/Sidebar";
import { TopBar } from "./components/TopBar";
import { CommOpener } from "./components/CommOpener";
import { useMachineState } from "./lib/machineState";
import { sectionLabel, sectionsOf } from "./lib/sections";
import { PageIdContext, requestSection, useCurrentSections } from "./lib/sectionState";
import { logQuietly, api, appApi, appcareApi, lockApi, noteApi, portalsApi, systemApi, troubleshootApi, tweaksApi, workApi, type AppInfo, type LockStatus, type TargetUser, type UpdateInfo } from "./lib/api";
import { PageActiveContext } from "./lib/pageActive";
import { comboOf, getPrefs, usePrefs } from "./lib/prefs";
import { analyze, analyzeQuick } from "./lib/diagRun";
import { lastPortalKey, watchPortals } from "./lib/portalState";
import { isModestMachine, machineSummary } from "./lib/machine";
import { NAVIGATE_EVENT, ONBOARDING_EVENT } from "./lib/navigate";
import { appStarted, appStartMs, codeLoaded, onPerfChange, pageLoads, pageOpened, pagePainted } from "./lib/perf";
import { Dashboard } from "./pages/Dashboard";
import { AuditBanner, AuditToggle } from "./components/AuditMode";
import { PrivacyBanner, PrivacyToggle } from "./components/PrivacyMode";
import { PageHelp } from "./components/PageHelp";
import { TasksIndicator } from "./components/TasksIndicator";
import { CaseBar, NewCaseButton } from "./components/CaseBar";
import { openCase } from "./lib/currentCase";
import { SPLIT_LEFT, SPLIT_RIGHT } from "./lib/split";
import { SYMPTOMS } from "./lib/symptoms";
import { Loading } from "./components/ui";
import { openHelp, useHelp } from "./lib/help";

// Solo el Panel se carga al abrir la app; el resto de páginas, al visitarlas.
// eslint-disable-next-line @typescript-eslint/no-explicit-any -- cada página tiene sus propias props
const lazyPage = <T extends string>(name: T, load: () => Promise<Record<T, React.ComponentType<any>>>) =>
  lazy(() => {
    const t = performance.now();
    return load().then((m) => {
      codeLoaded(t);
      return { default: m[name] };
    });
  });

/** Se pinta junto a la página: cuando aparece, la página ya está en pantalla. */
function PaintMark({ page }: { page: string }) {
  useEffect(() => pagePainted(page), [page]);
  return null;
}
// Ventanas que se abren poco: se cargan al usarlas.
const About = lazyPage("About", () => import("./components/About"));
const HelpCenter = lazyPage("HelpCenter", () => import("./components/HelpCenter"));
const CommandPalette = lazyPage("CommandPalette", () => import("./components/CommandPalette"));
const Onboarding = lazyPage("Onboarding", () => import("./components/Onboarding"));
const TweaksPage = lazyPage("TweaksPage", () => import("./pages/TweaksPage"));
const Processes = lazyPage("Processes", () => import("./pages/Processes"));
const DiskTools = lazyPage("DiskTools", () => import("./pages/Merged"));
const Tickets = lazyPage("Tickets", () => import("./pages/Tickets"));
const Remote = lazyPage("Remote", () => import("./pages/Remote"));
const SettingsPage = lazyPage("SettingsPage", () => import("./pages/SettingsPage"));
const Troubleshoot = lazyPage("Troubleshoot", () => import("./pages/Troubleshoot"));
const Contacts = lazyPage("Contacts", () => import("./pages/Contacts"));
const Knowledge = lazyPage("Knowledge", () => import("./pages/Knowledge"));
const Apps = lazyPage("Apps", () => import("./pages/Merged"));
const WindowsToolbox = lazyPage("WindowsToolbox", () => import("./pages/Merged"));
const ServiceSession = lazyPage("ServiceSession", () => import("./pages/Merged"));
const DataTools = lazyPage("DataTools", () => import("./pages/Merged"));
const AccountsAndDomain = lazyPage("AccountsAndDomain", () => import("./pages/Merged"));
const MachineState = lazyPage("MachineState", () => import("./pages/Merged"));
const PrintersAndShares = lazyPage("PrintersAndShares", () => import("./pages/Merged"));
const MyNetwork = lazyPage("MyNetwork", () => import("./pages/Merged"));
const Workstations = lazyPage("Workstations", () => import("./pages/Merged"));
const InventoryPage = lazyPage("InventoryPage", () => import("./pages/Merged"));
const Agenda = lazyPage("Agenda", () => import("./pages/Agenda"));
const PeopleAndClients = lazyPage("PeopleAndClients", () => import("./pages/Merged"));
const WindowsTweaks = lazyPage("WindowsTweaks", () => import("./pages/Merged"));
const RecipesAndProfiles = lazyPage("RecipesAndProfiles", () => import("./pages/Merged"));

/** Páginas que son una lista de ajustes del catálogo, por categoría. */
const TWEAK_PAGES: Partial<Record<PageId, string>> = {
  repair: "repair",
};

const LAST_PAGE = "adminops.lastPage";
/** Página que va al lado del portal en la pantalla dividida (se recuerda). */
const SPLIT_KEY = "adminops.split";

/** Página donde vive el portal de cada tipo (para precargar solo el que toca). */
const PORTAL_PAGE = { inventory: "inventory", mail: "mail", teams: "teams" } as const;
/** Páginas que se mantienen vivas a la vez; la menos usada se descarta al pasar de aquí. */
/** «Análisis rápido» pedido desde el icono junto al reloj. */
const TRAY_QUICK_SCAN = "adminops-tray-quick-scan";

/** Acciones de «Todo AdminOps» que se ejecutan al momento: clave, ajuste que las hace, título y otras formas de decirlo. */
const DO_NOW: [string, string, string, string][] = [
  ["print-queue", "repair.print-queue", "Vaciar la cola de impresión", "impresora atascada no imprime cola spooler trabajos"],
  ["audio", "repair.audio", "Reiniciar el sonido", "audio no suena altavoz microfono servicio"],
  ["explorer", "repair.explorer", "Reiniciar el Explorador", "barra de tareas colgada escritorio explorer"],
  ["dns", "cleanup.dns-cache", "Vaciar la caché de DNS", "dns pagina no carga flushdns"],
  ["temp", "cleanup.user-temp", "Limpiar los temporales del usuario", "temp basura liberar espacio"],
  ["time", "repair.time-sync", "Sincronizar la hora", "reloj hora mal fecha"],
  ["icons", "repair.icon-cache", "Reconstruir la caché de iconos", "iconos en blanco mal"],
];

const MAX_ALIVE = 12;
/** Si la interfaz tardó más que esto en empezar, el arranque fue lento y no se precarga nada. */
const SLOW_START_MS = 5000;

/** La última versión con la que se abrió AdminOps (para enseñar sus novedades una vez). */
const SEEN_VERSION_KEY = "adminops-seen-version";
function seenVersion(): string | null {
  try {
    return localStorage.getItem(SEEN_VERSION_KEY);
  } catch {
    return null;
  }
}
function rememberVersion(v: string) {
  try {
    localStorage.setItem(SEEN_VERSION_KEY, v);
  } catch {
    /* sin almacenamiento: se volverán a enseñar, sin más */
  }
}

/** Página de inicio elegida en Ajustes, o la última visitada (Diagnóstico no: se ejecuta solo al abrirlo). */
function initialPage(): PageId {
  const prefs = getPrefs();
  if (prefs.mode === "user") return "dashboard";
  const start = prefs.startPage;
  if (start !== "last" && isPageId(start)) return resolvePage(start)[0];
  try {
    const p = localStorage.getItem(LAST_PAGE);
    if (p && p !== "diagnostics" && isPageId(p)) return resolvePage(p)[0];
  } catch {
    /* sin almacenamiento */
  }
  return "dashboard";
}

export default function App() {
  const [page, setPage] = useState<PageId>(() => {
    // La primera página se mide desde que se abrió la ventana (arranque completo).
    const p = initialPage();
    appStarted();
    pageOpened(p, 0);
    return p;
  });
  // Páginas ya montadas: abrir una nueva se mide (se marca antes de pintarla).
  const aliveRef = useRef<PageId[]>([]);
  // Sección o ajuste a resaltar al llegar desde un hallazgo del diagnóstico o la búsqueda.
  const [focus, setFocus] = useState<string | null>(null);
  // Historial de páginas para Alt+← / Alt+→.
  const back = useRef<PageId[]>([]);
  const forward = useRef<PageId[]>([]);
  const pageRef = useRef(page);
  pageRef.current = page;
  const navigate = useCallback((to: PageId, focusOn: string | null = null) => {
    // Las páginas que se unieron a otras llevan a la nueva, en su pestaña.
    const [p, f] = resolvePage(to, focusOn);
    // En modo usuario, lo que no está permitido no se abre por ningún camino
    // (Ctrl+K, un aviso, un enlace de una solución…).
    if (!allowedInMode(p, getPrefs().mode)) return;
    if (p !== pageRef.current && !aliveRef.current.includes(p)) pageOpened(p);
    if (p !== pageRef.current) {
      back.current = [...back.current.slice(-49), pageRef.current];
      forward.current = [];
    }
    setPage(p);
    setFocus(f);
  }, []);
  /** Ir a una pantalla y, si se dice, a una de sus secciones. */
  const goTo = useCallback(
    (to: PageId, section: string | null = null) => {
      navigate(to, section);
      if (section) requestSection(resolvePage(to)[0], section);
    },
    [navigate],
  );
  const goHistory = useCallback((dir: "back" | "forward") => {
    const from = dir === "back" ? back : forward;
    const to = dir === "back" ? forward : back;
    const p = from.current.pop();
    if (!p) return;
    to.current.push(pageRef.current);
    setPage(p);
    setFocus(null);
  }, []);
  // Cuando la primera página está lista, el resumen del arranque va al registro
  // técnico: así se comparan versiones con datos (Ajustes → Rendimiento lo detalla).
  useEffect(() => {
    let logged = false;
    const off = onPerfChange(() => {
      const first = pageLoads().find((l) => l.at === 0);
      if (logged || !first) return;
      logged = true;
      void import("./components/PerfPanel").then(({ fetchTiming, startupSummary }) =>
        fetchTiming()
          // Con el equipo delante: los mismos milisegundos no significan lo mismo
          // en un i7 con 32 GB que en un portátil de hace ocho años.
          .then((t) => appApi.logTiming(`${machineSummary()} · ${startupSummary(t, first)}`))
          .catch(logQuietly("App")),
      );
    });
    return off;
  }, []);
  // Diálogos compartidos que llevan a otra página (p. ej. enviar el informe con el Correo).
  useEffect(() => {
    const f = (e: Event) => {
      const { page: p, focus: f } = (e as CustomEvent<{ page: string; focus: string | null }>).detail;
      if (isPageId(p)) navigate(p, f);
    };
    window.addEventListener(NAVIGATE_EVENT, f);
    return () => window.removeEventListener(NAVIGATE_EVENT, f);
  }, [navigate]);
  // «Volver a ver la bienvenida» (Ajustes → General).
  useEffect(() => {
    const f = () => setOnboarding(true);
    window.addEventListener(ONBOARDING_EVENT, f);
    return () => window.removeEventListener(ONBOARDING_EVENT, f);
  }, []);
  // Al pulsar un aviso de Windows, Windows trae AdminOps al frente. Como el
  // aviso no dice cuál se pulsó, si acaba de salir uno se abre la campana con lo
  // pendiente, en vez de dejar al técnico buscándolo.
  const [alertSignal, setAlertSignal] = useState(0);
  useEffect(() => {
    let notifiedAt = 0;
    const un = listen("alert-notified", () => (notifiedAt = Date.now()));
    // Lo que se pide desde el icono junto al reloj y hace la interfaz.
    const tray = listen<string>("tray-action", ({ payload }) => {
      if (payload === "alerts") setAlertSignal((n) => n + 1);
      if (payload === "quick-scan") window.dispatchEvent(new Event(TRAY_QUICK_SCAN));
    });
    const onFocus = () => {
      if (notifiedAt && Date.now() - notifiedAt < 2 * 60_000) {
        notifiedAt = 0;
        setAlertSignal((n) => n + 1);
      }
    };
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("focus", onFocus);
      void un.then((f) => f());
      void tray.then((f) => f());
    };
  }, []);

  // Portales: el estado de cada vista web se escucha desde el principio y el
  // último portal usado de cada tipo se precarga, para que al entrar ya esté listo.
  useEffect(() => {
    watchPortals();
    // Cada vista precargada es un proceso de navegador entero, y el correo y
    // Teams son de las webs más pesadas que existen. Antes se precargaba el
    // último portal de CADA tipo: hasta cuatro navegadores abiertos por detrás
    // sin que el técnico hubiera pedido ninguno. Ahora solo el último usado, y
    // en un equipo justo de recursos ninguno.
    if (!getPrefs().preloadPortals || isModestMachine()) return;
    const t = setTimeout(() => {
      let id: string | null = null;
      try {
        id = localStorage.getItem(lastPortalKey(""));
        for (const k of ["inventory", "mail", "teams"] as const) {
          const v = localStorage.getItem(lastPortalKey(k));
          if (v && localStorage.getItem(LAST_PAGE) === PORTAL_PAGE[k]) id = v;
        }
      } catch {
        /* sin almacenamiento */
      }
      // Un arranque lento (pendrive, equipo ocupado) no se carga con otro navegador más.
      if (!id || (appStartMs() ?? 0) > SLOW_START_MS) return;
      const [w, h] = [Math.max(800, window.innerWidth - 260), Math.max(500, window.innerHeight - 140)];
      void portalsApi.preload(id, w, h).catch(logQuietly("App"));
      // Bien entrada la sesión: primero que termine de abrirse la aplicación y
      // de leer lo del Panel (a los 8 s coincidía con todo lo demás).
    }, 20_000);
    // Cada vista web es un proceso: las que llevan mucho sin verse se cierran
    // (al volver a entrar se abren de nuevo y la sesión de la web sigue).
    const idle = setInterval(() => void portalsApi.closeIdle().catch(logQuietly("App")), 10 * 60_000);
    return () => {
      clearTimeout(t);
      clearInterval(idle);
    };
  }, []);
  // «Diagnosticar al abrir»: se lanza cuando la ventana ya está pintada, sin
  // competir con la carga de la primera página.
  useEffect(() => {
    if (!getPrefs().diagnoseOnOpen) return;
    const t = setTimeout(() => void analyze().catch(logQuietly("App")), 2500);
    return () => clearTimeout(t);
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
  // La ayuda es una ventana encima de todo: los portales (vistas nativas) se ocultan mientras.
  const helpOpen = useHelp() !== null;
  const [alertsOpen, setAlertsOpen] = useState(false);
  // Un diálogo del caso está abierto: tapa los portales como cualquier otro diálogo.
  const [caseDialog, setCaseDialog] = useState(false);
  // Pantalla dividida: el portal a la izquierda y esta página a la derecha.
  const [split, setSplitState] = useState<PageId | null>(() => {
    try {
      const v = localStorage.getItem(SPLIT_KEY);
      return v && SPLIT_RIGHT.includes(v as PageId) ? (v as PageId) : null;
    } catch {
      return null;
    }
  });
  const setSplit = useCallback((p: PageId | null) => {
    setSplitState(p);
    try {
      if (p) localStorage.setItem(SPLIT_KEY, p);
      else localStorage.removeItem(SPLIT_KEY);
    } catch {
      /* sin almacenamiento */
    }
  }, []);
  const [sessionActive, setSessionActive] = useState(false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  // Aviso de versión nueva (Ajustes → General), unos segundos después de abrir.
  useEffect(() => {
    const t = window.setTimeout(() => {
      workApi
        .settings()
        .then((s) => (s.checkUpdates ? appcareApi.checkUpdate() : null))
        .then((u) => u?.newer && setUpdate(u))
        .catch(logQuietly("App"));
      // No corre prisa: que no compita con las primeras lecturas del Panel.
    }, 30_000);
    return () => window.clearTimeout(t);
  }, []);
  const prefs = usePrefs();
  const machine = useMachineState();
  const sections = useCurrentSections();
  // Páginas vivas (las más recientes primero) y cuántas veces se ha recargado cada una.
  const [alive, setAlive] = useState<PageId[]>([page]);
  const [reloads, setReloads] = useState<Partial<Record<PageId, number>>>({});
  aliveRef.current = alive;
  useEffect(() => {
    setAlive((a) => (a[0] === page ? a : [page, ...a.filter((x) => x !== page)].slice(0, MAX_ALIVE)));
    setOpened((t) => (t.includes(page) ? t : [...t, page]));
  }, [page]);
  // Pestañas: las pantallas vivas, en el orden en que se abrieron.
  const [opened, setOpened] = useState<PageId[]>([page]);
  const openTabs = opened.filter((p) => alive.includes(p));
  const tabsRef = useRef<PageId[]>([]);
  tabsRef.current = openTabs;
  const closeTab = useCallback(
    (p: PageId) => {
      const list = tabsRef.current;
      if (list.length < 2) return;
      // Si es la que se mira, se pasa a la de al lado (la de la derecha, o la de la izquierda si era la última).
      if (p === pageRef.current) {
        const i = list.indexOf(p);
        navigate(list[i + 1] ?? list[i - 1]);
      }
      setOpened((t) => t.filter((x) => x !== p));
      setAlive((a) => a.filter((x) => x !== p));
    },
    [navigate],
  );
  const [findOpen, setFindOpen] = useState(false);
  // La pantalla dividida está puesta si se está en un portal y hay página para el lado.
  const splitOn = split !== null && SPLIT_LEFT.includes(page) && allowedInMode(split, prefs.mode);
  // La página de la derecha tiene que estar montada, aunque no sea la actual.
  useEffect(() => {
    if (splitOn && split) setAlive((a) => (a.includes(split) ? a : [...a, split].slice(-MAX_ALIVE)));
  }, [splitOn, split]);
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
      if (((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") || (e.key === "F1" && !e.ctrlKey && !e.altKey && !e.shiftKey)) {
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
      } else if (e.ctrlKey && !e.altKey && e.key.toLowerCase() === "f") {
        // Buscar en la pantalla actual (Ctrl+K es para toda la app).
        e.preventDefault();
        setFindOpen(true);
      } else if (e.ctrlKey && e.key === "Tab" && getPrefs().pageTabs) {
        e.preventDefault();
        const list = tabsRef.current;
        const i = list.indexOf(pageRef.current);
        if (list.length > 1) navigate(list[(i + (e.shiftKey ? -1 : 1) + list.length) % list.length]);
      } else if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === "w" && getPrefs().pageTabs) {
        e.preventDefault();
        closeTab(pageRef.current);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate, goHistory, closeTab]);

  useEffect(() => {
    api.isAdmin().then(setIsAdmin).catch(() => setIsAdmin(false));
    systemApi.targetUser().then(setTargetUser).catch(logQuietly("App"));
    appApi
      .info()
      .then((info) => {
        setAppInfo(info);
        // Solo para capturas y mediciones (scripts/bench.ps1 -Page): "tickets,dashboard"
        // abre la primera página y pasa a la siguiente cada 5 s.
        const seq = (info.startPage ?? "").split(",").filter((p) => NAV.some((n) => n.id === p)) as PageId[];
        seq.forEach((p, i) => window.setTimeout(() => setPage(resolvePage(p)[0]), i * 5000));
        // Primer arranque: asistente (no en las capturas automáticas).
        if (!info.startPage)
          workApi
            .settings()
            .then((s) => {
              setOnboarding(!s.onboarded);
              // La primera vez que se abre una versión nueva, sus novedades (una sola vez).
              if (s.onboarded && seenVersion() !== info.version) openHelp("news");
              rememberVersion(info.version);
            })
            .catch(logQuietly("App"));
      })
      .catch(logQuietly("App"));
    workApi.session().then((s) => setSessionActive(!!s)).catch(logQuietly("App"));
  }, []);

  const actions = useMemo<PaletteAction[]>(
    () => [
      ...(isAdmin === false ? [{ id: "admin", title: "Reiniciar AdminOps como administrador", run: () => void api.relaunchAsAdmin() }] : []),
      { id: "support", title: "Crear paquete de soporte", subtitle: "Registro y último diagnóstico en un .zip", run: () => void appApi.supportPackage() },
      { id: "help-guide", title: "Guía de AdminOps", subtitle: "Qué hace cada pantalla y cómo se usa", keywords: "ayuda manual glosario como usar documentacion", run: () => openHelp("guide") },
      { id: "help-news", title: "Novedades de cada versión", subtitle: "Lo que se ha añadido desde la primera", keywords: "cambios version changelog nuevo", run: () => openHelp("news") },
      { id: "help-report", title: "Reportar un problema", subtitle: "Prepara el correo para el autor con el diagnóstico adjunto", keywords: "fallo error bug soporte contacto", run: () => openHelp("report") },
      { id: "help-terms", title: "Términos de uso", keywords: "licencia responsabilidad garantia legal", run: () => openHelp("terms") },
      // Cosas que se hacen al momento, sin salir de donde estás: el resultado sale abajo.
      ...DO_NOW.map(([id, tweak, title, keywords]) => ({
        id: `do:${id}`,
        title,
        subtitle: "Se hace al pulsar Intro",
        keywords,
        run: () => tweaksApi.run(tweak).then((r) => `${title}: ${r.message}`),
      })),
      { id: "do:restore-point", title: "Crear un punto de restauración", subtitle: "Se hace al pulsar Intro", keywords: "restaurar sistema punto copia antes de cambiar", run: () => tweaksApi.createRestorePoint().then(() => "Punto de restauración creado.") },
      {
        id: "do:quick-scan",
        title: "Análisis rápido del equipo",
        subtitle: "Se hace al pulsar Intro: una primera mirada en segundos",
        keywords: "diagnostico rapido revisar mirar estado",
        run: () =>
          analyzeQuick().then((d) => {
            const n = d.findings.filter((f) => f.severity !== "info").length;
            return n ? `Análisis rápido: ${n} ${n === 1 ? "cosa que atender" : "cosas que atender"}. Está en Diagnóstico.` : "Análisis rápido: nada urgente.";
          }),
      },
      { id: "diag", title: "Ejecutar un diagnóstico", run: () => navigate("diagnostics") },
      { id: "report", title: "Generar informe PDF", run: () => navigate("report") },
      { id: "shortcut", title: "Añadir un acceso directo propio", run: () => navigate("tools") },
      { id: "join", title: "Unir el equipo a un dominio", run: () => navigate("domain") },
      { id: "newuser", title: "Crear un usuario local", run: () => navigate("users") },
      { id: "setup", title: "Volver a abrir el asistente de inicio", run: () => setOnboarding(true) },
      { id: "case", title: "Nuevo caso", subtitle: "Lo que hagas queda apuntado y la resolución se redacta sola", keywords: "ticket incidencia caso abrir atender", run: () => openCase() },
      { id: "minimon", title: "Mini monitor siempre encima", subtitle: "Una ventanita con procesador, memoria, temperatura y red", keywords: "monitor flotante widget vigilar temperatura cpu ram encima", run: () => workApi.miniMonitor() },
      { id: "note", title: "Nota de llamada", subtitle: "También con Ctrl+Alt+N, aunque AdminOps esté minimizado", keywords: "telefono llamada apuntar nota rapida", run: () => noteApi.open() },
      { id: "clip", title: "Recorte de pantalla", subtitle: "Tapa solo rutas, usuario y equipo; queda en el portapapeles para el ticket", keywords: "captura pantallazo imagen recortes ocr privacidad", run: () => noteApi.screenClip() },
      {
        id: "redact",
        title: "Tapar datos personales del portapapeles",
        subtitle: "Para una captura hecha fuera de AdminOps: rutas, usuario y equipo, con el OCR de Windows",
        keywords: "ocr privacidad captura recorte ocultar censurar",
        run: () =>
          void noteApi.redactClipboard().then(
            (r) => window.dispatchEvent(new CustomEvent("adminops:clip", { detail: r })),
            (e) => window.dispatchEvent(new CustomEvent("adminops:clip", { detail: { covered: 0, words: 0, error: String(e) } })),
          ),
      },
      // Síntomas: abren «Solucionar problemas» y lo comprueban.
      ...SYMPTOMS.map((s) => ({ id: `trouble:${s.id}`, title: `Solucionar: ${s.title}`, subtitle: s.hint, keywords: `${s.keywords} problema arreglar no funciona`, run: () => navigate("troubleshoot", s.id) })),
      { id: "netrepair", title: "Reparar la red", subtitle: "DNS, IP y adaptadores, con antes y después", keywords: "internet conexion winsock tcp ip renovar", run: () => navigate("network") },
      { id: "wifistate", title: "Estado de la Wi-Fi", subtitle: "Reiniciar la tarjeta, ahorro de energía, adaptadores fantasma", keywords: "wifi tarjeta adaptador driver codigo 10", run: () => navigate("network") },
      // Páginas que ahora son pestañas de otra: se siguen encontrando por su nombre.
      { id: "go:webinventory", title: "Inventario web de la empresa", subtitle: "Soporte → Inventario → Inventario web", keywords: "inventario web intranet portal empresa", run: () => navigate("inventory", "webinventory") },
      { id: "go:localaccount", title: "Pasar el equipo a una cuenta local", subtitle: "Cuentas → salir de Entra ID o de la cuenta de Microsoft", keywords: "cuenta profesional azure entra desconectar local microsoft", run: () => navigate("accounts") },
      { id: "go:winupdate", title: "Windows Update", subtitle: "Actualizaciones → Windows Update", keywords: "parches actualizaciones windows", run: () => navigate("winupdate") },
      { id: "go:devices", title: "Dispositivos en la red", subtitle: "Red → Dispositivos", keywords: "escanear red ip mac intrusos", run: () => navigate("devices") },
      { id: "go:inventory", title: "Inventario de equipos", subtitle: "Soporte → Inventario", keywords: "equipos clientes renovar", run: () => navigate("inventory") },
      { id: "go:repair", title: "Reparaciones de Windows", subtitle: "Solucionar problemas → Reparaciones", keywords: "sfc dism reparar winsock", run: () => navigate("repair") },
      { id: "sheet", title: "Ficha del equipo", subtitle: "Modelo, serie, licencia, red… para el inventario", keywords: "inventario serie numero modelo garantia licencia", run: () => navigate("hardware", "sheet") },
      { id: "newsolution", title: "Nueva solución", subtitle: "Apuntar lo que funcionó", keywords: "conocimiento base problema", run: () => navigate("knowledge", "solution:") },
      { id: "notes", title: "Notas de este equipo y de esta red", keywords: "apuntar recordar nota", run: () => navigate("knowledge", "notes") },
      { id: "recipes", title: "Preparar un equipo nuevo", keywords: "plantilla receta preparar instalar bloatware dominio usuario", run: () => navigate("recipes") },
      { id: "stations", title: "Comprobar puestos", subtitle: "Qué equipos responden y cuáles necesitan atención", keywords: "equipos oficina ping disco reinicio", run: () => navigate("stations") },
      { id: "backup", title: "Copia de seguridad cifrada de mis datos", keywords: "backup usb contactos restaurar", run: () => navigate("settings") },
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
  // Con la barra de solo áreas, las pantallas del área van como pestañas bajo el título.
  const showTabs = prefs.sidebar.mode === "mini";
  const tabs = showTabs ? (visibleAreas(page).find((a) => a.pages.includes(page))?.pages ?? []) : [];
  // La ruta de arriba: área › pantalla › sección.
  const section = sectionLabel(page, sections[page] ?? sectionsOf(page)[0]?.id);

  // Cada página se construye una vez y se mantiene viva al cambiar de página (conserva sus datos,
  // su scroll y lo que esté haciendo). Se vuelve a cargar con «Recargar» o al cerrar la app.
  const renderPage = (p: PageId) => {
    const category = TWEAK_PAGES[p];
    if (p === "dashboard") return <Dashboard onNavigate={navigate} />;
    if (p === "processes") return <Processes isAdmin={!!isAdmin} />;
    if (p === "troubleshoot") return <Troubleshoot isAdmin={!!isAdmin} focus={p === page ? focus : null} onNavigate={(x: PageId) => navigate(x)} />;
    if (p === "tweaks") return <WindowsTweaks isAdmin={!!isAdmin} focus={p === page ? focus : null} />;
    if (p === "machine") return <MachineState isAdmin={!!isAdmin} focus={p === page ? focus : null} onNavigate={navigate} />;
    if (p === "apps") return <Apps isAdmin={!!isAdmin} focus={p === page ? focus : null} />;
    if (p === "tools") return <WindowsToolbox isAdmin={!!isAdmin} focus={p === page ? focus : null} />;
    if (p === "data") return <DataTools focus={p === page ? focus : null} onNavigate={(x: PageId) => navigate(x)} isAdmin={!!isAdmin} />;
    if (p === "space") return <DiskTools isAdmin={!!isAdmin} focus={p === page ? focus : null} onNavigate={navigate} />;
    if (p === "session") return <ServiceSession onSessionChange={setSessionActive} focus={p === page ? focus : null} />;
    if (p === "agenda") return <Agenda onNavigate={navigate} focus={p === page ? focus : null} />;
    if (p === "people") return <PeopleAndClients focus={p === page ? focus : null} />;
    if (p === "contacts") return <Contacts focus={p === page ? focus : null} onNavigate={navigate} />;
    if (p === "knowledge") return <Knowledge focus={p === page ? focus : null} onNavigate={navigate} />;
    if (p === "recipes") return <RecipesAndProfiles isAdmin={!!isAdmin} focus={p === page ? focus : null} />;
    if (p === "stations") return <Workstations />;
    if (p === "inventory") return <InventoryPage covered={aboutOpen || helpOpen || paletteOpen || onboarding || alertsOpen || caseDialog || locked !== false || p !== page} focus={p === page ? focus : null} />;
    if (p === "users") return <AccountsAndDomain isAdmin={!!isAdmin} focus={p === page ? focus : null} />;
    if (p === "printers") return <PrintersAndShares isAdmin={!!isAdmin} focus={p === page ? focus : null} />;
    if (p === "tickets") return <Tickets split={split} onSplit={setSplit} covered={aboutOpen || helpOpen || paletteOpen || onboarding || alertsOpen || caseDialog || locked !== false || p !== page} />;
    if (p === "mail") return <Tickets kind="mail" split={split} onSplit={setSplit} covered={aboutOpen || helpOpen || paletteOpen || onboarding || alertsOpen || caseDialog || locked !== false || p !== page} />;
    if (p === "teams") return <Tickets kind="teams" split={split} onSplit={setSplit} covered={aboutOpen || helpOpen || paletteOpen || onboarding || alertsOpen || caseDialog || locked !== false || p !== page} />;
    if (p === "router") return <MyNetwork isAdmin={!!isAdmin} covered={aboutOpen || helpOpen || paletteOpen || onboarding || alertsOpen || caseDialog || locked !== false || p !== page} focus={p === page ? focus : null} />;
    if (p === "remote") return <Remote isAdmin={!!isAdmin} />;
    if (p === "settings") return <SettingsPage appInfo={appInfo} onNavigate={(x: PageId) => navigate(x)} focus={p === page ? focus : null} />;
    if (category) return <TweaksPage category={category} isAdmin={!!isAdmin} focus={p === page ? focus : null} />;
    return null;
  };

  return (
    <ToastProvider>
      {locked !== false && (locked === null ? <div className="fixed inset-0 z-[100] bg-void" /> : <LockScreen kind={lock?.kind ?? "password"} onUnlock={() => {
        lastActivity.current = Date.now();
        setLocked(false);
      }} />)}
      <div className="flex h-full flex-col">
      <TopBar
        state={machine}
        isAdmin={isAdmin}
        targetUser={targetUser}
        appInfo={appInfo}
        onAbout={() => setAboutOpen(true)}
        onSearch={() => setPaletteOpen(true)}
        onNavigate={goTo}
        right={
          <>
            <NewCaseButton hidden={false} />
            <TasksIndicator />
            <PrivacyToggle />
            <AuditToggle />
            <QuitButton />
            <AlertCenter onNavigate={(p, s) => goTo(p, s ?? null)} onOpenChange={setAlertsOpen} openSignal={alertSignal} />
          </>
        }
      />
      <div className={`flex min-h-0 flex-1 ${prefs.sidebar.position === "right" ? "flex-row-reverse" : ""}`}>
        <Sidebar
          active={page}
          onArea={openArea}
          onNavigate={goTo}
          isAdmin={isAdmin}
          badges={machine.badges}
          sessionActive={sessionActive}
          onLock={lock?.enabled ? () => setLocked(true) : undefined}
          update={update}
          recent={recent}
        />
        <main className="flex min-w-0 flex-1 flex-col">
          <AuditBanner />
          <PrivacyBanner />
          <CaseBar onOpenChange={setCaseDialog} />
          {prefs.pageTabs && openTabs.length > 1 && <OpenTabs tabs={openTabs} current={page} onPick={(p) => navigate(p)} onClose={closeTab} />}
          <header className="flex items-end justify-between border-b border-line px-8 pt-4 pb-4">
            <div className="min-w-0">
              {/* Dónde estás: área › pantalla › sección. Cada parte lleva a su sitio. */}
              <nav aria-label="Dónde estás" className="mb-1 flex min-w-0 flex-wrap items-center gap-1 text-xs text-mute">
                {area && (
                  <>
                    <button onClick={() => openArea(area)} className="rounded px-1 hover:bg-panel-2 hover:text-ink">
                      {area.label}
                    </button>
                    <span aria-hidden>›</span>
                  </>
                )}
                {section ? (
                  <>
                    <button onClick={() => goTo(page, sectionsOf(page)[0]?.id ?? null)} className="rounded px-1 hover:bg-panel-2 hover:text-ink">
                      {pageLabel(nav.id)}
                    </button>
                    <span aria-hidden>›</span>
                    <span className="px-1 text-dim">{section}</span>
                  </>
                ) : (
                  <span className="px-1 text-dim">{pageLabel(nav.id)}</span>
                )}
              </nav>
              <h1 className="flex items-center gap-2 text-[22px] font-semibold tracking-tight">
                <span className="truncate">{section ?? pageLabel(nav.id)}</span>
                <PageHelp text={nav.help} />
              </h1>
              {showTabs && tabs.length > 1 && (
                <div className="no-scrollbar mt-3 -mb-4 flex gap-1 overflow-x-auto">
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
            </div>
          </header>
          <div className="relative min-h-0 flex-1">
            {findOpen && <PageFind page={page} onClose={() => setFindOpen(false)} />}
            {alive.map((p) => {
              const left = splitOn && p === page;
              const right = splitOn && p === split;
              const visible = p === page || right;
              const pos = left ? "inset-y-0 left-0 w-[56%] border-r border-line" : right ? "inset-y-0 right-0 w-[44%]" : "inset-0";
              return (
              <div key={`${p}-${reloads[p] ?? 0}`} data-page={p} hidden={!visible} className={`absolute overflow-y-auto ${pos}`}>
                <PageIdContext.Provider value={p}>
                <PageActiveContext.Provider value={visible}>
                  <ErrorBoundary onHome={() => navigate("dashboard")}>
                    <Suspense fallback={<Loading page />}>
                      {renderPage(p)}
                      <PaintMark page={p} />
                    </Suspense>
                  </ErrorBoundary>
                </PageActiveContext.Provider>
                </PageIdContext.Provider>
              </div>
              );
            })}
          </div>
        </main>
      </div>
      </div>
      <Suspense fallback={null}>
        {aboutOpen && <About open onClose={() => setAboutOpen(false)} appInfo={appInfo} />}
        {helpOpen && <HelpCenter version={appInfo?.version ?? ""} />}
        {paletteOpen && <CommandPalette open onClose={() => setPaletteOpen(false)} onNavigate={navigate} onSection={goTo} badges={machine.badges} actions={actions} />}
      </Suspense>
      {/* Teams y el Correo: como se haya elegido, o pregunta la primera vez. */}
      <CommOpener onAdminOps={(p) => navigate(p)} />
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
