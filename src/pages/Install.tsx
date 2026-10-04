import { Building2, CheckCircle2, Download, Eye, EyeOff, ListPlus, Loader2, PackagePlus, Plus, RefreshCw, Search, SlidersHorizontal, Trash2, TriangleAlert, X, XCircle } from "lucide-react";
import { bytes } from "../lib/format";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, ErrorState, Loading, Modal, inputClass, softBtn } from "../components/ui";
import { appsApi, type AppCatalog, type CatalogApp, type CatalogView, type InstallResult } from "../lib/api";
import { catalogCounts, isVisible } from "../lib/catalogView";

const CATEGORY: Record<string, string> = {
  browser: "Navegadores",
  compress: "Compresión",
  media: "Multimedia",
  chat: "Comunicación",
  remote: "Control remoto",
  vpn: "VPN y escritorios de la empresa",
  office: "Oficina, PDF y nube",
  utils: "Utilidades",
  tech: "Herramientas del técnico",
  security: "Seguridad",
  backup: "Copias de seguridad",
  drivers: "Drivers y utilidades del fabricante",
  runtime: "Componentes",
  games: "Juegos",
  dev: "Desarrollo",
};

const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export function Install({ isAdmin }: { isAdmin: boolean }) {
  const [catalog, setCatalog] = useState<AppCatalog | null>(null);
  const [installed, setInstalled] = useState<Set<string> | null>(null);
  const [installedError, setInstalledError] = useState<string | null>(null);
  // Seleccionados, en orden de selección; pueden venir de la búsqueda de winget.
  const [selected, setSelected] = useState<CatalogApp[]>([]);
  const [query, setQuery] = useState("");
  const [extra, setExtra] = useState<CatalogApp[]>([]);
  const [searching, setSearching] = useState(false);
  const [running, setRunning] = useState(false);
  const [checking, setChecking] = useState(false);
  const [results, setResults] = useState<InstallResult[] | null>(null);
  const [saving, setSaving] = useState(false);
  /** Categoría que se está viendo (null: todas). */
  const [category, setCategory] = useState<string | null>(null);
  const [hideInstalled, setHideInstalled] = useState(false);
  /** Eligiendo qué enseña el catálogo: los programas se ocultan o se muestran al pulsarlos. */
  const [customizing, setCustomizing] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const [failed, setFailed] = useState<string | null>(null);
  const loadCatalog = useCallback(() => {
    setFailed(null);
    return appsApi
      .catalog()
      .then(setCatalog)
      .catch((e) => setFailed(String(e)));
  }, []);
  const loadInstalled = useCallback(() => {
    setInstalled(null);
    setInstalledError(null);
    appsApi
      .installed()
      .then((ids) => setInstalled(new Set(ids.map((i) => i.toLowerCase()))))
      .catch((e) => setInstalledError(String(e)));
  }, []);

  useEffect(() => {
    void loadCatalog();
    loadInstalled();
  }, [loadCatalog, loadInstalled]);

  const isSel = (id: string) => selected.some((a) => a.id === id);
  const toggle = (a: CatalogApp) => setSelected((s) => (s.some((x) => x.id === a.id) ? s.filter((x) => x.id !== a.id) : [...s, a]));
  const isInstalled = (id: string) => installed?.has(id.toLowerCase()) ?? false;

  const byId = useMemo(() => new Map((catalog?.apps ?? []).map((a) => [a.id, a])), [catalog]);

  const selectList = (apps: CatalogApp[]) => {
    const add = apps.filter((a) => !isInstalled(a.id));
    setSelected((s) => [...s, ...add.filter((a) => !s.some((x) => x.id === a.id))]);
    const skipped = apps.length - add.length;
    toast("info", `${add.length} programas añadidos a la selección${skipped ? ` (${skipped} ya instalados)` : ""}.`);
  };

  const searchWinget = async () => {
    setSearching(true);
    try {
      const r = await appsApi.search(query);
      const known = new Set(catalog?.apps.map((a) => a.id));
      setExtra(r.filter((a) => !known.has(a.id)).map(({ id, name, source }) => ({ id, name, source, category: "" })));
      if (!r.length) toast("info", `winget no encontró nada para «${query}».`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setSearching(false);
    }
  };

  const install = async () => {
    // Antes de una instalación larga: red, espacio, winget y permisos. Vale más
    // avisar ahora que dejar el equipo del cliente a medio hacer.
    setChecking(true);
    const pre = await appsApi.preflight(selected.length).catch(() => null);
    setChecking(false);
    const ok = await confirm({
      title: `Instalar ${selected.length} programas`,
      body: (
        <>
          {pre && pre.warnings.length > 0 && (
            <ul className="mb-3 space-y-1.5 rounded-lg border border-warn/40 bg-warn/5 p-3">
              {pre.warnings.map((w) => (
                <li key={w} className="flex items-start gap-2 text-xs text-warn">
                  <TriangleAlert size={13} className="mt-0.5 shrink-0" />
                  {w}
                </li>
              ))}
            </ul>
          )}
          Se descargarán e instalarán en silencio, uno tras otro, desde los repositorios oficiales de winget y Microsoft Store.
          {!isAdmin && " Sin administrador, algunos instaladores pedirán permiso (UAC)."}
          {pre && !pre.lowSpace && pre.free > 0 && <span className="mt-2 block text-xs text-mute">Quedan {bytes(pre.free)} libres en el disco del sistema.</span>}
        </>
      ),
      confirmLabel: pre && !pre.online ? "Instalar de todas formas" : "Instalar",
      danger: !!pre && (!pre.online || !pre.winget),
    });
    if (!ok) return;
    setRunning(true);
    setResults(null);
    try {
      const r = await appsApi.install(selected);
      setResults(r);
      const good = r.filter((x) => x.ok).length;
      toast(good === r.length ? "ok" : "info", `${good} de ${r.length} programas instalados.`);
      setSelected((s) => s.filter((a) => r.some((x) => x.id === a.id && !x.ok)));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(false);
      loadInstalled();
    }
  };

  const deleteList = async (id: string, name: string) => {
    if (!(await confirm({ title: "Eliminar lista", body: `¿Eliminar la lista «${name}»?`, confirmLabel: "Eliminar", danger: true }))) return;
    await appsApi.deleteList(id).catch((e) => toast("error", String(e)));
    void loadCatalog();
  };

  if (!catalog) return failed ? <ErrorState page message={failed} onRetry={() => void loadCatalog()} /> : <Loading page text="Cargando catálogo…" />;

  const view = catalog.view;
  // Se guarda al momento y viaja con tus datos: en otro equipo ves el mismo catálogo.
  const saveView = (next: CatalogView) => {
    setCatalog({ ...catalog, view: next });
    appsApi.setCatalogView(next).catch((e) => toast("error", String(e)));
  };
  const toggleApp = (id: string) => saveView({ ...view, hiddenApps: view.hiddenApps.includes(id) ? view.hiddenApps.filter((x) => x !== id) : [...view.hiddenApps, id] });
  const toggleCategory = (c: string) =>
    saveView({ ...view, hiddenCategories: view.hiddenCategories.includes(c) ? view.hiddenCategories.filter((x) => x !== c) : [...view.hiddenCategories, c] });

  const q = norm(query.trim());
  const matches = (a: CatalogApp) => !q || norm(a.name).includes(q) || norm(a.id).includes(q);
  const shown = (a: CatalogApp) => isVisible(a, view) && !(hideInstalled && isInstalled(a.id));
  // Personalizando se ve todo el catálogo: lo oculto sale apagado, para poder volver a mostrarlo.
  const pool = catalog.apps.filter((a) => matches(a) && (customizing || shown(a)));
  // Al buscar, lo oculto que coincide se ofrece aparte: no se pierde nada por ocultarlo.
  const hiddenMatches = q && !customizing ? catalog.apps.filter((a) => matches(a) && !isVisible(a, view)) : [];
  const counts = catalogCounts(catalog.apps, view);
  const categories = Object.keys(CATEGORY)
    .map((c) => ({ id: c, apps: pool.filter((a) => a.category === c) }))
    .filter((g) => g.apps.length);
  const groups = categories.filter((g) => category === null || g.id === category);
  // Una lista de fábrica se enseña si la mayoría de sus programas están a la vista.
  const presets = catalog.presets.filter((p) => customizing || p.apps.filter((id) => byId.get(id) && isVisible(byId.get(id)!, view)).length * 2 > p.apps.length);

  const chip = (a: CatalogApp) => {
    const on = isSel(a.id);
    const inst = isInstalled(a.id);
    const off = !isVisible(a, view);
    if (customizing) {
      const byCategory = view.hiddenCategories.includes(a.category);
      const byBusiness = view.business && !!a.home;
      const byHand = view.hiddenApps.includes(a.id);
      return (
        <button
          key={a.id}
          onClick={() => toggleApp(a.id)}
          disabled={byCategory}
          title={byCategory ? "Su categoría entera está oculta" : byHand ? "Volver a mostrarlo" : "Ocultarlo del catálogo"}
          className={`flex items-center gap-2 rounded-md border px-2.5 py-1.5 text-left text-[13px] transition-colors disabled:cursor-not-allowed ${
            off ? "border-dashed border-line text-mute" : "border-line bg-panel text-ink hover:border-line-2"
          }`}
        >
          {byHand || byCategory ? <EyeOff size={13} className="shrink-0" /> : <Eye size={13} className="shrink-0 text-neon" />}
          <span className={`truncate ${byHand || byCategory ? "line-through" : ""}`}>{a.name}</span>
          {byBusiness && !byHand && !byCategory && <span className="ml-auto shrink-0 rounded border border-line px-1 text-[10px] text-mute">uso personal</span>}
        </button>
      );
    }
    return (
      <button
        key={a.id}
        onClick={() => toggle(a)}
        disabled={running}
        title={`${a.id}${a.source === "msstore" ? " · Microsoft Store" : ""}${inst ? " · ya instalado (se actualizará si hay versión nueva)" : ""}`}
        className={`flex items-center gap-2 rounded-md border px-2.5 py-1.5 text-left text-[13px] transition-colors ${
          on ? "border-neon/60 bg-neon/10 text-neon" : off ? "border-dashed border-line text-dim hover:border-line-2" : "border-line bg-panel text-ink hover:border-line-2"
        }`}
      >
        <input type="checkbox" readOnly checked={on} className="pointer-events-none size-3.5 accent-[var(--color-neon)]" />
        <span className="truncate">{a.name}</span>
        {inst && <span className="ml-auto shrink-0 rounded border border-ok/40 px-1 text-[10px] text-ok">instalado</span>}
      </button>
    );
  };

  const pill = (on: boolean) => `rounded-full border px-2.5 py-0.5 text-xs transition-colors ${on ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`;

  return (
    <div className="mx-auto max-w-6xl p-6">
      {/* Listas */}
      <section className="mb-5">
        <h2 className="mb-2 text-[11px] font-semibold text-dim">Listas</h2>
        <div className="flex flex-wrap gap-2">
          {presets.map((p) => (
            <button
              key={p.id}
              onClick={() => selectList(p.apps.map((id) => byId.get(id)!).filter(Boolean))}
              disabled={running || customizing}
              title={`${p.description}\n\n${p.apps.map((id) => byId.get(id)?.name).join(", ")}`}
              className="rounded-lg border border-line bg-panel px-3 py-2 text-left transition-colors hover:border-neon/50 disabled:opacity-50"
            >
              <div className="text-sm font-medium text-ink">{p.name}</div>
              <div className="text-[11px] text-mute">{p.apps.length} programas</div>
            </button>
          ))}
          {catalog.lists.map((l) => (
            <div key={l.id} className="group relative">
              <button
                onClick={() => selectList(l.apps)}
                disabled={running || customizing}
                title={l.apps.map((a) => a.name).join(", ")}
                className="rounded-lg border border-neon/30 bg-panel px-3 py-2 pr-7 text-left transition-colors hover:border-neon/60 disabled:opacity-50"
              >
                <div className="text-sm font-medium text-neon">{l.name}</div>
                <div className="text-[11px] text-mute">{l.apps.length} programas · tuya</div>
              </button>
              <button
                onClick={() => deleteList(l.id, l.name)}
                className="absolute top-1.5 right-1.5 rounded p-0.5 text-mute opacity-0 group-hover:opacity-100 hover:text-bad"
                title="Eliminar lista"
              >
                <Trash2 size={11} />
              </button>
            </div>
          ))}
          <button
            onClick={() => setSaving(true)}
            disabled={!selected.length}
            className="flex items-center gap-1.5 rounded-lg border border-dashed border-line px-3 py-2 text-sm text-mute transition-colors hover:border-neon/50 hover:text-ink disabled:opacity-40"
            title={selected.length ? "Guardar la selección actual como lista" : "Selecciona programas para guardarlos como lista"}
          >
            <ListPlus size={14} /> Guardar selección como lista
          </button>
        </div>
      </section>

      {/* Búsqueda y qué se ve */}
      <div className="mb-3 flex flex-wrap items-center gap-3">
        <div className="relative w-80 max-w-full min-w-40">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setExtra([]);
            }}
            onKeyDown={(e) => e.key === "Enter" && query.trim().length >= 2 && searchWinget()}
            placeholder="Filtrar el catálogo o buscar en winget (Enter)…"
            className="w-full rounded-md border border-line bg-panel py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <Button kind="ghost" onClick={searchWinget} disabled={query.trim().length < 2 || searching}>
          {searching ? <Loader2 size={14} className="animate-spin" /> : <Search size={14} />} Buscar en winget
        </Button>
        <span className="flex overflow-hidden rounded-md border border-line text-xs" role="tablist" aria-label="Qué programas se ven">
          <button
            role="tab"
            aria-selected={view.business}
            onClick={() => saveView({ ...view, business: true })}
            className={`flex items-center gap-1.5 px-3 py-1.5 ${view.business ? "bg-neon/15 text-neon" : "text-mute hover:text-ink"}`}
            title="Sin juegos ni programas de uso personal"
          >
            <Building2 size={12} /> Empresa
          </button>
          <button role="tab" aria-selected={!view.business} onClick={() => saveView({ ...view, business: false })} className={`px-3 py-1.5 ${!view.business ? "bg-neon/15 text-neon" : "text-mute hover:text-ink"}`}>
            Todo
          </button>
        </span>
        <Button kind={customizing ? "primary" : "ghost"} onClick={() => setCustomizing(!customizing)} title="Elegir qué programas y categorías se muestran">
          <SlidersHorizontal size={14} /> {customizing ? "Listo" : "Personalizar"}
        </Button>
        <span className="ml-auto flex items-center gap-2 text-xs text-mute">
          {installed === null && !installedError && (
            <>
              <Loader2 size={12} className="animate-spin" /> Comprobando qué está instalado…
            </>
          )}
          {installed && (
            <label className="flex cursor-pointer items-center gap-1.5">
              <input type="checkbox" checked={hideInstalled} onChange={(e) => setHideInstalled(e.target.checked)} className="accent-[var(--color-neon)]" />
              Ocultar los {catalog.apps.filter((a) => isVisible(a, view) && isInstalled(a.id)).length} ya instalados
            </label>
          )}
          {installedError && <span className="text-warn" title={installedError}>No se pudo comprobar qué está instalado</span>}
          <button onClick={loadInstalled} className="rounded p-1 hover:text-ink" title="Volver a comprobar">
            <RefreshCw size={12} />
          </button>
        </span>
      </div>

      {customizing ? (
        <div className="mb-4 flex flex-wrap items-center gap-3 rounded-xl border border-neon/30 bg-neon/5 px-4 py-2.5 text-sm text-ink">
          <span className="min-w-0 flex-1">
            Pulsa un programa para ocultarlo o volver a mostrarlo, o esconde una categoría entera con el ojo de su título. Se guarda al momento y viaja con tus datos.
            <span className="block text-xs text-dim">Lo oculto sigue apareciendo si lo buscas por su nombre, y las listas que ya tenías siguen instalándolo.</span>
          </span>
          {(view.hiddenApps.length > 0 || view.hiddenCategories.length > 0) && (
            <Button kind="ghost" onClick={() => saveView({ ...view, hiddenApps: [], hiddenCategories: [] })}>
              Mostrar todo otra vez
            </Button>
          )}
        </div>
      ) : (
        <p className="mb-4 text-xs text-mute">
          {counts.visible} programas a la vista
          {counts.hidden > 0 && (
            <>
              {" · "}
              {counts.hidden} ocultos
              {counts.home > 0 && ` (${counts.home} de uso personal, fuera de la vista «Empresa»)`}
            </>
          )}
        </p>
      )}

      {results && (
        <div className="mb-4 rounded-xl border border-line bg-panel p-4">
          <div className="mb-2 flex items-center justify-between">
            <h3 className="text-sm font-medium">Resultado</h3>
            <button onClick={() => setResults(null)} className="text-mute hover:text-ink">
              <X size={14} />
            </button>
          </div>
          <ul className="space-y-1 text-xs">
            {results.map((r) => (
              <li key={r.id} className="flex items-start gap-2">
                {r.ok ? <CheckCircle2 size={13} className="mt-px shrink-0 text-ok" /> : <XCircle size={13} className="mt-px shrink-0 text-bad" />}
                <span className="text-ink">{r.name}</span>
                <span className={`min-w-0 flex-1 truncate ${r.ok ? "text-dim" : "text-bad"}`} title={r.message}>
                  {r.message}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Categorías: de un vistazo cuántos hay en cada una, y ver solo una. */}
      {categories.length > 1 && (
        <div className="mb-4 flex flex-wrap gap-1.5">
          <button onClick={() => setCategory(null)} className={pill(category === null)}>
            Todas · {pool.length}
          </button>
          {categories.map((g) => (
            <button key={g.id} onClick={() => setCategory(category === g.id ? null : g.id)} className={pill(category === g.id)}>
              {CATEGORY[g.id]} · {g.apps.length}
            </button>
          ))}
        </div>
      )}

      {extra.length > 0 && (
        <section className="mb-5">
          <h2 className="mb-2 text-[11px] font-semibold text-dim">Encontrado en winget</h2>
          <div className="grid grid-cols-2 gap-1.5 md:grid-cols-3 xl:grid-cols-4">{extra.map(chip)}</div>
        </section>
      )}

      {groups.map((g) => {
        const hiddenCategory = view.hiddenCategories.includes(g.id);
        return (
          <section key={g.id} className="mb-5">
            <h2 className="mb-2 flex items-center gap-2 text-[11px] font-semibold text-dim">
              <span className={hiddenCategory ? "line-through" : ""}>{CATEGORY[g.id]}</span>
              <span className="font-normal text-mute">{g.apps.length}</span>
              {customizing && (
                <button onClick={() => toggleCategory(g.id)} className="flex items-center gap-1 font-normal text-mute hover:text-ink" title={hiddenCategory ? "Volver a mostrar la categoría" : "Ocultar la categoría entera"}>
                  {hiddenCategory ? <EyeOff size={12} /> : <Eye size={12} className="text-neon" />} {hiddenCategory ? "Oculta" : "Ocultar categoría"}
                </button>
              )}
            </h2>
            <div className="grid grid-cols-2 gap-1.5 md:grid-cols-3 xl:grid-cols-4">{g.apps.map(chip)}</div>
          </section>
        );
      })}

      {hiddenMatches.length > 0 && (
        <section className="mb-5">
          <h2 className="mb-2 text-[11px] font-semibold text-dim">
            Ocultos que coinciden <span className="font-normal text-mute">· no salen en tu catálogo, pero se pueden instalar</span>
          </h2>
          <div className="grid grid-cols-2 gap-1.5 md:grid-cols-3 xl:grid-cols-4">{hiddenMatches.map(chip)}</div>
        </section>
      )}

      {groups.length === 0 && !extra.length && hiddenMatches.length === 0 && (
        <p className="text-sm text-mute">
          {q ? (
            <>
              Nada en el catálogo coincide con «{query}». Pulsa <span className="text-ink">Buscar en winget</span> para buscar entre miles de programas.
            </>
          ) : (
            "No hay programas que enseñar con lo que tienes oculto. Pulsa «Personalizar» para volver a mostrarlos."
          )}
        </p>
      )}

      {selected.length > 0 && (
        <div className="sticky bottom-0 z-30 -mx-6 -mb-6 mt-6 border-t border-line bg-panel px-6 py-3">
          <div className="mx-auto flex max-w-6xl items-center justify-between gap-4">
            {running ? (
              <TaskStatus task="install-apps" active={running} fallback="Instalando…" />
            ) : (
              <span className="min-w-0 truncate text-sm text-dim" title={selected.map((a) => a.name).join(", ")}>
                <span className="font-mono text-neon">{selected.length}</span> seleccionados: {selected.map((a) => a.name).join(", ")}
              </span>
            )}
            <div className="flex shrink-0 gap-2">
              {!running && (
                <Button kind="ghost" onClick={() => setSelected([])}>
                  Vaciar
                </Button>
              )}
              <button
                onClick={install}
                disabled={running || checking}
                className={softBtn}
              >
                {checking ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />} {checking ? "Comprobando…" : "Instalar"}
              </button>
            </div>
          </div>
        </div>
      )}

      {saving && (
        <SaveList
          apps={selected}
          onClose={() => setSaving(false)}
          onSaved={() => {
            setSaving(false);
            void loadCatalog();
            toast("ok", "Lista guardada.");
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function SaveList({ apps, onClose, onSaved }: { apps: CatalogApp[]; onClose: () => void; onSaved: () => void }) {
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const save = async () => {
    try {
      await appsApi.saveList({ id: "", name, apps });
      onSaved();
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <Modal
      title="Guardar lista"
      onClose={onClose}
      footer={
        <>
          {error && <p className="mr-auto text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save} disabled={!name.trim()}>
            <Plus size={14} /> Guardar
          </Button>
        </>
      }
    >
      <input
        autoFocus
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && name.trim() && save()}
        maxLength={40}
        placeholder="Nombre de la lista (p. ej. Cliente oficina)"
        className={inputClass}
      />
      <p className="mt-3 text-xs text-mute">
        <PackagePlus size={12} className="mr-1 inline" />
        {apps.map((a) => a.name).join(", ")}
      </p>
    </Modal>
  );
}
