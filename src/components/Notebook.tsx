import { listen } from "@tauri-apps/api/event";
import { Pin, Plus, Search, StickyNote, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { logQuietly } from "../lib/api/core";
import { notesApi, NOTES_CHANGED_EVENT, type Note } from "../lib/api/notebook";
import { useConfirm, useToast } from "./feedback";
import { Button, EmptyState } from "./ui";

const fmt = (ms: number) => new Date(ms).toLocaleString("es", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

/**
 * El cuaderno de «Notas»: todo lo apuntado con la nota rápida (Ctrl+Alt+N o la barra del cohete), en una galería
 * de tarjetas. Buscar, fijar arriba, abrir para editar y borrar.
 */
export function Notebook() {
  const [notes, setNotes] = useState<Note[] | null>(null);
  const [q, setQ] = useState("");
  const [edit, setEditState] = useState<Note | null>(null);
  // Cómo estaba la nota al abrirla: para saber si hay cambios sin guardar.
  const [orig, setOrig] = useState<Note | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const load = useCallback(() => {
    notesApi.list().then(setNotes, (e) => toast("error", String(e)));
  }, [toast]);
  useEffect(() => {
    load();
    const off = listen(NOTES_CHANGED_EVENT, load);
    return () => void off.then((f) => f());
  }, [load]);
  const shown = useMemo(() => {
    const w = q.trim().toLowerCase();
    return (notes ?? []).filter((n) => !w || `${n.title} ${n.text}`.toLowerCase().includes(w));
  }, [notes, q]);
  const dirty = !!edit && (!orig || edit.title !== orig.title || edit.text !== orig.text);
  // Salir de la pestaña o de la página con cambios sin guardar: se guardan solos (no hay a quién preguntar).
  const pending = useRef<Note | null>(null);
  pending.current = dirty && edit && (edit.title.trim() || edit.text.trim()) ? edit : null;
  useEffect(
    () => () => {
      // Desmontada no hay dónde enseñar el error: al menos queda en el registro de AdminOps.
      if (pending.current) void notesApi.save(pending.current).catch(logQuietly("Notebook.saveOnLeave"));
    },
    [],
  );
  const setEdit = (n: Note | null) => {
    setEditState(n);
    setOrig(n);
  };
  // Abrir otra nota (o cerrar) con cambios sin guardar: se pregunta antes de perderlos.
  const open = async (n: Note | null) => {
    if (n && edit && n.id && n.id === edit.id) return;
    if (dirty && !(await confirm({ title: "Cambios sin guardar", body: "La nota abierta tiene cambios que no se han guardado.", confirmLabel: "Descartarlos", danger: true }))) return;
    setEdit(n);
  };
  // Guardar desde el editor lo cierra; fijar otra nota desde su tarjeta no toca lo que se está escribiendo.
  const save = async (n: Note, fromEditor: boolean) => {
    try {
      const saved = await notesApi.save(n);
      if (fromEditor) setEdit(null);
      else if (edit && edit.id && edit.id === n.id) {
        setEditState({ ...edit, pinned: saved.pinned, updated: saved.updated });
        setOrig((o) => (o ? { ...o, pinned: saved.pinned, updated: saved.updated } : o));
      }
      load();
    } catch (e) {
      toast("error", String(e));
    }
  };
  const remove = async (n: Note) => {
    if (!(await confirm({ title: "Borrar la nota", body: `«${n.title || n.text.slice(0, 40)}» se borra del cuaderno.`, confirmLabel: "Borrar", danger: true }))) return;
    try {
      await notesApi.remove(n.id);
      setEdit(null);
    } catch (e) {
      // Si no se pudo borrar, el editor sigue abierto con la nota.
      toast("error", String(e));
    }
    load();
  };
  const blank: Note = { id: "", title: "", text: "", created: 0, updated: 0, pinned: false };
  return (
    <div>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <label className="flex min-w-0 flex-1 items-center gap-2 rounded-lg border border-line bg-panel px-3 py-2 focus-within:border-neon/50">
          <Search size={14} className="text-mute" />
          <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Buscar en las notas…" className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-mute" />
        </label>
        <Button onClick={() => void open(blank)}>
          <Plus size={14} /> Nota nueva
        </Button>
      </div>
      {edit && (
        <div className="mb-4 rounded-xl border border-neon/40 bg-panel p-3">
          <input value={edit.title} onChange={(e) => setEditState({ ...edit, title: e.target.value })} placeholder="Título (opcional)" className="mb-2 w-full rounded-md border border-line bg-void/60 px-3 py-2 text-sm font-medium text-ink outline-none focus:border-neon/50" />
          <textarea autoFocus value={edit.text} onChange={(e) => setEditState({ ...edit, text: e.target.value })} placeholder="Escribe…" className="h-40 w-full resize-y rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none focus:border-neon/50" />
          <div className="mt-2 flex items-center gap-2">
            <Button size="sm" onClick={() => void save(edit, true)}>
              Guardar
            </Button>
            <Button size="sm" kind="ghost" onClick={() => void open(null)}>
              Cancelar
            </Button>
            {edit.id && (
              <button type="button" className="ml-auto flex items-center gap-1 text-xs text-bad hover:underline" onClick={() => void remove(edit)}>
                <Trash2 size={12} /> Borrar
              </button>
            )}
          </div>
        </div>
      )}
      {notes && notes.length === 0 && !edit ? (
        <EmptyState icon={<StickyNote size={22} />} title="Aún no hay notas">
          Apunta con «Notas» en la barra del cohete o con Ctrl+Alt+N, o pulsa «Nota nueva».
        </EmptyState>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {shown.map((n) => (
            <article key={n.id} className={`group flex cursor-pointer flex-col rounded-xl border bg-panel p-3 shadow-elev-1 transition-colors hover:border-neon/50 ${n.pinned ? "border-warn/50" : "border-line"}`} onClick={() => void open(n)}>
              <div className="flex items-start gap-2">
                <h3 className="min-w-0 flex-1 truncate text-sm font-semibold text-ink">{n.title || n.text.split("\n")[0] || "Nota"}</h3>
                <button
                  type="button"
                  title={n.pinned ? "Quitar de arriba" : "Fijar arriba"}
                  className={`shrink-0 ${n.pinned ? "text-warn" : "text-mute opacity-0 group-hover:opacity-100"} hover:text-ink`}
                  onClick={(e) => {
                    e.stopPropagation();
                    void save({ ...n, pinned: !n.pinned }, false);
                  }}
                >
                  <Pin size={13} />
                </button>
              </div>
              <p className="mt-1 line-clamp-5 whitespace-pre-wrap text-[13px] text-dim">{n.title ? n.text : n.text.split("\n").slice(1).join("\n")}</p>
              <span className="mt-auto pt-2 text-[11px] text-mute">{fmt(n.updated)}</span>
            </article>
          ))}
        </div>
      )}
      {dialog}
    </div>
  );
}
