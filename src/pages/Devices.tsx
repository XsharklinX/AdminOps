import {
  Building2,
  Camera,
  Check,
  Copy,
  Cpu,
  Eye,
  EyeOff,
  ExternalLink,
  Gamepad2,
  HardDrive,
  HelpCircle,
  Laptop,
  Monitor,
  MonitorSmartphone,
  Network,
  Pencil,
  Power,
  Printer,
  Radar,
  Router as RouterIcon,
  Save,
  Smartphone,
  Speaker,
  Tablet,
  TriangleAlert,
  Tv,
  Wifi,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, Modal } from "../components/ui";
import { lanApi, officeApi, wifiApi, workApi, type Client, type IpConflict, type LanDevice, type LanScan } from "../lib/api";

const ipNum = (ip: string) => ip.split(".").reduce((a, o) => a * 256 + Number(o), 0);

const KINDS: Record<string, { label: string; icon: LucideIcon }> = {
  router: { label: "Router", icon: RouterIcon },
  pc: { label: "Ordenador", icon: Monitor },
  laptop: { label: "Portátil", icon: Laptop },
  phone: { label: "Móvil", icon: Smartphone },
  tablet: { label: "Tablet", icon: Tablet },
  tv: { label: "Televisor", icon: Tv },
  streaming: { label: "Reproductor (Chromecast, Fire TV…)", icon: Tv },
  printer: { label: "Impresora", icon: Printer },
  camera: { label: "Cámara", icon: Camera },
  nas: { label: "NAS / almacenamiento", icon: HardDrive },
  console: { label: "Consola", icon: Gamepad2 },
  speaker: { label: "Altavoz", icon: Speaker },
  iot: { label: "Domótica / IoT", icon: Cpu },
  network: { label: "Equipo de red (repetidor, switch…)", icon: Network },
  unknown: { label: "Sin identificar", icon: HelpCircle },
};

const PORT_INFO: Record<number, string> = {
  22: "SSH (acceso remoto de administración)",
  23: "Telnet (acceso sin cifrar: inseguro)",
  80: "Página web (HTTP)",
  443: "Página web segura (HTTPS)",
  445: "Carpetas compartidas de Windows",
  515: "Impresión (LPD)",
  548: "Carpetas compartidas de Apple (AFP)",
  554: "Vídeo en directo (RTSP, cámaras)",
  631: "Impresión (IPP)",
  1883: "MQTT (domótica)",
  3389: "Escritorio remoto",
  5000: "Panel de NAS (Synology…)",
  5001: "Panel seguro de NAS",
  8008: "Google Cast",
  8009: "Google Cast",
  8080: "Página web alternativa",
  9100: "Impresión directa (RAW)",
  62078: "Sincronización de iPhone/iPad",
};

function kindKey(d: LanDevice): string {
  if (d.gateway) return "router";
  if (d.thisPc) return "pc";
  if (d.kind && KINDS[d.kind]) return d.kind;
  return d.privateMac ? "phone" : "unknown";
}

/** Nombre más útil que se tiene del dispositivo. */
const title = (d: LanDevice) =>
  d.alias || d.friendly || d.name || d.netbios || (d.gateway ? "Router" : d.thisPc ? "Este equipo" : [d.manufacturer, d.model].filter(Boolean).join(" ") || "Sin nombre");

/** Contraseña de la Wi-Fi de esta red: oculta hasta pulsar el ojo. */
function WifiCard() {
  const [ssid, setSsid] = useState<string | null>(null);
  const [password, setPassword] = useState<string | null>(null);
  const [note, setNote] = useState("");
  const [shown, setShown] = useState(false);
  const toast = useToast();

  useEffect(() => {
    (async () => {
      let listError = "";
      const [info, profiles] = await Promise.all([
        lanApi.info().catch(() => null),
        wifiApi.list().catch((e) => {
          listError = String(e);
          return [];
        }),
      ]);
      const connected = profiles.find((p) => p.connected);
      // Por cable: la Wi-Fi guardada con el nombre de esta red (suele ser la del mismo router).
      const byName = info ? profiles.find((p) => p.ssid === info.network || p.name === info.network) : undefined;
      const p = connected ?? byName;
      if (p) {
        setSsid(p.ssid);
        setPassword(p.password);
        setNote(connected ? "" : "Conectado por cable: es la Wi-Fi guardada con el nombre de esta red.");
      } else if (listError) {
        // Sin tarjeta Wi-Fi que funcione Windows no deja leer las redes guardadas.
        const st = await wifiApi.state().catch(() => null);
        setNote(
          st && st.level !== "ok"
            ? `${st.summary} Mientras no funcione, Windows no deja leer las redes guardadas ni su contraseña. Puedes arreglarla en Red → Estado de la Wi-Fi.`
            : listError,
        );
      } else {
        setNote(profiles.length ? "Este equipo no tiene guardada la Wi-Fi de esta red." : "Este equipo no tiene redes Wi-Fi guardadas.");
      }
    })();
  }, []);

  return (
    <Card title="Wi-Fi de esta red" icon={<Wifi size={14} />}>
      {ssid ? (
        <div className="flex flex-wrap items-center gap-x-6 gap-y-2 text-sm">
          <span>
            <span className="text-dim">Red: </span>
            <span className="text-ink">{ssid}</span>
          </span>
          <span className="flex items-center gap-2">
            <span className="text-dim">Contraseña:</span>
            {password ? (
              <>
                <span className="font-mono text-ink select-text">{shown ? password : "••••••••••"}</span>
                <button onClick={() => setShown(!shown)} className="text-mute hover:text-ink" title={shown ? "Ocultar" : "Mostrar"}>
                  {shown ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
                <button onClick={() => navigator.clipboard.writeText(password).then(() => toast("ok", "Contraseña copiada."))} className="text-mute hover:text-ink" title="Copiar">
                  <Copy size={14} />
                </button>
              </>
            ) : (
              <span className="text-mute">no disponible (red de empresa o sin permisos de administrador)</span>
            )}
          </span>
          {note && <span className="w-full text-xs text-mute">{note}</span>}
        </div>
      ) : (
        <p className="text-sm text-mute">{note || "Leyendo…"}</p>
      )}
    </Card>
  );
}

export function Devices() {
  const [scan, setScan] = useState<LanScan | null>(null);
  const [busy, setBusy] = useState<"scan" | "identify" | null>(null);
  const [vendorsBusy, setVendorsBusy] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [alias, setAlias] = useState("");
  const [conflicts, setConflicts] = useState<IpConflict[]>([]);
  const [clients, setClients] = useState<Client[]>([]);
  const [clientId, setClientId] = useState("");
  const [kindFilter, setKindFilter] = useState("all");
  const [detail, setDetail] = useState<LanDevice | null>(null);
  const toast = useToast();

  useEffect(() => {
    officeApi.conflicts().then(setConflicts).catch(() => {});
    workApi
      .clients()
      .then((c) => {
        setClients(c);
        setClientId((x) => x || c[0]?.id || "");
      })
      .catch(() => {});
  }, []);

  const run = async () => {
    setBusy("scan");
    try {
      const s = await lanApi.scan();
      setScan(s);
      // Segunda fase: qué es cada dispositivo (UPnP, mDNS, NetBIOS, puertos…).
      setBusy("identify");
      setScan({ ...s, devices: await lanApi.identify(s.key, s.devices) });
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const identifyVendors = async () => {
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
    try {
      await lanApi.setAlias(scan.key, d.mac || `ip-${d.ip}`, alias);
      setScan({ ...scan, devices: scan.devices.map((x) => (x.ip === d.ip ? { ...x, alias: alias.trim() } : x)) });
    } catch (e) {
      toast("error", String(e));
    }
    setEditing(null);
  };

  const saveMap = async () => {
    if (!scan || !clientId) return;
    try {
      const info = await lanApi.info();
      await workApi.saveNetworkMap(clientId, {
        name: info?.ssid ?? info?.network ?? "",
        gateway: info?.gateway ?? "",
        devices: scan.devices.map((d) => ({ ip: d.ip, mac: d.mac, name: title(d), vendor: [d.vendor || d.manufacturer, d.model].filter(Boolean).join(" "), alias: d.alias })),
      });
      toast("ok", `Mapa de la red guardado en la ficha de ${clients.find((c) => c.id === clientId)?.name ?? "el cliente"}.`);
    } catch (e) {
      toast("error", String(e));
    }
  };

  const all = useMemo(() => [...(scan?.devices ?? [])].sort((a, b) => Number(b.new) - Number(a.new) || ipNum(a.ip) - ipNum(b.ip)), [scan]);
  const kindCounts = useMemo(() => {
    const m = new Map<string, number>();
    for (const d of all) m.set(kindKey(d), (m.get(kindKey(d)) ?? 0) + 1);
    return m;
  }, [all]);
  const devices = all.filter((d) => kindFilter === "all" || kindKey(d) === kindFilter);
  const fresh = all.filter((d) => d.new).length;
  const unknownVendors = all.some((d) => d.mac && !d.privateMac && !d.vendor);
  const conflictName = (mac: string) => {
    const d = all.find((x) => x.mac === mac.replace(/-/g, ":").toLowerCase());
    return d ? ` (${title(d)})` : "";
  };
  const shown = detail ? (all.find((d) => d.ip === detail.ip) ?? detail) : null;

  const actions = (d: LanDevice, always = false) => (
    <span className={`inline-flex gap-2 ${always ? "" : "opacity-0 group-hover:opacity-100"}`}>
      <button onClick={() => navigator.clipboard.writeText(d.ip).then(() => toast("ok", "IP copiada."))} className="text-mute hover:text-ink" title="Copiar IP">
        <Copy size={13} />
      </button>
      {!d.thisPc && (
        <button onClick={() => lanApi.openDevice(d.ip).catch((e) => toast("error", String(e)))} className="text-mute hover:text-ink" title="Abrir su página web (impresoras, cámaras, routers…)">
          <ExternalLink size={13} />
        </button>
      )}
      {!d.thisPc && !d.gateway && (
        <button onClick={() => officeApi.rdp(d.ip).catch((e) => toast("error", String(e)))} className="text-mute hover:text-ink" title="Conectar por Escritorio remoto">
          <MonitorSmartphone size={13} />
        </button>
      )}
      {!d.thisPc && !d.gateway && d.mac && !d.privateMac && (
        <button
          onClick={() =>
            officeApi
              .wake(d.mac)
              .then(() => toast("ok", "Señal de encendido enviada."))
              .catch((e) => toast("error", String(e)))
          }
          className="text-mute hover:text-ink"
          title="Encender por la red (Wake-on-LAN)"
        >
          <Power size={13} />
        </button>
      )}
    </span>
  );

  return (
    <div className="mx-auto max-w-6xl space-y-4 p-6">
      <WifiCard />

      <Card title="Dispositivos conectados a la red" icon={<Radar size={14} />}>
        <div className="flex flex-wrap items-center gap-3">
          <p className="min-w-0 flex-1 text-sm text-dim">
            Busca todo lo que hay en la red y lo identifica: tipo, fabricante, modelo y qué servicios ofrece. Los nuevos desde la última búsqueda se
            marcan para detectar intrusos.
          </p>
          {scan && unknownVendors && (
            <Button kind="ghost" onClick={identifyVendors} disabled={vendorsBusy || busy !== null} title="Consulta en Internet solo el prefijo del fabricante de cada MAC">
              <Building2 size={14} /> {vendorsBusy ? "Identificando…" : "Fabricantes por MAC"}
            </Button>
          )}
          <Button onClick={run} disabled={busy !== null}>
            <Radar size={14} /> {scan ? "Volver a buscar" : "Buscar dispositivos"}
          </Button>
        </div>
        <TaskStatus task={busy === "identify" ? "lan-identify" : "lan-scan"} active={busy !== null} fallback={busy === "identify" ? "Identificando…" : "Buscando…"} className="mt-3" />
      </Card>

      {conflicts.length > 0 && (
        <div className="rounded-xl border border-warn/30 bg-warn/5 px-4 py-3">
          <div className="flex items-center gap-2 text-sm font-medium text-ink">
            <TriangleAlert size={15} className="text-warn" /> IP duplicadas en los últimos 30 días
          </div>
          <p className="mt-1 text-xs text-dim">
            Windows detectó otro dispositivo usando la misma IP que este equipo. Provoca cortes de red intermitentes. Solución: dejar que el router asigne
            las IP (DHCP) o reservar una IP fija para cada equipo en el router.
          </p>
          <ul className="mt-2 space-y-0.5">
            {conflicts.slice(0, 5).map((c, i) => (
              <li key={i} className="font-mono text-xs text-ink">
                {new Date(c.time).toLocaleString("es", { dateStyle: "short", timeStyle: "short" })} · IP {c.ip}
                {c.mac && ` · el otro dispositivo: ${c.mac.replace(/-/g, ":").toLowerCase()}${conflictName(c.mac)}`}
              </li>
            ))}
          </ul>
        </div>
      )}

      {scan && (
        <Card title={`${all.length} dispositivos${fresh ? ` · ${fresh} nuevos` : ""}`}>
          {scan.truncated && <p className="mb-3 text-xs text-warn">La red es grande: se revisó solo el bloque de 254 direcciones de este equipo.</p>}
          {kindCounts.size > 1 && (
            <div className="mb-3 flex flex-wrap gap-1.5">
              <button onClick={() => setKindFilter("all")} className={`rounded-full px-2.5 py-0.5 text-xs ${kindFilter === "all" ? "bg-panel-2 text-ink" : "text-dim hover:text-ink"}`}>
                Todos {all.length}
              </button>
              {[...kindCounts.entries()].map(([k, n]) => (
                <button key={k} onClick={() => setKindFilter(k)} className={`rounded-full px-2.5 py-0.5 text-xs ${kindFilter === k ? "bg-panel-2 text-ink" : "text-dim hover:text-ink"}`}>
                  {KINDS[k].label.split(" (")[0]} {n}
                </button>
              ))}
            </div>
          )}
          <table className="w-full text-[13px]">
            <thead>
              <tr className="text-left text-xs text-mute">
                <th className="w-8 pb-2" />
                <th className="pb-2 font-medium">Dispositivo</th>
                <th className="pb-2 font-medium">IP</th>
                <th className="pb-2 font-medium">MAC</th>
                <th className="pb-2 font-medium">Fabricante</th>
                <th className="pb-2 text-right font-medium">Ping</th>
                <th className="w-24 pb-2" />
              </tr>
            </thead>
            <tbody>
              {devices.map((d) => {
                const key = kindKey(d);
                const Icon = KINDS[key].icon;
                const sub = [d.manufacturer && d.manufacturer !== d.vendor ? d.manufacturer : "", d.model, key === "unknown" ? "" : KINDS[key].label.split(" (")[0], d.os].filter(Boolean).join(" · ");
                return (
                  <tr key={d.ip} className="group cursor-pointer border-t border-line/60 hover:bg-panel-2/40" onClick={() => editing !== d.ip && setDetail(d)}>
                    <td className="py-2 text-mute" title={KINDS[key].label}>
                      <Icon size={15} />
                    </td>
                    <td className="py-2 pr-3" onClick={(e) => editing === d.ip && e.stopPropagation()}>
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
                        <>
                          <div className="flex items-center gap-2">
                            <span className="text-ink">{title(d)}</span>
                            {d.gateway && <span className="rounded bg-panel-2 px-1.5 text-[11px] text-dim">router</span>}
                            {d.thisPc && <span className="rounded bg-panel-2 px-1.5 text-[11px] text-dim">este equipo</span>}
                            {d.new && <span className="rounded bg-warn/15 px-1.5 text-[11px] text-warn">nuevo</span>}
                            {!d.thisPc && (
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
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
                          {sub && <div className="truncate text-[11px] text-mute">{sub}</div>}
                        </>
                      )}
                    </td>
                    <td className="py-2 pr-3 font-mono text-xs text-ink">{d.ip}</td>
                    <td className="py-2 pr-3 font-mono text-[11px] text-dim">{d.mac || "—"}</td>
                    <td className="py-2 pr-3 text-xs text-dim">
                      {d.privateMac ? <span className="text-mute" title="Los móviles modernos usan una MAC aleatoria por red">MAC privada (móvil)</span> : d.vendor || d.manufacturer || "—"}
                    </td>
                    <td className="py-2 text-right font-mono text-xs text-dim">{d.ms === null ? <span className="text-mute" title="No responde al ping, pero está en la red">—</span> : `${d.ms} ms`}</td>
                    <td className="py-2 text-right" onClick={(e) => e.stopPropagation()}>
                      {actions(d)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <p className="mt-3 text-[11px] text-mute">
            Haz clic en un dispositivo para ver todo lo que se sabe de él. Los móviles y algunos equipos no responden al ping pero aparecen igualmente. Ponle
            nombre a los tuyos para reconocer rápido uno desconocido.
          </p>
          {clients.length > 0 && (
            <div className="mt-3 flex flex-wrap items-center justify-end gap-2 border-t border-line/60 pt-3">
              <span className="text-xs text-dim">Guardar esta lista en la ficha de</span>
              <select value={clientId} onChange={(e) => setClientId(e.target.value)} className="rounded-md border border-line bg-void/60 px-2 py-1.5 text-sm text-ink outline-none">
                {clients.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
              <Button kind="ghost" onClick={saveMap}>
                <Save size={14} /> Guardar mapa de la red
              </Button>
            </div>
          )}
        </Card>
      )}

      {shown && (
        <Modal title={title(shown)} onClose={() => setDetail(null)} width="w-[560px]">
          <DeviceDetail
            d={shown}
            actions={actions(shown, true)}
            onRename={() => {
              setDetail(null);
              setEditing(shown.ip);
              setAlias(shown.alias);
            }}
          />
        </Modal>
      )}
    </div>
  );
}

function DeviceDetail({ d, actions, onRename }: { d: LanDevice; actions: React.ReactNode; onRename: () => void }) {
  const K = KINDS[kindKey(d)];
  const Icon = K.icon;
  const uniq = (...xs: string[]) => xs.filter((v, i, a) => v && a.indexOf(v) === i).join(" · ");
  const rows: [string, string][] = [
    ["IP", d.ip],
    ["MAC", d.mac ? `${d.mac}${d.privateMac ? " (privada/aleatoria)" : ""}` : ""],
    ["Fabricante", uniq(d.manufacturer, d.vendor)],
    ["Modelo", d.model],
    ["Nombre que anuncia", d.friendly],
    ["Nombre en la red", uniq(d.name, d.netbios)],
    ["Sistema (aprox.)", d.os],
    ["Respuesta", d.ms === null ? "No responde al ping" : `${d.ms} ms`],
    ["Visto por primera vez", d.firstSeen ? new Date(d.firstSeen * 1000).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" }) : ""],
  ];
  return (
    <div className="space-y-3 text-sm">
      <div className="flex items-center gap-2 text-dim">
        <Icon size={16} /> {K.label}
      </div>
      <dl className="grid grid-cols-[150px_1fr] gap-x-3 gap-y-1.5">
        {rows
          .filter(([, v]) => v)
          .map(([k, v]) => (
            <div key={k} className="contents">
              <dt className="text-dim">{k}</dt>
              <dd className="text-ink select-text">{v}</dd>
            </div>
          ))}
      </dl>
      {d.services.length > 0 && (
        <div>
          <div className="mb-1 text-xs text-dim">Servicios que anuncia</div>
          <div className="flex flex-wrap gap-1.5">
            {d.services.map((s) => (
              <span key={s} className="rounded bg-panel-2 px-2 py-0.5 text-xs text-ink">
                {s}
              </span>
            ))}
          </div>
        </div>
      )}
      {d.ports.length > 0 && (
        <div>
          <div className="mb-1 text-xs text-dim">Puertos abiertos</div>
          <ul className="space-y-0.5 text-xs">
            {d.ports.map((p) => (
              <li key={p} className={p === 23 ? "text-bad" : "text-ink"}>
                <span className="inline-block w-14 font-mono">{p}</span>
                {PORT_INFO[p] ?? ""}
              </li>
            ))}
          </ul>
        </div>
      )}
      {!d.kind && !d.thisPc && <p className="text-xs text-mute">Aún sin identificar a fondo: vuelve a buscar para completarlo.</p>}
      <div className="flex items-center justify-between border-t border-line/60 pt-3">
        {actions}
        {!d.thisPc && (
          <Button kind="ghost" onClick={onRename}>
            <Pencil size={13} /> Ponerle nombre
          </Button>
        )}
      </div>
    </div>
  );
}
