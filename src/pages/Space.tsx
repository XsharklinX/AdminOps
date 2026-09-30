import { listen } from "@tauri-apps/api/event";
import { CalendarClock, ChevronRight, File, Folder, FolderOpen, HardDrive, Loader2, Recycle, ScanSearch, Sparkles, Square, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card } from "../components/ui";
import { api, toolsApi, type Freeable, type SpaceEntry, type SpaceView } from "../lib/api";
import { bytes } from "../lib/format";
import type { PageId } from "../components/Sidebar";


/** Hace cuánto se tocó por última vez, en palabras. */
function since(epoch: number | undefined): string {
  if (!epoch) return "";
  const days = Math.floor(Date.now() / 1000 - epoch) / 86_400;
  if (days < 60) return `hace ${Math.max(1, Math.round(days))} días`;
  if (days < 730) return `hace ${Math.round(days / 30)} meses`;
  return `hace ${(days / 365).toFixed(0)} años`;
}

export function Space({ onNavigate }: { onNavigate?: (p: PageId, focus?: string | null) => void }) {
  const [drives, setDrives] = useState<{ mount: string; free: number; total: number }[]>([]);
  const [path, setPath] = useState("C:\\");
  const [scan, setScan] = useState<SpaceView | null>(null);
  const [crumbs, setCrumbs] = useState<{ name: string; path: string }[]>([]);
  const [entries, setEntries] = useState<SpaceEntry[]>([]);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<{ files: number; bytes: number } | null>(null);
  const [freeable, setFreeable] = useState<Freeable[] | null>(null);
  const [fileTab, setFileTab] = useState<"big" | "old">("big");
  const [picked, setPicked] = useState<Set<string>>(new Set());
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
  useEffect(() => {
    loadFreeable();
  }, [loadFreeable]);

  useEffect(() => {
    void api.liveMetrics().then((m) => setDrives(m.disks.map((d) => ({ mount: d.mount, free: d.available, total: d.total }))));
    const un = listen<{ files: number; bytes: number }>("space-progress", (e) => setProgress(e.payload));
    return () => {
      void un.then((f) => f());
    };
  }, []);

  const start = async (target = path) => {
    setRunning(true);
    setProgress(null);
    try {
      const v = await toolsApi.scanSpace(target);
      setScan(v);
      setPicked(new Set());
      setEntries(v.children);
      setCrumbs([{ name: v.root, path: v.root }]);
    } catch (e) {
      toast(String(e).includes("cancelado") ? "info" : "error", String(e));
    } finally {
      setRunning(false);
    }
  };

  const open = async (e: SpaceEntry) => {
    if (!e.hasChildren) return;
    try {
      setEntries(await toolsApi.spaceChildren(e.path));
      setCrumbs((c) => [...c, { name: e.name, path: e.path }]);
    } catch (err) {
      toast("error", String(err));
    }
  };

  const goTo = async (i: number) => {
    const target = crumbs[i];
    try {
      setEntries(i === 0 && scan ? scan.children : await toolsApi.spaceChildren(target.path));
      setCrumbs((c) => c.slice(0, i + 1));
    } catch (err) {
      toast("error", String(err));
    }
  };

  const reveal = (p: string) => toolsApi.revealInExplorer(p).catch((e) => toast("error", String(e)));

  const files = fileTab === "big" ? scan?.largestFiles ?? [] : scan?.oldFiles ?? [];
  const pick = (path: string) =>
    setPicked((s) => {
      const n = new Set(s);
      if (n.has(path)) n.delete(path);
      else n.add(path);
      return n;
    });
  const pickedSize = files.filter((f) => picked.has(f.path)).reduce((n, f) => n + f.size, 0);

  const recycle = async () => {
    const list = [...picked];
    const ok = await confirm({
      title: `¿Enviar ${list.length} ${list.length === 1 ? "archivo" : "archivos"} a la papelera?`,
      body: (
        <p>
          Se liberarán <span className="font-mono text-ink">{bytes(pickedSize)}</span>. Van a la papelera de Windows, así que se pueden recuperar hasta que se
          vacíe.
        </p>
      ),
      confirmLabel: "Enviar a la papelera",
      danger: true,
    });
    if (!ok) return;
    setRecycling(true);
    try {
      const freed = await toolsApi.spaceRecycle(list);
      toast("ok", `${list.length} elemento(s) a la papelera · ${bytes(freed)} liberados.`);
      setPicked(new Set());
      loadFreeable();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRecycling(false);
    }
  };
  const parentSize = entries.length ? Math.max(...entries.map((e) => e.size)) : 1;

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <Card title="Qué analizar" icon={<HardDrive size={14} />} className="col-span-12">
        <div className="flex flex-wrap items-center gap-2">
          {drives.map((d) => (
            <button
              key={d.mount}
              onClick={() => setPath(d.mount)}
              className={`rounded-lg border px-3 py-1.5 text-left text-xs ${path === d.mount ? "border-neon/60 bg-neon/10 text-neon" : "border-line-2 text-dim hover:text-ink"}`}
            >
              <div className="font-mono text-sm">{d.mount}</div>
              <div className="text-[11px] text-mute">
                {bytes(d.free)} libres de {bytes(d.total)}
              </div>
            </button>
          ))}
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            className="ml-2 min-w-64 flex-1 rounded-md border border-line bg-void/60 px-3 py-2 font-mono text-xs text-ink outline-none focus:border-neon/50"
            placeholder="C:\Users"
          />
          {running ? (
            <button onClick={() => toolsApi.cancelSpaceScan()} className="flex items-center gap-1.5 rounded-md border border-bad/50 px-4 py-2 text-sm text-bad hover:bg-bad/10">
              <Square size={12} fill="currentColor" /> Detener
            </button>
          ) : (
            <button
              onClick={() => start()}
              className="flex items-center gap-1.5 rounded-md border border-neon/50 bg-neon/10 px-4 py-2 text-sm font-medium text-neon hover:bg-neon/20"
            >
              <ScanSearch size={14} /> Analizar
            </button>
          )}
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

      {scan && (
        <>
          <Card title="Carpetas" icon={<Folder size={14} />} className="col-span-12 lg:col-span-7">
            <div className="mb-3 flex flex-wrap items-center gap-1 text-xs">
              {crumbs.map((c, i) => (
                <span key={c.path} className="flex items-center gap-1">
                  {i > 0 && <ChevronRight size={11} className="text-mute" />}
                  <button onClick={() => goTo(i)} className={i === crumbs.length - 1 ? "text-neon" : "text-dim hover:text-ink"}>
                    {c.name}
                  </button>
                </span>
              ))}
            </div>
            <ul className="space-y-1">
              {entries.map((e) => (
                <li key={e.path} className="group rounded-md px-2 py-1.5 hover:bg-panel-2">
                  <div className="flex items-center gap-2 text-sm">
                    <button onClick={() => open(e)} className={`flex min-w-0 flex-1 items-center gap-2 text-left ${e.hasChildren ? "text-ink hover:text-neon" : "cursor-default text-dim"}`}>
                      <Folder size={13} className="shrink-0 text-neon/70" />
                      <span className="truncate">{e.name}</span>
                    </button>
                    <span className="font-mono text-xs tabular">{bytes(e.size)}</span>
                    <button onClick={() => reveal(e.path)} className="text-mute opacity-0 group-hover:opacity-100 hover:text-ink" title="Abrir en el Explorador">
                      <FolderOpen size={13} />
                    </button>
                  </div>
                  <div className="mt-1 h-1 overflow-hidden rounded-full bg-line">
                    <div className="h-full rounded-full bg-neon/70" style={{ width: `${(e.size / parentSize) * 100}%` }} />
                  </div>
                </li>
              ))}
              {entries.length === 0 && <li className="text-sm text-mute">Sin subcarpetas de más de 1 MB.</li>}
            </ul>
          </Card>

          <Card
            title="Archivos"
            icon={<File size={14} />}
            className="col-span-12 lg:col-span-5"
            right={
              <div className="flex gap-1 text-[11px]">
                {(
                  [
                    ["big", "Más grandes"],
                    ["old", `Sin usar (${scan.oldFiles.length})`],
                  ] as const
                ).map(([id, label]) => (
                  <button
                    key={id}
                    onClick={() => setFileTab(id)}
                    className={`rounded px-2 py-0.5 transition-colors ${fileTab === id ? "bg-neon/10 text-neon" : "text-mute hover:text-ink"}`}
                  >
                    {label}
                  </button>
                ))}
              </div>
            }
          >
            {fileTab === "old" && (
              <p className="mb-2 flex items-center gap-1.5 text-[11px] text-mute">
                <CalendarClock size={11} /> Grandes y sin abrirse en más de un año: lo primero que se puede ofrecer borrar.
              </p>
            )}
            {files.length === 0 ? (
              <p className="py-6 text-center text-sm text-mute">
                {fileTab === "old" ? "No hay archivos grandes sin usar en más de un año." : "Sin archivos que destaquen."}
              </p>
            ) : (
              <ul className="max-pane-lg space-y-0.5 overflow-y-auto">
                {files.map((f) => (
                  <li key={f.path} className={`group flex items-center gap-2 rounded px-1.5 py-1 text-xs hover:bg-panel-2 ${picked.has(f.path) ? "bg-neon/5" : ""}`}>
                    <input type="checkbox" checked={picked.has(f.path)} onChange={() => pick(f.path)} className="shrink-0 accent-[var(--color-neon)]" />
                    <span className="min-w-0 flex-1 truncate text-dim" title={f.name}>
                      {f.name}
                      {f.modified && <span className="ml-1.5 text-mute">· {since(f.modified)}</span>}
                    </span>
                    <span className="font-mono tabular text-ink">{bytes(f.size)}</span>
                    <button onClick={() => reveal(f.path)} className="text-mute opacity-0 group-hover:opacity-100 hover:text-ink" title="Abrir en el Explorador">
                      <FolderOpen size={12} />
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {picked.size > 0 && (
              <div className="mt-3 flex items-center gap-3 border-t border-line pt-3">
                <span className="text-xs text-dim">
                  {picked.size} marcado(s) · <span className="font-mono text-ink">{bytes(pickedSize)}</span>
                </span>
                <button onClick={() => setPicked(new Set())} className="text-[11px] text-mute hover:text-ink">
                  Quitar
                </button>
                <div className="ml-auto">
                  <Button kind="danger" onClick={recycle} disabled={recycling}>
                    {recycling ? <Loader2 size={13} className="animate-spin" /> : <Recycle size={13} />} A la papelera
                  </Button>
                </div>
              </div>
            )}
          </Card>
        </>
      )}
      {dialog}
    </div>
  );
}
