// Particiones y arranque: ver la tabla de cada disco, detectar lo que está roto,
// encontrar particiones perdidas y devolverlas. Lo que se hacía con TestDisk.
// Mirar es solo lectura; escribir guarda antes una copia de la tabla y pide confirmación.
import { History, Loader2, RefreshCw, Save, Search, Wrench } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "./feedback";
import { TaskStatus } from "./TaskStatus";
import { NeedsAdmin } from "./AdminBanner";
import { Button, Card, inputClass, Loading, Modal } from "./ui";
import { disksApi, partitionsApi, type DiskReport, type FoundPartition, type PartitionLayout, type TableBackup } from "../lib/api";
import { bytes } from "../lib/format";

const PILL = { ok: "border-ok/40 bg-ok/10 text-ok", warn: "border-warn/40 bg-warn/10 text-warn", bad: "border-bad/40 bg-bad/10 text-bad" } as const;

/** Pide escribir una palabra antes de hacer algo que cambia el disco. */
export function TypedConfirm({ title, body, word, confirmLabel, onClose, onConfirm }: { title: string; body: string; word: string; confirmLabel: string; onClose: () => void; onConfirm: () => void }) {
  const [typed, setTyped] = useState("");
  const ready = typed.trim().toUpperCase() === word.toUpperCase();
  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button kind="danger" disabled={!ready} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </>
      }
    >
      <p className="mb-3 rounded-lg border border-bad/40 bg-bad/10 px-3 py-2 text-xs leading-relaxed text-bad">{body}</p>
      <label className="block text-xs text-dim">
        Para continuar, escribe <b className="font-mono text-ink">{word}</b>
        <input value={typed} onChange={(e) => setTyped(e.target.value)} className={`mt-1 ${inputClass}`} autoFocus />
      </label>
    </Modal>
  );
}

const SEG: Record<string, string> = {
  "Sistema EFI": "bg-neon/60",
  Datos: "bg-ok/60",
  "Recuperación de Windows": "bg-warn/60",
  "Reservada de Microsoft": "bg-line-2",
};

/** La tabla en una barra: cada partición con su ancho, y el espacio sin asignar rayado. */
function LayoutBar({ l }: { l: PartitionLayout }) {
  type Seg = { key: string; offset: number; size: number; label: string; cls: string; free?: boolean };
  const segs: Seg[] = [
    ...l.partitions.filter((p) => p.kind !== "Extendida").map((p) => ({ key: `p${p.index}`, offset: p.offset, size: p.size, label: `${p.fs || p.kind} · ${bytes(p.size, 0)}`, cls: SEG[p.kind] ?? "bg-neon-2/40" })),
    ...l.free.map((g, i) => ({ key: `f${i}`, offset: g.offset, size: g.size, label: `Sin asignar · ${bytes(g.size, 0)}`, cls: "bg-line", free: true })),
  ].sort((a, b) => a.offset - b.offset);
  if (segs.length === 0) return <p className="text-xs text-mute">Sin particiones.</p>;
  return (
    <div className="flex h-10 gap-0.5 overflow-hidden rounded-md" role="img" aria-label="Particiones del disco">
      {segs.map((s) => (
        <div
          key={s.key}
          title={s.label}
          style={{ flexGrow: Math.max(s.size / l.size, 0.04) }}
          className={`flex min-w-0 items-center justify-center overflow-hidden px-1 text-[10px] text-ink ${s.cls} ${s.free ? "bg-[repeating-linear-gradient(135deg,transparent,transparent_5px,var(--color-line-2)_5px,var(--color-line-2)_10px)] text-dim" : ""}`}
        >
          <span className="truncate">{s.label}</span>
        </div>
      ))}
    </div>
  );
}

const SCHEME = { gpt: "GPT", mbr: "MBR", none: "Sin tabla" } as const;
const date = (secs: number) => new Date(secs * 1000).toLocaleString("es", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

export function PartitionsTab({ isAdmin }: { isAdmin: boolean }) {
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [disks, setDisks] = useState<DiskReport[] | null>(null);
  const [sel, setSel] = useState<number | null>(null);
  const [layout, setLayout] = useState<PartitionLayout | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [backups, setBackups] = useState<TableBackup[]>([]);
  const [found, setFound] = useState<FoundPartition[] | null>(null);
  const [busy, setBusy] = useState<"layout" | "backup" | "find" | "restore" | "boot" | null>(null);
  const [typed, setTyped] = useState<{ kind: "partition"; f: FoundPartition } | { kind: "table"; b: TableBackup } | null>(null);

  useEffect(() => {
    disksApi
      .status()
      .then((d) => {
        setDisks(d);
        setSel((s) => s ?? d.find((x) => !x.system)?.number ?? d[0]?.number ?? null);
      })
      .catch((e) => setError(String(e)));
  }, []);
  const disk = disks?.find((d) => d.number === sel) ?? null;

  const load = useCallback(() => {
    if (sel === null || !isAdmin) return;
    setBusy("layout");
    setError(null);
    setFound(null);
    Promise.all([partitionsApi.layout(sel), partitionsApi.backups(sel)])
      .then(([l, b]) => (setLayout(l), setBackups(b)))
      .catch((e) => (setLayout(null), setError(String(e))))
      .finally(() => setBusy(null));
  }, [sel, isAdmin]);
  useEffect(load, [load]);

  if (!isAdmin) return <div className="mx-auto max-w-(--page-max) p-6"><NeedsAdmin>Leer la tabla de particiones de un disco necesita administrador.</NeedsAdmin></div>;
  if (disks === null) return error ? <p className="p-6 text-sm text-bad">{error}</p> : <Loading text="Leyendo los discos…" />;

  const run = async <T,>(kind: NonNullable<typeof busy>, f: () => Promise<T>, done?: (r: T) => void) => {
    setBusy(kind);
    try {
      const r = await f();
      done?.(r);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const sysLetter = disk?.volumes.find((v) => v.system)?.letter;
  const repairBoot = async () => {
    if (!sysLetter) return;
    const ok = await confirm({
      title: `Reparar el arranque de Windows de ${sysLetter}:`,
      body: "Reescribe los archivos de arranque (BCD) con bcdboot. No toca tus archivos. Úsalo si Windows no arranca o muestra «falta el administrador de arranque».",
      confirmLabel: "Reparar el arranque",
    });
    if (ok) await run("boot", () => partitionsApi.repairBoot(sysLetter), (m) => toast("ok", m));
  };

  const tableChanged = (l: PartitionLayout) => {
    setLayout(l);
    if (sel !== null) void partitionsApi.backups(sel).then(setBackups);
  };

  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="min-w-0 flex-1 text-xs text-dim">
          La tabla de cada disco: qué particiones tiene, si está rota y, si falta una, dónde puede estar. Mirar es solo lectura. Antes de escribir algo se guarda una copia de la tabla, y se comprueba después.
        </p>
        <select value={sel ?? ""} onChange={(e) => setSel(Number(e.target.value))} className={`${inputClass} w-auto max-w-xs`} aria-label="Disco">
          {disks.map((d) => (
            <option key={d.number} value={d.number}>
              Disco {d.number} · {d.model || d.bus} · {bytes(d.size, 0)}
              {d.system ? " · Windows" : ""}
            </option>
          ))}
        </select>
        <Button kind="ghost" onClick={load} disabled={busy !== null}>
          {busy === "layout" ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />} Leer otra vez
        </Button>
      </div>

      {error && <p className="text-sm text-bad">{error}</p>}
      {layout && disk && (
        <Card title={`Disco ${disk.number} · ${SCHEME[layout.scheme as keyof typeof SCHEME] ?? layout.scheme}`} right={<span className="text-[11px] text-mute">sectores de {layout.sector} bytes{layout.diskGuid ? ` · ${layout.diskGuid}` : ""}</span>}>
          <div className="space-y-3 p-4">
            <LayoutBar l={layout} />
            {layout.issues.length > 0 ? (
              <ul className="space-y-1.5">
                {layout.issues.map((i) => (
                  <li key={i.text} className={`rounded-md border px-3 py-1.5 text-xs ${PILL[i.level]}`}>
                    {i.text}
                  </li>
                ))}
              </ul>
            ) : (
              <p className="rounded-md border border-ok/40 bg-ok/10 px-3 py-1.5 text-xs text-ok">La tabla está bien: sin solapes, con su copia de seguridad y firmada correctamente.</p>
            )}
            {layout.partitions.length > 0 && (
              <div className="overflow-x-auto rounded-lg border border-line">
                <table className="w-full min-w-[520px] text-left text-xs">
                  <thead className="bg-panel-2 text-mute">
                    <tr>
                      <th className="px-3 py-1.5 font-medium">#</th>
                      <th className="px-3 py-1.5 font-medium">Tipo</th>
                      <th className="px-3 py-1.5 font-medium">Dentro</th>
                      <th className="px-3 py-1.5 text-right font-medium">Empieza en</th>
                      <th className="px-3 py-1.5 text-right font-medium">Tamaño</th>
                    </tr>
                  </thead>
                  <tbody>
                    {layout.partitions.map((p) => (
                      <tr key={p.index} className="border-t border-line/60">
                        <td className="px-3 py-1.5 text-mute">{p.index}</td>
                        <td className="px-3 py-1.5 text-ink">
                          {p.kind}
                          {p.boot ? " · activa" : ""}
                          {p.name ? <span className="text-mute"> · {p.name}</span> : null}
                        </td>
                        <td className="px-3 py-1.5 text-dim">{p.fs || "—"}</td>
                        <td className="tabular px-3 py-1.5 text-right font-mono text-dim">{bytes(p.offset)}</td>
                        <td className="tabular px-3 py-1.5 text-right font-mono text-ink">{bytes(p.size)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            <div className="flex flex-wrap gap-2">
              <Button kind="secondary" size="sm" disabled={busy !== null} onClick={() => void run("backup", () => partitionsApi.backup(disk.number, disk.model), (b) => (toast("ok", "Copia de la tabla guardada."), setBackups([b, ...backups])))}>
                {busy === "backup" ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />} Guardar copia de la tabla
              </Button>
              <Button kind="secondary" size="sm" disabled={busy !== null} onClick={() => void run("find", () => partitionsApi.findLost(disk.number), setFound)}>
                {busy === "find" ? <Loader2 size={13} className="animate-spin" /> : <Search size={13} />} Buscar particiones perdidas
              </Button>
              {disk.system && sysLetter && (
                <Button kind="secondary" size="sm" disabled={busy !== null} onClick={() => void repairBoot()}>
                  {busy === "boot" ? <Loader2 size={13} className="animate-spin" /> : <Wrench size={13} />} Reparar el arranque de Windows
                </Button>
              )}
            </div>
            <TaskStatus task={`part-scan:${disk.number}`} active={busy === "find"} fallback="Mirando el espacio sin asignar…" />
            {disk.system && <p className="text-[11px] text-mute">Es el disco de Windows: se puede mirar y reparar el arranque, pero no se escribe su tabla con Windows en marcha.</p>}
          </div>
        </Card>
      )}

      {found && disk && (
        <Card title="Particiones encontradas" icon={<Search size={14} />}>
          <div className="space-y-2 p-4">
            {found.length === 0 ? (
              <p className="text-sm text-dim">No se encontró ningún resto de NTFS, FAT32 o exFAT en el espacio sin asignar. Si el disco estaba formateado de nuevo, prueba la recuperación de archivos «a fondo» en Datos del equipo.</p>
            ) : (
              found.map((f) => (
                <div key={f.offset} className="flex flex-wrap items-center gap-3 rounded-lg border border-line bg-panel-2 px-3 py-2">
                  <div className="min-w-0 flex-1">
                    <div className="text-sm text-ink">
                      {f.fs} · {bytes(f.size, 0)} <span className="text-mute">· empieza en {bytes(f.offset)}</span>
                    </div>
                    <div className="text-[11px] text-mute">{f.detail}</div>
                  </div>
                  <span className={`rounded-full border px-2 py-px text-[10px] ${f.confidence === "alta" ? PILL.ok : f.confidence === "media" ? PILL.warn : PILL.bad}`}>Fiabilidad {f.confidence}</span>
                  <Button size="sm" disabled={busy !== null || disk.system} onClick={() => setTyped({ kind: "partition", f })}>
                    Restaurar
                  </Button>
                </div>
              ))
            )}
            {found.length > 0 && <p className="text-[11px] text-mute">Restaurar da de alta la partición en la tabla, sin tocar su contenido. Si luego no abre bien, se devuelve la tabla con la copia que se guarda antes.</p>}
          </div>
        </Card>
      )}

      {backups.length > 0 && disk && (
        <Card title="Copias de la tabla" icon={<History size={14} />}>
          <ul className="divide-y divide-line/60">
            {backups.map((b) => (
              <li key={b.path} className="flex items-center gap-3 px-4 py-2 text-xs">
                <span className="flex-1 text-dim">
                  {date(b.created)} · {SCHEME[b.scheme as keyof typeof SCHEME] ?? b.scheme} · {bytes(b.size, 0)}
                </span>
                <Button kind="secondary" size="sm" disabled={busy !== null || disk.system} onClick={() => setTyped({ kind: "table", b })}>
                  Volver a esta
                </Button>
              </li>
            ))}
          </ul>
        </Card>
      )}

      {typed?.kind === "partition" && disk && (
        <TypedConfirm
          title="Restaurar la partición"
          body={`Se dará de alta una partición ${typed.f.fs} de ${bytes(typed.f.size, 0)} en el disco ${disk.number} (${disk.model}). Se guarda antes una copia de la tabla y se comprueba el resultado; si algo no cuadra, la tabla vuelve a como estaba.`}
          word="RESTAURAR"
          confirmLabel="Restaurar la partición"
          onClose={() => setTyped(null)}
          onConfirm={() => {
            const f = typed.f;
            setTyped(null);
            void run("restore", () => partitionsApi.restore(disk.number, disk.model, f), (l) => (tableChanged(l), setFound(null), toast("ok", "Partición restaurada. Ya debería verse en el Explorador.")));
          }}
        />
      )}
      {typed?.kind === "table" && disk && (
        <TypedConfirm
          title="Volver a una copia de la tabla"
          body={`Se escribirá la tabla de particiones del disco ${disk.number} guardada el ${date(typed.b.created)}. Los cambios hechos en las particiones desde entonces se pierden.`}
          word="RESTAURAR"
          confirmLabel="Volver a esta copia"
          onClose={() => setTyped(null)}
          onConfirm={() => {
            const b = typed.b;
            setTyped(null);
            void run("restore", () => partitionsApi.restoreTable(b.path, disk.number), (l) => (tableChanged(l), toast("ok", "Tabla restaurada.")));
          }}
        />
      )}
      {dialog}
    </div>
  );
}
