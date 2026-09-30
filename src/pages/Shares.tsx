import { FileText, FolderPlus, Folder, RotateCw, Share2, Trash2, TriangleAlert, Users } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, inputClass, Modal, Loading } from "../components/ui";
import { officeApi, usersApi, vaultApi, type SharingStatus } from "../lib/api";
import { friendlyPath } from "../lib/format";

const RIGHT: Record<string, string> = { Full: "Control total", Change: "Leer y modificar", Read: "Solo leer", Custom: "Personalizado" };

export function Shares({ isAdmin }: { isAdmin: boolean }) {
  const [status, setStatus] = useState<SharingStatus | null>(null);
  const [adding, setAdding] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    try {
      setStatus(await officeApi.shares());
    } catch (e) {
      toast("error", String(e));
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  const remove = async (name: string, open: number) => {
    const ok = await confirm({
      title: `¿Dejar de compartir «${name}»?`,
      confirmLabel: "Dejar de compartir",
      body: (
        <p>
          La carpeta y sus archivos no se tocan: solo deja de verse desde otros equipos.
          {open > 0 && ` Hay ${open} archivo(s) abiertos desde otros equipos que se cerrarán.`}
        </p>
      ),
    });
    if (!ok) return;
    try {
      await officeApi.removeShare(name);
      toast("ok", "Carpeta ya no compartida.");
    } catch (e) {
      toast("error", String(e));
    }
    void load();
  };

  const enable = async () => {
    try {
      await officeApi.enableSharing();
      toast("ok", "Red privada y compartir archivos activados.");
    } catch (e) {
      toast("error", String(e));
    }
    void load();
  };

  const blocked = status && (status.category === "Public" || !status.fileSharing);

  return (
    <div className="mx-auto max-w-6xl space-y-4 p-6">
      {blocked && (
        <div className="flex flex-wrap items-center gap-3 rounded-xl border border-warn/30 bg-warn/5 px-4 py-3">
          <TriangleAlert size={16} className="shrink-0 text-warn" />
          <p className="min-w-0 flex-1 text-sm text-ink">
            {status.category === "Public"
              ? "Windows trata esta red como pública: las carpetas compartidas no se ven desde otros equipos."
              : "Compartir archivos e impresoras está desactivado en el firewall."}
            <span className="block text-xs text-dim">Si es la red de casa o de la oficina, actívalo. En una Wi-Fi pública (cafetería, hotel) déjalo así.</span>
          </p>
          <Button onClick={enable} disabled={!isAdmin}>
            Activar para esta red
          </Button>
        </div>
      )}

      <Card title="Carpetas compartidas de este equipo" icon={<Share2 size={14} />}>
        <div className="mb-3 flex items-center gap-3">
          <p className="min-w-0 flex-1 text-sm text-dim">Qué carpetas ven los demás equipos de la red, con qué permisos, y quién tiene archivos abiertos.</p>
          <Button kind="ghost" onClick={load}>
            <RotateCw size={14} />
          </Button>
          <Button onClick={() => setAdding(true)} disabled={!isAdmin}>
            <FolderPlus size={14} /> Compartir una carpeta
          </Button>
        </div>
        {status === null ? (
          <Loading />
        ) : status.shares.length === 0 ? (
          <p className="text-sm text-mute">Este equipo no comparte ninguna carpeta.</p>
        ) : (
          <ul className="divide-y divide-line/60">
            {status.shares.map((s) => (
              <li key={s.name} className="flex items-start gap-3 py-3">
                <Folder size={17} className="mt-0.5 shrink-0 text-mute" />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2 text-sm text-ink">
                    {s.name}
                    {s.openFiles > 0 && <span className="rounded bg-neon/10 px-1.5 text-[11px] text-neon">{s.openFiles} archivo(s) en uso</span>}
                    {s.missingPath && (
                      <span className="flex items-center gap-1 rounded border border-bad/40 px-1.5 py-px text-[11px] text-bad">
                        <TriangleAlert size={9} /> La carpeta ya no existe
                      </span>
                    )}
                    {s.ntfsBlocks && (
                      <span className="flex items-center gap-1 rounded border border-warn/40 px-1.5 py-px text-[11px] text-warn">
                        <TriangleAlert size={9} /> Los permisos del disco no dejan entrar
                      </span>
                    )}
                  </div>
                  <div className="truncate text-xs text-mute">{friendlyPath(s.path)}</div>
                  {s.missingPath && (
                    <p className="mt-1 text-[11px] text-bad">Quien intente entrar verá un error de Windows. Vuelve a crear la carpeta o deja de compartirla.</p>
                  )}
                  {s.ntfsBlocks && (
                    <p className="mt-1 text-[11px] text-warn">
                      Se comparte con «Todos», pero los permisos del disco no lo permiten y el acceso real es lo que dejen los dos a la vez: por eso la gente recibe
                      «acceso denegado» sin que se entienda por qué. Se arregla en la pestaña Seguridad de las propiedades de la carpeta.
                    </p>
                  )}
                  <div className="mt-1 flex flex-wrap gap-1.5">
                    {s.access.map((a) => (
                      <span key={a.account + a.right} className={`rounded px-1.5 py-0.5 text-[11px] ${a.allow ? "bg-panel-2 text-dim" : "bg-bad/10 text-bad"}`}>
                        {a.allow ? "" : "Denegado · "}
                        {a.account.replace(/^[^\\]+\\/, "")}: {RIGHT[a.right] ?? a.right}
                      </span>
                    ))}
                  </div>
                </div>
                <button onClick={() => remove(s.name, s.openFiles)} disabled={!isAdmin} className="rounded-md p-1.5 text-mute hover:bg-panel-2 hover:text-bad disabled:opacity-30" title="Dejar de compartir">
                  <Trash2 size={14} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </Card>

      {status && (status.sessions.length > 0 || status.open.length > 0) && (
        <Card title={`Conectados ahora · ${status.sessions.length}`} icon={<Users size={14} />}>
          <ul className="space-y-1 text-sm text-dim">
            {status.sessions.map((s) => (
              <li key={s} className="font-mono text-xs">
                {s}
              </li>
            ))}
          </ul>
          {status.open.length > 0 && (
            <>
              <div className="mt-3 mb-1 text-[11px] font-semibold text-dim">Archivos abiertos ahora mismo</div>
              <p className="mb-2 text-[11px] text-mute">Un archivo abierto no se puede mover, renombrar ni borrar, y quien lo tenga bloqueado impide que otros lo guarden.</p>
              <ul className="space-y-1">
                {status.open.map((f, i) => (
                  <li key={`${f.user}-${f.name}-${i}`} className="flex items-center gap-2 text-xs">
                    <FileText size={12} className="shrink-0 text-mute" />
                    <span className="min-w-0 flex-1 truncate text-ink">{f.name}</span>
                    {f.locked && <span className="shrink-0 rounded border border-warn/40 px-1.5 text-[11px] text-warn">bloqueado</span>}
                    <span className="shrink-0 font-mono text-mute">{f.user}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </Card>
      )}

      {adding && (
        <NewShare
          onClose={() => setAdding(false)}
          onDone={() => {
            setAdding(false);
            void load();
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function NewShare({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const [path, setPath] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [who, setWho] = useState("everyone");
  const [write, setWrite] = useState(false);
  const [users, setUsers] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  useEffect(() => {
    usersApi
      .list()
      .then((u) => setUsers(u.filter((x) => x.enabled && !x.builtin).map((x) => x.name)))
      .catch(() => {});
  }, []);

  const pick = async () => {
    const p = await vaultApi.pickFolder();
    if (p) {
      setPath(p);
      if (!name) setName(p.split("\\").filter(Boolean).pop() ?? "");
    }
  };

  const save = async () => {
    if (!path) return;
    setBusy(true);
    try {
      await officeApi.createShare(path, name, who, write);
      toast("ok", `«${name}» compartida.`);
      onDone();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="Compartir una carpeta"
      onClose={onClose}
      width="w-[500px]"
      footer={
        <Button onClick={save} disabled={busy || !path || !name.trim()}>
          <Share2 size={14} /> Compartir
        </Button>
      }
    >
      <div className="space-y-4">
        <div className="flex items-center gap-2">
          <span className="min-w-0 flex-1 truncate rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-dim">{path ? friendlyPath(path) : "Ninguna carpeta elegida"}</span>
          <Button kind="ghost" onClick={pick}>
            Elegir…
          </Button>
        </div>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Nombre en la red</span>
          <input value={name} onChange={(e) => setName(e.target.value)} className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Quién puede entrar</span>
          <select value={who} onChange={(e) => setWho(e.target.value)} className={inputClass}>
            <option value="everyone">Todos los usuarios de la red</option>
            {users.map((u) => (
              <option key={u} value={u}>
                Solo el usuario «{u}» (con su contraseña)
              </option>
            ))}
          </select>
        </label>
        <div className="flex gap-4 text-sm">
          <label className="flex items-center gap-2">
            <input type="radio" checked={!write} onChange={() => setWrite(false)} className="accent-[var(--color-neon)]" /> Solo leer
          </label>
          <label className="flex items-center gap-2">
            <input type="radio" checked={write} onChange={() => setWrite(true)} className="accent-[var(--color-neon)]" /> Leer y modificar
          </label>
        </div>
        <p className="text-[11px] text-mute">Se ajustan también los permisos de la carpeta. Cada usuario solo ve dentro lo que tiene permiso para abrir.</p>
      </div>
    </Modal>
  );
}
