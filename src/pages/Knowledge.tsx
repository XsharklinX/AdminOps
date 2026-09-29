import { ClipboardCopy, Copy, FileText, Lightbulb, MapPin, Network, Pencil, Plus, Search, StickyNote, Trash2, Wand2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { TagChip, TagInput } from "../components/contacts/Tags";
import { useConfirm, useToast } from "../components/feedback";
import { Button, EmptyState, inputClass, Loading, Modal } from "../components/ui";
import { libraryApi, type PlaceNote, type Solution, type TextTemplate } from "../lib/api";
import { norm } from "../lib/contacts";
import { AUTO_VARS, autoValues, fill, questions } from "../lib/templates";
import { PlaceNotes } from "../components/PlaceNotes";

type Tab = "solutions" | "templates" | "notes";

const TABS: { id: Tab; label: string; icon: typeof Lightbulb; hint: string }[] = [
  { id: "solutions", label: "Soluciones", icon: Lightbulb, hint: "Problema → lo que funcionó. Tu memoria de técnico, en todos los equipos." },
  { id: "templates", label: "Plantillas", icon: FileText, hint: "Textos que repites: respuestas, correos, pasos. Con variables que se rellenan solas." },
  { id: "notes", label: "Notas de equipos y redes", icon: StickyNote, hint: "Lo que hay que saber de un equipo o de una red. Aparecen solas al volver." },
];

const TAB_KEY = "adminops.knowledge.tab";

/** Conocimiento del técnico: soluciones, plantillas y notas por equipo y por red. */
export function Knowledge({ focus }: { focus: string | null }) {
  const [tab, setTab] = useState<Tab>(() => {
    try {
      const t = localStorage.getItem(TAB_KEY);
      return TABS.some((x) => x.id === t) ? (t as Tab) : "solutions";
    } catch {
      return "solutions";
    }
  });
  const [openId, setOpenId] = useState<string | null>(null);

  const choose = (t: Tab) => {
    setTab(t);
    try {
      localStorage.setItem(TAB_KEY, t);
    } catch {
      /* sin almacenamiento */
    }
  };

  // Desde la búsqueda global: «solution:<id>», «template:<id>» o «notes».
  useEffect(() => {
    if (!focus) return;
    const [kind, id] = focus.split(":");
    if (kind === "solution") {
      choose("solutions");
      setOpenId(id || "new");
    } else if (kind === "template") {
      choose("templates");
      setOpenId(id ?? null);
    } else if (kind === "notes") choose("notes");
  }, [focus]);

  const current = TABS.find((t) => t.id === tab)!;
  return (
    <div className="mx-auto max-w-5xl space-y-4 p-6">
      <div className="flex gap-1 border-b border-line">
        {TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => choose(t.id)}
            className={`-mb-px flex items-center gap-1.5 border-b-2 px-3.5 py-2 text-sm ${tab === t.id ? "border-neon font-medium text-ink" : "border-transparent text-dim hover:text-ink"}`}
          >
            <t.icon size={14} /> {t.label}
          </button>
        ))}
      </div>
      <p className="text-xs text-dim">{current.hint}</p>
      {tab === "solutions" && <Solutions openId={openId} />}
      {tab === "templates" && <Templates openId={openId} />}
      {tab === "notes" && <Notes />}
    </div>
  );
}

// ---------- Soluciones ----------

const EMPTY_SOLUTION: Solution = { id: "", title: "", problem: "", solution: "", tags: [] };

export function SolutionEditor({ initial, tags, onClose, onSaved }: { initial: Solution; tags: string[]; onClose: () => void; onSaved?: (s: Solution) => void }) {
  const [s, setS] = useState(initial);
  const toast = useToast();
  const save = async () => {
    if (!s.title.trim() || !s.solution.trim()) return toast("error", "Pon un título y lo que funcionó.");
    try {
      const saved = await libraryApi.save("solutions", { ...s, title: s.title.trim() });
      toast("ok", "Solución guardada.");
      onSaved?.(saved);
      onClose();
    } catch (e) {
      toast("error", String(e));
    }
  };
  return (
    <Modal
      title={s.id ? "Editar solución" : "Nueva solución"}
      onClose={onClose}
      width="w-[680px] max-w-[95vw]"
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save}>Guardar</Button>
        </>
      }
    >
      <div className="space-y-3">
        <label className="block">
          <span className="mb-1 block text-[11px] text-mute">Título (cómo lo buscarías)</span>
          <input autoFocus value={s.title} onChange={(e) => setS({ ...s, title: e.target.value })} placeholder="Outlook pide la contraseña sin parar" className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-[11px] text-mute">Problema (síntomas, mensaje de error)</span>
          <textarea value={s.problem} onChange={(e) => setS({ ...s, problem: e.target.value })} rows={3} className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-[11px] text-mute">Lo que funcionó (pasos)</span>
          <textarea value={s.solution} onChange={(e) => setS({ ...s, solution: e.target.value })} rows={7} className={`${inputClass} font-mono text-xs`} />
        </label>
        <div>
          <span className="mb-1 block text-[11px] text-mute">Etiquetas</span>
          <TagInput value={s.tags} onChange={(t) => setS({ ...s, tags: t })} suggestions={tags} colors={{}} placeholder="Outlook, impresoras, red…" />
        </div>
      </div>
    </Modal>
  );
}

function Solutions({ openId }: { openId: string | null }) {
  const [list, setList] = useState<Solution[] | null>(null);
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  const [open, setOpen] = useState<string | null>(openId);
  const [editing, setEditing] = useState<Solution | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => libraryApi.list("solutions").then(setList).catch((e) => toast("error", String(e))), [toast]);
  useEffect(() => {
    load();
  }, [load]);
  useEffect(() => {
    if (openId === "new") setEditing({ ...EMPTY_SOLUTION });
    else if (openId) setOpen(openId);
  }, [openId]);

  const tags = useMemo(() => [...new Set((list ?? []).flatMap((s) => s.tags))].sort((a, b) => a.localeCompare(b, "es")), [list]);
  const shown = useMemo(() => {
    const words = norm(query).split(/\s+/).filter(Boolean);
    return (list ?? [])
      .filter((s) => (!tag || s.tags.includes(tag)) && words.every((w) => norm(`${s.title} ${s.problem} ${s.solution} ${s.tags.join(" ")}`).includes(w)))
      .sort((a, b) => (b.uses ?? 0) - (a.uses ?? 0) || (b.updated ?? 0) - (a.updated ?? 0));
  }, [list, query, tag]);

  const copy = (s: Solution) => {
    navigator.clipboard.writeText(`${s.title}\n\n${s.solution}`).then(() => toast("ok", "Solución copiada."));
    libraryApi.touch("solutions", s.id).catch(() => {});
  };
  const remove = async (s: Solution) => {
    if (!(await confirm({ title: "Borrar solución", body: `Se borrará «${s.title}».`, confirmLabel: "Borrar", danger: true }))) return;
    await libraryApi.remove("solutions", s.id).catch((e) => toast("error", String(e)));
    load();
  };

  return (
    <div className="space-y-3">
      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search size={14} className="absolute top-2.5 left-3 text-mute" />
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Busca por síntoma, programa o error…" className={`${inputClass} pl-9`} />
        </div>
        <Button onClick={() => setEditing({ ...EMPTY_SOLUTION })}>
          <Plus size={14} /> Nueva solución
        </Button>
      </div>
      {tags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {tags.map((t) => (
            <TagChip key={t} name={t} active={tag === t} onClick={() => setTag(tag === t ? null : t)} />
          ))}
        </div>
      )}
      {list === null ? (
        <Loading />
      ) : shown.length === 0 ? (
        list.length ? (
          <EmptyState title="Nada coincide con la búsqueda." />
        ) : (
          <EmptyState
            icon={<Lightbulb size={22} />}
            title="Aún no hay soluciones"
            action={
              <Button onClick={() => setEditing({ ...EMPTY_SOLUTION })}>
                <Plus size={14} /> Primera solución
              </Button>
            }
          >
            Apunta aquí lo que te funcionó (o guárdalo al terminar una sesión de servicio) y lo encontrarás en cualquier equipo con Ctrl+K.
          </EmptyState>
        )
      ) : (
        shown.map((s) => (
          <div key={s.id} className={`rounded-xl border bg-panel ${open === s.id ? "border-neon/50" : "border-line"}`}>
            <button onClick={() => setOpen(open === s.id ? null : s.id)} className="flex w-full items-start gap-2.5 px-4 py-3 text-left">
              <Lightbulb size={15} className="mt-0.5 shrink-0 text-neon" />
              <span className="min-w-0 flex-1">
                <span className="block text-sm font-medium text-ink">{s.title}</span>
                {s.problem && open !== s.id && <span className="block truncate text-xs text-dim">{s.problem}</span>}
              </span>
              <span className="flex shrink-0 flex-wrap justify-end gap-1">
                {s.tags.map((t) => (
                  <TagChip key={t} name={t} />
                ))}
              </span>
            </button>
            {open === s.id && (
              <div className="border-t border-line px-4 py-3">
                {s.problem && (
                  <>
                    <p className="text-[11px] tracking-wide text-mute uppercase">Problema</p>
                    <p className="mb-3 text-sm whitespace-pre-wrap text-dim select-text">{s.problem}</p>
                  </>
                )}
                <p className="text-[11px] tracking-wide text-mute uppercase">Lo que funcionó</p>
                <pre className="mt-1 rounded-md border border-line bg-void/50 p-3 font-mono text-xs whitespace-pre-wrap text-ink select-text">{s.solution}</pre>
                <div className="mt-3 flex items-center gap-2">
                  <Button onClick={() => copy(s)}>
                    <ClipboardCopy size={13} /> Copiar
                  </Button>
                  <Button kind="ghost" onClick={() => setEditing({ ...s })}>
                    <Pencil size={13} /> Editar
                  </Button>
                  <button onClick={() => remove(s)} className="ml-auto p-1 text-mute hover:text-bad" title="Borrar">
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            )}
          </div>
        ))
      )}
      {editing && <SolutionEditor initial={editing} tags={tags} onClose={() => setEditing(null)} onSaved={(s) => (load(), setOpen(s.id))} />}
      {dialog}
    </div>
  );
}

// ---------- Plantillas ----------

const EMPTY_TEMPLATE: TextTemplate = { id: "", name: "", category: "", body: "" };

function UseTemplate({ t, onClose }: { t: TextTemplate; onClose: () => void }) {
  const qs = useMemo(() => questions(t.body), [t.body]);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [auto, setAuto] = useState<Record<string, string> | null>(null);
  const toast = useToast();
  useEffect(() => {
    autoValues(t.body).then(setAuto);
  }, [t.body]);
  const text = auto ? fill(t.body, auto, answers) : "";
  const copy = () => {
    navigator.clipboard.writeText(text).then(() => toast("ok", "Texto copiado."));
    libraryApi.touch("templates", t.id).catch(() => {});
    onClose();
  };
  return (
    <Modal
      title={t.name}
      onClose={onClose}
      width="w-[640px] max-w-[95vw]"
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cerrar
          </Button>
          <Button onClick={copy} disabled={!auto}>
            <Copy size={13} /> Copiar
          </Button>
        </>
      }
    >
      {qs.length > 0 && (
        <div className="mb-3 grid grid-cols-2 gap-2">
          {qs.map((q, i) => (
            <label key={q} className="block">
              <span className="mb-1 block text-[11px] text-mute">{q}</span>
              <input autoFocus={i === 0} value={answers[q] ?? ""} onChange={(e) => setAnswers({ ...answers, [q]: e.target.value })} className={inputClass} />
            </label>
          ))}
        </div>
      )}
      {auto ? (
        <pre className="max-h-80 overflow-y-auto rounded-md border border-line bg-void/50 p-3 text-sm whitespace-pre-wrap text-ink select-text">{text}</pre>
      ) : (
        <p className="text-sm text-mute">Rellenando las variables…</p>
      )}
    </Modal>
  );
}

function TemplateEditor({ initial, categories, onClose, onSaved }: { initial: TextTemplate; categories: string[]; onClose: () => void; onSaved: () => void }) {
  const [t, setT] = useState(initial);
  const toast = useToast();
  const save = async () => {
    if (!t.name.trim() || !t.body.trim()) return toast("error", "Pon un nombre y el texto.");
    try {
      await libraryApi.save("templates", { ...t, name: t.name.trim(), category: t.category.trim() });
      toast("ok", "Plantilla guardada.");
      onSaved();
      onClose();
    } catch (e) {
      toast("error", String(e));
    }
  };
  const insert = (v: string) => setT((x) => ({ ...x, body: `${x.body}{${v}}` }));
  return (
    <Modal
      title={t.id ? "Editar plantilla" : "Nueva plantilla"}
      onClose={onClose}
      width="w-[720px] max-w-[95vw]"
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save}>Guardar</Button>
        </>
      }
    >
      <div className="grid grid-cols-3 gap-3">
        <label className="col-span-2 block">
          <span className="mb-1 block text-[11px] text-mute">Nombre</span>
          <input autoFocus value={t.name} onChange={(e) => setT({ ...t, name: e.target.value })} placeholder="Aviso de mantenimiento" className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-[11px] text-mute">Categoría</span>
          <input list="tpl-cats" value={t.category} onChange={(e) => setT({ ...t, category: e.target.value })} placeholder="Correos, respuestas…" className={inputClass} />
          <datalist id="tpl-cats">
            {categories.map((c) => (
              <option key={c} value={c} />
            ))}
          </datalist>
        </label>
        <label className="col-span-3 block">
          <span className="mb-1 block text-[11px] text-mute">Texto</span>
          <textarea value={t.body} onChange={(e) => setT({ ...t, body: e.target.value })} rows={10} className={inputClass} />
        </label>
        <div className="col-span-3">
          <p className="mb-1 text-[11px] text-mute">
            Variables (clic para insertar). Para preguntar algo al usarla, escribe <code className="text-neon">{"{?Lo que quieres preguntar}"}</code>.
          </p>
          <div className="flex flex-wrap gap-1">
            {AUTO_VARS.map(([v, hint]) => (
              <button key={v} onClick={() => insert(v)} title={hint} className="rounded border border-line px-1.5 font-mono text-[11px] text-dim hover:border-neon/50 hover:text-neon">
                {`{${v}}`}
              </button>
            ))}
          </div>
        </div>
      </div>
    </Modal>
  );
}

function Templates({ openId }: { openId: string | null }) {
  const [list, setList] = useState<TextTemplate[] | null>(null);
  const [query, setQuery] = useState("");
  const [using, setUsing] = useState<TextTemplate | null>(null);
  const [editing, setEditing] = useState<TextTemplate | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => libraryApi.list("templates").then(setList).catch((e) => toast("error", String(e))), [toast]);
  useEffect(() => {
    load();
  }, [load]);
  useEffect(() => {
    if (openId && list) setUsing(list.find((t) => t.id === openId) ?? null);
  }, [openId, list]);

  const categories = useMemo(() => [...new Set((list ?? []).map((t) => t.category).filter(Boolean))].sort((a, b) => a.localeCompare(b, "es")), [list]);
  const groups = useMemo(() => {
    const words = norm(query).split(/\s+/).filter(Boolean);
    const shown = (list ?? []).filter((t) => words.every((w) => norm(`${t.name} ${t.category} ${t.body}`).includes(w)));
    const m = new Map<string, TextTemplate[]>();
    for (const t of shown.sort((a, b) => a.name.localeCompare(b.name, "es"))) {
      const k = t.category || "Sin categoría";
      m.set(k, [...(m.get(k) ?? []), t]);
    }
    return [...m.entries()].sort((a, b) => Number(a[0] === "Sin categoría") - Number(b[0] === "Sin categoría") || a[0].localeCompare(b[0], "es"));
  }, [list, query]);

  const remove = async (t: TextTemplate) => {
    if (!(await confirm({ title: "Borrar plantilla", body: `Se borrará «${t.name}».`, confirmLabel: "Borrar", danger: true }))) return;
    await libraryApi.remove("templates", t.id).catch((e) => toast("error", String(e)));
    load();
  };

  return (
    <div className="space-y-3">
      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search size={14} className="absolute top-2.5 left-3 text-mute" />
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Buscar plantilla…" className={`${inputClass} pl-9`} />
        </div>
        <Button onClick={() => setEditing({ ...EMPTY_TEMPLATE })}>
          <Plus size={14} /> Nueva plantilla
        </Button>
      </div>
      {list === null ? (
        <Loading />
      ) : !groups.length ? (
        list.length ? (
          <EmptyState title="Nada coincide con la búsqueda." />
        ) : (
          <EmptyState
            icon={<FileText size={22} />}
            title="Aún no hay plantillas"
            action={
              <Button onClick={() => setEditing({ ...EMPTY_TEMPLATE })}>
                <Plus size={14} /> Primera plantilla
              </Button>
            }
          >
            Por ejemplo: «Hola {"{?Nombre}"}, el equipo {"{equipo}"} ya está listo. Un saludo, {"{tecnico}"}».
          </EmptyState>
        )
      ) : (
        groups.map(([cat, items]) => (
          <section key={cat}>
            <h3 className="mb-1.5 text-[11px] font-medium tracking-wide text-mute uppercase">{cat}</h3>
            <div className="grid gap-2 md:grid-cols-2">
              {items.map((t) => (
                <div key={t.id} className="group flex items-start gap-2 rounded-xl border border-line bg-panel px-3.5 py-2.5">
                  <FileText size={14} className="mt-0.5 shrink-0 text-neon" />
                  <button onClick={() => setUsing(t)} className="min-w-0 flex-1 text-left">
                    <span className="block text-sm text-ink">{t.name}</span>
                    <span className="line-clamp-2 block text-xs text-mute">{t.body}</span>
                  </button>
                  <span className="flex shrink-0 gap-0.5 opacity-60 group-hover:opacity-100">
                    <button onClick={() => setUsing(t)} className="rounded p-1 text-neon hover:bg-neon/10" title="Usar">
                      <Wand2 size={13} />
                    </button>
                    <button onClick={() => setEditing({ ...t })} className="rounded p-1 text-mute hover:text-ink" title="Editar">
                      <Pencil size={13} />
                    </button>
                    <button onClick={() => remove(t)} className="rounded p-1 text-mute hover:text-bad" title="Borrar">
                      <Trash2 size={13} />
                    </button>
                  </span>
                </div>
              ))}
            </div>
          </section>
        ))
      )}
      {using && <UseTemplate t={using} onClose={() => setUsing(null)} />}
      {editing && <TemplateEditor initial={editing} categories={categories} onClose={() => setEditing(null)} onSaved={load} />}
      {dialog}
    </div>
  );
}

// ---------- Notas por equipo y por red ----------

function Notes() {
  const [list, setList] = useState<PlaceNote[] | null>(null);
  const [place, setPlace] = useState<Awaited<ReturnType<typeof libraryApi.place>> | null>(null);
  const [query, setQuery] = useState("");
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => libraryApi.list("notes").then(setList).catch((e) => toast("error", String(e))), [toast]);
  useEffect(() => {
    load();
    libraryApi.place().then(setPlace).catch(() => {});
  }, [load]);

  const groups = useMemo(() => {
    const words = norm(query).split(/\s+/).filter(Boolean);
    const m = new Map<string, { scope: PlaceNote["scope"]; label: string; here: boolean; items: PlaceNote[] }>();
    for (const n of list ?? []) {
      if (!words.every((w) => norm(`${n.label} ${n.text}`).includes(w))) continue;
      const k = `${n.scope}:${n.key}`;
      const here = !!place && (n.key === place.machine || n.key === place.network);
      if (!m.has(k)) m.set(k, { scope: n.scope, label: n.label, here, items: [] });
      m.get(k)!.items.push(n);
    }
    return [...m.values()].sort((a, b) => Number(b.here) - Number(a.here) || a.label.localeCompare(b.label, "es"));
  }, [list, place, query]);

  const remove = async (n: PlaceNote) => {
    if (!(await confirm({ title: "Borrar nota", body: "Se borrará la nota.", confirmLabel: "Borrar", danger: true }))) return;
    await libraryApi.remove("notes", n.id).catch((e) => toast("error", String(e)));
    load();
  };

  return (
    <div className="space-y-3">
      <PlaceNotes onChanged={load} />
      <div className="relative">
        <Search size={14} className="absolute top-2.5 left-3 text-mute" />
        <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Buscar en todas las notas…" className={`${inputClass} pl-9`} />
      </div>
      {list === null ? (
        <Loading />
      ) : (
        groups
          .filter((g) => !g.here)
          .map((g) => (
            <section key={`${g.scope}${g.label}`} className="rounded-xl border border-line bg-panel px-4 py-3">
              <h3 className="mb-1.5 flex items-center gap-1.5 text-sm text-ink">
                {g.scope === "machine" ? <MapPin size={13} className="text-neon" /> : <Network size={13} className="text-neon" />}
                {g.label || "Sin nombre"} <span className="text-[11px] text-mute">{g.scope === "machine" ? "equipo" : "red"}</span>
              </h3>
              {g.items.map((n) => (
                <div key={n.id} className="group flex items-start gap-2 py-1 text-sm">
                  <p className="flex-1 whitespace-pre-wrap text-dim select-text">{n.text}</p>
                  <button onClick={() => remove(n)} className="p-1 text-mute opacity-0 group-hover:opacity-100 hover:text-bad" title="Borrar">
                    <Trash2 size={12} />
                  </button>
                </div>
              ))}
            </section>
          ))
      )}
      {dialog}
    </div>
  );
}
