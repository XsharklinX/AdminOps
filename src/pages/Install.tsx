import { CheckCircle2, Download, ListPlus, Loader2, PackagePlus, Plus, RefreshCw, Search, Trash2, X, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Modal, inputClass } from "../components/ui";
import { appsApi, type AppCatalog, type CatalogApp, type InstallResult } from "../lib/api";

const CATEGORY: Record<string, string> = {
  browser: "Navegadores",
  compress: "Compresión",
  media: "Multimedia",
  chat: "Comunicación",
  remote: "Control remoto",
  office: "Oficina, PDF y nube",
  utils: "Utilidades",
  tech: "Herramientas del técnico",
  security: "Seguridad",
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
  const [results, setResults] = useState<InstallResult[] | null>(null);
  const [saving, setSaving] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const loadCatalog = useCallback(() => appsApi.catalog().then(setCatalog).catch((e) => toast("error", String(e))), [toast]);
  const loadInstalled = useCallback(() => {
    setInstalled(null);
    setInstalledError(null);
    appsApi
      .installed()
      .then((ids) => setInstalled(new Set(ids.map((i) => i.toLowerCase()))))
      .catch((e) => setInstalledError(String(e)));
  }, []);

  useEffect(() => {
    loadCatalog();
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
    const ok = await confirm({
      title: `Instalar ${selected.length} programas`,
      body: (
        <>
          Se descargarán e instalarán en silencio, uno tras otro, desde los repositorios oficiales de winget y Microsoft Store.
          {!isAdmin && " Sin administrador, algunos instaladores pedirán permiso (UAC)."}
        </>
      ),
      confirmLabel: "Instalar",
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
    loadCatalog();
  };

  if (!catalog) return <p className="p-8 font-mono text-sm text-mute">Cargando catálogo…</p>;

  const q = norm(query.trim());
  const filtered = catalog.apps.filter((a) => !q || norm(a.name).includes(q) || norm(a.id).includes(q));
  const groups = Object.keys(CATEGORY)
    .map((c) => [c, filtered.filter((a) => a.category === c)] as const)
    .filter(([, apps]) => apps.length);

  const chip = (a: CatalogApp) => {
    const on = isSel(a.id);
    const inst = isInstalled(a.id);
    return (
      <button
        key={a.id}
        onClick={() => toggle(a)}
        disabled={running}
        title={`${a.id}${a.source === "msstore" ? " · Microsoft Store" : ""}${inst ? " · ya instalado (se actualizará si hay versión nueva)" : ""}`}
        className={`flex items-center gap-2 rounded-md border px-2.5 py-1.5 text-left text-[13px] transition-colors ${
          on ? "border-neon/60 bg-neon/10 text-neon" : "border-line bg-panel text-ink hover:border-line-2"
        }`}
      >
        <input type="checkbox" readOnly checked={on} className="pointer-events-none size-3.5 accent-[var(--color-neon)]" />
        <span className="truncate">{a.name}</span>
        {inst && <span className="ml-auto shrink-0 rounded border border-ok/40 px-1 text-[10px] text-ok">instalado</span>}
      </button>
    );
  };

  return (
    <div className="mx-auto max-w-6xl p-6 pb-24">
      {/* Listas */}
      <section className="mb-5">
        <h2 className="mb-2 text-[11px] font-semibold text-dim">Listas</h2>
        <div className="flex flex-wrap gap-2">
          {catalog.presets.map((p) => (
            <button
              key={p.id}
              onClick={() => selectList(p.apps.map((id) => byId.get(id)!).filter(Boolean))}
              disabled={running}
              title={`${p.description}\n\n${p.apps.map((id) => byId.get(id)?.name).join(", ")}`}
              className="rounded-lg border border-line bg-panel px-3 py-2 text-left transition-colors hover:border-neon/50"
            >
              <div className="text-sm font-medium text-ink">{p.name}</div>
              <div className="text-[11px] text-mute">{p.apps.length} programas</div>
            </button>
          ))}
          {catalog.lists.map((l) => (
            <div key={l.id} className="group relative">
              <button
                onClick={() => selectList(l.apps)}
                disabled={running}
                title={l.apps.map((a) => a.name).join(", ")}
                className="rounded-lg border border-neon/30 bg-panel px-3 py-2 pr-7 text-left transition-colors hover:border-neon/60"
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

      {/* Búsqueda */}
      <div className="mb-4 flex items-center gap-3">
        <div className="relative w-96">
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
        <span className="ml-auto flex items-center gap-2 text-xs text-mute">
          {installed === null && !installedError && (
            <>
              <Loader2 size={12} className="animate-spin" /> Comprobando qué está instalado…
            </>
          )}
          {installed && <>{catalog.apps.filter((a) => isInstalled(a.id)).length} del catálogo ya instalados</>}
          {installedError && <span className="text-warn" title={installedError}>No se pudo comprobar qué está instalado</span>}
          <button onClick={loadInstalled} className="rounded p-1 hover:text-ink" title="Volver a comprobar">
            <RefreshCw size={12} />
          </button>
        </span>
      </div>

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

      {extra.length > 0 && (
        <section className="mb-5">
          <h2 className="mb-2 text-[11px] font-semibold text-dim">Encontrado en winget</h2>
          <div className="grid grid-cols-2 gap-1.5 md:grid-cols-3 xl:grid-cols-4">{extra.map(chip)}</div>
        </section>
      )}

      {groups.map(([c, apps]) => (
        <section key={c} className="mb-5">
          <h2 className="mb-2 text-[11px] font-semibold text-dim">{CATEGORY[c]}</h2>
          <div className="grid grid-cols-2 gap-1.5 md:grid-cols-3 xl:grid-cols-4">{apps.map(chip)}</div>
        </section>
      ))}
      {groups.length === 0 && !extra.length && (
        <p className="text-sm text-mute">
          Nada en el catálogo coincide con «{query}». Pulsa <span className="text-ink">Buscar en winget</span> para buscar entre miles de programas.
        </p>
      )}

      {selected.length > 0 && (
        <div className="fixed right-0 bottom-0 left-60 z-30 border-t border-line bg-panel px-6 py-3">
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
                disabled={running}
                className="flex items-center gap-1.5 rounded-md border border-neon/50 bg-neon/10 px-4 py-1.5 text-sm font-medium text-neon hover:bg-neon/20 disabled:opacity-50"
              >
                <Download size={14} /> Instalar
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
            loadCatalog();
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
