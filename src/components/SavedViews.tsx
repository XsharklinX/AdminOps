// Vistas guardadas: una combinación de filtros con nombre («Equipos con
// Windows 10 sin TPM», «Cambios fallidos de esta semana») que queda como un chip
// arriba de la lista. Un clic y vuelve a estar. Se guardan por pantalla.
import { BookmarkPlus, Pin, X } from "lucide-react";
import { useState } from "react";

export interface SavedView<T> {
  name: string;
  value: T;
  pinned: boolean;
}

const key = (scope: string) => `adminops.views.${scope}`;

export function loadViews<T>(scope: string): SavedView<T>[] {
  try {
    const raw = JSON.parse(localStorage.getItem(key(scope)) ?? "[]");
    return Array.isArray(raw) ? raw.filter((v) => v && typeof v.name === "string") : [];
  } catch {
    return [];
  }
}

function storeViews<T>(scope: string, views: SavedView<T>[]) {
  try {
    localStorage.setItem(key(scope), JSON.stringify(views));
  } catch {
    /* sin almacenamiento: duran esta sesión */
  }
}

/** Añade o sustituye una vista por su nombre; las fijadas van primero. */
export function upsertView<T>(views: SavedView<T>[], name: string, value: T): SavedView<T>[] {
  const old = views.find((v) => v.name === name);
  const next = [...views.filter((v) => v.name !== name), { name, value, pinned: old?.pinned ?? false }];
  return next.sort((a, b) => Number(b.pinned) - Number(a.pinned));
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

export function SavedViews<T>({ scope, current, onApply, isDefault }: { scope: string; current: T; onApply: (value: T) => void; /** Sin filtros: no tiene sentido guardarla. */ isDefault: boolean }) {
  const [views, setViews] = useState(() => loadViews<T>(scope));
  const [naming, setNaming] = useState<string | null>(null);
  const save = (list: SavedView<T>[]) => {
    setViews(list);
    storeViews(scope, list);
  };
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {views.map((v) => {
        const on = same(v.value, current);
        return (
          <span key={v.name} className={`group/view flex items-center rounded-full border text-xs ${on ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-dim"}`}>
            <button onClick={() => onApply(v.value)} className="py-0.5 pr-1 pl-2.5 hover:text-ink" title="Aplicar esta vista">
              {v.pinned && <Pin size={10} className="mr-1 inline text-neon" fill="currentColor" />}
              {v.name}
            </button>
            <button
              onClick={() => save(views.map((x) => (x.name === v.name ? { ...x, pinned: !x.pinned } : x)).sort((a, b) => Number(b.pinned) - Number(a.pinned)))}
              className="hidden p-0.5 text-mute group-hover/view:block hover:text-neon"
              title={v.pinned ? "Desfijar" : "Fijar delante"}
              aria-label={v.pinned ? `Desfijar ${v.name}` : `Fijar ${v.name}`}
            >
              <Pin size={10} />
            </button>
            <button onClick={() => save(views.filter((x) => x.name !== v.name))} className="p-0.5 pr-1.5 text-mute hover:text-bad" title="Borrar esta vista" aria-label={`Borrar la vista ${v.name}`}>
              <X size={11} />
            </button>
          </span>
        );
      })}
      {naming !== null ? (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            const n = naming.trim();
            if (n) save(upsertView(views, n, current));
            setNaming(null);
          }}
          className="flex items-center gap-1"
        >
          <input
            autoFocus
            value={naming}
            onChange={(e) => setNaming(e.target.value)}
            onBlur={() => setNaming(null)}
            onKeyDown={(e) => e.key === "Escape" && setNaming(null)}
            placeholder="Nombre de la vista"
            className="h-6 w-44 rounded-full border border-neon/50 bg-void/60 px-2.5 text-xs text-ink outline-none"
            aria-label="Nombre de la vista"
          />
        </form>
      ) : (
        !isDefault &&
        !views.some((v) => same(v.value, current)) && (
          <button onClick={() => setNaming("")} className="flex items-center gap-1 rounded-full border border-dashed border-line-2 px-2.5 py-0.5 text-xs text-mute hover:border-neon/50 hover:text-ink" title="Guardar estos filtros con un nombre">
            <BookmarkPlus size={11} /> Guardar vista
          </button>
        )
      )}
    </div>
  );
}
