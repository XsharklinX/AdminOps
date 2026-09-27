import { CircleCheck, Gauge, Loader2, RefreshCw, Router, Wifi, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { SpeedTest } from "../components/SpeedTest";
import { WifiProfiles } from "../components/WifiProfiles";
import { Card } from "../components/ui";
import { toolsApi, type NetworkReport, type SpeedResult } from "../lib/api";

const when = (ts: number) => new Date(ts * 1000).toLocaleString("es", { dateStyle: "short", timeStyle: "short" });

export function Network({ isAdmin }: { isAdmin: boolean }) {
  const [report, setReport] = useState<NetworkReport | null>(null);
  const [diagBusy, setDiagBusy] = useState(false);
  const [history, setHistory] = useState<SpeedResult[]>([]);
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
    toolsApi.speedHistory().then(setHistory);
  }, [diagnose]);

  const mainAdapter = report?.adapters.find((a) => a.gateway.length > 0 && a.kind !== "Virtual/VPN") ?? null;

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      {/* Speedtest */}
      <Card title="Test de velocidad" icon={<Gauge size={14} />} className="col-span-12">
        <SpeedTest adapter={mainAdapter} onResult={(r) => setHistory((h) => [r, ...h])} />
      </Card>

      {/* Historial */}
      <Card title="Historial de tests" className="col-span-12 lg:col-span-5 lg:order-last">
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
                <tr key={h.timestamp} className="border-t border-line/60">
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

      <WifiProfiles isAdmin={isAdmin} />
    </div>
  );
}
