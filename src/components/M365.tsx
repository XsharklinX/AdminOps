// Microsoft 365 (Graph) en la interfaz: conectar la cuenta del técnico, y lo que
// se hace con ella desde Personas, Hoy, la Agenda y el caso.
//
// Todo pasa por la cuenta del propio técnico: AdminOps solo puede lo que IT le
// concedió a la aplicación y lo que su rol le deja hacer. Si falta un permiso,
// esa acción lo dice y las demás siguen.
import { CalendarCheck, Check, Copy, ExternalLink, KeyRound, Loader2, LogIn, LogOut, MessagesSquare, ShieldCheck, ShieldOff, Trash2, TriangleAlert } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useConfirm, useToast } from "./feedback";
import { Button, Card, inputClass, Modal } from "./ui";
import { graphApi, type AuthMethod, type DeviceCode, type GraphStatus, type SignIn } from "../lib/api";
import { useLiveEffect } from "../lib/useLiveEffect";

/** Se lanza al conectar o desconectar, para que todas las vistas se enteren. */
const CHANGED = "adminops:graph-changed";

const SIN_CONEXION: GraphStatus = { configured: false, connected: false, account: "", tenant: "", clientId: "" };
/** Una sola lectura para todas las vistas (la Agenda pinta un botón por visita). */
let cached: Promise<GraphStatus> | null = null;
const readStatus = () => (cached ??= graphApi.status().catch(() => SIN_CONEXION));

/** Estado de la conexión con Microsoft 365 (null mientras se lee). */
export function useGraph(): GraphStatus | null {
  const [s, setS] = useState<GraphStatus | null>(null);
  useEffect(() => {
    let vivo = true;
    const read = () => {
      void readStatus().then((x) => vivo && setS(x));
    };
    read();
    window.addEventListener(CHANGED, read);
    return () => {
      vivo = false;
      window.removeEventListener(CHANGED, read);
    };
  }, []);
  return s;
}

const changed = () => {
  cached = null;
  window.dispatchEvent(new Event(CHANGED));
};

const cuando = (iso: string) => {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString("es", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
};

// ---------- Ajustes ----------

/** Tarjeta de Ajustes → Portales y correo. */
export function M365Settings() {
  const st = useGraph();
  const [tenant, setTenant] = useState("");
  const [clientId, setClientId] = useState("");
  const [busy, setBusy] = useState(false);
  const [code, setCode] = useState<DeviceCode | null>(null);
  const toast = useToast();

  useEffect(() => {
    if (!st) return;
    setTenant(st.tenant);
    setClientId(st.clientId);
  }, [st]);

  if (!st) return null;
  const dirty = tenant.trim() !== st.tenant || clientId.trim().toLowerCase() !== st.clientId;

  const save = async () => {
    setBusy(true);
    try {
      await graphApi.configure(tenant, clientId);
      toast("ok", "Datos de Microsoft 365 guardados.");
      changed();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const connect = async () => {
    setBusy(true);
    try {
      if (dirty) await graphApi.configure(tenant, clientId);
      setCode(await graphApi.loginStart());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const logout = async () => {
    try {
      await graphApi.logout();
      toast("ok", "Microsoft 365 desconectado. Los datos de la aplicación se conservan.");
      changed();
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <Card title="Microsoft 365" icon={<ShieldCheck size={14} />}>
      <p className="text-xs text-dim">
        Con tu cuenta de Microsoft 365: por qué falla el inicio de sesión de alguien (incluido el MFA), quitar el método de MFA de un móvil perdido, cerrar
        todas sus sesiones, el estado de los servicios en «Hoy», las visitas de la Agenda en tu Outlook y avisar por Teams al cerrar un caso.
      </p>

      <div className="mt-3 grid gap-3 sm:grid-cols-2">
        <label className="text-xs text-mute">
          Inquilino (dominio o GUID)
          <input value={tenant} onChange={(e) => setTenant(e.target.value)} placeholder="empresa.onmicrosoft.com" className={`${inputClass} mt-1 font-mono text-xs`} />
        </label>
        <label className="text-xs text-mute">
          Id de la aplicación (lo da IT)
          <input value={clientId} onChange={(e) => setClientId(e.target.value)} placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" className={`${inputClass} mt-1 font-mono text-xs`} />
        </label>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        {st.connected ? (
          <>
            <span className="flex min-w-0 items-center gap-1.5 text-sm text-ok">
              <Check size={14} /> Conectado{st.account ? <span className="truncate text-dim"> como {st.account}</span> : null}
            </span>
            <span className="flex-1" />
            {dirty && (
              <Button kind="ghost" onClick={() => void save()} disabled={busy}>
                Guardar
              </Button>
            )}
            <Button kind="ghost" onClick={() => void logout()}>
              <LogOut size={14} /> Desconectar
            </Button>
          </>
        ) : (
          <>
            <Button onClick={() => void connect()} disabled={busy || !tenant.trim() || !clientId.trim()}>
              {busy ? <Loader2 size={14} className="animate-spin" /> : <LogIn size={14} />} Conectar
            </Button>
            {dirty && st.configured && (
              <Button kind="ghost" onClick={() => void save()} disabled={busy}>
                Guardar sin conectar
              </Button>
            )}
          </>
        )}
      </div>

      <details className="mt-3">
        <summary className="cursor-pointer text-xs text-dim hover:text-ink">Qué tiene que preparar IT (una vez)</summary>
        <ol className="mt-2 list-decimal space-y-1 pl-5 text-xs text-dim">
          <li>
            En Entra ID → Registros de aplicaciones → Nuevo registro: cuentas «solo de este directorio», plataforma «Aplicaciones móviles y de escritorio».
          </li>
          <li>En Autenticación, activar «Permitir flujos de clientes públicos» (el inicio de sesión por código).</li>
          <li>
            Permisos <strong>delegados</strong> de Microsoft Graph, con consentimiento de administrador: <span className="font-mono">User.Read</span>,{" "}
            <span className="font-mono">AuditLog.Read.All</span>, <span className="font-mono">Directory.Read.All</span>,{" "}
            <span className="font-mono">UserAuthenticationMethod.ReadWrite.All</span>, <span className="font-mono">User.RevokeSessions.All</span>,{" "}
            <span className="font-mono">ServiceHealth.Read.All</span>, <span className="font-mono">Calendars.ReadWrite</span>,{" "}
            <span className="font-mono">Chat.Create</span>, <span className="font-mono">ChatMessage.Send</span>,{" "}
            <span className="font-mono">Presence.Read.All</span> (presencia de Teams) y <span className="font-mono">User.ReadBasic.All</span> (fotos). Los que no se den, esa función avisa y el resto funciona.
          </li>
          <li>
            Además, tu cuenta necesita el rol que corresponde: leer inicios de sesión (Lector de informes o Lector global, y Entra ID P1), tocar el MFA de otros
            (Administrador de autenticación), ver el estado de los servicios (Lector de estado del servicio o similar).
          </li>
        </ol>
        <p className="mt-2 text-[11px] text-mute">
          El inicio de sesión se hace en tu navegador, en microsoft.com/devicelogin, con tu MFA de siempre. AdminOps guarda la sesión cifrada en este equipo y
          nunca ve tu contraseña.
        </p>
      </details>

      {code && (
        <DeviceLogin
          code={code}
          onClose={() => setCode(null)}
          onDone={(account) => {
            setCode(null);
            toast("ok", account ? `Microsoft 365 conectado como ${account}.` : "Microsoft 365 conectado.");
            changed();
          }}
        />
      )}
    </Card>
  );
}

/** El código que se escribe en microsoft.com/devicelogin, esperando a que se complete. */
function DeviceLogin({ code, onClose, onDone }: { code: DeviceCode; onClose: () => void; onDone: (account: string) => void }) {
  const [problem, setProblem] = useState<string | null>(null);
  const toast = useToast();
  const onDoneRef = useRef(onDone);
  useEffect(() => {
    onDoneRef.current = onDone;
  }, [onDone]);

  useEffect(() => {
    // Nada más abrir: el código al portapapeles y la página de Microsoft en el navegador.
    navigator.clipboard.writeText(code.userCode).catch(() => {});
    graphApi.openDeviceLogin().catch(() => {});
    let vivo = true;
    let timer = 0;
    const poll = () => {
      graphApi
        .loginPoll()
        .then((r) => {
          if (!vivo) return;
          if (r.state === "done") onDoneRef.current(r.account);
          else if (r.state === "pending") timer = window.setTimeout(poll, Math.max(3, code.interval) * 1000);
          else setProblem(r.message || "No se completó el inicio de sesión.");
        })
        .catch((e) => vivo && setProblem(String(e)));
    };
    timer = window.setTimeout(poll, Math.max(3, code.interval) * 1000);
    return () => {
      vivo = false;
      window.clearTimeout(timer);
    };
  }, [code]);

  return (
    <Modal title="Conectar Microsoft 365" onClose={onClose} footer={<Button kind="ghost" onClick={onClose}>{problem ? "Cerrar" : "Cancelar"}</Button>}>
      {problem ? (
        <p className="flex items-start gap-2 text-sm text-bad">
          <TriangleAlert size={14} className="mt-0.5 shrink-0" /> {problem}
        </p>
      ) : (
        <div className="space-y-3 text-sm">
          <p className="text-dim">Se ha abierto microsoft.com/devicelogin en tu navegador. Escribe este código (ya está copiado) e inicia sesión con tu cuenta:</p>
          <div className="flex items-center justify-center gap-2">
            <span className="rounded-lg border border-neon/50 bg-neon/10 px-4 py-2 font-mono text-2xl tracking-[0.2em] text-neon select-text">{code.userCode}</span>
            <button
              onClick={() => navigator.clipboard.writeText(code.userCode).then(() => toast("ok", "Código copiado."), () => {})}
              className="text-mute hover:text-ink"
              title="Copiar"
            >
              <Copy size={16} />
            </button>
          </div>
          <div className="flex justify-center">
            <Button kind="ghost" onClick={() => void graphApi.openDeviceLogin().catch((e) => toast("error", String(e)))}>
              <ExternalLink size={14} /> Abrir la página otra vez
            </Button>
          </div>
          <p className="flex items-center justify-center gap-2 text-xs text-mute">
            <Loader2 size={12} className="animate-spin" /> Esperando a que termines en el navegador (el código vale {Math.round(code.expiresIn / 60)} minutos)…
          </p>
        </div>
      )}
    </Modal>
  );
}

// ---------- Personas ----------

/** Microsoft 365 de una persona: sus inicios de sesión, su MFA y sus sesiones. */
export function M365Person({ upn, name }: { upn: string; name: string }) {
  const st = useGraph();
  const [view, setView] = useState<"signins" | "mfa" | null>(null);
  const [signins, setSignins] = useState<SignIn[] | null>(null);
  const [methods, setMethods] = useState<AuthMethod[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [writing, setWriting] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  // Otra persona: se empieza de cero.
  useLiveEffect(() => {
    setView(null);
    setSignins(null);
    setMethods(null);
    setError(null);
  }, [upn]);

  if (!st?.connected) {
    return st ? (
      <Card title="Microsoft 365" icon={<ShieldCheck size={14} />}>
        <p className="text-xs text-mute">Conecta Microsoft 365 en Ajustes → Portales y correo para ver aquí por qué no puede entrar, su MFA y sus sesiones.</p>
      </Card>
    ) : null;
  }
  if (!upn) {
    return (
      <Card title="Microsoft 365" icon={<ShieldCheck size={14} />}>
        <p className="text-xs text-mute">Esta cuenta del dominio no tiene usuario de Microsoft 365 (userPrincipalName).</p>
      </Card>
    );
  }

  const showSignins = async () => {
    setView("signins");
    setError(null);
    setSignins(null);
    try {
      setSignins(await graphApi.signins(upn));
    } catch (e) {
      setError(String(e));
    }
  };

  const showMfa = async () => {
    setView("mfa");
    setError(null);
    setMethods(null);
    try {
      setMethods(await graphApi.mfaMethods(upn));
    } catch (e) {
      setError(String(e));
    }
  };

  const remove = async (m: AuthMethod) => {
    const ok = await confirm({
      title: "Quitar método de MFA",
      body: `Se quitará «${m.label}» de ${name || upn}. Si era su único método, al volver a entrar Microsoft le pedirá registrar uno nuevo.`,
      confirmLabel: "Quitar",
      danger: true,
    });
    if (!ok) return;
    setBusy(true);
    try {
      await graphApi.mfaRemove(upn, m.kind, m.id);
      toast("ok", "Método de MFA quitado.");
      await showMfa();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const revoke = async () => {
    const ok = await confirm({
      title: "Cerrar todas sus sesiones",
      body: `${name || upn} tendrá que volver a iniciar sesión en Outlook, Teams y el resto de Microsoft 365, en todos sus dispositivos. Úsalo si perdió el móvil o si su cuenta pudo verse comprometida.`,
      confirmLabel: "Cerrar sesiones",
      danger: true,
    });
    if (!ok) return;
    setBusy(true);
    try {
      await graphApi.revokeSessions(upn);
      toast("ok", "Sesiones cerradas. Puede tardar unos minutos en notarse en todos sus dispositivos.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card title="Microsoft 365" icon={<ShieldCheck size={14} />}>
      <div className="flex flex-wrap gap-2">
        <Button kind={view === "signins" ? "primary" : "ghost"} onClick={() => void showSignins()}>
          <LogIn size={14} /> Inicios de sesión
        </Button>
        <Button kind={view === "mfa" ? "primary" : "ghost"} onClick={() => void showMfa()}>
          <KeyRound size={14} /> MFA
        </Button>
        <Button kind="ghost" onClick={() => void revoke()} disabled={busy}>
          <ShieldOff size={14} /> Cerrar sus sesiones
        </Button>
        <Button kind="ghost" onClick={() => setWriting(true)}>
          <MessagesSquare size={14} /> Mensaje por Teams
        </Button>
      </div>

      {error && (
        <p className="mt-3 flex items-start gap-2 text-sm text-bad">
          <TriangleAlert size={14} className="mt-0.5 shrink-0" /> {error}
        </p>
      )}

      {view === "signins" && !error && (
        <div className="mt-3">
          {signins === null ? (
            <p className="flex items-center gap-2 text-xs text-mute">
              <Loader2 size={12} className="animate-spin" /> Leyendo sus inicios de sesión…
            </p>
          ) : signins.length === 0 ? (
            <p className="text-xs text-mute">No hay inicios de sesión recientes (los registros de Entra ID guardan los últimos 30 días).</p>
          ) : (
            <ul className="space-y-1.5">
              {signins.map((s, i) => (
                <li key={`${s.when}-${i}`} className="rounded-lg border border-line bg-panel-2 px-3 py-2">
                  <div className="flex items-start gap-2 text-sm">
                    <span className={`mt-1 size-2 shrink-0 rounded-full ${s.ok ? "bg-ok" : "bg-bad"}`} />
                    <span className={`min-w-0 flex-1 ${s.ok ? "text-dim" : "text-ink"}`}>{s.reason}</span>
                    <span className="shrink-0 text-[11px] text-mute">{cuando(s.when)}</span>
                  </div>
                  <div className="mt-0.5 pl-4 text-[11px] text-mute">
                    {[s.app, s.client, s.place, s.ip, s.mfa && `MFA: ${s.mfa}`, s.conditionalAccess === "failure" && "bloqueado por acceso condicional"]
                      .filter(Boolean)
                      .join(" · ")}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {view === "mfa" && !error && (
        <div className="mt-3">
          {methods === null ? (
            <p className="flex items-center gap-2 text-xs text-mute">
              <Loader2 size={12} className="animate-spin" /> Leyendo sus métodos…
            </p>
          ) : (
            <>
              <ul className="space-y-1.5">
                {methods.map((m) => (
                  <li key={`${m.kind}-${m.id}`} className="flex items-center gap-2 rounded-lg border border-line bg-panel-2 px-3 py-1.5 text-sm">
                    <span className="min-w-0 flex-1 truncate text-dim">{m.label}</span>
                    {m.removable && (
                      <button onClick={() => void remove(m)} disabled={busy} className="text-mute hover:text-bad disabled:opacity-50" title="Quitar este método">
                        <Trash2 size={14} />
                      </button>
                    )}
                  </li>
                ))}
              </ul>
              {methods.filter((m) => m.removable).length === 0 && (
                <p className="mt-2 text-xs text-mute">No tiene métodos de MFA registrados: al entrar, Microsoft le pedirá registrar uno.</p>
              )}
            </>
          )}
        </div>
      )}

      <p className="mt-3 text-[11px] text-mute">Quitar el MFA o cerrar sesiones queda en el diario y el modo auditoría lo bloquea.</p>
      {writing && <TeamsDialog upn={upn} name={name} initial="" onClose={() => setWriting(false)} />}
      {dialog}
    </Card>
  );
}

// ---------- Teams ----------

/** Escribir a alguien por Teams sin abrir Teams. */
export function TeamsDialog({ upn, name, initial, onClose }: { upn: string; name: string; initial: string; onClose: () => void }) {
  const [text, setText] = useState(initial);
  const [sending, setSending] = useState(false);
  const toast = useToast();

  const send = async () => {
    setSending(true);
    try {
      await graphApi.teamsSend(upn, text);
      toast("ok", `Mensaje enviado a ${name || upn} por Teams.`);
      onClose();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setSending(false);
    }
  };

  return (
    <Modal
      title={`Teams · ${name || upn}`}
      onClose={onClose}
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={() => void send()} disabled={sending || !text.trim()}>
            {sending ? <Loader2 size={14} className="animate-spin" /> : <MessagesSquare size={14} />} Enviar
          </Button>
        </>
      }
    >
      <textarea value={text} onChange={(e) => setText(e.target.value)} rows={5} maxLength={2000} autoFocus className={`${inputClass} resize-y`} placeholder="Escribe el mensaje…" />
      <p className="mt-1 text-[11px] text-mute">Sale de tu cuenta, en un chat uno a uno con esa persona.</p>
    </Modal>
  );
}

// ---------- Agenda ----------

/** Pone la visita en tu calendario de Outlook, o la actualiza si ya estaba. */
export function OutlookButton({ visitId, inOutlook, onDone, className }: { visitId: string; inOutlook: boolean; onDone: () => void; className: string }) {
  const st = useGraph();
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  if (!st?.connected) return null;

  const sync = async () => {
    setBusy(true);
    try {
      await graphApi.calendarSync(visitId);
      toast("ok", inOutlook ? "Visita actualizada en Outlook." : "Visita puesta en tu calendario de Outlook.");
      onDone();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <button
      onClick={() => void sync()}
      disabled={busy}
      className={`${className} ${inOutlook ? "text-ok" : ""}`}
      title={inOutlook ? "Ya está en tu Outlook: actualizarla con los cambios" : "Ponerla en tu calendario de Outlook"}
    >
      {busy ? <Loader2 size={14} className="animate-spin" /> : <CalendarCheck size={14} />}
    </button>
  );
}
