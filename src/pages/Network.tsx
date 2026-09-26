import { listen } from "@tauri-apps/api/event";
import { ArrowDown, ArrowUp, CircleCheck, Gauge, Globe, Loader2, Play, RefreshCw, Router, Square, TriangleAlert, Wifi, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { Card } from "../components/ui";
import { toolsApi, type NetworkReport, type SpeedProgress, type SpeedResult } from "../lib/api";

const PHASES: Record<SpeedProgress["phase"], string> = {
  meta: "Conectando con el servidor…",
  latency: "Midiendo latencia…",
  download: "Midiendo bajada…",
  upload: "Midiendo subida…",
  done: "Listo",
};

/** Arco de velocidad con escala logarítmica (1 Mbps – 1 Gbps). */
function SpeedGauge({ mbps, phase }: { mbps: number; phase: SpeedProgress["phase"] | null }) {
  const size = 240;
  const r = 100;
  const c = Math.PI * r;
  const frac = mbps <= 0 ? 0 : Math.min(1, Math.log10(Math.max(1, mbps)) / 3);
  const color = phase === "upload" ? "var(--color-neon-2)" : "var(--color-neon)";
  return (
    <div className="relative mx-auto" style={{ width: size, height: size / 2 + 30 }}>
      <svg width={size} height={size / 2 + 12} viewBox={`0 0 ${size} ${size / 2 + 12}`}>
        <path d={`M 20 ${size / 2} A ${r} ${r} 0 0 1 ${size - 20} ${size / 2}`} fill="none" stroke="var(--color-line)" strokeWidth={14} strokeLinecap="round" />
        <path
          d={`M 20 ${size / 2} A ${r} ${r} 0 0 1 ${size - 20} ${size / 2}`}
          fill="none"
          stroke={color}
          strokeWidth={14}
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c - frac * c}
          style={{ transition: "stroke-dashoffset .25s linear" }}
        />
      </svg>
      <div className="absolute inset-x-0 bottom-0 text-center">
        <div className="font-mono text-4xl font-semibold tabular" style={{ color }}>
          {mbps >= 100 ? mbps.toFixed(0) : mbps.toFixed(1)}
        </div>
        <div className="text-xs text-mute">Mbps</div>
      </div>
    </div>
  );
}

function Tile({ icon, label, value, unit, sub, color }: { icon: React.ReactNode; label: string; value: string; unit: string; sub?: string; color?: string }) {
  return (
    <div className="rounded-lg border border-line bg-void/40 px-3 py-2.5">
      <div className="flex items-center gap-1.5 text-[10px] tracking-widest text-mute uppercase">
        {icon} {label}
      </div>
      <div className="mt-0.5 font-mono text-2xl tabular" style={{ color }}>
        {value} <span className="text-xs text-mute">{unit}</span>
      </div>
      {sub && <div className="text-[11px] text-dim">{sub}</div>}
    </div>
  );
}

const when = (ts: number) => new Date(ts * 1000).toLocaleString("es", { dateStyle: "short", timeStyle: "short" });

export function Network() {
  const [report, setReport] = useState<NetworkReport | null>(null);
  const [diagBusy, setDiagBusy] = useState(false);
  const [history, setHistory] = useState<SpeedResult[]>([]);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<SpeedProgress | null>(null);
  const [latencies, setLatencies] = useState<number[]>([]);
  const [result, setResult] = useState<SpeedResult | null>(null);
  const toast = useToast();

  const diagnose = useCallback(async () => {
    setDiagBusy(true);
    try {
      setReport(await toolsApi.network());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setDiagBusy(false);
    }
  }, [toast]);

  useEffect(() => {
    diagnose();
    toolsApi.speedHistory().then((h) => {
      setHistory(h);
      setResult((r) => r ?? h[0] ?? null);
    });
    const un = listen<SpeedProgress>("speedtest-progress", (e) => {
      setProgress(e.payload);
      if (e.payload.phase === "latency" && e.payload.latencyMs !== null) setLatencies((l) => [...l, e.payload.latencyMs!]);
    });
    return () => {
      un.then((f) => f());
    };
  }, [diagnose]);

  const run = async () => {
    setRunning(true);
    setResult(null);
    setLatencies([]);
    setProgress({ phase: "meta", mbps: 0, progress: 0, latencyMs: null });
    try {
      const r = await toolsApi.speedtest();
      setResult(r);
      setHistory((h) => [r, ...h]);
    } catch (e) {
      toast(String(e).includes("cancelado") ? "info" : "error", String(e));
    } finally {
      setRunning(false);
      setProgress(null);
    }
  };

  const liveLatency = latencies.length ? [...latencies].sort((a, b) => a - b)[Math.floor(latencies.length / 2)] : null;
  const shownMbps = running ? (progress?.mbps ?? 0) : 0;
  const bloat = result?.downloadLatencyMs != null ? result.downloadLatencyMs - result.latencyMs : null;

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      {/* Speedtest */}
      <Card title="Test de velocidad" icon={<Gauge size={14} />} className="col-span-12 lg:col-span-7">
        <div className="grid grid-cols-2 items-center gap-6">
          <div>
            <SpeedGauge mbps={running ? shownMbps : (result?.downloadMbps ?? 0)} phase={running ? (progress?.phase ?? null) : "download"} />
            <div className="mt-3 text-center text-xs text-dim">
              {running ? PHASES[progress?.phase ?? "meta"] : result ? `Último test · ${when(result.timestamp)}` : "Sin tests todavía"}
            </div>
            {running && progress && (progress.phase === "download" || progress.phase === "upload" || progress.phase === "latency") && (
              <div className="mx-auto mt-2 h-1 w-40 overflow-hidden rounded-full bg-line">
                <div className="h-full bg-neon transition-[width]" style={{ width: `${progress.progress * 100}%` }} />
              </div>
            )}
            <div className="mt-4 flex justify-center">
              {running ? (
                <button
                  onClick={() => toolsApi.cancelSpeedtest()}
                  className="flex items-center gap-1.5 rounded-full border border-bad/50 px-5 py-2 text-sm text-bad hover:bg-bad/10"
                >
                  <Square size={12} fill="currentColor" /> Detener
                </button>
              ) : (
                <button
                  onClick={run}
                  className="flex items-center gap-2 rounded-full border border-neon/60 bg-neon/10 px-6 py-2 text-sm font-semibold text-neon glow-neon hover:bg-neon/20"
                >
                  <Play size={14} fill="currentColor" /> {result ? "Repetir test" : "Iniciar test"}
                </button>
              )}
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2.5">
            <Tile
              icon={<ArrowDown size={11} className="text-neon" />}
              label="Bajada"
              value={result ? result.downloadMbps.toFixed(1) : running && progress?.phase === "download" ? progress.mbps.toFixed(1) : "—"}
              unit="Mbps"
              color="var(--color-neon)"
            />
            <Tile
              icon={<ArrowUp size={11} className="text-neon-2" />}
              label="Subida"
              value={result ? result.uploadMbps.toFixed(1) : running && progress?.phase === "upload" ? progress.mbps.toFixed(1) : "—"}
              unit="Mbps"
              color="var(--color-neon-2)"
            />
            <Tile
              icon={<Globe size={11} />}
              label="Latencia"
              value={result ? result.latencyMs.toFixed(0) : liveLatency ? liveLatency.toFixed(0) : "—"}
              unit="ms"
              sub={result ? `Jitter ${result.jitterMs.toFixed(1)} ms` : undefined}
            />
            <Tile
              icon={<TriangleAlert size={11} />}
              label="Con carga"
              value={result?.downloadLatencyMs != null ? result.downloadLatencyMs.toFixed(0) : "—"}
              unit="ms"
              sub={
                bloat === null ? "Latencia durante la bajada" : bloat > 100 ? "Bufferbloat alto: tirones en llamadas" : bloat > 30 ? "Bufferbloat moderado" : "Sin bufferbloat"
              }
              color={bloat !== null && bloat > 100 ? "var(--color-bad)" : bloat !== null && bloat > 30 ? "var(--color-warn)" : undefined}
            />
          </div>
        </div>
        {result && (
          <p className="mt-4 text-center text-[11px] text-mute">
            {result.server}
            {result.ip && <> · IP {result.ip}</>} · {result.transferredMb.toFixed(0)} MB transferidos
          </p>
        )}
        <p className="mt-2 text-center text-[11px] text-mute">
          Varias conexiones en paralelo durante 10 s por sentido; se descarta el arranque de TCP. Servidores de Cloudflare.
        </p>
      </Card>

      {/* Historial */}
      <Card title="Historial de tests" className="col-span-12 lg:col-span-5">
        {history.length === 0 ? (
          <p className="text-sm text-mute">Aún no hay tests en este equipo.</p>
        ) : (
          <table className="w-full text-xs">
            <thead>
              <tr className="text-left text-[10px] tracking-widest text-mute uppercase">
                <th className="pb-2 font-medium">Fecha</th>
                <th className="pb-2 text-right font-medium">Bajada</th>
                <th className="pb-2 text-right font-medium">Subida</th>
                <th className="pb-2 text-right font-medium">Ping</th>
              </tr>
            </thead>
            <tbody className="font-mono tabular">
              {history.slice(0, 12).map((h) => (
                <tr key={h.timestamp} onClick={() => setResult(h)} className="cursor-pointer border-t border-line/60 hover:bg-panel-2">
                  <td className="py-1.5 font-sans text-dim">{when(h.timestamp)}</td>
                  <td className="py-1.5 text-right text-neon">{h.downloadMbps.toFixed(1)}</td>
                  <td className="py-1.5 text-right text-neon-2">{h.uploadMbps.toFixed(1)}</td>
                  <td className="py-1.5 text-right">{h.latencyMs.toFixed(0)} ms</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      {/* Diagnóstico */}
      <Card
        title="Diagnóstico de red"
        icon={<Router size={14} />}
        className="col-span-12"
        right={
          <button onClick={diagnose} disabled={diagBusy} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
            <RefreshCw size={11} className={diagBusy ? "animate-spin" : ""} /> Volver a comprobar
          </button>
        }
      >
        {!report ? (
          <p className="flex items-center gap-2 text-sm text-dim">
            <Loader2 size={14} className="animate-spin" /> Comprobando adaptadores, router, DNS e Internet…
          </p>
        ) : (
          <div className="grid grid-cols-12 gap-4">
            <div className="col-span-12 lg:col-span-5">
              <div
                className={`mb-3 flex items-center gap-2 rounded-lg border px-3 py-2 text-sm ${
                  report.internet ? "border-ok/30 bg-ok/5 text-ok" : "border-bad/30 bg-bad/5 text-bad"
                }`}
              >
                {report.internet ? <CircleCheck size={15} /> : <XCircle size={15} />}
                {report.internet
                  ? "Conectado a Internet"
                  : report.captivePortal
                    ? "Red con portal de acceso (hotel, aeropuerto…): abre el navegador para iniciar sesión"
                    : "Sin salida a Internet"}
              </div>
              {report.adapters.map((a) => (
                <div key={a.name} className="mb-2 rounded-lg border border-line bg-void/40 px-3 py-2 text-xs">
                  <div className="flex items-center gap-2 text-sm text-ink">
                    {a.kind === "Wi-Fi" ? <Wifi size={13} className="text-neon" /> : <Router size={13} className="text-neon" />}
                    {a.name}
                    <span className="text-[11px] text-mute">{a.linkSpeed}</span>
                    {a.signal !== null && <span className="ml-auto text-[11px] text-dim">Señal {a.signal}%</span>}
                  </div>
                  <div className="truncate text-mute">{a.description}</div>
                  <div className="mt-1 grid grid-cols-2 gap-x-3 font-mono text-[11px] text-dim">
                    {a.ssid && <span>SSID: {a.ssid}</span>}
                    <span>IP: {a.ipv4.join(", ") || "—"}</span>
                    <span>Router: {a.gateway.join(", ") || "—"}</span>
                    <span>DNS: {a.dns.join(", ") || "—"}</span>
                    <span>{a.dhcp === null ? "" : a.dhcp ? "DHCP" : "IP fija"}</span>
                  </div>
                </div>
              ))}
            </div>
            <div className="col-span-12 lg:col-span-7">
              <table className="w-full text-xs">
                <thead>
                  <tr className="text-left text-[10px] tracking-widest text-mute uppercase">
                    <th className="pb-2 font-medium">Destino</th>
                    <th className="pb-2 text-right font-medium">Respuestas</th>
                    <th className="pb-2 text-right font-medium">Media</th>
                    <th className="pb-2 text-right font-medium">Mín / Máx</th>
                  </tr>
                </thead>
                <tbody className="font-mono tabular">
                  {report.pings.map((p) => {
                    const lost = p.sent - p.received;
                    return (
                      <tr key={p.target} className="border-t border-line/60">
                        <td className="py-1.5 font-sans">
                          <div className="text-ink">{p.label}</div>
                          <div className="font-mono text-[10px] text-mute">{p.target}</div>
                        </td>
                        <td className={`py-1.5 text-right ${lost === 0 ? "text-ok" : lost === p.sent ? "text-bad" : "text-warn"}`}>
                          {p.received}/{p.sent}
                        </td>
                        <td className="py-1.5 text-right">{p.avgMs !== null ? `${p.avgMs.toFixed(0)} ms` : "—"}</td>
                        <td className="py-1.5 text-right text-dim">{p.minMs !== null ? `${p.minMs} / ${p.maxMs} ms` : "—"}</td>
                      </tr>
                    );
                  })}
                  {report.dns.map((d) => (
                    <tr key={d.host} className="border-t border-line/60">
                      <td className="py-1.5 font-sans">
                        <div className="text-ink">DNS · {d.host}</div>
                        <div className="truncate font-mono text-[10px] text-mute">{d.addresses.slice(0, 2).join(", ")}</div>
                      </td>
                      <td className={`py-1.5 text-right ${d.ok ? "text-ok" : "text-bad"}`}>{d.ok ? "Resuelve" : "Falla"}</td>
                      <td className="py-1.5 text-right">{d.ms.toFixed(0)} ms</td>
                      <td />
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="mt-2 text-[11px] text-mute">
                Router lento o con pérdidas → problema de la red local o del Wi-Fi. Router bien pero Internet mal → problema del proveedor.
              </p>
            </div>
          </div>
        )}
      </Card>
    </div>
  );
}
