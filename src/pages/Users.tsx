import { Eye, EyeOff, KeyRound, Loader2, Pencil, RefreshCw, Search, ShieldAlert, ShieldCheck, ShieldOff, Trash2, TriangleAlert, UserCheck, UserPlus, UserRound, UserX } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Avatar } from "../components/contacts/Avatar";
import { useToast } from "../components/feedback";
import { Button, EmptyLine, EmptyState, ErrorState, Loading, Modal, Tile, inputClass } from "../components/ui";
import { bytes } from "../lib/format";
import { usersApi, type LocalUser, type NewUser } from "../lib/api";
import { WindowsTools } from "../components/WindowsTools";
import { useLiveEffect } from "../lib/useLiveEffect";
import { userIssues, worstIssue, type UserIssue } from "../lib/userIssues";

type Action = "delete" | "disable" | "enable" | "demote" | "promote" | "password" | "rename";

const BUILTIN_LABEL = { administrator: "Administrador integrado", guest: "Invitado", default: "Cuenta de Windows", wdag: "Cuenta de Windows" };
const FORBIDDEN = /["/\\[\]:;|=,+*?<>@]/;

/** Mismas reglas que el backend (users.rs → guard), para desactivar los botones con el motivo. */
function blockReason(users: LocalUser[], u: LocalUser, action: Action): string | null {
  if (u.builtin === "default" || u.builtin === "wdag") return "Cuenta interna de Windows.";
  const removesAdmin = u.admin && u.enabled && (action === "delete" || action === "disable" || action === "demote");
  if (removesAdmin && !users.some((x) => x.sid !== u.sid && x.admin && x.enabled))
    return "Es el único administrador activo: el equipo se quedaría sin administrador.";
  if ((action === "delete" || action === "disable" || action === "demote") && u.isSelf) return "Es la cuenta que está usando AdminOps.";
  if ((action === "delete" || action === "disable") && (u.isTarget || u.signedIn)) return "Tiene la sesión iniciada.";
  if (action === "delete" && u.builtin) return "Las cuentas integradas se pueden desactivar, no eliminar.";
  if ((action === "promote" || action === "demote") && u.builtin === "guest") return "Invitado no puede ser administrador.";
  if (action === "password" && u.microsoft) return "Cuenta de Microsoft: la contraseña se cambia en account.microsoft.com.";
  if (action === "rename" && u.builtin) return "Las cuentas integradas de Windows no se renombran.";
  if (action === "rename" && u.microsoft) return "Cuenta de Microsoft: el nombre se cambia en account.microsoft.com.";
  if (action === "rename" && (u.signedIn || u.isTarget || u.isSelf)) return "Tiene la sesión iniciada: ciérrala antes de renombrarla.";
  return null;
}

function nameError(name: string): string | null {
  const n = name.trim();
  if (!n) return "Escribe un nombre de usuario.";
  if ([...n].length > 20) return "Máximo 20 caracteres.";
  const bad = n.match(FORBIDDEN);
  if (bad) return `No puede contener «${bad[0]}».`;
  if (/^[. ]+$/.test(n)) return "No puede ser solo puntos o espacios.";
  if (n.endsWith(".")) return "No puede terminar en punto.";
  return null;
}

const when = (iso: string | null) =>
  iso ? new Date(iso).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" }) : null;

/** «hace 3 días», para la lista: la fecha exacta está en la ficha. */
function ago(iso: string | null): string | null {
  if (!iso) return null;
  const days = Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000);
  if (days <= 0) return "hoy";
  if (days === 1) return "ayer";
  if (days < 60) return `hace ${days} días`;
  if (days < 730) return `hace ${Math.floor(days / 30)} meses`;
  return `hace ${Math.floor(days / 365)} años`;
}

type Filter = "all" | "admins" | "disabled" | "review";

export function Users({ isAdmin }: { isAdmin: boolean }) {
  const [users, setUsers] = useState<LocalUser[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [showSystem, setShowSystem] = useState(false);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [selected, setSelected] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [password, setPassword] = useState<LocalUser | null>(null);
  const [renaming, setRenaming] = useState<LocalUser | null>(null);
  const [deleting, setDeleting] = useState<LocalUser | null>(null);
  const toast = useToast();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setUsers(await usersApi.list());
      setFailed(null);
    } catch (e) {
      setFailed(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  /** Lo que conviene revisar de cada cuenta, por SID. */
  const issues = useMemo(() => new Map((users ?? []).map((u) => [u.sid, userIssues(u)])), [users]);

  // Las cuentas internas de Windows no se enseñan salvo que se pidan… o que
  // estén activas, que es justo cuando importa verlas.
  const base = useMemo(() => (users ?? []).filter((u) => showSystem || !u.builtin || u.enabled), [users, showSystem]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return base.filter(
      (u) =>
        (filter === "all" || (filter === "admins" && u.admin && u.enabled) || (filter === "disabled" && !u.enabled) || (filter === "review" && (issues.get(u.sid)?.length ?? 0) > 0)) &&
        (!q || `${u.name} ${u.fullName} ${u.description}`.toLowerCase().includes(q)),
    );
  }, [base, query, filter, issues]);

  // Siempre hay alguien abierto: quien usa el equipo, o el primero de la lista.
  const current = visible.find((u) => u.sid === selected) ?? visible.find((u) => u.isTarget) ?? visible[0] ?? null;

  const run = async (u: LocalUser, label: string, op: () => Promise<void>) => {
    setBusy(u.sid);
    try {
      await op();
      toast("ok", label);
      await load();
    } catch (e) {
      toast("error", `${u.name}: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  if (!users) return failed ? <ErrorState page message={failed} onRetry={() => void load()} /> : <Loading page text="Leyendo usuarios del equipo…" />;

  const hidden = users.length - base.length;
  const people = users.filter((u) => !u.builtin);
  const toReview = base.filter((u) => (issues.get(u.sid)?.length ?? 0) > 0).length;
  const toggle = (f: Filter) => setFilter((cur) => (cur === f ? "all" : f));

  return (
    <div className="@container mx-auto max-w-6xl space-y-4 p-6">
      <div className="grid grid-cols-2 gap-2 @2xl:grid-cols-4">
        <Tile label={people.length === 1 ? "Usuario" : "Usuarios"} value={people.length} />
        <Tile label="Administradores activos" value={users.filter((u) => u.admin && u.enabled).length} active={filter === "admins"} onClick={() => toggle("admins")} />
        <Tile label="Desactivadas" value={base.filter((u) => !u.enabled).length} active={filter === "disabled"} onClick={() => toggle("disabled")} />
        <Tile label="Con algo que revisar" value={toReview} warn active={filter === "review"} onClick={() => toggle("review")} />
      </div>

      {failed && <ErrorState message={failed} onRetry={() => void load()} />}

      {!isAdmin && (
        <p className="flex items-center gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn">
          <TriangleAlert size={13} className="shrink-0" /> Sin administrador solo puedes ver los usuarios. Para crearlos o modificarlos, reinicia AdminOps como administrador.
        </p>
      )}

      <div className="grid grid-cols-12 gap-4">
        <div className="col-span-12 @3xl:col-span-5">
          <div className="flex items-center gap-2">
            <div className="relative min-w-0 flex-1">
              <Search size={14} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
              <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Buscar un usuario" className={`${inputClass} pl-8`} aria-label="Buscar un usuario" />
            </div>
            <button onClick={() => void load()} disabled={loading} className="rounded-md p-2 text-dim transition-colors hover:bg-panel-2 hover:text-ink" title="Volver a leer">
              <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
            </button>
            <Button onClick={() => setCreating(true)} disabled={!isAdmin} title={isAdmin ? undefined : "Requiere ejecutar AdminOps como administrador"}>
              <UserPlus size={14} /> Nuevo
            </Button>
          </div>

          <div className="mt-3 overflow-hidden rounded-xl border border-line bg-panel">
            {visible.length === 0 ? (
              <EmptyLine>{query ? `Nadie coincide con «${query}».` : "Ninguna cuenta con ese filtro."}</EmptyLine>
            ) : (
              <ul className="divide-y divide-line/60">
                {visible.map((u) => {
                  const worst = worstIssue(issues.get(u.sid) ?? []);
                  return (
                    <li key={u.sid}>
                      <button
                        onClick={() => setSelected(u.sid)}
                        className={`flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors ${current?.sid === u.sid ? "bg-neon/10" : "hover:bg-panel-2"} ${u.enabled ? "" : "opacity-60"}`}
                      >
                        {u.builtin ? (
                          <span className="grid size-9 shrink-0 place-items-center rounded-full border border-line bg-void/60 text-dim">
                            <UserRound size={15} />
                          </span>
                        ) : (
                          <Avatar c={{ name: u.fullName || u.name, favorite: false }} size={36} />
                        )}
                        <span className="min-w-0 flex-1">
                          <span className="flex items-center gap-1.5">
                            <span className="truncate text-sm text-ink">{u.name}</span>
                            {u.admin && <ShieldCheck size={12} className="shrink-0 text-neon" aria-label="Administrador" />}
                          </span>
                          <span className="block truncate text-[11px] text-mute">
                            {!u.enabled ? "Desactivada" : u.signedIn ? "Sesión iniciada" : ago(u.lastLogon) ? `Entró ${ago(u.lastLogon)}` : u.hasProfile ? "Sin fecha de último inicio" : "Nunca ha entrado"}
                          </span>
                        </span>
                        {worst && <span className={`size-2 shrink-0 rounded-full ${worst === "bad" ? "bg-bad" : "bg-warn"}`} title="Tiene algo que revisar" />}
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>

          <label className="mt-2 flex items-center gap-1.5 text-xs text-dim">
            <input type="checkbox" checked={showSystem} onChange={(e) => setShowSystem(e.target.checked)} className="accent-[var(--color-neon)]" />
            Mostrar cuentas del sistema{hidden > 0 && !showSystem ? ` (${hidden})` : ""}
          </label>
        </div>

        <div className="col-span-12 @3xl:col-span-7">
          {current ? (
            <UserCard
              key={current.sid}
              u={current}
              issues={issues.get(current.sid) ?? []}
              working={busy === current.sid}
              reason={(a) => (!isAdmin ? "Requiere administrador." : blockReason(users, current, a))}
              onRename={() => setRenaming(current)}
              onPassword={() => setPassword(current)}
              onDelete={() => setDeleting(current)}
              onAdmin={(admin) => void run(current, admin ? `${current.name} ahora es administrador.` : `${current.name} ahora es usuario estándar.`, () => usersApi.setAdmin(current.sid, admin))}
              onEnabled={(enabled) => void run(current, enabled ? `Cuenta ${current.name} activada.` : `Cuenta ${current.name} desactivada.`, () => usersApi.setEnabled(current.sid, enabled))}
            />
          ) : (
            <EmptyState icon={<UserRound size={28} />} title="Elige una cuenta">
              Su estado, lo que conviene revisar y lo que se puede hacer con ella.
            </EmptyState>
          )}
        </div>
      </div>

      <p className="text-xs text-mute">
        AdminOps nunca deja el equipo sin un administrador activo ni permite borrar la cuenta con la sesión abierta. Las contraseñas no se guardan en el historial ni en el registro de
        actividad.
      </p>

      <WindowsTools
        links={[
          { id: "lusrmgr", what: "Opciones de contraseña, grupos y desbloquear una cuenta (renombrar y editar sus datos ya se hace aquí)." },
          { id: "netplwiz", what: "Inicio de sesión automático y grupo de cada cuenta." },
          { id: "user-profiles", what: "Perfiles guardados en el equipo: tamaño, tipo y borrar los que sobran." },
          { id: "secpol", what: "Longitud y caducidad de las contraseñas, bloqueo tras varios intentos." },
          { id: "taskmgr", what: "Pestaña Usuarios: quién tiene la sesión abierta, cerrarla o enviarle un mensaje." },
          { id: "compmgmt", what: "Administración del equipo: usuarios, carpetas compartidas y sesiones abiertas." },
        ]}
      />

      {creating && (
        <CreateUser
          onClose={() => setCreating(false)}
          onCreated={(name) => {
            setCreating(false);
            toast("ok", `Usuario ${name} creado.`);
            void load();
          }}
        />
      )}
      {renaming && (
        <RenameDialog
          user={renaming}
          onClose={() => setRenaming(null)}
          onDone={(aviso) => {
            setRenaming(null);
            toast("ok", aviso);
            void load();
          }}
        />
      )}
      {password && (
        <PasswordDialog
          user={password}
          onClose={() => setPassword(null)}
          onDone={() => {
            toast("ok", `Contraseña de ${password.name} actualizada.`);
            setPassword(null);
            void load();
          }}
        />
      )}
      {deleting && (
        <DeleteUser
          user={deleting}
          onClose={() => setDeleting(null)}
          onDeleted={() => {
            toast("ok", `Usuario ${deleting.name} eliminado.`);
            setDeleting(null);
            setSelected(null);
            void load();
          }}
        />
      )}
    </div>
  );
}

/** La ficha de una cuenta: qué pasa con ella arriba, sus datos y lo que se puede hacer. */
function UserCard({
  u,
  issues,
  working,
  reason,
  onRename,
  onPassword,
  onDelete,
  onAdmin,
  onEnabled,
}: {
  u: LocalUser;
  issues: UserIssue[];
  working: boolean;
  /** Por qué no se puede hacer algo (null: se puede). */
  reason: (a: Action) => string | null;
  onRename: () => void;
  onPassword: () => void;
  onDelete: () => void;
  onAdmin: (admin: boolean) => void;
  onEnabled: (enabled: boolean) => void;
}) {
  const fix = (i: UserIssue) => {
    if (i.fix === "password" && !reason("password"))
      return (
        <Button onClick={onPassword} disabled={working}>
          <KeyRound size={14} /> Cambiar contraseña
        </Button>
      );
    if (i.fix === "disable" && !reason("disable"))
      return (
        <Button onClick={() => onEnabled(false)} disabled={working}>
          <UserX size={14} /> Desactivar
        </Button>
      );
    return null;
  };

  return (
    <div className="space-y-3">
      {issues.map((i) => (
        <div key={i.text} className={`flex flex-wrap items-center gap-3 rounded-xl border px-4 py-3 text-sm ${i.level === "bad" ? "border-bad/40 bg-bad/10 text-bad" : "border-warn/40 bg-warn/10 text-warn"}`}>
          <ShieldAlert size={16} className="shrink-0" />
          <span className="min-w-0 flex-1">{i.text}</span>
          {fix(i)}
        </div>
      ))}

      <section className="rounded-xl border border-line bg-panel p-4">
        <div className="flex flex-wrap items-center gap-3">
          {u.builtin ? (
            <span className="grid size-12 shrink-0 place-items-center rounded-full border border-line bg-void/60 text-dim">
              <UserRound size={20} />
            </span>
          ) : (
            <Avatar c={{ name: u.fullName || u.name, favorite: false }} size={48} />
          )}
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-baseline gap-x-2">
              <h2 className="text-base font-semibold text-ink">{u.name}</h2>
              {u.fullName && u.fullName !== u.name && <span className="text-sm text-dim">{u.fullName}</span>}
            </div>
            <div className="mt-1 flex flex-wrap gap-1.5">
              <Badge tone={u.admin ? "neon" : "mute"}>{u.admin ? "Administrador" : "Estándar"}</Badge>
              {u.microsoft && <Badge tone="mute">Cuenta Microsoft</Badge>}
              {u.builtin && <Badge tone="mute">{BUILTIN_LABEL[u.builtin]}</Badge>}
              {!u.enabled && <Badge tone="warn">Desactivada</Badge>}
              {u.signedIn && <Badge tone="ok">Sesión iniciada</Badge>}
              {u.isSelf && <Badge tone="neon">Ejecuta AdminOps</Badge>}
            </div>
          </div>
          {working && <Loader2 size={18} className="animate-spin text-neon" />}
        </div>

        {u.description && <p className="mt-3 text-sm text-dim">{u.description}</p>}

        <dl className="mt-4 grid grid-cols-2 gap-2 @2xl:grid-cols-4">
          <Fact label="Último inicio">{when(u.lastLogon) ?? (u.signedIn || u.hasProfile ? "Sin registrar" : "Nunca")}</Fact>
          <Fact label="Contraseña cambiada">{u.microsoft ? "La gestiona Microsoft" : (when(u.passwordLastSet) ?? "Nunca")}</Fact>
          <Fact label="Caduca">{when(u.passwordExpires) ?? "No caduca"}</Fact>
          <Fact label="Carpeta personal">{u.hasProfile ? "Sí, en este equipo" : "Aún no (no ha entrado)"}</Fact>
        </dl>

        <div className="mt-4 flex flex-wrap gap-2 border-t border-line/60 pt-4">
          <CardAction icon={KeyRound} label="Cambiar contraseña" reason={reason("password")} disabled={working} onClick={onPassword} />
          <CardAction icon={Pencil} label="Editar sus datos" reason={reason("rename")} disabled={working} onClick={onRename} />
          {u.admin ? (
            <CardAction icon={ShieldOff} label="Pasar a estándar" reason={reason("demote")} disabled={working} onClick={() => onAdmin(false)} />
          ) : (
            <CardAction icon={ShieldCheck} label="Hacer administrador" reason={reason("promote")} disabled={working} onClick={() => onAdmin(true)} />
          )}
          {u.enabled ? (
            <CardAction icon={UserX} label="Desactivar" reason={reason("disable")} disabled={working} onClick={() => onEnabled(false)} />
          ) : (
            <CardAction icon={UserCheck} label="Activar" reason={reason("enable")} disabled={working} onClick={() => onEnabled(true)} />
          )}
          <CardAction icon={Trash2} label="Eliminar" danger reason={reason("delete")} disabled={working} onClick={onDelete} />
        </div>
      </section>
    </div>
  );
}

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="min-w-0 rounded-lg bg-panel-2/50 px-3 py-2">
      <dt className="text-[10px] tracking-wide text-mute uppercase">{label}</dt>
      <dd className="mt-0.5 truncate text-xs text-ink">{children}</dd>
    </div>
  );
}

function Badge({ tone, children }: { tone: "neon" | "mute" | "warn" | "ok"; children: React.ReactNode }) {
  const c = { neon: "border-neon/40 text-neon", mute: "border-line text-mute", warn: "border-warn/40 text-warn", ok: "border-ok/40 text-ok" }[tone];
  return <span className={`rounded border px-1.5 py-px text-[11px] ${c}`}>{children}</span>;
}

/** Botón de la ficha. Si no se puede, queda apagado y dice por qué al pasar el ratón. */
function CardAction({
  icon: Icon,
  label,
  reason,
  danger,
  disabled,
  onClick,
}: {
  icon: React.ComponentType<{ size?: number }>;
  label: string;
  reason: string | null;
  danger?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      disabled={!!reason || disabled}
      title={reason ?? undefined}
      className={`flex h-9 items-center gap-1.5 rounded-lg border px-3 text-[13px] transition-colors disabled:cursor-not-allowed disabled:opacity-35 ${
        danger ? "border-bad/40 text-bad hover:bg-bad/10" : "border-line-2 text-dim hover:bg-panel-2 hover:text-ink"
      }`}
    >
      <Icon size={14} /> {label}
    </button>
  );
}

function PasswordFields({
  value,
  confirm,
  onChange,
  onConfirm,
}: {
  value: string;
  confirm: string;
  onChange: (v: string) => void;
  onConfirm: (v: string) => void;
}) {
  const [show, setShow] = useState(false);
  return (
    <div className="grid grid-cols-2 gap-2">
      <div className="relative">
        <input
          type={show ? "text" : "password"}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="Contraseña"
          autoComplete="new-password"
          className={`${inputClass} pr-8`}
        />
        <button type="button" onClick={() => setShow(!show)} className="absolute top-1/2 right-2 -translate-y-1/2 text-mute hover:text-ink" title={show ? "Ocultar" : "Mostrar"}>
          {show ? <EyeOff size={13} /> : <Eye size={13} />}
        </button>
      </div>
      <input
        type={show ? "text" : "password"}
        value={confirm}
        onChange={(e) => onConfirm(e.target.value)}
        placeholder="Repetir contraseña"
        autoComplete="new-password"
        className={inputClass}
      />
    </div>
  );
}

function Check({ checked, disabled, onChange, children }: { checked: boolean; disabled?: boolean; onChange: (v: boolean) => void; children: React.ReactNode }) {
  return (
    <label className={`flex items-start gap-2 text-sm ${disabled ? "text-mute" : "text-dim"}`}>
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} className="mt-0.5 accent-[var(--color-neon)]" />
      <span>{children}</span>
    </label>
  );
}

function CreateUser({ onClose, onCreated }: { onClose: () => void; onCreated: (name: string) => void }) {
  const [u, setU] = useState<NewUser>({ name: "", fullName: "", password: "", admin: false, passwordNeverExpires: true, mustChange: false });
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const nameErr = u.name ? nameError(u.name) : null;
  const mismatch = u.password !== confirm;
  const valid = !!u.name.trim() && !nameErr && !mismatch;

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await usersApi.create({ ...u, name: u.name.trim() });
      onCreated(u.name.trim());
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title="Nuevo usuario local"
      onClose={onClose}
      width="w-[520px]"
      footer={
        <>
          {error && <p className="mr-auto max-w-72 text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save} disabled={!valid || saving}>
            {saving ? <Loader2 size={14} className="animate-spin" /> : <UserPlus size={14} />} Crear usuario
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="grid grid-cols-2 gap-2">
          <label>
            <span className="mb-1 block text-xs text-dim">Nombre de usuario</span>
            <input autoFocus value={u.name} onChange={(e) => setU({ ...u, name: e.target.value })} maxLength={20} placeholder="maria" className={inputClass} />
          </label>
          <label>
            <span className="mb-1 block text-xs text-dim">Nombre completo (opcional)</span>
            <input value={u.fullName} onChange={(e) => setU({ ...u, fullName: e.target.value })} maxLength={64} placeholder="María García" className={inputClass} />
          </label>
        </div>
        {nameErr && <p className="text-xs text-bad">{nameErr}</p>}

        <div>
          <span className="mb-1 block text-xs text-dim">Contraseña</span>
          <PasswordFields value={u.password} confirm={confirm} onChange={(password) => setU({ ...u, password })} onConfirm={setConfirm} />
          {mismatch && confirm && <p className="mt-1 text-xs text-bad">Las contraseñas no coinciden.</p>}
          {!u.password && <p className="mt-1 text-xs text-warn">Sin contraseña, cualquiera con acceso al equipo podrá entrar en esta cuenta.</p>}
        </div>

        <div className="grid grid-cols-2 gap-2">
          {[false, true].map((admin) => (
            <button
              key={String(admin)}
              onClick={() => setU({ ...u, admin })}
              className={`rounded-lg border px-3 py-2 text-left transition-colors ${u.admin === admin ? "border-neon/60 bg-neon/10" : "border-line hover:border-line-2"}`}
            >
              <div className={`text-sm font-medium ${u.admin === admin ? "text-neon" : "text-ink"}`}>{admin ? "Administrador" : "Estándar"}</div>
              <div className="text-[11px] text-mute">{admin ? "Puede instalar programas y cambiar la configuración." : "Recomendado para el uso diario."}</div>
            </button>
          ))}
        </div>

        <div className="space-y-1.5">
          <Check
            checked={u.passwordNeverExpires && !u.mustChange}
            disabled={!u.password || u.mustChange}
            onChange={(passwordNeverExpires) => setU({ ...u, passwordNeverExpires })}
          >
            La contraseña no caduca <span className="text-mute">(como las cuentas creadas desde Configuración)</span>
          </Check>
          <Check checked={u.mustChange && !!u.password} disabled={!u.password} onChange={(mustChange) => setU({ ...u, mustChange })}>
            Pedir que la cambie al iniciar sesión por primera vez
          </Check>
        </div>
      </div>
    </Modal>
  );
}

function PasswordDialog({ user, onClose, onDone }: { user: LocalUser; onClose: () => void; onDone: () => void }) {
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const [mustChange, setMustChange] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await usersApi.setPassword(user.sid, pw, mustChange && !!pw);
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title={`Contraseña de ${user.name}`}
      onClose={onClose}
      footer={
        <>
          {error && <p className="mr-auto max-w-60 text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save} disabled={pw !== confirm || saving}>
            {saving && <Loader2 size={14} className="animate-spin" />} {pw ? "Cambiar contraseña" : "Quitar contraseña"}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <PasswordFields value={pw} confirm={confirm} onChange={setPw} onConfirm={setConfirm} />
        {pw !== confirm && confirm && <p className="text-xs text-bad">Las contraseñas no coinciden.</p>}
        {!pw && <p className="text-xs text-warn">Déjala vacía para quitar la contraseña: la cuenta entrará sin pedirla.</p>}
        <Check checked={mustChange && !!pw} disabled={!pw} onChange={setMustChange}>
          Pedir que la cambie al iniciar sesión
        </Check>
        <p className="text-[11px] text-mute">
          Aviso: si el usuario tenía archivos cifrados con EFS o contraseñas guardadas en Windows, dejarán de estar accesibles tras
          restablecer la contraseña.
        </p>
      </div>
    </Modal>
  );
}

function DeleteUser({ user, onClose, onDeleted }: { user: LocalUser; onClose: () => void; onDeleted: () => void }) {
  const [profile, setProfile] = useState<{ exists: boolean; size: number; files: number } | null>(null);
  const [withProfile, setWithProfile] = useState(false);
  const [typed, setTyped] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Al cambiar de usuario, el tamaño del perfil anterior ya no vale.
  useLiveEffect(
    (vigente) => {
      usersApi
        .profileSize(user.sid)
        .then((p) => vigente() && setProfile(p))
        .catch(() => vigente() && setProfile({ exists: false, size: 0, files: 0 }));
    },
    [user.sid],
  );

  const remove = async () => {
    setDeleting(true);
    setError(null);
    try {
      await usersApi.remove(user.sid, withProfile);
      onDeleted();
    } catch (e) {
      setError(String(e));
      setDeleting(false);
    }
  };

  // Borrar datos exige escribir el nombre: no se puede deshacer.
  const confirmed = !withProfile || typed.trim().toLowerCase() === user.name.toLowerCase();

  return (
    <Modal
      title={`Eliminar a ${user.name}`}
      onClose={deleting ? () => {} : onClose}
      footer={
        <>
          {error && <p className="mr-auto max-w-60 text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose} disabled={deleting}>
            Cancelar
          </Button>
          <Button kind="danger" onClick={remove} disabled={!confirmed || deleting || !profile}>
            {deleting ? <Loader2 size={14} className="animate-spin" /> : <Trash2 size={14} />} Eliminar
          </Button>
        </>
      }
    >
      <div className="space-y-3 text-sm text-dim">
        <p>
          Se eliminará la cuenta <span className="font-medium text-ink">{user.name}</span>
          {user.admin && " (administrador)"}. Esto no se puede deshacer.
        </p>
        {!profile ? (
          <p className="flex items-center gap-2 text-xs text-mute">
            <Loader2 size={12} className="animate-spin" /> Calculando el tamaño de su perfil…
          </p>
        ) : profile.exists ? (
          <>
            <Check checked={withProfile} onChange={setWithProfile}>
              Borrar también su carpeta de perfil:{" "}
              <span className="font-mono text-ink">{bytes(profile.size)}</span> en {profile.files.toLocaleString("es")} archivos (Escritorio,
              Documentos, Descargas, Imágenes…)
            </Check>
            {!withProfile && <p className="text-xs text-mute">La carpeta quedará en el disco y podrás copiar sus archivos o borrarla más tarde.</p>}
            {withProfile && (
              <label className="block">
                <span className="mb-1 block text-xs text-bad">Sus archivos se borrarán para siempre. Escribe «{user.name}» para confirmar:</span>
                <input autoFocus value={typed} onChange={(e) => setTyped(e.target.value)} className={inputClass} />
              </label>
            )}
          </>
        ) : (
          <p className="text-xs text-mute">No tiene carpeta de perfil (nunca inició sesión).</p>
        )}
      </div>
    </Modal>
  );
}

/**
 * Renombrar la cuenta y editar sus datos. Hasta ahora había que salir a
 * `lusrmgr.msc` para algo tan corriente como corregir el nombre de un usuario.
 */
function RenameDialog({ user, onClose, onDone }: { user: LocalUser; onClose: () => void; onDone: (aviso: string) => void }) {
  const [name, setName] = useState(user.name);
  const [fullName, setFullName] = useState(user.fullName);
  const [description, setDescription] = useState(user.description);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const cambiaNombre = name.trim().toLowerCase() !== user.name.toLowerCase();
  const problema = cambiaNombre ? nameError(name) : null;

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      onDone(await usersApi.rename(user.sid, name.trim(), fullName, description));
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  };

  return (
    <Modal
      title={`Editar «${user.name}»`}
      onClose={onClose}
      width="w-[460px]"
      footer={
        <>
          {error && <p className="mr-auto max-w-56 text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save} disabled={saving || !!problema || !name.trim()}>
            {saving && <Loader2 size={14} className="animate-spin" />} Guardar
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Nombre de la cuenta</span>
          <input value={name} onChange={(e) => setName(e.target.value)} maxLength={20} className={inputClass} />
          {problema && <span className="mt-1 block text-[11px] text-bad">{problema}</span>}
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Nombre completo</span>
          <input value={fullName} onChange={(e) => setFullName(e.target.value)} maxLength={256} placeholder="María Pérez" className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Descripción</span>
          <input value={description} onChange={(e) => setDescription(e.target.value)} maxLength={256} placeholder="Recepción · segunda planta" className={inputClass} />
        </label>
        {cambiaNombre && user.hasProfile && (
          <p className="flex items-start gap-2 rounded-lg border border-warn/40 bg-warn/10 p-2.5 text-[11px] text-warn">
            <TriangleAlert size={13} className="mt-0.5 shrink-0" />
            Su carpeta personal seguirá llamándose «{user.name}»: Windows no la renombra al cambiar la cuenta. Todo funciona igual, y
            cambiarla a mano rompe el perfil.
          </p>
        )}
      </div>
    </Modal>
  );
}
