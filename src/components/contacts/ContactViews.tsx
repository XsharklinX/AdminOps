import { ArrowDown, Copy, Mail, Phone, Smartphone, Star } from "lucide-react";
import type { MouseEvent, ReactNode } from "react";
import type { Contact } from "../../lib/api";
import { COLUMNS, type Column, type SortBy } from "../../lib/contacts";
import type { ContactActions } from "./ContactDetail";
import { colorOf, TagChip, type TagColors } from "./Tags";

export interface ViewProps {
  items: Contact[];
  selected: Set<string>;
  selecting: boolean;
  onSelect: (c: Contact, e: MouseEvent) => void;
  onOpen: (c: Contact) => void;
  actions: ContactActions;
  colors: TagColors;
}

const stop = (fn: () => void) => (e: MouseEvent) => {
  e.stopPropagation();
  fn();
};

function Check({ c, selected, selecting, onSelect }: { c: Contact } & Pick<ViewProps, "selected" | "selecting" | "onSelect">) {
  return (
    <input
      type="checkbox"
      checked={selected.has(c.id)}
      onClick={(e) => {
        e.stopPropagation();
        onSelect(c, e);
      }}
      onChange={() => {}}
      className={`size-3.5 shrink-0 accent-[var(--color-neon)] ${selecting || selected.has(c.id) ? "" : "opacity-0 group-hover:opacity-100"}`}
      title="Seleccionar (Mayús para un rango)"
    />
  );
}

function StarBtn({ c, actions }: { c: Contact; actions: ContactActions }) {
  return (
    <button onClick={stop(() => actions.star(c))} className={`shrink-0 ${c.favorite ? "text-warn" : "text-mute opacity-40 hover:text-warn hover:opacity-100"}`} title="Favorito">
      <Star size={13} fill={c.favorite ? "currentColor" : "none"} />
    </button>
  );
}

function CopyLine({ icon, value, onCopy, mono = true }: { icon: ReactNode; value: string; onCopy: () => void; mono?: boolean }) {
  return (
    <button onClick={stop(onCopy)} className={`flex max-w-full items-center gap-1.5 text-left text-xs text-dim hover:text-neon ${mono ? "font-mono" : ""}`} title="Copiar">
      <span className="shrink-0 text-mute">{icon}</span>
      <span className="truncate">{value}</span>
    </button>
  );
}

// ---------- Tarjetas ----------

export function CardsView({ items, selected, selecting, onSelect, onOpen, actions: a, colors }: ViewProps) {
  return (
    <div className="grid grid-cols-1 gap-2 md:grid-cols-2 xl:grid-cols-3">
      {items.map((c) => (
        <div
          key={c.id}
          id={`contact-${c.id}`}
          onClick={() => onOpen(c)}
          className={`group flex cursor-pointer flex-col gap-1.5 rounded-xl border bg-panel px-3.5 py-3 transition-colors hover:border-line-2 ${
            selected.has(c.id) ? "border-neon/60 bg-neon/5" : "border-line"
          }`}
        >
          <div className="flex items-start gap-2">
            <Check c={c} selected={selected} selecting={selecting} onSelect={onSelect} />
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-ink">{c.name}</p>
              {(c.role || c.company) && <p className="truncate text-[11px] text-dim">{[c.role, c.company].filter(Boolean).join(" · ")}</p>}
            </div>
            {c.extension && (
              <button onClick={stop(() => a.copy(c, c.extension, "Extensión"))} className="shrink-0 rounded-md border border-line px-1.5 font-mono text-sm text-ink hover:border-neon/50 hover:text-neon" title="Copiar extensión">
                {c.extension}
              </button>
            )}
            <StarBtn c={c} actions={a} />
          </div>
          {c.reason && <p className="line-clamp-2 text-xs text-neon">Para: {c.reason}</p>}
          <div className="flex flex-col gap-0.5">
            {c.phone && <CopyLine icon={<Phone size={11} />} value={c.phone} onCopy={() => a.copy(c, c.phone, "Teléfono")} />}
            {c.mobile && <CopyLine icon={<Smartphone size={11} />} value={c.mobile} onCopy={() => a.copy(c, c.mobile, "Móvil")} />}
            {c.email && <CopyLine icon={<Mail size={11} />} value={c.email} onCopy={() => a.copy(c, c.email, "Correo")} mono={false} />}
          </div>
          {c.tags.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {c.tags.map((t) => (
                <TagChip key={t} name={t} color={colorOf(colors, t)} />
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

// ---------- Tabla ----------

const SORTABLE: Partial<Record<Column | "name", SortBy>> = { name: "name", company: "company", extension: "extension" };

export function TableView({
  items,
  selected,
  selecting,
  onSelect,
  onOpen,
  actions: a,
  colors,
  columns,
  sort,
  onSort,
  onSelectAll,
}: ViewProps & { columns: Column[]; sort: SortBy; onSort: (s: SortBy) => void; onSelectAll: (on: boolean) => void }) {
  const cols = COLUMNS.filter((c) => columns.includes(c.id));
  const all = items.length > 0 && items.every((c) => selected.has(c.id));
  const head = (id: Column | "name", label: string) => {
    const s = SORTABLE[id];
    return (
      <th key={id} className="px-2 pb-2 font-medium whitespace-nowrap">
        {s ? (
          <button onClick={() => onSort(s)} className={`flex items-center gap-0.5 hover:text-ink ${sort === s ? "text-neon" : ""}`}>
            {label} {sort === s && <ArrowDown size={10} />}
          </button>
        ) : (
          label
        )}
      </th>
    );
  };
  const cell = (c: Contact, id: Column) => {
    switch (id) {
      case "extension":
        return c.extension && (
          <button onClick={stop(() => a.copy(c, c.extension, "Extensión"))} className="font-mono text-ink hover:text-neon" title="Copiar">
            {c.extension}
          </button>
        );
      case "phone":
      case "mobile":
        return c[id] && (
          <button onClick={stop(() => a.copy(c, c[id], id === "phone" ? "Teléfono" : "Móvil"))} className="font-mono whitespace-nowrap hover:text-neon" title="Copiar">
            {c[id]}
          </button>
        );
      case "email":
        return c.email && (
          <button onClick={stop(() => a.copy(c, c.email, "Correo"))} className="hover:text-neon" title="Copiar">
            {c.email}
          </button>
        );
      case "tags":
        return (
          <span className="flex flex-wrap gap-1">
            {c.tags.map((t) => (
              <TagChip key={t} name={t} color={colorOf(colors, t)} />
            ))}
          </span>
        );
      default:
        return c[id];
    }
  };
  return (
    <div className="overflow-x-auto rounded-xl border border-line bg-panel p-3">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-left text-[11px] text-mute">
            <th className="w-6 pb-2">
              <input type="checkbox" checked={all} onChange={(e) => onSelectAll(e.target.checked)} className="size-3.5 accent-[var(--color-neon)]" title="Seleccionar todos" />
            </th>
            <th className="w-5 pb-2" />
            {head("name", "Nombre")}
            {cols.map((c) => head(c.id, c.label))}
          </tr>
        </thead>
        <tbody>
          {items.map((c) => (
            <tr
              key={c.id}
              id={`contact-${c.id}`}
              onClick={() => onOpen(c)}
              className={`group cursor-pointer border-t border-line/60 text-xs text-dim hover:bg-panel-2 ${selected.has(c.id) ? "bg-neon/5" : ""}`}
            >
              <td className="py-1.5">
                <Check c={c} selected={selected} selecting={selecting} onSelect={onSelect} />
              </td>
              <td>
                <StarBtn c={c} actions={a} />
              </td>
              <td className="px-2 py-1.5 text-sm text-ink">{c.name}</td>
              {cols.map((col) => (
                <td key={col.id} className="max-w-64 px-2 py-1.5">
                  {cell(c, col.id)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ---------- Directorio de extensiones ----------

export function DirectoryView({ items, selected, selecting, onSelect, onOpen, actions: a }: ViewProps) {
  return (
    <div className="grid grid-cols-1 gap-x-6 rounded-xl border border-line bg-panel px-3 py-2 lg:grid-cols-2">
      {items.map((c) => (
        <div key={c.id} id={`contact-${c.id}`} onClick={() => onOpen(c)} className="group flex cursor-pointer items-center gap-2 border-b border-line/40 py-1.5 hover:bg-panel-2">
          <Check c={c} selected={selected} selecting={selecting} onSelect={onSelect} />
          <span className="min-w-0 flex-1 truncate text-sm text-ink">
            {c.favorite && <Star size={10} className="mr-1 inline text-warn" fill="currentColor" />}
            {c.name}
            {c.role && <span className="ml-2 text-[11px] text-mute">{c.role}</span>}
          </span>
          {c.extension ? (
            <button onClick={stop(() => a.copy(c, c.extension, "Extensión"))} className="w-16 shrink-0 text-right font-mono text-base font-semibold text-ink hover:text-neon" title="Copiar extensión">
              {c.extension}
            </button>
          ) : c.phone || c.mobile ? (
            <button onClick={stop(() => a.copy(c, c.phone || c.mobile, "Teléfono"))} className="shrink-0 font-mono text-xs text-dim hover:text-neon" title="Copiar">
              {c.phone || c.mobile}
            </button>
          ) : (
            <span className="w-16 shrink-0 text-right text-xs text-mute">—</span>
          )}
          <button onClick={stop(() => a.copyCard(c))} className="shrink-0 p-1 text-mute opacity-0 group-hover:opacity-100 hover:text-neon" title="Copiar tarjeta">
            <Copy size={12} />
          </button>
        </div>
      ))}
    </div>
  );
}
