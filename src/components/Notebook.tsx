import { listen } from "@tauri-apps/api/event";
import { Pin, Plus, Search, StickyNote, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { logQuietly } from "../lib/api/core";
import { checkCount, CHECK_RE, notesApi, NOTES_CHANGED_EVENT, toggleCheck, type Note } from "../lib/api/notebook";
import { useConfirm, useToast } from "./feedback";
import { Button, EmptyState } from "./ui";

const fmt = (ms: number) => new Date(ms).toLocaleString("es", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

/** El texto de la tarjeta: las líneas «[ ] algo» son casillas que se marcan con un clic (sin abrir la nota). */
function NoteBody({ text, onToggle }: { text: string; onToggle: (line: number) => void }) {
  const lines = text.split("\n");
  const shown = lines.slice(0, 7);
  return (
    <div className="mt-1 space-y-0.5 text-[13px] text-dim">
      {shown.map((l, i) => {
        const m = CHECK_RE.exec(l);
        if (!m) return l.trim() ? <p key={i} className="truncate">{l}</p> : null;
        const done = m[2] !== " ";
        return (
          <label key={i} className="flex cursor-pointer items-center gap-1.5" onClick={(e) => e.stopPropagation()}>
            <input type="checkbox" className="accent-[var(--color-neon)]" checked={done} onChange={() => onToggle(i)} />
            <span className={`truncate ${done ? "text-mute line-through" : "text-ink"}`}>{m[3]}</span>
          </label>
        );
      })}
      {lines.length > shown.length && <p className="text-mute">…</p>}
    </div>
  );
}

/**
 * El cuaderno de «Notas»: todo lo apuntado con la nota rápida (Ctrl+Alt+N o la barra del cohete), en una galería
 * de tarjetas. Buscar, fijar arriba, abrir para editar y borrar.
 */
export function Notebook({ openId = null, onOpened }: { openId?: string | null; onOpened?: () => void } = {}) {
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
  // Pedida desde Ctrl+K: se abre como al tocar su tarjeta (pregunta si la abierta tiene cambios) y se avisa de que
  // ya está, para poder pedirla otra vez.
  const openRef = useRef<(n: Note | null) => Promise<void>>(async () => {});
  useEffect(() => {
    if (!openId || !notes) return;
    const n = notes.find((x) => x.id === openId);
    onOpened?.();
    if (n) void openRef.current(n);
  }, [openId, notes, onOpened]);
  const shown = useMemo(() => {
    const w = q.trim().toLowerCase();
    return (notes ?? []).filter((n) => !w || `${n.title} ${n.text}`.toLowerCase().includes(w));
  }, [notes, q]);
  const dirty = !!edit && (!orig || edit.title !== orig.title || edit.text !== orig.text || (edit.images ?? []).join("|") !== (orig.images ?? []).join("|"));
  // Pegar una captura en la nota (Ctrl+V con una imagen en el portapapeles).
  const pasteImage = (e: React.ClipboardEvent) => {
    if (!edit) return;
    const files = Array.from(e.clipboardData.files).filter((f) => f.type.startsWith("image/"));
    if (!files.length) return;
    e.preventDefault();
    for (const f of files) {
      if (f.size > 2_200_000) {
        toast("error", "La imagen es demasiado grande (unos 2 MB como mucho).");
        continue;
      }
      const r = new FileReader();
      r.onload = () =>
        setEditState((cur) => {
          if (!cur || typeof r.result !== "string") return cur;
          const images = cur.images ?? [];
          if (images.length >= 6) {
            toast("error", "Como mucho 6 imágenes por nota.");
            return cur;
          }
          return { ...cur, images: [...images, r.result] };
        });
      r.readAsDataURL(f);
    }
  };
  // Salir de la pestaña o de la página con cambios sin guardar: se guardan solos (no hay a quién preguntar).
  const pending = useRef<Note | null>(null);
  pending.current = dirty && edit && (edit.title.trim() || edit.text.trim() || (edit.images ?? []).length) ? edit : null;
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
  openRef.current = open;
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
  const blank: Note = { id: "", title: "", text: "", created: 0, updated: 0, pinned: false, images: [] };
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
          <textarea autoFocus value={edit.text} onChange={(e) => setEditState({ ...edit, text: e.target.value })} onPaste={pasteImage} placeholder="Escribe… «[ ] algo» hace una casilla; Ctrl+V pega una captura." className="h-40 w-full resize-y rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none focus:border-neon/50" />
          {(edit.images ?? []).length > 0 && (
            <div className="mt-2 flex flex-wrap gap-2">
              {(edit.images ?? []).map((src, i) => (
                <div key={i} className="group/img relative">
                  <img src={src} alt={`Imagen ${i + 1} de la nota`} className="h-20 w-auto max-w-40 rounded-md border border-line object-cover" />
                  <button
                    type="button"
                    aria-label={`Quitar la imagen ${i + 1}`}
                    className="absolute -right-1.5 -top-1.5 rounded-full border border-line bg-panel px-1 text-[11px] text-mute hover:text-bad"
                    onClick={() => setEditState({ ...edit, images: (edit.images ?? []).filter((_, j) => j !== i) })}
                  >
                    ✕
                  </button>
                </div>
              ))}
            </div>
          )}
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
              {(n.images ?? []).length > 0 && <img src={n.images[0]} alt="" className="mt-2 h-24 w-full rounded-md border border-line object-cover" />}
              <NoteBody text={n.title ? n.text : n.text.split("\n").slice(1).join("\n")} onToggle={(line) => {
                  const next = { ...n, text: toggleCheck(n.text, n.title ? line : line + 1) };
                  // Al momento en la lista: la siguiente casilla parte de esta, sin esperar al guardado.
                  setNotes((l) => l?.map((x) => (x.id === n.id ? next : x)) ?? l);
                  void save(next, false);
                }} />
              {(() => {
                const c = checkCount(n.text);
                return c.total > 0 ? <span className={`mt-1 text-[11px] ${c.done === c.total ? "text-ok" : "text-mute"}`}>{c.done} de {c.total} hechas</span> : null;
              })()}
              <span className="mt-auto pt-2 text-[11px] text-mute">{fmt(n.updated)}</span>
            </article>
          ))}
        </div>
      )}
      {dialog}
    </div>
  );
}
