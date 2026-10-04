import { Building2, CheckCircle2, Circle, ExternalLink, KeyRound, Loader2, LogOut, Mail, RefreshCw, Search, ShieldAlert, Trash2, UserRound, Users } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, inputClass, Loading } from "../components/ui";
import { WindowsTools } from "../components/WindowsTools";
import { accountsApi, troubleshootApi, usersApi, type AccountsStatus } from "../lib/api";
import { NeedsAdmin } from "../components/AdminBanner";

const KIND = {
  local: { label: "Cuenta local", color: "text-ok" },
  microsoft: { label: "Cuenta de Microsoft", color: "text-neon" },
  azuread: { label: "Cuenta profesional (Entra ID)", color: "text-warn" },
  domain: { label: "Cuenta del dominio", color: "text-neon" },
};

const settings = (uri: string) => troubleshootApi.fix(`open:ms-settings:${uri}`);

/** Qué cuentas hay en el equipo y en la sesión, y desconectarlas sin dar vueltas por Configuración. */
export function Accounts({ isAdmin }: { isAdmin: boolean }) {
  const [s, setS] = useState<AccountsStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [showCreds, setShowCreds] = useState(false);
  const [name, setName] = useState("admin.local");
  const [password, setPassword] = useState("");
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    try {
      setS(await accountsApi.status());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const run = async (key: string, fn: () => Promise<unknown>) => {
    setBusy(key);
    try {
      const msg = await fn();
      if (typeof msg === "string" && msg) toast("ok", msg);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      void load();
    }
  };

  const creds = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (s?.credentials ?? []).filter((c) => !q || `${c.target} ${c.user}`.toLowerCase().includes(q));
  }, [s, query]);

  if (error) return <p className="p-8 text-sm text-bad">{error}</p>;
  if (!s) return <Loading page text="Leyendo las cuentas del equipo…" />;

  const k = KIND[s.sessionKind];
  const joined = s.device.azureAdJoined;
  const needsLocal = joined || s.sessionKind === "azuread";

  const createAdmin = () =>
    run("create", async () => {
      if (password.length < 6) throw "La contraseña debe tener al menos 6 caracteres.";
      await usersApi.create({ name: name.trim(), fullName: "", password, admin: true, passwordNeverExpires: true, mustChange: false });
      setPassword("");
      return `Administrador local «${name.trim()}» creado. Apunta la contraseña.`;
    });

  const leave = async () => {
    const ok = await confirm({
      title: "Sacar el equipo de Entra ID",
      body: "El equipo dejará de estar unido a la organización (Entra ID / Azure AD). Las cuentas profesionales no podrán iniciar sesión en él; se entrará con la cuenta local. Los archivos del usuario no se borran.",
      confirmLabel: "Sacar el equipo",
      danger: true,
    });
    if (ok) void run("leave", accountsApi.leaveAzureAd);
  };

  const signOut = async () => {
    const ok = await confirm({ title: "Cerrar la sesión de Windows", body: "Se cerrará la sesión ahora (guarda lo que tengas abierto). Después entra con la otra cuenta.", confirmLabel: "Cerrar sesión", danger: true });
    if (ok) void run("signout", accountsApi.signOut);
  };

  const step = (done: boolean, n: number) => (done ? <CheckCircle2 size={16} className="shrink-0 text-ok" /> : <span className="grid size-4 shrink-0 place-items-center rounded-full border border-line-2 text-[10px] text-mute">{n}</span>);

  return (
    <div className="mx-auto grid max-w-5xl grid-cols-12 gap-4 p-6">
      {/* Esta sesión y el equipo */}
      <Card
        title="Esta sesión"
        icon={<UserRound size={14} />}
        className="col-span-12"
        right={
          <button onClick={load} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
            <RefreshCw size={11} /> Volver a leer
          </button>
        }
      >
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <p className="text-[11px] text-mute">Sesión iniciada con</p>
            <p className={`text-sm font-medium ${k.color}`}>{k.label}</p>
            <p className="truncate text-xs text-dim">{s.sessionUser.split("\\").pop()}</p>
          </div>
          <div>
            <p className="text-[11px] text-mute">El equipo</p>
            <p className="text-sm text-ink">
              {joined ? "Unido a Entra ID (Azure AD)" : s.device.domainJoined ? `Unido al dominio ${s.device.domainName}` : "No unido a ninguna organización"}
            </p>
            {joined && s.device.tenantName && <p className="text-xs text-dim">Organización: {s.device.tenantName}</p>}
          </div>
          <div className="flex flex-wrap items-start gap-2 md:justify-end">
            <Button kind="ghost" onClick={signOut} disabled={!!busy}>
              <LogOut size={13} /> Cerrar sesión
            </Button>
          </div>
        </div>
        {s.otherUser && (
          <p className="mt-3 text-xs text-warn">
            AdminOps se abrió con otra cuenta de Windows: se muestran las cuentas del usuario de la sesión, pero algunas solo se pueden quitar abriendo AdminOps desde su
            propia sesión.
          </p>
        )}
      </Card>

      {/* El caso típico: se entró con una cuenta profesional y debía ser local */}
      {needsLocal && (
        <Card title="Pasar el equipo a una cuenta local" icon={<ShieldAlert size={14} />} className="col-span-12 border-warn/40">
          <p className="mb-3 text-sm text-dim">
            Este equipo {joined ? "está unido a una organización (Entra ID)" : "usa una cuenta profesional"}. Si debía usarse con una cuenta local, hazlo en este orden:
          </p>
          <ol className="space-y-3">
            <li className="flex items-start gap-3">
              {step(s.hasLocalAdmin, 1)}
              <div className="min-w-0 flex-1">
                <p className="text-sm text-ink">Crear un administrador local {s.hasLocalAdmin && <span className="text-xs text-ok">· ya hay uno</span>}</p>
                {!s.hasLocalAdmin && (
                  <div className="mt-2 flex flex-wrap gap-2">
                    <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Usuario" className={`${inputClass} w-44`} />
                    <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="Contraseña" className={`${inputClass} w-44`} />
                    <Button onClick={createAdmin} disabled={!!busy || !isAdmin || !name.trim()}>
                      {busy === "create" && <Loader2 size={13} className="animate-spin" />} Crear
                    </Button>
                  </div>
                )}
              </div>
            </li>
            <li className="flex items-start gap-3">
              {step(!joined, 2)}
              <div className="min-w-0 flex-1">
                <p className="text-sm text-ink">Sacar el equipo de Entra ID {!joined && <span className="text-xs text-ok">· hecho</span>}</p>
                {joined && (
                  <Button kind="danger" onClick={leave} disabled={!!busy || !isAdmin || !s.hasLocalAdmin} title={!s.hasLocalAdmin ? "Crea antes el administrador local" : undefined}>
                    {busy === "leave" && <Loader2 size={13} className="animate-spin" />} Sacar el equipo
                  </Button>
                )}
              </div>
            </li>
            <li className="flex items-start gap-3">
              {step(false, 3)}
              <div className="min-w-0 flex-1">
                <p className="text-sm text-ink">Cerrar la sesión y entrar con la cuenta local</p>
                <Button kind="ghost" onClick={signOut} disabled={!!busy}>
                  <LogOut size={13} /> Cerrar sesión
                </Button>
              </div>
            </li>
          </ol>
          {!isAdmin && <NeedsAdmin className="mt-3">Estos pasos requieren administrador.</NeedsAdmin>}
        </Card>
      )}

      {s.sessionKind === "microsoft" && (
        <Card title="Pasar esta cuenta de Microsoft a local" icon={<Users size={14} />} className="col-span-12">
          <p className="text-sm text-dim">
            Windows solo deja convertirla desde Configuración (pide la contraseña actual): «Tu información → Iniciar sesión con una cuenta local». Los archivos se conservan. Si no
            se sabe la contraseña, crea un administrador local en Usuarios y cierra la sesión.
          </p>
          <div className="mt-3 flex gap-2">
            <Button kind="ghost" onClick={() => settings("yourinfo")}>
              <ExternalLink size={13} /> Abrir «Tu información»
            </Button>
          </div>
        </Card>
      )}

      {/* Cuentas profesionales o educativas */}
      <Card title={`Cuentas profesionales o educativas · ${s.workAccounts.length}`} icon={<Building2 size={14} />} className="col-span-12 lg:col-span-6">
        {s.workAccounts.length === 0 ? (
          <p className="text-sm text-mute">Ninguna. Es lo normal en un equipo personal o que solo usa el dominio.</p>
        ) : (
          <ul className="space-y-2">
            {s.workAccounts.map((a) => (
              <li key={a.id} className="flex items-center gap-2 text-sm">
                <Circle size={8} className="shrink-0 fill-current text-warn" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-ink">{a.email || "Cuenta sin correo"}</span>
                  <span className="text-[11px] text-mute">{a.scope === "device" ? "El equipo está unido con ella (Entra ID)" : "Añadida en «Acceso al trabajo o la escuela»"}</span>
                </span>
                {a.scope === "user" ? (
                  <button
                    onClick={async () => {
                      if (await confirm({ title: "Desconectar cuenta", body: `Windows dejará de usar ${a.email} (Outlook, Teams, OneDrive de empresa pedirán iniciar sesión otra vez).`, confirmLabel: "Desconectar", danger: true }))
                        void run(a.id, () => accountsApi.removeWorkAccount(a.id));
                    }}
                    disabled={!!busy}
                    className="rounded px-2 py-0.5 text-xs text-bad hover:bg-bad/10 disabled:opacity-40"
                  >
                    {busy === a.id ? <Loader2 size={12} className="animate-spin" /> : "Desconectar"}
                  </button>
                ) : (
                  <span className="text-[11px] text-mute">Ver «Pasar a cuenta local»</span>
                )}
              </li>
            ))}
          </ul>
        )}
        <button onClick={() => settings("workplace")} className="mt-3 flex items-center gap-1 text-xs text-mute hover:text-ink">
          <ExternalLink size={11} /> Acceso al trabajo o la escuela
        </button>
      </Card>

      {/* Cuentas de Microsoft personales y Office */}
      <Card title="Cuentas de Microsoft y Office" icon={<Mail size={14} />} className="col-span-12 lg:col-span-6">
        <p className="mb-1 text-[11px] tracking-wide text-mute uppercase">En Windows (Store, Correo, OneDrive…)</p>
        {s.microsoftAccounts.length === 0 ? (
          <p className="mb-3 text-sm text-mute">Ninguna.</p>
        ) : (
          <ul className="mb-3 space-y-1">
            {s.microsoftAccounts.map((m) => (
              <li key={m} className="truncate text-sm text-ink">
                {m}
              </li>
            ))}
          </ul>
        )}
        <p className="mb-1 text-[11px] tracking-wide text-mute uppercase">En Office</p>
        {s.officeAccounts.length === 0 ? (
          <p className="text-sm text-mute">Ninguna.</p>
        ) : (
          <ul className="space-y-1.5">
            {s.officeAccounts.map((o) => (
              <li key={o.id} className="flex items-center gap-2 text-sm">
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-ink">{o.email}</span>
                  <span className="text-[11px] text-mute">{o.kind}</span>
                </span>
                <button onClick={() => run(o.id, () => accountsApi.officeSignOut(o.id))} disabled={!!busy} className="rounded px-2 py-0.5 text-xs text-dim hover:bg-panel-2 hover:text-ink disabled:opacity-40">
                  {busy === o.id ? <Loader2 size={12} className="animate-spin" /> : "Cerrar sesión"}
                </button>
              </li>
            ))}
          </ul>
        )}
        <button onClick={() => settings("emailandaccounts")} className="mt-3 flex items-center gap-1 text-xs text-mute hover:text-ink">
          <ExternalLink size={11} /> Correo y cuentas de Windows (para quitar las de Microsoft)
        </button>
      </Card>

      {/* Credenciales guardadas */}
      <Card
        title={`Credenciales guardadas · ${s.credentials.length}`}
        icon={<KeyRound size={14} />}
        className="col-span-12"
        right={
          <button onClick={() => setShowCreds(!showCreds)} className="text-[11px] text-neon hover:underline">
            {showCreds ? "Ocultar" : "Mostrar"}
          </button>
        }
      >
        <p className="text-xs text-dim">
          Contraseñas que Windows recuerda (carpetas de red, impresoras, Escritorio remoto, Outlook…). Borrar la de un sitio arregla el típico «sigue usando la contraseña
          antigua».
        </p>
        {showCreds && (
          <>
            <div className="relative mt-3">
              <Search size={14} className="absolute top-2.5 left-3 text-mute" />
              <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Buscar servidor, carpeta o usuario…" className={`${inputClass} pl-9`} />
            </div>
            <ul className="mt-2 pane-md overflow-y-auto">
              {creds.map((c) => (
                <li key={c.target} className="flex items-center gap-2 border-t border-line/60 py-1.5 text-xs">
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-mono text-ink" title={c.target}>
                      {c.target}
                    </span>
                    <span className="text-mute">
                      {c.kind}
                      {c.user && ` · ${c.user}`}
                    </span>
                  </span>
                  <button
                    onClick={async () => {
                      if (await confirm({ title: "Borrar credencial", body: `Windows olvidará la contraseña guardada para «${c.target}» y la pedirá la próxima vez.`, confirmLabel: "Borrar", danger: true }))
                        await run(c.target, async () => {
                          await accountsApi.deleteCredential(c.target);
                          return "Credencial borrada.";
                        });
                    }}
                    disabled={!!busy}
                    className="rounded p-1 text-mute hover:text-bad disabled:opacity-40"
                    title="Borrar"
                  >
                    {busy === c.target ? <Loader2 size={12} className="animate-spin" /> : <Trash2 size={12} />}
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
      </Card>
      <WindowsTools
        className="col-span-12"
        links={[
          { id: "netplwiz", what: "Cuentas del equipo e inicio de sesión automático." },
          { id: "lusrmgr", what: "Usuarios y grupos locales al completo." },
          { id: "user-profiles", what: "Perfiles guardados: ver y borrar los que sobran." },
          { id: "credentials", what: "Contraseñas guardadas de Windows, redes y sitios." },
        ]}
      />
      {dialog}
    </div>
  );
}
