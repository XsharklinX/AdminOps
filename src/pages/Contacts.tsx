import {
  BookUser,
  ChevronDown,
  ChevronRight,
  Columns3,
  Download,
  HelpCircle,
  LayoutGrid,
  List,
  Plus,
  Save,
  Search,
  Settings2,
  Star,
  Table2,
  Tag,
  Trash2,
  Upload,
  UserRound,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { ContactDetail, type ContactActions } from "../components/contacts/ContactDetail";
import { ContactEditor } from "../components/contacts/ContactEditor";
import { ContactTools, type ToolTab } from "../components/contacts/ContactTools";
import { QuickDial } from "../components/contacts/QuickDial";
import { CardsView, DirectoryView, TableView, type ViewProps } from "../components/contacts/ContactViews";
import { colorOf, TagChip, TagInput, type TagColors } from "../components/contacts/Tags";
import { ChipRow } from "../components/ChipRow";
import { useConfirm, useToast } from "../components/feedback";
import { Button, EmptyState, inputClass, Loading } from "../components/ui";
import { logQuietly, contactsApi, officeApi, portalsApi, workApi, type Client, type Contact } from "../lib/api";
import type { PageId } from "../components/Sidebar";
import {
  applyFilters,
  cardText,
  COLUMNS,
  EMPTY_CONTACT,
  EMPTY_FILTERS,
  findDuplicates,
  GROUPS,
  groupContacts,
  hasFilters,
  loadView,
  norm,
  PREFIX_HELP,
  QUICK,
  saveView,
  SORTS,
  sortContacts,
  toCsv,
  toVcard,
  type ContactsView,
  type Filters,
  type ViewMode,
} from "../lib/contacts";
import { useLiveEffect } from "../lib/useLiveEffect";

const VIEWS: { id: ViewMode; label: string; icon: typeof LayoutGrid }[] = [
  { id: "cards", label: "Tarjetas", icon: LayoutGrid },
  { id: "table", label: "Tabla", icon: Table2 },
  { id: "directory", label: "Directorio", icon: List },
];

const selectClass = "h-8 rounded-md border border-line bg-void/60 px-2 text-xs text-ink outline-none focus:border-neon/50";

/** Agenda del técnico: a quién llamar y para qué. Es la misma en todos los equipos. */
export function Contacts({ focus, onNavigate }: { focus: string | null; onNavigate?: (page: PageId) => void }) {
  const [all, setAll] = useState<Contact[] | null>(null);
  const [colors, setColors] = useState<TagColors>({});
  const [clients, setClients] = useState<Client[]>([]);
  const [view, setViewState] = useState<ContactsView>(loadView);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [anchor, setAnchor] = useState<string | null>(null);
  const [detail, setDetail] = useState<string | null>(null);
  const [editing, setEditing] = useState<Contact | null>(null);
  const [tools, setTools] = useState<ToolTab | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [menu, setMenu] = useState<"export" | "columns" | "help" | "bulkTag" | "save" | null>(null);
  const [bulkTags, setBulkTags] = useState<string[]>([]);
  const [saveName, setSaveName] = useState("");
  const search = useRef<HTMLInputElement>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const setView = (patch: Partial<ContactsView>) =>
    setViewState((v) => {
      const next = { ...v, ...patch };
      saveView(next);
      return next;
    });
  const setFilters = (patch: Partial<Filters>) => setView({ filters: { ...view.filters, ...patch } });

  const load = useCallback(async () => {
    try {
      const [list, meta] = await Promise.all([contactsApi.list(), contactsApi.tagColors().catch(() => [])]);
      setAll(list);
      setColors(Object.fromEntries(meta.map((m) => [norm(m.name), m.color])));
    } catch (e) {
      toast("error", String(e));
    }
  }, [toast]);

  useLiveEffect(
    (vigente) => {
      void load();
      workApi.clients().then((c) => vigente() && setClients(c)).catch(logQuietly("Contacts"));
    },
    [load],
  );

  const live = useMemo(() => (all ?? []).filter((c) => !c.deleted), [all]);
  const byId = useMemo(() => new Map((all ?? []).map((c) => [c.id, c])), [all]);
  const filtered = useMemo(() => sortContacts(applyFilters(live, view.filters), view.sort), [live, view.filters, view.sort]);
  const groups = useMemo(() => groupContacts(filtered, view.group), [filtered, view.group]);
  const flat = useMemo(() => groups.flatMap((g) => g.items), [groups]);
  const trashCount = (all ?? []).length - live.length;
  const dupCount = useMemo(() => findDuplicates(live).length, [live]);

  const tagCounts = useMemo(() => {
    const m = new Map<string, number>();
    for (const c of live) for (const t of c.tags) m.set(t, (m.get(t) ?? 0) + 1);
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0], "es"));
  }, [live]);
  const tagNames = useMemo(() => tagCounts.map(([t]) => t), [tagCounts]);
  const companies = useMemo(() => [...new Set(live.map((c) => c.company).filter(Boolean))].sort((a, b) => a.localeCompare(b, "es")), [live]);
  // Marcación rápida: primero los favoritos, luego los que más se usan.
  const frequent = useMemo(
    () =>
      [...live]
        .filter((c) => c.favorite || c.uses > 0)
        .sort((a, b) => Number(b.favorite) - Number(a.favorite) || b.uses - a.uses || b.lastUsed - a.lastUsed)
        .slice(0, 8),
    [live],
  );
  const stats = useMemo(() => {
    const month = Date.now() / 1000 - 30 * 86_400;
    return {
      total: live.length,
      favorites: live.filter((c) => c.favorite).length,
      withExt: live.filter((c) => c.extension).length,
      companies: new Set(live.map((c) => c.company).filter(Boolean)).size,
      usedMonth: live.filter((c) => c.lastUsed > month).length,
      incomplete: live.filter((c) => !(c.phone || c.mobile || c.extension) || !c.email).length,
    };
  }, [live]);

  // Desde la búsqueda global (Ctrl+K): abrir la ficha del contacto.
  useEffect(() => {
    if (!focus || !all) return;
    if (byId.has(focus)) {
      setDetail(focus);
      window.setTimeout(() => document.getElementById(`contact-${focus}`)?.scrollIntoView({ block: "center", behavior: "smooth" }), 80);
    }
  }, [focus, all, byId]);

  // Los menús se cierran al hacer clic fuera de ellos.
  useEffect(() => {
    if (!menu) return;
    const onDown = (e: globalThis.MouseEvent) => {
      if (!(e.target instanceof Element) || !e.target.closest("[data-menu]")) setMenu(null);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [menu]);

  // Esc cierra la ficha; «/» o Ctrl+F van a la búsqueda.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
      if (e.key === "Escape" && detail && !editing && !tools) setDetail(null);
      if (!typing && (e.key === "/" || (e.ctrlKey && e.key.toLowerCase() === "f"))) {
        e.preventDefault();
        search.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [detail, editing, tools]);

  // ---------- Acciones (cuentan como uso para «Más usados») ----------

  const touch = (c: Contact) => {
    contactsApi.touch(c.id).catch(() => {});
    setAll((l) => l?.map((x) => (x.id === c.id ? { ...x, uses: x.uses + 1, lastUsed: Math.floor(Date.now() / 1000) } : x)) ?? l);
  };
  const run = (c: Contact, p: Promise<unknown>) => {
    touch(c);
    p.catch((e) => toast("error", String(e)));
  };

  const actions: ContactActions = {
    copy: (c, text, what) => run(c, navigator.clipboard.writeText(text).then(() => toast("ok", `${what} copiado.`))),
    call: (c, n) => run(c, contactsApi.call(n)),
    // Con el Correo de AdminOps configurado, el mensaje se escribe ahí; si no, en el programa de correo de Windows.
    email: (c, addr) =>
      run(
        c,
        portalsApi.compose(addr).then((inApp) => (inApp ? onNavigate?.("mail") : contactsApi.email(addr))),
      ),
    // Con Teams configurado en AdminOps, el chat se abre aquí; si no, en la
    // aplicación de Teams del equipo (o en el navegador).
    teams: (c, addr, call) =>
      run(
        c,
        portalsApi.teams(addr, call).then((inApp) => (inApp ? onNavigate?.("teams") : contactsApi.teams(addr, call))),
      ),
    copyCard: (c) => run(c, navigator.clipboard.writeText(cardText(c, c.substituteId ? byId.get(c.substituteId) : null)).then(() => toast("ok", "Tarjeta copiada."))),
    edit: (c) => setEditing({ ...c }),
    trash: async (c) => {
      await contactsApi.bulk([c.id], { op: "delete" }).catch((e) => toast("error", String(e)));
      if (detail === c.id) setDetail(null);
      toast("ok", `«${c.name}» está en la papelera (se puede recuperar 30 días).`);
      void load();
    },
    star: async (c) => {
      setAll((l) => l?.map((x) => (x.id === c.id ? { ...x, favorite: !x.favorite } : x)) ?? l);
      await contactsApi.bulk([c.id], { op: "favorite", value: !c.favorite }).catch((e) => toast("error", String(e)));
    },
    open: (c) => setDetail(c.id),
  };

  const save = async (c: Contact) => {
    try {
      const saved = await contactsApi.save(c);
      toast("ok", c.id ? "Contacto guardado." : "Contacto añadido.");
      await load();
      setDetail(saved.id);
      return true;
    } catch (e) {
      toast("error", String(e));
      return false;
    }
  };

  // ---------- Selección ----------

  const onSelect = (c: Contact, e: MouseEvent) => {
    setSelected((s) => {
      const n = new Set(s);
      if (e.shiftKey && anchor) {
        const a = flat.findIndex((x) => x.id === anchor);
        const b = flat.findIndex((x) => x.id === c.id);
        if (a >= 0 && b >= 0) flat.slice(Math.min(a, b), Math.max(a, b) + 1).forEach((x) => n.add(x.id));
      } else if (n.has(c.id)) n.delete(c.id);
      else n.add(c.id);
      return n;
    });
    setAnchor(c.id);
  };
  const selectAll = (on: boolean) => setSelected(on ? new Set(flat.map((c) => c.id)) : new Set());
  const selectedList = flat.filter((c) => selected.has(c.id));

  const bulk = async (action: Parameters<typeof contactsApi.bulk>[1], done: string) => {
    const ids = [...selected];
    try {
      const n = await contactsApi.bulk(ids, action);
      toast("ok", `${done} (${n}).`);
      if (action.op === "delete") setSelected(new Set());
      void load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  const bulkTrash = async () => {
    if (!(await confirm({ title: "Enviar a la papelera", body: `${selected.size} contacto(s) irán a la papelera. Se pueden recuperar durante 30 días.`, confirmLabel: "A la papelera", danger: true }))) return;
    void bulk({ op: "delete" }, "En la papelera");
  };

  // ---------- Importar y exportar ----------

  const importFile = async () => {
    try {
      const r = await contactsApi.importFile();
      if (!r) return;
      toast("ok", `${r.added} contacto(s) añadidos${r.updated ? `, ${r.updated} ya existían y se completaron` : ""}.`);
      void load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  const exportAs = async (kind: "csv" | "vcf", list: Contact[]) => {
    setMenu(null);
    try {
      const file = kind === "csv" ? await officeApi.exportCsv("Contactos", toCsv(list)) : await contactsApi.saveVcard("Contactos", toVcard(list));
      if (file) toast("ok", `${list.length} contacto(s) exportados.`);
    } catch (e) {
      toast("error", String(e));
    }
  };

  // ---------- Búsquedas guardadas ----------

  const saveSearch = () => {
    const name = saveName.trim();
    if (!name) return;
    setView({ saved: [...view.saved.filter((s) => s.name !== name), { ...view.filters, name }] });
    setSaveName("");
    setMenu(null);
    toast("ok", `Búsqueda «${name}» guardada.`);
  };

  const f = view.filters;
  const filtering = hasFilters(f);
  const toggleQuick = (q: (typeof QUICK)[number]["id"]) => setFilters({ quick: f.quick.includes(q) ? f.quick.filter((x) => x !== q) : [...f.quick, q] });
  const toggleTag = (t: string) => setFilters({ tags: f.tags.some((x) => norm(x) === norm(t)) ? f.tags.filter((x) => norm(x) !== norm(t)) : [...f.tags, t] });

  const viewProps: ViewProps = { items: [], selected, selecting: selected.size > 0, onSelect, onOpen: actions.open, actions, colors };
  const renderItems = (items: Contact[]) =>
    view.view === "table" ? (
      <TableView {...viewProps} items={items} columns={view.columns} sort={view.sort} onSort={(sort) => setView({ sort })} onSelectAll={selectAll} />
    ) : view.view === "directory" ? (
      <DirectoryView {...viewProps} items={items} />
    ) : (
      <CardsView {...viewProps} items={items} />
    );

  const detailContact = detail ? byId.get(detail) : undefined;
  // Presencia de Teams y fotos: de los que se ven (y los de marcación rápida).

  return (
    <>
    <div className="mx-auto grid max-w-(--page-max) grid-cols-12 gap-4 p-6">
      {/* De un vistazo: cuántos hay y qué falta. Cada cifra filtra. */}
      {live.length > 0 && (
        <div className="col-span-12 grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-6">
          <StatTile label="Contactos" value={stats.total} onClick={() => setView({ filters: EMPTY_FILTERS })} active={!filtering} />
          <StatTile label="Favoritos" value={stats.favorites} onClick={() => toggleQuick("favorites")} active={f.quick.includes("favorites")} />
          <StatTile label="Con extensión" value={stats.withExt} onClick={() => toggleQuick("withExt")} active={f.quick.includes("withExt")} />
          <StatTile label="Empresas" value={stats.companies} onClick={() => setView({ group: view.group === "company" ? "none" : "company" })} active={view.group === "company"} />
          <StatTile label="Usados este mes" value={stats.usedMonth} onClick={() => setView({ sort: "recent" })} active={view.sort === "recent"} />
          <StatTile label="Sin completar" value={stats.incomplete} onClick={() => toggleQuick("incomplete")} active={f.quick.includes("incomplete")} warn />
        </div>
      )}

      {/* Sin Card: su «contain: paint» recortaría los menús desplegables. */}
      <section className="col-span-12 rounded-xl border border-line bg-panel p-4">
        <header className="mb-3 flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-sm font-semibold text-ink">
            <UserRound size={14} className="text-neon" /> Contactos
          </h2>
          <div className="flex items-center gap-3 text-[11px]">
            <button onClick={importFile} className="flex items-center gap-1 text-mute hover:text-ink" title="CSV de Excel u Outlook, o vCard (.vcf)">
              <Upload size={11} /> Importar
            </button>
            <div className="relative" data-menu>
              <button onClick={() => setMenu(menu === "export" ? null : "export")} disabled={!live.length} className="flex items-center gap-1 text-mute hover:text-ink disabled:opacity-40">
                <Download size={11} /> Exportar <ChevronDown size={10} />
              </button>
              {menu === "export" && (
                <div className="absolute top-full right-0 z-20 mt-1 w-64 rounded-md border border-line-2 bg-panel py-1 text-xs shadow-xl">
                  {[
                    ["csv", "CSV (Excel)"],
                    ["vcf", "vCard (.vcf) para el móvil u Outlook"],
                  ].map(([k, label]) => (
                    <div key={k}>
                      <button onClick={() => exportAs(k as "csv" | "vcf", filtering ? filtered : live)} className="block w-full px-3 py-1.5 text-left text-dim hover:bg-panel-2 hover:text-ink">
                        {label} · {filtering ? `los ${filtered.length} filtrados` : `todos (${live.length})`}
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
            <button onClick={() => setTools("tags")} className="flex items-center gap-1 text-mute hover:text-ink">
              <Settings2 size={11} /> Gestionar
              {(dupCount > 0 || trashCount > 0) && <span className="rounded-full bg-warn/20 px-1.5 text-[10px] text-warn">{dupCount + trashCount}</span>}
            </button>
          </div>
        </header>
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative min-w-72 flex-1" data-menu>
            <Search size={14} className="absolute top-2.5 left-3 text-mute" />
            <input
              ref={search}
              value={f.query}
              onChange={(e) => setFilters({ query: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === "Escape") setFilters({ query: "" });
                if (e.key === "Enter" && filtered.length === 1) setDetail(filtered[0].id);
              }}
              placeholder="Buscar… (ext:21, empresa:x, #etiqueta, -excluir)   Atajo: /"
              className={`${inputClass} pr-9 pl-9`}
            />
            <button onClick={() => setMenu(menu === "help" ? null : "help")} className="absolute top-2 right-2.5 text-mute hover:text-ink" title="Cómo buscar">
              <HelpCircle size={15} />
            </button>
            {menu === "help" && (
              <div className="absolute top-full right-0 z-20 mt-1 w-80 rounded-md border border-line-2 bg-panel p-3 text-xs shadow-xl">
                <p className="mb-2 text-dim">Combina palabras y prefijos. Todo sin tildes ni mayúsculas.</p>
                {PREFIX_HELP.map(([ex, what]) => (
                  <button
                    key={ex}
                    onClick={() => {
                      setFilters({ query: `${f.query} ${ex}`.trim() });
                      setMenu(null);
                      search.current?.focus();
                    }}
                    className="flex w-full items-center justify-between rounded px-1.5 py-1 hover:bg-panel-2"
                  >
                    <code className="text-neon">{ex}</code> <span className="text-mute">{what}</span>
                  </button>
                ))}
                <p className="mt-2 text-mute">Con un solo resultado, Enter abre su ficha.</p>
              </div>
            )}
          </div>
          <Button onClick={() => setEditing({ ...EMPTY_CONTACT })}>
            <Plus size={14} /> Nuevo contacto
          </Button>
        </div>

        {/* Vista, agrupación y orden */}
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <span className="flex overflow-hidden rounded-md border border-line">
            {VIEWS.map((v) => (
              <button
                key={v.id}
                onClick={() => setView({ view: v.id })}
                className={`flex items-center gap-1 px-2.5 py-1 text-xs ${view.view === v.id ? "bg-neon/15 text-neon" : "text-mute hover:text-ink"}`}
              >
                <v.icon size={12} /> {v.label}
              </button>
            ))}
          </span>
          <label className="flex items-center gap-1.5 text-xs text-mute">
            Agrupar
            <select value={view.group} onChange={(e) => setView({ group: e.target.value as ContactsView["group"] })} className={selectClass}>
              {GROUPS.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.label}
                </option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-1.5 text-xs text-mute">
            Ordenar
            <select value={view.sort} onChange={(e) => setView({ sort: e.target.value as ContactsView["sort"] })} className={selectClass}>
              {SORTS.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label}
                </option>
              ))}
            </select>
          </label>
          {companies.length > 1 && (
            <label className="flex items-center gap-1.5 text-xs text-mute">
              Empresa
              <select value={f.company} onChange={(e) => setFilters({ company: e.target.value })} className={`${selectClass} max-w-44`}>
                <option value="">Todas</option>
                {companies.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
            </label>
          )}
          {view.view === "table" && (
            <div className="relative" data-menu>
              <button onClick={() => setMenu(menu === "columns" ? null : "columns")} className="flex h-8 items-center gap-1 rounded-md border border-line px-2 text-xs text-mute hover:text-ink">
                <Columns3 size={12} /> Columnas
              </button>
              {menu === "columns" && (
                <div className="absolute top-full left-0 z-20 mt-1 w-48 rounded-md border border-line-2 bg-panel p-2 shadow-xl">
                  {COLUMNS.map((c) => (
                    <label key={c.id} className="flex items-center gap-2 px-1 py-0.5 text-xs text-dim">
                      <input
                        type="checkbox"
                        checked={view.columns.includes(c.id)}
                        onChange={(e) => setView({ columns: e.target.checked ? COLUMNS.map((x) => x.id).filter((id) => id === c.id || view.columns.includes(id)) : view.columns.filter((x) => x !== c.id) })}
                        className="accent-[var(--color-neon)]"
                      />
                      {c.label}
                    </label>
                  ))}
                </div>
              )}
            </div>
          )}
          {view.group !== "none" && (
            <button onClick={() => setCollapsed(collapsed.size ? new Set() : new Set(groups.map((g) => g.key)))} className="text-xs text-mute hover:text-ink">
              {collapsed.size ? "Desplegar todo" : "Plegar todo"}
            </button>
          )}
        </div>

        {/* Filtros rápidos y etiquetas */}
        <div className="mt-3 flex flex-wrap items-center gap-1.5">
          {QUICK.map((q) => (
            <button
              key={q.id}
              onClick={() => toggleQuick(q.id)}
              title={q.hint}
              className={`rounded-full border px-2.5 py-0.5 text-xs transition-colors ${f.quick.includes(q.id) ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`}
            >
              {q.id === "favorites" && <Star size={10} className="mr-1 inline" />}
              {q.label}
            </button>
          ))}
        </div>
        {tagCounts.length > 0 && (
          <ChipRow storageKey="contacts" className="mt-2">
            <Tag size={12} className="text-mute" />
            {tagCounts.map(([t, n]) => (
              <TagChip key={t} name={t} count={n} color={colorOf(colors, t)} active={f.tags.some((x) => norm(x) === norm(t))} onClick={() => toggleTag(t)} />
            ))}
            {f.tags.length > 1 && (
              <span className="ml-1 flex overflow-hidden rounded-md border border-line text-[11px]">
                {(["all", "any"] as const).map((m) => (
                  <button key={m} onClick={() => setFilters({ tagMode: m })} className={`px-2 py-0.5 ${f.tagMode === m ? "bg-neon/15 text-neon" : "text-mute hover:text-ink"}`}>
                    {m === "all" ? "Todas" : "Alguna"}
                  </button>
                ))}
              </span>
            )}
          </ChipRow>
        )}

        {/* Búsquedas guardadas */}
        {(view.saved.length > 0 || filtering) && (
          <div className="mt-3 flex flex-wrap items-center gap-1.5 border-t border-line pt-3">
            {view.saved.map((s) => (
              <span key={s.name} className="inline-flex items-center overflow-hidden rounded-md border border-line text-xs">
                <button onClick={() => setView({ filters: { ...EMPTY_FILTERS, ...s } })} className="flex items-center gap-1 px-2 py-0.5 text-dim hover:bg-panel-2 hover:text-ink">
                  <BookUser size={11} /> {s.name}
                </button>
                <button onClick={() => setView({ saved: view.saved.filter((x) => x.name !== s.name) })} className="border-l border-line px-1 text-mute hover:text-bad" title="Borrar búsqueda">
                  <X size={10} />
                </button>
              </span>
            ))}
            {filtering && (
              <>
                <div className="relative" data-menu>
                  <button onClick={() => setMenu(menu === "save" ? null : "save")} className="flex items-center gap-1 rounded-md px-2 py-0.5 text-xs text-neon hover:bg-neon/10">
                    <Save size={11} /> Guardar esta búsqueda
                  </button>
                  {menu === "save" && (
                    <div className="absolute top-full left-0 z-20 mt-1 flex w-64 gap-1 rounded-md border border-line-2 bg-panel p-2 shadow-xl">
                      <input
                        autoFocus
                        value={saveName}
                        onChange={(e) => setSaveName(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && saveSearch()}
                        placeholder="Nombre: Proveedores urgentes"
                        className={`${inputClass} h-8 py-1 text-xs`}
                      />
                      <Button onClick={saveSearch}>OK</Button>
                    </div>
                  )}
                </div>
                <button onClick={() => setView({ filters: EMPTY_FILTERS })} className="ml-auto flex items-center gap-1 text-xs text-mute hover:text-ink">
                  <X size={11} /> Quitar filtros
                </button>
              </>
            )}
          </div>
        )}
      </section>

      {/* Acciones con la selección */}
      {selected.size > 0 && (
        <div className="sticky top-2 z-20 col-span-12 flex flex-wrap items-center gap-2 rounded-xl border border-neon/40 bg-panel px-4 py-2 shadow-xl">
          <span className="text-sm text-ink">{selected.size} seleccionado(s)</span>
          <button onClick={() => selectAll(true)} className="text-xs text-neon hover:underline">
            Todos los visibles ({flat.length})
          </button>
          <span className="mx-1 h-4 w-px bg-line" />
          <div className="relative" data-menu>
            <Button kind="ghost" onClick={() => setMenu(menu === "bulkTag" ? null : "bulkTag")}>
              <Tag size={13} /> Etiquetas
            </Button>
            {menu === "bulkTag" && (
              <div className="absolute top-full left-0 z-30 mt-1 w-80 rounded-md border border-line-2 bg-panel p-3 shadow-xl">
                <TagInput value={bulkTags} onChange={setBulkTags} suggestions={tagNames} colors={colors} placeholder="Etiquetas a poner o quitar…" autoFocus />
                <div className="mt-2 flex justify-end gap-2">
                  <Button
                    kind="ghost"
                    onClick={async () => {
                      for (const t of bulkTags) await bulk({ op: "removeTag", tag: t }, `«${t}» quitada`);
                      setBulkTags([]);
                      setMenu(null);
                    }}
                    disabled={!bulkTags.length}
                  >
                    Quitar
                  </Button>
                  <Button
                    onClick={async () => {
                      for (const t of bulkTags) await bulk({ op: "addTag", tag: t }, `«${t}» puesta`);
                      setBulkTags([]);
                      setMenu(null);
                    }}
                    disabled={!bulkTags.length}
                  >
                    Poner
                  </Button>
                </div>
              </div>
            )}
          </div>
          <Button kind="ghost" onClick={() => bulk({ op: "favorite", value: !selectedList.every((c) => c.favorite) }, "Favoritos actualizados")}>
            <Star size={13} /> {selectedList.every((c) => c.favorite) ? "Quitar favorito" : "Favorito"}
          </Button>
          <Button kind="ghost" onClick={() => exportAs("csv", selectedList)}>
            <Download size={13} /> CSV
          </Button>
          <Button kind="ghost" onClick={() => exportAs("vcf", selectedList)}>
            <Download size={13} /> vCard
          </Button>
          <Button kind="danger" onClick={bulkTrash}>
            <Trash2 size={13} /> Papelera
          </Button>
          <button onClick={() => setSelected(new Set())} className="ml-auto text-xs text-mute hover:text-ink">
            Deseleccionar
          </button>
        </div>
      )}

      {/* Marcación rápida */}
      {!filtering && frequent.length > 0 && view.view !== "table" && (
        <div className="col-span-12">
          <p className="mb-1.5 flex items-center gap-1.5 text-[11px] font-medium tracking-wide text-mute uppercase">
            <Star size={11} /> Marcación rápida
          </p>
          <QuickDial items={frequent} actions={actions} />
        </div>
      )}

      {/* Lista */}
      <div className="col-span-12 space-y-4">
        {all === null ? (
          <Loading />
        ) : live.length === 0 ? (
          <EmptyState
            icon={<UserRound size={22} />}
            title="Todavía no hay contactos"
            action={
              <div className="flex gap-2">
                <Button onClick={() => setEditing({ ...EMPTY_CONTACT })}>
                  <Plus size={14} /> Nuevo contacto
                </Button>
                <Button kind="ghost" onClick={importFile}>
                  <Upload size={14} /> Importar CSV o vCard
                </Button>
              </div>
            }
          >
            A quién llamar y para qué. Viaja contigo en todos los equipos y se encuentra desde Ctrl+K.
          </EmptyState>
        ) : filtered.length === 0 ? (
          <p className="py-10 text-center text-sm text-mute">Nadie coincide con la búsqueda.</p>
        ) : (
          groups.map((g) =>
            view.group === "none" ? (
              <div key={g.key}>{renderItems(g.items)}</div>
            ) : (
              <section key={g.key}>
                <button
                  onClick={() =>
                    setCollapsed((s) => {
                      const n = new Set(s);
                      if (n.has(g.key)) n.delete(g.key);
                      else n.add(g.key);
                      return n;
                    })
                  }
                  className="mb-2 flex items-center gap-1.5 text-xs font-medium tracking-wide text-dim uppercase hover:text-ink"
                >
                  {collapsed.has(g.key) ? <ChevronRight size={13} /> : <ChevronDown size={13} />}
                  {view.group === "tag" && colorOf(colors, g.label) ? <TagChip name={g.label} color={colorOf(colors, g.label)} /> : g.label}
                  <span className="text-mute">{g.items.length}</span>
                </button>
                {!collapsed.has(g.key) && renderItems(g.items)}
              </section>
            ),
          )
        )}
        {live.length > 0 && (
          <p className="text-center text-[11px] text-mute">
            {filtered.length} de {live.length} contacto(s)
            {view.group === "tag" && " · un contacto con varias etiquetas sale en cada grupo"}
          </p>
        )}
      </div>

      {detailContact && (
        <ContactDetail contact={detailContact} byId={byId} clients={clients} colors={colors} actions={actions} onClose={() => setDetail(null)} />
      )}
      {editing && (
        <ContactEditor initial={editing} contacts={live} clients={clients} tagSuggestions={tagNames} colors={colors} onSave={save} onClose={() => setEditing(null)} />
      )}
      {tools && <ContactTools tab={tools} contacts={all ?? []} colors={colors} onChanged={load} onClose={() => setTools(null)} />}
      {dialog}
    </div>
    </>
  );
}

function StatTile({ label, value, onClick, active, warn = false }: { label: string; value: number; onClick: () => void; active: boolean; warn?: boolean }) {
  return (
    <button
      onClick={onClick}
      className={`rounded-xl border px-3 py-2.5 text-left transition-colors ${active ? "border-neon/50 bg-neon/10" : "border-line bg-panel hover:border-line-2"}`}
    >
      <div className={`font-mono text-xl leading-none font-semibold ${warn && value > 0 ? "text-warn" : "text-ink"}`}>{value}</div>
      <div className="mt-1 truncate text-[11px] text-mute">{label}</div>
    </button>
  );
}
