import { usePageActive } from "../lib/pageActive";
import { ArrowDown, ArrowUp, ChevronRight, Cpu, FolderOpen, Layers, Lock, MemoryStick, Pause, Play, Search, ShieldAlert, Skull, TriangleAlert, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { EmptyLine, ErrorState, Loading } from "../components/ui";
import { toolsApi, type ProcessView } from "../lib/api";
import { bytes } from "../lib/format";
import { groupProcesses, isSystemProcess, killable, type ProcessGroup } from "../lib/processGroups";

type SortKey = "name" | "pid" | "cpu" | "memory" | "disk";
type Scope = "all" | "people" | "system";
type Selection = { kind: "proc"; pid: number } | { kind: "group"; key: string } | null;

const rate = (b: number) => (b < 1024 ? "0 B/s" : `${bytes(b)}/s`);
const GROUPED_KEY = "adminops.processes.grouped";

/** Cifra con una barra de fondo: se ve de un vistazo quién pesa más. */
function Meter({ pct, warn, children }: { pct: number; warn?: boolean; children: React.ReactNode }) {
  return (
    <td className="relative px-3 py-1.5 text-right">
      <span className={`absolute inset-y-1 right-1 rounded-sm ${warn ? "bg-warn/20" : "bg-neon/10"}`} style={{ width: `${Math.min(100, Math.max(0, pct)) * 0.9}%` }} />
      <span className={`relative ${warn ? "text-warn" : ""}`}>{children}</span>
    </td>
  );
}

function Shield({ protection }: { protection: ProcessView["protection"] }) {
  if (protection === "critical") return <Lock size={11} className="shrink-0 text-bad" />;
  if (protection === "sensitive") return <TriangleAlert size={11} className="shrink-0 text-warn" />;
  return null;
}

export function Processes({ isAdmin }: { isAdmin: boolean }) {
  const active = usePageActive();
  const [procs, setProcs] = useState<ProcessView[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({ key: "cpu", desc: true });
  const [scope, setScope] = useState<Scope>("all");
  const [grouped, setGroupedState] = useState(() => {
    try {
      return localStorage.getItem(GROUPED_KEY) !== "0";
    } catch {
      return true;
    }
  });
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<Selection>(null);
  const [paused, setPaused] = useState(false);
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const inflight = useRef(false);

  const setGrouped = (v: boolean) => {
    setGroupedState(v);
    setSelected(null);
    try {
      localStorage.setItem(GROUPED_KEY, v ? "1" : "0");
    } catch {
      /* sin almacenamiento */
    }
  };

  // Se relee cada 2 s: un fallo se enseña en su sitio, no como un aviso que se repite.
  const load = useCallback(async () => {
    if (inflight.current || document.hidden) return;
    inflight.current = true;
    try {
      setProcs(await toolsApi.processes());
      setFailed(null);
    } catch (e) {
      setFailed(String(e));
    } finally {
      inflight.current = false;
    }
  }, []);

  useEffect(() => {
    void load();
    if (paused || !active) return;
    const t = window.setInterval(load, 2000);
    return () => window.clearInterval(t);
  }, [load, paused, active]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (procs ?? []).filter(
      (p) =>
        (scope === "all" || (scope === "system") === isSystemProcess(p)) &&
        (!q || p.name.toLowerCase().includes(q) || String(p.pid) === q || p.exe?.toLowerCase().includes(q) || p.user?.toLowerCase().includes(q)),
    );
  }, [procs, query, scope]);

  const order = useCallback(
    <T,>(items: T[], val: (x: T) => string | number) =>
      [...items].sort((a, b) => {
        const x = val(a);
        const y = val(b);
        const r = x < y ? -1 : x > y ? 1 : 0;
        return sort.desc ? -r : r;
      }),
    [sort.desc],
  );

  const list = useMemo(
    () => order(filtered, (p) => (sort.key === "name" ? p.name.toLowerCase() : sort.key === "disk" ? p.diskReadPerSec + p.diskWritePerSec : p[sort.key])),
    [filtered, order, sort.key],
  );
  const groups = useMemo(
    () => order(groupProcesses(filtered), (g) => (sort.key === "name" ? g.key : sort.key === "pid" ? g.procs.length : g[sort.key])),
    [filtered, order, sort.key],
  );

  /** Lo que más pesa ahora mismo, de todo el equipo (no solo de lo filtrado). */
  const heaviest = useMemo(() => {
    const all = groupProcesses(procs ?? []);
    if (!all.length) return null;
    const cpu = all.reduce((a, b) => (b.cpu > a.cpu ? b : a));
    const memory = all.reduce((a, b) => (b.memory > a.memory ? b : a));
    return { cpu, memory };
  }, [procs]);

  const maxMemory = Math.max(1, ...(grouped ? groups.map((g) => g.memory) : list.map((p) => p.memory)));
  const sel = selected?.kind === "proc" ? (procs?.find((p) => p.pid === selected.pid) ?? null) : null;
  const selGroup = selected?.kind === "group" ? (groups.find((g) => g.key === selected.key) ?? null) : null;
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
      void load();
    }
  };

  /** Cierra el programa entero: todos sus procesos, uno a uno. */
  const killGroup = async (g: ProcessGroup) => {
    const targets = killable(g);
    const ok = await confirm({
      title: `¿Finalizar ${g.name}?`,
      danger: true,
      confirmLabel: targets.length === 1 ? "Finalizar" : `Finalizar los ${targets.length}`,
      body: (
        <p>
          Se cerrarán {targets.length === 1 ? "su proceso" : `sus ${targets.length} procesos`} ({bytes(g.memory)} de memoria). Se perderá lo que no esté guardado en ese programa.
        </p>
      ),
    });
    if (!ok) return;
    setBusy(true);
    let failures = 0;
    let lastError = "";
    for (const p of targets) {
      try {
        await toolsApi.killProcess(p.pid, p.name, false);
      } catch (e) {
        // Un proceso que ya se cerró solo (al caer el principal) no es un fallo.
        const still = (await toolsApi.processes().catch(() => null))?.some((x) => x.pid === p.pid) ?? true;
        if (still) {
          failures++;
          lastError = String(e);
        }
      }
    }
    setBusy(false);
    if (failures) toast("error", `${g.name}: ${failures} de ${targets.length} no se pudieron finalizar. ${lastError}`);
    else toast("ok", `${g.name} finalizado.`);
    setSelected(null);
    void load();
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

  const procRow = (p: ProcessView, nested = false) => (
    <tr
      key={p.pid}
      onClick={() => setSelected({ kind: "proc", pid: p.pid })}
      className={`cursor-pointer border-t border-line/60 ${selected?.kind === "proc" && selected.pid === p.pid ? "bg-neon/10" : nested ? "bg-void/30 hover:bg-panel-2" : "hover:bg-panel-2"}`}
    >
      <td className={`max-w-0 truncate py-1.5 pr-3 font-sans text-[13px] ${nested ? "pl-9 text-dim" : "pl-3 text-ink"}`} title={p.exe ?? undefined}>
        <span className="inline-flex items-center gap-1.5">
          <Shield protection={p.protection} />
          {p.name}
        </span>
      </td>
      <td className="px-3 py-1.5 text-right text-mute">{p.pid}</td>
      <td className="max-w-32 truncate px-3 py-1.5 font-sans text-dim">{p.user ?? "—"}</td>
      <Meter pct={p.cpu} warn={p.cpu >= 10}>
        {p.cpu.toFixed(1)}%
      </Meter>
      <Meter pct={(p.memory / maxMemory) * 100}>{bytes(p.memory)}</Meter>
      <td className="px-3 py-1.5 text-right text-dim">{rate(p.diskReadPerSec + p.diskWritePerSec)}</td>
    </tr>
  );

  const totalCpu = (procs ?? []).reduce((a, p) => a + p.cpu, 0);
  const totalMem = (procs ?? []).reduce((a, p) => a + p.memory, 0);
  const chip = (on: boolean) => `rounded-full border px-2.5 py-0.5 text-xs transition-colors ${on ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`;
  const pick = (g: ProcessGroup) => {
    setQuery("");
    setScope("all");
    setSelected(grouped ? { kind: "group", key: g.key } : { kind: "proc", pid: g.procs[0].pid });
  };

  if (!procs) return failed ? <ErrorState page message={failed} onRetry={() => void load()} /> : <Loading page text="Leyendo los procesos…" />;

  return (
    <div className="flex h-full flex-col p-6">
      <div className="mb-3 flex flex-wrap items-center gap-x-3 gap-y-2">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{procs.length}</span> procesos · <span className="font-mono">{totalCpu.toFixed(0)}%</span> CPU ·{" "}
          <span className="font-mono">{bytes(totalMem)}</span> RAM
        </p>
        {heaviest && (
          <>
            <button onClick={() => pick(heaviest.cpu)} className="flex items-center gap-1.5 rounded-full border border-line px-2.5 py-0.5 text-xs text-dim hover:border-line-2 hover:text-ink" title="Lo que más procesador usa ahora">
              <Cpu size={11} className={heaviest.cpu.cpu >= 25 ? "text-warn" : "text-mute"} /> {heaviest.cpu.name} · {heaviest.cpu.cpu.toFixed(0)}%
            </button>
            <button onClick={() => pick(heaviest.memory)} className="flex items-center gap-1.5 rounded-full border border-line px-2.5 py-0.5 text-xs text-dim hover:border-line-2 hover:text-ink" title="Lo que más memoria usa ahora">
              <MemoryStick size={11} className="text-mute" /> {heaviest.memory.name} · {bytes(heaviest.memory.memory)}
            </button>
          </>
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

      <div className="mb-3 flex flex-wrap items-center gap-2">
        {(
          [
            ["all", "Todos"],
            ["people", "De quien usa el equipo"],
            ["system", "De Windows"],
          ] as const
        ).map(([id, label]) => (
          <button key={id} onClick={() => setScope(id)} className={chip(scope === id)}>
            {label}
          </button>
        ))}
        <button onClick={() => setGrouped(!grouped)} className={`${chip(grouped)} flex items-center gap-1.5`} title="Un programa con muchos procesos cuenta como uno, con la suma de todos">
          <Layers size={11} /> Agrupar por programa
        </button>
        {!isAdmin && (
          <span className="ml-auto flex items-center gap-1 text-xs text-warn">
            <ShieldAlert size={12} /> Sin administrador solo puedes finalizar tus propios procesos.
          </span>
        )}
      </div>

      {failed && (
        <div className="mb-3">
          <ErrorState message={failed} onRetry={() => void load()} />
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto rounded-xl border border-line bg-panel">
        <table className="w-full text-sm">
          <thead className="sticky top-0 z-10 bg-panel-2 text-left text-[11px] text-mute">
            <tr>
              {header("name", grouped ? "Programa" : "Proceso")}
              {header("pid", grouped ? "Procesos" : "PID", true)}
              <th className="px-3 py-2 font-medium">Usuario</th>
              {header("cpu", "CPU", true)}
              {header("memory", "Memoria", true)}
              {header("disk", "Disco", true)}
            </tr>
          </thead>
          <tbody className="font-mono text-xs tabular">
            {grouped
              ? groups.flatMap((g) => {
                  const single = g.procs.length === 1;
                  const expanded = open.has(g.key);
                  const row = (
                    <tr
                      key={g.key}
                      onClick={() => setSelected(single ? { kind: "proc", pid: g.procs[0].pid } : { kind: "group", key: g.key })}
                      className={`cursor-pointer border-t border-line/60 ${
                        (selected?.kind === "group" && selected.key === g.key) || (single && selected?.kind === "proc" && selected.pid === g.procs[0].pid) ? "bg-neon/10" : "hover:bg-panel-2"
                      }`}
                    >
                      <td className="max-w-0 truncate py-1.5 pr-3 pl-1.5 font-sans text-[13px] text-ink" title={single ? (g.procs[0].exe ?? undefined) : undefined}>
                        <span className="inline-flex items-center gap-1">
                          {single ? (
                            <span className="w-5" />
                          ) : (
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                setOpen((cur) => {
                                  const next = new Set(cur);
                                  if (!next.delete(g.key)) next.add(g.key);
                                  return next;
                                });
                              }}
                              className="rounded p-0.5 text-mute hover:bg-line hover:text-ink"
                              title={expanded ? "Ocultar sus procesos" : "Ver sus procesos"}
                            >
                              <ChevronRight size={12} className={`transition-transform ${expanded ? "rotate-90" : ""}`} />
                            </button>
                          )}
                          <Shield protection={g.protection} />
                          {g.name}
                        </span>
                      </td>
                      <td className="px-3 py-1.5 text-right text-mute">{single ? g.procs[0].pid : `× ${g.procs.length}`}</td>
                      <td className="max-w-32 truncate px-3 py-1.5 font-sans text-dim">{g.user ?? "—"}</td>
                      <Meter pct={g.cpu} warn={g.cpu >= 10}>
                        {g.cpu.toFixed(1)}%
                      </Meter>
                      <Meter pct={(g.memory / maxMemory) * 100}>{bytes(g.memory)}</Meter>
                      <td className="px-3 py-1.5 text-right text-dim">{rate(g.disk)}</td>
                    </tr>
                  );
                  return expanded && !single ? [row, ...g.procs.map((p) => procRow(p, true))] : [row];
                })
              : list.map((p) => procRow(p))}
          </tbody>
        </table>
        {filtered.length === 0 && <EmptyLine>{query ? `Ningún proceso coincide con «${query}».` : "Ningún proceso con ese filtro."}</EmptyLine>}
      </div>

      {selGroup && (
        <div className="mt-3 flex flex-wrap items-center gap-3 rounded-xl border border-line-2 bg-panel px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-2">
              <span className="font-medium text-ink">{selGroup.name}</span>
              <span className="text-xs text-dim">
                {selGroup.procs.length} procesos · {selGroup.cpu.toFixed(1)}% CPU · {bytes(selGroup.memory)}
              </span>
            </div>
            <div className="truncate font-mono text-[11px] text-mute select-text">{selGroup.procs[0].exe ?? "Ruta no disponible"}</div>
            {selGroup.protection === "critical" && <div className="text-xs text-bad">Es parte de Windows: finalizarlo puede colgar o reiniciar el equipo.</div>}
          </div>
          <button
            onClick={() => toolsApi.openProcessLocation(selGroup.procs[0].pid).catch((e) => toast("error", String(e)))}
            className="flex items-center gap-1.5 rounded-md border border-line-2 px-3 py-1.5 text-xs text-dim hover:text-ink"
          >
            <FolderOpen size={13} /> Abrir ubicación
          </button>
          {selGroup.protection !== "critical" && selGroup.protection !== "self" && killable(selGroup).length > 0 && (
            <button
              onClick={() => void killGroup(selGroup)}
              disabled={busy}
              className="flex items-center gap-1.5 rounded-md border border-bad/60 bg-bad/10 px-3 py-1.5 text-xs font-medium text-bad hover:bg-bad/20 disabled:opacity-40"
              title="Cierra el programa entero: todos sus procesos"
            >
              <XCircle size={13} /> Finalizar el programa
            </button>
          )}
        </div>
      )}

      {sel && (
        <div className="mt-3 flex flex-wrap items-center gap-3 rounded-xl border border-line-2 bg-panel px-4 py-3">
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
                onClick={() => void kill(sel, true)}
                disabled={busy}
                className="flex items-center gap-1.5 rounded-md border border-bad/40 px-3 py-1.5 text-xs text-bad hover:bg-bad/10 disabled:opacity-40"
                title="Finaliza el proceso y todos los que ha abierto"
              >
                <Skull size={13} /> Finalizar árbol
              </button>
              <button
                onClick={() => void kill(sel, false)}
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
