import { listen } from "@tauri-apps/api/event";
import { ArrowDown, ArrowUp, Check, Eye, EyeOff, Globe, Server, Square, Wifi, X, Zap } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toolsApi, type Adapter, type SpeedMeta, type SpeedProgress, type SpeedResult } from "../lib/api";
import { useToast } from "./feedback";

/** Marcas del medidor, repartidas uniformemente en el arco (escala no lineal). */
const TICKS = [0, 5, 10, 25, 50, 100, 250, 500, 1000];
const SWEEP = 270;

function angleFor(mbps: number) {
  const v = Math.max(0, Math.min(mbps, TICKS[TICKS.length - 1]));
  const step = SWEEP / (TICKS.length - 1);
  for (let i = 1; i < TICKS.length; i++) {
    if (v <= TICKS[i]) return -135 + step * (i - 1 + (v - TICKS[i - 1]) / (TICKS[i] - TICKS[i - 1]));
  }
  return 135;
}

const polar = (cx: number, cy: number, r: number, deg: number) => {
  const a = (deg * Math.PI) / 180;
  return [cx + r * Math.sin(a), cy - r * Math.cos(a)];
};

function Gauge({
  mbps,
  color,
  running,
  onStart,
  label,
}: {
  mbps: number;
  color: string;
  running: boolean;
  onStart: () => void;
  label: string;
}) {
  const size = 280;
  const c = size / 2;
  const r = 112;
  const circ = 2 * Math.PI * r;
  const arc = circ * (SWEEP / 360);
  const angle = angleFor(mbps);
  const filled = arc * ((angle + 135) / SWEEP);

  return (
    <div className="relative mx-auto select-none" style={{ width: size, height: size - 30 }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="absolute top-0 left-0">
        <circle cx={c} cy={c} r={r} fill="none" stroke="var(--color-line)" strokeWidth={14} strokeLinecap="round" strokeDasharray={`${arc} ${circ}`} transform={`rotate(135 ${c} ${c})`} />
        <circle
          cx={c}
          cy={c}
          r={r}
          fill="none"
          stroke={color}
          strokeWidth={14}
          strokeLinecap="round"
          strokeDasharray={`${filled} ${circ}`}
          transform={`rotate(135 ${c} ${c})`}
          style={{ transition: "stroke-dasharray .35s ease-out" }}
        />
        {TICKS.map((t, i) => {
          const deg = -135 + (SWEEP / (TICKS.length - 1)) * i;
          const [x1, y1] = polar(c, c, r - 16, deg);
          const [x2, y2] = polar(c, c, r - 10, deg);
          const [tx, ty] = polar(c, c, r - 30, deg);
          return (
            <g key={t}>
              <line x1={x1} y1={y1} x2={x2} y2={y2} stroke="var(--color-mute)" strokeWidth={1.5} />
              <text x={tx} y={ty + 3} textAnchor="middle" fontSize={10} fill="var(--color-mute)" fontFamily="Cascadia Mono, monospace">
                {t}
              </text>
            </g>
          );
        })}
        {running && (
          <g style={{ transform: `rotate(${angle}deg)`, transformOrigin: `${c}px ${c}px`, transition: "transform .35s ease-out" }}>
            <line x1={c} y1={c + 10} x2={c} y2={c - r + 26} stroke={color} strokeWidth={3} strokeLinecap="round" />
            <circle cx={c} cy={c} r={7} fill="var(--color-panel)" stroke={color} strokeWidth={3} />
          </g>
        )}
      </svg>
      <div className="absolute inset-x-0 text-center" style={{ top: c + 34 }}>
        {running ? (
          <>
            <div className="font-mono text-4xl font-semibold tabular" style={{ color }}>
              {mbps >= 100 ? mbps.toFixed(0) : mbps.toFixed(1)}
            </div>
            <div className="text-xs text-dim">Mbps · {label}</div>
          </>
        ) : null}
      </div>
      {!running && (
        <button
          onClick={onStart}
          className="absolute top-1/2 left-1/2 grid size-24 -translate-x-1/2 -translate-y-[50%] place-items-center rounded-full border-2 border-neon bg-neon/10 text-base font-bold text-neon transition-transform hover:scale-105 hover:bg-neon/20"
        >
          INICIAR
        </button>
      )}
    </div>
  );
}

type Sample = { phase: "download" | "upload"; t: number; mbps: number };

function LiveChart({ samples }: { samples: Sample[] }) {
  const w = 520;
  const h = 90;
  const max = Math.max(10, ...samples.map((s) => s.mbps)) * 1.1;
  const x = (s: Sample) => (s.phase === "download" ? 0 : w / 2) + (s.t / 10) * (w / 2);
  const path = (phase: Sample["phase"]) => {
    const pts = samples.filter((s) => s.phase === phase);
    if (pts.length < 2) return null;
    const line = pts.map((s) => `${x(s)},${h - (s.mbps / max) * (h - 4)}`).join(" ");
    return { line, area: `${x(pts[0])},${h} ${line} ${x(pts[pts.length - 1])},${h}` };
  };
  const d = path("download");
  const u = path("upload");
  return (
    <svg viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" className="h-24 w-full">
      <line x1={w / 2} y1={0} x2={w / 2} y2={h} stroke="var(--color-line)" strokeDasharray="3 3" />
      {d && (
        <>
          <polygon points={d.area} fill="var(--color-neon)" opacity={0.15} />
          <polyline points={d.line} fill="none" stroke="var(--color-neon)" strokeWidth={1.8} vectorEffect="non-scaling-stroke" />
        </>
      )}
      {u && (
        <>
          <polygon points={u.area} fill="var(--color-neon-2)" opacity={0.15} />
          <polyline points={u.line} fill="none" stroke="var(--color-neon-2)" strokeWidth={1.8} vectorEffect="non-scaling-stroke" />
        </>
      )}
      <text x={6} y={12} fontSize={10} fill="var(--color-mute)">Bajada</text>
      <text x={w / 2 + 6} y={12} fontSize={10} fill="var(--color-mute)">Subida</text>
    </svg>
  );
}

const HIDE_KEY = "adminops.hideIp";
const readHide = () => {
  try {
    return localStorage.getItem(HIDE_KEY) === "1";
  } catch {
    return false;
  }
};

export function SpeedTest({ adapter, onResult }: { adapter: Adapter | null; onResult: (r: SpeedResult) => void }) {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<SpeedProgress | null>(null);
  const [meta, setMeta] = useState<SpeedMeta | null>(null);
  const [result, setResult] = useState<SpeedResult | null>(null);
  const [phaseValues, setPhaseValues] = useState<{ latency?: number; download?: number; upload?: number }>({});
  const [samples, setSamples] = useState<Sample[]>([]);
  const [hideIp, setHideIp] = useState(readHide);
  const toast = useToast();
  const phaseStart = useRef(0);

  useEffect(() => {
    toolsApi.speedHistory().then((h) => h[0] && setResult((r) => r ?? h[0]));
    const a = listen<SpeedProgress>("speedtest-progress", (e) => {
      const p = e.payload;
      setProgress(p);
      if (p.progress >= 1) {
        if (p.phase === "latency" && p.latencyMs !== null) setPhaseValues((v) => ({ ...v, latency: p.latencyMs! }));
        if (p.phase === "download" || p.phase === "upload") setPhaseValues((v) => ({ ...v, [p.phase]: p.mbps }));
      } else if (p.phase === "download" || p.phase === "upload") {
        setSamples((s) => {
          if (!s.some((x) => x.phase === p.phase)) phaseStart.current = performance.now();
          return [...s, { phase: p.phase as Sample["phase"], t: p.progress * 10, mbps: p.mbps }];
        });
      }
    });
    const b = listen<SpeedMeta>("speedtest-meta", (e) => setMeta(e.payload));
    return () => {
      a.then((f) => f());
      b.then((f) => f());
    };
  }, []);

  const start = async () => {
    setRunning(true);
    setResult(null);
    setMeta(null);
    setSamples([]);
    setPhaseValues({});
    setProgress({ phase: "meta", mbps: 0, progress: 0, latencyMs: null });
    try {
      const r = await toolsApi.speedtest();
      setResult(r);
      onResult(r);
    } catch (e) {
      toast(String(e).includes("cancelado") ? "info" : "error", String(e));
    } finally {
      setRunning(false);
      setProgress(null);
    }
  };

  const toggleHide = () => {
    setHideIp((h) => {
      try {
        localStorage.setItem(HIDE_KEY, h ? "0" : "1");
      } catch {
        /* sin almacenamiento */
      }
      return !h;
    });
  };

  const phase = progress?.phase ?? "meta";
  const color = phase === "upload" ? "var(--color-neon-2)" : "var(--color-neon)";
  const gaugeValue = running ? (phase === "download" || phase === "upload" ? (progress?.mbps ?? 0) : 0) : 0;
  const label = { meta: "conectando", latency: "latencia", download: "bajada", upload: "subida", done: "listo" }[phase];

  const shown = result;
  const latency = phaseValues.latency ?? shown?.latencyMs;
  const live = (ph: "download" | "upload") => (running && phase === ph && progress && progress.progress < 1 ? progress.mbps : undefined);
  const down = phaseValues.download ?? live("download") ?? shown?.downloadMbps;
  const up = phaseValues.upload ?? live("upload") ?? shown?.uploadMbps;
  const bloat = shown?.downloadLatencyMs != null ? shown.downloadLatencyMs - shown.latencyMs : null;

  const steps = [
    { key: "latency", label: "Latencia", value: latency != null ? `${latency.toFixed(0)} ms` : null, icon: Zap },
    { key: "download", label: "Bajada", value: down != null ? `${down.toFixed(1)} Mbps` : null, icon: ArrowDown },
    { key: "upload", label: "Subida", value: up != null ? `${up.toFixed(1)} Mbps` : null, icon: ArrowUp },
  ] as const;
  const order = ["meta", "latency", "download", "upload", "done"];

  const suited = shown
    ? [
        { label: "Streaming 4K", ok: shown.downloadMbps >= 25 },
        { label: "Videollamadas HD", ok: shown.downloadMbps >= 5 && shown.uploadMbps >= 3 && shown.latencyMs < 100 && shown.jitterMs < 30 },
        { label: "Juegos online", ok: shown.latencyMs < 50 && shown.jitterMs < 15 && (bloat === null || bloat < 50) },
        { label: "Teletrabajo y subir archivos", ok: shown.uploadMbps >= 10 },
      ]
    : [];

  const ipv4 = meta?.ipv4 ?? shown?.ipv4 ?? (shown?.ip && !shown.ip.includes(":") ? shown.ip : null);
  const ipv6 = meta?.ipv6 ?? shown?.ipv6 ?? (shown?.ip?.includes(":") ? shown.ip : null);
  const isp = meta?.isp ?? shown?.isp;
  const location = meta?.location ?? shown?.location;
  const server = meta?.colo ? `Cloudflare ${meta.colo}${meta.country ? ` (${meta.country})` : ""}` : shown?.server;
  const mask = (v: string | null | undefined) => (!v ? "—" : hideIp ? v.replace(/[0-9a-f]/gi, "•") : v);

  return (
    <div className="grid grid-cols-12 gap-5">
      <div className="col-span-12 lg:col-span-6">
        <Gauge mbps={gaugeValue} color={color} running={running} onStart={start} label={label} />
        {/* Fases */}
        <div className="mt-2 grid grid-cols-3 gap-2">
          {steps.map((s) => {
            const idx = order.indexOf(s.key);
            const cur = order.indexOf(phase);
            const active = running && cur === idx;
            const done = s.value !== null && (!running || cur > idx || (phase === s.key && progress?.progress === 1));
            return (
              <div
                key={s.key}
                className={`rounded-lg border px-3 py-2 text-center transition-colors ${
                  active ? "border-neon/60 bg-neon/10" : done ? "border-line-2 bg-void/40" : "border-line bg-void/20"
                }`}
              >
                <div className={`flex items-center justify-center gap-1 text-[11px] ${active ? "text-neon" : "text-mute"}`}>
                  <s.icon size={11} className={active ? "animate-bounce" : ""} /> {s.label}
                </div>
                <div className="mt-0.5 font-mono text-sm tabular text-ink">{s.value ?? (active ? "…" : "—")}</div>
              </div>
            );
          })}
        </div>
        <div className="mt-3 flex items-center justify-center gap-3">
          {running ? (
            <button onClick={() => toolsApi.cancelSpeedtest()} className="flex items-center gap-1.5 rounded-full border border-bad/50 px-4 py-1.5 text-xs text-bad hover:bg-bad/10">
              <Square size={10} fill="currentColor" /> Detener
            </button>
          ) : (
            shown && (
              <button onClick={start} className="text-xs text-dim hover:text-neon">
                Repetir test ↻
              </button>
            )
          )}
        </div>
        {(running || samples.length > 0) && (
          <div className="mt-3 rounded-lg border border-line bg-void/30 px-2 pt-1">
            <LiveChart samples={samples} />
          </div>
        )}
      </div>

      <div className="col-span-12 space-y-3 lg:col-span-6">
        {/* Resultado */}
        <div className="grid grid-cols-2 gap-2.5">
          <div className="rounded-lg border border-line bg-void/40 px-3 py-2.5">
            <div className="flex items-center gap-1 text-[11px] text-mute">
              <ArrowDown size={11} className="text-neon" /> Bajada
            </div>
            <div className="font-mono text-3xl tabular text-neon">{down != null ? down.toFixed(1) : "—"}</div>
            <div className="text-[11px] text-mute">Mbps</div>
          </div>
          <div className="rounded-lg border border-line bg-void/40 px-3 py-2.5">
            <div className="flex items-center gap-1 text-[11px] text-mute">
              <ArrowUp size={11} className="text-neon-2" /> Subida
            </div>
            <div className="font-mono text-3xl tabular text-neon-2">{up != null ? up.toFixed(1) : "—"}</div>
            <div className="text-[11px] text-mute">Mbps</div>
          </div>
          <div className="rounded-lg border border-line bg-void/40 px-3 py-2">
            <div className="text-[11px] text-mute">Latencia · jitter</div>
            <div className="font-mono text-lg tabular">
              {latency != null ? `${latency.toFixed(0)} ms` : "—"}
              {shown && <span className="text-sm text-dim"> · {shown.jitterMs.toFixed(1)} ms</span>}
            </div>
          </div>
          <div className="rounded-lg border border-line bg-void/40 px-3 py-2">
            <div className="text-[11px] text-mute">Latencia con carga</div>
            <div className={`font-mono text-lg tabular ${bloat !== null && bloat > 100 ? "text-bad" : bloat !== null && bloat > 30 ? "text-warn" : ""}`}>
              {shown?.downloadLatencyMs != null ? `${shown.downloadLatencyMs.toFixed(0)} ms` : "—"}
            </div>
            <div className="text-[11px] text-mute">
              {bloat === null ? "Durante la bajada" : bloat > 100 ? "Bufferbloat alto" : bloat > 30 ? "Bufferbloat moderado" : "Sin bufferbloat"}
            </div>
          </div>
        </div>

        {suited.length > 0 && (
          <div className="rounded-lg border border-line bg-void/30 px-3 py-2">
            <div className="mb-1 text-[11px] text-mute">Apto para</div>
            <div className="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
              {suited.map((q) => (
                <span key={q.label} className={`flex items-center gap-1.5 ${q.ok ? "text-ink" : "text-mute"}`}>
                  {q.ok ? <Check size={12} className="text-ok" /> : <X size={12} className="text-bad" />} {q.label}
                </span>
              ))}
            </div>
          </div>
        )}

        {/* Conexión */}
        <div className="rounded-lg border border-line bg-void/30 px-3 py-2.5 text-xs">
          <div className="mb-1.5 flex items-center justify-between">
            <span className="text-[11px] text-mute">Tu conexión</span>
            <button onClick={toggleHide} className="flex items-center gap-1 text-mute hover:text-ink" title="Ocultar las IP (por ejemplo al compartir pantalla)">
              {hideIp ? <EyeOff size={12} /> : <Eye size={12} />} {hideIp ? "Mostrar IP" : "Ocultar IP"}
            </button>
          </div>
          <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
            <dt className="text-mute">IP pública (IPv4)</dt>
            <dd className="font-mono text-ink select-text">{mask(ipv4)}</dd>
            {ipv6 && (
              <>
                <dt className="text-mute">IPv6</dt>
                <dd className="truncate font-mono text-dim select-text" title={hideIp ? undefined : ipv6}>
                  {mask(ipv6)}
                </dd>
              </>
            )}
            <dt className="flex items-center gap-1 text-mute">
              <Globe size={11} /> Proveedor
            </dt>
            <dd className="text-ink">{isp ?? "—"}</dd>
            <dt className="text-mute">Ubicación aprox.</dt>
            <dd className="text-dim">{location ?? "—"}</dd>
            <dt className="flex items-center gap-1 text-mute">
              <Server size={11} /> Servidor
            </dt>
            <dd className="text-dim">{server ?? "—"}</dd>
            {adapter && (
              <>
                <dt className="flex items-center gap-1 text-mute">
                  <Wifi size={11} /> Adaptador
                </dt>
                <dd className="text-dim">
                  {adapter.name} · {adapter.linkSpeed}
                  {adapter.signal !== null && ` · señal ${adapter.signal}%`} · <span className="font-mono">{mask(adapter.ipv4[0])}</span>
                </dd>
              </>
            )}
          </dl>
        </div>
        <p className="text-[11px] text-mute">
          Servidores de Cloudflare; proveedor y ubicación según ipinfo.io. {shown && `${shown.transferredMb.toFixed(0)} MB transferidos.`}
        </p>
      </div>
    </div>
  );
}
