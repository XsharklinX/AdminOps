import { Copy, Download, ExternalLink, Headset, KeyRound, MonitorSmartphone, Pencil, Plug, Plus, Power, RotateCw, Search, SlidersHorizontal, Stethoscope, Trash2, UserMinus, UserPlus } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, EmptyLine, EmptyState, inputClass, Modal, Loading, iconBtn } from "../components/ui";
import { useLiveEffect } from "../lib/useLiveEffect";
import { logQuietly, officeApi, remoteApi, usersApi, workApi, type Connection, type RdpOptions, type RdpServer, type Reach, type RemoteStatus, type RemoteTool } from "../lib/api";
import { ago } from "../lib/format";

const KIND: Record<string, string> = { rdp: "Escritorio remoto", anydesk: "AnyDesk", rustdesk: "RustDesk", teamviewer: "TeamViewer" };
const DEFAULT_OPTIONS: RdpOptions = { fullscreen: true, multimon: false, clipboard: true, drives: false, printers: false, audio: true };
const OPTION_LABELS: [keyof RdpOptions, string][] = [
  ["fullscreen", "Pantalla completa"],
  ["multimon", "Usar todos los monitores"],
  ["clipboard", "Copiar y pegar entre equipos"],
  ["drives", "Ver mis unidades en el remoto"],
  ["printers", "Usar mis impresoras"],
  ["audio", "Sonido en este equipo"],
];

function Options({ value, onChange }: { value: RdpOptions; onChange: (o: RdpOptions) => void }) {
  return (
    <div className="grid grid-cols-2 gap-x-4 gap-y-1.5">
      {OPTION_LABELS.map(([k, label]) => (
        <label key={k} className="flex items-center gap-2 text-sm text-dim">
          <input type="checkbox" checked={value[k]} onChange={(e) => onChange({ ...value, [k]: e.target.checked })} className="size-4 accent-[var(--color-neon)]" />
          {label}
        </label>
      ))}
    </div>
  );
}

/** Lo último comprobado de cada equipo (por nombre o IP), mientras AdminOps está abierta. */
const reachCache = new Map<string, Reach>();

function ReachResult({ r }: { r: Reach }) {
  return (
    <div className={`rounded-md px-3 py-2 text-xs ${r.rdpOpen ? "bg-ok/10 text-ink" : "bg-warn/10 text-ink"}`}>
      {r.rdpOpen ? (
        <>Listo para conectar{r.pingMs !== null ? ` · responde en ${r.pingMs} ms` : ""}{r.resolved ? ` · ${r.resolved}` : ""}.</>
      ) : (
        r.hint
      )}
    </div>
  );
}

/** «hace 3 días», para saber de un vistazo cuáles se usan. */
const usedAgo = (ts: number) => (ts ? `usada ${ago(ts)}` : "sin usar todavía");

export function Remote({ isAdmin }: { isAdmin: boolean }) {
  const [connections, setConnections] = useState<Connection[] | null>(null);
  const [editing, setEditing] = useState<Connection | null>(null);
  const [status, setStatus] = useState<RemoteStatus | null>(null);
  const [tools, setTools] = useState<RemoteTool[] | null>(null);
  const [server, setServer] = useState<RdpServer | null>(null);
  const [host, setHost] = useState("");
  const [user, setUser] = useState("");
  const [options, setOptions] = useState<RdpOptions>(DEFAULT_OPTIONS);
  const [showOptions, setShowOptions] = useState(false);
  const [reach, setReach] = useState<Reach | null>(null);
  const [testing, setTesting] = useState(false);
  /** Si contesta cada equipo guardado (solo Escritorio remoto): sin entrada si no se ha
   *  comprobado, `null` mientras se comprueba y `false` si no se pudo comprobar. */
  const [reachOf, setReachOf] = useState<Record<string, Reach | null | false>>({});
  const [query, setQuery] = useState("");
  const [mac, setMac] = useState("");
  const [installing, setInstalling] = useState<string | null>(null);
  const [toolTarget, setToolTarget] = useState<Record<string, string>>({});
  const [pwFor, setPwFor] = useState<Connection | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => {
    remoteApi
      .list()
      .then(setConnections)
      .catch(() => setConnections([]));
    officeApi.remoteStatus().then(setStatus).catch((e) => toast("error", String(e)));
    remoteApi.tools().then(setTools).catch(() => setTools([]));
    remoteApi.server().then(setServer).catch(logQuietly("Remote"));
  }, [toast]);

  useEffect(load, [load]);

  // Si contesta cada equipo de Escritorio remoto se mira al pulsar «Comprobar»
  // (no al abrir: con muchos equipos apagados eso llena la red de intentos), de dos
  // en dos. Lo comprobado se recuerda mientras AdminOps está abierta.
  const targets = (connections ?? [])
    .filter((c) => c.kind === "rdp")
    .map((c) => `${c.id}\n${c.target}`)
    .join("\n\n");
  const [round, setRound] = useState(0);
  useLiveEffect(
    (vigente) => {
      const queue = targets ? targets.split("\n\n").map((t) => t.split("\n") as [string, string]) : [];
      if (round === 0) {
        setReachOf(Object.fromEntries(queue.flatMap(([id, target]) => (reachCache.has(target) ? [[id, reachCache.get(target)!]] : []))));
        return;
      }
      setReachOf(Object.fromEntries(queue.map(([id]) => [id, null])));
      const worker = async () => {
        for (let next = queue.shift(); next && vigente(); next = queue.shift()) {
          const [id, target] = next;
          const r = await remoteApi.test(target).catch(() => undefined);
          if (!vigente()) return;
          if (r) reachCache.set(target, r);
          setReachOf((cur) => ({ ...cur, [id]: r ?? false }));
        }
      };
      void worker();
      void worker();
    },
    [targets, round],
  );

  const fail = (e: unknown) => toast("error", String(e));
  const copy = (t: string, what = "Copiado") => navigator.clipboard.writeText(t).then(() => toast("ok", `${what}.`), () => toast("error", "No se pudo copiar."));

  const test = async () => {
    setTesting(true);
    try {
      setReach(await remoteApi.test(host));
    } catch (e) {
      fail(e);
    } finally {
      setTesting(false);
    }
  };

  const remove = async (c: Connection) => {
    const ok = await confirm({ title: `¿Borrar «${c.name}»?`, confirmLabel: "Borrar", danger: true, body: <p>Se quita de la agenda{c.savedPassword ? " y se borra su contraseña guardada en Windows" : ""}.</p> });
    if (!ok) return;
    await remoteApi.remove(c.id).catch(fail);
    load();
  };

  const install = async (t: RemoteTool) => {
    setInstalling(t.id);
    try {
      await remoteApi.install(t.id);
      toast("ok", `${t.name} instalado.`);
      load();
    } catch (e) {
      fail(e);
    } finally {
      setInstalling(null);
    }
  };

  const toggleRdp = async () => {
    if (!status) return;
    if (!status.rdpEnabled) {
      const ok = await confirm({
        title: "¿Permitir Escritorio remoto en este equipo?",
        confirmLabel: "Permitir",
        body: <p>Otros equipos de la red podrán conectarse con un usuario y contraseña de este. Usa contraseñas fuertes y no abras el puerto 3389 del router a Internet.</p>,
      });
      if (!ok) return;
    }
    await officeApi.setRdp(!status.rdpEnabled).catch(fail);
    load();
  };

  const newConnection = (over: Partial<Connection> = {}): Connection => ({ id: "", name: "", kind: "rdp", target: "", username: "", client: "", notes: "", options: DEFAULT_OPTIONS, savedPassword: false, lastUsed: 0, ...over });

  // Las que más se usan, primero.
  const q = query.trim().toLowerCase();
  const shown = [...(connections ?? [])]
    .filter((c) => !q || `${c.name} ${c.target} ${c.client} ${c.notes} ${KIND[c.kind]}`.toLowerCase().includes(q))
    .sort((a, b) => b.lastUsed - a.lastUsed || a.name.localeCompare(b.name, "es"));

  const wired = status?.adapters.filter((a) => a.wired) ?? [];
  const wolReady = wired.some((a) => a.magicPacket === "Enabled") && status && !status.fastStartup;

  return (
    <div className="@container mx-auto max-w-6xl space-y-4 p-6">
      {/* Conexión rápida: un nombre y a conectar; lo demás, a un clic. */}
      <section className="rounded-xl border border-line bg-panel p-4">
        <form
          className="flex flex-wrap items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (host.trim()) remoteApi.connectRdp(host, user, options).catch(fail);
          }}
        >
          <div className="relative min-w-56 flex-1">
            <MonitorSmartphone size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
            <input
              value={host}
              onChange={(e) => (setHost(e.target.value), setReach(null))}
              placeholder="Conectar por Escritorio remoto a… (PC-RECEPCION, 192.168.1.20)"
              className={`${inputClass} py-2.5 pl-9`}
              aria-label="Equipo o IP"
            />
          </div>
          <Button onClick={() => remoteApi.connectRdp(host, user, options).catch(fail)} disabled={!host.trim()}>
            <Plug size={14} /> Conectar
          </Button>
          <Button kind="ghost" onClick={() => void test()} disabled={!host.trim() || testing}>
            <Stethoscope size={14} /> {testing ? "Probando…" : "Probar"}
          </Button>
          <Button kind="ghost" onClick={() => setShowOptions(!showOptions)} title="Usuario, pantalla completa, monitores, portapapeles…">
            <SlidersHorizontal size={14} /> Opciones
          </Button>
          <Button kind="ghost" onClick={() => setEditing(newConnection({ name: host.trim(), target: host.trim(), username: user.trim(), options }))} disabled={!host.trim()} title="Guardarla en la agenda de abajo">
            <Plus size={14} /> Guardar
          </Button>
        </form>
        {reach && (
          <div className="mt-3">
            <ReachResult r={reach} />
          </div>
        )}
        {showOptions && (
          <div className="mt-3 space-y-3 border-t border-line/60 pt-3">
            <input value={user} onChange={(e) => setUser(e.target.value)} placeholder="Usuario (opcional, p. ej. EQUIPO\ana)" className={`${inputClass} max-w-md`} aria-label="Usuario" />
            <Options value={options} onChange={setOptions} />
          </div>
        )}
      </section>

      <Card
        title={connections?.length ? `Conexiones guardadas · ${connections.length}` : "Conexiones guardadas"}
        icon={<Plug size={14} />}
        right={
          <div className="flex items-center gap-1">
            {(connections?.length ?? 0) > 6 && (
              <div className="relative">
                <Search size={13} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-mute" />
                <input
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder="Buscar"
                  className="h-8 w-44 rounded-md border border-line bg-void/60 pr-2 pl-7 text-xs text-ink outline-none placeholder:text-mute focus:border-neon/50"
                  aria-label="Buscar una conexión"
                />
              </div>
            )}
            <button onClick={() => setRound((n) => n + 1)} className={iconBtn} title="Comprobar si contestan" aria-label="Comprobar si contestan">
              <RotateCw size={14} />
            </button>
            <Button onClick={() => setEditing(newConnection())}>
              <Plus size={14} /> Nueva
            </Button>
          </div>
        }
      >
        {connections === null ? (
          <Loading />
        ) : connections.length === 0 ? (
          <EmptyState icon={<Plug size={26} />} title="Aún no hay conexiones guardadas" action={<Button onClick={() => setEditing(newConnection())}>Guardar la primera</Button>}>
            Tu agenda de equipos: Escritorio remoto con sus opciones y su contraseña (guardada en Windows), o el ID de AnyDesk, RustDesk o TeamViewer.
          </EmptyState>
        ) : shown.length === 0 ? (
          <EmptyLine>Ninguna conexión coincide con «{query}».</EmptyLine>
        ) : (
          <ul className="grid grid-cols-1 gap-2 @2xl:grid-cols-2 @5xl:grid-cols-3">
            {shown.map((c) => {
              const r = reachOf[c.id];
              const checking = c.kind === "rdp" && r === null;
              const dot = c.kind !== "rdp" || !r ? (checking ? "animate-pulse bg-mute" : "bg-line-2") : r.rdpOpen ? "bg-ok" : "bg-bad";
              const state =
                c.kind !== "rdp"
                  ? KIND[c.kind]
                  : checking
                    ? "Comprobando…"
                    : r === undefined
                      ? "Sin comprobar"
                      : r === false
                        ? "No se pudo comprobar"
                        : r?.rdpOpen
                          ? `Contesta${r.pingMs !== null ? ` · ${r.pingMs} ms` : ""}`
                          : "No contesta";
              return (
                <li key={c.id} className="group flex flex-col rounded-lg border border-line bg-panel-2/30 p-3 transition-colors hover:border-line-2">
                  <div className="flex items-start gap-2">
                    <span className={`mt-1.5 size-2 shrink-0 rounded-full ${dot}`} title={r && !r.rdpOpen ? r.hint : state} />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-1.5">
                        <span className="truncate text-sm font-medium text-ink">{c.name}</span>
                        {c.savedPassword && <KeyRound size={11} className="shrink-0 text-mute" aria-label="Contraseña guardada" />}
                      </div>
                      <div className="truncate font-mono text-[11px] text-dim">{c.target}</div>
                    </div>
                    <span className="shrink-0 rounded bg-panel-2 px-1.5 py-px text-[10px] text-dim">{KIND[c.kind]}</span>
                  </div>
                  <div className="mt-1.5 min-h-8 text-[11px] text-mute">
                    <div className="truncate">
                      {c.kind === "rdp" && <span className={!r ? "" : r.rdpOpen ? "text-ok" : "text-bad"}>{state} · </span>}
                      {usedAgo(c.lastUsed)}
                      {c.client && ` · ${c.client}`}
                      {c.username && ` · ${c.username}`}
                    </div>
                    {r && !r.rdpOpen ? <div className="truncate text-warn" title={r.hint}>{r.hint}</div> : c.notes && <div className="truncate text-dim">{c.notes}</div>}
                  </div>
                  <div className="mt-2 flex items-center gap-0.5">
                    <Button onClick={() => remoteApi.connect(c.id).then(load).catch(fail)}>Conectar</Button>
                    <span className="flex-1" />
                    {c.kind === "rdp" && (
                      <button onClick={() => setPwFor(c)} className={iconBtn} title={c.savedPassword ? "Cambiar u olvidar la contraseña" : "Guardar la contraseña en Windows"}>
                        <KeyRound size={14} />
                      </button>
                    )}
                    <button onClick={() => void copy(c.target, c.kind === "rdp" ? "Equipo copiado" : "ID copiado")} className={iconBtn} title="Copiar">
                      <Copy size={14} />
                    </button>
                    <button onClick={() => setEditing(c)} className={iconBtn} title="Editar">
                      <Pencil size={14} />
                    </button>
                    <button onClick={() => void remove(c)} className={`${iconBtn} hover:text-bad`} title="Borrar">
                      <Trash2 size={14} />
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </Card>

      <div className="grid grid-cols-12 gap-4">
        <Card title="Herramientas de asistencia" icon={<Headset size={14} />} className="col-span-12 @4xl:col-span-7">
          {tools === null ? (
            <Loading text="Buscando…" />
          ) : (
            <ul className="space-y-3">
              {tools.map((t) => (
                <li key={t.id} className="rounded-lg border border-line px-3 py-2.5">
                  <div className="flex items-center gap-2">
                    <span className="flex-1 text-sm text-ink">{t.name}</span>
                    {t.installed ? (
                      <Button kind="ghost" onClick={() => remoteApi.openTool(t.id).catch(fail)}>
                        <ExternalLink size={13} /> Abrir
                      </Button>
                    ) : (
                      <Button kind="ghost" onClick={() => void install(t)} disabled={installing !== null || !isAdmin}>
                        <Download size={13} /> Instalar
                      </Button>
                    )}
                  </div>
                  {t.installed && (
                    <>
                      <div className="mt-1 flex items-center gap-2 text-xs text-dim">
                        ID de este equipo:
                        {t.thisId ? (
                          <>
                            <span className="font-mono text-ink select-text">{t.thisId}</span>
                            <button onClick={() => void copy(t.thisId!, "ID copiado")} className="text-mute hover:text-ink" title="Copiar">
                              <Copy size={12} />
                            </button>
                          </>
                        ) : (
                          <span className="text-mute">ábrelo una vez para que lo genere</span>
                        )}
                      </div>
                      <form
                        className="mt-2 flex gap-2"
                        onSubmit={(e) => {
                          e.preventDefault();
                          if ((toolTarget[t.id] ?? "").trim()) remoteApi.connectTool(t.id, toolTarget[t.id] ?? "").catch(fail);
                        }}
                      >
                        <input
                          value={toolTarget[t.id] ?? ""}
                          onChange={(e) => setToolTarget({ ...toolTarget, [t.id]: e.target.value })}
                          placeholder="ID del otro equipo"
                          className="min-w-0 flex-1 rounded-md border border-line bg-void/60 px-2 py-1.5 font-mono text-xs text-ink outline-none focus:border-neon/50"
                        />
                        <Button onClick={() => remoteApi.connectTool(t.id, toolTarget[t.id] ?? "").catch(fail)} disabled={!(toolTarget[t.id] ?? "").trim()}>
                          Conectar
                        </Button>
                      </form>
                    </>
                  )}
                </li>
              ))}
            </ul>
          )}
          <TaskStatus task="remote-install" active={installing !== null} fallback="Instalando…" className="mt-3" />
        </Card>

        <Card title="Más maneras de llegar" icon={<Power size={14} />} className="col-span-12 @4xl:col-span-5">
          <div className="space-y-4">
            <div>
              <div className="text-sm text-ink">Encender un equipo por la red</div>
              <p className="mb-2 text-xs text-mute">Con su MAC, por cable y con «Wake on LAN» activado en su BIOS.</p>
              <form
                className="flex gap-2"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (mac.trim())
                    officeApi
                      .wake(mac)
                      .then(() => toast("ok", "Señal de encendido enviada."))
                      .catch(fail);
                }}
              >
                <input value={mac} onChange={(e) => setMac(e.target.value)} placeholder="00:11:22:33:44:55" className={`${inputClass} font-mono`} aria-label="MAC del equipo" />
                <Button
                  onClick={() =>
                    officeApi
                      .wake(mac)
                      .then(() => toast("ok", "Señal de encendido enviada."))
                      .catch(fail)
                  }
                  disabled={!mac.trim()}
                >
                  <Power size={14} /> Encender
                </Button>
              </form>
            </div>
            <div className="border-t border-line/60 pt-4">
              <div className="text-sm text-ink">Asistencia remota de Windows</div>
              <p className="mb-2 text-xs text-mute">La que trae Windows: la otra persona te manda una invitación y ve lo que haces.</p>
              <Button kind="ghost" onClick={() => officeApi.remoteAssistance().catch(fail)}>
                <Headset size={14} /> Abrir Asistencia remota
              </Button>
            </div>
          </div>
        </Card>
      </div>

      <Card
        title="Este equipo"
        icon={<MonitorSmartphone size={14} />}
        right={
          <button onClick={load} className={iconBtn} title="Volver a comprobar">
            <RotateCw size={14} />
          </button>
        }
      >
        {!status ? (
          <Loading />
        ) : (
          <div className="grid grid-cols-1 gap-6 @3xl:grid-cols-3">
            <div className="space-y-1 text-sm">
              {[
                ["Nombre", status.host],
                ["IP", status.ip],
                ["MAC", status.mac],
              ].map(([k, v]) => (
                <div key={k} className="flex items-center gap-3">
                  <span className="w-16 text-dim">{k}</span>
                  <span className="font-mono text-ink">{v || "—"}</span>
                  {v && (
                    <button onClick={() => void copy(v)} className="text-mute hover:text-ink" title="Copiar">
                      <Copy size={12} />
                    </button>
                  )}
                </div>
              ))}
            </div>
            <div className="text-sm">
              <div className="flex items-center gap-3">
                <div className="min-w-0 flex-1">
                  <div className="text-ink">Recibir Escritorio remoto</div>
                  <div className="text-xs text-mute">
                    {!status.rdpSupported
                      ? "Windows Home no puede recibirlo (sí conectarse a otros)."
                      : status.rdpEnabled
                        ? `Activado · puerto ${server?.port ?? 3389}${server && !server.nla ? " · sin autenticación de red (menos seguro)" : ""}`
                        : "Desactivado."}
                  </div>
                </div>
                {status.rdpSupported && (
                  <Button kind={status.rdpEnabled ? "ghost" : "primary"} onClick={() => void toggleRdp()} disabled={!isAdmin}>
                    {status.rdpEnabled ? "Desactivar" : "Activar"}
                  </Button>
                )}
              </div>
              {status.rdpSupported && server && <RdpUsers server={server} onChange={load} isAdmin={isAdmin} />}
            </div>
            <div className="text-sm">
              <div className="flex items-center gap-3">
                <div className="min-w-0 flex-1">
                  <div className="text-ink">Encenderse por la red</div>
                  <div className="text-xs text-mute">
                    {wired.length === 0
                      ? "Solo por cable, y este equipo no tiene adaptador de red por cable."
                      : wolReady
                        ? "Preparado en Windows. Actívalo también en la BIOS («Wake on LAN»)."
                        : "No preparado: se activa en el adaptador por cable y se desactiva el inicio rápido."}
                  </div>
                </div>
                {wired.length > 0 && !wolReady && (
                  <Button onClick={() => officeApi.enableWol().then(load).catch(fail)} disabled={!isAdmin}>
                    Preparar
                  </Button>
                )}
              </div>
            </div>
          </div>
        )}
      </Card>

      {editing && (
        <ConnectionEditor
          value={editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            load();
          }}
        />
      )}
      {pwFor && <PasswordDialog c={pwFor} onClose={() => setPwFor(null)} onDone={load} />}
      {dialog}
    </div>
  );
}

function RdpUsers({ server, onChange, isAdmin }: { server: RdpServer; onChange: () => void; isAdmin: boolean }) {
  const [users, setUsers] = useState<string[]>([]);
  const [pick, setPick] = useState("");
  const toast = useToast();
  useEffect(() => {
    usersApi
      .list()
      .then((u) => setUsers(u.filter((x) => x.enabled && !x.builtin && !x.admin).map((x) => x.name)))
      .catch(logQuietly("Remote"));
  }, []);
  const short = (n: string) => n.replace(/^[^\\]+\\/, "");
  const available = users.filter((u) => !server.users.some((s) => short(s).toLowerCase() === u.toLowerCase()));
  const set = (user: string, allow: boolean) =>
    remoteApi
      .setUser(user, allow)
      .then(onChange)
      .catch((e) => toast("error", String(e)));
  return (
    <div className="mt-3 border-t border-line/60 pt-3">
      <div className="mb-1 text-xs text-dim">Pueden entrar (además de los administradores)</div>
      {server.users.length === 0 ? (
        <p className="text-xs text-mute">Solo los administradores.</p>
      ) : (
        <ul className="space-y-0.5">
          {server.users.map((u) => (
            <li key={u} className="flex items-center gap-2 text-xs text-ink">
              {short(u)}
              <button onClick={() => set(short(u), false)} disabled={!isAdmin} className="text-mute hover:text-bad" title="Quitar permiso">
                <UserMinus size={12} />
              </button>
            </li>
          ))}
        </ul>
      )}
      {available.length > 0 && (
        <div className="mt-2 flex gap-2">
          <select value={pick} onChange={(e) => setPick(e.target.value)} className="flex-1 rounded-md border border-line bg-void/60 px-2 py-1 text-xs text-ink outline-none">
            <option value="">Dar permiso a…</option>
            {available.map((u) => (
              <option key={u} value={u}>
                {u}
              </option>
            ))}
          </select>
          <button onClick={() => pick && set(pick, true)} disabled={!pick || !isAdmin} className="rounded-md border border-line-2 px-2 text-xs text-dim hover:text-ink disabled:opacity-40">
            <UserPlus size={12} />
          </button>
        </div>
      )}
    </div>
  );
}

function ConnectionEditor({ value, onClose, onSaved }: { value: Connection; onClose: () => void; onSaved: () => void }) {
  const [c, setC] = useState(value);
  const [password, setPassword] = useState("");
  const [clients, setClients] = useState<string[]>([]);
  const toast = useToast();
  useEffect(() => {
    workApi
      .clients()
      .then((l) => setClients(l.map((x) => x.name)))
      .catch(logQuietly("Remote"));
  }, []);
  const save = async () => {
    try {
      const saved = await remoteApi.save(c);
      if (c.kind === "rdp" && password) await remoteApi.setPassword(saved.id, password);
      toast("ok", "Conexión guardada.");
      onSaved();
    } catch (e) {
      toast("error", String(e));
    }
  };
  const field = (label: string, node: React.ReactNode) => (
    <label className="block">
      <span className="mb-1 block text-xs text-dim">{label}</span>
      {node}
    </label>
  );
  return (
    <Modal title={c.id ? "Editar conexión" : "Nueva conexión"} onClose={onClose} width="w-[560px]" footer={<Button onClick={save}>Guardar</Button>}>
      <div className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          {field("Nombre", <input value={c.name} onChange={(e) => setC({ ...c, name: e.target.value })} placeholder="PC de recepción" className={inputClass} autoFocus />)}
          {field(
            "Tipo",
            <select value={c.kind} onChange={(e) => setC({ ...c, kind: e.target.value })} className={inputClass}>
              {Object.entries(KIND).map(([k, l]) => (
                <option key={k} value={k}>
                  {l}
                </option>
              ))}
            </select>,
          )}
          {field(
            c.kind === "rdp" ? "Equipo o IP" : `ID de ${KIND[c.kind]}`,
            <input value={c.target} onChange={(e) => setC({ ...c, target: e.target.value })} className={`${inputClass} font-mono`} />,
          )}
          {c.kind === "rdp" && field("Usuario", <input value={c.username} onChange={(e) => setC({ ...c, username: e.target.value })} placeholder="EQUIPO\usuario" className={inputClass} />)}
          {field(
            "Cliente",
            <>
              <input list="remote-clients" value={c.client} onChange={(e) => setC({ ...c, client: e.target.value })} className={inputClass} />
              <datalist id="remote-clients">
                {clients.map((n) => (
                  <option key={n} value={n} />
                ))}
              </datalist>
            </>,
          )}
          {c.kind === "rdp" &&
            field(
              c.savedPassword ? "Contraseña (vacía: no cambiar)" : "Contraseña (opcional)",
              <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="new-password" className={inputClass} />,
            )}
        </div>
        {c.kind === "rdp" && <Options value={c.options} onChange={(options) => setC({ ...c, options })} />}
        {field("Notas", <textarea value={c.notes} onChange={(e) => setC({ ...c, notes: e.target.value })} rows={2} className={`${inputClass} resize-y`} />)}
        {c.kind === "rdp" && <p className="text-[11px] text-mute">La contraseña se guarda en el Administrador de credenciales de Windows (no en AdminOps), y Escritorio remoto la usa al conectar.</p>}
      </div>
    </Modal>
  );
}

function PasswordDialog({ c, onClose, onDone }: { c: Connection; onClose: () => void; onDone: () => void }) {
  const [pw, setPw] = useState("");
  const toast = useToast();
  const apply = async (value: string) => {
    try {
      await remoteApi.setPassword(c.id, value);
      toast("ok", value ? "Contraseña guardada en Windows." : "Contraseña olvidada.");
      onDone();
      onClose();
    } catch (e) {
      toast("error", String(e));
    }
  };
  return (
    <Modal
      title={`Contraseña de «${c.name}»`}
      onClose={onClose}
      width="w-[420px]"
      footer={
        <>
          {c.savedPassword && (
            <Button kind="danger" onClick={() => apply("")}>
              Olvidar
            </Button>
          )}
          <Button onClick={() => apply(pw)} disabled={!pw}>
            Guardar
          </Button>
        </>
      }
    >
      <p className="mb-2 text-sm text-dim">Usuario: {c.username || "(ponlo antes en Editar)"}</p>
      <input type="password" value={pw} onChange={(e) => setPw(e.target.value)} autoFocus autoComplete="new-password" className={inputClass} />
    </Modal>
  );
}
