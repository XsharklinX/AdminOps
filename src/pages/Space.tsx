import { listen } from "@tauri-apps/api/event";
import { ChevronRight, File, Folder, FolderOpen, HardDrive, Loader2, ScanSearch, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { Card } from "../components/ui";
import { api, toolsApi, type SpaceEntry, type SpaceView } from "../lib/api";
import { bytes } from "../lib/format";

export function Space() {
  const [drives, setDrives] = useState<{ mount: string; free: number; total: number }[]>([]);
  const [path, setPath] = useState("C:\\");
  const [scan, setScan] = useState<SpaceView | null>(null);
  const [crumbs, setCrumbs] = useState<{ name: string; path: string }[]>([]);
  const [entries, setEntries] = useState<SpaceEntry[]>([]);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<{ files: number; bytes: number } | null>(null);
  const toast = useToast();

  useEffect(() => {
    api.liveMetrics().then((m) => setDrives(m.disks.map((d) => ({ mount: d.mount, free: d.available, total: d.total }))));
    const un = listen<{ files: number; bytes: number }>("space-progress", (e) => setProgress(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);

  const start = async (target = path) => {
    setRunning(true);
    setProgress(null);
    try {
      const v = await toolsApi.scanSpace(target);
      setScan(v);
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

          <Card title="Archivos más grandes" icon={<File size={14} />} className="col-span-12 lg:col-span-5">
            <ul className="max-h-[560px] space-y-0.5 overflow-y-auto">
              {scan.largestFiles.map((f) => (
                <li key={f.path} className="group flex items-center gap-2 rounded px-1.5 py-1 text-xs hover:bg-panel-2" title={f.path}>
                  <span className="min-w-0 flex-1 truncate text-dim">{f.name}</span>
                  <span className="font-mono tabular text-ink">{bytes(f.size)}</span>
                  <button onClick={() => reveal(f.path)} className="text-mute opacity-0 group-hover:opacity-100 hover:text-ink">
                    <FolderOpen size={12} />
                  </button>
                </li>
              ))}
            </ul>
          </Card>
        </>
      )}
    </div>
  );
}
