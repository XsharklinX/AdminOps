import { usePageActive } from "../lib/pageActive";
import { ArrowDown, ArrowUp, FolderOpen, Lock, Pause, Play, Search, ShieldAlert, Skull, TriangleAlert, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { toolsApi, type ProcessView } from "../lib/api";
import { bytes } from "../lib/format";

type SortKey = "name" | "pid" | "cpu" | "memory" | "disk";

const rate = (b: number) => (b < 1024 ? "0 B/s" : `${bytes(b)}/s`);

export function Processes({ isAdmin }: { isAdmin: boolean }) {
  const active = usePageActive();
  const [procs, setProcs] = useState<ProcessView[] | null>(null);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({ key: "cpu", desc: true });
  const [selected, setSelected] = useState<number | null>(null);
  const [paused, setPaused] = useState(false);
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const inflight = useRef(false);

  const load = useCallback(async () => {
    if (inflight.current || document.hidden) return;
    inflight.current = true;
    try {
      setProcs(await toolsApi.processes());
    } catch (e) {
      toast("error", String(e));
    } finally {
      inflight.current = false;
    }
  }, [toast]);

  useEffect(() => {
    load();
    if (paused || !active) return;
    const t = window.setInterval(load, 2000);
    return () => window.clearInterval(t);
  }, [load, paused, active]);

  const list = useMemo(() => {
    const q = query.trim().toLowerCase();
    const v = (procs ?? []).filter(
      (p) => !q || p.name.toLowerCase().includes(q) || String(p.pid) === q || p.exe?.toLowerCase().includes(q) || p.user?.toLowerCase().includes(q),
    );
    const val = (p: ProcessView) =>
      sort.key === "name" ? p.name.toLowerCase() : sort.key === "disk" ? p.diskReadPerSec + p.diskWritePerSec : p[sort.key];
    v.sort((a, b) => {
      const x = val(a);
      const y = val(b);
      const r = x < y ? -1 : x > y ? 1 : 0;
      return sort.desc ? -r : r;
    });
    return v;
  }, [procs, query, sort]);

  const sel = procs?.find((p) => p.pid === selected) ?? null;
  const children = sel ? (procs ?? []).filter((p) => p.parent === sel.pid).length : 0;

  const kill = async (p: ProcessView, tree: boolean) => {
    const ok = await confirm({
      title: `¿Finalizar ${tree ? "el árbol de " : ""}${p.name}?`,
      danger: true,
      confirmLabel: tree ? "Finalizar árbol" : "Finalizar",
      body: (
        <>
          <p className="mb-2">
            PID <span className="font-mono text-ink">{p.pid}</span>
            {tree && children > 0 && <> y sus {children} procesos hijos</>}. Se perderá lo que no esté guardado en ese programa.
          </p>
          {p.note && (
            <p className="flex gap-1.5 text-warn">
              <TriangleAlert size={14} className="mt-0.5 shrink-0" /> {p.note}
            </p>
          )}
        </>
      ),
    });
    if (!ok) return;
    setBusy(true);
    try {
      await toolsApi.killProcess(p.pid, p.name, tree);
      toast("ok", `${p.name} finalizado.`);
      setSelected(null);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
      load();
    }
  };

  const header = (key: SortKey, label: string, right = false) => (
    <th
      onClick={() => setSort((s) => ({ key, desc: s.key === key ? !s.desc : key !== "name" }))}
      className={`cursor-pointer px-3 py-2 font-medium select-none hover:text-ink ${right ? "text-right" : ""}`}
    >
      <span className={`inline-flex items-center gap-1 ${sort.key === key ? "text-neon" : ""}`}>
        {label}
        {sort.key === key && (sort.desc ? <ArrowDown size={10} /> : <ArrowUp size={10} />)}
      </span>
    </th>
  );

  const totalCpu = (procs ?? []).reduce((a, p) => a + p.cpu, 0);
  const totalMem = (procs ?? []).reduce((a, p) => a + p.memory, 0);

  return (
    <div className="flex h-full flex-col p-6">
      <div className="mb-3 flex items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{procs?.length ?? "…"}</span> procesos ·{" "}
          <span className="font-mono">{totalCpu.toFixed(0)}%</span> CPU · <span className="font-mono">{bytes(totalMem)}</span> RAM
        </p>
        {!isAdmin && (
          <span className="flex items-center gap-1 text-xs text-warn">
            <ShieldAlert size={12} /> Sin administrador solo puedes finalizar tus propios procesos.
          </span>
        )}
        <div className="relative ml-auto w-72">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar nombre, PID, ruta o usuario…"
            className="w-full rounded-md border border-line bg-panel py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <button
          onClick={() => setPaused(!paused)}
          className="flex items-center gap-1.5 rounded-md border border-line-2 px-2.5 py-1.5 text-xs text-dim hover:text-ink"
          title="Pausar la actualización para seleccionar con calma"
        >
          {paused ? <Play size={12} /> : <Pause size={12} />} {paused ? "Reanudar" : "Pausar"}
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-auto rounded-xl border border-line bg-panel">
        <table className="w-full text-sm">
          <thead className="sticky top-0 z-10 bg-panel-2 text-left text-[11px] text-mute">
            <tr>
              {header("name", "Proceso")}
              {header("pid", "PID", true)}
              <th className="px-3 py-2 font-medium">Usuario</th>
              {header("cpu", "CPU", true)}
              {header("memory", "Memoria", true)}
              {header("disk", "Disco", true)}
            </tr>
          </thead>
          <tbody className="font-mono text-xs tabular">
            {list.map((p) => (
              <tr
                key={p.pid}
                onClick={() => setSelected(p.pid)}
                className={`cursor-pointer border-t border-line/60 ${selected === p.pid ? "bg-neon/10" : "hover:bg-panel-2"}`}
              >
                <td className="max-w-0 truncate px-3 py-1.5 font-sans text-[13px] text-ink" title={p.exe ?? undefined}>
                  <span className="inline-flex items-center gap-1.5">
                    {p.protection === "critical" && <Lock size={11} className="text-bad" />}
                    {p.protection === "sensitive" && <TriangleAlert size={11} className="text-warn" />}
                    {p.name}
                  </span>
                </td>
                <td className="px-3 py-1.5 text-right text-mute">{p.pid}</td>
                <td className="max-w-32 truncate px-3 py-1.5 font-sans text-dim">{p.user ?? "—"}</td>
                <td className={`px-3 py-1.5 text-right ${p.cpu >= 10 ? "text-warn" : ""}`}>{p.cpu.toFixed(1)}%</td>
                <td className="px-3 py-1.5 text-right">{bytes(p.memory)}</td>
                <td className="px-3 py-1.5 text-right text-dim">{rate(p.diskReadPerSec + p.diskWritePerSec)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {sel && (
        <div className="mt-3 flex items-center gap-3 rounded-xl border border-line-2 bg-panel px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-2">
              <span className="font-medium text-ink">{sel.name}</span>
              <span className="font-mono text-xs text-mute">PID {sel.pid}</span>
              {children > 0 && <span className="text-xs text-dim">{children} procesos hijos</span>}
            </div>
            <div className="truncate font-mono text-[11px] text-mute select-text">{sel.exe ?? "Ruta no disponible"}</div>
            {sel.note && <div className={`text-xs ${sel.protection === "critical" ? "text-bad" : sel.protection === "self" ? "text-dim" : "text-warn"}`}>{sel.note}</div>}
          </div>
          <button
            onClick={() => toolsApi.openProcessLocation(sel.pid).catch((e) => toast("error", String(e)))}
            className="flex items-center gap-1.5 rounded-md border border-line-2 px-3 py-1.5 text-xs text-dim hover:text-ink"
          >
            <FolderOpen size={13} /> Abrir ubicación
          </button>
          {sel.protection !== "critical" && sel.protection !== "self" && (
            <>
              <button
                onClick={() => kill(sel, true)}
                disabled={busy}
                className="flex items-center gap-1.5 rounded-md border border-bad/40 px-3 py-1.5 text-xs text-bad hover:bg-bad/10 disabled:opacity-40"
                title="Finaliza el proceso y todos los que ha abierto"
              >
                <Skull size={13} /> Finalizar árbol
              </button>
              <button
                onClick={() => kill(sel, false)}
                disabled={busy}
                className="flex items-center gap-1.5 rounded-md border border-bad/60 bg-bad/10 px-3 py-1.5 text-xs font-medium text-bad hover:bg-bad/20 disabled:opacity-40"
              >
                <XCircle size={13} /> Finalizar tarea
              </button>
            </>
          )}
        </div>
      )}
      {dialog}
    </div>
  );
}
