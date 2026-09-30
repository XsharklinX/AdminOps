import { Eye, EyeOff, KeyRound, Loader2, Pencil, RefreshCw, ShieldCheck, ShieldOff, Trash2, TriangleAlert, UserCheck, UserPlus, UserRound, UserX } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useToast } from "../components/feedback";
import { Button, Modal, inputClass } from "../components/ui";
import { bytes } from "../lib/format";
import { usersApi, type LocalUser, type NewUser } from "../lib/api";
import { WindowsTools } from "../components/WindowsTools";
import { useLiveEffect } from "../lib/useLiveEffect";

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

export function Users({ isAdmin }: { isAdmin: boolean }) {
  const [users, setUsers] = useState<LocalUser[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [showSystem, setShowSystem] = useState(false);
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
    } catch (e) {
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  const visible = useMemo(
    () => (users ?? []).filter((u) => showSystem || !u.builtin || u.enabled),
    [users, showSystem],
  );

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

  if (!users) return <p className="p-8 font-mono text-sm text-mute">Leyendo usuarios del equipo…</p>;

  const hidden = users.length - visible.length;
  const people = users.filter((u) => !u.builtin);

  return (
    <div className="mx-auto max-w-5xl p-6">
      <div className="mb-4 flex items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{people.length}</span> {people.length === 1 ? "usuario" : "usuarios"} ·{" "}
          <span className="font-mono text-neon">{people.filter((u) => u.admin).length}</span> administradores
        </p>
        <label className="ml-auto flex items-center gap-1.5 text-xs text-dim">
          <input type="checkbox" checked={showSystem} onChange={(e) => setShowSystem(e.target.checked)} className="accent-[var(--color-neon)]" />
          Mostrar cuentas del sistema{hidden > 0 && !showSystem ? ` (${hidden})` : ""}
        </label>
        <button onClick={load} disabled={loading} className="rounded-md p-1.5 text-dim transition-colors hover:bg-panel-2 hover:text-ink" title="Volver a leer">
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
        <Button onClick={() => setCreating(true)} disabled={!isAdmin} title={isAdmin ? undefined : "Requiere ejecutar AdminOps como administrador"}>
          <UserPlus size={14} /> Nuevo usuario
        </Button>
      </div>

      {!isAdmin && (
        <p className="mb-4 flex items-center gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn">
          <TriangleAlert size={13} /> Sin administrador solo puedes ver los usuarios. Para crearlos o modificarlos, reinicia AdminOps como administrador.
        </p>
      )}

      <div className="overflow-hidden rounded-xl border border-line bg-panel">
        {visible.map((u, i) => {
          const reason = (a: Action) => (!isAdmin ? "Requiere administrador." : blockReason(users, u, a));
          const working = busy === u.sid;
          return (
            <div key={u.sid} className={`flex items-center gap-4 px-4 py-3 ${i > 0 ? "border-t border-line/70" : ""} ${u.enabled ? "" : "opacity-60"}`}>
              <span
                className={`grid size-9 shrink-0 place-items-center rounded-full border text-sm font-semibold ${
                  u.admin ? "border-neon/50 bg-neon/10 text-neon" : "border-line bg-void/60 text-dim"
                }`}
              >
                {u.builtin ? <UserRound size={15} /> : u.name.slice(0, 1).toUpperCase()}
              </span>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="text-sm font-medium text-ink">{u.name}</span>
                  {u.fullName && u.fullName !== u.name && <span className="text-xs text-mute">{u.fullName}</span>}
                  <Badge tone={u.admin ? "neon" : "mute"}>{u.admin ? "Administrador" : "Estándar"}</Badge>
                  {u.microsoft && <Badge tone="mute">Cuenta Microsoft</Badge>}
                  {u.builtin && <Badge tone="mute">{BUILTIN_LABEL[u.builtin]}</Badge>}
                  {!u.enabled && <Badge tone="warn">Desactivada</Badge>}
                  {u.signedIn && <Badge tone="ok">Sesión iniciada</Badge>}
                  {u.isSelf && <Badge tone="neon">Ejecuta AdminOps</Badge>}
                </div>
                <div className="mt-0.5 text-[11px] text-mute">
                  {when(u.lastLogon)
                    ? `Último inicio: ${when(u.lastLogon)}`
                    : u.signedIn || u.hasProfile
                      ? "Último inicio: sin registrar"
                      : "Nunca ha iniciado sesión"}
                  {u.passwordLastSet && ` · Contraseña cambiada: ${when(u.passwordLastSet)}`}
                  {u.passwordExpires && ` · Caduca: ${when(u.passwordExpires)}`}
                </div>
              </div>
              {working ? (
                <Loader2 size={16} className="animate-spin text-neon" />
              ) : (
                <div className="flex shrink-0 items-center gap-0.5">
                  <IconAction icon={Pencil} label="Renombrar o editar sus datos" reason={reason("rename")} onClick={() => setRenaming(u)} />
                  <IconAction icon={KeyRound} label="Cambiar contraseña" reason={reason("password")} onClick={() => setPassword(u)} />
                  {u.admin ? (
                    <IconAction
                      icon={ShieldOff}
                      label="Convertir en usuario estándar"
                      reason={reason("demote")}
                      onClick={() => run(u, `${u.name} ahora es usuario estándar.`, () => usersApi.setAdmin(u.sid, false))}
                    />
                  ) : (
                    <IconAction
                      icon={ShieldCheck}
                      label="Hacer administrador"
                      reason={reason("promote")}
                      onClick={() => run(u, `${u.name} ahora es administrador.`, () => usersApi.setAdmin(u.sid, true))}
                    />
                  )}
                  {u.enabled ? (
                    <IconAction
                      icon={UserX}
                      label="Desactivar cuenta"
                      reason={reason("disable")}
                      onClick={() => run(u, `Cuenta ${u.name} desactivada.`, () => usersApi.setEnabled(u.sid, false))}
                    />
                  ) : (
                    <IconAction
                      icon={UserCheck}
                      label="Activar cuenta"
                      reason={reason("enable")}
                      onClick={() => run(u, `Cuenta ${u.name} activada.`, () => usersApi.setEnabled(u.sid, true))}
                    />
                  )}
                  <IconAction icon={Trash2} label="Eliminar usuario" danger reason={reason("delete")} onClick={() => setDeleting(u)} />
                </div>
              )}
            </div>
          );
        })}
      </div>
      <p className="mt-3 text-xs text-mute">
        AdminOps nunca deja el equipo sin un administrador activo ni permite borrar la cuenta con la sesión abierta. Las contraseñas no
        se guardan en el historial ni en el registro de actividad.
      </p>

      <WindowsTools
        className="mt-4"
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
            void load();
          }}
        />
      )}
    </div>
  );
}

function Badge({ tone, children }: { tone: "neon" | "mute" | "warn" | "ok"; children: React.ReactNode }) {
  const c = { neon: "border-neon/40 text-neon", mute: "border-line text-mute", warn: "border-warn/40 text-warn", ok: "border-ok/40 text-ok" }[tone];
  return <span className={`rounded border px-1.5 py-px text-[11px] ${c}`}>{children}</span>;
}

function IconAction({
  icon: Icon,
  label,
  reason,
  danger,
  onClick,
}: {
  icon: React.ComponentType<{ size?: number }>;
  label: string;
  reason: string | null;
  danger?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      disabled={!!reason}
      title={reason ? `${label}: ${reason}` : label}
      className={`rounded-md p-2 text-dim transition-colors disabled:opacity-25 ${danger ? "hover:bg-bad/10 hover:text-bad" : "hover:bg-panel-2 hover:text-neon"}`}
    >
      <Icon size={15} />
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
