import { Building2, Copy, Download, Mail, Pencil, Phone, Plus, Search, Smartphone, Star, Trash2, Upload, UserRound, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, inputClass } from "../components/ui";
import { contactsApi, officeApi, type Contact } from "../lib/api";

const EMPTY: Contact = { id: "", name: "", role: "", company: "", extension: "", phone: "", mobile: "", email: "", reason: "", tags: [], notes: "", favorite: false, updated: 0 };

const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
const haystack = (c: Contact) => norm([c.name, c.role, c.company, c.extension, c.phone, c.mobile, c.email, c.reason, c.notes, ...c.tags].join(" "));

const csvCell = (s: string) => (/[;"\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);

/** Agenda del técnico: a quién llamar y para qué. Es la misma en todos los equipos. */
export function Contacts({ focus }: { focus: string | null }) {
  const [list, setList] = useState<Contact[] | null>(null);
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  const [editing, setEditing] = useState<Contact | null>(null);
  const [tagsText, setTagsText] = useState("");
  const [saving, setSaving] = useState(false);
  const [flash, setFlash] = useState<string | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => contactsApi.list().then(setList).catch((e) => toast("error", String(e))), [toast]);

  useEffect(() => {
    load();
  }, [load]);

  // Desde la búsqueda global: resaltar el contacto elegido.
  useEffect(() => {
    if (!focus || !list) return;
    setQuery("");
    setTag(null);
    setFlash(focus);
    window.setTimeout(() => document.getElementById(`contact-${focus}`)?.scrollIntoView({ block: "center", behavior: "smooth" }), 50);
    const t = window.setTimeout(() => setFlash(null), 2500);
    return () => window.clearTimeout(t);
  }, [focus, list]);

  const tags = useMemo(() => {
    const m = new Map<string, number>();
    for (const c of list ?? []) for (const t of c.tags) m.set(t, (m.get(t) ?? 0) + 1);
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0], "es"));
  }, [list]);

  const shown = useMemo(() => {
    const words = norm(query.trim()).split(/\s+/).filter(Boolean);
    return (list ?? [])
      .filter((c) => (!tag || c.tags.includes(tag)) && words.every((w) => haystack(c).includes(w)))
      .sort((a, b) => Number(b.favorite) - Number(a.favorite) || a.name.localeCompare(b.name, "es"));
  }, [list, query, tag]);

  const edit = (c: Contact) => {
    setEditing({ ...c });
    setTagsText(c.tags.join(", "));
  };

  const save = async () => {
    if (!editing) return;
    setSaving(true);
    try {
      await contactsApi.save({ ...editing, tags: tagsText.split(",").map((t) => t.trim()).filter(Boolean) });
      toast("ok", editing.id ? "Contacto guardado." : "Contacto añadido.");
      setEditing(null);
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setSaving(false);
    }
  };

  const remove = async (c: Contact) => {
    if (!(await confirm({ title: "Borrar contacto", body: `Se borrará «${c.name}» de la agenda.`, confirmLabel: "Borrar", danger: true }))) return;
    await contactsApi.remove(c.id).catch((e) => toast("error", String(e)));
    load();
  };

  const star = async (c: Contact) => {
    await contactsApi.save({ ...c, favorite: !c.favorite }).catch((e) => toast("error", String(e)));
    load();
  };

  const copy = (text: string, what: string) => navigator.clipboard.writeText(text).then(() => toast("ok", `${what} copiado.`));

  const importCsv = async () => {
    try {
      const r = await contactsApi.importCsv();
      if (!r) return;
      toast("ok", `${r.added} contacto(s) añadidos${r.updated ? `, ${r.updated} ya existían y se completaron` : ""}.`);
      load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  const exportCsv = async () => {
    const head = ["Nombre", "Cargo", "Empresa", "Extensión", "Teléfono", "Móvil", "Correo", "Motivo", "Etiquetas", "Notas"];
    const rows = (list ?? []).map((c) => [c.name, c.role, c.company, c.extension, c.phone, c.mobile, c.email, c.reason, c.tags.join(", "), c.notes].map(csvCell).join(";"));
    try {
      const file = await officeApi.exportCsv("Contactos", [head.join(";"), ...rows].join("\r\n"));
      if (file) toast("ok", "Contactos exportados.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  const field = (key: keyof Contact, label: string, placeholder = "", wide = false) => (
    <label className={`block ${wide ? "col-span-2" : ""}`}>
      <span className="mb-1 block text-[11px] text-mute">{label}</span>
      <input
        value={editing?.[key] as string}
        onChange={(e) => setEditing((c) => (c ? { ...c, [key]: e.target.value } : c))}
        placeholder={placeholder}
        className={inputClass}
      />
    </label>
  );

  return (
    <div className="mx-auto grid max-w-5xl grid-cols-12 gap-4 p-6">
      <Card
        title="Contactos"
        icon={<UserRound size={14} />}
        className="col-span-12"
        right={
          <div className="flex gap-3 text-[11px]">
            <button onClick={importCsv} className="flex items-center gap-1 text-mute hover:text-ink" title="CSV de Excel u Outlook">
              <Upload size={11} /> Importar CSV
            </button>
            <button onClick={exportCsv} disabled={!list?.length} className="flex items-center gap-1 text-mute hover:text-ink disabled:opacity-40">
              <Download size={11} /> Exportar CSV
            </button>
          </div>
        }
      >
        <p className="mb-3 text-xs text-dim">
          A quién llamar o escribir y para qué. La agenda viaja con AdminOps (en el portable, dentro del USB): es la misma en todos los equipos. También se encuentra desde la
          búsqueda (Ctrl+K).
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative min-w-60 flex-1">
            <Search size={14} className="absolute top-2.5 left-3 text-mute" />
            <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Buscar por nombre, extensión, empresa, motivo…" className={`${inputClass} pl-9`} />
          </div>
          <Button onClick={() => edit(EMPTY)}>
            <Plus size={14} /> Nuevo contacto
          </Button>
        </div>
        {tags.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-1.5">
            {tags.map(([t, n]) => (
              <button
                key={t}
                onClick={() => setTag(tag === t ? null : t)}
                className={`rounded-full border px-2.5 py-0.5 text-xs transition-colors ${tag === t ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`}
              >
                {t} <span className="text-mute">{n}</span>
              </button>
            ))}
          </div>
        )}
      </Card>

      {editing && (
        <Card title={editing.id ? "Editar contacto" : "Nuevo contacto"} icon={<Pencil size={14} />} className="col-span-12">
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            {field("name", "Nombre", "Ana López")}
            {field("role", "Cargo o área", "Sistemas")}
            {field("company", "Empresa", "Proveedor, cliente…")}
            {field("extension", "Extensión", "2104")}
            {field("phone", "Teléfono")}
            {field("mobile", "Móvil")}
            {field("email", "Correo", "", true)}
            {field("reason", "Para qué llamarle", "Altas de usuarios, impresoras, la fibra…", true)}
            <label className="col-span-2 block">
              <span className="mb-1 block text-[11px] text-mute">Etiquetas (separadas por comas)</span>
              <input value={tagsText} onChange={(e) => setTagsText(e.target.value)} placeholder="TI, proveedores, urgencias" className={inputClass} />
            </label>
            <label className="col-span-2 block md:col-span-4">
              <span className="mb-1 block text-[11px] text-mute">Notas</span>
              <textarea
                value={editing.notes}
                onChange={(e) => setEditing((c) => (c ? { ...c, notes: e.target.value } : c))}
                rows={2}
                className={inputClass}
              />
            </label>
          </div>
          <div className="mt-4 flex justify-end gap-2">
            <Button kind="ghost" onClick={() => setEditing(null)}>
              Cancelar
            </Button>
            <Button onClick={save} disabled={saving}>
              Guardar
            </Button>
          </div>
        </Card>
      )}

      <div className="col-span-12 space-y-2">
        {list === null ? (
          <p className="text-sm text-mute">Cargando…</p>
        ) : list.length === 0 ? (
          <p className="py-8 text-center text-sm text-mute">Todavía no hay contactos. Añade el primero o importa un CSV de Excel u Outlook.</p>
        ) : shown.length === 0 ? (
          <p className="py-8 text-center text-sm text-mute">Nadie coincide con la búsqueda.</p>
        ) : (
          shown.map((c) => (
            <div
              key={c.id}
              id={`contact-${c.id}`}
              className={`group flex flex-wrap items-start gap-x-6 gap-y-2 rounded-xl border bg-panel px-4 py-3 transition-colors ${flash === c.id ? "border-neon" : "border-line"}`}
            >
              <button onClick={() => star(c)} className={`mt-0.5 ${c.favorite ? "text-warn" : "text-mute hover:text-warn"}`} title={c.favorite ? "Quitar de favoritos" : "Favorito (sale primero)"}>
                <Star size={15} fill={c.favorite ? "currentColor" : "none"} />
              </button>
              <div className="min-w-48 flex-1">
                <p className="text-sm font-medium text-ink">{c.name}</p>
                {(c.role || c.company) && (
                  <p className="flex items-center gap-1 text-xs text-dim">
                    {c.company && <Building2 size={11} />} {[c.role, c.company].filter(Boolean).join(" · ")}
                  </p>
                )}
                {c.reason && <p className="mt-1 text-xs text-neon">Para: {c.reason}</p>}
                {c.notes && <p className="mt-1 text-xs whitespace-pre-wrap text-mute select-text">{c.notes}</p>}
                {c.tags.length > 0 && (
                  <div className="mt-1.5 flex flex-wrap gap-1">
                    {c.tags.map((t) => (
                      <span key={t} className="rounded border border-line px-1.5 text-[11px] text-mute">
                        {t}
                      </span>
                    ))}
                  </div>
                )}
              </div>
              <div className="flex min-w-56 flex-col gap-1 text-sm">
                {c.extension && (
                  <button onClick={() => copy(c.extension, "Extensión")} className="flex items-center gap-2 text-left hover:text-neon" title="Copiar">
                    <Phone size={13} className="text-mute" /> <span className="text-xs text-mute">Ext.</span> <span className="font-mono text-base text-ink">{c.extension}</span>
                  </button>
                )}
                {c.phone && (
                  <button onClick={() => copy(c.phone, "Teléfono")} className="flex items-center gap-2 text-left font-mono text-xs text-dim hover:text-neon" title="Copiar">
                    <Phone size={13} className="text-mute" /> {c.phone}
                  </button>
                )}
                {c.mobile && (
                  <button onClick={() => copy(c.mobile, "Móvil")} className="flex items-center gap-2 text-left font-mono text-xs text-dim hover:text-neon" title="Copiar">
                    <Smartphone size={13} className="text-mute" /> {c.mobile}
                  </button>
                )}
                {c.email && (
                  <span className="flex items-center gap-2 text-xs">
                    <Mail size={13} className="text-mute" />
                    <button onClick={() => contactsApi.email(c.email).catch((e) => toast("error", String(e)))} className="truncate text-dim hover:text-neon" title="Escribir un correo">
                      {c.email}
                    </button>
                    <button onClick={() => copy(c.email, "Correo")} className="text-mute hover:text-ink" title="Copiar">
                      <Copy size={11} />
                    </button>
                  </span>
                )}
              </div>
              <div className="flex gap-1 opacity-60 group-hover:opacity-100">
                <button onClick={() => edit(c)} className="rounded p-1 text-mute hover:text-ink" title="Editar">
                  <Pencil size={13} />
                </button>
                <button onClick={() => remove(c)} className="rounded p-1 text-mute hover:text-bad" title="Borrar">
                  <Trash2 size={13} />
                </button>
              </div>
            </div>
          ))
        )}
        {list && list.length > 0 && (
          <p className="pt-1 text-center text-[11px] text-mute">
            {shown.length} de {list.length} contacto(s)
            {(query || tag) && (
              <button
                onClick={() => {
                  setQuery("");
                  setTag(null);
                }}
                className="ml-2 inline-flex items-center gap-0.5 text-neon hover:underline"
              >
                <X size={10} /> Quitar filtros
              </button>
            )}
          </p>
        )}
      </div>
      {dialog}
    </div>
  );
}
