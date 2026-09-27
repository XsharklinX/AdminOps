import { Building2, Check, Copy, ExternalLink, Laptop, Pencil, Radar, Router as RouterIcon, Smartphone } from "lucide-react";
import { useMemo, useState } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card } from "../components/ui";
import { lanApi, type LanDevice, type LanScan } from "../lib/api";

const ipNum = (ip: string) => ip.split(".").reduce((a, o) => a * 256 + Number(o), 0);

export function Devices() {
  const [scan, setScan] = useState<LanScan | null>(null);
  const [busy, setBusy] = useState(false);
  const [vendorsBusy, setVendorsBusy] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [alias, setAlias] = useState("");
  const toast = useToast();

  const run = async () => {
    setBusy(true);
    try {
      setScan(await lanApi.scan());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const identify = async () => {
    if (!scan) return;
    setVendorsBusy(true);
    try {
      const macs = scan.devices.filter((d) => d.mac && !d.privateMac && !d.vendor).map((d) => d.mac);
      const found = await lanApi.vendors(scan.key, macs);
      setScan({ ...scan, devices: scan.devices.map((d) => (found[d.mac] ? { ...d, vendor: found[d.mac] } : d)) });
      toast("ok", `${Object.keys(found).length} fabricantes identificados.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setVendorsBusy(false);
    }
  };

  const saveAlias = async (d: LanDevice) => {
    if (!scan) return;
    const id = d.mac || `ip-${d.ip}`;
    try {
      await lanApi.setAlias(scan.key, id, alias);
      setScan({ ...scan, devices: scan.devices.map((x) => (x === d ? { ...x, alias: alias.trim() } : x)) });
    } catch (e) {
      toast("error", String(e));
    }
    setEditing(null);
  };

  const devices = useMemo(() => [...(scan?.devices ?? [])].sort((a, b) => Number(b.new) - Number(a.new) || ipNum(a.ip) - ipNum(b.ip)), [scan]);
  const fresh = devices.filter((d) => d.new).length;
  const unknownVendors = devices.some((d) => d.mac && !d.privateMac && !d.vendor);

  const icon = (d: LanDevice) =>
    d.gateway ? <RouterIcon size={15} /> : d.thisPc ? <Laptop size={15} /> : d.privateMac ? <Smartphone size={15} /> : <Building2 size={15} />;

  return (
    <div className="mx-auto max-w-6xl space-y-4 p-6">
      <Card title="Dispositivos conectados a la red" icon={<Radar size={14} />}>
        <div className="flex flex-wrap items-center gap-3">
          <p className="min-w-0 flex-1 text-sm text-dim">
            Busca todo lo que hay en la red local: ordenadores, móviles, impresoras, cámaras, televisores… Los nuevos desde la última búsqueda se marcan
            para detectar intrusos.
          </p>
          {scan && unknownVendors && (
            <Button kind="ghost" onClick={identify} disabled={vendorsBusy} title="Consulta en Internet solo el prefijo del fabricante de cada MAC">
              <Building2 size={14} /> {vendorsBusy ? "Identificando…" : "Identificar fabricantes"}
            </Button>
          )}
          <Button onClick={run} disabled={busy}>
            <Radar size={14} /> {scan ? "Volver a buscar" : "Buscar dispositivos"}
          </Button>
        </div>
        <TaskStatus task="lan-scan" active={busy} fallback="Buscando…" className="mt-3" />
      </Card>

      {scan && (
        <Card title={`${devices.length} dispositivos${fresh ? ` · ${fresh} nuevos` : ""}`}>
          {scan.truncated && <p className="mb-3 text-xs text-warn">La red es grande: se revisó solo el bloque de 254 direcciones de este equipo.</p>}
          <table className="w-full text-[13px]">
            <thead>
              <tr className="text-left text-xs text-mute">
                <th className="w-8 pb-2" />
                <th className="pb-2 font-medium">Dispositivo</th>
                <th className="pb-2 font-medium">IP</th>
                <th className="pb-2 font-medium">MAC</th>
                <th className="pb-2 font-medium">Fabricante</th>
                <th className="pb-2 text-right font-medium">Ping</th>
                <th className="w-16 pb-2" />
              </tr>
            </thead>
            <tbody>
              {devices.map((d) => (
                <tr key={d.ip} className="group border-t border-line/60">
                  <td className="py-2 text-mute">{icon(d)}</td>
                  <td className="py-2 pr-3">
                    {editing === d.ip ? (
                      <form
                        className="flex gap-1"
                        onSubmit={(e) => {
                          e.preventDefault();
                          saveAlias(d);
                        }}
                      >
                        <input autoFocus value={alias} onChange={(e) => setAlias(e.target.value)} placeholder="Nombre para reconocerlo" className="w-44 rounded border border-line bg-void px-2 py-0.5 text-[13px] text-ink outline-none focus:border-neon/50" />
                        <button type="submit" className="text-ok" title="Guardar">
                          <Check size={14} />
                        </button>
                      </form>
                    ) : (
                      <div className="flex items-center gap-2">
                        <span className="text-ink">{d.alias || d.name || (d.gateway ? "Router" : d.thisPc ? "Este equipo" : "Sin nombre")}</span>
                        {d.alias && d.name && <span className="truncate text-xs text-mute">{d.name}</span>}
                        {d.gateway && <span className="rounded bg-panel-2 px-1.5 text-[11px] text-dim">router</span>}
                        {d.thisPc && <span className="rounded bg-panel-2 px-1.5 text-[11px] text-dim">este equipo</span>}
                        {d.new && <span className="rounded bg-warn/15 px-1.5 text-[11px] text-warn">nuevo</span>}
                        {!d.thisPc && (
                          <button
                            onClick={() => {
                              setEditing(d.ip);
                              setAlias(d.alias);
                            }}
                            className="text-mute opacity-0 group-hover:opacity-100 hover:text-ink"
                            title="Ponerle un nombre"
                          >
                            <Pencil size={12} />
                          </button>
                        )}
                      </div>
                    )}
                  </td>
                  <td className="py-2 pr-3 font-mono text-xs text-ink">{d.ip}</td>
                  <td className="py-2 pr-3 font-mono text-[11px] text-dim">{d.mac || "—"}</td>
                  <td className="py-2 pr-3 text-xs text-dim">{d.privateMac ? <span className="text-mute" title="Los móviles modernos usan una MAC aleatoria por red">MAC privada (móvil)</span> : d.vendor || "—"}</td>
                  <td className="py-2 text-right font-mono text-xs text-dim">{d.ms === null ? <span className="text-mute" title="No responde al ping, pero está en la red">—</span> : `${d.ms} ms`}</td>
                  <td className="py-2 text-right">
                    <span className="inline-flex gap-2 opacity-0 group-hover:opacity-100">
                      <button onClick={() => navigator.clipboard.writeText(d.ip).then(() => toast("ok", "IP copiada."))} className="text-mute hover:text-ink" title="Copiar IP">
                        <Copy size={13} />
                      </button>
                      {!d.thisPc && (
                        <button onClick={() => lanApi.openDevice(d.ip).catch((e) => toast("error", String(e)))} className="text-mute hover:text-ink" title="Abrir su página web (impresoras, cámaras, routers…)">
                          <ExternalLink size={13} />
                        </button>
                      )}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="mt-3 text-[11px] text-mute">
            Los móviles y algunos equipos no responden al ping pero aparecen igualmente. Ponle nombre a los tuyos para reconocer rápido un dispositivo desconocido.
          </p>
        </Card>
      )}
    </div>
  );
}
