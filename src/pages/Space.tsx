import { listen } from "@tauri-apps/api/event";
import {
  ArrowUp,
  CalendarClock,
  Check,
  ChevronRight,
  Copy,
  CornerDownRight,
  File,
  Folder,
  FolderOpen,
  HardDrive,
  Loader2,
  Recycle,
  ScanSearch,
  Search,
  Sparkles,
  Square,
  TriangleAlert,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, smallBtn } from "../components/ui";
import { api, logQuietly, toolsApi, type Freeable, type SpaceEntry, type SpaceFolder, type SpaceView } from "../lib/api";
import { bytes, ago, friendlyPath } from "../lib/format";
import { crumbsOf, type FolderSort, kindColor, kindLabel, kindShares, LOOSE, looseSize, matchesText, parentOf, percent, sortEntries } from "../lib/spaceView";
import { squarify } from "../lib/treemap";
import type { PageId } from "../components/Sidebar";

/** Hace cuánto se tocó por última vez, en palabras. */
const since = (epoch: number | undefined) => (epoch ? ago(epoch) : "");

type FileTab = "here" | "big" | "old" | "kind";
/** Lo marcado para la papelera: de cada cosa, lo que hace falta para contarlo. */
type Picked = Map<string, { name: string; size: number; dir: boolean }>;

const NONE: SpaceEntry[] = [];

const SORTS: [FolderSort, string][] = [
  ["size", "Lo que más ocupa"],
  ["old", "Lo más antiguo"],
  ["files", "Más archivos"],
  ["name", "Nombre"],
];

export function Space({ onNavigate }: { onNavigate?: (p: PageId, focus?: string | null) => void }) {
  const [drives, setDrives] = useState<{ mount: string; free: number; total: number }[]>([]);
  const [path, setPath] = useState("C:\\");
  const [scan, setScan] = useState<SpaceView | null>(null);
  // La carpeta que se está mirando dentro del análisis.
  const [folder, setFolder] = useState<SpaceFolder | null>(null);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<{ files: number; bytes: number } | null>(null);
  const [freeable, setFreeable] = useState<Freeable[] | null>(null);
  const [folderView, setFolderView] = useState<"map" | "list">("map");
  const [sort, setSort] = useState<FolderSort>("size");
  // La subcarpeta señalada (un clic): se ve su ficha debajo.
  const [focused, setFocused] = useState<string | null>(null);
  const [fileTab, setFileTab] = useState<FileTab>("big");
  const [kind, setKind] = useState<string | null>(null);
  const [hereFiles, setHereFiles] = useState<SpaceEntry[] | null>(null);
  const [kindFiles, setKindFiles] = useState<SpaceEntry[] | null>(null);
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<Picked>(new Map());
  const [recycling, setRecycling] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const loadFreeable = useCallback(() => {
    setFreeable(null);
    toolsApi
      .spaceFreeable()
      .then(setFreeable)
      .catch(() => setFreeable([]));
  }, []);
  const loadDrives = useCallback(() => {
    void api
      .liveMetrics()
      .then((m) => setDrives(m.disks.map((d) => ({ mount: d.mount, free: d.available, total: d.total }))))
      .catch(logQuietly("Space"));
  }, []);
  useEffect(() => {
    loadFreeable();
    loadDrives();
    const un = listen<{ files: number; bytes: number }>("space-progress", (e) => setProgress(e.payload));
    return () => {
      void un.then((f) => f());
    };
  }, [loadFreeable, loadDrives]);

  // Los archivos sueltos de la carpeta actual se leen al mirarlos, no antes.
  const herePath = folder?.path;
  useEffect(() => {
    if (fileTab !== "here" || !herePath) return;
    let stale = false;
    setHereFiles(null);
    toolsApi
      .spaceFiles(herePath)
      .then((f) => !stale && setHereFiles(f))
      .catch(() => !stale && setHereFiles([]));
    return () => {
      stale = true;
    };
  }, [fileTab, herePath]);

  const start = async (target = path) => {
    setRunning(true);
    setProgress(null);
    try {
      const v = await toolsApi.scanSpace(target);
      setScan(v);
      setFolder(v.folder);
      setPicked(new Map());
      setFocused(null);
      setKind(null);
      setKindFiles(null);
      setQuery("");
      setFileTab("big");
    } catch (e) {
      toast(String(e).includes("cancelado") ? "info" : "error", String(e));
    } finally {
      setRunning(false);
    }
  };

  const openFolder = async (target: string) => {
    try {
      setFolder(await toolsApi.spaceFolder(target));
      setFocused(null);
    } catch (err) {
      toast("error", String(err));
    }
  };

  const showKind = (id: string) => {
    setKind(id);
    setFileTab("kind");
    setKindFiles(null);
    toolsApi
      .spaceKindFiles(id)
      .then(setKindFiles)
      .catch(() => setKindFiles([]));
  };

  const reveal = (p: string) => toolsApi.revealInExplorer(p).catch((e) => toast("error", String(e)));
  const copyPath = (p: string) =>
    navigator.clipboard
      .writeText(p)
      .then(() => toast("ok", "Ruta copiada."))
      .catch(() => toast("error", "No se pudo copiar."));

  const toggle = (e: SpaceEntry, dir: boolean) =>
    setPicked((m) => {
      const n = new Map(m);
      if (n.has(e.path)) n.delete(e.path);
      else n.set(e.path, { name: e.name, size: e.size, dir });
      return n;
    });
  const pickedSize = [...picked.values()].reduce((n, p) => n + p.size, 0);

  const recycle = async () => {
    const list = [...picked.keys()];
    const folders = [...picked.values()].filter((p) => p.dir).length;
    const names = [...picked.values()].slice(0, 5).map((p) => p.name);
    const ok = await confirm({
      title: `¿Enviar ${list.length} ${list.length === 1 ? "elemento" : "elementos"} a la papelera?`,
      body: (
        <div className="space-y-2">
          <p>
            Se liberarán unos <span className="font-mono text-ink">{bytes(pickedSize)}</span>. Van a la papelera de Windows, así que se pueden recuperar hasta
            que se vacíe.
          </p>
          <ul className="list-inside list-disc text-[13px] text-dim">
            {names.map((n) => (
              <li key={n} className="truncate">
                {n}
              </li>
            ))}
            {list.length > names.length && <li>y {list.length - names.length} más</li>}
          </ul>
          {folders > 0 && (
            <p className="text-[13px] text-warn">
              {folders === 1 ? "Incluye una carpeta entera" : `Incluye ${folders} carpetas enteras`}, con todo lo que hay dentro.
            </p>
          )}
          <p className="text-[12px] text-mute">Lo que sea demasiado grande para la papelera, Windows lo borra del todo.</p>
        </div>
      ),
      confirmLabel: "Enviar a la papelera",
      danger: true,
    });
    if (!ok) return;
    setRecycling(true);
    try {
      const freed = await toolsApi.spaceRecycle(list);
      toast("ok", `${list.length} elemento(s) a la papelera · ${bytes(freed)} liberados.`);
      // El análisis ya está al día en el equipo: aquí solo se quita de las listas.
      const gone = (e: SpaceEntry) => list.some((p) => e.path === p || e.path.startsWith(`${p}\\`));
      setScan((s) => s && { ...s, size: Math.max(0, s.size - freed), largestFiles: s.largestFiles.filter((e) => !gone(e)), oldFiles: s.oldFiles.filter((e) => !gone(e)) });
      setHereFiles((f) => f && f.filter((e) => !gone(e)));
      setKindFiles((f) => f && f.filter((e) => !gone(e)));
      setPicked(new Map());
      setFocused(null);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRecycling(false);
      if (folder) {
        // Si la carpeta que se miraba ya no existe, se vuelve al principio.
        await toolsApi
          .spaceFolder(folder.path)
          .catch(() => (scan ? toolsApi.spaceFolder(scan.root) : Promise.reject()))
          .then(setFolder)
          .catch(logQuietly("Space"));
      }
      loadFreeable();
      loadDrives();
    }
  };

  const crumbs = scan && folder ? crumbsOf(scan.root, folder.path) : [];
  const entries = useMemo(() => (folder ? sortEntries(folder.children, sort) : []), [folder, sort]);
  const loose = folder ? looseSize(folder) : 0;
  const shares = useMemo(() => (folder ? kindShares(folder.kinds) : []), [folder]);
  const focusedEntry = entries.find((e) => e.path === focused) ?? null;
  const biggest = entries.reduce((m, e) => Math.max(m, e.size), 1);

  const tabFiles = fileTab === "here" ? hereFiles : fileTab === "kind" ? kindFiles : fileTab === "old" ? (scan?.oldFiles ?? NONE) : (scan?.largestFiles ?? NONE);
  const files = useMemo(() => (tabFiles ?? NONE).filter((f) => matchesText(f.name, query)), [tabFiles, query]);
  const allPicked = files.length > 0 && files.every((f) => picked.has(f.path));
  const pickAll = () =>
    setPicked((m) => {
      const n = new Map(m);
      for (const f of files) {
        if (allPicked) n.delete(f.path);
        else n.set(f.path, { name: f.name, size: f.size, dir: false });
      }
      return n;
    });

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <Card title="Qué analizar" icon={<HardDrive size={14} />} className="col-span-12">
        <div className="flex flex-wrap items-stretch gap-2">
          {drives.map((d) => {
            const used = d.total ? (d.total - d.free) / d.total : 0;
            return (
              <button
                key={d.mount}
                onClick={() => setPath(d.mount)}
                onDoubleClick={() => {
                  setPath(d.mount);
                  if (!running) void start(d.mount);
                }}
                title="Doble clic para analizarla"
                className={`w-44 rounded-lg border px-3 py-2 text-left text-xs transition-colors ${path === d.mount ? "border-neon/60 bg-neon/10" : "border-line-2 hover:border-line-2 hover:bg-panel-2"}`}
              >
                <div className="flex items-baseline justify-between">
                  <span className={`font-mono text-sm ${path === d.mount ? "text-neon" : "text-ink"}`}>{d.mount}</span>
                  <span className="text-[11px] text-mute">{Math.round(used * 100)} % lleno</span>
                </div>
                <div className="my-1.5 h-1.5 overflow-hidden rounded-full bg-line">
                  <div className={`h-full rounded-full ${used >= 0.9 ? "bg-bad" : used >= 0.75 ? "bg-warn" : "bg-neon/70"}`} style={{ width: `${used * 100}%` }} />
                </div>
                <div className="text-[11px] text-mute">
                  {bytes(d.free)} libres de {bytes(d.total)}
                </div>
              </button>
            );
          })}
          <div className="flex min-w-72 flex-1 items-center gap-2">
            <input
              value={path}
              onChange={(e) => setPath(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && !running && void start()}
              className="min-w-0 flex-1 rounded-md border border-line bg-void/60 px-3 py-2 font-mono text-xs text-ink outline-none focus:border-neon/50"
              placeholder="C:\Users"
              aria-label="Carpeta o unidad que analizar"
            />
            {running ? (
              <button onClick={() => toolsApi.cancelSpaceScan()} className="flex items-center gap-1.5 rounded-md border border-bad/50 px-4 py-2 text-sm text-bad hover:bg-bad/10">
                <Square size={12} fill="currentColor" /> Detener
              </button>
            ) : (
              <button onClick={() => start()} className="flex items-center gap-1.5 rounded-md border border-neon/50 bg-neon/10 px-4 py-2 text-sm font-medium text-neon hover:bg-neon/20">
                <ScanSearch size={14} /> Analizar
              </button>
            )}
          </div>
        </div>
        {running && (
          <p className="mt-3 flex items-center gap-2 font-mono text-xs text-neon">
            <Loader2 size={12} className="animate-spin" />
            {progress ? `${progress.files.toLocaleString("es")} archivos · ${bytes(progress.bytes)}` : "Empezando…"}
          </p>
        )}
        {scan && !running && (
          <p className="mt-3 text-xs text-dim">
            {bytes(scan.size)} en {scan.files.toLocaleString("es")} archivos · analizado en {scan.seconds.toFixed(1)} s
            {scan.denied > 0 && <span className="text-mute"> · {scan.denied} carpetas sin acceso (ejecuta como administrador para verlas)</span>}
          </p>
        )}
      </Card>

      {scan && folder && (
        <>
          <Card
            title="Carpetas"
            icon={<Folder size={14} />}
            className="col-span-12 lg:col-span-7"
            right={
              <div className="flex items-center gap-2 text-[11px]">
                {folderView === "list" && (
                  <select value={sort} onChange={(e) => setSort(e.target.value as FolderSort)} aria-label="Ordenar por" className="rounded border border-line bg-void/60 px-1.5 py-0.5 text-[11px] text-dim outline-none">
                    {SORTS.map(([id, label]) => (
                      <option key={id} value={id}>
                        {label}
                      </option>
                    ))}
                  </select>
                )}
                <div className="flex gap-1" role="group" aria-label="Cómo verlas">
                  {(
                    [
                      ["map", "Mapa"],
                      ["list", "Lista"],
                    ] as const
                  ).map(([id, label]) => (
                    <button
                      key={id}
                      onClick={() => setFolderView(id)}
                      aria-pressed={folderView === id}
                      className={`rounded px-2 py-0.5 transition-colors ${folderView === id ? "bg-neon/10 text-neon" : "text-mute hover:text-ink"}`}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            }
          >
            <div className="mb-2 flex flex-wrap items-center gap-1 text-xs">
              <button
                onClick={() => void openFolder(parentOf(folder.path))}
                disabled={crumbs.length < 2}
                className="mr-1 rounded p-1 text-mute hover:bg-panel-2 hover:text-ink disabled:pointer-events-none disabled:opacity-30"
                title="Subir una carpeta"
                aria-label="Subir una carpeta"
              >
                <ArrowUp size={13} />
              </button>
              {crumbs.map((c, i) => (
                <span key={c.path} className="flex items-center gap-1">
                  {i > 0 && <ChevronRight size={11} className="text-mute" />}
                  <button onClick={() => void openFolder(c.path)} className={i === crumbs.length - 1 ? "text-neon" : "text-dim hover:text-ink"}>
                    {c.name}
                  </button>
                </span>
              ))}
              <span className="ml-auto font-mono text-[11px] text-mute">
                {bytes(folder.size)} · {folder.files.toLocaleString("es")} archivos
              </span>
            </div>

            {shares.length > 0 && (
              <div className="mb-3">
                <div className="flex h-2.5 overflow-hidden rounded-full bg-line" role="img" aria-label="De qué está llena esta carpeta">
                  {shares.map((k) => (
                    <button
                      key={k.id}
                      onClick={() => showKind(k.id)}
                      title={`${kindLabel(k.id)} · ${bytes(k.size)} (${percent(k.size, folder.size)})`}
                      aria-label={`${kindLabel(k.id)}, ${bytes(k.size)}`}
                      className="h-full transition-[filter] hover:brightness-125"
                      style={{ width: `${k.share * 100}%`, background: kindColor(k.id) }}
                    />
                  ))}
                </div>
                <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-1">
                  {shares
                    .filter((k) => k.share >= 0.01)
                    .map((k) => (
                      <button
                        key={k.id}
                        onClick={() => showKind(k.id)}
                        className={`flex items-center gap-1.5 text-[11px] hover:text-ink ${fileTab === "kind" && kind === k.id ? "text-ink" : "text-dim"}`}
                        title="Ver los archivos más grandes de este tipo"
                      >
                        <span className="size-2 rounded-sm" style={{ background: kindColor(k.id) }} />
                        {kindLabel(k.id)} <span className="font-mono text-mute">{bytes(k.size)}</span>
                      </button>
                    ))}
                </div>
              </div>
            )}

            {folderView === "map" ? (
              <SpaceMap
                entries={entries}
                loose={loose}
                focused={focused}
                picked={picked}
                onFocus={setFocused}
                onOpen={(e) => e.hasChildren && void openFolder(e.path)}
                onLoose={() => setFileTab("here")}
              />
            ) : (
              <ul className="max-pane-lg space-y-0.5 overflow-y-auto">
                {entries.map((e) => (
                  <li
                    key={e.path}
                    onClick={() => setFocused(e.path)}
                    onDoubleClick={() => e.hasChildren && void openFolder(e.path)}
                    className={`group cursor-default rounded-md px-2 py-1.5 ${focused === e.path ? "bg-neon/10" : "hover:bg-panel-2"}`}
                  >
                    <div className="flex items-center gap-2 text-sm">
                      <input
                        type="checkbox"
                        checked={picked.has(e.path)}
                        onChange={() => toggle(e, true)}
                        onClick={(ev) => ev.stopPropagation()}
                        aria-label={`Marcar ${e.name}`}
                        className="shrink-0 accent-[var(--color-neon)]"
                      />
                      <Folder size={13} className="shrink-0 text-neon/70" />
                      <span className="min-w-0 flex-1 truncate text-ink">{e.name}</span>
                      {e.modified && <span className="hidden shrink-0 text-[11px] text-mute sm:inline">{since(e.modified)}</span>}
                      <span className="w-10 shrink-0 text-right text-[11px] text-mute">{percent(e.size, folder.size)}</span>
                      <span className="w-20 shrink-0 text-right font-mono text-xs tabular">{bytes(e.size)}</span>
                    </div>
                    <div className="mt-1 ml-[22px] h-1 overflow-hidden rounded-full bg-line">
                      <div className="h-full rounded-full bg-neon/70" style={{ width: `${(e.size / biggest) * 100}%` }} />
                    </div>
                  </li>
                ))}
                {loose > 0 && (
                  <li>
                    <button onClick={() => setFileTab("here")} className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-dim hover:bg-panel-2 hover:text-ink">
                      <span className="w-[13px]" />
                      <File size={13} className="shrink-0 text-mute" />
                      <span className="min-w-0 flex-1 truncate">Archivos sueltos y carpetas pequeñas</span>
                      <span className="w-10 shrink-0 text-right text-[11px] text-mute">{percent(loose, folder.size)}</span>
                      <span className="w-20 shrink-0 text-right font-mono text-xs tabular">{bytes(loose)}</span>
                    </button>
                  </li>
                )}
                {entries.length === 0 && loose === 0 && <li className="px-2 text-sm text-mute">Carpeta vacía.</li>}
              </ul>
            )}

            {focusedEntry ? (
              <div className="mt-3 rounded-lg border border-line-2 bg-panel-2/40 px-3.5 py-2.5">
                <div className="flex flex-wrap items-baseline gap-x-2">
                  <span className="min-w-0 truncate text-sm font-medium text-ink">{focusedEntry.name}</span>
                  <span className="ml-auto font-mono text-sm text-neon">{bytes(focusedEntry.size)}</span>
                </div>
                <p className="mt-0.5 text-[11px] text-mute">
                  {percent(focusedEntry.size, folder.size)} de esta carpeta · {percent(focusedEntry.size, scan.size)} de lo analizado · {focusedEntry.files.toLocaleString("es")} archivos
                  {focusedEntry.modified ? ` · último cambio ${since(focusedEntry.modified)}` : ""}
                </p>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {focusedEntry.hasChildren && (
                    <button onClick={() => void openFolder(focusedEntry.path)} className={smallBtn}>
                      <CornerDownRight size={12} /> Entrar
                    </button>
                  )}
                  <button onClick={() => reveal(focusedEntry.path)} className={smallBtn}>
                    <FolderOpen size={12} /> Ver en el Explorador
                  </button>
                  <button onClick={() => copyPath(focusedEntry.path)} className={smallBtn}>
                    <Copy size={12} /> Copiar ruta
                  </button>
                  <button onClick={() => toggle(focusedEntry, true)} className={smallBtn}>
                    {picked.has(focusedEntry.path) ? (
                      <>
                        <Check size={12} /> Marcada · quitar
                      </>
                    ) : (
                      <>
                        <Recycle size={12} /> Marcar para la papelera
                      </>
                    )}
                  </button>
                </div>
              </div>
            ) : (
              <p className="mt-3 text-[11px] text-mute">Un clic señala una carpeta y enseña sus datos; doble clic entra en ella.</p>
            )}
          </Card>

          <Card title="Archivos" icon={<File size={14} />} className="col-span-12 lg:col-span-5">
            <div className="mb-2 flex flex-wrap gap-1 text-[11px]" role="tablist">
              {(
                [
                  ["big", "Más grandes"],
                  ["old", `Sin usar (${scan.oldFiles.length})`],
                  ["here", "De esta carpeta"],
                  ...(kind ? ([["kind", kindLabel(kind)]] as const) : []),
                ] as [FileTab, string][]
              ).map(([id, label]) => (
                <button
                  key={id}
                  role="tab"
                  aria-selected={fileTab === id}
                  onClick={() => setFileTab(id)}
                  className={`flex items-center gap-1.5 rounded px-2 py-0.5 transition-colors ${fileTab === id ? "bg-neon/10 text-neon" : "text-mute hover:text-ink"}`}
                >
                  {id === "kind" && kind && <span className="size-2 rounded-sm" style={{ background: kindColor(kind) }} />}
                  {label}
                </button>
              ))}
            </div>
            <p className="mb-2 flex items-start gap-1.5 text-[11px] text-mute">
              {fileTab === "old" && (
                <>
                  <CalendarClock size={11} className="mt-0.5 shrink-0" /> Grandes y sin cambios en más de un año: lo primero que se puede ofrecer borrar.
                </>
              )}
              {fileTab === "big" && "Los archivos que más ocupan en todo lo analizado."}
              {fileTab === "here" && `Los archivos sueltos de «${crumbs[crumbs.length - 1]?.name ?? ""}», de mayor a menor.`}
              {fileTab === "kind" && "Los más grandes de este tipo en todo lo analizado (de 1 MB en adelante)."}
            </p>
            <div className="mb-2 flex items-center gap-2">
              <input
                type="checkbox"
                checked={allPicked}
                onChange={pickAll}
                disabled={!files.length}
                aria-label="Marcar todos los de la lista"
                title="Marcar todos los de la lista"
                className="ml-1.5 shrink-0 accent-[var(--color-neon)]"
              />
              <label className="flex min-w-0 flex-1 items-center gap-1.5 rounded-md border border-line bg-void/60 px-2 py-1">
                <Search size={12} className="shrink-0 text-mute" />
                <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Buscar por nombre o extensión" className="min-w-0 flex-1 bg-transparent text-xs text-ink outline-none" />
              </label>
            </div>
            {tabFiles === null ? (
              <p className="flex items-center gap-2 py-6 text-sm text-mute">
                <Loader2 size={13} className="animate-spin" /> Leyendo…
              </p>
            ) : files.length === 0 ? (
              <p className="py-6 text-center text-sm text-mute">
                {query
                  ? "Nada con ese nombre en esta lista."
                  : fileTab === "old"
                    ? "No hay archivos grandes sin usar en más de un año."
                    : fileTab === "here"
                      ? "Esta carpeta no tiene archivos sueltos: todo está en sus subcarpetas."
                      : "Sin archivos que destaquen."}
              </p>
            ) : (
              <ul className="max-pane-lg space-y-0.5 overflow-y-auto">
                {files.map((f) => (
                  <li key={f.path} className={`group flex items-center gap-2 rounded px-1.5 py-1 text-xs hover:bg-panel-2 ${picked.has(f.path) ? "bg-neon/5" : ""}`}>
                    <input type="checkbox" checked={picked.has(f.path)} onChange={() => toggle(f, false)} aria-label={`Marcar ${f.name}`} className="shrink-0 accent-[var(--color-neon)]" />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-dim" title={f.name}>
                        {f.name}
                      </span>
                      <span className="block truncate text-[10.5px] text-mute">
                        {fileTab !== "here" && `${friendlyPath(parentOf(f.path))}${f.modified ? " · " : ""}`}
                        {since(f.modified)}
                      </span>
                    </span>
                    <span className="shrink-0 font-mono tabular text-ink">{bytes(f.size)}</span>
                    {fileTab !== "here" && (
                      <button
                        onClick={() => {
                          setFileTab("here");
                          void openFolder(parentOf(f.path));
                        }}
                        className="text-mute opacity-0 group-hover:opacity-100 hover:text-ink focus:opacity-100"
                        title="Ir a su carpeta en el mapa"
                        aria-label="Ir a su carpeta en el mapa"
                      >
                        <CornerDownRight size={12} />
                      </button>
                    )}
                    <button onClick={() => reveal(f.path)} className="text-mute opacity-0 group-hover:opacity-100 hover:text-ink focus:opacity-100" title="Ver en el Explorador" aria-label="Ver en el Explorador">
                      <FolderOpen size={12} />
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </Card>

          {picked.size > 0 && (
            <div className="sticky bottom-3 z-10 col-span-12 flex flex-wrap items-center gap-3 rounded-xl border border-line-2 bg-panel px-4 py-2.5 shadow-2xl">
              <Recycle size={14} className="text-mute" />
              <span className="text-sm text-dim">
                {picked.size} {picked.size === 1 ? "marcado" : "marcados"} · <span className="font-mono text-ink">{bytes(pickedSize)}</span>
              </span>
              <span className="min-w-0 flex-1 truncate text-[11px] text-mute">
                {[...picked.values()]
                  .slice(0, 4)
                  .map((p) => p.name)
                  .join(" · ")}
                {picked.size > 4 ? " …" : ""}
              </span>
              <button onClick={() => setPicked(new Map())} className="text-xs text-mute hover:text-ink">
                Quitar marcas
              </button>
              <Button kind="danger" onClick={recycle} disabled={recycling}>
                {recycling ? <Loader2 size={13} className="animate-spin" /> : <Recycle size={13} />} A la papelera
              </Button>
            </div>
          )}
        </>
      )}

      <Card
        title={`Qué puedes liberar${freeable?.length ? ` · ${bytes(freeable.reduce((n, f) => n + f.size, 0))}` : ""}`}
        icon={<Sparkles size={14} />}
        className="col-span-12"
        right={
          onNavigate && (
            <button onClick={() => onNavigate("tweaks", "cleanup")} className="flex items-center gap-1 text-[11px] text-mute hover:text-neon">
              Ir a Limpieza <ChevronRight size={11} />
            </button>
          )
        }
      >
        {freeable === null ? (
          <p className="flex items-center gap-2 text-sm text-mute">
            <Loader2 size={13} className="animate-spin" /> Midiendo dónde se acumula el espacio…
          </p>
        ) : freeable.length === 0 ? (
          <p className="text-sm text-mute">No se encontró espacio recuperable en los sitios de siempre.</p>
        ) : (
          <ul className="grid gap-2 md:grid-cols-2">
            {freeable.map((f) => (
              <li key={f.key} className="flex items-start gap-3 rounded-lg border border-line px-3 py-2">
                <span className={`mt-1 size-2 shrink-0 rounded-full ${f.safe ? "bg-ok" : "bg-warn"}`} />
                <div className="min-w-0 flex-1">
                  <div className="flex items-baseline gap-2">
                    <span className="truncate text-sm text-ink">{f.name}</span>
                    <span className="ml-auto shrink-0 font-mono text-sm text-neon">{bytes(f.size)}</span>
                  </div>
                  <p className="text-[11px] text-mute">{f.detail}</p>
                </div>
              </li>
            ))}
          </ul>
        )}
        <p className="mt-3 flex items-start gap-1.5 text-[11px] text-mute">
          <TriangleAlert size={12} className="mt-0.5 shrink-0 text-warn" />
          Verde: se puede borrar sin pensarlo. Naranja: míralo antes, puede haber cosas del usuario.
        </p>
      </Card>
      {dialog}
    </div>
  );
}

/**
 * Las carpetas como rectángulos proporcionales a lo que ocupan. Un clic señala
 * (su ficha sale debajo); doble clic entra. Lo que no es una subcarpeta con
 * nombre va en un rectángulo gris que lleva a los archivos de la carpeta.
 */
function SpaceMap({
  entries,
  loose,
  focused,
  picked,
  onFocus,
  onOpen,
  onLoose,
}: {
  entries: SpaceEntry[];
  loose: number;
  focused: string | null;
  picked: Picked;
  onFocus: (path: string) => void;
  onOpen: (e: SpaceEntry) => void;
  onLoose: () => void;
}) {
  const W = 100;
  const H = 62;
  const items = useMemo(
    () => (loose > 0 ? [...entries, { name: "Archivos sueltos y carpetas pequeñas", path: LOOSE, size: loose, files: 0, hasChildren: false } as SpaceEntry] : entries),
    [entries, loose],
  );
  const total = items.reduce((a, e) => a + e.size, 0);
  const placed = useMemo(() => squarify(items, (e) => e.size, { x: 0, y: 0, w: W, h: H }), [items]);
  if (!placed.length) return <p className="text-sm text-mute">Carpeta vacía.</p>;
  return (
    <div className="relative w-full overflow-hidden rounded-lg border border-line" style={{ aspectRatio: `${W} / ${H}` }}>
      {placed.map((p) => {
        const share = p.item.size / total;
        const big = p.w > 12 && p.h > 7;
        const isLoose = p.item.path === LOOSE;
        const marked = picked.has(p.item.path);
        return (
          <button
            key={p.item.path}
            onClick={() => (isLoose ? onLoose() : onFocus(p.item.path))}
            onDoubleClick={() => !isLoose && onOpen(p.item)}
            aria-label={`${p.item.name}, ${bytes(p.item.size)}`}
            aria-pressed={focused === p.item.path}
            title={`${p.item.name} · ${bytes(p.item.size)} (${Math.round(share * 100)} %)${isLoose ? " · clic para ver los archivos" : p.item.hasChildren ? " · doble clic para entrar" : ""}`}
            className={`absolute overflow-hidden border text-left transition-[filter] hover:brightness-125 ${focused === p.item.path ? "z-[1] border-ink" : "border-void"}`}
            style={{
              left: `${(p.x / W) * 100}%`,
              top: `${(p.y / H) * 100}%`,
              width: `${(p.w / W) * 100}%`,
              height: `${(p.h / H) * 100}%`,
              // Los más grandes, más intensos: se ve de un vistazo dónde se va el disco.
              background: isLoose
                ? "repeating-linear-gradient(45deg, var(--color-panel-2), var(--color-panel-2) 6px, var(--color-line) 6px, var(--color-line) 12px)"
                : `color-mix(in srgb, var(--color-neon) ${Math.round(18 + 55 * Math.sqrt(share))}%, var(--color-panel-2))`,
            }}
          >
            {big && (
              <span className="block p-1.5 leading-tight">
                <span className="block truncate text-[12px] font-medium text-ink">{p.item.name}</span>
                <span className="block font-mono text-[10.5px] text-ink/80">{bytes(p.item.size)}</span>
              </span>
            )}
            {marked && (
              <span className="absolute top-1 right-1 grid size-4 place-items-center rounded-full bg-bad text-white">
                <Recycle size={10} />
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
