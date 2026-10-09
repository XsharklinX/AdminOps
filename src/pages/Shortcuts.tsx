import { usePageActive } from "../lib/pageActive";
import { Keyboard, Play, Radar, Search, Star } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useToast } from "../components/feedback";
import { invoke } from "@tauri-apps/api/core";
import { SHORTCUT_GROUPS, type Shortcut } from "../lib/shortcuts";
import { EmptyLine } from "../components/ui";

const FAVS = "adminops.shortcutFavorites";
const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
const idOf = (s: Shortcut) => s.keys.join("+");

function readFavs(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(FAVS) ?? "[]");
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}

/** Nombre de tecla del catálogo a partir de un evento de teclado. */
function keyName(e: KeyboardEvent): string | null {
  const map: Record<string, string> = {
    ArrowLeft: "Left",
    ArrowRight: "Right",
    ArrowUp: "Up",
    ArrowDown: "Down",
    " ": "Space",
    Delete: "Supr",
    Backspace: "Retroceso",
    Escape: "Esc",
    Enter: "Enter",
    Tab: "Tab",
    Home: "Home",
    End: "End",
  };
  if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return null;
  if (map[e.key]) return map[e.key];
  if (/^F\d{1,2}$/.test(e.key)) return e.key;
  return e.key.length === 1 ? e.key.toUpperCase() : e.key;
}

/** Teclas "reales" de un atajo del catálogo (sin las variantes "Left / Right"). */
function matches(s: Shortcut, pressed: string[]): boolean {
  const expand = s.keys.flatMap((k) => k.split(" / ")[0]).map((k) => k.toUpperCase());
  const p = pressed.map((k) => k.toUpperCase());
  return expand.length === p.length && expand.every((k) => p.includes(k));
}

function Keys({ keys }: { keys: string[] }) {
  return (
    <span className="flex shrink-0 flex-wrap items-center gap-1">
      {keys.map((k, i) => (
        <span key={i} className="flex items-center gap-1">
          {i > 0 && <span className="text-[11px] text-mute">+</span>}
          <kbd className="min-w-7 rounded-md border border-line-2 border-b-2 bg-panel-2 px-1.5 py-0.5 text-center font-mono text-[11px] text-ink">{k}</kbd>
        </span>
      ))}
    </span>
  );
}

export function Shortcuts() {
  const active = usePageActive();
  const [query, setQuery] = useState("");
  const [group, setGroup] = useState<string | null>(null);
  const [favorites, setFavorites] = useState<string[]>(readFavs);
  const [discover, setDiscover] = useState(false);
  const [pressed, setPressed] = useState<string[] | null>(null);
  const toast = useToast();

  // Modo descubrir: captura la combinación pulsada y busca qué hace.
  useEffect(() => {
    if (!discover || !active) return;
    const onKey = (e: KeyboardEvent) => {
      const k = keyName(e);
      if (!k) return;
      e.preventDefault();
      e.stopPropagation();
      if (k === "Esc" && !e.ctrlKey && !e.altKey && !e.shiftKey) {
        setDiscover(false);
        return;
      }
      setPressed([...(e.ctrlKey ? ["Ctrl"] : []), ...(e.altKey ? ["Alt"] : []), ...(e.shiftKey ? ["Shift"] : []), k]);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [discover, active]);

  const toggleFav = (s: Shortcut) => {
    const id = idOf(s);
    const next = favorites.includes(id) ? favorites.filter((f) => f !== id) : [...favorites, id];
    setFavorites(next);
    try {
      localStorage.setItem(FAVS, JSON.stringify(next));
    } catch {
      /* sin almacenamiento */
    }
  };

  const tryIt = (s: Shortcut) =>
    invoke("try_shortcut", { keys: s.keys })
      .then(() => toast("info", `Enviado ${s.keys.join(" + ")}`))
      .catch((e) => toast("error", String(e)));

  const all = useMemo(() => SHORTCUT_GROUPS.flatMap((g) => g.items.map((s) => ({ s, group: g }))), []);

  const results = useMemo(() => {
    const q = norm(query.trim());
    if (!q) return null;
    return all.filter(({ s }) => norm(`${s.keys.join(" ")} ${s.what} ${s.tip ?? ""}`).includes(q));
  }, [all, query]);

  const found = pressed ? all.filter(({ s }) => matches(s, pressed)) : [];
  const favItems = all.filter(({ s }) => favorites.includes(idOf(s)));
  const groups = group ? SHORTCUT_GROUPS.filter((g) => g.id === group) : SHORTCUT_GROUPS;

  const row = (s: Shortcut, key: string, context?: string) => (
    <div key={key} className="group flex items-start gap-4 px-4 py-2.5">
      <div className="w-40 shrink-0 pt-0.5 lg:w-64">
        <Keys keys={s.keys} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-sm text-ink">
          {s.what}
          {s.win11 && <span className="ml-2 rounded border border-line px-1 text-[10px] text-mute">Windows 11</span>}
          {context && <span className="ml-2 text-[11px] text-mute">· {context}</span>}
        </div>
        {s.tip && <div className="mt-0.5 text-xs text-dim">{s.tip}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-1">
        {s.try && (
          <button onClick={() => tryIt(s)} className="flex items-center gap-1 rounded-md border border-neon/40 px-2 py-0.5 text-[11px] text-neon hover:bg-neon/10" title="Simular este atajo">
            <Play size={10} /> Probar
          </button>
        )}
        <button
          onClick={() => toggleFav(s)}
          className={`rounded p-1 ${favorites.includes(idOf(s)) ? "text-warn" : "text-mute opacity-0 group-hover:opacity-100 hover:text-ink"}`}
          title={favorites.includes(idOf(s)) ? "Quitar de mis atajos" : "Guardar en mis atajos"}
        >
          <Star size={13} fill={favorites.includes(idOf(s)) ? "currentColor" : "none"} />
        </button>
      </div>
    </div>
  );

  return (
    <div className="mx-auto max-w-(--page-max) p-6">
      <div className="mb-4 flex items-center gap-3">
        <div className="relative flex-1">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="¿Qué quieres hacer? Captura, pestaña, portapapeles, ventana, emoji…"
            className="w-full rounded-md border border-line bg-panel py-2 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <button
          onClick={() => {
            setDiscover(!discover);
            setPressed(null);
          }}
          className={`flex items-center gap-1.5 rounded-md border px-3 py-2 text-sm transition-colors ${
            discover ? "border-neon/60 bg-neon/10 text-neon" : "border-line text-dim hover:text-ink"
          }`}
          title="Pulsa una combinación y te digo qué hace"
        >
          <Radar size={14} className={discover ? "animate-pulse" : ""} /> {discover ? "Escuchando… (Esc para salir)" : "Descubrir"}
        </button>
      </div>

      {discover && (
        <div className="mb-4 rounded-xl border border-neon/40 bg-neon/5 p-4">
          {!pressed ? (
            <p className="flex items-center gap-2 text-sm text-dim">
              <Keyboard size={15} className="text-neon" /> Pulsa cualquier combinación (por ejemplo Ctrl + Shift + T) y te diré para qué sirve.
            </p>
          ) : (
            <>
              <div className="mb-2 flex items-center gap-3">
                <span className="text-xs text-mute">Has pulsado</span>
                <Keys keys={pressed} />
              </div>
              {found.length ? (
                <div className="divide-y divide-line/60 rounded-lg border border-line bg-panel">{found.map(({ s, group: g }, i) => row(s, `f${i}`, g.title))}</div>
              ) : (
                <p className="text-sm text-mute">No tengo esa combinación en la lista (puede que el programa abierto le dé otro uso).</p>
              )}
            </>
          )}
          <p className="mt-2 text-[11px] text-mute">Las combinaciones con la tecla Windows, Alt+Tab y Ctrl+Alt+Supr las atrapa Windows antes de llegar aquí: búscalas en la lista.</p>
        </div>
      )}

      {results ? (
        <div className="overflow-hidden rounded-xl border border-line bg-panel">
          <div className="divide-y divide-line/60">{results.map(({ s, group: g }, i) => row(s, `r${i}`, g.title))}</div>
          {results.length === 0 && <EmptyLine>Nada coincide con «{query}».</EmptyLine>}
        </div>
      ) : (
        <>
          <div className="mb-4 flex flex-wrap gap-1.5">
            <button onClick={() => setGroup(null)} className={`rounded-full border px-3 py-1 text-xs ${!group ? "border-neon/60 bg-neon/10 text-neon" : "border-line text-dim hover:text-ink"}`}>
              Todos
            </button>
            {SHORTCUT_GROUPS.map((g) => (
              <button
                key={g.id}
                onClick={() => setGroup(g.id)}
                className={`rounded-full border px-3 py-1 text-xs ${group === g.id ? "border-neon/60 bg-neon/10 text-neon" : "border-line text-dim hover:text-ink"}`}
              >
                {g.title}
              </button>
            ))}
          </div>

          {!group && favItems.length > 0 && (
            <section className="mb-5">
              <h2 className="mb-1 flex items-center gap-1.5 text-[11px] font-semibold text-warn/80">
                <Star size={10} fill="currentColor" /> Mis atajos
              </h2>
              <div className="divide-y divide-line/60 overflow-hidden rounded-xl border border-line bg-panel">{favItems.map(({ s, group: g }, i) => row(s, `fav${i}`, g.title))}</div>
            </section>
          )}

          {groups.map((g) => (
            <section key={g.id} className="mb-5">
              <h2 className="text-[11px] font-semibold text-dim">{g.title}</h2>
              <p className="mb-2 text-xs text-mute">{g.intro}</p>
              <div className="divide-y divide-line/60 overflow-hidden rounded-xl border border-line bg-panel">{g.items.map((s, i) => row(s, `${g.id}${i}`))}</div>
            </section>
          ))}
        </>
      )}
    </div>
  );
}
