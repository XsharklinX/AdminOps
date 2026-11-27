import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import {
  rocketApi,
  ROCKET_AUTO_EVENT,
  ROCKET_DOCK_EVENT,
  ROCKET_HUNG_EVENT,
  ROCKET_LAUNCH_EVENT,
  ROCKET_MODE_EVENT,
  type AutoNote,
  type CallStep,
  type DeepItem,
  type Grow,
  type RocketBoost,
  type RocketHung,
  type RocketMode,
} from "../lib/api/rocket";
import { logQuietly } from "../lib/api/core";
import { bytes } from "../lib/format";
import { BUBBLE, memLevel, PANEL_H, PANEL_W, ROCKET_MIN_MS, ROCKET_RESULT_MS, rocketSummary, share, shouldNudge, type PanelKind } from "../lib/rocket";

/**
 * El cohete de «Optimizar ya» (rocket.rs, rocketplus.rs): una burbuja que se arrastra a cualquier sitio.
 * Un toque: despega dentro de su círculo mientras AdminOps aligera el equipo; tocar el resultado enseña
 * qué se llevó cada programa. Mantenerlo pulsado: «a fondo». Clic derecho: menú. Su anillo enseña la
 * memoria en uso y late cuando lleva un rato alta; se pone rojo si el programa de delante no responde.
 * Dos casas: la ventanita del escritorio, siempre encima (`RocketWindow`), y dentro de AdminOps
 * (`RocketInApp`). Colores fijos del tema oscuro: tiene que leerse igual sobre cualquier fondo.
 */

type Phase = "idle" | "working" | "done" | "error";
export type Panel =
  | { kind: "result"; boost: RocketBoost; line: string; sub: string }
  | { kind: "details"; boost: RocketBoost }
  | { kind: "error"; line: string }
  | { kind: "menu" }
  | { kind: "nudge"; pct: number }
  | { kind: "auto"; note: AutoNote }
  | { kind: "hung"; hung: RocketHung; busy?: boolean; msg?: string; ok?: boolean }
  | { kind: "deep"; state: "scan" | "pick" | "clean" | "done"; items: DeepItem[]; picked: string[]; freed?: number; error?: string }
  | { kind: "call"; steps: CallStep[] | null };
export type Action = "boost" | "deep" | "call" | "open" | "move" | "hide" | "custom";

/** Píxeles que hay que mover el ratón para que sea arrastrar y no tocar; y lo que hay que mantener para «a fondo». */
const DRAG_PX = 5;
const HOLD_MS = 650;
const TITLE = "Optimizar ya: memoria, temporales y DNS, sin cerrar nada. Mantenlo pulsado para «a fondo»; arrástralo para moverlo; clic derecho para más.";

const kindOf = (p: Panel | null): PanelKind | null => (p ? (p.kind as PanelKind) : null);

/** El dibujo: burbuja, anillo (memoria en reposo, progreso al despegar), estelas y el cohete con su llama. */
export function RocketBubble({ phase, mem, hung, pulse, holding }: { phase: Phase; mem: number | null; hung: boolean; pulse: boolean; holding: boolean }) {
  const flying = phase === "working";
  const level = mem === null ? null : memLevel(mem);
  // En reposo el anillo es la memoria en uso: un arco que empieza arriba.
  const idleRing = phase === "idle" && !hung && !holding && mem !== null ? { strokeDasharray: `${(mem / 100) * 194.8} 195`, transform: "rotate(-90deg)" } : undefined;
  return (
    <div className="rk-shell" data-phase={hung ? "hung" : phase} data-mem={level ?? undefined} data-holding={holding || undefined}>
      <svg className="rk-ring" viewBox="0 0 68 68" aria-hidden>
        <circle cx="34" cy="34" r="31" style={idleRing} />
      </svg>
      <div className={`rk-bubble ${pulse ? "rk-pulse" : ""}`}>
        {flying && (
          <>
            <i className="rk-streak" style={{ left: "30%", animationDelay: "0ms" }} />
            <i className="rk-streak" style={{ left: "52%", animationDelay: "260ms" }} />
            <i className="rk-streak" style={{ left: "70%", animationDelay: "520ms" }} />
          </>
        )}
        <svg className="rk-rocket" viewBox="0 0 32 34" aria-hidden>
          <g className="rk-flame">
            <path d="M12.9 24.6 Q16 35 19.1 24.6 Z" fill="#f5b84b" />
            <path d="M14.5 24.6 Q16 30.6 17.5 24.6 Z" fill="#fff4d8" />
          </g>
          <path d="M10.6 16.6 L6.4 21.4 L6.4 24.6 L10.6 22.4 Z" fill="#5b8def" />
          <path d="M21.4 16.6 L25.6 21.4 L25.6 24.6 L21.4 22.4 Z" fill="#5b8def" />
          <path d="M16 2.6 C20.3 6.6 21.6 11.6 21.6 16.8 L21.6 22.8 L10.4 22.8 L10.4 16.8 C10.4 11.6 11.7 6.6 16 2.6 Z" fill="#e8eef4" />
          <path d="M16 2.6 C18.2 4.6 19.6 7 20.5 9.6 L11.5 9.6 C12.4 7 13.8 4.6 16 2.6 Z" fill={hung ? "#f2706b" : "#5b8def"} />
          <circle cx="16" cy="14.4" r="2.7" fill="#0e1319" />
          <circle cx="16" cy="14.4" r="1.7" fill="#7fa6f3" />
          <rect x="12.6" y="22.8" width="6.8" height="1.9" rx="0.6" fill="#76869a" />
          <path d="M16 18.6 L16 24.6" stroke="#2e3b49" strokeWidth="1" />
        </svg>
        {phase === "done" && (
          <svg className="rk-badge" viewBox="0 0 16 16" aria-hidden>
            <circle cx="8" cy="8" r="7.5" fill="#3ecf8e" />
            <path d="M4.6 8.2 L7 10.5 L11.4 5.8" fill="none" stroke="#0c1422" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        )}
        {hung && phase === "idle" && <span className="rk-alert">!</span>}
      </div>
    </div>
  );
}

const btn = "rounded-md px-2 py-[3px] text-[11.5px] font-medium text-[#e8eef4] transition-colors hover:bg-[#2e3b49] disabled:opacity-50";
const primary = "rounded-md bg-[#5b8def] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422] transition-colors hover:bg-[#7fa6f3] disabled:opacity-50";

function Bars({ list }: { list: { name: string; bytes: number }[] }) {
  const max = Math.max(0, ...list.map((x) => x.bytes));
  return (
    <>
      {list.map((x) => (
        <div key={x.name} className="mt-[3px] flex items-center gap-2 text-[11px]">
          <span className="w-[118px] truncate text-[#a8b4c0]">{x.name}</span>
          <span className="h-[5px] flex-1 overflow-hidden rounded bg-[#0e1319]">
            <i className="block h-full rounded bg-[#5b8def]" style={{ width: `${share(x.bytes, max)}%` }} />
          </span>
          <span className="w-[58px] text-right font-mono text-[10.5px] text-[#a8b4c0]">{bytes(x.bytes)}</span>
        </div>
      ))}
    </>
  );
}

/** Lo que sale al lado de la burbuja. */
export type Ops = { restartHung: (h: RocketHung) => void; deepClean: (picked: string[]) => void };

export function SidePanel({ panel, side, inApp, setPanel, act, ops }: { panel: Panel; side: "left" | "right"; inApp: boolean; setPanel: (p: Panel | null) => void; act: (a: Action) => void; ops: Ops }) {
  const base = `rk-panel ${side === "left" ? "rk-panel-left" : "rk-panel-right"}`;
  switch (panel.kind) {
    case "menu":
      return (
        <div className={base} role="menu">
          <div className="mb-1 text-[10.5px] text-[#76869a]">Optimizar ya · arrástralo a donde quieras</div>
          {(
            [
              ["boost", "Optimizar ya", "Ctrl+Alt+O"],
              ["deep", "Optimizar a fondo", "o mantenerlo pulsado"],
              ["call", "Preparar una llamada", "Teams, Zoom…"],
              ...(inApp ? [] : [["custom", "Personalizar la barra", ""]]),
              ...(inApp ? [] : [["open", "Abrir AdminOps", ""]]),
              ["move", inApp ? "Llevarlo al escritorio" : "Solo dentro de AdminOps", ""],
              ["hide", "Ocultar el cohete", ""],
            ] as [Action, string, string][]
          ).map(([a, label, hint]) => (
            <button key={a} type="button" role="menuitem" className={`${btn} flex w-full items-center justify-between py-[5px] text-left`} onClick={() => act(a)}>
              {label}
              {hint && <span className="text-[10.5px] font-normal text-[#76869a]">{hint}</span>}
            </button>
          ))}
        </div>
      );
    case "error":
      return (
        <div className={base} role="status">
          <div className="text-[12px] font-medium text-[#f2706b]">No se pudo optimizar</div>
          <div className="line-clamp-2 text-[10.5px] leading-[14px] text-[#a8b4c0]">{panel.line}</div>
        </div>
      );
    case "nudge":
      return (
        <button type="button" className={`${base} text-left`} onClick={() => act("boost")}>
          <div className="text-[12px] font-semibold text-[#f5b84b]">Memoria al {panel.pct} %</div>
          <div className="text-[11px] text-[#a8b4c0]">Tócame para aligerar el equipo</div>
        </button>
      );
    case "auto":
      return (
        <div className={base} role="status">
          <div className="text-[12px] font-medium text-[#e8eef4]">Optimizado solo a las {panel.note.at}</div>
          <div className="text-[10.5px] text-[#3ecf8e]">
            Memoria del {panel.note.beforePct} % al {panel.note.afterPct} % · {panel.note.reason === "memoria" ? "estaba alta" : "con el equipo sin usar"}
          </div>
        </div>
      );
    case "hung":
      return (
        <div className={base} role="alert">
          <div className="text-[12px] font-semibold text-[#f2706b]">
            {panel.hung.name} no responde desde hace {panel.hung.secs} s
          </div>
          {panel.msg ? (
            <div className={`mt-1 text-[11px] ${panel.ok ? "text-[#3ecf8e]" : "text-[#f2706b]"}`}>{panel.msg}</div>
          ) : (
            <>
              <div className="mt-1 flex gap-1">
                <button type="button" className={btn} disabled={panel.busy} onClick={() => setPanel(null)}>
                  Esperar un poco más
                </button>
                <button
                  type="button"
                  className={primary}
                  disabled={panel.busy}
                  onClick={() => ops.restartHung(panel.hung)}
                >
                  {panel.busy ? "Cerrando…" : "Cerrar y volver a abrir"}
                </button>
              </div>
              <div className="mt-1 text-[10.5px] text-[#76869a]">Office, al volver a abrirse, ofrece recuperar el documento.</div>
            </>
          )}
        </div>
      );
    case "deep": {
      const total = panel.items.filter((i) => panel.picked.includes(i.id)).reduce((s, i) => s + i.bytes, 0);
      return (
        <div className={base} role="dialog" aria-label="Optimizar a fondo">
          <div className="text-[12px] font-semibold text-[#e8eef4]">{panel.state === "done" ? `A fondo: ${bytes(panel.freed ?? 0)} liberados` : "Optimizar a fondo"}</div>
          {panel.error || (panel.state === "pick" && panel.items.length === 0) ? (
            <>
              <div className={`mt-1 text-[11px] ${panel.error ? "text-[#f2706b]" : "text-[#3ecf8e]"}`}>{panel.error ?? "No hay nada que merezca la pena: el equipo está limpio."}</div>
              <button type="button" className={`${btn} mt-1`} onClick={() => setPanel(null)}>
                Cerrar
              </button>
            </>
          ) : panel.state === "scan" ? (
            <div className="mt-1 text-[11px] text-[#a8b4c0]">Mirando cuánto ocupa cada cosa…</div>
          ) : panel.state === "done" ? (
            <div className="mt-1 text-[11px] text-[#3ecf8e]">Solo lo seguro: lo que estaba en uso se dejó.</div>
          ) : (
            <>
              {panel.items.map((i) => (
                <label key={i.id} className="mt-[3px] flex cursor-pointer items-center gap-2 text-[11.5px] text-[#a8b4c0]">
                  <input
                    type="checkbox"
                    className="accent-[#5b8def]"
                    checked={panel.picked.includes(i.id)}
                    disabled={panel.state === "clean"}
                    onChange={(e) => setPanel({ ...panel, picked: e.target.checked ? [...panel.picked, i.id] : panel.picked.filter((x) => x !== i.id) })}
                  />
                  <span className="flex-1 truncate">{i.label}</span>
                  <span className="font-mono text-[10.5px]">{bytes(i.bytes)}</span>
                </label>
              ))}
              <div className="mt-1.5 flex gap-1">
                <button
                  type="button"
                  className={primary}
                  disabled={panel.state === "clean" || total === 0}
                  onClick={() => ops.deepClean(panel.picked)}
                >
                  {panel.state === "clean" ? "Liberando…" : `Liberar ${bytes(total)}`}
                </button>
                <button type="button" className={btn} disabled={panel.state === "clean"} onClick={() => setPanel(null)}>
                  Ahora no
                </button>
              </div>
            </>
          )}
        </div>
      );
    }
    case "call":
      return (
        <div className={base} role="status">
          <div className="text-[12px] font-semibold text-[#e8eef4]">{panel.steps ? (panel.steps.every((s) => s.ok) ? "Lista para la llamada" : "Casi lista para la llamada") : "Preparando la llamada…"}</div>
          {(panel.steps ?? []).map((s) => (
            <div key={s.text} className={`mt-[3px] text-[11px] leading-[14px] ${s.ok ? "text-[#3ecf8e]" : "text-[#f5b84b]"}`}>
              {s.ok ? "✓" : "!"} {s.text}
            </div>
          ))}
        </div>
      );
    case "details":
      return (
        <button type="button" className={`${base} text-left`} onClick={() => setPanel(null)} title="Clic para cerrar">
          <div className="text-[12px] font-semibold text-[#e8eef4]">Lo que se llevó</div>
          {panel.boost.apps.length > 0 ? <Bars list={panel.boost.apps} /> : <div className="text-[11px] text-[#76869a]">Ningún programa soltó memoria que se note.</div>}
          {panel.boost.folders.length > 0 && (
            <>
              <div className="mt-1.5 text-[10.5px] text-[#76869a]">Temporales</div>
              <Bars list={panel.boost.folders} />
            </>
          )}
          <div className="mt-1 text-[10.5px] text-[#76869a]">{panel.boost.dns ? "Caché DNS vaciada · " : ""}sin cerrar nada</div>
        </button>
      );
    case "result":
      return (
        <button type="button" className={`${base} text-left`} onClick={() => setPanel({ kind: "details", boost: panel.boost })} title="Clic para ver qué se llevó">
          <div className="line-clamp-2 text-[12px] font-medium leading-[15px] text-[#e8eef4]">{panel.line}</div>
          <div className="mt-0.5 truncate text-[10.5px] text-[#3ecf8e]">{panel.sub} · ver qué se llevó</div>
        </button>
      );
  }
}

/** Lo común a las dos casas: fase, panel, memoria, avisos y acciones. */
export function useRocket(opts: { inApp: boolean; onPanel: (k: PanelKind | null) => void }) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [panel, setPanelState] = useState<Panel | null>(null);
  const [mem, setMem] = useState<number | null>(null);
  const [hung, setHung] = useState<RocketHung | null>(null);
  const [high, setHigh] = useState(false);
  const panelRef = useRef<Panel | null>(null);
  const phaseRef = useRef<Phase>("idle");
  const timer = useRef(0);
  // La fase tiene su propio reloj: cerrar o cambiar de panel no la deja atascada en «hecho».
  const phaseTimer = useRef(0);
  const highSince = useRef<number | null>(null);
  const nudged = useRef(false);
  const dismissedPid = useRef<number | null>(null);
  const onPanelRef = useRef(opts.onPanel);
  useEffect(() => {
    onPanelRef.current = opts.onPanel;
  });

  const setPanel = useCallback((p: Panel | null, ms?: number) => {
    window.clearTimeout(timer.current);
    panelRef.current = p;
    setPanelState(p);
    onPanelRef.current(kindOf(p));
    if (ms) timer.current = window.setTimeout(() => setPanel(null), ms);
  }, []);
  const setPh = useCallback((p: Phase, backToIdleMs?: number) => {
    window.clearTimeout(phaseTimer.current);
    phaseRef.current = p;
    setPhase(p);
    if (backToIdleMs) phaseTimer.current = window.setTimeout(() => setPh("idle"), backToIdleMs);
  }, []);

  const launch = useCallback(async () => {
    if (phaseRef.current === "working") return;
    setPanel(null);
    setPh("working");
    const started = Date.now();
    let next: Panel;
    let ok = true;
    try {
      const b = await rocketApi.boost();
      const s = rocketSummary(b);
      next = { kind: "result", boost: b, line: s.line, sub: s.sub };
    } catch (e) {
      ok = false;
      next = { kind: "error", line: String(e) };
    }
    await new Promise((r) => window.setTimeout(r, Math.max(0, ROCKET_MIN_MS - (Date.now() - started))));
    setPh(ok ? "done" : "error", ROCKET_RESULT_MS);
    setPanel(next);
    timer.current = window.setTimeout(() => {
      // El desglose abierto se queda hasta que lo cierren.
      if (panelRef.current?.kind === "result" || panelRef.current?.kind === "error") setPanel(null);
    }, ROCKET_RESULT_MS);
    highSince.current = null;
    nudged.current = false;
    setHigh(false);
  }, [setPanel, setPh]);

  const deep = useCallback(() => {
    if (phaseRef.current === "working") return;
    setPanel({ kind: "deep", state: "scan", items: [], picked: [] });
    rocketApi.deepScan().then(
      (items) => {
        if (panelRef.current?.kind === "deep") setPanel({ kind: "deep", state: "pick", items, picked: items.map((i) => i.id) });
      },
      (e) => setPanel({ kind: "deep", state: "pick", items: [], picked: [], error: String(e) }),
    );
  }, [setPanel]);

  const call = useCallback(() => {
    if (phaseRef.current === "working") return;
    setPh("working");
    setPanel({ kind: "call", steps: null });
    rocketApi.prepareCall().then(
      (steps) => {
        setPh("done", 15_000);
        setPanel({ kind: "call", steps }, 15_000);
      },
      (e) => {
        setPh("error", 8000);
        setPanel({ kind: "error", line: String(e) }, 8000);
      },
    );
  }, [setPanel, setPh]);

  const deepClean = useCallback(
    (picked: string[]) => {
      const cur = panelRef.current;
      if (cur?.kind !== "deep") return;
      setPanel({ ...cur, state: "clean" });
      rocketApi.deepClean(picked).then(
        (freed) => {
          const now = panelRef.current;
          if (now?.kind === "deep") setPanel({ ...now, state: "done", freed }, 8000);
        },
        (e) => {
          const now = panelRef.current;
          if (now?.kind === "deep") setPanel({ ...now, state: "pick", error: String(e) });
        },
      );
    },
    [setPanel],
  );

  const restartHung = useCallback(
    (h: RocketHung) => {
      const cur = panelRef.current;
      if (cur?.kind !== "hung") return;
      setPanel({ ...cur, busy: true });
      rocketApi.restartHung(h.pid, h.name).then(
        (msg) => setPanel({ kind: "hung", hung: h, msg, ok: true }, 6000),
        (e) => setPanel({ kind: "hung", hung: h, msg: String(e), ok: false }, 8000),
      );
    },
    [setPanel],
  );

  const act = useCallback(
    (a: Action) => {
      setPanel(null);
      if (a === "boost") void launch();
      else if (a === "deep") deep();
      else if (a === "call") call();
      else if (a === "custom") window.dispatchEvent(new Event("rocket:custom"));
      else if (a === "open") void rocketApi.openMain().catch(logQuietly("Rocket.open"));
      else if (a === "move") void rocketApi.setMode(opts.inApp ? "desktop" : "app").catch(logQuietly("Rocket.mode"));
      else void rocketApi.setMode("off").catch(logQuietly("Rocket.mode"));
    },
    [launch, deep, call, setPanel, opts.inApp],
  );

  // La memoria cada 3 s: el anillo, el latido y el «tócame».
  useEffect(() => {
    let alive = true;
    const tick = () =>
      rocketApi
        .memory()
        .then((pct) => {
          if (!alive) return;
          setMem(pct);
          const now = Date.now();
          if (pct >= 90) highSince.current ??= now;
          else {
            highSince.current = null;
            nudged.current = false;
          }
          const isHigh = shouldNudge(highSince.current, now);
          setHigh(isHigh);
          if (isHigh && !nudged.current && !panelRef.current && phaseRef.current === "idle") {
            nudged.current = true;
            setPanel({ kind: "nudge", pct }, 8000);
          }
        })
        .catch(() => {});
    void tick();
    const id = window.setInterval(() => void tick(), 3000);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, [setPanel]);

  // Avisos del programa: despegar (Ctrl+K, Ctrl+Alt+O, menú del reloj), colgado y despegue automático.
  useEffect(() => {
    const offs = [
      listen(ROCKET_LAUNCH_EVENT, () => void launch()),
      listen<RocketHung | null>(ROCKET_HUNG_EVENT, (e) => {
        const h = e.payload;
        if (!h) {
          setHung(null);
          dismissedPid.current = null;
          // Ya responde: el aviso se va (si acaba de cerrarse y reabrirse, su mensaje se queda unos segundos).
          if (panelRef.current?.kind === "hung" && !panelRef.current.msg && !panelRef.current.busy) setPanel(null);
          return;
        }
        setHung(h);
        if (dismissedPid.current === h.pid) return;
        const cur = panelRef.current;
        if (cur?.kind === "hung" && (cur.busy || cur.msg)) return;
        if (!cur || cur.kind === "hung" || cur.kind === "nudge") setPanel({ kind: "hung", hung: h });
      }),
      listen<AutoNote>(ROCKET_AUTO_EVENT, (e) => {
        if (!panelRef.current) setPanel({ kind: "auto", note: e.payload }, 12_000);
      }),
    ];
    return () => offs.forEach((o) => void o.then((f) => f()));
  }, [launch, setPanel]);

  // «Esperar un poco más» con el aviso de colgado: no vuelve a salir para ese programa.
  const setPanelUser = useCallback(
    (p: Panel | null) => {
      if (!p && panelRef.current?.kind === "hung") dismissedPid.current = panelRef.current.hung.pid;
      setPanel(p);
    },
    [setPanel],
  );

  const ops: Ops = { restartHung, deepClean };
  return { phase, panel, mem, hung, high, launch, deep, act, ops, setPanel: setPanelUser, panelRef };
}

/**
 * Tocar, arrastrar o mantener: si el ratón se mueve unos píxeles con el botón pulsado es arrastrar;
 * si se mantiene quieto HOLD_MS, «a fondo»; si se suelta antes, tocar.
 */
export function usePress(handlers: { tap: () => void; hold: () => void; drag: (e: PointerEvent, start: { x: number; y: number }) => void; pressing: (on: boolean) => void }) {
  return (e: ReactPointerEvent) => {
    if (e.button !== 0) return;
    const start = { x: e.clientX, y: e.clientY };
    let done = false;
    handlers.pressing(true);
    const finish = () => {
      done = true;
      window.clearTimeout(t);
      handlers.pressing(false);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    const t = window.setTimeout(() => {
      if (done) return;
      finish();
      handlers.hold();
    }, HOLD_MS);
    const move = (ev: PointerEvent) => {
      if (done || Math.hypot(ev.clientX - start.x, ev.clientY - start.y) < DRAG_PX) return;
      finish();
      handlers.drag(ev, start);
    };
    const up = () => {
      if (done) return;
      finish();
      handlers.tap();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
}

export function bubbleProps(r: ReturnType<typeof useRocket>, press: (e: ReactPointerEvent) => void) {
  return {
    type: "button" as const,
    className: "rk-hit",
    title: r.phase === "idle" && !r.panel ? TITLE : undefined,
    "aria-label": "Optimizar ya",
    onPointerDown: press,
    // Intro o Espacio con el foco en el cohete. No con onClick: al crecer la ventana bajo el ratón, Windows
    // manda un clic «sin coordenadas» al soltar, y despegaba después de mantenerlo pulsado.
    onKeyDown: (e: React.KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        void r.launch();
      }
    },
    onContextMenu: (e: React.MouseEvent) => {
      e.preventDefault();
      if (r.phase !== "working") r.setPanel(r.panel?.kind === "menu" ? null : { kind: "menu" });
    },
  };
}

/** La ventanita del escritorio: siempre encima, sin marco y transparente alrededor de la burbuja. */
export function RocketWindow() {
  const zoom = (window as unknown as { __ADMINOPS_ROCKET_ZOOM__?: number }).__ADMINOPS_ROCKET_ZOOM__ ?? 1;
  const [grow, setGrow] = useState<Grow>({ left: false, up: false });
  const [docked, setDocked] = useState<"left" | "right" | null>(null);
  const [hover, setHover] = useState(false);
  const [holding, setHolding] = useState(false);
  const hoverRef = useRef(false);
  const r = useRocket({
    inApp: false,
    onPanel: (k) => {
      const w = k ? PANEL_W : BUBBLE;
      const h = k ? PANEL_H[k] : BUBBLE;
      rocketApi
        .size(w * zoom, h * zoom)
        .then((g) => {
          if (k) setGrow(g);
          // Plegada y sin el ratón encima: si estaba pegada al borde, vuelve a esconderse.
          else if (!hoverRef.current) window.setTimeout(() => !hoverRef.current && void rocketApi.peek(false).catch(() => {}), 400);
        })
        .catch(logQuietly("Rocket.size"));
    },
  });
  const { panelRef, setPanel } = r;

  useEffect(() => {
    // Sin fondo: solo se ve la burbuja (la ventana es transparente). Con el tamaño elegido en Ajustes.
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    (document.body.style as CSSStyleDeclaration & { zoom: string }).zoom = String(zoom);
    const offDock = listen<"left" | "right" | null>(ROCKET_DOCK_EVENT, (e) => setDocked(e.payload));
    rocketApi.dockState().then(setDocked, () => {});
    // El menú se pliega al hacer clic fuera (la ventanita pierde el foco) o con Escape.
    const blur = () => {
      if (panelRef.current?.kind === "menu") setPanel(null);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" && panelRef.current) setPanel(null);
    };
    window.addEventListener("blur", blur);
    window.addEventListener("keydown", key);
    return () => {
      void offDock.then((f) => f());
      window.removeEventListener("blur", blur);
      window.removeEventListener("keydown", key);
    };
  }, [zoom, panelRef, setPanel]);

  const press = usePress({
    tap: () => void r.launch(),
    hold: r.deep,
    pressing: setHolding,
    // Desplegada no se arrastra: al plegarse volvería a donde estaba.
    drag: () => {
      if (panelRef.current === null) void getCurrentWindow().startDragging().catch(logQuietly("Rocket.drag"));
    },
  });

  const enter = () => {
    hoverRef.current = true;
    setHover(true);
    if (docked) void rocketApi.peek(true).catch(() => {});
  };
  const leave = () => {
    hoverRef.current = false;
    setHover(false);
    window.setTimeout(() => {
      if (!hoverRef.current && !panelRef.current) void rocketApi.peek(false).catch(() => {});
    }, 1200);
  };
  const side = grow.left ? "left" : "right";
  const tucked = docked && !hover && !r.panel && r.phase === "idle" && !r.hung;
  return (
    <div className={`flex h-screen w-screen select-none ${grow.left && r.panel ? "flex-row-reverse" : ""} ${grow.up && r.panel ? "items-end" : "items-start"}`} onPointerEnter={enter} onPointerLeave={leave}>
      <button {...bubbleProps(r, press)} style={tucked ? { opacity: 0.55 } : undefined}>
        <RocketBubble phase={r.phase} mem={r.mem} hung={!!r.hung} pulse={r.high && r.phase === "idle"} holding={holding} />
      </button>
      {r.panel && <SidePanel panel={r.panel} side={side} inApp={false} setPanel={r.setPanel} act={r.act} ops={r.ops} />}
    </div>
  );
}

const POS_KEY = "adminops.rocket.pos";

/** La misma burbuja dentro de la ventana de AdminOps, para quien no la quiere en el escritorio. */
export function RocketInApp() {
  const r = useRocket({ inApp: true, onPanel: () => {} });
  const [holding, setHolding] = useState(false);
  // Distancia a la esquina de abajo a la derecha: así sigue en su sitio al cambiar el tamaño de la ventana.
  const [pos, setPos] = useState<{ right: number; bottom: number }>(() => {
    try {
      const p = JSON.parse(localStorage.getItem(POS_KEY) ?? "null") as { right: number; bottom: number } | null;
      if (p && Number.isFinite(p.right) && Number.isFinite(p.bottom)) return p;
    } catch {
      /* posición de siempre */
    }
    return { right: 22, bottom: 22 };
  });
  const clamp = (p: { right: number; bottom: number }) => ({
    right: Math.min(Math.max(0, p.right), Math.max(0, window.innerWidth - BUBBLE)),
    bottom: Math.min(Math.max(0, p.bottom), Math.max(0, window.innerHeight - BUBBLE)),
  });
  const view = clamp(pos);

  const press = usePress({
    tap: () => void r.launch(),
    hold: r.deep,
    pressing: setHolding,
    drag: (_ev, start) => {
      const from = view;
      const move = (ev: PointerEvent) => setPos(clamp({ right: from.right - (ev.clientX - start.x), bottom: from.bottom - (ev.clientY - start.y) }));
      const up = (ev: PointerEvent) => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        const end = clamp({ right: from.right - (ev.clientX - start.x), bottom: from.bottom - (ev.clientY - start.y) });
        setPos(end);
        try {
          localStorage.setItem(POS_KEY, JSON.stringify(end));
        } catch {
          /* sin memoria del navegador: vuelve a su sitio la próxima vez */
        }
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    },
  });

  // Con la burbuja en la mitad derecha, lo de al lado sale a su izquierda; en la de abajo, hacia arriba.
  const left = view.right + BUBBLE / 2 < window.innerWidth / 2;
  const up = view.bottom + BUBBLE / 2 < window.innerHeight / 2;
  const style: React.CSSProperties = {
    ...(left ? { right: view.right } : { left: window.innerWidth - view.right - BUBBLE }),
    ...(up ? { bottom: view.bottom } : { top: window.innerHeight - view.bottom - BUBBLE }),
  };
  return (
    <div
      className={`fixed z-30 flex select-none ${left ? "flex-row-reverse" : ""} ${up ? "items-end" : "items-start"}`}
      style={style}
      onMouseLeave={() => r.panel?.kind === "menu" && r.setPanel(null)}
    >
      <button {...bubbleProps(r, press)}>
        <RocketBubble phase={r.phase} mem={r.mem} hung={false} pulse={r.high && r.phase === "idle"} holding={holding} />
      </button>
      {r.panel && r.panel.kind !== "hung" && <SidePanel panel={r.panel} side={left ? "left" : "right"} inApp setPanel={r.setPanel} act={r.act} ops={r.ops} />}
    </div>
  );
}

/** El modo del cohete, al día con Ajustes y con su propio menú. */
export function useRocketMode(initial: RocketMode | null): RocketMode | null {
  const [mode, setMode] = useState<RocketMode | null>(initial);
  useEffect(() => setMode(initial), [initial]);
  useEffect(() => {
    const off = listen<RocketMode>(ROCKET_MODE_EVENT, (e) => setMode(e.payload));
    return () => {
      void off.then((f) => f());
    };
  }, []);
  return mode;
}
