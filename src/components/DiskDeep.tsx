// Discos a fondo: lo que antes se hacía con CrystalDiskInfo y con programas de
// escaneo de superficie. La tabla SMART completa y explicada, la nota de salud
// con la vida que queda, las autopruebas del disco, la curva de velocidad y el
// mapa de superficie. Todo en solo lectura.
import { Activity, Bell, ChevronDown, ChevronRight, Gauge, Loader2, Play, ScanSearch, Square, TestTube } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "./feedback";
import { TaskStatus } from "./TaskStatus";
import { Button, Card, Toggle } from "./ui";
import { appApi, disksApi, type DiskReport, type HealthScore, type ScanLive, type ScanResult, type SelfTest, type SmartFull, type SmartRow, type WatchConfig } from "../lib/api";
import { bytes } from "../lib/format";

const TEXT = { ok: "text-ok", warn: "text-warn", bad: "text-bad" } as const;
const PILL = {
  ok: "border-ok/40 bg-ok/10 text-ok",
  warn: "border-warn/40 bg-warn/10 text-warn",
  bad: "border-bad/40 bg-bad/10 text-bad",
} as const;
const STATUS_LABEL = { ok: "Bien", warn: "Vigilar", bad: "Mal" } as const;

const scoreLevel = (n: number): "ok" | "warn" | "bad" => (n >= 70 ? "ok" : n >= 50 ? "warn" : "bad");

// ---------- Nota de salud ----------

function ScoreRing({ value, size = 64 }: { value: number; size?: number }) {
  const stroke = 6;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const color = value >= 70 ? "var(--color-ok)" : value >= 50 ? "var(--color-warn)" : "var(--color-bad)";
  return (
    <div className="relative shrink-0" style={{ width: size, height: size }} role="img" aria-label={`Nota de salud ${value} de 100`}>
      <svg width={size} height={size} className="-rotate-90" aria-hidden>
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--color-line)" strokeWidth={stroke} />
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={color} strokeWidth={stroke} strokeLinecap="round" strokeDasharray={c} strokeDashoffset={c - (value / 100) * c} />
      </svg>
      <span className="absolute inset-0 flex items-center justify-center text-lg font-semibold tabular text-ink">{value}</span>
    </div>
  );
}

/** La nota del disco. En modo usuario, solo la nota y una frase. */
export function ScoreCard({ score, technician }: { score: HealthScore; technician: boolean }) {
  const lvl = scoreLevel(score.total);
  return (
    <div className="mx-4 mb-3 rounded-lg border border-line bg-panel-2 p-3">
      <div className="flex items-center gap-3">
        <ScoreRing value={score.total} />
        <div className="min-w-0 flex-1">
          <div className={`text-sm font-medium ${TEXT[lvl]}`}>{score.label}</div>
          <p className="text-xs text-dim">{score.sentence}</p>
          {score.lifeYears !== null && technician && <p className="mt-0.5 text-[11px] text-mute">{score.lifeText}</p>}
        </div>
      </div>
      {technician && (
        <ul className="mt-3 grid gap-x-6 gap-y-1.5 sm:grid-cols-2">
          {score.parts.map((p) => (
            <li key={p.name} className="min-w-0">
              <div className="flex items-baseline justify-between gap-2 text-[11px]">
                <span className="text-dim">{p.name}</span>
                <span className="tabular font-mono text-mute">{p.score}</span>
              </div>
              <div className="mt-0.5 h-1 overflow-hidden rounded-full bg-line">
                <div className="h-full rounded-full" style={{ width: `${p.score}%`, background: p.score >= 70 ? "var(--color-ok)" : p.score >= 50 ? "var(--color-warn)" : "var(--color-bad)" }} />
              </div>
              <div className="mt-0.5 truncate text-[10px] text-mute" title={p.text}>
                {p.text}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// ---------- Tabla SMART ----------

function SmartTab({ d }: { d: DiskReport }) {
  const [data, setData] = useState<SmartFull | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [onlyIssues, setOnlyIssues] = useState(false);

  const load = useCallback(() => {
    setData(null);
    setError(null);
    disksApi
      .smartFull(d.number, d.bus, d.model)
      .then(setData)
      .catch((e) => setError(String(e)));
  }, [d.number, d.bus, d.model]);
  useEffect(load, [load]);

  if (error) return <p className="px-4 py-3 text-xs text-bad">{error}</p>;
  if (!data)
    return (
      <p className="flex items-center gap-2 px-4 py-3 text-xs text-mute">
        <Loader2 size={12} className="animate-spin" /> Leyendo la tabla del disco…
      </p>
    );
  const rows = onlyIssues ? data.rows.filter((r) => r.status !== "ok") : data.rows;
  const issues = data.rows.filter((r) => r.status !== "ok").length;
  const hasLevels = data.kind === "ata";

  return (
    <div className="px-4 pb-3">
      <div className="mb-2 flex flex-wrap items-center gap-3 text-[11px] text-mute">
        <span>
          {data.rows.length} datos · {issues === 0 ? "ninguno preocupa" : `${issues} para vigilar`}
        </span>
        <label className="flex items-center gap-1.5">
          <input type="checkbox" checked={onlyIssues} onChange={(e) => setOnlyIssues(e.target.checked)} /> Solo los que preocupan
        </label>
        <button type="button" className="ml-auto text-neon hover:underline" onClick={load}>
          Volver a leer
        </button>
      </div>
      {data.note && <p className="mb-2 rounded-md border border-line bg-panel-2 px-2.5 py-1.5 text-[11px] text-dim">{data.note}</p>}
      {data.rows.length === 0 ? (
        <p className="text-xs text-mute">Este disco no ofrece datos de salud por este camino.</p>
      ) : (
        <div className="overflow-x-auto rounded-lg border border-line">
          <table className="w-full min-w-[560px] text-left text-[11px]">
            <thead className="bg-panel-2 text-mute">
              <tr>
                <th className="w-6 px-2 py-1.5" />
                <th className="px-2 py-1.5 font-medium">ID</th>
                <th className="px-2 py-1.5 font-medium">Dato</th>
                {hasLevels && (
                  <>
                    <th className="px-2 py-1.5 text-right font-medium">Valor</th>
                    <th className="px-2 py-1.5 text-right font-medium">Peor</th>
                    <th className="px-2 py-1.5 text-right font-medium">Umbral</th>
                  </>
                )}
                <th className="px-2 py-1.5 text-right font-medium">Bruto</th>
                <th className="px-2 py-1.5 font-medium">Estado</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <SmartLine key={r.id} r={r} levels={hasLevels} open={open === r.id} onToggle={() => setOpen(open === r.id ? null : r.id)} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function SmartLine({ r, levels, open, onToggle }: { r: SmartRow; levels: boolean; open: boolean; onToggle: () => void }) {
  const Chev = open ? ChevronDown : ChevronRight;
  return (
    <>
      <tr className={`cursor-pointer border-t border-line/60 hover:bg-panel-2 ${r.status !== "ok" ? "bg-panel-2/60" : ""}`} onClick={onToggle}>
        <td className="px-2 py-1.5 text-mute">
          <Chev size={12} />
        </td>
        <td className="px-2 py-1.5 font-mono text-mute">{r.id.startsWith("nvme") ? "" : r.id}</td>
        <td className="px-2 py-1.5 text-ink">{r.name}</td>
        {levels && (
          <>
            <td className="tabular px-2 py-1.5 text-right font-mono text-dim">{r.current ?? "—"}</td>
            <td className="tabular px-2 py-1.5 text-right font-mono text-dim">{r.worst ?? "—"}</td>
            <td className="tabular px-2 py-1.5 text-right font-mono text-dim">{r.threshold ?? "—"}</td>
          </>
        )}
        <td className="tabular px-2 py-1.5 text-right font-mono text-ink">{r.raw}</td>
        <td className="px-2 py-1.5">
          <span className={`rounded-full border px-2 py-px text-[10px] ${PILL[r.status]}`}>{STATUS_LABEL[r.status]}</span>
        </td>
      </tr>
      {open && (
        <tr className="border-t border-line/40 bg-panel-2/40">
          <td />
          <td colSpan={levels ? 7 : 4} className="px-2 py-2 text-xs leading-relaxed text-dim">
            {r.explain}
            {r.action && <p className={`mt-1 font-medium ${TEXT[r.status]}`}>{r.action}</p>}
          </td>
        </tr>
      )}
    </>
  );
}

// ---------- Mapa y curva ----------

const CELL_CLASS: Record<string, string> = {
  ".": "bg-ok/70",
  s: "bg-warn/80",
  v: "bg-bad/60",
  x: "bg-bad",
  "?": "bg-line",
};

function SurfaceMap({ cells }: { cells: string }) {
  return (
    <div>
      <div className="grid gap-px" style={{ gridTemplateColumns: "repeat(50, minmax(0, 1fr))" }} role="img" aria-label="Mapa de la superficie del disco">
        {Array.from(cells, (c, i) => (
          <i key={i} className={`aspect-square rounded-[1px] ${CELL_CLASS[c] ?? "bg-line"}`} />
        ))}
      </div>
      <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-[10px] text-mute">
        <Legend cls="bg-ok/70" text="Bien" />
        <Legend cls="bg-warn/80" text="Lenta" />
        <Legend cls="bg-bad/60" text="Muy lenta" />
        <Legend cls="bg-bad" text="Error" />
        <Legend cls="bg-line" text="Sin mirar" />
      </div>
    </div>
  );
}

function Legend({ cls, text }: { cls: string; text: string }) {
  return (
    <span className="flex items-center gap-1">
      <i className={`inline-block size-2 rounded-[2px] ${cls}`} /> {text}
    </span>
  );
}

/** Velocidad de lectura a lo largo del disco, del principio al final. */
export function SpeedCurve({ curve, avg, min, max }: { curve: number[]; avg: number; min: number; max: number }) {
  if (curve.length < 2) return null;
  const w = 600;
  const h = 120;
  const top = Math.max(max, 1) * 1.1;
  const x = (i: number) => (i / (curve.length - 1)) * w;
  const y = (v: number) => h - (v / top) * h;
  const pts = curve.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  // Caída brusca: un punto por debajo de la mitad de la media.
  const dips = curve.map((v, i) => ({ v, i })).filter((p) => p.v < avg * 0.5);
  return (
    <div>
      <svg viewBox={`0 0 ${w} ${h + 16}`} className="w-full" role="img" aria-label="Curva de velocidad de lectura a lo largo del disco">
        {[0.25, 0.5, 0.75].map((f) => (
          <line key={f} x1={0} x2={w} y1={h * f} y2={h * f} stroke="var(--color-line)" strokeWidth={1} />
        ))}
        <polyline points={`0,${h} ${pts} ${w},${h}`} fill="var(--color-neon)" opacity={0.12} />
        <polyline points={pts} fill="none" stroke="var(--color-neon)" strokeWidth={1.6} vectorEffect="non-scaling-stroke" />
        {dips.map((p) => (
          <circle key={p.i} cx={x(p.i)} cy={y(p.v)} r={3.5} fill="var(--color-bad)" />
        ))}
        <text x={0} y={h + 12} fontSize={10} fill="var(--color-mute)">
          Principio
        </text>
        <text x={w} y={h + 12} fontSize={10} fill="var(--color-mute)" textAnchor="end">
          Final
        </text>
      </svg>
      <p className="mt-1 text-[11px] text-mute">
        Media {avg.toFixed(0)} MB/s · mínima {min.toFixed(0)} · máxima {max.toFixed(0)}
        {dips.length > 0 && <span className="text-bad"> · {dips.length} caídas bruscas: una zona del disco se lee mucho más lento que el resto.</span>}
      </p>
    </div>
  );
}

// ---------- Pruebas ----------

function TestsTab({ d }: { d: DiskReport }) {
  const toast = useToast();
  const [test, setTest] = useState<SelfTest | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"short" | "extended" | "abort" | null>(null);
  const [curve, setCurve] = useState<ScanResult | null>(null);
  const [scanning, setScanning] = useState(false);
  const isHdd = d.media.toUpperCase() !== "SSD" && d.bus.toUpperCase() !== "NVME";
  const sata = d.bus.toUpperCase() !== "NVME";

  const refresh = useCallback(() => {
    if (!sata) return;
    disksApi
      .selfTestStatus(d.number)
      .then((t) => (setTest(t), setTestError(null)))
      .catch((e) => setTestError(String(e)));
  }, [d.number, sata]);
  useEffect(refresh, [refresh]);
  useEffect(() => {
    disksApi
      .scanLast(d.number, d.model, d.size)
      .then((r) => r && r.mode === "quick" && setCurve(r))
      .catch(() => undefined);
  }, [d.number, d.model, d.size]);
  // Mientras corre, se pregunta cada pocos segundos cuánto lleva.
  useEffect(() => {
    if (test?.state !== "running") return;
    const t = window.setInterval(refresh, 5000);
    return () => window.clearInterval(t);
  }, [test?.state, refresh]);

  const start = async (action: "short" | "extended" | "abort") => {
    setBusy(action);
    try {
      setTest(await disksApi.selfTest(d.number, action));
      if (action !== "abort") toast("ok", `Autoprueba ${action === "short" ? "corta" : "extendida"} en marcha: el disco la hace por su cuenta y puedes seguir usándolo.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const runCurve = async () => {
    setScanning(true);
    try {
      setCurve(await disksApi.scan(d.number, d.model, isHdd, "quick", false));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setScanning(false);
    }
  };

  const running = test?.state === "running";
  return (
    <div className="space-y-3 px-4 pb-3">
      <div className="rounded-lg border border-line bg-panel-2 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <TestTube size={14} className="text-mute" />
          <span className="text-xs font-medium text-ink">Autoprueba del propio disco</span>
          <span className="text-[11px] text-mute">(SMART self-test: la hace el disco por dentro, sin tocar los datos)</span>
        </div>
        {!sata ? (
          <p className="mt-2 text-xs text-dim">Los NVMe no tienen autopruebas por este camino: vigilan su salud continuamente (mira la tabla de salud) y la curva de lectura de abajo sí sirve.</p>
        ) : testError ? (
          <p className="mt-2 text-xs text-dim">{testError}</p>
        ) : (
          <>
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <Button kind="secondary" size="sm" onClick={() => void start("short")} disabled={busy !== null || running}>
                Corta{test?.shortMinutes ? ` · ${test.shortMinutes} min` : ""}
              </Button>
              <Button kind="secondary" size="sm" onClick={() => void start("extended")} disabled={busy !== null || running}>
                Extendida{test?.extendedMinutes ? ` · ≈ ${test.extendedMinutes >= 90 ? `${Math.round(test.extendedMinutes / 60)} h` : `${test.extendedMinutes} min`}` : ""}
              </Button>
              {running && (
                <Button kind="danger" size="sm" onClick={() => void start("abort")} disabled={busy !== null}>
                  <Square size={12} /> Cancelar
                </Button>
              )}
              {busy && <Loader2 size={13} className="animate-spin text-neon" />}
            </div>
            {test && (
              <div className="mt-2">
                {running && (
                  <div className="mb-1 h-1.5 overflow-hidden rounded-full bg-line">
                    <div className="h-full rounded-full bg-neon" style={{ width: `${test.percent}%` }} />
                  </div>
                )}
                <p className={`text-xs ${test.state === "failed" ? "text-bad" : test.state === "passed" ? "text-ok" : "text-dim"}`}>
                  {running ? `En marcha · ${test.percent} %` : test.text}
                </p>
              </div>
            )}
          </>
        )}
      </div>

      <div className="rounded-lg border border-line bg-panel-2 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <Gauge size={14} className="text-mute" />
          <span className="text-xs font-medium text-ink">Curva de velocidad de lectura</span>
          <span className="text-[11px] text-mute">(unos minutos, solo lectura)</span>
          <span className="ml-auto" />
          <Button kind="secondary" size="sm" onClick={() => void runCurve()} disabled={scanning}>
            {scanning ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} />} {curve ? "Repetir" : "Medir"}
          </Button>
        </div>
        {scanning && <TaskStatus task={`disk-scan:${d.number}`} active className="mt-2" fallback="Leyendo muestras del disco…" />}
        {curve && !scanning && (
          <div className="mt-2">
            <SpeedCurve curve={curve.curve} avg={curve.avgMbps} min={curve.minMbps} max={curve.maxMbps} />
            <p className="mt-1 text-[11px] text-mute">
              Medido el {new Date(curve.finished * 1000).toLocaleDateString("es", { day: "numeric", month: "short" })}.
              {!isHdd && " En un SSD, una curva que baja con el uso indica que se ralentiza al llenarse."}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

// ---------- Superficie ----------

const eta = (s: number) => (s >= 3600 ? `${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min` : `${Math.max(1, Math.round(s / 60))} min`);

function SurfaceTab({ d }: { d: DiskReport }) {
  const toast = useToast();
  const [last, setLast] = useState<ScanResult | null>(null);
  const [live, setLive] = useState<ScanLive | null>(null);
  const [busy, setBusy] = useState<"quick" | "full" | null>(null);
  const isHdd = d.media.toUpperCase() !== "SSD" && d.bus.toUpperCase() !== "NVME";
  const task = `disk-scan:${d.number}`;

  useEffect(() => {
    disksApi
      .scanLast(d.number, d.model, d.size)
      .then(setLast)
      .catch(() => undefined);
  }, [d.number, d.model, d.size]);
  const timer = useRef<number | null>(null);
  useEffect(() => {
    if (!busy) return;
    const tick = () =>
      disksApi
        .scanLive(d.number)
        .then((l) => l && setLive(l))
        .catch(() => undefined);
    timer.current = window.setInterval(tick, 1000);
    return () => {
      if (timer.current) window.clearInterval(timer.current);
      setLive(null);
    };
  }, [busy, d.number]);

  const run = async (mode: "quick" | "full", resume: boolean) => {
    setBusy(mode);
    try {
      setLast(await disksApi.scan(d.number, d.model, isHdd, mode, resume));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };
  const stop = () => void appApi.cancelTask(task).catch(() => undefined);

  const partial = last && last.finished === 0;
  const shown = busy && live ? live.cells : (last?.cells ?? "");
  return (
    <div className="space-y-3 px-4 pb-3">
      <p className="text-xs text-dim">
        Lee el disco por zonas, en solo lectura, y pinta cada una según lo que tarda: verde lee bien, ámbar lento, rojo da error. No mueve ni arregla nada (eso lo hace «Buscar sectores dañados»); sirve para saber qué zonas están enfermas
        <b className="text-ink"> antes</b> de decidir.
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <Button kind="secondary" size="sm" onClick={() => void run("quick", false)} disabled={busy !== null}>
          {busy === "quick" ? <Loader2 size={13} className="animate-spin" /> : <ScanSearch size={13} />} Rápida (una muestra de cada zona)
        </Button>
        <Button kind="secondary" size="sm" onClick={() => void run("full", false)} disabled={busy !== null}>
          {busy === "full" ? <Loader2 size={13} className="animate-spin" /> : <ScanSearch size={13} />} Completa (todo el disco)
        </Button>
        {partial && !busy && (
          <Button size="sm" onClick={() => void run(last.mode, true)}>
            <Play size={13} /> Seguir ({Math.round((last.done * 100) / Math.max(last.total, 1))} %)
          </Button>
        )}
        {busy && (
          <Button kind="danger" size="sm" onClick={stop}>
            <Square size={12} /> Parar
          </Button>
        )}
      </div>
      {busy && (
        <div>
          <TaskStatus task={task} active cancellable={false} fallback="Leyendo el disco…" />
          {live && (
            <p className="mt-1 text-[11px] text-mute">
              {live.percent} % · {live.mbps.toFixed(0)} MB/s · quedan {eta(live.etaSecs)} · {live.slow + live.verySlow} lentas · {live.bad} con error
            </p>
          )}
        </div>
      )}
      {shown && (
        <div className="rounded-lg border border-line bg-panel-2 p-3">
          <SurfaceMap cells={shown} />
        </div>
      )}
      {last && !busy && (
        <div className={`rounded-lg border p-3 text-xs ${last.finished === 0 ? "border-line bg-panel-2 text-dim" : last.level === "bad" ? "border-bad/40 bg-bad/10" : last.level === "warn" ? "border-warn/40 bg-warn/10" : "border-ok/40 bg-ok/10"}`}>
          <p className={last.finished === 0 ? "text-dim" : TEXT[last.level]}>{last.text}</p>
          <p className="mt-1 text-[11px] text-mute">
            {last.mode === "quick" ? "Prueba rápida" : "Prueba completa"} · {bytes(last.size)} · {last.total} zonas de {bytes(last.cellBytes, 0)} · media {last.avgMbps.toFixed(0)} MB/s
            {last.finished > 0 && ` · ${new Date(last.finished * 1000).toLocaleDateString("es", { day: "numeric", month: "short", year: "numeric" })}`}
          </p>
        </div>
      )}
    </div>
  );
}

// ---------- Contenedor ----------

const TABS = [
  { id: "smart", label: "Salud SMART", Icon: Activity },
  { id: "tests", label: "Pruebas y velocidad", Icon: Gauge },
  { id: "surface", label: "Mapa de superficie", Icon: ScanSearch },
] as const;

/** Las pestañas de análisis a fondo de un disco (solo técnico y con administrador). */
export function DiskDeep({ d, isAdmin }: { d: DiskReport; isAdmin: boolean }) {
  const [tab, setTab] = useState<(typeof TABS)[number]["id"] | null>(null);
  if (!isAdmin) return null;
  return (
    <div className="border-t border-line/60">
      <div className="flex flex-wrap gap-1 px-3 pt-2" role="tablist" aria-label={`Análisis a fondo de ${d.model}`}>
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            onClick={() => setTab(tab === t.id ? null : t.id)}
            className={`flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs transition-colors ${tab === t.id ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2 hover:text-ink"}`}
          >
            <t.Icon size={13} /> {t.label}
          </button>
        ))}
      </div>
      <div className={tab ? "pt-2" : ""}>
        {tab === "smart" && <SmartTab d={d} />}
        {tab === "tests" && <TestsTab d={d} />}
        {tab === "surface" && <SurfaceTab d={d} />}
      </div>
    </div>
  );
}

// ---------- Vigilante ----------

export function DiskWatchCard() {
  const [cfg, setCfg] = useState<WatchConfig | null>(null);
  const toast = useToast();
  useEffect(() => {
    disksApi
      .watchGet()
      .then(setCfg)
      .catch(() => undefined);
  }, []);
  if (!cfg) return null;
  const save = (next: WatchConfig) => {
    setCfg(next);
    disksApi
      .watchSet(next)
      .then(setCfg)
      .catch((e) => toast("error", String(e)));
  };
  const num = (label: string, value: number, unit: string, set: (n: number) => void, min: number, max: number) => (
    <label className="flex items-center justify-between gap-3 text-xs text-dim">
      <span>{label}</span>
      <span className="flex items-center gap-1.5">
        <input
          type="number"
          min={min}
          max={max}
          value={value}
          disabled={!cfg.enabled}
          onChange={(e) => set(Number(e.target.value))}
          className="h-8 w-20 rounded-md border border-line-2 bg-void px-2 text-right text-xs text-ink tabular"
        />
        <span className="w-8 text-mute">{unit}</span>
      </span>
    </label>
  );
  return (
    <Card title="Vigilante de discos" icon={<Bell size={14} />}>
      <div className="space-y-3 p-4">
        <div className="flex items-start gap-3">
          <Toggle checked={cfg.enabled} onChange={(v) => save({ ...cfg, enabled: v })} label="Vigilar los discos en segundo plano" />
          <p className="min-w-0 flex-1 text-xs text-dim">
            Con AdminOps abierto, aunque esté minimizado, mira los discos cada cierto tiempo y avisa solo cuando algo <b className="text-ink">cambia</b>: sectores pendientes que suben, un disco que anuncia que va a fallar, calor sostenido, poco
            espacio libre o un disco fijo que desaparece. Los avisos van a la campana y a «Hoy», y quedan en el diario.
          </p>
        </div>
        <div className="grid gap-2 sm:grid-cols-2">
          {num("Mirar cada", cfg.intervalMin, "min", (n) => save({ ...cfg, intervalMin: n }), 5, 1440)}
          {num("Avisar si queda menos de", cfg.freePct, "% libre", (n) => save({ ...cfg, freePct: n }), 1, 50)}
          {num("Calor sostenido en discos mecánicos", cfg.tempHdd, "°C", (n) => save({ ...cfg, tempHdd: n }), 35, 90)}
          {num("Calor sostenido en SSD y NVMe", cfg.tempSsd, "°C", (n) => save({ ...cfg, tempSsd: n }), 35, 100)}
        </div>
      </div>
    </Card>
  );
}
