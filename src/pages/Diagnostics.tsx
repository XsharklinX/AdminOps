import { celebrate, celebrationText } from "../components/Celebrate";
import {
  AlertOctagon,
  ArrowRight,
  ExternalLink,
  BatteryMedium,
  Check,
  CircleCheck,
  CircleHelp,
  Cpu,
  HardDrive,
  Info,
  Loader2,
  RefreshCw,
  ShieldCheck,
  TriangleAlert,
  Lightbulb,
  Zap,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { solutionForFinding } from "../lib/solutionsCatalog";
import { analyze, analyzeQuick, DIAG_UPDATED, lastDiagnostics, notifyDiagCount } from "../lib/diagRun";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import type { PageId } from "../components/Sidebar";
import { Bar, Button, Card, inputClass, Modal } from "../components/ui";
import {
  diagApi,
  logQuietly,
  tweaksApi,
  toolsApi,
  type Accepted,
  type Diagnostics as Diag,
  type DiagChanges,
  type Finding,
  type FindingAction,
  type Section,
  type Severity,
  type Tool,
} from "../lib/api";
import { bytes, dateTime as date } from "../lib/format";
import { DriverRestoreButton } from "../components/Maintenance";
import { ago } from "../lib/format";

/** Mientras se analiza, cada sección se pinta en cuanto el backend la termina. */
const EMPTY_SECTION = { data: null, error: null } as unknown as Section<never>;

function skeleton(): Diag {
  const s = EMPTY_SECTION;
  return {
    timestamp: 0,
    host: "",
    os: "",
    cpu: "",
    ramTotal: 0,
    admin: false,
    volumes: [],
    disks: s,
    stability: s,
    drivers: s,
    battery: s,
    system: s,
    startupEnabled: s,
    bloatInstalled: s,
    softwareUpdates: s,
    hardware: s,
    smart: s,
    memoryTest: s,
    temperatures: s,
    security: s,
    tweaksApplied: 0,
    findings: [],
    unchecked: [],
  } as unknown as Diag;
}

const SEV: Record<Severity, { icon: typeof Info; cls: string; label: string }> =
  {
    bad: { icon: AlertOctagon, cls: "text-bad", label: "Urgente" },
    warn: { icon: TriangleAlert, cls: "text-warn", label: "Conviene" },
    info: { icon: Info, cls: "text-dim", label: "Sugerencias" },
  };

/** Las partes del análisis, en el orden en que se cuentan. */
const PARTS: [string, string][] = [
  ["disks", "Discos"],
  ["stability", "Estabilidad"],
  ["drivers", "Drivers"],
  ["battery", "Batería"],
  ["system", "Sistema"],
  ["startupEnabled", "Inicio de Windows"],
  ["softwareUpdates", "Actualizaciones"],
  ["hardware", "Piezas"],
  ["smart", "SMART"],
  ["temperatures", "Temperaturas"],
  ["security", "Seguridad"],
];
/** Las que mira el análisis rápido. */
const QUICK_PARTS = new Set(["disks", "stability", "drivers", "battery", "system"]);
const secs = (ms: number) => `${(ms / 1000).toFixed(1)} s`;

/**
 * Qué se está leyendo y cuánto ha tardado cada parte. Mientras dura, una marca por
 * parte; al terminar, una línea con el total y lo que más tardó.
 */
function Progress({ timings, running, quick }: { timings: Record<string, number>; running: boolean; quick: boolean }) {
  const parts = PARTS.filter(([k]) => !quick || QUICK_PARTS.has(k));
  if (!running) {
    const done = parts.filter(([k]) => timings[k] !== undefined);
    if (!done.length) return null;
    const slow = done.reduce((a, b) => (timings[b[0]] > timings[a[0]] ? b : a));
    return (
      <p className="mb-3 text-[11px] text-mute">
        Análisis {quick ? "rápido " : ""}en {secs(timings[slow[0]])} · lo que más tardó: {slow[1]}
      </p>
    );
  }
  return (
    <div className="mb-3 flex flex-wrap gap-1.5" role="status" aria-label="Partes del análisis">
      {parts.map(([k, label]) => {
        const t = timings[k];
        return (
          <span key={k} className={`flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] ${t !== undefined ? "border-ok/30 text-dim" : "border-line text-mute"}`}>
            {t !== undefined ? <Check size={11} className="text-ok" /> : <Loader2 size={11} className="animate-spin" />}
            {label}
            {t !== undefined && <span className="font-mono text-mute">{secs(t)}</span>}
          </span>
        );
      })}
    </div>
  );
}

/** Qué cambió desde el análisis anterior: problemas nuevos y resueltos. */
function ChangesStrip({ c }: { c: DiagChanges }) {
  const [open, setOpen] = useState(false);
  const when = new Date(c.since * 1000).toLocaleString("es", {
    dateStyle: "medium",
    timeStyle: "short",
  });
  if (!c.new.length && !c.resolved.length)
    return (
      <p className="mb-3 text-xs text-mute">
        Sin cambios desde el análisis del {when}.
      </p>
    );
  return (
    <div className="mb-3 rounded-lg border border-line bg-void/40 px-3 py-2 text-xs">
      <button
        onClick={() => setOpen(!open)}
        className="flex w-full flex-wrap items-center gap-x-3 gap-y-1 text-left"
      >
        <span className="text-dim">Desde el análisis del {when}:</span>
        {c.new.length > 0 && (
          <span className="text-warn">
            {c.new.length}{" "}
            {c.new.length === 1 ? "problema nuevo" : "problemas nuevos"}
          </span>
        )}
        {c.resolved.length > 0 && (
          <span className="text-ok">
            {c.resolved.length}{" "}
            {c.resolved.length === 1 ? "resuelto" : "resueltos"}
          </span>
        )}
        {c.resolved.length > 0 && (
          <span className="ml-auto text-neon">
            {open ? "Ocultar" : "Ver resueltos"}
          </span>
        )}
      </button>
      {open && (
        <ul className="mt-2 space-y-0.5">
          {c.resolved.map((f) => (
            <li key={f.title} className="flex items-center gap-2 text-dim">
              <CircleCheck size={12} className="shrink-0 text-ok" />
              <span className="line-through decoration-mute/60">{f.title}</span>
              <span className="text-mute">· {f.area}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Qué tarjeta del detalle cubre cada área de los hallazgos. */
const CARD_AREAS: Record<string, string[]> = {
  disks: ["Discos", "Almacenamiento"],
  security: ["Seguridad", "Sistema"],
  stability: ["Estabilidad", "Rendimiento"],
  drivers: ["Drivers"],
  battery: ["Hardware", "Temperatura", "Memoria"],
};

const FOLD_KEY = "adminops.diag.onlyAttention";

function Unavailable<T>({
  section,
  children,
}: {
  section: Section<T>;
  children: (d: T) => ReactNode;
}) {
  if (section.data !== null && section.data !== undefined)
    return <>{children(section.data)}</>;
  // Sin datos y sin error: el backend todavía está con esa sección.
  if (!section.error)
    return (
      <p className="flex items-center gap-2 text-xs text-mute">
        <Loader2 size={12} className="animate-spin" /> Analizando…
      </p>
    );
  return <p className="text-xs break-words text-mute">{section.error}</p>;
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex justify-between gap-4 border-t border-line/70 py-1.5 text-sm first:border-t-0">
      <span className="text-dim">{label}</span>
      <span className="text-right text-ink">{children}</span>
    </div>
  );
}

const flag = (
  v: boolean | null,
  yes: string,
  no: string,
  goodWhenTrue = true,
) =>
  v === null ? (
    <span className="text-mute">Requiere admin</span>
  ) : (
    <span className={v === goodWhenTrue ? "text-ok" : "text-warn"}>
      {v ? yes : no}
    </span>
  );

function ToolButton({
  tool,
  label,
  onOpen,
}: {
  tool: Tool;
  label: string;
  onOpen: (t: Tool) => void;
}) {
  return (
    <button
      onClick={() => onOpen(tool)}
      className="flex items-center gap-1 text-[11px] text-mute hover:text-neon"
    >
      {label} <ExternalLink size={10} />
    </button>
  );
}

export function Diagnostics({
  focus,
  onNavigate,
}: {
  focus?: string | null;
  onNavigate: (page: PageId, focus?: string | null) => void;
}) {
  const [d, setD] = useState<Diag | null>(lastDiagnostics);
  const [running, setRunning] = useState(false);
  // Secciones que ya ha terminado el backend mientras el análisis sigue en curso.
  const [live, setLive] = useState<Diag | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hl, setHl] = useState<string | null>(null);
  const toast = useToast();
  // «Ya lo sé»: lo que el técnico da por sabido en este equipo.
  const [accepted, setAccepted] = useState<Accepted[]>([]);
  const [accepting, setAccepting] = useState<Finding | null>(null);
  const [reason, setReason] = useState("");
  useEffect(() => {
    diagApi.accepted().then(setAccepted).catch(logQuietly("Diagnostics"));
  }, []);
  const acceptedKeys = new Set(accepted.map((a) => a.key));
  // Cuándo llegó cada parte del análisis (ms desde que empezó), y de qué tipo fue.
  const [timings, setTimings] = useState<Record<string, number>>({});
  const [quickRun, setQuickRun] = useState(false);
  const startedAt = useRef(0);

  // Resaltar y llevar a una sección de esta misma página.
  const goTo = useCallback((section: string) => {
    const el = document.getElementById(`focus-${section}`);
    if (!el) return;
    el.scrollIntoView({ behavior: "smooth", block: "start" });
    setHl(section);
    window.setTimeout(() => setHl((h) => (h === section ? null : h)), 2500);
  }, []);

  useEffect(() => {
    if (focus && d) goTo(focus);
  }, [focus, d, goTo]);

  const openTool = (tool: Tool) =>
    diagApi.openTool(tool).catch((e) => toast("error", String(e)));

  // Lo que está bien no necesita ocupar sitio: se pliega y queda a un clic.
  const [onlyAttention, setOnlyAttention] = useState(() => {
    try {
      return localStorage.getItem(FOLD_KEY) !== "0";
    } catch {
      return true;
    }
  });
  const [unfolded, setUnfolded] = useState<Set<string>>(new Set());
  const setOnly = (v: boolean) => {
    setOnlyAttention(v);
    setUnfolded(new Set());
    try {
      localStorage.setItem(FOLD_KEY, v ? "1" : "0");
    } catch {
      /* sin almacenamiento */
    }
  };

  // `fixing`: id del arreglo en curso (o "all" para el lote).
  const [fixing, setFixing] = useState<string | null>(null);
  // Arreglos hechos desde aquí y problemas del análisis anterior: si se pasa de
  // «hay cosas» a «nada pendiente», se celebra (Ajustes → Apariencia lo apaga).
  const fixedHere = useRef(0);
  const lastProblems = useRef<number | null>(null);
  useEffect(() => {
    if (!d) return;
    const known = new Set(accepted.map((a) => a.key));
    const n = d.findings.filter((f) => f.severity !== "info" && !known.has(f.key)).length;
    if (lastProblems.current !== null && lastProblems.current > 0 && n === 0) celebrate(celebrationText(fixedHere.current));
    lastProblems.current = n;
  }, [d, accepted]);

  const applyFix = useCallback(
    async (id: string, label: string) => {
      setFixing(id);
      try {
        toast("ok", `${label}: ${await tweaksApi.fixFinding(id)}`);
        fixedHere.current++;
      } catch (e) {
        toast("error", `${label}: ${e}`);
      } finally {
        setFixing(null);
      }
    },
    [toast],
  );

  const act = (a: FindingAction) => {
    if (a.kind === "fix") void applyFix(a.id, a.label);
    else if (a.kind === "tool") void openTool(a.tool);
    else if (a.page === "diagnostics") {
      if (a.focus) goTo(a.focus);
    } else onNavigate(a.page as PageId, a.focus);
  };

  // Lo aceptado («Ya lo sé») no cuenta: ni para arreglar en lote ni para desplegar tarjetas.
  const active = (d?.findings ?? []).filter((f) => !acceptedKeys.has(f.key));
  // Todo lo que se puede arreglar sin riesgo, de una vez.
  const safeFixes = active.flatMap((f) =>
    f.actions.filter((a) => a.kind === "fix" && a.safe),
  );

  // Áreas con algo que revisar (las sugerencias no cuentan: no son problemas).
  const problemAreas = new Set(
    active.filter((f) => f.severity !== "info").map((f) => f.area),
  );
  const hasProblem = (card: string) =>
    (CARD_AREAS[card] ?? []).some((a) => problemAreas.has(a));
  /** Propiedades de plegado de una tarjeta del detalle (o nada, si hay que mirarla). */
  const fold = (card: string, note = "Sin problemas") =>
    !onlyAttention || hasProblem(card)
      ? undefined
      : {
          collapsed: !unfolded.has(card),
          note,
          onToggle: () =>
            setUnfolded((u) => {
              const n = new Set(u);
              if (n.has(card)) n.delete(card);
              else n.add(card);
              return n;
            }),
        };
  const fixAllSafe = async () => {
    setFixing("all");
    let done = 0;
    const failed: string[] = [];
    for (const a of safeFixes) {
      if (a.kind !== "fix") continue;
      try {
        await tweaksApi.fixFinding(a.id);
        done++;
        fixedHere.current++;
      } catch {
        failed.push(a.label);
      }
    }
    setFixing(null);
    toast(
      failed.length ? "info" : "ok",
      failed.length
        ? `${done} arreglados; fallaron: ${failed.join(", ")}`
        : `${done} arreglados. Vuelve a analizar para comprobarlo.`,
    );
  };

  const ring = (section: string) =>
    hl === section ? "border-neon! glow-neon" : "";

  const [backingUp, setBackingUp] = useState(false);
  const backupDrivers = async () => {
    setBackingUp(true);
    try {
      toast("ok", await toolsApi.backupDrivers());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBackingUp(false);
    }
  };

  const run = useCallback(async (force = false, quick = false) => {
    setRunning(true);
    setError(null);
    setLive(skeleton());
    setTimings({});
    setQuickRun(quick);
    startedAt.current = performance.now();
    try {
      setD(await (quick ? analyzeQuick() : analyze(force)));
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
      setLive(null);
    }
  }, []);

  useEffect(() => {
    if (!lastDiagnostics()) void run();
  }, [run]);

  // Las actualizaciones de programas pueden llegar después: el análisis se completa solo.
  useEffect(() => {
    const onUpdated = (e: Event) => {
      const next = (e as CustomEvent<Diag>).detail;
      setD((cur) => (cur && cur.timestamp === next.timestamp ? next : cur));
    };
    window.addEventListener(DIAG_UPDATED, onUpdated);
    return () => window.removeEventListener(DIAG_UPDATED, onUpdated);
  }, []);

  // El backend avisa de cada sección en cuanto la termina: se pintan una a una
  // en vez de esperar con la pantalla en blanco a que acaben todas.
  useEffect(() => {
    const un = listen<{ key: string; section: unknown }>(
      "diagnostics-progress",
      ({ payload }) => {
        if (payload.key !== "meta") setTimings((t) => (t[payload.key] !== undefined ? t : { ...t, [payload.key]: performance.now() - startedAt.current }));
        setLive((prev) => {
          if (!prev) return prev;
          if (payload.key === "meta") {
            const meta = payload.section as Partial<Diag>;
            return { ...prev, ...meta, findings: prev.findings };
          }
          return { ...prev, [payload.key]: payload.section } as Diag;
        });
      },
    );
    return () => {
      void un.then((f) => f());
    };
  }, []);

  if (error && !d)
    return (
      <div className="grid h-full place-items-center p-8 text-center">
        <p className="text-bad">{error}</p>
      </div>
    );

  // Sin resultado todavía: se va pintando lo que ya ha llegado.
  const shown = d ?? live;
  if (!shown)
    return (
      <div className="grid h-full place-items-center p-8 text-center">
        <div>
          <Loader2 size={28} className="mx-auto mb-3 animate-spin text-neon" />
          <p className="text-sm text-dim">
            Analizando discos, eventos, drivers y seguridad…
          </p>
        </div>
      </div>
    );

  const visible = shown.findings.filter((f) => !acceptedKeys.has(f.key));
  const main = visible.filter((f) => f.severity !== "info");
  const suggestions = visible.filter((f) => f.severity === "info");
  const known = shown.findings.filter((f) => acceptedKeys.has(f.key));
  const count = (s: Severity) => visible.filter((f) => f.severity === s).length;

  const accept = async () => {
    if (!accepting) return;
    try {
      setAccepted(await diagApi.accept(accepting.key, accepting.title, reason));
      setAccepting(null);
      setReason("");
      notifyDiagCount();
    } catch (e) {
      toast("error", String(e));
    }
  };
  const unaccept = (key: string) =>
    diagApi
      .unaccept(key)
      .then((list) => {
        setAccepted(list);
        notifyDiagCount();
      })
      .catch((e) => toast("error", String(e)));

  const row = (f: Finding, i: number) => {
    const S = SEV[f.severity];
    return (
      <li
        key={`${f.key}-${i}`}
        onClick={() => f.actions[0] && act(f.actions[0])}
        className={`group flex gap-3 rounded-lg px-2 py-2 hover:bg-panel-2 ${f.actions.length ? "cursor-pointer" : ""}`}
        title={f.actions[0]?.label}
      >
        <S.icon size={15} className={`mt-0.5 shrink-0 ${S.cls}`} />
        <div className="min-w-0 flex-1">
          <p className="text-sm text-ink group-hover:text-neon">
            {f.title}
            {shown.changes?.new.includes(f.title) && <span className="ml-2 rounded bg-warn/15 px-1.5 py-px align-middle text-[10px] font-medium text-warn">Nuevo</span>}
          </p>
          <p className="text-xs break-words text-dim">
            <span className="text-mute">{f.area}</span>
            {f.detail && <> · {f.detail}</>}
          </p>
          <div className="mt-1.5 flex flex-wrap gap-1.5">
            {(() => {
              const sol = solutionForFinding(f);
              return sol ? (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onNavigate("knowledge", `solution:${sol}`);
                  }}
                  title="Los pasos para arreglarlo, con botones para hacerlo desde aquí"
                  className="flex items-center gap-1 rounded-md border border-line-2 px-2 py-0.5 text-[11px] text-dim transition-colors hover:border-neon/50 hover:text-neon"
                >
                  <Lightbulb size={10} /> Cómo se arregla
                </button>
              ) : null;
            })()}
            {f.actions.map((a) => (
              <button
                key={a.label}
                onClick={(e) => {
                  e.stopPropagation();
                  act(a);
                }}
                disabled={a.kind === "fix" && fixing !== null}
                className={`flex items-center gap-1 rounded-md border px-2 py-0.5 text-[11px] transition-colors disabled:opacity-40 ${
                  a.kind === "fix" ? "border-neon/50 text-neon hover:bg-neon/10" : "border-line-2 text-dim hover:border-neon/50 hover:text-neon"
                }`}
              >
                {a.kind === "fix" && fixing === a.id ? <Loader2 size={10} className="animate-spin" /> : null}
                {a.label}
                {a.kind === "tool" ? <ExternalLink size={10} /> : a.kind === "page" ? <ArrowRight size={10} /> : null}
              </button>
            ))}
            {f.key && !shown.quick && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setReason("");
                  setAccepting(f);
                }}
                title="Ya lo sabes y no vas a arreglarlo ahora: pasa a «Aceptados» y deja de contar en este equipo"
                className="flex items-center gap-1 rounded-md px-2 py-0.5 text-[11px] text-mute opacity-0 transition-opacity group-hover:opacity-100 hover:text-ink focus-visible:opacity-100"
              >
                <Check size={10} /> Ya lo sé
              </button>
            )}
          </div>
        </div>
      </li>
    );
  };

  return (
    <div className="mx-auto max-w-(--page-max) p-6">
      <div className="mb-4 flex items-center gap-3">
        <p className="text-sm text-dim">
          {shown.timestamp
            ? `Análisis del ${new Date(shown.timestamp * 1000).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" })}`
            : "Analizando el equipo…"}
          {!shown.admin && (
            <span className="ml-2 text-warn">
              · sin administrador algunos datos no están disponibles
            </span>
          )}
        </p>
        <div className="ml-auto flex items-center gap-1.5">
          <button
            onClick={() => void run(false, true)}
            disabled={running}
            title="Una primera mirada en segundos: discos, estabilidad, drivers, batería y sistema. Sin actualizaciones, SMART, piezas, temperaturas ni seguridad. No se guarda."
            className="rounded-md px-2.5 py-1.5 text-xs text-dim transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-50"
          >
            Rápido
          </button>
          <button
            onClick={() => void run(false)}
            disabled={running}
            title="El análisis completo. Se guarda, y se compara con el anterior."
            className="flex items-center gap-1.5 rounded-md border border-neon/40 px-3 py-1.5 text-xs font-medium text-neon transition-colors hover:bg-neon/10 disabled:opacity-50"
          >
            <RefreshCw size={13} className={running ? "animate-spin" : ""} />
            {running ? "Analizando…" : "Analizar"}
          </button>
          <button
            onClick={() => void run(true)}
            disabled={running}
            title="Completo, y además vuelve a leer el hardware y las actualizaciones (winget) en vez de reutilizar lo de hace unos minutos"
            className="rounded-md px-2.5 py-1.5 text-xs text-dim transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-50"
          >
            A fondo
          </button>
        </div>
      </div>

      {/* Hallazgos, por lo que hay que hacer con ellos */}
      <section className="mb-4 rounded-xl border border-line bg-panel p-4">
        <div className="mb-3 flex flex-wrap gap-3">
          {(["bad", "warn", "info"] as Severity[]).map((s) => {
            const S = SEV[s];
            return (
              <div key={s} className="flex items-center gap-2 rounded-lg border border-line bg-void/40 px-3 py-2">
                <S.icon size={16} className={S.cls} />
                <span className={`font-mono text-lg ${S.cls}`}>{count(s)}</span>
                <span className="text-xs text-dim">{S.label}</span>
              </div>
            );
          })}
          {known.length > 0 && (
            <div className="flex items-center gap-2 rounded-lg border border-line bg-void/40 px-3 py-2" title="Hallazgos que has marcado «Ya lo sé» en este equipo: no cuentan">
              <Check size={16} className="text-mute" />
              <span className="font-mono text-lg text-mute">{known.length}</span>
              <span className="text-xs text-dim">Aceptados</span>
            </div>
          )}
        </div>
        <Progress timings={timings} running={running} quick={quickRun} />
        {shown.quick && !running && (
          <p className="mb-3 flex flex-wrap items-center gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn">
            Análisis rápido: una primera mirada. No se guarda ni cuenta para el Panel o el informe.
            <button onClick={() => void run(false)} className="underline">
              Hacer el completo
            </button>
          </p>
        )}
        {shown.changes && <ChangesStrip c={shown.changes} />}
        {safeFixes.length > 0 && (
          <button
            onClick={fixAllSafe}
            disabled={fixing !== null}
            className="mb-3 flex items-center gap-1.5 rounded-md border border-neon/50 px-3 py-1.5 text-xs font-medium text-neon transition-colors hover:bg-neon/10 disabled:opacity-50"
            title="Aplica solo lo que no cambia el comportamiento de Windows ni borra archivos tuyos"
          >
            {fixing === "all" ? <Loader2 size={13} className="animate-spin" /> : <Zap size={13} />}
            Arreglar todo lo seguro ({safeFixes.length})
          </button>
        )}
        {main.length > 0 ? (
          <ul className="space-y-1">{main.map(row)}</ul>
        ) : (
          !running && (
            <p className="flex items-center gap-2 py-2 text-sm text-ok">
              <CircleCheck size={16} /> Nada urgente ni que convenga arreglar{suggestions.length > 0 ? ": solo sugerencias." : "."}
            </p>
          )
        )}
        {suggestions.length > 0 && (
          <details className="mt-3 border-t border-line pt-3">
            <summary className="cursor-pointer text-xs font-medium text-dim select-none">
              Sugerencias de mejora · {suggestions.length} <span className="font-normal text-mute">— no son problemas: el equipo funciona bien sin ellas</span>
            </summary>
            <ul className="mt-2 space-y-1">{suggestions.map(row)}</ul>
          </details>
        )}
        {known.length > 0 && (
          <details className="mt-3 border-t border-line pt-3">
            <summary className="cursor-pointer text-xs font-medium text-dim select-none">
              Aceptados · {known.length} <span className="font-normal text-mute">— siguen ahí, pero ya los conoces y no cuentan</span>
            </summary>
            <ul className="mt-2 space-y-1.5">
              {known.map((f) => {
                const a = accepted.find((x) => x.key === f.key);
                return (
                  <li key={f.key} className="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 px-2 text-xs">
                    <span className="text-dim">{f.title}</span>
                    <span className="min-w-0 flex-1 text-mute">
                      {a?.reason ? `«${a.reason}»` : "sin motivo apuntado"}
                      {a ? ` · ${ago(a.at)}` : ""}
                    </span>
                    <button onClick={() => void unaccept(f.key)} className="shrink-0 text-[11px] text-dim hover:text-neon">
                      Volver a avisar
                    </button>
                  </li>
                );
              })}
            </ul>
          </details>
        )}
        {/* Lo que no se pudo mirar: «sin avisos» no es lo mismo que «no se pudo comprobar». */}
        {!running && (shown.unchecked?.length ?? 0) > 0 && (
          <div className="mt-3 border-t border-line pt-3">
            <p className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-dim">
              <CircleHelp size={13} /> No se pudo comprobar
            </p>
            <ul className="space-y-1">
              {shown.unchecked!.map((u) => (
                <li key={u.what} className="flex flex-wrap items-baseline gap-x-2 text-xs">
                  <span className="text-ink">{u.what}:</span>
                  <span className="min-w-0 flex-1 text-mute">{u.why}</span>
                  {u.action && (
                    <button onClick={() => act(u.action!)} className="flex shrink-0 items-center gap-1 text-[11px] text-dim hover:text-neon">
                      {u.action.label} <ArrowRight size={10} />
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </div>
        )}
      </section>

      {accepting && (
        <Modal
          title="Ya lo sé"
          onClose={() => setAccepting(null)}
          width="w-[480px]"
          footer={
            <>
              <Button kind="ghost" onClick={() => setAccepting(null)}>
                Cancelar
              </Button>
              <Button onClick={() => void accept()}>Aceptar el hallazgo</Button>
            </>
          }
        >
          <p className="text-sm text-ink">{accepting.title}</p>
          <p className="mt-1 text-xs text-mute">Pasa a «Aceptados» y deja de contar en este equipo: en el diagnóstico, en el Panel y en la barra de arriba. No se arregla ni se borra; en el informe sigue saliendo. Se deshace con «Volver a avisar».</p>
          <label className="mt-3 block">
            <span className="mb-1 block text-xs text-dim">Motivo (para acordarte tú, o quien venga después)</span>
            <input value={reason} onChange={(e) => setReason(e.target.value.slice(0, 200))} onKeyDown={(e) => e.key === "Enter" && void accept()} placeholder="Disco pendiente de cambio; avisado el cliente" className={inputClass} autoFocus />
          </label>
        </Modal>
      )}

      <div className="mb-3 flex items-center gap-3">
        <h2 className="text-sm font-medium text-ink">Detalle del equipo</h2>
        <label
          className="ml-auto flex items-center gap-1.5 text-xs text-dim"
          title="Lo que está bien se pliega; sigue a un clic"
        >
          <input
            type="checkbox"
            checked={onlyAttention}
            onChange={(e) => setOnly(e.target.checked)}
            className="accent-[var(--color-neon)]"
          />
          Plegar lo que está bien
        </label>
      </div>

      <div className="grid grid-cols-12 gap-4">
        {/* Discos */}
        <Card
          id="focus-disks"
          fold={fold("disks")}
          title="Discos"
          icon={<HardDrive size={14} />}
          className={`col-span-12 lg:col-span-7 ${ring("disks")}`}
          right={
            <button onClick={() => onNavigate("space", "health")} className="flex items-center gap-1 text-[11px] text-mute hover:text-neon">
              Salud y reparación <ArrowRight size={10} />
            </button>
          }
        >
          <Unavailable section={shown.disks}>
            {(disks) => (
              <ul className="divide-y divide-line/70">
                {disks.map((k) => {
                  const healthy = k.health === "Healthy";
                  return (
                    <li key={k.name + k.size} className="flex flex-wrap items-center gap-x-3 gap-y-0.5 py-1.5 text-sm">
                      <span className={`size-2 shrink-0 rounded-full ${healthy ? "bg-ok" : "bg-bad"}`} />
                      <span className="min-w-0 flex-1 truncate text-ink">
                        {k.name}
                        {k.isSystem && <span className="ml-2 rounded border border-line-2 px-1 text-[10px] text-mute">Windows</span>}
                      </span>
                      <span className="font-mono text-[11px] text-mute">
                        {k.mediaType} · {bytes(k.size)}
                        {k.temperature ? ` · ${k.temperature} °C` : ""}
                        {k.wear !== null ? ` · ${k.wear} % gastado` : ""}
                      </span>
                      <span className={`text-xs ${healthy ? "text-ok" : "text-bad"}`}>{healthy ? "Saludable" : k.health}</span>
                    </li>
                  );
                })}
              </ul>
            )}
          </Unavailable>
          <p className="mt-2 text-xs text-mute">Un resumen. El detalle (SMART, reparar, rescatar archivos) está en Discos → Salud y reparación, y qué ocupa el espacio en Discos → Espacio.</p>
        </Card>

        {/* Seguridad */}
        <Card
          id="focus-security"
          fold={fold("security")}
          title="Sistema y seguridad"
          icon={<ShieldCheck size={14} />}
          className={`col-span-12 lg:col-span-5 ${ring("security")}`}
          right={
            <button onClick={() => onNavigate("security")} className="flex items-center gap-1 text-[11px] text-mute hover:text-neon">
              Seguridad <ArrowRight size={10} />
            </button>
          }
        >
          {shown.security?.data &&
            (() => {
              const a = shown.security.data;
              const weak = a.checks.filter((c) => c.status === "warn" || c.status === "bad").length;
              return (
                <Row label="Nota de seguridad">
                  <span className={a.score >= 80 ? "text-ok" : a.score >= 60 ? "text-warn" : "text-bad"}>{a.score}/100</span>
                  {weak > 0 && <span className="text-mute"> · {weak} a mejorar</span>}
                </Row>
              );
            })()}
          <Unavailable section={shown.system}>
            {(s) => (
              <>
                {s.partial && <p className="mb-2 text-xs text-warn">Faltan datos (actualizaciones, antivirus, activación): {s.partial}</p>}
                <Row label="Antivirus">
                  {s.antivirus.join(", ") || (s.partial ? <span className="text-mute">Sin comprobar</span> : <span className="text-bad">No detectado</span>)}
                </Row>
                <Row label="Última actualización">{s.lastUpdate ? `${date(s.lastUpdate).split(",")[0]} · ${s.lastUpdateId ?? ""}` : "—"}</Row>
                <Row label="Reinicio pendiente">{flag(s.pendingReboot, "Sí", "No", false)}</Row>
                <Row label="Windows activado">{flag(s.activated, "Sí", "No")}</Row>
              </>
            )}
          </Unavailable>
          <p className="mt-2 text-xs text-mute">Un resumen. Cada punto de la nota, BitLocker y las cuentas están en Seguridad; arranque seguro y TPM, en la ficha del equipo (Hardware).</p>
        </Card>

        {/* Estabilidad */}
        <Card
          id="focus-stability"
          fold={fold("stability")}
          title="Estabilidad"
          icon={<Zap size={14} />}
          className={`col-span-12 lg:col-span-7 ${ring("stability")}`}
          right={
            <div className="flex gap-3">
              <ToolButton
                tool="reliability"
                label="Confiabilidad"
                onOpen={openTool}
              />
              <ToolButton
                tool="eventViewer"
                label="Eventos"
                onOpen={openTool}
              />
            </div>
          }
        >
          <Unavailable section={shown.stability}>
            {(s) => (
              <>
                <div className="mb-3 grid grid-cols-3 gap-3">
                  {[
                    {
                      n: s.bugchecks.length,
                      label: "Pantallazos azules",
                      bad: s.bugchecks.length > 0,
                    },
                    {
                      n: s.unexpectedShutdowns.length,
                      label: "Apagados inesperados",
                      bad: s.unexpectedShutdowns.length >= 2,
                    },
                    {
                      n: s.crashes.reduce((a, c) => a + c.count, 0),
                      label: "Cierres de apps",
                      bad: false,
                    },
                  ].map((x) => (
                    <div
                      key={x.label}
                      className="rounded-lg border border-line bg-void/40 px-3 py-2"
                    >
                      <div
                        className={`font-mono text-xl ${x.bad ? "text-bad" : "text-ink"}`}
                      >
                        {x.n}
                      </div>
                      <div className="text-[11px] text-dim">
                        {x.label} · {s.days} días
                      </div>
                    </div>
                  ))}
                </div>
                {s.crashes.length > 0 && (
                  <>
                    <p className="mt-2 mb-1 text-[11px] text-mute">
                      Apps que fallan
                    </p>
                    {s.crashes.map((c) => (
                      <div
                        key={c.app}
                        className="flex items-center gap-3 py-1 text-sm"
                      >
                        <span className="min-w-0 flex-1 truncate text-ink">
                          {c.app}
                        </span>
                        <span className="text-xs text-mute">
                          {date(c.last)}
                        </span>
                        <span className="w-8 text-right font-mono text-xs text-warn">
                          ×{c.count}
                        </span>
                      </div>
                    ))}
                  </>
                )}
                <button
                  onClick={() => onNavigate("machine", "boots")}
                  className="mt-3 flex items-center gap-1 text-xs text-dim hover:text-neon"
                >
                  Pantallazos con su driver probable, apagones y cuánto tarda en arrancar: Arranques y cuelgues <ArrowRight size={11} />
                </button>
              </>
            )}
          </Unavailable>
        </Card>

        <div className="col-span-12 flex flex-col gap-4 lg:col-span-5">
          {/* Drivers */}
          <Card
            id="focus-drivers"
            fold={fold("drivers")}
            title="Drivers"
            icon={<Cpu size={14} />}
            className={ring("drivers")}
            right={
              <div className="flex gap-3">
                <button
                  onClick={backupDrivers}
                  disabled={backingUp}
                  className="flex items-center gap-1 text-[11px] text-mute hover:text-neon disabled:opacity-50"
                  title="Exporta los drivers de terceros para reinstalarlos tras formatear"
                >
                  Copia de drivers <ArrowRight size={10} />
                </button>
                <DriverRestoreButton />
                <ToolButton
                  tool="deviceManager"
                  label="Dispositivos"
                  onOpen={openTool}
                />
              </div>
            }
          >
            <TaskStatus
              task="drivers-backup"
              active={backingUp}
              fallback="Copiando drivers…"
              cancellable={false}
              className="mb-2"
            />
            <Unavailable section={shown.drivers}>
              {(drivers) =>
                drivers.length === 0 ? (
                  <p className="flex items-center gap-2 text-sm text-ok">
                    <CircleCheck size={15} /> Todos los dispositivos funcionan.
                  </p>
                ) : (
                  <ul className="space-y-2">
                    {drivers.map((x) => (
                      <li key={x.deviceId} className="text-sm">
                        <div className="text-ink">{x.name}</div>
                        <div className="text-xs text-warn">
                          {x.problem}{" "}
                          <span className="font-mono text-mute">
                            (código {x.code})
                          </span>
                        </div>
                      </li>
                    ))}
                  </ul>
                )
              }
            </Unavailable>
          </Card>

          {/* Batería */}
          <Card
            id="focus-battery"
            fold={fold("battery")}
            title="Batería"
            icon={<BatteryMedium size={14} />}
            className={ring("battery")}
          >
            <Unavailable section={shown.battery}>
              {(b) => {
                if (!b)
                  return (
                    <p className="text-sm text-mute">
                      Equipo de sobremesa: sin batería.
                    </p>
                  );
                const health = b.design ? (b.full / b.design) * 100 : 0;
                return (
                  <>
                    <div className="mb-1 flex justify-between text-sm">
                      <span className="text-dim">
                        Capacidad respecto a la original
                      </span>
                      <span className="font-mono">{health.toFixed(0)}%</span>
                    </div>
                    <Bar
                      value={health}
                      color={
                        health < 60
                          ? "var(--color-bad)"
                          : health < 80
                            ? "var(--color-warn)"
                            : "var(--color-ok)"
                      }
                    />
                    <p className="mt-2 text-xs text-mute">
                      {b.full.toLocaleString("es")} de{" "}
                      {b.design.toLocaleString("es")} mWh
                      {b.cycles !== null && ` · ${b.cycles} ciclos`} ·{" "}
                      {b.manufacturer} {b.name}
                    </p>
                  </>
                );
              }}
            </Unavailable>
          </Card>
        </div>
      </div>
    </div>
  );
}
