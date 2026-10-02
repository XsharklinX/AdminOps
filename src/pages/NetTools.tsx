import { listen } from "@tauri-apps/api/event";
import { CircleCheck, FileText, Loader2, Play, RefreshCw, Route, Save, Search, Server, Square, TriangleAlert, Undo2, Waypoints } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, inputClass, Loading } from "../components/ui";
import { netApi, type DnsAdapter, type PortEntry, type Probe } from "../lib/api";
import { DataTable } from "../components/DataTable";

type Tab = "probe" | "dns" | "ports" | "hosts";

const TABS: { id: Tab; label: string; icon: React.ComponentType<{ size?: number }> }[] = [
  { id: "probe", label: "Ping y traza de ruta", icon: Route },
  { id: "dns", label: "DNS", icon: Server },
  { id: "ports", label: "Puertos en uso", icon: Waypoints },
  { id: "hosts", label: "Archivo hosts", icon: FileText },
];

export function NetTools({ isAdmin }: { isAdmin: boolean }) {
  const [tab, setTab] = useState<Tab>("probe");
  return (
    <div className="mx-auto max-w-6xl p-6">
      <div className="mb-4 flex gap-1 rounded-lg border border-line bg-panel p-1">
        {TABS.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={`flex flex-1 items-center justify-center gap-2 rounded-md py-2 text-sm transition-colors ${
              tab === id ? "bg-neon/10 text-neon" : "text-dim hover:text-ink"
            }`}
          >
            <Icon size={14} /> {label}
          </button>
        ))}
      </div>
      {tab === "probe" && <ProbePanel />}
      {tab === "dns" && <DnsPanel isAdmin={isAdmin} />}
      {tab === "ports" && <PortsPanel />}
      {tab === "hosts" && <HostsPanel isAdmin={isAdmin} />}
    </div>
  );
}

// ---------- Ping y traza ----------

function ProbePanel() {
  const [host, setHost] = useState("8.8.8.8");
  const [mode, setMode] = useState<"ping" | "trace">("ping");
  const [running, setRunning] = useState(false);
  const [target, setTarget] = useState<{ target: string; ip: string } | null>(null);
  const [rows, setRows] = useState<Probe[]>([]);
  const [error, setError] = useState<string | null>(null);
  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const subs = [
      listen<Probe>("net-ping", (e) => setRows((r) => [...r, e.payload])),
      listen<Probe>("net-trace", (e) => setRows((r) => [...r, e.payload])),
      listen("net-ping-done", () => setRunning(false)),
      listen("net-trace-done", () => setRunning(false)),
    ];
    return () => {
      void netApi.stop();
      subs.forEach((p) => p.then((un) => un()));
    };
  }, []);

  // Con llaves: en Chromium reciente scrollIntoView devuelve una Promise, y React
  // trataría lo que devuelve el efecto como función de limpieza (pantalla en negro).
  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "nearest" });
  }, [rows.length]);

  const start = async () => {
    setError(null);
    setRows([]);
    try {
      const t = mode === "ping" ? await netApi.ping(host, 100) : await netApi.trace(host);
      setTarget(t);
      setRunning(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const stats = useMemo(() => {
    const ok = rows.filter((r) => r.ms !== null).map((r) => r.ms!);
    if (!rows.length) return null;
    return {
      sent: rows.length,
      lost: rows.length - ok.length,
      avg: ok.length ? ok.reduce((a, b) => a + b, 0) / ok.length : null,
      min: ok.length ? Math.min(...ok) : null,
      max: ok.length ? Math.max(...ok) : null,
    };
  }, [rows]);

  return (
    <Card>
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <div className="flex rounded-md border border-line p-0.5">
          {(["ping", "trace"] as const).map((m) => (
            <button
              key={m}
              onClick={() => setMode(m)}
              disabled={running}
              className={`rounded px-3 py-1 text-sm ${mode === m ? "bg-neon/10 text-neon" : "text-dim hover:text-ink"}`}
            >
              {m === "ping" ? "Ping" : "Traza de ruta"}
            </button>
          ))}
        </div>
        <input
          value={host}
          onChange={(e) => setHost(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !running && start()}
          placeholder="IP o nombre (google.com)"
          className={`${inputClass} w-72 font-mono`}
          disabled={running}
        />
        {["8.8.8.8", "1.1.1.1", "google.com"].map((h) => (
          <button key={h} onClick={() => setHost(h)} disabled={running} className="rounded border border-line px-2 py-1 font-mono text-[11px] text-mute hover:text-ink">
            {h}
          </button>
        ))}
        <div className="ml-auto">
          {running ? (
            <Button kind="danger" onClick={() => netApi.stop()}>
              <Square size={13} /> Detener
            </Button>
          ) : (
            <Button onClick={start} disabled={!host.trim()}>
              <Play size={13} /> Iniciar
            </Button>
          )}
        </div>
      </div>
      {error && <p className="mb-3 text-sm text-bad">{error}</p>}
      {target && (
        <p className="mb-2 text-xs text-mute">
          {mode === "ping" ? "Ping a" : "Ruta hasta"} <span className="text-ink">{target.target}</span>
          {target.ip !== target.target && <span className="font-mono"> ({target.ip})</span>}
          {running && <Loader2 size={11} className="ml-2 inline animate-spin text-neon" />}
        </p>
      )}
      {mode === "ping" && stats && (
        <div className="mb-3 grid grid-cols-5 gap-3 rounded-lg border border-line bg-void/40 px-4 py-2 text-center">
          {[
            ["Enviados", stats.sent],
            ["Perdidos", `${stats.lost} (${Math.round((stats.lost / stats.sent) * 100)}%)`],
            ["Media", stats.avg !== null ? `${stats.avg.toFixed(0)} ms` : "—"],
            ["Mínimo", stats.min !== null ? `${stats.min} ms` : "—"],
            ["Máximo", stats.max !== null ? `${stats.max} ms` : "—"],
          ].map(([l, v]) => (
            <div key={l}>
              <div className="text-[11px] text-mute">{l}</div>
              <div className={`font-mono text-sm ${l === "Perdidos" && stats.lost ? "text-warn" : "text-ink"}`}>{v}</div>
            </div>
          ))}
        </div>
      )}
      <div className="max-pane-lg overflow-y-auto rounded-lg border border-line">
        <DataTable
          padded
          sticky
          mono
          size="xs"
          rows={rows}
          rowKey={(r) => r.seq}
          empty={mode === "ping" ? "Envía un ping por segundo (hasta 100) para ver cortes y latencia en vivo." : "Muestra cada router por el que pasa la conexión hasta el destino (máx. 30 saltos)."}
          columns={[
            { id: "seq", header: mode === "ping" ? "#" : "Salto", sortBy: (r) => r.seq, cell: (r) => r.seq, className: "text-mute" },
            {
              id: "from",
              header: "Responde",
              cell: (r) =>
                r.from ? (
                  <span className={r.reached && mode === "trace" ? "text-ok" : "text-ink"}>{r.from}</span>
                ) : (
                  <span className="text-mute">{r.status === "timeout" ? "sin respuesta" : r.status === "unreachable" ? "inalcanzable" : "error"}</span>
                ),
            },
            { id: "ms", header: "Tiempo", align: "right", sortBy: (r) => r.ms, cell: (r) => (r.ms === null ? "*" : `${r.ms} ms`), className: (r) => (r.ms === null ? "text-mute" : r.ms > 150 ? "text-warn" : "text-neon") },
          ]}
        />
        <div ref={bottom} />
      </div>
      {mode === "trace" && (
        <p className="mt-2 text-[11px] text-mute">
          «Sin respuesta» en un salto intermedio es normal (muchos routers no contestan). Solo preocupa si a partir de un salto ya no responde nadie.
        </p>
      )}
    </Card>
  );
}

// ---------- DNS ----------

const DNS_PRESETS: { id: string; name: string; servers: string[]; note: string }[] = [
  { id: "auto", name: "Automáticos", servers: [], note: "Los que da el router (DHCP)" },
  { id: "cloudflare", name: "Cloudflare", servers: ["1.1.1.1", "1.0.0.1"], note: "Rápidos y privados" },
  { id: "google", name: "Google", servers: ["8.8.8.8", "8.8.4.4"], note: "Muy fiables" },
  { id: "quad9", name: "Quad9", servers: ["9.9.9.9", "149.112.112.112"], note: "Bloquean webs maliciosas" },
  { id: "family", name: "Cloudflare familia", servers: ["1.1.1.3", "1.0.0.3"], note: "Bloquean malware y adultos" },
];

function DnsPanel({ isAdmin }: { isAdmin: boolean }) {
  const [adapters, setAdapters] = useState<DnsAdapter[] | null>(null);
  const [showVirtual, setShowVirtual] = useState(false);
  const [busy, setBusy] = useState<number | null>(null);
  const [custom, setCustom] = useState<Record<number, string>>({});
  const toast = useToast();

  const load = useCallback(() => netApi.dnsAdapters().then(setAdapters).catch((e) => toast("error", String(e))), [toast]);
  useEffect(() => {
    void load();
  }, [load]);

  const apply = async (a: DnsAdapter, servers: string[], label: string) => {
    setBusy(a.index);
    try {
      await netApi.setDns(a.index, servers);
      toast("ok", `${a.name}: DNS ${label}.`);
      await load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  if (!adapters) return <Loading text="Leyendo adaptadores…" />;
  const list = adapters.filter((a) => showVirtual || !a.virtual);

  return (
    <div className="space-y-3">
      {!isAdmin && (
        <p className="flex items-center gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn">
          <TriangleAlert size={13} /> Cambiar los DNS requiere ejecutar AdminOps como administrador.
        </p>
      )}
      {list.map((a) => {
        const current = DNS_PRESETS.find((p) => p.servers.length && p.servers.join() === a.dns.slice(0, 2).join());
        return (
          <Card key={a.index}>
            <div className="mb-3 flex items-start justify-between gap-4">
              <div className="min-w-0">
                <div className="text-sm font-medium text-ink">{a.name}</div>
                <div className="truncate text-xs text-mute">{a.description}</div>
              </div>
              <div className="text-right">
                <div className="font-mono text-sm text-neon">{a.dns.length ? a.dns.join(", ") : "—"}</div>
                <div className="text-[11px] text-mute">
                  {a.manual ? (current ? `Manual · ${current.name}` : "Manual") : "Automáticos (router)"}
                </div>
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {DNS_PRESETS.map((p) => {
                const active = p.id === "auto" ? !a.manual : a.manual && current?.id === p.id;
                return (
                  <button
                    key={p.id}
                    onClick={() => apply(a, p.servers, p.id === "auto" ? "automáticos" : `de ${p.name}`)}
                    disabled={!isAdmin || busy !== null || active}
                    title={`${p.note}${p.servers.length ? ` · ${p.servers.join(", ")}` : ""}`}
                    className={`rounded-md border px-3 py-1.5 text-xs transition-colors disabled:cursor-default ${
                      active ? "border-neon/60 bg-neon/10 text-neon" : "border-line text-dim hover:border-line-2 hover:text-ink disabled:opacity-40"
                    }`}
                  >
                    {p.name}
                  </button>
                );
              })}
              <input
                value={custom[a.index] ?? ""}
                onChange={(e) => setCustom({ ...custom, [a.index]: e.target.value })}
                placeholder="Otros: 192.168.1.10, 1.1.1.1"
                className={`${inputClass} w-56 py-1.5 font-mono text-xs`}
                disabled={!isAdmin}
              />
              <Button
                kind="ghost"
                disabled={!isAdmin || busy !== null || !(custom[a.index] ?? "").trim()}
                onClick={() =>
                  apply(
                    a,
                    custom[a.index].split(/[\s,;]+/).filter(Boolean),
                    "personalizados",
                  )
                }
              >
                Aplicar
              </Button>
              {busy === a.index && <Loader2 size={14} className="animate-spin text-neon" />}
            </div>
          </Card>
        );
      })}
      <label className="flex items-center gap-1.5 text-xs text-dim">
        <input type="checkbox" checked={showVirtual} onChange={(e) => setShowVirtual(e.target.checked)} className="accent-[var(--color-neon)]" />
        Mostrar adaptadores virtuales y VPN ({adapters.filter((a) => a.virtual).length})
      </label>
      <p className="text-[11px] text-mute">
        Cambia los DNS IPv4 del adaptador y vacía la caché DNS. El valor anterior queda anotado en el Historial para volver a él.
      </p>
    </div>
  );
}

// ---------- Puertos ----------

const WELL_KNOWN: Record<number, string> = {
  20: "FTP datos", 21: "FTP", 22: "SSH", 23: "Telnet", 25: "SMTP", 53: "DNS", 67: "DHCP", 68: "DHCP", 80: "HTTP", 110: "POP3", 123: "Hora (NTP)",
  135: "RPC de Windows", 137: "NetBIOS", 138: "NetBIOS", 139: "NetBIOS", 143: "IMAP", 443: "HTTPS", 445: "Archivos compartidos (SMB)",
  1900: "UPnP/SSDP", 3306: "MySQL", 3389: "Escritorio remoto", 5040: "Servicio de Windows", 5353: "mDNS", 5355: "LLMNR",
  5432: "PostgreSQL", 7680: "Optimización de distribución", 8080: "HTTP alternativo",
};

function PortsPanel() {
  const [ports, setPorts] = useState<PortEntry[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState("");
  const [onlyListen, setOnlyListen] = useState(true);
  const toast = useToast();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setPorts(await netApi.ports());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);
  useEffect(() => {
    void load();
  }, [load]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (ports ?? []).filter(
      (p) =>
        (!onlyListen || p.state === "Listen") &&
        (!q || [String(p.localPort), p.process ?? "", String(p.pid), p.remoteAddress ?? "", WELL_KNOWN[p.localPort] ?? ""].some((s) => s.toLowerCase().includes(q))),
    );
  }, [ports, query, onlyListen]);

  return (
    <Card>
      <div className="mb-3 flex items-center gap-3">
        <div className="relative w-72">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Puerto, programa o PID…"
            className="w-full rounded-md border border-line bg-void/60 py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <label className="flex items-center gap-1.5 text-xs text-dim">
          <input type="checkbox" checked={onlyListen} onChange={(e) => setOnlyListen(e.target.checked)} className="accent-[var(--color-neon)]" />
          Solo puertos abiertos (escuchando)
        </label>
        <span className="ml-auto text-xs text-mute">{visible.length} entradas</span>
        <button onClick={load} disabled={loading} className="rounded-md p-1.5 text-dim hover:bg-panel-2 hover:text-ink" title="Volver a leer">
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
      </div>
      {!ports ? (
        <Loading text="Leyendo conexiones…" />
      ) : (
        <div className="max-pane-lg overflow-y-auto rounded-lg border border-line">
          <DataTable
            padded
            sticky
            mono
            size="xs"
            rows={visible}
            rowKey={(p, i) => `${p.protocol}-${p.localAddress}-${p.localPort}-${p.remoteAddress}-${p.remotePort}-${i}`}
            columns={[
              { id: "proto", header: "Proto", sortBy: (p) => p.protocol, cell: (p) => p.protocol, className: "text-mute" },
              {
                id: "local",
                header: "Local",
                sortBy: (p) => p.localPort,
                className: "text-ink",
                cell: (p) => (
                  <>
                    {p.localAddress}:<span className="text-neon">{p.localPort}</span>
                  </>
                ),
              },
              { id: "service", header: "Servicio", sortBy: (p) => WELL_KNOWN[p.localPort] ?? "", cell: (p) => WELL_KNOWN[p.localPort] ?? "", className: "font-sans text-dim" },
              {
                id: "remote",
                header: onlyListen ? "Estado" : "Remoto",
                sortBy: (p) => (p.state === "Listen" ? "" : `${p.remoteAddress}:${p.remotePort}`),
                className: "text-dim",
                cell: (p) => (p.state === "Listen" ? <span className="font-sans text-ok">Escuchando</span> : `${p.remoteAddress}:${p.remotePort}`),
              },
              {
                id: "program",
                header: "Programa",
                sortBy: (p) => p.process ?? "",
                cell: (p) => (
                  <>
                    <span className="font-sans text-ink">{p.process ?? "—"}</span> <span className="text-mute">{p.pid}</span>
                  </>
                ),
              },
            ]}
          />
        </div>
      )}
      <p className="mt-2 text-[11px] text-mute">Para cerrar un programa que ocupa un puerto, búscalo por su PID en Procesos.</p>
    </Card>
  );
}

// ---------- Hosts ----------

const DEFAULT_HOSTS = `# Copyright (c) 1993-2009 Microsoft Corp.
#
# This is a sample HOSTS file used by Microsoft TCP/IP for Windows.
#
# This file contains the mappings of IP addresses to host names. Each
# entry should be kept on an individual line. The IP address should
# be placed in the first column followed by the corresponding host name.
# The IP address and the host name should be separated by at least one
# space.
#
# Additionally, comments (such as these) may be inserted on individual
# lines or following the machine name denoted by a '#' symbol.
#
# For example:
#
#      102.54.94.97     rhino.acme.com          # source server
#       38.25.63.10     x.acme.com              # x client host

# localhost name resolution is handled within DNS itself.
#	127.0.0.1       localhost
#	::1             localhost
`;

function HostsPanel({ isAdmin }: { isAdmin: boolean }) {
  const [original, setOriginal] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(
    () =>
      netApi
        .readHosts()
        .then((t) => {
          setOriginal(t);
          setText(t);
        })
        .catch((e) => toast("error", String(e))),
    [toast],
  );
  useEffect(() => {
    void load();
  }, [load]);

  const entries = text.split("\n").filter((l) => l.trim() && !l.trim().startsWith("#")).length;
  const dirty = original !== null && text !== original;

  const save = async () => {
    const ok = await confirm({
      title: "Guardar el archivo hosts",
      body: "Se guardará una copia de la versión actual como hosts.adminops.bak en la misma carpeta y se vaciará la caché DNS.",
      confirmLabel: "Guardar",
    });
    if (!ok) return;
    setSaving(true);
    try {
      await netApi.saveHosts(text);
      toast("ok", "Archivo hosts guardado.");
      await load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card>
      <div className="mb-3 flex items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{entries}</span> {entries === 1 ? "entrada activa" : "entradas activas"}
          {entries > 0 && <span className="text-xs text-mute"> · las líneas activas redirigen o bloquean dominios</span>}
          {entries === 0 && original !== null && (
            <span className="ml-2 text-xs text-ok">
              <CircleCheck size={11} className="inline" /> sin redirecciones
            </span>
          )}
        </p>
        <div className="ml-auto flex gap-2">
          <Button kind="ghost" onClick={() => setText(DEFAULT_HOSTS)} disabled={saving} title="Carga el contenido original de Windows (no se guarda hasta pulsar Guardar)">
            <Undo2 size={13} /> Original de Windows
          </Button>
          <Button kind="ghost" onClick={() => original !== null && setText(original)} disabled={!dirty || saving}>
            Descartar cambios
          </Button>
          <Button onClick={save} disabled={!isAdmin || !dirty || saving} title={isAdmin ? undefined : "Requiere administrador"}>
            {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />} Guardar
          </Button>
        </div>
      </div>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        spellCheck={false}
        className="pane-lg w-full resize-none rounded-lg border border-line bg-void/60 p-3 font-mono text-xs leading-relaxed text-ink outline-none focus:border-neon/50"
      />
      <p className="mt-2 text-[11px] text-mute">
        Formato: <span className="font-mono">IP  dominio</span> por línea (p. ej. <span className="font-mono">0.0.0.0 publicidad.com</span> para bloquear). Las
        líneas con # son comentarios. Algunos antivirus avisan o bloquean al modificar este archivo.
      </p>
      {dialog}
    </Card>
  );
}
