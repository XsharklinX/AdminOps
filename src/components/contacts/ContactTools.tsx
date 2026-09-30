import { ArchiveRestore, Check, Copy, History, Loader2, Pencil, Save, Tags, Trash2, Undo2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../feedback";
import { Button, inputClass, Modal, Loading } from "../ui";
import { contactsApi, type Contact } from "../../lib/api";
import { findDuplicates, norm } from "../../lib/contacts";
import { ColorPicker, colorOf, TagChip, type TagColors } from "./Tags";

export type ToolTab = "tags" | "duplicates" | "trash" | "backups";

const TABS: [ToolTab, string, typeof Tags][] = [
  ["tags", "Etiquetas", Tags],
  ["duplicates", "Duplicados", Copy],
  ["trash", "Papelera", Trash2],
  ["backups", "Copias", History],
];

const when = (secs: number) => new Date(secs * 1000).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" });

/** Mantenimiento de la agenda. */
export function ContactTools({ tab: initial, contacts, colors, onChanged, onClose }: { tab: ToolTab; contacts: Contact[]; colors: TagColors; onChanged: () => void; onClose: () => void }) {
  const [tab, setTab] = useState<ToolTab>(initial);
  const live = useMemo(() => contacts.filter((c) => !c.deleted), [contacts]);
  const trash = useMemo(() => contacts.filter((c) => c.deleted).sort((a, b) => (b.deleted ?? 0) - (a.deleted ?? 0)), [contacts]);
  const dups = useMemo(() => findDuplicates(live), [live]);
  const count = { tags: 0, duplicates: dups.length, trash: trash.length, backups: 0 };

  return (
    <Modal title="Gestionar contactos" onClose={onClose} width="w-[720px] max-w-[95vw]">
      <div className="-mt-1 mb-4 flex gap-1 border-b border-line">
        {TABS.map(([id, label, Icon]) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={`-mb-px flex items-center gap-1.5 border-b-2 px-3 py-2 text-sm ${tab === id ? "border-neon text-ink" : "border-transparent text-dim hover:text-ink"}`}
          >
            <Icon size={13} /> {label}
            {count[id] > 0 && <span className="rounded-full bg-neon/15 px-1.5 text-[10px] text-neon">{count[id]}</span>}
          </button>
        ))}
      </div>
      {tab === "tags" && <TagManager contacts={live} colors={colors} onChanged={onChanged} />}
      {tab === "duplicates" && <Duplicates groups={dups} onChanged={onChanged} />}
      {tab === "trash" && <Trash items={trash} onChanged={onChanged} />}
      {tab === "backups" && <Backups onChanged={onChanged} />}
    </Modal>
  );
}

function TagManager({ contacts, colors, onChanged }: { contacts: Contact[]; colors: TagColors; onChanged: () => void }) {
  const [renaming, setRenaming] = useState<string | null>(null);
  const [to, setTo] = useState("");
  const [coloring, setColoring] = useState<string | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const tags = useMemo(() => {
    const m = new Map<string, number>();
    for (const c of contacts) for (const t of c.tags) m.set(t, (m.get(t) ?? 0) + 1);
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0], "es"));
  }, [contacts]);

  const rename = async (from: string) => {
    const merging = tags.some(([t]) => norm(t) === norm(to) && norm(t) !== norm(from));
    if (merging && !(await confirm({ title: "Fusionar etiquetas", body: `«${to}» ya existe: los contactos con «${from}» pasarán a tener «${to}».`, confirmLabel: "Fusionar" }))) return;
    try {
      const n = await contactsApi.renameTag(from, to);
      toast("ok", `${merging ? "Fusionada" : "Renombrada"} en ${n} contacto(s).`);
      setRenaming(null);
      onChanged();
    } catch (e) {
      toast("error", String(e));
    }
  };

  const remove = async (t: string, n: number) => {
    if (!(await confirm({ title: "Borrar etiqueta", body: `Se quitará «${t}» de ${n} contacto(s). Los contactos no se borran.`, confirmLabel: "Borrar etiqueta", danger: true }))) return;
    await contactsApi.deleteTag(t).catch((e) => toast("error", String(e)));
    onChanged();
  };

  const setColor = async (t: string, color: string) => {
    await contactsApi.setTagColor(t, color).catch((e) => toast("error", String(e)));
    setColoring(null);
    onChanged();
  };

  if (!tags.length) return <p className="py-6 text-center text-sm text-mute">Todavía no hay etiquetas.</p>;
  return (
    <div className="space-y-1">
      <p className="mb-2 text-xs text-dim">Renombrar una etiqueta con el nombre de otra las fusiona. Los colores se ven en toda la agenda.</p>
      {tags.map(([t, n]) => (
        <div key={t} className="rounded-md px-2 py-1.5 hover:bg-panel-2">
          <div className="flex items-center gap-2">
            {renaming === t ? (
              <>
                <input
                  autoFocus
                  value={to}
                  onChange={(e) => setTo(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") void rename(t);
                    if (e.key === "Escape") setRenaming(null);
                  }}
                  className={`${inputClass} h-8 py-1`}
                />
                <button onClick={() => rename(t)} className="p-1 text-neon" title="Guardar">
                  <Check size={14} />
                </button>
              </>
            ) : (
              <>
                <TagChip name={t} color={colorOf(colors, t)} />
                <span className="flex-1 text-[11px] text-mute">{n} contacto(s)</span>
                <button onClick={() => setColoring(coloring === t ? null : t)} className="rounded px-1.5 py-0.5 text-[11px] text-mute hover:text-ink">
                  Color
                </button>
                <button
                  onClick={() => {
                    setRenaming(t);
                    setTo(t);
                  }}
                  className="p-1 text-mute hover:text-ink"
                  title="Renombrar o fusionar"
                >
                  <Pencil size={12} />
                </button>
                <button onClick={() => remove(t, n)} className="p-1 text-mute hover:text-bad" title="Borrar etiqueta">
                  <Trash2 size={12} />
                </button>
              </>
            )}
          </div>
          {coloring === t && (
            <div className="mt-2 pl-1">
              <ColorPicker value={colorOf(colors, t) ?? ""} onChange={(c) => setColor(t, c)} />
            </div>
          )}
        </div>
      ))}
      {dialog}
    </div>
  );
}

function Duplicates({ groups, onChanged }: { groups: Contact[][]; onChanged: () => void }) {
  const [keep, setKeep] = useState<Record<number, string>>({});
  const [busy, setBusy] = useState<number | null>(null);
  const toast = useToast();

  const merge = async (i: number, g: Contact[]) => {
    // Por defecto se queda el más completo.
    const score = (c: Contact) => [c.role, c.company, c.extension, c.phone, c.mobile, c.email, c.reason].filter(Boolean).length + c.tags.length;
    const k = keep[i] ?? [...g].sort((a, b) => score(b) - score(a))[0].id;
    setBusy(i);
    try {
      await contactsApi.merge(
        k,
        g.map((c) => c.id).filter((id) => id !== k),
      );
      toast("ok", "Contactos fusionados. Los demás están en la papelera.");
      onChanged();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  if (!groups.length) return <p className="py-6 text-center text-sm text-mute">No hay duplicados. Se buscan por nombre, correo y teléfono.</p>;
  return (
    <div className="space-y-3">
      <p className="text-xs text-dim">Elige cuál se queda: recibirá lo que le falte de los otros (teléfonos, correos, etiquetas, notas). Los otros van a la papelera.</p>
      {groups.map((g, i) => (
        <div key={g.map((c) => c.id).join()} className="rounded-lg border border-line p-3">
          {g.map((c) => (
            <label key={c.id} className="flex cursor-pointer items-start gap-2 py-1 text-sm">
              <input type="radio" name={`dup-${i}`} checked={(keep[i] ?? "") === c.id} onChange={() => setKeep((k) => ({ ...k, [i]: c.id }))} className="mt-1 accent-[var(--color-neon)]" />
              <span className="min-w-0">
                <span className="text-ink">{c.name}</span>
                <span className="block text-[11px] text-mute">
                  {[c.role, c.company, c.extension && `ext. ${c.extension}`, c.phone, c.email].filter(Boolean).join(" · ") || "Sin más datos"}
                </span>
              </span>
            </label>
          ))}
          <div className="mt-2 flex justify-end">
            <Button kind="ghost" onClick={() => merge(i, g)} disabled={busy !== null}>
              {busy === i && <Loader2 size={13} className="animate-spin" />} Fusionar {keep[i] ? "" : "(se queda el más completo)"}
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}

function Trash({ items, onChanged }: { items: Contact[]; onChanged: () => void }) {
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const act = async (ids: string[], op: "restore" | "purge") => {
    if (op === "purge" && !(await confirm({ title: "Borrar para siempre", body: `${ids.length} contacto(s) se borrarán sin posibilidad de recuperarlos.`, confirmLabel: "Borrar", danger: true }))) return;
    try {
      await contactsApi.bulk(ids, { op });
      toast("ok", op === "restore" ? "Recuperado." : "Borrado para siempre.");
      onChanged();
    } catch (e) {
      toast("error", String(e));
    }
  };
  if (!items.length) return <p className="py-6 text-center text-sm text-mute">La papelera está vacía. Lo que borres se puede recuperar aquí durante 30 días.</p>;
  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <p className="text-xs text-dim">Se vacía sola a los 30 días.</p>
        <div className="flex gap-2">
          <Button kind="ghost" onClick={() => act(items.map((c) => c.id), "restore")}>
            <Undo2 size={13} /> Recuperar todo
          </Button>
          <Button kind="danger" onClick={() => act(items.map((c) => c.id), "purge")}>
            Vaciar papelera
          </Button>
        </div>
      </div>
      {items.map((c) => (
        <div key={c.id} className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-panel-2">
          <span className="min-w-0 flex-1">
            <span className="text-sm text-ink">{c.name}</span>
            <span className="block text-[11px] text-mute">Borrado el {when(c.deleted ?? 0)}</span>
          </span>
          <button onClick={() => act([c.id], "restore")} className="flex items-center gap-1 rounded px-2 py-0.5 text-xs text-neon hover:bg-neon/10">
            <ArchiveRestore size={12} /> Recuperar
          </button>
          <button onClick={() => act([c.id], "purge")} className="p-1 text-mute hover:text-bad" title="Borrar para siempre">
            <Trash2 size={12} />
          </button>
        </div>
      ))}
      {dialog}
    </div>
  );
}

function Backups({ onChanged }: { onChanged: () => void }) {
  const [list, setList] = useState<{ id: number; contacts: number }[] | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const load = () => contactsApi.backups().then(setList).catch(() => setList([]));
  useEffect(() => {
    void load();
  }, []);

  const restore = async (b: { id: number; contacts: number }) => {
    if (!(await confirm({ title: "Volver a esta copia", body: `La agenda pasará a tener ${b.contacts} contacto(s), como el ${when(b.id)}. La agenda actual se guarda antes como otra copia.`, confirmLabel: "Restaurar" }))) return;
    try {
      await contactsApi.restoreBackup(b.id);
      toast("ok", "Agenda restaurada.");
      onChanged();
      void load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <div>
      <div className="mb-2 flex items-center justify-between gap-3">
        <p className="text-xs text-dim">AdminOps guarda sola una copia a la semana (las 5 últimas) junto a la agenda. Para tenerla fuera del USB, exporta a CSV o vCard.</p>
        <Button
          kind="ghost"
          onClick={() =>
            contactsApi
              .backupNow()
              .then(() => {
                toast("ok", "Copia hecha.");
                void load();
              })
              .catch((e) => toast("error", String(e)))
          }
        >
          <Save size={13} /> Copia ahora
        </Button>
      </div>
      {list === null ? (
        <Loading />
      ) : list.length === 0 ? (
        <p className="py-6 text-center text-sm text-mute">Aún no hay copias: la primera se hace al cambiar algo de la agenda.</p>
      ) : (
        list.map((b) => (
          <div key={b.id} className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-panel-2">
            <History size={13} className="text-mute" />
            <span className="flex-1 text-sm text-ink">{when(b.id)}</span>
            <span className="text-xs text-mute">{b.contacts} contacto(s)</span>
            <button onClick={() => restore(b)} className="rounded px-2 py-0.5 text-xs text-neon hover:bg-neon/10">
              Restaurar
            </button>
          </div>
        ))
      )}
      {dialog}
    </div>
  );
}
