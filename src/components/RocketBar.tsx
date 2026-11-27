import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Activity, ChevronLeft, ChevronRight, Gauge, GripHorizontal, IdCard, LifeBuoy, Mail, NotebookPen, Pencil, ScanLine, Wrench } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { api, diagApi, faultRecApi, FAULTREC_DONE_EVENT, FAULTREC_REGION_EVENT, FAULTREC_TICK_EVENT, noteApi, sheetApi, toolsApi, workApi, type FaultRecDone, type FaultRecWindow, type FaultRegionPicked, type SpeedResult } from "../lib/api";
import { logQuietly } from "../lib/api/core";
import { rocketApi, ROCKET_DOCK_EVENT } from "../lib/api/rocket";
import { barApi, BAR_EVENT, BAR_TOOLS, type BarTool } from "../lib/api/rocketbar";
import { pillText } from "../lib/faultrec";
import { bytes } from "../lib/format";
import { BUBBLE, PANEL_H, PANEL_W } from "../lib/rocket";
import { RecDot, TeamsIcon } from "./BrandIcons";
import { bubbleProps, RocketBubble, SidePanel, usePress, useRocket } from "./Rocket";
import logo from "../assets/logo.svg";

/**
 * La barra de herramientas del cohete (aspecto C, elegido el 26 nov): el cohete arriba como cabeza y las
 * herramientas debajo, vertical, pegada al borde derecho (o al izquierdo si se lleva allí). Escondida asoma
 * medio cohete con una flecha; al tocarla sale la barra, y se vuelve a esconder sola a los 5 s sin el ratón.
 * Ctrl+Alt+T la saca o la esconde. Clic derecho en el cohete → «Personalizar la barra».
 */

const STEP = 40; // alto de cada herramienta (botón de 36 + hueco)
const TOOL_PANEL_H = { msg: 68, rec: 186, fix: 0, custom: 420, speed: 168, card: 178, recdone: 196, glance: 150 } as const;
const HIDE_AFTER_MS = 5000;
/** La pestaña escondida (solo la flecha) y lo que se espera tras abrir antes de poder esconderse. */
const TAB_W = 22;
const TAB_H = 64;
const GRACE_MS = 1200;

const LABEL: Record<BarTool, string> = {
  rec: "Grabar pantalla",
  clip: "Captura de pantalla",
  note: "Notas",
  mon: "Mini monitor",
  fix: "Arreglos rápidos",
  teams: "Teams",
  mail: "Correo",
  app: "Abrir AdminOps",
  speed: "Velocidad de internet",
  card: "Datos del equipo",
  help: "Asistencia rápida",
};
const KEY: Partial<Record<BarTool, string>> = { rec: "Ctrl+Alt+G", note: "Ctrl+Alt+N" };

function icon(t: BarTool, recording = false): ReactNode {
  switch (t) {
    case "rec":
      return <RecDot size={20} recording={recording} />;
    case "clip":
      return <ScanLine size={17} />;
    case "note":
      return <NotebookPen size={17} />;
    case "mon":
      return <Activity size={17} />;
    case "fix":
      return <Wrench size={17} />;
    case "teams":
      return <TeamsIcon size={18} />;
    case "mail":
      return <Mail size={17} />;
    case "app":
      return <img src={logo} alt="" className="h-[18px] w-[18px]" />;
    case "speed":
      return <Gauge size={17} />;
    case "card":
      return <IdCard size={17} />;
    case "help":
      return <LifeBuoy size={17} />;
  }
}

type ToolPanel =
  | { kind: "msg"; text: string; ok: boolean }
  | { kind: "rec"; windows: FaultRecWindow[] | null; mic: boolean; msg?: string }
  | { kind: "fix"; fixes: [string, string][] | null; msg?: string }
  | { kind: "custom"; list: { id: BarTool; on: boolean }[] }
  | { kind: "speed"; running: boolean; result: SpeedResult | null; history: SpeedResult[]; msg?: string }
  | { kind: "card"; rows: [string, string][] | null; copied?: string; msg?: string }
  | { kind: "recdone"; id: string; thumb: string | null; secs: number | null; msg?: string; ok?: boolean }
  | { kind: "glance" }
  | null;

export function RocketBar() {
  const zoom = (window as unknown as { __ADMINOPS_ROCKET_ZOOM__?: number }).__ADMINOPS_ROCKET_ZOOM__ ?? 1;
  const [open, setOpen] = useState(false);
  const [right, setRight] = useState(true);
  const [tools, setTools] = useState<BarTool[]>([...BAR_TOOLS]);
  const [tool, setToolState] = useState<ToolPanel>(null);
  const [holding, setHolding] = useState(false);
  const [recSecs, setRecSecs] = useState<number | null>(null);
  const [fixDot, setFixDot] = useState(false);
  const [lastSpeed, setLastSpeed] = useState<number | null>(null);
  const glanceTimer = useRef(0);
  const openRef = useRef(false);
  const toolRef = useRef<ToolPanel>(null);
  const inside = useRef(false);
  const hideTimer = useRef(0);
  const kindRef = useRef<keyof typeof PANEL_H | null>(null);
  const openedAt = useRef(0);

  // Cohete + agarre + herramientas + lápiz; como mucho, lo que quepa en la pantalla (el resto, con scroll).
  const maxH = Math.max(240, Math.floor(window.screen.availHeight / zoom) - 12);
  const barH = Math.min(BUBBLE + 14 + tools.length * STEP + 40, maxH);
  const toolsH = barH - BUBBLE - 14 - 40;
  // Coloca la ventana: escondida (medio cohete), abierta (la barra) o con un panel al lado.
  const layout = useCallback(
    (wantOpen: boolean) => {
      const k = kindRef.current;
      const t = toolRef.current;
      const panelH = t ? (t.kind === "fix" ? 60 + (t.fixes?.length ?? 2) * 30 : TOOL_PANEL_H[t.kind]) : k ? PANEL_H[k] : 0;
      const isOpen = wantOpen || !!k || !!t;
      if (isOpen && !openRef.current) openedAt.current = Date.now();
      openRef.current = isOpen;
      setOpen(isOpen);
      const w = isOpen ? (k || t ? PANEL_W : BUBBLE) : TAB_W;
      const h = isOpen ? Math.max(barH, panelH) : TAB_H;
      rocketApi
        .bar(isOpen, w * zoom, h * zoom)
        .then(setRight)
        .catch(logQuietly("RocketBar.layout"));
    },
    [barH, zoom],
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
      const changed = toolRef.current?.kind !== t?.kind || (t?.kind === "fix" && toolRef.current?.kind === "fix" && toolRef.current.fixes?.length !== t.fixes?.length);
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
    workApi
      .settings()
      .then((s) => s.rocketTools && setTools(s.rocketTools.filter((t): t is BarTool => (BAR_TOOLS as readonly string[]).includes(t))))
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
    toolsApi
      .speedHistory()
      .then((h) => h.length && setLastSpeed(Math.round(h[h.length - 1].downloadMbps)))
      .catch(() => {});
    layout(false);
    const offs = [
      listen(BAR_EVENT, () => toggle()),
      listen<"left" | "right" | null>(ROCKET_DOCK_EVENT, (e) => e.payload && setRight(e.payload === "right")),
      listen<number>(FAULTREC_TICK_EVENT, (e) => setRecSecs(e.payload)),
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
    const custom = () => setTool({ kind: "custom", list: [...tools.map((id) => ({ id, on: true })), ...BAR_TOOLS.filter((t) => !tools.includes(t)).map((id) => ({ id, on: false }))] });
    window.addEventListener("rocket:custom", custom);
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (toolRef.current) setTool(null);
      else if (panelRef.current) setPanel(null);
      else layout(false);
    };
    window.addEventListener("keydown", key);
    return () => {
      offs.forEach((o) => void o.then((f) => f()));
      window.removeEventListener("rocket:custom", custom);
      window.removeEventListener("keydown", key);
    };
    // Una sola vez al montar: los manejadores leen por refs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

  const run = async (t: BarTool) => {
    try {
      if (t === "rec") {
        if (recSecs !== null) await faultRecApi.stop();
        else setTool({ kind: "rec", windows: null, mic: false });
      } else if (t === "clip") await barApi.screenClip();
      else if (t === "note") await noteApi.open();
      else if (t === "mon") await workApi.miniMonitor();
      else if (t === "fix") {
        setTool({ kind: "fix", fixes: null });
        const fixes = await barApi.fixes();
        if (toolRef.current?.kind === "fix") setTool({ kind: "fix", fixes });
      } else if (t === "teams" || t === "mail") await barApi.openComm(t);
      else if (t === "app") await rocketApi.openMain();
      else if (t === "help") {
        setTool({ kind: "msg", text: "Abriendo Asistencia rápida…", ok: true });
        setTool({ kind: "msg", text: await barApi.quickAssist(), ok: true });
      }
      else if (t === "speed") {
        const history = await toolsApi.speedHistory().catch(() => [] as SpeedResult[]);
        setTool({ kind: "speed", running: true, result: null, history });
        try {
          const result = await toolsApi.speedtest();
          setLastSpeed(Math.round(result.downloadMbps));
          if (toolRef.current?.kind === "speed") setTool({ kind: "speed", running: false, result, history });
        } catch (e) {
          if (toolRef.current?.kind === "speed") setTool({ kind: "speed", running: false, result: null, history, msg: String(e) });
        }
      } else if (t === "card") {
        setTool({ kind: "card", rows: null });
        const s = await sheetApi.get();
        const rows: [string, string][] = [
          ["Equipo", s.host],
          ["IP", s.ip],
          ["Usuario", s.domain ? `${s.domain}\\${s.user}` : s.user],
          ["Número de serie", s.serial],
          ["Modelo", [s.manufacturer, s.model].filter(Boolean).join(" ")],
        ];
        if (toolRef.current?.kind === "card") setTool({ kind: "card", rows: rows.filter(([, v]) => v) });
      }
    } catch (e) {
      setTool({ kind: "msg", text: String(e), ok: false });
    }
  };

  const tucked = !open;
  const memBadge = r.mem !== null && r.mem >= 80 ? `${r.mem}%` : null;
  const side = right ? "left" : "right";
  return (
    <div
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
            <div className="rkb-tools" style={{ maxHeight: toolsH }}>
            {tools.map((t) => (
              <button
                key={t}
                type="button"
                className={`rkb-btn ${t === "rec" && recSecs !== null ? "rkb-on" : ""}`}
                aria-label={LABEL[t]}
                title={`${t === "rec" && recSecs !== null ? "Parar la grabación" : LABEL[t]}${KEY[t] ? ` (${KEY[t]})` : ""}`}
                onClick={() => void run(t)}
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
          </>
        )}
      </div>
      )}
      {r.panel && <SidePanel panel={r.panel} side={side} inApp={false} setPanel={r.setPanel} act={r.act} ops={r.ops} />}
      {tool && <ToolSide tool={tool} side={side} setTool={setTool} tools={tools} setTools={setTools} />}
    </div>
  );
}

const btn = "rounded-md px-2 py-[4px] text-left text-[11.5px] font-medium text-[#e8eef4] transition-colors hover:bg-[#2e3b49] disabled:opacity-50";

function ToolSide({ tool, side, setTool, tools, setTools }: { tool: Exclude<ToolPanel, null>; side: "left" | "right"; setTool: (t: ToolPanel) => void; tools: BarTool[]; setTools: (t: BarTool[]) => void }) {
  const base = `rk-panel ${side === "left" ? "rk-panel-left" : "rk-panel-right"}`;
  const drag = useRef<number | null>(null);
  // Grabar una zona: la capa de dibujo responde con un aviso.
  useEffect(() => {
    if (tool.kind !== "rec") return;
    const off = listen<FaultRegionPicked>(FAULTREC_REGION_EVENT, (e) => {
      if (!e.payload.region) return;
      faultRecApi
        .start(e.payload.monitor, { region: e.payload.region, input: false, mic: tool.mic })
        .then(() => setTool(null), (err) => setTool({ ...tool, msg: String(err) }));
    });
    return () => void off.then((f) => f());
  }, [tool, setTool]);

  if (tool.kind === "rec") {
    const start = (opts: { window?: number }) =>
      faultRecApi.start(null, { window: opts.window ?? null, input: false, mic: tool.mic }).then(
        () => setTool(null),
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
        {tool.msg && <div className="mt-1 text-[11px] text-[#f2706b]">{tool.msg}</div>}
      </div>
    );
  }
  if (tool.kind === "msg")
    return (
      <div className={base} role="status">
        <div className={`text-[11.5px] leading-[15px] ${tool.ok ? "text-[#3ecf8e]" : "text-[#f2706b]"}`}>{tool.text}</div>
        <button type="button" className={`${btn} mt-1`} onClick={() => setTool(null)}>
          Cerrar
        </button>
      </div>
    );
  if (tool.kind === "speed") {
    const res = tool.result;
    return (
      <div className={base} role="status">
        <div className="text-[12px] font-semibold text-[#e8eef4]">Velocidad de internet</div>
        {tool.running ? (
          <div className="mt-1 text-[11px] text-[#a8b4c0]">Midiendo bajada, subida y ping… (unos 15 s)</div>
        ) : res ? (
          <div className="mt-1 text-[12px] text-[#e8eef4]">
            <b>{res.downloadMbps.toFixed(0)} Mb/s</b> bajada · <b>{res.uploadMbps.toFixed(0)}</b> subida · ping <b>{Math.round(res.latencyMs)} ms</b>
          </div>
        ) : (
          <div className="mt-1 text-[11px] text-[#f2706b]">{tool.msg ?? "No se pudo medir."}</div>
        )}
        {tool.history.length > 0 && <div className="mt-2 text-[10.5px] text-[#76869a]">Antes</div>}
        {tool.history
          .slice(-4)
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
    );
  }
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
    const done = (p: Promise<unknown>, ok: string) => p.then(() => setTool({ ...tool, msg: ok, ok: true }), (e) => setTool({ ...tool, msg: String(e), ok: false }));
    return (
      <div className={base} role="dialog" aria-label="Grabación terminada">
        <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">Grabación{tool.secs !== null ? ` de ${pillText(tool.secs).replace(/^.*?(\d+:\d+).*$/, "$1")}` : ""}</div>
        {tool.thumb ? <img src={tool.thumb} alt="" className="mb-1 h-[86px] w-full rounded-md object-cover" /> : <div className="mb-1 h-[86px] rounded-md bg-[#0e1319]" />}
        <div className="flex gap-1">
          <button type="button" className="rounded-md bg-[#5b8def] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422]" onClick={() => void done(faultRecApi.attachCase(tool.id).then((already) => already), "En el caso")}>
            Al caso
          </button>
          <button type="button" className={btn} onClick={() => void done(faultRecApi.copyFile(tool.id), "Copiada: pégala en Teams o en el correo")}>
            Copiar
          </button>
          <button type="button" className={btn} onClick={() => void done(faultRecApi.openFile(tool.id), "Abierta")}>
            Abrir
          </button>
          <button type="button" className={`${btn} ml-auto text-[#a8b4c0]`} onClick={() => setTool(null)}>
            Cerrar
          </button>
        </div>
        {tool.msg && <div className={`mt-1 text-[11px] ${tool.ok ? "text-[#3ecf8e]" : "text-[#f2706b]"}`}>{tool.msg}</div>}
      </div>
    );
  }
  if (tool.kind === "glance") return <Glance base={base} />;
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
      .catch(logQuietly("RocketBar.custom"));
  };
  return (
    <div className={base} role="dialog" aria-label="Personalizar la barra">
      <div className="mb-1 text-[12px] font-semibold text-[#e8eef4]">Personalizar la barra</div>
      <div className="mb-1 text-[10.5px] text-[#76869a]">Marca las que quieres y arrástralas para ordenarlas.</div>
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
          <span className="flex h-[18px] w-[18px] items-center justify-center">{icon(x.id)}</span>
          {LABEL[x.id]}
        </div>
      ))}
      <div className="mt-1.5 flex gap-1">
        <button type="button" className="rounded-md bg-[#5b8def] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422]" onClick={save}>
          Guardar
        </button>
        <button type="button" className={btn} onClick={() => setTool(null)}>
          Cancelar
        </button>
        <span className="ml-auto self-center text-[10.5px] text-[#76869a]">{tools.length} en la barra</span>
      </div>
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
