import { Copy, Download, ExternalLink, Headset, KeyRound, MonitorSmartphone, Pencil, Plug, Plus, Power, RotateCw, Stethoscope, Trash2, UserMinus, UserPlus } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, inputClass, Modal, Loading } from "../components/ui";
import { officeApi, remoteApi, usersApi, workApi, type Connection, type RdpOptions, type RdpServer, type Reach, type RemoteStatus, type RemoteTool } from "../lib/api";

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

export function Remote({ isAdmin }: { isAdmin: boolean }) {
  const [connections, setConnections] = useState<Connection[]>([]);
  const [editing, setEditing] = useState<Connection | null>(null);
  const [status, setStatus] = useState<RemoteStatus | null>(null);
  const [tools, setTools] = useState<RemoteTool[] | null>(null);
  const [server, setServer] = useState<RdpServer | null>(null);
  const [host, setHost] = useState("");
  const [user, setUser] = useState("");
  const [options, setOptions] = useState<RdpOptions>(DEFAULT_OPTIONS);
  const [reach, setReach] = useState<Reach | null>(null);
  const [testing, setTesting] = useState<string | null>(null);
  const [mac, setMac] = useState("");
  const [installing, setInstalling] = useState<string | null>(null);
  const [toolTarget, setToolTarget] = useState<Record<string, string>>({});
  const [pwFor, setPwFor] = useState<Connection | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => {
    remoteApi.list().then(setConnections).catch(() => {});
    officeApi.remoteStatus().then(setStatus).catch((e) => toast("error", String(e)));
    remoteApi.tools().then(setTools).catch(() => setTools([]));
    remoteApi.server().then(setServer).catch(() => {});
  }, [toast]);

  useEffect(load, [load]);

  const fail = (e: unknown) => toast("error", String(e));
  const copy = (t: string, what = "Copiado") => navigator.clipboard.writeText(t).then(() => toast("ok", `${what}.`));

  const test = async (key: string, target: string) => {
    setTesting(key);
    try {
      const r = await remoteApi.test(target);
      if (key === "quick") setReach(r);
      else toast(r.rdpOpen ? "ok" : "info", r.rdpOpen ? `Listo para conectar${r.pingMs !== null ? ` (${r.pingMs} ms)` : ""}.` : r.hint);
    } catch (e) {
      fail(e);
    } finally {
      setTesting(null);
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

  const wired = status?.adapters.filter((a) => a.wired) ?? [];
  const wolReady = wired.some((a) => a.magicPacket === "Enabled") && status && !status.fastStartup;
  const btn = "rounded-md p-1.5 text-mute transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-30";

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <Card title="Conexiones guardadas" icon={<Plug size={14} />} className="col-span-12">
        <div className="mb-3 flex items-center gap-3">
          <p className="min-w-0 flex-1 text-sm text-dim">Tu agenda de equipos: Escritorio remoto con sus opciones y contraseña (guardada en Windows), o el ID de AnyDesk, RustDesk o TeamViewer.</p>
          <Button onClick={() => setEditing({ id: "", name: "", kind: "rdp", target: "", username: "", client: "", notes: "", options: DEFAULT_OPTIONS, savedPassword: false, lastUsed: 0 })}>
            <Plus size={14} /> Nueva conexión
          </Button>
        </div>
        {connections.length === 0 ? (
          <p className="text-sm text-mute">Aún no hay conexiones guardadas.</p>
        ) : (
          <ul className="divide-y divide-line/60">
            {connections.map((c) => (
              <li key={c.id} className="group flex items-center gap-3 py-2.5">
                <MonitorSmartphone size={16} className="shrink-0 text-mute" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2 text-sm">
                    <span className="text-ink">{c.name}</span>
                    <span className="rounded bg-panel-2 px-1.5 text-[11px] text-dim">{KIND[c.kind]}</span>
                    {c.savedPassword && <KeyRound size={12} className="text-mute" aria-label="Contraseña guardada" />}
                  </div>
                  <div className="truncate text-xs text-mute">
                    <span className="font-mono">{c.target}</span>
                    {c.username && ` · ${c.username}`}
                    {c.client && ` · ${c.client}`}
                    {c.lastUsed > 0 && ` · usada ${new Date(c.lastUsed * 1000).toLocaleDateString("es", { dateStyle: "medium" })}`}
                  </div>
                  {c.notes && <div className="truncate text-xs text-dim">{c.notes}</div>}
                </div>
                <span className="flex gap-0.5 opacity-60 group-hover:opacity-100">
                  {c.kind === "rdp" && (
                    <>
                      <button onClick={() => test(c.id, c.target)} disabled={testing !== null} className={btn} title="Probar la conexión">
                        <Stethoscope size={14} />
                      </button>
                      <button onClick={() => setPwFor(c)} className={btn} title={c.savedPassword ? "Cambiar u olvidar la contraseña" : "Guardar la contraseña en Windows"}>
                        <KeyRound size={14} />
                      </button>
                    </>
                  )}
                  <button onClick={() => setEditing(c)} className={btn} title="Editar">
                    <Pencil size={14} />
                  </button>
                  <button onClick={() => remove(c)} className={btn} title="Borrar">
                    <Trash2 size={14} />
                  </button>
                </span>
                <Button onClick={() => remoteApi.connect(c.id).then(load).catch(fail)}>Conectar</Button>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card title="Conexión rápida" icon={<MonitorSmartphone size={14} />} className="col-span-12 lg:col-span-7">
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <input value={host} onChange={(e) => (setHost(e.target.value), setReach(null))} placeholder="Equipo o IP (PC-RECEPCION, 192.168.1.20)" className={inputClass} />
            <input value={user} onChange={(e) => setUser(e.target.value)} placeholder="Usuario (opcional, p. ej. EQUIPO\ana)" className={inputClass} />
          </div>
          <Options value={options} onChange={setOptions} />
          {reach && <ReachResult r={reach} />}
          <div className="flex flex-wrap gap-2">
            <Button kind="ghost" onClick={() => test("quick", host)} disabled={!host.trim() || testing !== null}>
              <Stethoscope size={14} /> {testing === "quick" ? "Probando…" : "Probar"}
            </Button>
            <Button onClick={() => remoteApi.connectRdp(host, user, options).catch(fail)} disabled={!host.trim()}>
              Conectar
            </Button>
            <Button
              kind="ghost"
              onClick={() => setEditing({ id: "", name: host.trim(), kind: "rdp", target: host.trim(), username: user.trim(), client: "", notes: "", options, savedPassword: false, lastUsed: 0 })}
              disabled={!host.trim()}
            >
              <Plus size={14} /> Guardar en la agenda
            </Button>
          </div>
        </div>
        <div className="mt-5 grid grid-cols-1 gap-4 border-t border-line/60 pt-4 md:grid-cols-2">
          <div>
            <div className="mb-1 text-xs text-dim">Asistencia rápida de Windows (por Internet, con un código)</div>
            <Button kind="ghost" onClick={() => officeApi.quickAssist().catch(fail)}>
              <Headset size={14} /> Abrir Asistencia rápida
            </Button>
          </div>
          <div>
            <div className="mb-1 text-xs text-dim">Encender un equipo por la red</div>
            <div className="flex gap-2">
              <input value={mac} onChange={(e) => setMac(e.target.value)} placeholder="MAC (00:11:22:33:44:55)" className={`${inputClass} font-mono`} />
              <Button
                onClick={() =>
                  officeApi
                    .wake(mac)
                    .then(() => toast("ok", "Señal de encendido enviada."))
                    .catch(fail)
                }
                disabled={!mac.trim()}
              >
                <Power size={14} />
              </Button>
            </div>
          </div>
        </div>
      </Card>

      <Card title="Herramientas de asistencia" className="col-span-12 lg:col-span-5">
        {tools === null ? (
          <p className="font-mono text-xs text-mute">Buscando…</p>
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
                    <Button kind="ghost" onClick={() => install(t)} disabled={installing !== null || !isAdmin}>
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
                          <button onClick={() => copy(t.thisId!, "ID copiado")} className="text-mute hover:text-ink" title="Copiar">
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
                        remoteApi.connectTool(t.id, toolTarget[t.id] ?? "").catch(fail);
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

      <Card title="Este equipo" className="col-span-12">
        {!status ? (
          <Loading />
        ) : (
          <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
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
                    <button onClick={() => copy(v)} className="text-mute hover:text-ink" title="Copiar">
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
                  <Button kind={status.rdpEnabled ? "ghost" : "primary"} onClick={toggleRdp} disabled={!isAdmin}>
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
        <div className="mt-3 flex justify-end">
          <Button kind="ghost" onClick={load}>
            <RotateCw size={14} /> Volver a comprobar
          </Button>
        </div>
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
      .catch(() => {});
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
      .catch(() => {});
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
