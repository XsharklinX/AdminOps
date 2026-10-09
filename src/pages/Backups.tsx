// Copias de seguridad: no basta con que exista una copia. Se comprueba que es reciente, que está
// en otro disco, que cubre lo importante y que un archivo de prueba se puede leer.
import { CheckCircle2, CircleAlert, DatabaseBackup, FolderOutput, Loader2, Pencil, Plus, RefreshCw, Trash2, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, EmptyState, inputClass, Loading, Modal } from "../components/ui";
import { backupsApi, disksApi, type BackupSet, type BackupStatus } from "../lib/api";

const TONE = {
  ok: { cls: "border-ok/40 bg-ok/10 text-ok", Icon: CheckCircle2, label: "Protegido" },
  warn: { cls: "border-warn/40 bg-warn/10 text-warn", Icon: CircleAlert, label: "Con avisos" },
  bad: { cls: "border-bad/40 bg-bad/10 text-bad", Icon: XCircle, label: "Sin protección fiable" },
} as const;
const COLOR = { ok: "text-ok", warn: "text-warn", bad: "text-bad" } as const;
const date = (s: number) => new Date(s * 1000).toLocaleString("es", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

export function Backups() {
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [sets, setSets] = useState<BackupSet[] | null>(null);
  const [status, setStatus] = useState<BackupStatus[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [edit, setEdit] = useState<BackupSet | null>(null);

  const load = useCallback(() => {
    void Promise.all([backupsApi.get(), backupsApi.status()])
      .then(([s, st]) => (setSets(s), setStatus(st)))
      .catch((e) => toast("error", String(e)));
  }, [toast]);
  useEffect(load, [load]);

  const check = async (id: string | null) => {
    setBusy(id ?? "all");
    try {
      setStatus(await backupsApi.check(id));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const run = async (s: BackupSet) => {
    const ok = await confirm({
      title: `Copiar ahora «${s.name}»`,
      body: `Copia a ${s.dest} los archivos nuevos o cambiados de ${s.sources.length} carpetas. No borra nada del destino ni pisa lo que allí sea más nuevo.`,
      confirmLabel: "Copiar ahora",
    });
    if (!ok) return;
    setBusy(`run:${s.id}`);
    try {
      toast("ok", await backupsApi.run(s.id));
      await check(s.id);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const save = async (next: BackupSet[]) => {
    try {
      setSets(await backupsApi.save(next));
      setStatus(await backupsApi.status());
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (sets === null) return <Loading page />;
  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="min-w-60 flex-1 text-sm text-dim">
          Que exista una copia no basta. Aquí se comprueba que es <b className="font-medium text-ink">reciente</b>, que está en <b className="font-medium text-ink">otro disco</b>, que <b className="font-medium text-ink">cubre lo importante</b> y que un archivo de
          prueba <b className="font-medium text-ink">se puede leer</b>. Lo que falla aparece en «Hoy» y en el informe del cliente.
        </p>
        {sets.length > 0 && (
          <Button kind="secondary" onClick={() => void check(null)} disabled={busy !== null}>
            {busy === "all" ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />} Comprobar todas
          </Button>
        )}
        <Button onClick={() => void backupsApi.defaults().then((d) => setEdit({ id: "", name: "Mis archivos", dest: "", sources: d }))}>
          <Plus size={14} /> Añadir copia
        </Button>
      </div>
      <TaskStatus task="backup-check" active={busy === "all" || (busy !== null && !busy.startsWith("run:"))} fallback="Mirando las copias…" />

      {sets.length === 0 ? (
        <EmptyState icon={<DatabaseBackup size={28} />} title="Todavía no hay copias que vigilar">
          Añade dónde está la copia (un disco externo, otro disco o una carpeta de red) y qué carpetas debería proteger: AdminOps comprobará cada día que sigue sirviendo.
        </EmptyState>
      ) : (
        sets.map((s) => {
          const st = status.find((x) => x.id === s.id);
          const t = st ? TONE[st.level] : null;
          return (
            <Card key={s.id} title={s.name} icon={<DatabaseBackup size={14} />} right={st ? <span className="text-[11px] text-mute">comprobada el {date(st.checked)}</span> : undefined}>
              <div className="space-y-3 p-4">
                <div className="flex flex-wrap items-center gap-3">
                  <div className="min-w-0 flex-1 text-xs text-dim">
                    <span className="font-mono text-ink">{s.dest}</span>
                    <span className="text-mute"> · {s.sources.length} carpetas de origen</span>
                  </div>
                  {t && (
                    <span className={`flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs ${t.cls}`}>
                      <t.Icon size={12} /> {t.label}
                    </span>
                  )}
                </div>
                {st ? (
                  <ul className="divide-y divide-line/60 rounded-lg border border-line">
                    {st.checks.map((c) => (
                      <li key={c.id} className="flex items-start gap-3 px-3 py-2 text-xs">
                        <span className="w-44 shrink-0 text-dim" title={c.label}>
                          <span className="block truncate">{c.label}</span>
                        </span>
                        <span className={`min-w-0 flex-1 ${COLOR[c.level]}`}>{c.text}</span>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="text-xs text-mute">Todavía no se ha comprobado.</p>
                )}
                <div className="flex flex-wrap gap-2">
                  <Button kind="secondary" size="sm" onClick={() => void check(s.id)} disabled={busy !== null}>
                    {busy === s.id ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />} Comprobar
                  </Button>
                  <Button kind="secondary" size="sm" onClick={() => void run(s)} disabled={busy !== null}>
                    {busy === `run:${s.id}` ? <Loader2 size={13} className="animate-spin" /> : <FolderOutput size={13} />} Copiar ahora
                  </Button>
                  <Button kind="ghost" size="sm" onClick={() => setEdit(s)}>
                    <Pencil size={13} /> Editar
                  </Button>
                  <Button kind="ghost" size="sm" onClick={() => void save(sets.filter((x) => x.id !== s.id))}>
                    <Trash2 size={13} /> Quitar
                  </Button>
                </div>
                <TaskStatus task="backup-run" active={busy === `run:${s.id}`} fallback="Copiando…" />
              </div>
            </Card>
          );
        })
      )}

      {edit && (
        <SetDialog
          initial={edit}
          onClose={() => setEdit(null)}
          onSave={(v) => {
            setEdit(null);
            void save(edit.id ? sets.map((x) => (x.id === edit.id ? v : x)) : [...sets, v]).then(() => check(null));
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function SetDialog({ initial, onClose, onSave }: { initial: BackupSet; onClose: () => void; onSave: (s: BackupSet) => void }) {
  const [v, setV] = useState(initial);
  const [extra, setExtra] = useState("");
  const toast = useToast();
  const ok = v.name.trim() !== "" && v.dest.trim() !== "" && v.sources.length > 0;
  return (
    <Modal
      title={initial.id ? "Editar la copia" : "Añadir una copia"}
      onClose={onClose}
      width="w-[560px]"
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button disabled={!ok} onClick={() => onSave(v)}>
            Guardar
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <label className="block text-xs text-dim">
          Nombre
          <input value={v.name} onChange={(e) => setV({ ...v, name: e.target.value })} className={`mt-1 ${inputClass}`} />
        </label>
        <div>
          <span className="block text-xs text-dim">Dónde está la copia (otro disco, disco externo o carpeta de red)</span>
          <div className="mt-1 flex gap-2">
            <input value={v.dest} onChange={(e) => setV({ ...v, dest: e.target.value })} placeholder="E:\Copias o \\servidor\copias" className={inputClass} aria-label="Destino de la copia" />
            <Button kind="secondary" onClick={() => void disksApi.pickFolder().then((p) => p && setV({ ...v, dest: p })).catch((e) => toast("error", String(e)))}>
              Elegir…
            </Button>
          </div>
        </div>
        <div>
          <span className="block text-xs text-dim">Carpetas que debería proteger</span>
          <ul className="mt-1 space-y-1">
            {v.sources.map((s) => (
              <li key={s} className="flex items-center gap-2 text-xs">
                <span className="min-w-0 flex-1 truncate font-mono text-ink" title={s}>
                  {s}
                </span>
                <button type="button" className="text-mute hover:text-bad" onClick={() => setV({ ...v, sources: v.sources.filter((x) => x !== s) })} aria-label={`Quitar ${s}`}>
                  <Trash2 size={12} />
                </button>
              </li>
            ))}
          </ul>
          <div className="mt-2 flex gap-2">
            <input value={extra} onChange={(e) => setExtra(e.target.value)} placeholder="Otra carpeta, p. ej. D:\Contabilidad" className={inputClass} aria-label="Otra carpeta de origen" />
            <Button
              kind="secondary"
              disabled={!extra.trim()}
              onClick={() => {
                setV({ ...v, sources: [...new Set([...v.sources, extra.trim()])] });
                setExtra("");
              }}
            >
              Añadir
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}
