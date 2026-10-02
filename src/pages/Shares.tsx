import { CalendarClock, CircleHelp, Copy, FileText, Folder, FolderPlus, MessageSquareText, RotateCw, Share2, ShieldAlert, Trash2, TriangleAlert, UserCog, Users } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { NetworkDrives, OtherPcShares } from "../components/NetworkDrives";
import { AccessEditor, ShareBackupDialog, shortAccount, when, WhyNoAccess } from "../components/ShareDialogs";
import { Button, Card, EmptyLine, ErrorState, inputClass, Modal, Loading } from "../components/ui";
import { officeApi, usersApi, vaultApi, type Share, type ShareBackup, type ShareRisk, type ShareSize, type SharingStatus } from "../lib/api";
import { bytes, friendlyPath } from "../lib/format";

const RIGHT: Record<string, string> = { Full: "Control total", Change: "Leer y modificar", Read: "Solo leer", Custom: "Personalizado" };

/** Compartidos que suelen ser un descuido, y por qué importan. */
const RISK: Record<ShareRisk, { label: string; why: string }> = {
  "everyone-full": {
    label: "Todos: control total",
    why: "Cualquiera de la red puede borrar todo y cambiar los permisos. Casi nunca hace falta: con «Leer y modificar» se trabaja igual.",
  },
  "whole-disk": { label: "Disco entero", why: "Se comparte un disco completo, no una carpeta: queda a la vista todo lo que hay en él." },
  profile: { label: "Carpetas personales", why: "Se comparte la carpeta personal entera de alguien (documentos, escritorio, descargas y datos de sus programas)." },
  personal: { label: "Carpeta personal", why: "Es el Escritorio, Documentos o Descargas de alguien: suele compartirse sin querer. Mejor una carpeta hecha para eso." },
  system: { label: "Carpeta de Windows", why: "Es una carpeta del sistema o de programas: no debería verse desde otros equipos." },
};

const GB = 1024 ** 3;
/** Queda poco sitio donde está la carpeta: menos del 10 % o de 10 GB. */
const lowDisk = (z?: ShareSize) => !!z && z.diskTotal > 0 && (z.diskFree < 10 * GB || z.diskFree / z.diskTotal < 0.1);

/** Texto listo para mandarle a quien tiene que entrar. */
function instructions(name: string, unc: string) {
  return [
    `Para abrir la carpeta «${name}»:`,
    "1. Pulsa las teclas Windows + R.",
    `2. Escribe ${unc} y pulsa Intro.`,
    "3. Si pide usuario y contraseña, pon los que te dieron para ese equipo y marca «Recordar mis credenciales».",
    "",
    "Para tenerla siempre a mano: en el Explorador, botón derecho sobre «Este equipo» → «Conectar a unidad de red…» y pega la misma ruta.",
  ].join("\n");
}

type Dialog = { kind: "access"; share: string; user?: string } | { kind: "why"; share: string } | { kind: "backup"; share: string } | null;

export function Shares({ isAdmin }: { isAdmin: boolean }) {
  const [status, setStatus] = useState<SharingStatus | null>(null);
  const [sizes, setSizes] = useState<Map<string, ShareSize> | null>(null);
  const [backups, setBackups] = useState<Map<string, ShareBackup>>(new Map());
  const [failed, setFailed] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [dialogOpen, setDialogOpen] = useState<Dialog>(null);
  const [drivesTick, setDrivesTick] = useState(0);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const loadBackups = useCallback(async () => {
    try {
      setBackups(new Map((await officeApi.shareBackups()).map((b) => [b.share.toLowerCase(), b])));
    } catch {
      /* sin copias a la vista: la lista de carpetas sigue sirviendo */
    }
  }, []);

  const load = useCallback(async () => {
    try {
      setStatus(await officeApi.shares());
      setFailed(null);
    } catch (e) {
      setFailed(String(e));
    }
    void loadBackups();
    // Contar archivos tarda: va aparte para no retener la lista.
    officeApi
      .shareSizes()
      .then((z) => setSizes(new Map(z.map((x) => [x.name.toLowerCase(), x]))))
      .catch(() => setSizes(new Map()));
  }, [loadBackups]);

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
    await load();
  };

  /** «Todos: control total» → «Leer y modificar», que es lo que casi siempre se quería. */
  const lowerEveryone = async (s: Share) => {
    const everyone = s.access.find((a) => a.allow && a.sid === "S-1-1-0");
    if (!everyone) return;
    try {
      await officeApi.shareGrant(s.name, everyone.account, "Change");
      toast("ok", "«Todos» pasa a leer y modificar.");
    } catch (e) {
      toast("error", String(e));
    }
    void load();
  };

  const copy = (text: string, done: string) =>
    void navigator.clipboard.writeText(text).then(
      () => toast("ok", done),
      () => toast("error", "No se pudo copiar."),
    );

  const blocked = status && (status.category === "Public" || !status.fileSharing);
  const host = status?.host ?? "";

  /** Cuántas cosas conviene mirar, para decirlo arriba de una vez. */
  const review = useMemo(() => {
    if (!status) return 0;
    return status.shares.reduce((n, s) => n + s.risks.length + (s.missingPath ? 1 : 0) + (s.ntfsBlocks ? 1 : 0) + (lowDisk(sizes?.get(s.name.toLowerCase())) ? 1 : 0), 0);
  }, [status, sizes]);

  const selected = dialogOpen && status ? status.shares.find((s) => s.name === dialogOpen.share) : undefined;
  const iconBtn = "rounded-md p-1.5 text-mute transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-30";

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
          <Button onClick={() => void enable()} disabled={!isAdmin}>
            Activar para esta red
          </Button>
        </div>
      )}

      <Card title="Carpetas compartidas de este equipo" icon={<Share2 size={14} />}>
        <div className="mb-3 flex flex-wrap items-center gap-3">
          <p className="min-w-0 flex-1 text-sm text-dim">
            Qué carpetas ven los demás equipos de la red, con qué permisos, cuánto ocupan y quién tiene archivos abiertos.
            {review > 0 && (
              <span className="mt-0.5 flex items-center gap-1.5 text-xs text-warn">
                <ShieldAlert size={12} /> {review === 1 ? "Hay 1 cosa que conviene revisar" : `Hay ${review} cosas que conviene revisar`}, marcadas abajo.
              </span>
            )}
          </p>
          <Button kind="ghost" onClick={() => void load()} title="Volver a mirar">
            <RotateCw size={14} />
          </Button>
          {status && status.shares.length > 0 && (
            <Button kind="ghost" onClick={() => setDialogOpen({ kind: "why", share: status.shares[0].name })} title="Elige una carpeta y una persona: qué puede hacer de verdad y qué se lo impide">
              <CircleHelp size={14} /> ¿Por qué no puede entrar?
            </Button>
          )}
          <Button onClick={() => setAdding(true)} disabled={!isAdmin}>
            <FolderPlus size={14} /> Compartir una carpeta
          </Button>
        </div>
        {failed && <ErrorState message={failed} onRetry={() => void load()} />}
        {status === null ? (
          !failed && <Loading />
        ) : status.shares.length === 0 ? (
          <EmptyLine>Este equipo no comparte ninguna carpeta.</EmptyLine>
        ) : (
          <ul className="divide-y divide-line/60">
            {status.shares.map((s) => {
              const unc = `\\\\${host}\\${s.name}`;
              const size = sizes?.get(s.name.toLowerCase());
              const backup = backups.get(s.name.toLowerCase());
              return (
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
                      {s.risks.map((r) => (
                        <span key={r} className="flex items-center gap-1 rounded border border-warn/40 px-1.5 py-px text-[11px] text-warn" title={RISK[r]?.why}>
                          <ShieldAlert size={9} /> {RISK[r]?.label ?? r}
                        </span>
                      ))}
                    </div>
                    <div className="flex flex-wrap items-center gap-x-3 text-xs text-mute">
                      <span className="font-mono text-dim">{unc}</span>
                      <span className="truncate">{friendlyPath(s.path)}</span>
                      {!s.missingPath &&
                        (sizes === null ? (
                          <span>calculando tamaño…</span>
                        ) : (
                          size && (
                            <span>
                              {bytes(size.bytes)}
                              {size.files > 0 && ` · ${size.files.toLocaleString("es")} archivos`}
                            </span>
                          )
                        ))}
                    </div>
                    {s.missingPath && <p className="mt-1 text-[11px] text-bad">Quien intente entrar verá un error de Windows. Vuelve a crear la carpeta o deja de compartirla.</p>}
                    {s.ntfsBlocks && (
                      <p className="mt-1 text-[11px] text-warn">
                        Se comparte con «Todos», pero los permisos del disco no lo permiten y el acceso real es lo que dejen los dos a la vez: por eso la gente recibe «acceso
                        denegado» sin que se entienda por qué. Se arregla volviendo a dar el acceso en «Quién puede entrar».
                      </p>
                    )}
                    {s.risks.map((r) => (
                      <p key={r} className="mt-1 text-[11px] text-warn">
                        {RISK[r]?.why}{" "}
                        {r === "everyone-full" && isAdmin && (
                          <button onClick={() => void lowerEveryone(s)} className="text-neon hover:underline">
                            Bajarlo a «Leer y modificar»
                          </button>
                        )}
                      </p>
                    ))}
                    {lowDisk(size) && size && (
                      <p className="mt-1 flex items-center gap-1 text-[11px] text-warn">
                        <TriangleAlert size={10} /> Quedan {bytes(size.diskFree)} libres en ese disco: cuando se llene, nadie podrá guardar en la carpeta.
                      </p>
                    )}
                    <div className="mt-1 flex flex-wrap gap-1.5">
                      {s.access.map((a) => (
                        <span key={a.account + a.right + String(a.allow)} className={`rounded px-1.5 py-0.5 text-[11px] ${a.allow ? "bg-panel-2 text-dim" : "bg-bad/10 text-bad"}`}>
                          {a.allow ? "" : "Denegado · "}
                          {shortAccount(a.account)}: {RIGHT[a.right] ?? a.right}
                        </span>
                      ))}
                    </div>
                    {backup && (
                      <button onClick={() => setDialogOpen({ kind: "backup", share: s.name })} className={`mt-1.5 flex items-center gap-1.5 text-[11px] hover:underline ${backup.ok === false ? "text-bad" : "text-mute"}`}>
                        <CalendarClock size={11} />
                        {backup.running
                          ? "Copiando ahora…"
                          : backup.ok === false
                            ? `La última copia falló (${when(backup.lastRun)})`
                            : backup.ok
                              ? `Copia diaria a las ${backup.time} · última: ${when(backup.lastRun)}`
                              : `Copia diaria a las ${backup.time} · aún sin hacer`}
                      </button>
                    )}
                  </div>
                  <div className="flex shrink-0 items-center">
                    <button onClick={() => copy(unc, "Ruta copiada.")} className={iconBtn} title={`Copiar la ruta ${unc}`}>
                      <Copy size={14} />
                    </button>
                    <button onClick={() => copy(instructions(s.name, unc), "Instrucciones copiadas: pégalas en un correo o un chat.")} className={iconBtn} title="Copiar las instrucciones para quien tiene que entrar">
                      <MessageSquareText size={14} />
                    </button>
                    <button onClick={() => setDialogOpen({ kind: "access", share: s.name })} className={iconBtn} title="Quién puede entrar (cambiar permisos)">
                      <UserCog size={14} />
                    </button>
                    <button onClick={() => setDialogOpen({ kind: "why", share: s.name })} className={iconBtn} title="¿Por qué alguien no puede entrar?">
                      <CircleHelp size={14} />
                    </button>
                    <button onClick={() => setDialogOpen({ kind: "backup", share: s.name })} disabled={s.missingPath} className={iconBtn} title="Copia diaria a otro disco">
                      <CalendarClock size={14} />
                    </button>
                    <button onClick={() => void remove(s.name, s.openFiles)} disabled={!isAdmin} className={`${iconBtn} hover:text-bad`} title="Dejar de compartir">
                      <Trash2 size={14} />
                    </button>
                  </div>
                </li>
              );
            })}
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

      <NetworkDrives refresh={drivesTick} />
      <OtherPcShares onMapped={() => setDrivesTick((n) => n + 1)} />

      {adding && (
        <NewShare
          onClose={() => setAdding(false)}
          onDone={() => {
            setAdding(false);
            void load();
          }}
        />
      )}
      {dialogOpen?.kind === "access" && selected && (
        <AccessEditor share={selected} isAdmin={isAdmin} preselect={dialogOpen.user} onChanged={load} onClose={() => setDialogOpen(null)} />
      )}
      {dialogOpen?.kind === "why" && status && (
        <WhyNoAccess
          shares={status.shares}
          initial={dialogOpen.share}
          isAdmin={isAdmin}
          onEnableSharing={enable}
          onEditAccess={(share, user) => setDialogOpen({ kind: "access", share, user })}
          onClose={() => setDialogOpen(null)}
        />
      )}
      {dialogOpen?.kind === "backup" && selected && (
        <ShareBackupDialog share={selected} backup={backups.get(selected.name.toLowerCase())} isAdmin={isAdmin} onChanged={loadBackups} onClose={() => setDialogOpen(null)} />
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
        <p className="text-[11px] text-mute">Se ajustan también los permisos de la carpeta. Cada usuario solo ve dentro lo que tiene permiso para abrir. Después se puede cambiar quién entra sin volver a compartirla.</p>
      </div>
    </Modal>
  );
}
