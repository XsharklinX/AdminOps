import { Eye, EyeOff, GripVertical, Plus, RotateCcw, Star, Trash2 } from "lucide-react";
import { useState } from "react";
import { useConfirm } from "../../components/feedback";
import { AREA_ICONS, effectiveAreas, NAV, navLabel, type PageId } from "../../components/Sidebar";
import { Button, Card } from "../../components/ui";
import { DEFAULT_SIDEBAR, setPrefs, setSidebar, usePrefs, type NavArea, type SidebarPrefs } from "../../lib/prefs";

const defaultLabel = (p: PageId) => NAV.find((n) => n.id === p)?.label ?? p;

type Drag = { type: "area"; index: number } | { type: "page"; page: PageId; from: number };

function Segmented<T extends string>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className="inline-flex flex-wrap rounded-lg border border-line bg-void p-0.5">
      {options.map(([v, label]) => (
        <button
          key={v}
          onClick={() => onChange(v)}
          aria-pressed={value === v}
          className={`rounded-md px-2.5 py-1 text-xs ${value === v ? "bg-panel-2 font-medium text-ink" : "text-dim hover:text-ink"}`}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

function Opt({ title, sub, children }: { title: string; sub?: string; children: React.ReactNode }) {
  return (
    <div className="border-t border-line/60 py-2.5 first:border-t-0 first:pt-0">
      <div className="mb-1.5 text-[13px] text-ink">{title}</div>
      {sub && <div className="mb-1.5 text-[11px] text-mute">{sub}</div>}
      {children}
    </div>
  );
}

function Check({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <label className="flex items-center gap-2 py-0.5 text-[13px] text-dim">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="size-4 accent-[var(--color-neon)]" />
      {label}
    </label>
  );
}

/** Personalización completa de la navegación: estructura (arrastrar y soltar), nombres, iconos y comportamiento. */
export function NavEditor() {
  const prefs = usePrefs();
  const sb = prefs.sidebar;
  const { confirm, dialog } = useConfirm();
  const areas: NavArea[] = effectiveAreas(prefs.layout).map((a) => ({ id: a.id, label: a.label, icon: a.iconName, pages: [...a.pages] }));
  const hidden = new Set<PageId>(prefs.layout?.hidden ?? []);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const [iconFor, setIconFor] = useState<number | null>(null);

  const save = (next: NavArea[], nextHidden = hidden) => setPrefs({ layout: { areas: next, hidden: [...nextHidden] } });
  const setArea = (i: number, patch: Partial<NavArea>) => save(areas.map((a, k) => (k === i ? { ...a, ...patch } : a)));
  const toggleHidden = (p: PageId) => {
    const next = new Set(hidden);
    if (next.has(p)) next.delete(p);
    else next.add(p);
    save(areas, next);
  };
  const toggleFav = (p: PageId) => setSidebar({ favorites: sb.favorites.includes(p) ? sb.favorites.filter((x) => x !== p) : [...sb.favorites, p] });
  const rename = (p: PageId, label: string) => {
    const labels = { ...prefs.pageLabels };
    if (!label.trim() || label.trim() === defaultLabel(p)) delete labels[p];
    else labels[p] = label.slice(0, 40);
    setPrefs({ pageLabels: labels });
  };

  /** Suelta lo arrastrado sobre la sección `ai` (antes de la página `beforePage`, o al final). */
  const drop = (ai: number, beforePage?: PageId) => {
    setOver(null);
    if (!drag) return;
    if (drag.type === "area") {
      if (drag.index === ai) return;
      const next = [...areas];
      const [moved] = next.splice(drag.index, 1);
      next.splice(ai, 0, moved);
      save(next);
    } else {
      if (drag.page === beforePage) return;
      const next = areas.map((a) => ({ ...a, pages: a.pages.filter((p) => p !== drag.page) }));
      const target = next[ai].pages;
      const at = beforePage ? target.indexOf(beforePage) : -1;
      target.splice(at < 0 ? target.length : at, 0, drag.page);
      save(next);
    }
    setDrag(null);
  };

  const reset = async () => {
    const ok = await confirm({
      title: "¿Volver a la navegación de fábrica?",
      confirmLabel: "Restablecer todo",
      body: <p>Se pierden el orden, los nombres, las áreas propias, las pantallas ocultas, los fijados y las opciones de la barra lateral.</p>,
    });
    if (ok) setPrefs({ layout: null, pageLabels: {}, sidebar: DEFAULT_SIDEBAR });
  };

  const s = <K extends keyof SidebarPrefs>(k: K) => (v: SidebarPrefs[K]) => setSidebar({ [k]: v } as Partial<SidebarPrefs>);

  return (
    <div className="grid grid-cols-12 gap-4">
      <div className="col-span-12 space-y-3 lg:col-span-7">
        <Card title="Estructura">
          <div className="flex flex-wrap items-center gap-2">
            <p className="min-w-0 flex-1 text-xs text-dim">
              Arrastra las secciones y las páginas (por el asa ⋮⋮) para ordenarlas o moverlas de sección. Pulsa el icono de una sección para cambiarlo y escribe
              encima de cualquier nombre para renombrarlo. ★ fija la pantalla arriba de la barra (las secciones se fijan con la chincheta, en la propia barra); el ojo
              la oculta de la barra (sigue en «Todo» y en Ctrl+K).
            </p>
            <Button kind="ghost" onClick={() => save([...areas, { id: `custom-${Date.now().toString(36)}`, label: "Mi sección", icon: "Star", pages: [] }])}>
              <Plus size={14} /> Nueva sección
            </Button>
          </div>
        </Card>

        {areas.map((a, ai) => {
          const Icon = AREA_ICONS[a.icon] ?? AREA_ICONS.Folder;
          return (
            <div
              key={a.id}
              onDragOver={(e) => {
                e.preventDefault();
                setOver(`area-${ai}`);
              }}
              onDragLeave={() => setOver((o) => (o === `area-${ai}` ? null : o))}
              onDrop={(e) => {
                e.preventDefault();
                drop(ai);
              }}
              className={`rounded-xl border bg-panel transition-colors ${over === `area-${ai}` ? "border-neon" : "border-line"}`}
            >
              <div className="flex items-center gap-2 border-b border-line/60 px-3 py-2">
                <span
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.effectAllowed = "move";
                    e.dataTransfer.setData("text/plain", a.id);
                    setDrag({ type: "area", index: ai });
                  }}
                  onDragEnd={() => setDrag(null)}
                  className="cursor-grab text-mute hover:text-ink"
                  title="Arrastrar la sección"
                >
                  <GripVertical size={16} />
                </span>
                <div className="relative">
                  <button onClick={() => setIconFor(iconFor === ai ? null : ai)} className="grid size-8 place-items-center rounded-md border border-line text-dim hover:border-line-2 hover:text-ink" title="Cambiar icono">
                    <Icon size={16} />
                  </button>
                  {iconFor === ai && (
                    <div className="absolute top-9 left-0 z-20 grid w-64 grid-cols-8 gap-1 rounded-lg border border-line-2 bg-panel p-2 shadow-xl">
                      {Object.entries(AREA_ICONS).map(([name, I]) => (
                        <button
                          key={name}
                          onClick={() => {
                            setArea(ai, { icon: name });
                            setIconFor(null);
                          }}
                          className={`grid size-7 place-items-center rounded ${a.icon === name ? "bg-neon/15 text-neon" : "text-dim hover:bg-panel-2 hover:text-ink"}`}
                          title={name}
                        >
                          <I size={15} />
                        </button>
                      ))}
                    </div>
                  )}
                </div>
                <input
                  value={a.label}
                  onChange={(e) => setArea(ai, { label: e.target.value.slice(0, 30) })}
                  className="min-w-0 flex-1 rounded-md border border-transparent bg-transparent px-2 py-1 text-sm font-medium text-ink outline-none hover:border-line focus:border-neon/50"
                  aria-label="Nombre de la sección"
                />
                <span className="text-xs text-mute">{a.pages.length}</span>
                <button
                  onClick={() => save(areas.filter((_, k) => k !== ai))}
                  disabled={a.pages.length > 0}
                  className="rounded p-1 text-mute hover:bg-panel-2 hover:text-bad disabled:opacity-25"
                  title={a.pages.length ? "Mueve antes sus páginas a otra sección" : "Quitar sección"}
                >
                  <Trash2 size={14} />
                </button>
              </div>
              {a.pages.length === 0 ? (
                <p className="px-4 py-3 text-xs text-mute">Suelta aquí páginas. Las secciones vacías no aparecen en la barra lateral.</p>
              ) : (
                <ul className="px-2 py-1">
                  {a.pages.map((p) => {
                    const key = `page-${p}`;
                    return (
                      <li
                        key={p}
                        onDragOver={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          setOver(key);
                        }}
                        onDrop={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          drop(ai, p);
                        }}
                        className={`flex items-center gap-2 rounded-md px-1.5 py-0.5 ${over === key && drag?.type === "page" ? "border-t-2 border-neon" : ""} ${hidden.has(p) ? "opacity-50" : ""}`}
                      >
                        <span
                          draggable
                          onDragStart={(e) => {
                            e.dataTransfer.effectAllowed = "move";
                            e.dataTransfer.setData("text/plain", p);
                            setDrag({ type: "page", page: p, from: ai });
                          }}
                          onDragEnd={() => {
                            setDrag(null);
                            setOver(null);
                          }}
                          className="cursor-grab text-mute hover:text-ink"
                          title="Arrastrar la página"
                        >
                          <GripVertical size={14} />
                        </span>
                        <input
                          value={prefs.pageLabels[p] ?? defaultLabel(p)}
                          onChange={(e) => rename(p, e.target.value)}
                          className="min-w-0 flex-1 rounded border border-transparent bg-transparent px-1.5 py-1 text-[13px] text-ink outline-none hover:border-line focus:border-neon/50"
                          aria-label={`Nombre de ${defaultLabel(p)}`}
                          title={prefs.pageLabels[p] ? `Nombre original: ${defaultLabel(p)}` : undefined}
                        />
                        <button onClick={() => toggleFav(p)} className={`rounded p-1 ${sb.favorites.includes(p) ? "text-neon" : "text-mute hover:text-ink"}`} title="Favorito">
                          <Star size={13} fill={sb.favorites.includes(p) ? "currentColor" : "none"} />
                        </button>
                        <button onClick={() => toggleHidden(p)} className="rounded p-1 text-mute hover:text-ink" title={hidden.has(p) ? "Mostrar en la barra lateral" : "Ocultar de la barra lateral"}>
                          {hidden.has(p) ? <EyeOff size={13} /> : <Eye size={13} />}
                        </button>
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
          );
        })}
      </div>

      <div className="col-span-12 space-y-3 lg:col-span-5">
        <Card title="Barra lateral">
          <Opt title="Modo" sub="Completa: las áreas y, al lado, cada pantalla con sus secciones. Solo áreas deja más espacio; las pantallas del área van como pestañas bajo el título.">
            <Segmented value={sb.mode} onChange={s("mode")} options={[["full", "Completa"], ["mini", "Solo áreas"]]} />
          </Opt>
          <Opt title="Posición">
            <Segmented value={sb.position} onChange={s("position")} options={[["left", "Izquierda"], ["right", "Derecha"]]} />
          </Opt>
          <Opt title="Ancho de las pantallas">
            <div className="flex items-center gap-2">
              {/* Elegir un ancho aquí manda sobre el que se haya ajustado arrastrando. */}
              <Segmented value={sb.width} onChange={(v) => setSidebar({ width: v, widthPx: null })} options={[["narrow", "Estrecha"], ["normal", "Normal"], ["wide", "Ancha"]]} />
              {sb.widthPx !== null && (
                <span className="text-[11px] text-mute" title="Lo ajustaste arrastrando el borde de la barra">
                  ajustado a mano: {sb.widthPx} px
                </span>
              )}
            </div>
          </Opt>
          <Opt title="Densidad">
            <Segmented value={sb.density} onChange={s("density")} options={[["compact", "Compacta"], ["normal", "Normal"], ["comfortable", "Espaciosa"]]} />
          </Opt>
          <Opt title="Elementos visibles">
            <Check checked={sb.showBadges} onChange={s("showBadges")} label="Estado al lado de cada sección (avisos, dominio, espacio)" />
            <Check checked={sb.showSearch} onChange={s("showSearch")} label="Buscador en la barra de arriba (Ctrl+K y F1 funcionan igual)" />
            <Check checked={sb.showSession} onChange={s("showSession")} label="Aviso de sesión de servicio en curso" />
          </Opt>
          <Opt title="Al pulsar un área, ir a…">
            <Segmented value={sb.areaClick} onChange={s("areaClick")} options={[["last", "La última pantalla usada"], ["first", "Su primera pantalla"]]} />
          </Opt>
        </Card>

        <Card title="Accesos rápidos en la barra">
          <Opt title="Fijados" sub="Arriba de la barra: las pantallas marcadas con ★ aquí y lo que fijes con la chincheta al pasar el ratón por la barra (también secciones, como Dominio).">
            {sb.favorites.length === 0 ? (
              <p className="text-xs text-mute">Ninguna todavía.</p>
            ) : (
              <div className="flex flex-wrap gap-1.5">
                {sb.favorites.map((k) => (
                  <button
                    key={k}
                    onClick={() => setSidebar({ favorites: sb.favorites.filter((x) => x !== k) })}
                    className="rounded-full bg-panel-2 px-2.5 py-0.5 text-xs text-ink hover:text-bad"
                    title="Quitar de fijados"
                  >
                    {navLabel(k)} ×
                  </button>
                ))}
              </div>
            )}
          </Opt>
          <Opt title="Recientes" sub="Las últimas pantallas que has abierto.">
            <Segmented
              value={String(sb.recents)}
              onChange={(v) => setSidebar({ recents: Number(v) })}
              options={[
                ["0", "Ocultas"],
                ["3", "3"],
                ["5", "5"],
                ["8", "8"],
              ]}
            />
          </Opt>
        </Card>

        <Button kind="ghost" onClick={reset}>
          <RotateCcw size={14} /> Restablecer toda la navegación
        </Button>
      </div>
      {dialog}
    </div>
  );
}
