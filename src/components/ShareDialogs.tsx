// Carpetas compartidas: los diálogos de una carpeta. Quién puede entrar (cambiar
// permisos sin volver a compartirla), «¿por qué no puede entrar?» y la copia
// diaria a otro disco.
import { CalendarClock, Check, CircleCheck, CircleX, FolderOpen, Loader2, Play, Plus, Trash2, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { logQuietly, officeApi, usersApi, vaultApi, type Share, type ShareBackup, type ShareExplain, type ShareRight } from "../lib/api";
import { friendlyPath } from "../lib/format";
import { useToast } from "./feedback";
import { Button, inputClass, Modal } from "./ui";

const RIGHTS: [ShareRight, string][] = [
  ["Read", "Solo leer"],
  ["Change", "Leer y modificar"],
  ["Full", "Control total"],
];

/** «EQUIPO\ana» → «ana»: el equipo o el dominio no aportan nada en la lista. */
export const shortAccount = (account: string) => account.replace(/^[^\\]+\\/, "");

/** Usuarios de este equipo a los que tiene sentido dar acceso. */
function useLocalUsers(): string[] {
  const [users, setUsers] = useState<string[]>([]);
  useEffect(() => {
    usersApi
      .list()
      .then((u) => setUsers(u.filter((x) => x.enabled && !x.builtin).map((x) => x.name)))
      .catch(logQuietly("ShareDialogs"));
  }, []);
  return users;
}

/** Quién puede entrar a una carpeta ya compartida, y con qué permiso. */
export function AccessEditor({
  share,
  isAdmin,
  preselect,
  onChanged,
  onClose,
}: {
  share: Share;
  isAdmin: boolean;
  /** Cuenta que se propone añadir (viene de «¿por qué no puede entrar?»). */
  preselect?: string;
  onChanged: () => Promise<void> | void;
  onClose: () => void;
}) {
  const users = useLocalUsers();
  const [who, setWho] = useState(preselect || "everyone");
  const [right, setRight] = useState<ShareRight>("Read");
  const [busy, setBusy] = useState<string | null>(null);
  const toast = useToast();

  const run = async (key: string, action: () => Promise<void>, done: string) => {
    setBusy(key);
    try {
      await action();
      toast("ok", done);
      await onChanged();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const options = preselect && !users.includes(preselect) && preselect !== "everyone" ? [preselect, ...users] : users;

  return (
    <Modal title={`Quién puede entrar a «${share.name}»`} onClose={onClose} width="w-[560px]">
      <div className="space-y-4">
        {share.access.length === 0 ? (
          <p className="text-sm text-warn">Ahora mismo nadie puede entrar: la lista está vacía.</p>
        ) : (
          <ul className="divide-y divide-line/60 rounded-lg border border-line">
            {share.access.map((a) => {
              const key = `${a.account}|${a.allow}`;
              return (
                <li key={key} className="flex items-center gap-2 px-3 py-2">
                  <span className="min-w-0 flex-1 truncate text-sm text-ink" title={a.account}>
                    {shortAccount(a.account)}
                  </span>
                  {a.allow ? (
                    <select
                      value={a.right}
                      disabled={!isAdmin || busy !== null}
                      onChange={(e) => void run(key, () => officeApi.shareGrant(share.name, a.account, e.target.value as ShareRight), "Permiso cambiado.")}
                      className="h-8 rounded-md border border-line bg-void/60 px-2 text-xs text-ink outline-none focus:border-neon/50 disabled:opacity-50"
                      aria-label={`Permiso de ${shortAccount(a.account)}`}
                    >
                      {RIGHTS.map(([id, label]) => (
                        <option key={id} value={id}>
                          {label}
                        </option>
                      ))}
                      {!RIGHTS.some(([id]) => id === a.right) && <option value={a.right}>Personalizado</option>}
                    </select>
                  ) : (
                    <span className="rounded border border-bad/40 px-1.5 py-0.5 text-[11px] text-bad">Denegado</span>
                  )}
                  <button
                    onClick={() => void run(key, () => officeApi.shareRevoke(share.name, a.account, !a.allow), a.allow ? "Ya no puede entrar." : "Denegación quitada.")}
                    disabled={!isAdmin || busy !== null}
                    className="rounded-md p-1.5 text-mute hover:bg-panel-2 hover:text-bad disabled:opacity-30"
                    title={a.allow ? "Quitarle el acceso" : "Quitar la denegación"}
                  >
                    {busy === key ? <Loader2 size={14} className="animate-spin" /> : <Trash2 size={14} />}
                  </button>
                </li>
              );
            })}
          </ul>
        )}

        <div>
          <div className="mb-1 text-xs text-dim">Dar acceso a alguien más</div>
          <div className="flex flex-wrap items-center gap-2">
            <select value={who} onChange={(e) => setWho(e.target.value)} className={`${inputClass} min-w-0 flex-1`} aria-label="A quién">
              <option value="everyone">Todos los usuarios de la red</option>
              {options.map((u) => (
                <option key={u} value={u}>
                  {u}
                </option>
              ))}
            </select>
            <select value={right} onChange={(e) => setRight(e.target.value as ShareRight)} className={`${inputClass} w-44`} aria-label="Permiso">
              {RIGHTS.slice(0, 2).map(([id, label]) => (
                <option key={id} value={id}>
                  {label}
                </option>
              ))}
            </select>
            <Button onClick={() => void run("add", () => officeApi.shareGrant(share.name, who, right), "Acceso dado.")} disabled={!isAdmin || busy !== null}>
              {busy === "add" ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />} Añadir
            </Button>
          </div>
        </div>

        <p className="text-[11px] text-mute">
          Al dar acceso se ajustan también los permisos del disco, para que funcione de verdad. Al quitarlo o bajarlo, los del disco se dejan como están: manda siempre lo más
          restrictivo de los dos. Quien ya tenga la carpeta abierta nota el cambio al volver a entrar.
          {!isAdmin && " Para cambiar permisos, abre AdminOps como administrador."}
        </p>
      </div>
    </Modal>
  );
}

const SHARE_RIGHT = { none: "Nada", read: "Solo leer", change: "Leer y modificar", full: "Control total" } as const;
const DISK_RIGHT = { none: "Nada", read: "Solo leer", write: "Leer y modificar", unknown: "No se pudo leer" } as const;

/** Elige una carpeta y una persona: qué puede hacer de verdad y qué se lo impide. */
export function WhyNoAccess({
  shares,
  initial,
  isAdmin,
  onEnableSharing,
  onEditAccess,
  onClose,
}: {
  shares: Share[];
  initial: string;
  isAdmin: boolean;
  onEnableSharing: () => Promise<void>;
  /** Abre «Quién puede entrar» con esa cuenta ya propuesta. */
  onEditAccess: (share: string, user: string) => void;
  onClose: () => void;
}) {
  const users = useLocalUsers();
  const [share, setShare] = useState(initial);
  const [user, setUser] = useState("");
  const [other, setOther] = useState("");
  const [result, setResult] = useState<ShareExplain | null>(null);
  const [checked, setChecked] = useState("");
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const name = (user === "__other" ? other : user).trim();

  const check = async () => {
    if (!name) return;
    setBusy(true);
    try {
      setResult(await officeApi.shareExplain(share, name));
      setChecked(name);
    } catch (e) {
      setResult(null);
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const tone = result?.verdict === "none" ? "border-bad/40 bg-bad/10 text-bad" : result?.verdict === "read" ? "border-warn/40 bg-warn/10 text-warn" : "border-ok/40 bg-ok/10 text-ok";

  return (
    <Modal title="¿Por qué no puede entrar?" onClose={onClose} width="w-[600px]">
      <div className="space-y-4">
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Carpeta compartida</span>
            <select
              value={share}
              onChange={(e) => {
                setShare(e.target.value);
                setResult(null);
              }}
              className={inputClass}
            >
              {shares.map((s) => (
                <option key={s.name} value={s.name}>
                  {s.name}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Quién intenta entrar</span>
            <select
              value={user}
              onChange={(e) => {
                setUser(e.target.value);
                setResult(null);
              }}
              className={inputClass}
            >
              <option value="">Elige una cuenta…</option>
              {users.map((u) => (
                <option key={u} value={u}>
                  {u}
                </option>
              ))}
              <option value="__other">Otra cuenta…</option>
            </select>
          </label>
        </div>
        {user === "__other" && (
          <input
            value={other}
            onChange={(e) => setOther(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && void check()}
            placeholder="Nombre de la cuenta (o DOMINIO\usuario)"
            className={inputClass}
            aria-label="Nombre de la cuenta"
          />
        )}
        <Button onClick={() => void check()} disabled={busy || !name}>
          {busy ? <Loader2 size={14} className="animate-spin" /> : <Check size={14} />} Comprobar
        </Button>

        {result && (
          <div className="space-y-3">
            <div className={`rounded-xl border px-4 py-3 text-sm font-medium ${tone}`}>{result.headline}</div>
            <div className="grid grid-cols-3 gap-2 text-center">
              {[
                ["La compartición le deja", SHARE_RIGHT[result.shareRight]],
                ["El disco le deja", DISK_RIGHT[result.diskRight]],
                ["Lo que puede de verdad", result.verdict === "write" ? "Leer y modificar" : result.verdict === "read" ? "Solo leer" : "Nada"],
              ].map(([label, value]) => (
                <div key={label} className="rounded-lg border border-line bg-panel-2/40 px-2 py-2">
                  <div className="text-sm text-ink">{value}</div>
                  <div className="mt-0.5 text-[11px] text-mute">{label}</div>
                </div>
              ))}
            </div>
            <ul className="space-y-2">
              {result.findings.map((f) => (
                <li key={f.text} className="flex items-start gap-2 text-sm">
                  {f.level === "bad" ? (
                    <CircleX size={15} className="mt-0.5 shrink-0 text-bad" />
                  ) : f.level === "warn" ? (
                    <TriangleAlert size={15} className="mt-0.5 shrink-0 text-warn" />
                  ) : (
                    <CircleCheck size={15} className="mt-0.5 shrink-0 text-ok" />
                  )}
                  <span className="min-w-0 flex-1 text-dim">{f.text}</span>
                  {f.fix === "sharing" && (
                    <Button kind="ghost" disabled={!isAdmin} onClick={() => void onEnableSharing().then(check)}>
                      Activar
                    </Button>
                  )}
                  {f.fix === "permissions" && (
                    <Button kind="ghost" disabled={!isAdmin} onClick={() => onEditAccess(share, checked)}>
                      Darle acceso
                    </Button>
                  )}
                </li>
              ))}
            </ul>
            <p className="text-[11px] text-mute">Se cuentan sus grupos de este equipo. Si es una cuenta del dominio, los grupos del dominio no se pueden mirar desde aquí.</p>
          </div>
        )}
      </div>
    </Modal>
  );
}

export const when = (iso: string | null) => (iso ? new Date(iso).toLocaleString("es", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "");

/** Copia diaria de la carpeta a otro disco: lo nuevo y lo cambiado, sin borrar nada. */
export function ShareBackupDialog({
  share,
  backup,
  isAdmin,
  onChanged,
  onClose,
}: {
  share: Share;
  backup?: ShareBackup;
  isAdmin: boolean;
  onChanged: () => Promise<void> | void;
  onClose: () => void;
}) {
  // El destino guardado ya incluye la subcarpeta con el nombre de la compartida.
  // «E:\Facturas» → «E:\» (la raíz de un disco lleva su barra).
  const current = backup ? backup.dest.replace(/\\[^\\]+$/, "").replace(/^([A-Za-z]:)$/, "$1\\") : null;
  const [dest, setDest] = useState<string | null>(current);
  const [time, setTime] = useState(backup?.time || "14:00");
  const [busy, setBusy] = useState<string | null>(null);
  const toast = useToast();

  const run = async (key: string, action: () => Promise<void>, done: string, close = false) => {
    setBusy(key);
    try {
      await action();
      toast("ok", done);
      await onChanged();
      if (close) onClose();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const sameDisk = dest !== null && dest.slice(0, 2).toLowerCase() === share.path.slice(0, 2).toLowerCase();

  return (
    <Modal
      title={`Copia diaria de «${share.name}»`}
      onClose={onClose}
      width="w-[540px]"
      footer={
        <>
          {backup && (
            <Button kind="danger" disabled={!isAdmin || busy !== null} onClick={() => void run("remove", () => officeApi.removeShareBackup(share.name), "Copia diaria quitada. Lo ya copiado sigue en su sitio.", true)}>
              Quitar la copia
            </Button>
          )}
          <Button disabled={!isAdmin || busy !== null || !dest || !time} onClick={() => dest && void run("save", () => officeApi.setShareBackup(share.name, dest, time), "Copia diaria programada.", true)}>
            {busy === "save" ? <Loader2 size={14} className="animate-spin" /> : <CalendarClock size={14} />} {backup ? "Guardar" : "Programar"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        {backup && (
          <div className="rounded-lg border border-line bg-panel-2/40 px-3 py-2.5 text-sm">
            <div className="flex items-center gap-2">
              {backup.running ? (
                <span className="flex items-center gap-1.5 text-neon">
                  <Loader2 size={14} className="animate-spin" /> Copiando ahora…
                </span>
              ) : backup.ok === null ? (
                <span className="text-dim">Aún no se ha hecho ninguna copia.</span>
              ) : backup.ok ? (
                <span className="flex items-center gap-1.5 text-ok">
                  <CircleCheck size={14} /> Última copia: {when(backup.lastRun)}
                </span>
              ) : (
                <span className="flex items-center gap-1.5 text-bad">
                  <CircleX size={14} /> La última copia falló ({when(backup.lastRun)})
                </span>
              )}
              <span className="flex-1" />
              <Button kind="ghost" disabled={!isAdmin || busy !== null || backup.running} onClick={() => void run("now", () => officeApi.runShareBackup(share.name), "Copia en marcha.")}>
                {busy === "now" ? <Loader2 size={14} className="animate-spin" /> : <Play size={14} />} Copiar ahora
              </Button>
            </div>
            {backup.ok === false && <p className="mt-1 text-[11px] text-bad">Lo habitual: el disco de destino estaba desconectado o lleno. El detalle queda en el archivo .log junto a la copia.</p>}
            {backup.nextRun && <p className="mt-1 text-[11px] text-mute">Próxima: {when(backup.nextRun)}</p>}
          </div>
        )}

        <div>
          <span className="mb-1 block text-xs text-dim">Dónde se guarda la copia</span>
          <div className="flex items-center gap-2">
            <span className="min-w-0 flex-1 truncate rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-dim">
              {dest ? friendlyPath(`${dest.replace(/\\$/, "")}\\${share.name}`) : "Ninguna carpeta elegida"}
            </span>
            <Button kind="ghost" onClick={() => void vaultApi.pickFolder().then((p) => p && setDest(p))}>
              <FolderOpen size={14} /> Elegir…
            </Button>
          </div>
          {sameDisk && (
            <p className="mt-1.5 flex items-start gap-1.5 text-[11px] text-warn">
              <TriangleAlert size={12} className="mt-0.5 shrink-0" /> Está en el mismo disco que la carpeta: si ese disco falla, se pierden las dos. Mejor un disco externo u otro disco del equipo.
            </p>
          )}
        </div>

        <label className="block w-40">
          <span className="mb-1 block text-xs text-dim">Cada día a las</span>
          <input type="time" value={time} onChange={(e) => setTime(e.target.value)} className={inputClass} />
        </label>

        <p className="text-[11px] text-mute">
          Cada día a esa hora Windows copia lo nuevo y lo cambiado, aunque AdminOps esté cerrada. Nunca borra nada en la copia: lo que se borre en la carpeta sigue estando allí. Si
          el equipo estaba apagado, la hace al encenderlo.
          {!isAdmin && " Para programarla, abre AdminOps como administrador."}
        </p>
      </div>
    </Modal>
  );
}
