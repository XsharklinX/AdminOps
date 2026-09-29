import { Copy, Eye, EyeOff, FolderLock, FolderOpen, HardDrive, KeyRound, Lock, LockOpen, Plus, Save, Trash2, Unlock } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, inputClass, Modal, Loading } from "../components/ui";
import { vaultApi, type VaultStatus, type VaultSupport } from "../lib/api";
import { bytes, friendlyPath } from "../lib/format";

/** Contraseña con botón de mostrar. */
function PasswordInput({ value, onChange, placeholder, autoFocus }: { value: string; onChange: (v: string) => void; placeholder?: string; autoFocus?: boolean }) {
  const [shown, setShown] = useState(false);
  return (
    <div className="relative">
      <input
        type={shown ? "text" : "password"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoFocus={autoFocus}
        autoComplete="new-password"
        className={`${inputClass} pr-9`}
      />
      <button type="button" onClick={() => setShown(!shown)} className="absolute top-1/2 right-2.5 -translate-y-1/2 text-mute hover:text-ink" title={shown ? "Ocultar" : "Mostrar"}>
        {shown ? <EyeOff size={14} /> : <Eye size={14} />}
      </button>
    </div>
  );
}

/** Contraseña nueva + repetirla, con indicación de fortaleza. */
function NewPassword({ pw, setPw, pw2, setPw2 }: { pw: string; setPw: (v: string) => void; pw2: string; setPw2: (v: string) => void }) {
  const score = [pw.length >= 8, pw.length >= 12, /[A-Z]/.test(pw) && /[a-z]/.test(pw), /\d/.test(pw), /[^A-Za-z0-9]/.test(pw)].filter(Boolean).length;
  const label = pw.length === 0 ? "" : pw.length < 8 ? "Demasiado corta (mínimo 8)" : score <= 2 ? "Débil" : score <= 3 ? "Aceptable" : "Fuerte";
  const color = pw.length < 8 || score <= 2 ? "bg-bad" : score <= 3 ? "bg-warn" : "bg-ok";
  return (
    <div className="space-y-2">
      <PasswordInput value={pw} onChange={setPw} placeholder="Contraseña (mínimo 8 caracteres)" />
      {pw && (
        <div className="flex items-center gap-2">
          <div className="h-1 flex-1 overflow-hidden rounded-full bg-panel-2">
            <div className={`h-full ${color}`} style={{ width: `${Math.max(12, (score / 5) * 100)}%` }} />
          </div>
          <span className="w-44 text-right text-[11px] text-mute">{label}</span>
        </div>
      )}
      <PasswordInput value={pw2} onChange={setPw2} placeholder="Repite la contraseña" />
      {pw2 && pw !== pw2 && <p className="text-xs text-bad">Las contraseñas no coinciden.</p>}
    </div>
  );
}

const pwOk = (pw: string, pw2: string) => pw.length >= 8 && pw === pw2;

export function Vault() {
  const [support, setSupport] = useState<VaultSupport | null>(null);
  const [vaults, setVaults] = useState<VaultStatus[] | null>(null);
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [opening, setOpening] = useState<VaultStatus | null>(null);
  const [password, setPassword] = useState("");
  const [changing, setChanging] = useState<VaultStatus | null>(null);
  const [recovery, setRecovery] = useState<{ name: string; key: string; fresh: boolean } | null>(null);
  const [askKey, setAskKey] = useState<VaultStatus | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    try {
      setVaults(await vaultApi.list());
    } catch (e) {
      toast("error", String(e));
      setVaults([]);
    }
  }, [toast]);

  useEffect(() => {
    vaultApi.support().then(setSupport);
    load();
  }, [load]);

  const act = async (id: string, fn: () => Promise<unknown>, ok?: string) => {
    setBusy(id);
    try {
      await fn();
      if (ok) toast("ok", ok);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  const open = async () => {
    if (!opening) return;
    const v = opening;
    setOpening(null);
    await act(v.id, async () => {
      const letter = await vaultApi.open(v.id, password);
      toast("ok", `Caja fuerte abierta en ${letter}`);
    });
    setPassword("");
  };

  const remove = async (v: VaultStatus) => {
    const ok = await confirm({
      title: `¿Eliminar «${v.name}»?`,
      danger: true,
      confirmLabel: "Eliminar la caja y su contenido",
      body: <p>Se borra el archivo de la caja fuerte con todo lo que tiene dentro. No se puede deshacer.</p>,
    });
    if (ok) act(v.id, () => vaultApi.remove(v.id, true), "Caja fuerte eliminada.");
  };

  const showKey = async () => {
    if (!askKey) return;
    const v = askKey;
    setAskKey(null);
    try {
      setRecovery({ name: v.name, key: await vaultApi.recoveryKey(v.id, password), fresh: false });
    } catch (e) {
      toast("error", String(e));
    } finally {
      setPassword("");
      load();
    }
  };

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <Card title="Caja fuerte" icon={<Lock size={14} />} className="col-span-12">
        <div className="flex flex-wrap items-start gap-4">
          <p className="min-w-0 flex-1 text-sm text-dim">
            Un archivo que funciona como una unidad cifrada con BitLocker y contraseña. Guarda dentro lo que quieras proteger. Sigue protegido aunque
            se desinstale AdminOps: con <b className="font-medium text-ink">doble clic en el archivo .vhdx</b> Windows pide la contraseña y la abre;
            para cerrarla, clic derecho en la unidad → Expulsar.
          </p>
          <div className="flex gap-2">
            <Button kind="ghost" onClick={() => vaultApi.addExisting().then((v) => v && load()).catch((e) => toast("error", String(e)))}>
              <FolderOpen size={14} /> Añadir existente
            </Button>
            <Button onClick={() => setCreating(true)} disabled={!support?.bitlocker}>
              <Plus size={14} /> Nueva caja fuerte
            </Button>
          </div>
        </div>
        {support && !support.bitlocker && (
          <p className="mt-3 rounded-md bg-panel-2 px-3 py-2 text-xs text-dim">
            Esta edición de Windows ({support.edition || "Home"}) no puede crear unidades con BitLocker: hace falta Windows Pro. Usa la carpeta cifrada de abajo.
          </p>
        )}

        {vaults === null ? (
          <Loading />
        ) : vaults.length > 0 ? (
          <ul className="mt-4 divide-y divide-line/60 border-t border-line/60">
            {vaults.map((v) => (
              <li key={v.id} className="flex flex-wrap items-center gap-3 py-3">
                <span className={v.unlocked ? "text-ok" : "text-mute"}>{v.unlocked ? <LockOpen size={17} /> : <Lock size={17} />}</span>
                <div className="min-w-0 flex-1">
                  <div className="text-sm text-ink">{v.name}</div>
                  <div className="truncate text-xs text-mute" title={friendlyPath(v.path)}>
                    {friendlyPath(v.path)} · hasta {v.sizeGb} GB{v.exists ? ` · ocupa ${bytes(v.fileSize)}` : ""}
                  </div>
                </div>
                {!v.exists ? (
                  <span className="text-xs text-warn">No se encuentra el archivo (¿USB desconectado?)</span>
                ) : v.unlocked ? (
                  <>
                    <span className="rounded bg-ok/10 px-2 py-0.5 text-xs text-ok">Abierta en {v.letter}</span>
                    <Button kind="ghost" onClick={() => act(v.id, () => vaultApi.open(v.id, ""))}>
                      <FolderOpen size={14} /> Mostrar
                    </Button>
                    <Button onClick={() => act(v.id, () => vaultApi.close(v.id), "Caja fuerte cerrada.")} disabled={busy === v.id}>
                      <Lock size={14} /> Cerrar
                    </Button>
                  </>
                ) : (
                  <Button onClick={() => setOpening(v)} disabled={busy === v.id}>
                    <Unlock size={14} /> Abrir
                  </Button>
                )}
                <div className="flex gap-1">
                  <button onClick={() => setChanging(v)} disabled={!v.exists} className="rounded-md p-1.5 text-mute hover:bg-panel-2 hover:text-ink disabled:opacity-30" title="Cambiar la contraseña">
                    <KeyRound size={14} />
                  </button>
                  <button onClick={() => setAskKey(v)} disabled={!v.exists} className="rounded-md p-1.5 text-mute hover:bg-panel-2 hover:text-ink disabled:opacity-30" title="Ver la clave de recuperación">
                    <Save size={14} />
                  </button>
                  <button
                    onClick={() => act(v.id, () => vaultApi.remove(v.id, false), "Quitada de la lista (el archivo sigue ahí).")}
                    className="rounded-md p-1.5 text-mute hover:bg-panel-2 hover:text-ink"
                    title="Quitar de la lista (no borra el archivo)"
                  >
                    <HardDrive size={14} />
                  </button>
                  <button onClick={() => remove(v)} disabled={!v.exists} className="rounded-md p-1.5 text-mute hover:bg-panel-2 hover:text-bad disabled:opacity-30" title="Eliminar la caja y su contenido">
                    <Trash2 size={14} />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className="mt-4 text-sm text-mute">Aún no hay cajas fuertes.</p>
        )}
      </Card>

      <EncryptedFolder />

      {creating && (
        <CreateDialog
          onClose={() => setCreating(false)}
          onCreated={(name, key) => {
            setCreating(false);
            setRecovery({ name, key, fresh: true });
            load();
          }}
        />
      )}

      {(opening || askKey) && (
        <Modal
          title={opening ? `Abrir «${opening.name}»` : `Clave de recuperación de «${askKey!.name}»`}
          onClose={() => {
            setOpening(null);
            setAskKey(null);
            setPassword("");
          }}
          width="w-[420px]"
          footer={
            <Button onClick={opening ? open : showKey} disabled={!password && !(askKey?.unlocked)}>
              {opening ? <Unlock size={14} /> : <KeyRound size={14} />} {opening ? "Abrir" : "Ver clave"}
            </Button>
          }
        >
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (opening) open();
              else showKey();
            }}
          >
            <PasswordInput value={password} onChange={setPassword} placeholder="Contraseña de la caja fuerte" autoFocus />
          </form>
        </Modal>
      )}

      {changing && <ChangePassword vault={changing} onClose={() => setChanging(null)} onDone={load} />}

      {recovery && <RecoveryDialog {...recovery} onClose={() => setRecovery(null)} />}
      {dialog}
    </div>
  );
}

function CreateDialog({ onClose, onCreated }: { onClose: () => void; onCreated: (name: string, key: string) => void }) {
  const [name, setName] = useState("Caja fuerte");
  const [size, setSize] = useState(16);
  const [path, setPath] = useState<string | null>(null);
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [understood, setUnderstood] = useState(false);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const create = async () => {
    if (!path) return;
    setBusy(true);
    try {
      const r = await vaultApi.create(name, path, size, pw);
      onCreated(name, r.recovery);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="Nueva caja fuerte"
      onClose={busy ? () => {} : onClose}
      width="w-[520px]"
      footer={
        <>
          <Button kind="ghost" onClick={onClose} disabled={busy}>
            Cancelar
          </Button>
          <Button onClick={create} disabled={busy || !path || !pwOk(pw, pw2) || !understood || !name.trim()}>
            <Lock size={14} /> {busy ? "Creando…" : "Crear caja fuerte"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="grid grid-cols-3 gap-3">
          <label className="col-span-2 block">
            <span className="mb-1 block text-xs text-dim">Nombre</span>
            <input value={name} onChange={(e) => setName(e.target.value)} maxLength={32} className={inputClass} />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Tamaño máximo (GB)</span>
            <input type="number" min={1} max={2048} value={size} onChange={(e) => setSize(Math.min(2048, Math.max(1, Math.round(Number(e.target.value) || 1))))} className={inputClass} />
          </label>
        </div>
        <div>
          <span className="mb-1 block text-xs text-dim">Dónde guardarla</span>
          <div className="flex items-center gap-2">
            <span className="min-w-0 flex-1 truncate rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-dim">{path ? friendlyPath(path) : "Sin elegir"}</span>
            <Button kind="ghost" onClick={() => vaultApi.pickLocation(name).then((p) => p && setPath(p))}>
              Elegir…
            </Button>
          </div>
          <p className="mt-1 text-[11px] text-mute">El archivo empieza pequeño y crece según lo llenas, hasta el tamaño máximo. Puede estar en un USB.</p>
        </div>
        <NewPassword pw={pw} setPw={setPw} pw2={pw2} setPw2={setPw2} />
        <label className="flex items-start gap-2 text-sm text-dim">
          <input type="checkbox" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} className="mt-0.5 size-4 accent-[var(--color-neon)]" />
          Entiendo que si se pierden la contraseña y la clave de recuperación, lo que haya dentro no se puede recuperar de ninguna forma.
        </label>
        <TaskStatus task="vault" active={busy} fallback="Creando…" cancellable={false} />
      </div>
    </Modal>
  );
}

function ChangePassword({ vault, onClose, onDone }: { vault: VaultStatus; onClose: () => void; onDone: () => void }) {
  const [old, setOld] = useState("");
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const save = async () => {
    setBusy(true);
    try {
      await vaultApi.changePassword(vault.id, old, pw);
      toast("ok", "Contraseña cambiada.");
      onClose();
      onDone();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      title={`Cambiar la contraseña de «${vault.name}»`}
      onClose={onClose}
      width="w-[440px]"
      footer={
        <Button onClick={save} disabled={busy || !old || !pwOk(pw, pw2)}>
          <KeyRound size={14} /> Cambiar
        </Button>
      }
    >
      <div className="space-y-3">
        <PasswordInput value={old} onChange={setOld} placeholder="Contraseña actual" autoFocus />
        <NewPassword pw={pw} setPw={setPw} pw2={pw2} setPw2={setPw2} />
        <p className="text-[11px] text-mute">La clave de recuperación no cambia.</p>
      </div>
    </Modal>
  );
}

function RecoveryDialog({ name, key: recoveryKey, fresh, onClose }: { name: string; key: string; fresh: boolean; onClose: () => void }) {
  const [saved, setSaved] = useState(!fresh);
  const toast = useToast();
  return (
    <Modal
      title={fresh ? "Caja fuerte creada: guarda la clave de recuperación" : `Clave de recuperación de «${name}»`}
      onClose={saved ? onClose : () => toast("info", "Guarda o copia la clave antes de cerrar.")}
      width="w-[560px]"
      footer={
        <>
          <Button
            kind="ghost"
            onClick={() =>
              navigator.clipboard.writeText(recoveryKey).then(() => {
                setSaved(true);
                toast("ok", "Clave copiada.");
              })
            }
          >
            <Copy size={14} /> Copiar
          </Button>
          <Button
            kind="ghost"
            onClick={() =>
              vaultApi
                .saveRecovery(name, recoveryKey)
                .then((p) => {
                  if (p) {
                    setSaved(true);
                    toast("ok", "Clave guardada.");
                  }
                })
                .catch((e) => toast("error", String(e)))
            }
          >
            <Save size={14} /> Guardar en archivo…
          </Button>
          <Button onClick={onClose} disabled={!saved}>
            Ya la tengo guardada
          </Button>
        </>
      }
    >
      <p className="mb-3 text-sm text-dim">Si se olvida la contraseña, esta clave de 48 dígitos es la única forma de abrir la caja fuerte. Guárdala fuera del equipo (un USB o en papel).</p>
      <div className="rounded-lg border border-line bg-void px-4 py-3 text-center font-mono text-[15px] tracking-wide break-all text-ink select-text">{recoveryKey}</div>
      {fresh && <p className="mt-3 text-xs text-mute">AdminOps no guarda esta clave. Más adelante puedes volver a verla con la contraseña.</p>}
    </Modal>
  );
}

/** Carpeta cifrada en un .zip AES-256 (para Windows Home o para enviar). */
function EncryptedFolder() {
  const [folder, setFolder] = useState<string | null>(null);
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [removeOriginal, setRemoveOriginal] = useState(false);
  const [zip, setZip] = useState<string | null>(null);
  const [zipPw, setZipPw] = useState("");
  const [busy, setBusy] = useState<"encrypt" | "decrypt" | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const encrypt = async () => {
    if (!folder) return;
    if (removeOriginal) {
      const ok = await confirm({
        title: "¿Borrar la carpeta original?",
        danger: true,
        confirmLabel: "Cifrar y borrar la original",
        body: <p>Después de comprobar que el archivo cifrado se abre bien, la carpeta original se borra de forma segura (no va a la papelera). Sin la contraseña no hay forma de recuperar el contenido.</p>,
      });
      if (!ok) return;
    }
    setBusy("encrypt");
    try {
      const r = await vaultApi.encryptFolder(folder, pw, removeOriginal);
      toast("ok", `Carpeta cifrada: ${r.files} archivos en ${friendlyPath(r.path)}`);
      setFolder(null);
      setPw("");
      setPw2("");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const decrypt = async () => {
    if (!zip) return;
    setBusy("decrypt");
    try {
      const r = await vaultApi.decrypt(zip, zipPw);
      toast("ok", `${r.files} archivos descifrados en ${friendlyPath(r.path)}`);
      setZip(null);
      setZipPw("");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const pick = "min-w-0 flex-1 truncate rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-dim";
  return (
    <Card title="Carpeta cifrada" icon={<FolderLock size={14} />} className="col-span-12">
      <p className="mb-4 text-sm text-dim">
        Guarda una carpeta en un archivo .zip cifrado con AES-256 y contraseña. Funciona en cualquier Windows (también Home) y sirve para enviarla o
        guardarla en un USB. Se abre con 7-Zip, WinRAR o AdminOps (el Explorador de Windows no admite este cifrado).
      </p>
      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <section className="space-y-3">
          <h4 className="text-xs font-medium text-dim">Cifrar una carpeta</h4>
          <div className="flex items-center gap-2">
            <span className={pick}>{folder ? friendlyPath(folder) : "Ninguna carpeta elegida"}</span>
            <Button kind="ghost" onClick={() => vaultApi.pickFolder().then((f) => f && setFolder(f))}>
              Elegir…
            </Button>
          </div>
          <NewPassword pw={pw} setPw={setPw} pw2={pw2} setPw2={setPw2} />
          <label className="flex items-center gap-2 text-sm text-dim">
            <input type="checkbox" checked={removeOriginal} onChange={(e) => setRemoveOriginal(e.target.checked)} className="size-4 accent-[var(--color-neon)]" />
            Borrar la carpeta original de forma segura al terminar
          </label>
          <div className="flex justify-end">
            <Button onClick={encrypt} disabled={busy !== null || !folder || !pwOk(pw, pw2)}>
              <Lock size={14} /> Cifrar
            </Button>
          </div>
          <TaskStatus task="encrypt" active={busy === "encrypt"} fallback="Cifrando…" />
        </section>
        <section className="space-y-3">
          <h4 className="text-xs font-medium text-dim">Abrir una carpeta cifrada</h4>
          <div className="flex items-center gap-2">
            <span className={pick}>{zip ? friendlyPath(zip) : "Ningún archivo elegido"}</span>
            <Button kind="ghost" onClick={() => vaultApi.pickZip().then((f) => f && setZip(f))}>
              Elegir…
            </Button>
          </div>
          <PasswordInput value={zipPw} onChange={setZipPw} placeholder="Contraseña" />
          <div className="flex justify-end">
            <Button onClick={decrypt} disabled={busy !== null || !zip || !zipPw}>
              <Unlock size={14} /> Descifrar
            </Button>
          </div>
          <p className="text-[11px] text-mute">Se crea una carpeta nueva junto al archivo; el .zip cifrado no se toca.</p>
          <TaskStatus task="decrypt" active={busy === "decrypt"} fallback="Descifrando…" />
        </section>
      </div>
      {dialog}
    </Card>
  );
}
