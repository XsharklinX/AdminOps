import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Activity, ArrowDownToLine, BookUser, ChevronLeft, ChevronRight, FolderOpen, Gauge, GripHorizontal, IdCard, Keyboard, LifeBuoy, Lock, MonitorUp, NotebookPen, Pencil, Printer, RotateCcw, ScanLine, SquareTerminal, Wifi, Wrench } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { api, appcareApi, diagApi, faultRecApi, FAULTREC_CAMERA_EVENT, FAULTREC_DONE_EVENT, FAULTREC_DRAW_EVENT, FAULTREC_MARKED_EVENT, FAULTREC_PAUSED_EVENT, FAULTREC_REGION_EVENT, FAULTREC_TICK_EVENT, lanApi, noteApi, notesApi, sheetApi, toolsApi, workApi, type FaultRecDone, type FaultRecWindow, type FaultRegionPicked, type Note, type SpeedResult } from "../lib/api";
import { logQuietly } from "../lib/api/core";
import { rocketApi, ROCKET_DOCK_EVENT } from "../lib/api/rocket";
import { barApi, BAR_EVENT, BAR_TOOL_EVENT, BAR_TOOLS, DEFAULT_TOOLS, TOOL_LABEL, USER_TOOLS, type BarTool } from "../lib/api/rocketbar";
import { EXTRA_H, ExtraPanel, isExtra, type ExtraTool } from "./BarExtra";
import { clock, pillText } from "../lib/faultrec";
import { bytes } from "../lib/format";
import { BUBBLE, PANEL_H, PANEL_W } from "../lib/rocket";
import { OutlookIcon, RecDot, TeamsIcon } from "./BrandIcons";
import { bubbleProps, RocketBubble, SidePanel, usePress, useRocket } from "./Rocket";
import logo from "../assets/logo.svg";

/**
 * La barra de herramientas del cohete (aspecto C, elegido el 26 nov): el cohete arriba como cabeza y las
 * herramientas debajo, vertical, pegada al borde derecho (o al izquierdo si se lleva allí). Escondida asoma
 * medio cohete con una flecha; al tocarla sale la barra, y se vuelve a esconder sola a los 5 s sin el ratón.
 * Ctrl+Alt+T la saca o la esconde. Clic derecho en el cohete → «Personalizar la barra».
 */

const STEP = 40; // alto de cada herramienta (botón de 36 + hueco)
/** El pie de la barra: el lápiz (32 + 4), la flecha de esconder (28 + 4) y el remate redondeado de abajo (18). */
const FOOT_H = 86;
const TOOL_PANEL_H = { update: 120, menu: 150, msg: 68, rec: 228, recctl: 140, fix: 0, custom: 420, speed: 290, card: 350, recdone: 262, glance: 150, notes: 300 } as const;
const HIDE_AFTER_MS = 5000;
/** Personalizar: cabecera + pie (Guardar) y cada fila (la pastilla del icono mide 22 px aunque se vea al 80 %). */
const CUSTOM_HEAD = 108;
const CUSTOM_ROW = 28;
/** Lo que mide el panel de una herramienta. */
function toolPanelH(t: Exclude<ToolPanel, null>): number {
  return t.kind === "fix" ? 60 + (t.fixes?.length ?? 2) * 30 : t.kind === "extra" ? EXTRA_H[t.tool] : t.kind === "custom" ? Math.min(CUSTOM_HEAD + t.list.length * CUSTOM_ROW, screenMaxH()) : TOOL_PANEL_H[t.kind];
}

/** De qué herramienta es un panel (para abrirlo a su altura). */
function ownerOf(t: Exclude<ToolPanel, null>, last: BarTool | null): BarTool | null {
  switch (t.kind) {
    case "rec":
    case "recctl":
    case "recdone":
      return "rec";
    case "speed":
    case "card":
    case "fix":
      return t.kind;
    case "extra":
    case "menu":
      return t.tool;
    case "notes":
      return "note";
    case "msg":
      return last;
    default:
      return null;
  }
}

/** Lo más alto que puede ser la ventana de la barra: la pantalla, en píxeles de la página (con el zoom del cohete). */
function screenMaxH() {
  const zoom = (window as unknown as { __ADMINOPS_ROCKET_ZOOM__?: number }).__ADMINOPS_ROCKET_ZOOM__ ?? 1;
  return Math.max(240, Math.floor(window.screen.availHeight / zoom) - 12);
}
/** La pestaña escondida (solo la flecha) y lo que se espera tras abrir antes de poder esconderse. */
const TAB_W = 22;
const TAB_H = 64;
const GRACE_MS = 1200;

const LABEL = TOOL_LABEL;
const KEY: Partial<Record<BarTool, string>> = { rec: "Ctrl+Alt+G", note: "Ctrl+Alt+N" };

/** Cada herramienta con su color, como grabar o Teams: se reconoce de un vistazo sin leer. */
const TOOL_COLOR: Partial<Record<BarTool, string>> = {
  clip: "#a78bfa",
  note: "#f5b84b",
  mon: "#3ecf8e",
  fix: "#fb923c",
  speed: "#22d3ee",
  card: "#60a5fa",
  help: "#f472b6",
  contacts: "#2dd4bf",
  printer: "#94a3b8",
  screens: "#818cf8",
  wifi: "#38bdf8",
  keyboard: "#e2e8f0",
  folders: "#fcd34d",
  lock: "#fbbf24",
  restart: "#34d399",
  cmd: "#a3e635",
};

/** El icono dentro de una pastilla translúcida de su color. */
function Chip({ t, children }: { t: BarTool; children: ReactNode }) {
  const c = TOOL_COLOR[t] ?? "#e8eef4";
  return (
    <span className="rkb-chip" style={{ color: c, backgroundColor: `${c}29`, boxShadow: `inset 0 0 0 1px ${c}33` }}>
      {children}
    </span>
  );
}

function icon(t: BarTool, recording = false): ReactNode {
  switch (t) {
    case "rec":
      return <RecDot size={20} recording={recording} />;
    case "clip":
      return <Chip t={t}><ScanLine size={16} strokeWidth={2.1} /></Chip>;
    case "note":
      return <Chip t={t}><NotebookPen size={16} strokeWidth={2.1} /></Chip>;
    case "mon":
      return <Chip t={t}><Activity size={16} strokeWidth={2.1} /></Chip>;
    case "fix":
      return <Chip t={t}><Wrench size={16} strokeWidth={2.1} /></Chip>;
    case "teams":
      return <TeamsIcon size={18} />;
    case "mail":
      // El mismo icono de correo que la cabecera de AdminOps.
      return <OutlookIcon size={18} />;
    case "app":
      return <img src={logo} alt="" className="h-[18px] w-[18px]" />;
    case "speed":
      return <Chip t={t}><Gauge size={16} strokeWidth={2.1} /></Chip>;
    case "card":
      return <Chip t={t}><IdCard size={16} strokeWidth={2.1} /></Chip>;
    case "help":
      return <Chip t={t}><LifeBuoy size={16} strokeWidth={2.1} /></Chip>;
    case "contacts":
      return <Chip t={t}><BookUser size={16} strokeWidth={2.1} /></Chip>;
    case "printer":
      return <Chip t={t}><Printer size={16} strokeWidth={2.1} /></Chip>;
    case "screens":
      return <Chip t={t}><MonitorUp size={16} strokeWidth={2.1} /></Chip>;
    case "wifi":
      return <Chip t={t}><Wifi size={16} strokeWidth={2.1} /></Chip>;
    case "keyboard":
      return <Chip t={t}><Keyboard size={16} strokeWidth={2.1} /></Chip>;
    case "folders":
      return <Chip t={t}><FolderOpen size={16} strokeWidth={2.1} /></Chip>;
    case "lock":
      return <Chip t={t}><Lock size={16} strokeWidth={2.1} /></Chip>;
    case "restart":
      return <Chip t={t}><RotateCcw size={16} strokeWidth={2.1} /></Chip>;
    case "cmd":
      return <Chip t={t}><SquareTerminal size={16} strokeWidth={2.1} /></Chip>;
  }
}

type ToolPanel =
  | { kind: "update"; latest: string; size: number; state: "ask" | "busy" | "done"; msg?: string }
  | { kind: "msg"; text: string; ok: boolean }
  | { kind: "menu"; tool: BarTool }
  | { kind: "rec"; windows: FaultRecWindow[] | null; mic: boolean; system: boolean; camera: boolean; msg?: string }
  | { kind: "recctl"; paused: boolean; drawing: boolean; camera: boolean; msg?: string }
  | { kind: "fix"; fixes: [string, string][] | null; msg?: string }
  | { kind: "custom"; list: { id: BarTool; on: boolean }[]; msg?: string }
  | { kind: "notes"; notes: Note[] | null; copied?: string; msg?: string }
  | { kind: "speed"; running: boolean; result: SpeedResult | null; history: SpeedResult[]; msg?: string }
  | { kind: "card"; rows: [string, string][] | null; copied?: string; msg?: string }
  | { kind: "recdone"; id: string; thumb: string | null; secs: number | null; msg?: string; ok?: boolean; busy?: "small" | "gif" }
  | { kind: "glance" }
  | { kind: "extra"; tool: ExtraTool }
  | null;

export function RocketBar() {
  const zoom = (window as unknown as { __ADMINOPS_ROCKET_ZOOM__?: number }).__ADMINOPS_ROCKET_ZOOM__ ?? 1;
  const [open, setOpen] = useState(false);
  const [right, setRight] = useState(true);
  const [tools, setTools] = useState<BarTool[]>([...DEFAULT_TOOLS]);
  const [tool, setToolState] = useState<ToolPanel>(null);
  const [holding, setHolding] = useState(false);
  const [recSecs, setRecSecs] = useState<number | null>(null);
  const [fixDot, setFixDot] = useState(false);
  const [lastSpeed, setLastSpeed] = useState<number | null>(null);
  // Versión nueva lista para instalar: la flecha verde arriba de la barra.
  const [update, setUpdate] = useState<{ latest: string; size: number } | null>(null);
  // Sin administrador (los PC de los usuarios): ni abrir AdminOps entera ni instalar la actualización (piden credenciales).
  const [admin, setAdmin] = useState(true);
  const adminRef = useRef(true);
  // Los manejadores que se crean al montar leen la lista de ahora, no la de fábrica.
  const toolsRef = useRef<BarTool[]>(tools);
  toolsRef.current = tools;
  const runRef = useRef<(t: BarTool) => Promise<void>>(async () => {});
  // Cada panel se abre a la altura del botón que lo abrió (no siempre arriba, junto al cohete).
  const btnRefs = useRef<Partial<Record<BarTool, HTMLButtonElement | null>>>({});
  const rowRef = useRef<HTMLDivElement | null>(null);
  const lastToolRef = useRef<BarTool | null>(null);
  const [panelTop, setPanelTop] = useState(0);
  // Herramientas trabajando ahora (para no empezarlas otra vez con un segundo toque) y lo que enseñan mientras.
  const busyRef = useRef(new Set<BarTool>());
  const liveRef = useRef<Partial<Record<BarTool, Exclude<ToolPanel, null>>>>({});
  // El aviso abierto es el de Asistencia rápida (los avisos son de varias herramientas).
  const helpOwnRef = useRef(false);
  const hideRef = useRef<() => void>(() => {});
  const glanceTimer = useRef(0);
  const openRef = useRef(false);
  const toolRef = useRef<ToolPanel>(null);
  const inside = useRef(false);
  const hideTimer = useRef(0);
  const kindRef = useRef<keyof typeof PANEL_H | null>(null);
  const openedAt = useRef(0);

  // Cohete + agarre + herramientas + lápiz; como mucho, lo que quepa en la pantalla (el resto, con scroll).
  const maxH = screenMaxH();
  // La flecha verde de actualizar ocupa su sitio en la columna (38 px).
  const updH = update && admin ? 38 : 0;
  const barH = Math.min(BUBBLE + 14 + tools.length * STEP + FOOT_H + updH, maxH);
  const toolsH = barH - BUBBLE - 14 - FOOT_H - updH;
  // `layout` se usa en manejadores creados al montar (Ctrl+Alt+T, Esc, avisos): lee el alto de ahora por ref, no el
  // del primer render (la lista de fábrica).
  const barHRef = useRef(barH);
  barHRef.current = barH;
  // Coloca la ventana: escondida (medio cohete), abierta (la barra) o con un panel al lado.
  const layout = useCallback(
    (wantOpen: boolean) => {
      const k = kindRef.current;
      const t = toolRef.current;
      const panelH = t ? toolPanelH(t) : k ? PANEL_H[k] : 0;
      const isOpen = wantOpen || !!k || !!t;
      if (isOpen && !openRef.current) openedAt.current = Date.now();
      openRef.current = isOpen;
      setOpen(isOpen);
      const w = isOpen ? (k || t ? PANEL_W : BUBBLE) : TAB_W;
      const h = isOpen ? Math.max(barHRef.current, panelH) : TAB_H;
      rocketApi
        .bar(isOpen, w * zoom, h * zoom)
        .then(setRight)
        .catch(logQuietly("RocketBar.layout"));
    },
    [zoom],
  );
  const r = useRocket({
    inApp: false,
    onPanel: (k) => {
      kindRef.current = k;
      layout(openRef.current);
    },
  });
  const { panelRef, setPanel } = r;
  const setTool = useCallback(
    (t: ToolPanel) => {
      const changed = toolRef.current?.kind !== t?.kind || (t?.kind === "extra" && toolRef.current?.kind === "extra" && toolRef.current.tool !== t.tool) || (t?.kind === "fix" && toolRef.current?.kind === "fix" && toolRef.current.fixes?.length !== t.fixes?.length);
      toolRef.current = t;
      setToolState(t);
      if (t && panelRef.current) setPanel(null);
      if (changed) layout(openRef.current);
    },
    [layout, setPanel, panelRef],
  );

  // Se esconde sola a los 5 s sin el ratón, salvo con algo a la vista o grabando.
  const scheduleHide = useCallback(() => {
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      if (Date.now() - openedAt.current < GRACE_MS || inside.current || toolRef.current || panelRef.current || r.phase === "working" || recSecs !== null) return scheduleHide();
      layout(false);
    }, HIDE_AFTER_MS);
  }, [layout, panelRef, r.phase, recSecs]);

  const toggle = useCallback(() => {
    if (openRef.current && !toolRef.current && !panelRef.current) layout(false);
    else {
      layout(true);
      scheduleHide();
    }
  }, [layout, panelRef, scheduleHide]);

  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    (document.body.style as CSSStyleDeclaration & { zoom: string }).zoom = String(zoom);
    // AdminOps se abre sin elevar: lo que cuenta es si el USUARIO es administrador del equipo (su token, también por
    // un grupo del dominio), no si esta ventana lo es.
    const profileP = barApi.profile().catch(() => ({ admin: true, userTools: null }));
    void profileP.then((p) => {
      adminRef.current = p.admin;
      setAdmin(p.admin);
      if (!p.admin) setUpdate(null);
    });
    const known = (list: string[]) => list.filter((t): t is BarTool => (BAR_TOOLS as readonly string[]).includes(t));
    workApi
      .settings()
      .then(async (s) => {
        // Lo que personalizó él con el lápiz (sus ajustes son suyos: cada usuario tiene los suyos).
        if (s.rocketTools) return setTools(known(s.rocketTools));
        // Sin elegir: un usuario que no es administrador lleva la barra que el técnico eligió para este equipo, o
        // la de fábrica para usuarios.
        const p = await profileP;
        if (!p.admin) setTools(p.userTools ? known(p.userTools) : USER_TOOLS);
      })
      .catch(() => {});
    faultRecApi
      .status()
      .then((s) => setRecSecs(s.recording ? s.secs : null))
      .catch(() => {});
    diagApi
      .latest()
      .then((l) => setFixDot(!!l?.findings.some((f) => f.severity === "warn" || f.severity === "bad")))
      .catch(() => {});
    rocketApi.dockState().then((d) => d && setRight(d === "right"), () => {});
    const checkUpdate = () =>
      appcareApi
        .checkUpdate()
        .then((u) => setUpdate(adminRef.current && u.newer && u.installable ? { latest: u.latest, size: u.size } : null))
        .catch(() => {});
    const firstCheck = window.setTimeout(checkUpdate, 20_000);
    const everyCheck = window.setInterval(checkUpdate, 6 * 3600_000);
    toolsApi
      .speedHistory()
      .then((h) => h.length && setLastSpeed(Math.round(h[h.length - 1].downloadMbps)))
      .catch(() => {});
    layout(false);
    const offs = [
      listen(BAR_EVENT, () => toggle()),
      // Se conectó o quitó la base o un monitor: el cohete ya está en su sitio nuevo; la barra se recoge allí.
      listen("rocket://screens", () => {
        setTool(null);
        setPanel(null);
        layout(false);
      }),
      // Ctrl+Alt+Mayús+número: abre la barra y la herramienta de ese puesto.
      listen<number>(BAR_TOOL_EVENT, (e) => {
        const t = toolsRef.current[e.payload];
        if (!t) return;
        layout(true);
        void runRef.current(t);
        hideRef.current();
      }),
      listen<"left" | "right" | null>(ROCKET_DOCK_EVENT, (e) => e.payload && setRight(e.payload === "right")),
      listen<number>(FAULTREC_TICK_EVENT, (e) => setRecSecs(e.payload)),
      // Pausar y seguir (desde aquí, desde la píldora o desde AdminOps): el panel de controles lo refleja.
      listen<boolean>(FAULTREC_PAUSED_EVENT, (e) => toolRef.current?.kind === "recctl" && setTool({ ...toolRef.current, paused: e.payload })),
      listen<boolean>(FAULTREC_DRAW_EVENT, (e) => toolRef.current?.kind === "recctl" && setTool({ ...toolRef.current, drawing: e.payload })),
      listen<boolean>(FAULTREC_CAMERA_EVENT, (e) => toolRef.current?.kind === "recctl" && setTool({ ...toolRef.current, camera: e.payload })),
      // Una marca puesta con Ctrl+Alt+M: se confirma en el panel de controles, si está abierto.
      listen<number>(FAULTREC_MARKED_EVENT, (e) => toolRef.current?.kind === "recctl" && setTool({ ...toolRef.current, msg: `Marca puesta en ${clock(e.payload)}` })),
      // El reinicio de esta noche no se hizo (el equipo estaba suspendido a esa hora).
      listen("rocketbar://restart-skipped", () => {
        layout(true);
        setTool({ kind: "msg", text: "No se reinició esta noche: el equipo estaba suspendido a esa hora. Se puede volver a programar.", ok: false });
      }),
      listen<FaultRecDone>(FAULTREC_DONE_EVENT, (e) => {
        setRecSecs(null);
        const id = e.payload.id;
        if (!id || e.payload.replay) return;
        layout(true);
        setTool({ kind: "recdone", id, thumb: null, secs: null });
        faultRecApi.thumb(id).then((thumb) => toolRef.current?.kind === "recdone" && setTool({ ...toolRef.current, thumb }), () => {});
        faultRecApi.get(id).then((d) => toolRef.current?.kind === "recdone" && setTool({ ...toolRef.current, secs: Math.round(d.meta.secs) }), () => {});
      }),
    ];
    const custom = () => {
      const now = toolsRef.current;
      setTool({ kind: "custom", list: [...now.map((id) => ({ id, on: true })), ...BAR_TOOLS.filter((t) => !now.includes(t)).map((id) => ({ id, on: false }))] });
    };
    window.addEventListener("rocket:custom", custom);
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (toolRef.current) setTool(null);
      else if (panelRef.current) setPanel(null);
      else layout(false);
    };
    window.addEventListener("keydown", key);
    return () => {
      window.clearTimeout(firstCheck);
      window.clearInterval(everyCheck);
      offs.forEach((o) => void o.then((f) => f()));
      window.removeEventListener("rocket:custom", custom);
      window.removeEventListener("keydown", key);
    };
    // Una sola vez al montar: los manejadores leen por refs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Cambió el alto de la barra (otra lista al guardar Personalizar, o salió la flecha de actualizar): abierta,
  // la ventana se ajusta ya, sin esperar a que se esconda y vuelva a salir.
  useEffect(() => {
    if (openRef.current) layout(true);
  }, [barH, layout]);

  const press = usePress({
    // Escondida, tocar el cohete saca la barra; abierta, despega.
    tap: () => (openRef.current ? void r.launch() : toggle()),
    hold: () => (openRef.current ? r.deep() : toggle()),
    pressing: (on) => setHolding(on && openRef.current),
    // Se mueve escondida: al soltarla se pega al borde más cercano, a esa altura.
    drag: () => {
      if (openRef.current) return;
      void getCurrentWindow().startDragging().catch(logQuietly("RocketBar.drag"));
    },
  });
  // Arrastrar abierta: por el agarre. Se recoge y se arrastra el cohete.
  const grip = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    setTool(null);
    setPanel(null);
    layout(false);
    window.setTimeout(() => void getCurrentWindow().startDragging().catch(logQuietly("RocketBar.grip")), 120);
  };

  /** Lo que hace cada opción del clic derecho de una herramienta. */
  const option = async (t: BarTool, o: string) => {
    lastToolRef.current = t;
    try {
      if (t === "rec" && o === "now") await faultRecApi.start(null, { input: false, mic: false, systemAudio: false });
      else if (t === "speed" && o === "history") setTool({ kind: "speed", running: false, result: null, history: await toolsApi.speedHistory() });
      else if (t === "note" && o === "all") {
        // Sin administrador no se abre AdminOps entera (pide credenciales): las notas, aquí al lado.
        if (!adminRef.current) {
          setTool({ kind: "notes", notes: null });
          const notes = await notesApi.list();
          if (toolRef.current?.kind === "notes") setTool({ kind: "notes", notes });
          return;
        }
        try {
          localStorage.setItem("adminops.knowledge.tab", "notebook");
        } catch {
          /* sin almacenamiento */
        }
        await rocketApi.openMain();
        await emit("tray-action", "notebook");
      }
      else if (t === "mon" && o === "glance") setTool({ kind: "glance" });
      else if (t === "card" && o === "copy") {
        const s = await sheetApi.get();
        await navigator.clipboard.writeText([`Equipo: ${s.host}`, `IP: ${s.ip}`, `Usuario: ${s.user}`, `Número de serie: ${s.serial}`, `Modelo: ${s.manufacturer} ${s.model}`].join("\n"));
        setTool({ kind: "msg", text: "Datos del equipo copiados", ok: true });
      } else if (t === "mail" && o === "open") await barApi.openComm("outlook");
      else if (t === "screens" && (o === "clone" || o === "extend")) {
        await barApi.displayMode(o);
        setTool({ kind: "msg", text: o === "clone" ? "Pantallas duplicadas" : "Escritorio extendido", ok: true });
      } else if (t === "wifi" && o === "reconnect") setTool({ kind: "msg", text: await barApi.wifiReconnect(), ok: true });
      else if (t === "folders" && (o === "desktop" || o === "downloads")) await barApi.openFolder(o);
      else if (t === "cmd" && (o === "cmd-admin" || o === "powershell")) await barApi.console(o);
      else if (t === "restart" && o === "tonight") {
        const r = await barApi.restart("tonight");
        setTool({ kind: "msg", text: r.tonight ? `Se reiniciará a las ${new Date(r.tonight * 1000).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" })}. Deja AdminOps abierto.` : "Programado", ok: true });
      }
      else await run(t);
    } catch (e) {
      setTool({ kind: "msg", text: String(e), ok: false });
    }
  };
  const run = async (t: BarTool) => {
    lastToolRef.current = t;
    const cur = toolRef.current;
    // ¿El panel abierto es el de esta herramienta?
    const own =
      !!cur &&
      (cur.kind === "extra" ? cur.tool === t : t === "rec" ? cur.kind === "rec" || cur.kind === "recctl" : t === "help" ? cur.kind === "msg" && helpOwnRef.current : cur.kind === t);
    // Trabajando (midiendo, leyendo, abriendo): otro toque no la vuelve a empezar ni la cierra. Si su panel se cerró
    // mientras tanto, vuelve a verse tal como va.
    if (busyRef.current.has(t)) {
      const back = liveRef.current[t];
      if (!own && back) setTool(back);
      return;
    }
    // Terminada y con su panel abierto: otro toque lo cierra.
    if (own) {
      setTool(null);
      return;
    }
    busyRef.current.add(t);
    helpOwnRef.current = false;
    try {
      if (t === "rec") {
        // Grabando, el botón abre los controles (pausar, marcar, parar) en vez de parar de golpe.
        if (recSecs !== null) {
          const s = await faultRecApi.status().catch(() => null);
          setTool({ kind: "recctl", paused: !!s?.paused, drawing: !!s?.drawing, camera: !!s?.camera });
        } else setTool({ kind: "rec", windows: null, mic: false, system: false, camera: false });
      } else if (t === "clip") await barApi.screenClip();
      else if (t === "note") await noteApi.open();
      else if (t === "mon") await workApi.miniMonitor();
      else if (t === "fix") {
        liveRef.current.fix = { kind: "fix", fixes: null };
        setTool({ kind: "fix", fixes: null });
        const fixes = await barApi.fixes();
        if (toolRef.current?.kind === "fix") setTool({ kind: "fix", fixes });
      } else if (t === "teams" || t === "mail") await barApi.openComm(t);
      else if (t === "app") await rocketApi.openMain();
      else if (t === "lock") await barApi.lock();
      else if (t === "cmd") await barApi.console("cmd");
      else if (isExtra(t)) setTool({ kind: "extra", tool: t });
      else if (t === "help") {
        liveRef.current.help = { kind: "msg", text: "Abriendo Asistencia rápida…", ok: true };
        setTool(liveRef.current.help);
        helpOwnRef.current = true;
        const text = await barApi.quickAssist();
        setTool({ kind: "msg", text, ok: true });
        helpOwnRef.current = true;
      }
      else if (t === "speed") {
        const history = await toolsApi.speedHistory().catch(() => [] as SpeedResult[]);
        liveRef.current.speed = { kind: "speed", running: true, result: null, history };
        setTool(liveRef.current.speed);
        try {
          const result = await toolsApi.speedtest();
          setLastSpeed(Math.round(result.downloadMbps));
          if (toolRef.current?.kind === "speed") setTool({ kind: "speed", running: false, result, history });
        } catch (e) {
          if (toolRef.current?.kind === "speed") setTool({ kind: "speed", running: false, result: null, history, msg: String(e) });
        }
      } else if (t === "card") {
        liveRef.current.card = { kind: "card", rows: null };
        setTool(liveRef.current.card);
        const s = await sheetApi.get();
        // La IP pública se pregunta ahora (la de la última prueba de velocidad puede ser de otra red).
        const [net, live, pub, extra] = await Promise.all([
          barApi.network().catch(() => ""),
          api.liveMetrics().catch(() => null),
          lanApi.publicIp().catch(() => null),
          barApi.ticketExtras().catch(() => ({ printer: "", antivirus: "", lastUpdate: "" })),
        ]);
        const sys = live?.disks.find((d) => /^c:/i.test(d.mount)) ?? live?.disks[0];
        const lastIp = pub?.ip ?? null;
        const days = live ? Math.floor(live.uptime / 86400) : null;
        const rows: [string, string][] = [
          ["Equipo", s.host],
          ["IP", s.ip],
          ["IP pública", lastIp ?? ""],
          ["Red", net],
          ["Usuario", s.domain ? `${s.domain}\\${s.user}` : s.user],
          ["Windows", [s.os, s.osVersion].filter(Boolean).join(" ")],
          ["Encendido", days === null ? "" : days === 0 ? "hoy" : `${days} ${days === 1 ? "día" : "días"}`],
          ["Impresora", extra.printer],
          ["Antivirus", extra.antivirus],
          ["Espacio en C:", sys ? `${bytes(sys.available)} libres de ${bytes(sys.total)}` : ""],
          ["Última actualización", extra.lastUpdate],
          ["Número de serie", s.serial],
          ["Modelo", [s.manufacturer, s.model].filter(Boolean).join(" ")],
        ];
        if (toolRef.current?.kind === "card") setTool({ kind: "card", rows: rows.filter(([, v]) => v) });
      }
    } catch (e) {
      setTool({ kind: "msg", text: String(e), ok: false });
    } finally {
      busyRef.current.delete(t);
      delete liveRef.current[t];
    }
  };
  // Los atajos (manejador creado al montar) usan el `run` de ahora, con el estado de ahora.
  runRef.current = run;
  hideRef.current = scheduleHide;

  // A la altura del botón de su herramienta; si ahí no cabe, sube lo justo para entrar en la barra.
  useLayoutEffect(() => {
    if (!tool || !rowRef.current) return setPanelTop(0);
    const owner = ownerOf(tool, lastToolRef.current);
    const el = owner ? btnRefs.current[owner] : null;
    if (!el) return setPanelTop(0);
    const y = (el.getBoundingClientRect().top - rowRef.current.getBoundingClientRect().top) / zoom;
    setPanelTop(Math.max(0, Math.min(Math.round(y), barH - toolPanelH(tool))));
  }, [tool, barH, zoom]);

  const tucked = !open;
  const memBadge = r.mem !== null && r.mem >= 80 ? `${r.mem}%` : null;
  const side = right ? "left" : "right";
  return (
    <div
      ref={rowRef}
      className={`flex h-screen w-screen select-none items-start ${right ? "flex-row-reverse" : ""}`}
      onPointerEnter={() => {
        inside.current = true;
        window.clearTimeout(hideTimer.current);
      }}
      onPointerLeave={() => {
        inside.current = false;
        if (openRef.current) scheduleHide();
      }}
    >
      {tucked && (
        <button type="button" className={`rkb-tab ${right ? "rkb-tab-r" : "rkb-tab-l"}`} aria-label="Sacar la barra de AdminOps" title="Sacar la barra (Ctrl+Alt+T)" onPointerDown={press}>
          {right ? <ChevronLeft size={16} /> : <ChevronRight size={16} />}
        </button>
      )}
      {!tucked && (
      <div className={`rkb-col ${right ? "rkb-right" : "rkb-left"}`}>
        <div
          className="relative"
          // Con el ratón quieto 600 ms sobre el cohete (también si ya estaba encima al abrir la barra).
          onPointerMove={() => {
            if (!openRef.current || toolRef.current || panelRef.current) return;
            window.clearTimeout(glanceTimer.current);
            glanceTimer.current = window.setTimeout(() => {
              if (!toolRef.current && !panelRef.current && openRef.current) setTool({ kind: "glance" });
            }, 600);
          }}
          onPointerDown={() => {
            window.clearTimeout(glanceTimer.current);
            if (toolRef.current?.kind === "glance") setTool(null);
          }}
          onPointerLeave={() => {
            window.clearTimeout(glanceTimer.current);
            if (toolRef.current?.kind === "glance") setTool(null);
          }}
        >
          <button {...bubbleProps(r, press)} title={tucked ? "Sacar la barra (Ctrl+Alt+T)" : undefined}>
            <RocketBubble phase={r.phase} mem={r.mem} hung={!!r.hung} pulse={r.high && r.phase === "idle"} holding={holding} />
          </button>
          {memBadge && !tucked && <span className="rkb-badge">{memBadge}</span>}
        </div>
        {!tucked && (
          <>
            <button type="button" className="rkb-grip" title="Arrastrar para moverla" onPointerDown={grip}>
              <GripHorizontal size={14} />
            </button>
            {update && admin && (
              <button type="button" className="rkb-btn rkb-update" aria-label={`Actualizar a la ${update.latest}`} title={`Versión ${update.latest} lista: actualizar ahora`} onClick={() => setTool({ kind: "update", latest: update.latest, size: update.size, state: "ask" })}>
                <ArrowDownToLine size={17} />
              </button>
            )}
            <div className="rkb-tools" style={{ maxHeight: toolsH }}>
            {tools.map((t, i) => (
              <button
                key={t}
                type="button"
                className={`rkb-btn ${t === "rec" && recSecs !== null ? "rkb-on" : ""}`}
                aria-label={LABEL[t]}
                title={`${t === "rec" && recSecs !== null ? "Controles de la grabación" : LABEL[t]}${KEY[t] ? ` (${KEY[t]})` : ""}${i < 9 ? ` · Ctrl+Alt+Mayús+${i + 1}` : ""}`}
                ref={(el) => {
                  btnRefs.current[t] = el;
                }}
                onClick={() => void run(t)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setTool({ kind: "menu", tool: t });
                }}
              >
                {icon(t, t === "rec" && recSecs !== null)}
                {t === "rec" && recSecs !== null && <span className="rkb-badge rkb-rec">{pillText(recSecs).replace(/^.*?(\d+:\d+).*$/, "$1")}</span>}
                {t === "fix" && fixDot && <span className="rkb-dot" />}
                {t === "speed" && lastSpeed !== null && <span className="rkb-badge rkb-ok">{lastSpeed}</span>}
              </button>
            ))}
            </div>
            <button type="button" className="rkb-btn rkb-edit" aria-label="Personalizar la barra" title="Personalizar la barra" onClick={() => window.dispatchEvent(new Event("rocket:custom"))}>
              <Pencil size={15} />
            </button>
            {/* Esconderla a mano (además de sola a los 5 s y Ctrl+Alt+T): la flecha apunta al borde donde se guarda. */}
            <button
              type="button"
              className="rkb-btn rkb-hide"
              aria-label="Esconder la barra"
              title="Esconder la barra (Ctrl+Alt+T)"
              onClick={() => {
                setTool(null);
                setPanel(null);
                layout(false);
              }}
            >
              {right ? <ChevronRight size={17} /> : <ChevronLeft size={17} />}
            </button>
          </>
        )}
      </div>
      )}
      {r.panel && <SidePanel panel={r.panel} side={side} inApp={false} setPanel={r.setPanel} act={r.act} ops={r.ops} />}
      {tool && (
        <div className="min-w-0 flex-1" style={{ marginTop: panelTop, maxWidth: 274 }}>
          <ToolSide tool={tool} recSecs={recSecs} side={side} setTool={setTool} tools={tools} setTools={setTools} onOption={(t, o) => void option(t, o)} />
        </div>
      )}
    </div>
  );
}

const btn = "rounded-md px-2 py-[4px] text-left text-[11.5px] font-medium text-[#e8eef4] transition-colors hover:bg-[#2e3b49] disabled:opacity-50";

/** Las opciones del clic derecho de cada herramienta: [id, texto]. «main» es lo mismo que un clic normal. */
const OPTIONS: Record<BarTool, [string, string][]> = {
  rec: [["now", "Grabar la pantalla entera ya"], ["main", "Elegir ventana, zona o voz…"]],
  clip: [["main", "Recortar una zona"]],
  speed: [["main", "Medir ahora"], ["history", "Ver las mediciones anteriores"]],
  note: [["main", "Nota nueva"], ["all", "Ver todas las notas"]],
  mon: [["main", "Abrir el mini monitor"], ["glance", "Vistazo aquí al lado"]],
  fix: [["main", "Ver los arreglos rápidos"]],
  card: [["main", "Ver los datos"], ["copy", "Copiar todo"]],
  help: [["main", "Abrir Asistencia rápida"]],
  teams: [["main", "Abrir Teams"]],
  mail: [["main", "Correo nuevo"], ["open", "Abrir Outlook"]],
  app: [["main", "Abrir AdminOps"]],
  contacts: [["main", "Buscar un contacto"]],
  printer: [["main", "Ver la impresora"]],
  screens: [["clone", "Duplicar (proyector)"], ["extend", "Extender"], ["main", "Más opciones…"]],
  wifi: [["main", "Ver la Wi-Fi y el Bluetooth"], ["reconnect", "Reconectar la Wi-Fi"]],
  keyboard: [["main", "Ver el teclado"]],
  folders: [["desktop", "Abrir el Escritorio"], ["downloads", "Abrir Descargas"], ["main", "Todas mis carpetas…"]],
  lock: [["main", "Bloquear ahora (Win+L)"]],
  restart: [["main", "Reiniciar…"], ["tonight", "Reiniciar esta noche"]],
  cmd: [["main", "Abrir CMD"], ["cmd-admin", "CMD como administrador"], ["powershell", "Abrir PowerShell"]],
};

function ToolSide({ tool, recSecs, side, setTool, tools, setTools, onOption }: { tool: Exclude<ToolPanel, null>; recSecs: number | null; side: "left" | "right"; setTool: (t: ToolPanel) => void; tools: BarTool[]; setTools: (t: BarTool[]) => void; onOption: (t: BarTool, o: string) => void }) {
  const base = `rk-panel ${side === "left" ? "rk-panel-left" : "rk-panel-right"}`;
  const drag = useRef<number | null>(null);
  // Grabar una zona: la capa de dibujo responde con un aviso.
  useEffect(() => {
    if (tool.kind !== "rec") return;
    const off = listen<FaultRegionPicked>(FAULTREC_REGION_EVENT, (e) => {
      if (!e.payload.region) return;
      faultRecApi
        .start(e.payload.monitor, { region: e.payload.region, input: false, mic: tool.mic, systemAudio: tool.system })
        .then(
          () => {
            if (tool.camera) void faultRecApi.camera(true).catch(logQuietly("RocketBar.camera"));
            setTool(null);
          },
          (err) => setTool({ ...tool, msg: String(err) }),
        );
    });
    return () => void off.then((f) => f());
  }, [tool, setTool]);

  if (tool.kind === "rec") {
    const start = (opts: { window?: number }) =>
      faultRecApi.start(null, { window: opts.window ?? null, input: false, mic: tool.mic, systemAudio: tool.system }).then(
        () => {
          // Grabando una ventana la cámara no saldría en el vídeo: solo con la pantalla entera o una zona.
          if (tool.camera && opts.window == null) void faultRecApi.camera(true).catch(logQuietly("RocketBar.camera"));
          setTool(null);
        },
        (e) => setTool({ ...tool, msg: String(e) }),
      );
    return (
      <div className={base} role="menu">
        <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">Grabar pantalla</div>
        <button type="button" className={`${btn} block w-full`} onClick={() => void start({})}>
          Pantalla entera
        </button>
        <button
          type="button"
          className={`${btn} block w-full`}
          onClick={() =>
            faultRecApi.windows().then(
              (w) => setTool({ ...tool, windows: w.slice(0, 5) }),
              (e) => setTool({ ...tool, msg: String(e) }),
            )
          }
        >
          Una ventana…
        </button>
        {tool.windows?.map((w) => (
          <button key={w.id} type="button" className={`${btn} ml-3 block w-[calc(100%-12px)] truncate text-[#a8b4c0]`} onClick={() => void start({ window: w.id })}>
            {w.title}
          </button>
        ))}
        <button type="button" className={`${btn} block w-full`} onClick={() => void faultRecApi.pickRegion(null).catch((e) => setTool({ ...tool, msg: String(e) }))}>
          Una zona (dibujarla)
        </button>
        <label className="mt-1 flex items-center gap-2 px-2 text-[11px] text-[#a8b4c0]">
          <input type="checkbox" className="accent-[#5b8def]" checked={tool.mic} onChange={(e) => setTool({ ...tool, mic: e.target.checked })} /> Con mi voz
        </label>
        <label className="flex items-center gap-2 px-2 text-[11px] text-[#a8b4c0]">
          <input type="checkbox" className="accent-[#5b8def]" checked={tool.system} onChange={(e) => setTool({ ...tool, system: e.target.checked })} /> Con el sonido del equipo
        </label>
        <label className="flex items-center gap-2 px-2 text-[11px] text-[#a8b4c0]" title="Sale en el vídeo con la pantalla entera o una zona; no al grabar una ventana">
          <input type="checkbox" className="accent-[#5b8def]" checked={tool.camera} onChange={(e) => setTool({ ...tool, camera: e.target.checked })} /> Con mi cámara en una esquina
        </label>
        {tool.msg && <div className="mt-1 text-[11px] text-[#f2706b]">{tool.msg}</div>}
      </div>
    );
  }
  if (tool.kind === "recctl") {
    // Los controles de la grabación en marcha. «Parar» guarda y abre el panel «Grabación terminada».
    const act = (p: Promise<unknown>, ok?: string) => p.then(() => ok && setTool({ ...tool, msg: ok }), (e) => setTool({ ...tool, msg: String(e) }));
    return (
      <div className={base} role="dialog" aria-label="Controles de la grabación">
        <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">{tool.paused ? "En pausa" : "● Grabando"} {recSecs !== null ? clock(recSecs) : ""}</div>
        <div className="flex flex-wrap gap-1">
          <button type="button" className={btn} onClick={() => void (tool.paused ? faultRecApi.resume() : faultRecApi.pause()).then((s) => setTool({ ...tool, paused: !!s.paused, msg: undefined }), (e) => setTool({ ...tool, msg: String(e) }))}>
            {tool.paused ? "Seguir" : "Pausar"}
          </button>
          <button type="button" className={btn} onClick={() => void act(faultRecApi.mark().then((t) => setTool({ ...tool, msg: `Marca puesta en ${clock(t)}` })))}>
            Marcar
          </button>
          <button type="button" className={btn} onClick={() => void faultRecApi.draw(!tool.drawing).then((on) => setTool({ ...tool, drawing: on, msg: on ? "Dibuja con el ratón; Esc la quita" : undefined }), (e) => setTool({ ...tool, msg: String(e) }))}>
            {tool.drawing ? "Quitar dibujo" : "Dibujar"}
          </button>
          <button type="button" className={btn} onClick={() => void faultRecApi.camera(!tool.camera).then((on) => setTool({ ...tool, camera: on, msg: on ? "Cámara en la esquina: arrástrala para moverla" : undefined }), (e) => setTool({ ...tool, msg: String(e) }))}>
            {tool.camera ? "Quitar cámara" : "Cámara"}
          </button>
          <button type="button" className="rounded-md bg-[#f2706b] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422]" onClick={() => void act(faultRecApi.stop())}>
            Parar
          </button>
        </div>
        {tool.msg && <div className="mt-1 text-[11px] text-[#a8b4c0]">{tool.msg}</div>}
      </div>
    );
  }
  if (tool.kind === "update")
    return (
      <div className={base} role="dialog" aria-label="Actualizar AdminOps">
        <div className="text-[12px] font-semibold text-[#e8eef4]">AdminOps {tool.latest} está lista</div>
        {tool.state === "ask" ? (
          <>
            <div className="mt-0.5 text-[11px] text-[#a8b4c0]">Se descarga{tool.size ? ` (${bytes(tool.size)})` : ""}, se comprueba y se instala sola; AdminOps se vuelve a abrir al terminar.</div>
            <button
              type="button"
              className="mt-1.5 rounded-md bg-[#3ecf8e] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422]"
              onClick={() => {
                setTool({ ...tool, state: "busy" });
                appcareApi.installUpdate().then(
                  () => setTool({ ...tool, state: "done", msg: "Instalando: AdminOps se cerrará y volverá a abrirse sola." }),
                  (e) => setTool({ ...tool, state: "ask", msg: String(e) }),
                );
              }}
            >
              Actualizar ahora
            </button>
          </>
        ) : (
          <div className="mt-1 text-[11px] text-[#a8b4c0]">{tool.state === "busy" ? "Descargando y comprobando…" : tool.msg}</div>
        )}
        {tool.state === "ask" && tool.msg && <div className="mt-1 text-[11px] text-[#f2706b]">{tool.msg}</div>}
      </div>
    );
  if (tool.kind === "menu")
    return (
      <div className={base} role="menu">
        <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">{LABEL[tool.tool]}</div>
        {OPTIONS[tool.tool].map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="menuitem"
            className={`${btn} block w-full`}
            onClick={() => {
              setTool(null);
              onOption(tool.tool, id);
            }}
          >
            {label}
          </button>
        ))}
      </div>
    );
  if (tool.kind === "msg")
    return (
      <div className={base} role="status">
        <div className={`text-[11.5px] leading-[15px] ${tool.ok ? "text-[#3ecf8e]" : "text-[#f2706b]"}`}>{tool.text}</div>
        <button type="button" className={`${btn} mt-1`} onClick={() => setTool(null)}>
          Cerrar
        </button>
      </div>
    );
  if (tool.kind === "speed") return <SpeedPanel base={base} tool={tool} />;
  if (tool.kind === "card") {
    const copy = (label: string, text: string) =>
      navigator.clipboard.writeText(text).then(
        () => setTool({ ...tool, copied: label }),
        () => setTool({ ...tool, msg: "No se pudo copiar" }),
      );
    return (
      <div className={base} role="dialog" aria-label="Datos del equipo">
        <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">Datos del equipo</div>
        {tool.rows === null ? (
          <div className="text-[11px] text-[#a8b4c0]">Leyendo… (Windows tarda unos segundos)</div>
        ) : (
          <>
            {tool.rows.map(([k, v]) => (
              <div key={k} className="flex items-center justify-between gap-2 py-[2px] text-[11.5px]">
                <span className="min-w-0 truncate text-[#e8eef4]" title={k}>
                  {v}
                </span>
                <button type="button" className={`${btn} shrink-0 py-[1px] text-[10.5px]`} onClick={() => void copy(k, v)}>
                  {tool.copied === k ? "✓" : "Copiar"}
                </button>
              </div>
            ))}
            <button type="button" className="mt-1 rounded-md bg-[#5b8def] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422]" onClick={() => void copy("todo", tool.rows!.map(([k, v]) => `${k}: ${v}`).join("\n"))}>
              {tool.copied === "todo" ? "✓ Copiado todo" : "Copiar todo"}
            </button>
          </>
        )}
        {tool.msg && <div className="mt-1 text-[11px] text-[#f2706b]">{tool.msg}</div>}
      </div>
    );
  }
  if (tool.kind === "recdone") {
    const done = (p: Promise<unknown>, ok: string) => p.then(() => setTool({ ...tool, msg: ok, ok: true, busy: undefined }), (e) => setTool({ ...tool, msg: String(e), ok: false, busy: undefined }));
    // Lo pesado (recomprimir o sacar el GIF) tarda: se dice y no se puede pulsar dos veces.
    const slow = (what: "small" | "gif", p: () => Promise<unknown>, ok: string) => {
      if (tool.busy) return;
      setTool({ ...tool, busy: what, msg: what === "gif" ? "Preparando el GIF…" : "Preparando la copia pequeña…", ok: true });
      void done(p(), ok);
    };
    // El GIF, de los últimos 8 s: el fallo suele estar al final, justo antes de parar.
    const gifFrom = tool.secs !== null ? Math.max(0, tool.secs - 8) : 0;
    return (
      <div className={base} role="dialog" aria-label="Grabación terminada">
        <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">Grabación{tool.secs !== null ? ` de ${pillText(tool.secs).replace(/^.*?(\d+:\d+).*$/, "$1")}` : ""}</div>
        {tool.thumb ? <img src={tool.thumb} alt="" className="mb-1 h-[86px] w-full rounded-md object-cover" /> : <div className="mb-1 h-[86px] rounded-md bg-[#0e1319]" />}
        <button
          type="button"
          className="mb-1 w-full rounded-md bg-[#3ecf8e] px-2.5 py-[5px] text-[12px] font-semibold text-[#0c1422]"
          onClick={() => void barApi.saveDesktop(tool.id).then((name) => setTool({ ...tool, msg: `Guardado en el Escritorio: «${name}»`, ok: true }), (e) => setTool({ ...tool, msg: String(e), ok: false }))}
        >
          Guardar en el Escritorio
        </button>
        <div className="flex flex-wrap gap-1">
          <button type="button" className="rounded-md bg-[#5b8def] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422]" onClick={() => void done(faultRecApi.attachCase(tool.id).then((already) => already), "En el caso")}>
            Al caso
          </button>
          <button type="button" className={btn} onClick={() => void done(faultRecApi.copyFile(tool.id), "Copiada: pégala en Teams o en el correo")}>
            Copiar
          </button>
          <button type="button" className={btn} onClick={() => void done(faultRecApi.openFile(tool.id), "Abierta")}>
            Abrir
          </button>
          <button type="button" className={btn} disabled={!!tool.busy} onClick={() => slow("small", () => faultRecApi.exportSmall(tool.id), "Copia pequeña copiada: pégala en Teams o en el correo")}>
            {tool.busy === "small" ? "Preparando…" : "Para enviar"}
          </button>
          <button type="button" className={btn} disabled={!!tool.busy || tool.secs === null} title="Los últimos 8 segundos" onClick={() => slow("gif", () => faultRecApi.exportGif(tool.id, gifFrom * 1000, (gifFrom + 8) * 1000), "GIF copiado: pégalo en Teams o en el correo")}>
            {tool.busy === "gif" ? "Preparando…" : "GIF de 8 s"}
          </button>
          <button type="button" className={`${btn} ml-auto text-[#a8b4c0]`} onClick={() => setTool(null)}>
            Cerrar
          </button>
        </div>
        {tool.msg && <div className={`mt-1 text-[11px] ${tool.ok ? "text-[#3ecf8e]" : "text-[#f2706b]"}`}>{tool.msg}</div>}
      </div>
    );
  }
  if (tool.kind === "notes") {
    // Sin administrador: sus notas aquí al lado (las fijadas y las más nuevas primero), para leerlas y copiarlas.
    const list = (tool.notes ?? []).slice().sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updated - a.updated).slice(0, 6);
    const copy = (n: Note) =>
      navigator.clipboard.writeText([n.title, n.text].filter(Boolean).join("\n")).then(
        () => setTool({ ...tool, copied: n.id }),
        () => setTool({ ...tool, msg: "No se pudo copiar" }),
      );
    return (
      <div className={base} role="dialog" aria-label="Mis notas">
        <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">Mis notas</div>
        {tool.notes === null ? (
          <div className="text-[11px] text-[#a8b4c0]">Cargando…</div>
        ) : list.length === 0 ? (
          <div className="text-[11px] text-[#a8b4c0]">Aún no hay notas. Apunta una con «Nota nueva» o Ctrl+Alt+N.</div>
        ) : (
          <div className="max-h-[218px] overflow-y-auto">
            {list.map((n) => (
              <div key={n.id} className="flex items-start justify-between gap-2 border-b border-[#2e3b49] py-[3px] last:border-0">
                <div className="min-w-0">
                  <div className="truncate text-[11.5px] font-medium text-[#e8eef4]">{n.pinned ? "📌 " : ""}{n.title || n.text.split("\n")[0] || "Sin título"}</div>
                  {n.title && n.text && <div className="line-clamp-2 text-[10.5px] text-[#a8b4c0]">{n.text}</div>}
                </div>
                <button type="button" className={`${btn} shrink-0 py-[1px] text-[10.5px]`} onClick={() => void copy(n)}>
                  {tool.copied === n.id ? "✓" : "Copiar"}
                </button>
              </div>
            ))}
          </div>
        )}
        <button type="button" className="mt-1.5 rounded-md bg-[#5b8def] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422]" onClick={() => void noteApi.open().then(() => setTool(null), (e) => setTool({ ...tool, msg: String(e) }))}>
          Nota nueva
        </button>
        {tool.msg && <div className="mt-1 text-[11px] text-[#f2706b]">{tool.msg}</div>}
      </div>
    );
  }
  if (tool.kind === "glance") return <Glance base={base} />;
  if (tool.kind === "extra") return <ExtraPanel key={tool.tool} tool={tool.tool} base={base} />;
  if (tool.kind === "fix")
    return (
      <div className={base} role="menu">
        <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">Arreglos rápidos</div>
        {tool.fixes === null ? (
          <div className="text-[11px] text-[#a8b4c0]">Cargando…</div>
        ) : tool.fixes.length === 0 ? (
          <div className="text-[11px] text-[#a8b4c0]">Ninguno activado (Ajustes → General → Icono junto al reloj).</div>
        ) : (
          tool.fixes.map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={`${btn} block w-full`}
              onClick={() =>
                workApi.trayRunFix(id).then(
                  () => setTool({ ...tool, msg: `Hecho: ${label}` }),
                  (e) => setTool({ ...tool, msg: String(e) }),
                )
              }
            >
              {label}
            </button>
          ))
        )}
        {tool.msg && <div className={`mt-1 text-[11px] ${tool.msg.startsWith("Hecho") ? "text-[#3ecf8e]" : "text-[#f2706b]"}`}>{tool.msg}</div>}
      </div>
    );
  // Personalizar: marcar y arrastrar para ordenar.
  const move = (from: number, to: number) => {
    const list = [...tool.list];
    const [x] = list.splice(from, 1);
    list.splice(to, 0, x);
    setTool({ ...tool, list });
  };
  const save = () => {
    const next = tool.list.filter((x) => x.on).map((x) => x.id);
    workApi
      .settings()
      .then((s) => workApi.saveSettings({ ...s, rocketTools: next }))
      .then(() => {
        setTools(next);
        setTool(null);
      })
      .catch((e) => setTool({ ...tool, msg: `No se pudo guardar: ${String(e)}` }));
  };
  return (
    <div className={base} role="dialog" aria-label="Personalizar la barra">
      <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">Personalizar la barra</div>
      <div className="mb-1 text-[10.5px] text-[#76869a]">Marca las que quieres y arrástralas para ordenarlas.</div>
      {/* En una pantalla baja la lista hace scroll y «Guardar» queda siempre a la vista. */}
      <div className="overflow-y-auto" style={{ maxHeight: screenMaxH() - CUSTOM_HEAD }}>
      {tool.list.map((x, i) => (
        <div
          key={x.id}
          draggable
          onDragStart={() => (drag.current = i)}
          onDragOver={(e) => e.preventDefault()}
          onDrop={() => {
            if (drag.current !== null && drag.current !== i) move(drag.current, i);
            drag.current = null;
          }}
          className="flex cursor-grab items-center gap-2 rounded-md px-1.5 py-[3px] text-[11.5px] text-[#e8eef4] hover:bg-[#1b242e]"
        >
          <GripHorizontal size={12} className="text-[#76869a]" />
          <input type="checkbox" className="accent-[#5b8def]" checked={x.on} onChange={(e) => setTool({ ...tool, list: tool.list.map((y) => (y.id === x.id ? { ...y, on: e.target.checked } : y)) })} />
          <span className="flex h-[22px] w-[22px] scale-[0.8] items-center justify-center">{icon(x.id)}</span>
          {LABEL[x.id]}
        </div>
      ))}
      </div>
      <div className="mt-1.5 flex gap-1">
        <button type="button" className="rounded-md bg-[#5b8def] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422]" onClick={save}>
          Guardar
        </button>
        <button type="button" className={btn} onClick={() => setTool(null)}>
          Cancelar
        </button>
        <span className="ml-auto self-center text-[10.5px] text-[#76869a]">{tools.length} en la barra</span>
      </div>
      {tool.msg && <div className="mt-1 text-[11px] text-[#f2706b]">{tool.msg}</div>}
    </div>
  );
}

/** El vistazo: procesador (con su línea del último medio minuto), memoria, disco del sistema y red, cada segundo. */
function Glance({ base }: { base: string }) {
  const [cpu, setCpu] = useState<number[]>([]);
  const [m, setM] = useState<{ mem: number; disk: string; rx: number; tx: number } | null>(null);
  useEffect(() => {
    let alive = true;
    const tick = () =>
      api
        .liveMetrics()
        .then((x) => {
          if (!alive) return;
          setCpu((c) => [...c, x.cpuTotal].slice(-30));
          const sys = x.disks.find((d) => /^c:/i.test(d.mount)) ?? x.disks[0];
          setM({
            mem: x.memoryTotal ? Math.round((x.memoryUsed / x.memoryTotal) * 100) : 0,
            disk: sys ? `${Math.round(((sys.total - sys.available) / Math.max(1, sys.total)) * 100)} % lleno` : "—",
            rx: x.netRxPerSec,
            tx: x.netTxPerSec,
          });
        })
        .catch(() => {});
    void tick();
    const id = window.setInterval(() => void tick(), 1000);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, []);
  const last = cpu[cpu.length - 1] ?? 0;
  const pts = cpu.map((v, i) => `${(i / 29) * 180},${16 - (v / 100) * 15}`).join(" ");
  const row = (k: string, v: ReactNode, warn = false) => (
    <div className="flex justify-between text-[11px]">
      <span className="text-[#a8b4c0]">{k}</span>
      <b className={warn ? "text-[#f5b84b]" : "text-[#e8eef4]"}>{v}</b>
    </div>
  );
  return (
    <div className={base} role="status">
      {!m ? (
        <div className="text-[11px] text-[#a8b4c0]">Midiendo…</div>
      ) : (
        <>
          {row("Procesador", `${Math.round(last)} %`, last >= 85)}
          <svg width="180" height="18" className="my-[2px]">
            <polyline points={pts} fill="none" stroke="#5b8def" strokeWidth="1.5" />
          </svg>
          {row("Memoria", `${m.mem} %`, m.mem >= 80)}
          {row("Disco del sistema", m.disk)}
          {row("Red", `↓ ${bytes(m.rx)}/s · ↑ ${bytes(m.tx)}/s`)}
        </>
      )}
    </div>
  );
}

type Live = { phase: string; mbps: number; progress: number; latencyMs: number | null };
const PLAN_KEY = "adminops.speedPlan";

/** Velocímetro: arco con la aguja en `v` (de 0 a `max`). */
function SpeedGauge({ v, max, color }: { v: number; max: number; color: string }) {
  const a = Math.PI * (1 - Math.min(Math.max(v, 0), max) / max);
  const x = 60 + 46 * Math.cos(a);
  const y = 58 - 46 * Math.sin(a);
  return (
    <svg width="150" height="76" viewBox="0 0 120 64" className="mx-auto block">
      <path d="M14 58 A46 46 0 0 1 106 58" fill="none" stroke="#232e3a" strokeWidth="8" strokeLinecap="round" />
      {v > 0 && <path d={`M14 58 A46 46 0 0 1 ${x.toFixed(1)} ${y.toFixed(1)}`} fill="none" stroke={color} strokeWidth="8" strokeLinecap="round" />}
      <line x1="60" y1="58" x2={x.toFixed(1)} y2={y.toFixed(1)} stroke="#e8eef4" strokeWidth="2.5" strokeLinecap="round" style={{ transition: "all .25s" }} />
      <circle cx="60" cy="58" r="4" fill="#e8eef4" />
    </svg>
  );
}

/**
 * Velocidad de internet: aguja en vivo con la cifra grande, tres fases (ping, bajada, subida) que se completan,
 * el porcentaje de lo contratado (se pone una vez) y las mediciones de antes.
 */
function SpeedPanel({ base, tool }: { base: string; tool: Extract<ToolPanel, { kind: "speed" }> }) {
  const [live, setLive] = useState<Live | null>(null);
  const [plan, setPlan] = useState<number>(() => Number(localStorage.getItem(PLAN_KEY) ?? 0) || 0);
  const [editPlan, setEditPlan] = useState(false);
  useEffect(() => {
    if (!tool.running) return;
    const off = listen<Live>("speedtest-progress", (e) => setLive(e.payload));
    return () => void off.then((f) => f());
  }, [tool.running]);
  const res = tool.result;
  const order = ["latency", "download", "upload"];
  const cur = res ? 3 : live ? Math.max(0, order.indexOf(live.phase)) : 0;
  const mbps = res ? res.downloadMbps : live && live.phase !== "latency" ? live.mbps : 0;
  const max = plan > 0 ? plan * 1.2 : mbps > 500 ? 1000 : mbps > 150 ? 500 : 150;
  const pct = res && plan > 0 ? Math.round((res.downloadMbps / plan) * 100) : null;
  const color = pct === null ? "#5b8def" : pct >= 70 ? "#3ecf8e" : pct >= 40 ? "#f5b84b" : "#f2706b";
  // «Ver las mediciones anteriores»: solo la lista, sin la aguja a 0.
  const historyOnly = !tool.running && !res && !tool.msg;
  const phaseLabel = res ? "Bajada" : live?.phase === "upload" ? "Subida · midiendo" : live?.phase === "download" ? "Bajada · midiendo" : "Midiendo el ping…";
  return (
    <div className={base} role="status">
      <div className="text-[12px] font-semibold text-[#e8eef4]">{historyOnly ? "Mediciones anteriores" : "Velocidad de internet"}</div>
      {historyOnly ? (
        tool.history.length === 0 && <div className="mt-1 text-[11px] text-[#a8b4c0]">Aún no hay ninguna medición.</div>
      ) : tool.msg ? (
        <div className="mt-1 text-[11px] text-[#f2706b]">{tool.msg}</div>
      ) : (
        <>
          <SpeedGauge v={mbps} max={max} color={color} />
          <div className="-mt-1 text-center">
            <b className="text-[22px] text-[#e8eef4]">{Math.round(mbps)}</b> <span className="text-[11px] text-[#76869a]">Mb/s</span>
            <div className="text-[10.5px] text-[#76869a]">{phaseLabel}</div>
          </div>
          <div className="mt-1.5 flex gap-1">
            {[["Ping", res ? `${Math.round(res.latencyMs)} ms` : live?.latencyMs ? `${Math.round(live.latencyMs)} ms` : ""], ["Bajada", res ? `${res.downloadMbps.toFixed(0)}` : ""], ["Subida", res ? `${res.uploadMbps.toFixed(0)}` : ""]].map(([l, v], i) => (
              <div key={l} className="flex-1 text-center text-[10px]">
                <div className="h-1 overflow-hidden rounded bg-[#232e3a]">
                  <div className="h-full rounded" style={{ width: i < cur ? "100%" : i === cur && live ? `${Math.round(live.progress * 100)}%` : "0%", background: i < cur ? "#3ecf8e" : "#5b8def" }} />
                </div>
                <span className={i < cur ? "text-[#3ecf8e]" : i === cur ? "font-semibold text-[#e8eef4]" : "text-[#76869a]"}>{l}</span>
                {v && <div className="font-mono text-[#a8b4c0]">{v}</div>}
              </div>
            ))}
          </div>
          {res && (
            <div className="mt-1.5 text-center text-[11px]">
              {pct !== null ? <span style={{ color }}>Al {pct} % de {plan} Mb contratados{pct >= 70 ? " · va bien" : pct >= 40 ? " · algo lento" : " · muy por debajo"}</span> : null}{" "}
              {editPlan ? (
                <input
                  autoFocus
                  type="number"
                  min={1}
                  placeholder="Mb contratados"
                  className="w-24 rounded border border-[#2e3b49] bg-[#0e1319] px-1 text-[11px] text-[#e8eef4]"
                  onKeyDown={(e) => {
                    if (e.key !== "Enter") return;
                    const n = Math.max(0, Math.round(Number((e.target as HTMLInputElement).value) || 0));
                    setPlan(n);
                    localStorage.setItem(PLAN_KEY, String(n));
                    setEditPlan(false);
                  }}
                />
              ) : (
                <button type="button" className="text-[#5b8def] hover:underline" onClick={() => setEditPlan(true)}>
                  {plan > 0 ? "cambiar" : "¿Cuánto tienes contratado?"}
                </button>
              )}
            </div>
          )}
        </>
      )}
      {tool.history.length > 0 && !tool.running && (
        <div className="mt-1.5 border-t border-[#2e3b49] pt-1">
          {tool.history
            .slice(-3)
            .reverse()
            .map((h) => (
              <div key={h.timestamp} className="flex justify-between text-[10.5px] text-[#a8b4c0]">
                <span>{new Date(h.timestamp * (h.timestamp < 1e12 ? 1000 : 1)).toLocaleString("es", { weekday: "short", hour: "2-digit", minute: "2-digit" })}</span>
                <span className="font-mono">
                  {h.downloadMbps.toFixed(0)} / {h.uploadMbps.toFixed(0)} · {Math.round(h.latencyMs)} ms
                </span>
              </div>
            ))}
        </div>
      )}
    </div>
  );
}
