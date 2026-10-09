import {
  ArrowUpCircle,
  CheckCircle2,
  Circle,
  Download,
  FileWarning,
  Folder,
  KeyRound,
  Link2,
  Loader2,
  PackageX,
  RefreshCw,
  Search,
  SkipForward,
  Trash2,
  Wrench,
  XCircle,
} from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Modal, ErrorState, Loading, iconBtn } from "../components/ui";
import { DataTable, type Column } from "../components/DataTable";
import { bytes, friendlyPath } from "../lib/format";
import { programKey as key, norm } from "../lib/programs";
import { officeApi, programsApi, toolsApi, type InstalledProgram, type Leftover, type SoftwareUpdate } from "../lib/api";
import { useLiveEffect } from "../lib/useLiveEffect";
import { readCached, remember } from "../lib/cachedRead";

type Filter = "all" | "updates" | "big" | "recent" | "user" | "wizard" | "orphans";

const FILTERS: { id: Filter; label: string; hint: string }[] = [
  { id: "all", label: "Todos", hint: "" },
  { id: "updates", label: "Con actualización", hint: "winget tiene una versión más nueva" },
  { id: "big", label: "Grandes", hint: "Más de 1 GB" },
  { id: "recent", label: "Recientes", hint: "Instalados en los últimos 30 días" },
  { id: "user", label: "Solo este usuario", hint: "Instalados sin administrador" },
  { id: "wizard", label: "Necesitan asistente", hint: "No se pueden quitar en silencio" },
  { id: "orphans", label: "Huérfanas", hint: "Su desinstalador ya no existe" },
];

const GB = 1024 ** 3;
const daysAgo = (d: string | null) => (d ? (Date.now() - new Date(d).getTime()) / 86_400_000 : Infinity);

type RunState = "pending" | "running" | "ok" | "error" | "skipped";

export function Uninstall({ isAdmin }: { isAdmin: boolean }) {
  const [list, setList] = useState<InstalledProgram[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [hideComponents, setHideComponents] = useState(true);
  const [hideMicrosoft, setHideMicrosoft] = useState(false);
  const [updates, setUpdates] = useState<SoftwareUpdate[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [target, setTarget] = useState<InstalledProgram | null>(null);
  const [batch, setBatch] = useState<InstalledProgram[] | null>(null);
  const [running, setRunning] = useState<string | null>(null);
  const [leftovers, setLeftovers] = useState<{ program: InstalledProgram; items: Leftover[] }[] | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const [failed, setFailed] = useState<string | null>(null);
  /** `fromMemory`: al abrir la pantalla, lo último leído al momento (luego se lee de nuevo). */
  const load = useCallback(async (fromMemory = false) => {
    setFailed(null);
    setLoading(true);
    try {
      if (fromMemory) await readCached("programs", programsApi.list, (v) => setList(v));
      else {
        const v = await programsApi.list();
        setList(v);
        remember("programs", v);
      }
    } catch (e) {
      setFailed(String(e));
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useLiveEffect(
    (vigente) => {
      void load(true);
      // Actualizaciones de winget en segundo plano (se reutiliza la última lista si es reciente).
      toolsApi
        .softwareUpdates()
        .then((u) => vigente() && setUpdates(u))
        .catch(() => vigente() && setUpdates([]));
    },
    [load],
  );

  const updateOf = useMemo(() => {
    const m = new Map<string, SoftwareUpdate>();
    for (const u of updates ?? []) m.set(key(u.name), u);
    return (p: InstalledProgram) => m.get(key(p.name));
  }, [updates]);

  const visible = useMemo(() => {
    const q = norm(query.trim());
    const v = (list ?? []).filter((p) => {
      if (hideComponents && p.component && filter !== "orphans") return false;
      if (hideMicrosoft && p.publisher?.startsWith("Microsoft")) return false;
      if (q && !norm(`${p.name} ${p.publisher ?? ""}`).includes(q)) return false;
      switch (filter) {
        case "updates":
          return !!updateOf(p);
        case "big":
          return (p.size ?? 0) > GB;
        case "recent":
          return daysAgo(p.installed) <= 30;
        case "user":
          return p.perUser;
        case "wizard":
          return !p.silent && !p.orphan;
        case "orphans":
          return p.orphan;
        default:
          return true;
      }
    });
    return v;
  }, [list, query, filter, hideComponents, hideMicrosoft, updateOf]);

  const toggle = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  const uninstall = async (p: InstalledProgram, silent: boolean) => {
    setTarget(null);
    setRunning(p.id);
    try {
      const r = await programsApi.uninstall(p.id, silent);
      if (r.removed) {
        toast("ok", `${p.name}: ${r.message}`);
        if (r.leftovers.length) setLeftovers([{ program: p, items: r.leftovers }]);
      } else toast("info", `${p.name}: ${r.message}`);
      await load();
    } catch (e) {
      toast("error", `${p.name}: ${e}`);
    } finally {
      setRunning(null);
    }
  };

  const repair = async (p: InstalledProgram) => {
    setRunning(p.id);
    try {
      toast("ok", `${p.name}: ${await programsApi.repair(p.id)}`);
    } catch (e) {
      toast("error", `${p.name}: ${e}`);
    } finally {
      setRunning(null);
    }
  };

  const update = async (p: InstalledProgram, u: SoftwareUpdate) => {
    setRunning(p.id);
    try {
      const [r] = await toolsApi.upgradeSoftware([u.id]);
      toast(r?.ok ? "ok" : "error", `${p.name}: ${r?.message ?? "sin respuesta"}`);
      if (r?.ok) setUpdates((x) => x?.filter((y) => y.id !== u.id) ?? x);
      await load();
    } catch (e) {
      toast("error", `${p.name}: ${e}`);
    } finally {
      setRunning(null);
    }
  };

  const orphanCleanup = async (p: InstalledProgram) => {
    setRunning(p.id);
    try {
      const items = await programsApi.scanLeftovers(p.id);
      const ok = await confirm({
        title: "Quitar de la lista",
        body: `El desinstalador de «${p.name}» ya no existe, así que el programa probablemente ya no está. Se quitará su entrada (con copia .reg)${
          items.length ? ` y después podrás revisar ${items.length} resto(s) que quedaron.` : "."
        }`,
        confirmLabel: "Quitar",
      });
      if (!ok) return;
      await programsApi.removeOrphan(p.id);
      toast("ok", `«${p.name}» quitado de la lista.`);
      if (items.length) setLeftovers([{ program: p, items }]);
      void load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(null);
    }
  };

  const exportCsv = () => {
    const rows = [
      ["Programa", "Editor", "Versión", "Instalado", "Tamaño (MB)", "Actualización disponible", "Desinstalación silenciosa", "Solo este usuario"],
      ...(list ?? []).map((p) => [
        p.name,
        p.publisher ?? "",
        p.version ?? "",
        p.installed ?? "",
        p.size ? (p.size / 1024 ** 2).toFixed(0) : "",
        updateOf(p)?.available ?? "",
        p.silentKind || "No",
        p.perUser ? "Sí" : "No",
      ]),
    ];
    const csv = rows.map((r) => r.map((c) => (/[;"\n]/.test(c) ? `"${c.replace(/"/g, '""')}"` : c)).join(";")).join("\r\n");
    officeApi.exportCsv("Programas instalados", csv).catch((e) => toast("error", String(e)));
  };

  if (!list) return failed ? <ErrorState page message={failed} onRetry={() => void load()} /> : <Loading page text="Leyendo programas instalados…" />;

  const orphans = list.filter((p) => p.orphan).length;
  const busy = running !== null || batch !== null;
  const chosen = list.filter((p) => selected.has(p.id));
  const chosenSize = chosen.reduce((a, p) => a + (p.size ?? 0), 0);
  const withUpdate = list.filter((p) => updateOf(p)).length;
  const canTouch = (p: InstalledProgram) => p.perUser || isAdmin;

  const columns: Column<InstalledProgram>[] = [
    {
      id: "pick",
      header: "",
      stopClick: true,
      headClass: "w-8",
      cell: (p) => (
        <input
          type="checkbox"
          checked={selected.has(p.id)}
          onChange={() => toggle(p.id)}
          disabled={p.orphan}
          className="size-3.5 accent-[var(--color-neon)]"
          title="Seleccionar para desinstalar varios"
          aria-label={`Seleccionar ${p.name}`}
        />
      ),
    },
    {
      id: "name",
      header: "Programa",
      sortBy: (p) => p.name,
      className: "max-w-0 w-full",
      cell: (p) => {
        const u = updateOf(p);
        return (
          <div className="min-w-0">
            <div className="flex items-center gap-1.5">
              <span className="truncate text-ink">{p.name}</span>
              {p.orphan && (
                <span className="flex shrink-0 items-center gap-1 text-[11px] text-warn" title="El desinstalador ya no existe">
                  <FileWarning size={10} /> huérfana
                </span>
              )}
              {u && (
                <span className="flex shrink-0 items-center gap-0.5 rounded border border-neon/40 px-1 text-[10px] text-neon" title={`winget: ${u.id}`}>
                  <ArrowUpCircle size={10} /> {u.available}
                </span>
              )}
              {p.perUser && <span className="shrink-0 rounded border border-line px-1 text-[10px] text-mute">solo este usuario</span>}
              {p.component && <span className="shrink-0 rounded border border-line px-1 text-[10px] text-mute">componente</span>}
            </div>
            <div className="truncate text-[11px] text-mute">
              {[p.version && `v${p.version}`, p.silentKind ? `silencioso (${p.silentKind})` : !p.orphan && "con asistente"].filter(Boolean).join(" · ") || "—"}
            </div>
          </div>
        );
      },
    },
    {
      id: "publisher",
      header: "Editor",
      sortBy: (p) => p.publisher,
      className: "max-w-40 truncate text-xs text-dim",
      cell: (p) => <span title={p.publisher ?? undefined}>{p.publisher ?? "—"}</span>,
    },
    { id: "installed", header: "Instalado", sortBy: (p) => p.installed, className: "whitespace-nowrap font-mono text-xs text-dim", cell: (p) => p.installed ?? "" },
    { id: "size", header: "Tamaño", align: "right", sortBy: (p) => p.size ?? null, className: "whitespace-nowrap font-mono text-xs text-dim", cell: (p) => (p.size ? bytes(p.size) : "") },
    {
      id: "actions",
      header: "",
      align: "right",
      stopClick: true,
      cell: (p) => {
        const u = updateOf(p);
        return (
          <div className="flex items-center justify-end gap-1 whitespace-nowrap">
            {running === p.id ? (
              <Loader2 size={15} className="animate-spin text-neon" />
            ) : p.orphan ? (
              <Button kind="ghost" size="sm" onClick={() => void orphanCleanup(p)} disabled={busy || !canTouch(p)}>
                Quitar y limpiar restos
              </Button>
            ) : (
              <>
                {u && (
                  <button onClick={() => void update(p, u)} disabled={busy || !isAdmin} className="rounded px-2 py-1 text-xs text-neon hover:bg-neon/10 disabled:opacity-40" title={isAdmin ? `Actualizar a ${u.available}` : "Requiere administrador"}>
                    Actualizar
                  </button>
                )}
                {p.repairable && (
                  <button
                    onClick={() => void repair(p)}
                    disabled={busy || !canTouch(p)}
                    className={`${iconBtn} opacity-0 group-hover:opacity-100 focus-visible:opacity-100`}
                    title="Reparar (reinstala sus archivos)"
                    aria-label={`Reparar ${p.name}`}
                  >
                    <Wrench size={13} />
                  </button>
                )}
                <Button kind="ghost" size="sm" onClick={() => setTarget(p)} disabled={busy || !canTouch(p)} title={!canTouch(p) ? "Requiere administrador" : undefined}>
                  <Trash2 size={13} /> Desinstalar
                </Button>
              </>
            )}
          </div>
        );
      },
    },
  ];

  return (
    <div className="mx-auto max-w-(--page-max) p-6">
      <div className="mb-3 flex flex-wrap items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{list.length}</span> programas
          {withUpdate > 0 && <span className="text-neon"> · {withUpdate} con actualización</span>}
          {orphans > 0 && <span className="text-warn"> · {orphans} entradas huérfanas</span>}
          {updates === null && <span className="text-mute"> · buscando actualizaciones…</span>}
        </p>
        <div className="relative ml-auto w-72 min-w-40 shrink">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar programa o editor…"
            className="w-full rounded-md border border-line bg-panel py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <button onClick={exportCsv} className="flex items-center gap-1 rounded-md px-2 py-1.5 text-xs text-dim hover:bg-panel-2 hover:text-ink" title="Inventario de programas en CSV">
          <Download size={13} /> CSV
        </button>
        <button onClick={() => void load()} disabled={loading || busy} className={iconBtn} title="Volver a leer" aria-label="Volver a leer">
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-1.5">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            onClick={() => setFilter(f.id)}
            title={f.hint}
            className={`rounded-full border px-2.5 py-0.5 text-xs transition-colors ${filter === f.id ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`}
          >
            {f.label}
          </button>
        ))}
        <label className="ml-auto flex items-center gap-1.5 text-xs text-dim" title="Visual C++, .NET, drivers… normalmente no se tocan">
          <input type="checkbox" checked={hideComponents} onChange={(e) => setHideComponents(e.target.checked)} className="accent-[var(--color-neon)]" />
          Ocultar componentes y runtimes
        </label>
        <label className="flex items-center gap-1.5 text-xs text-dim">
          <input type="checkbox" checked={hideMicrosoft} onChange={(e) => setHideMicrosoft(e.target.checked)} className="accent-[var(--color-neon)]" />
          Ocultar Microsoft
        </label>
      </div>

      {selected.size > 0 && (
        <div className="sticky top-2 z-10 mb-3 flex flex-wrap items-center gap-2 rounded-xl border border-neon/40 bg-panel px-4 py-2 shadow-xl">
          <span className="text-sm text-ink">
            {selected.size} seleccionado(s){chosenSize > 0 && ` · ${bytes(chosenSize)}`}
          </span>
          <Button kind="danger" onClick={() => setBatch(chosen.filter((p) => !p.orphan))} disabled={busy || !chosen.some((p) => !p.orphan && canTouch(p))}>
            <Trash2 size={13} /> Desinstalar seleccionados
          </Button>
          <button onClick={() => setSelected(new Set())} className="ml-auto text-xs text-mute hover:text-ink">
            Deseleccionar
          </button>
        </div>
      )}

      {running && (
        <div className="mb-3 rounded-lg border border-neon/30 bg-neon/5 px-4 py-2">
          <TaskStatus task={`uninstall:${running}`} active fallback="Trabajando…" />
        </div>
      )}

      <div className="overflow-x-auto rounded-xl border border-line bg-panel">
        <DataTable
          padded
          rows={visible}
          rowKey={(p) => p.id}
          columns={columns}
          initialSort={{ id: "name", desc: false }}
          rowClass={(p) => `group ${p.orphan ? "opacity-70" : ""} ${selected.has(p.id) ? "bg-neon/5" : ""}`}
          empty="Ningún programa con esa búsqueda y ese filtro."
        />
      </div>
      <p className="mt-3 text-xs text-mute">
        Las apps de Microsoft Store se quitan desde <span className="text-ink">Bloatware</span>. Tras desinstalar, AdminOps busca carpetas, accesos directos y claves del registro
        que hayan quedado (nunca las de otro programa instalado).
      </p>

      {target && (
        <Modal
          title={`Desinstalar ${target.name}`}
          onClose={() => setTarget(null)}
          footer={
            <>
              <Button kind="ghost" onClick={() => setTarget(null)}>
                Cancelar
              </Button>
              {target.silent && (
                <Button kind="danger" onClick={() => uninstall(target, true)}>
                  Desinstalar en silencio
                </Button>
              )}
              <Button kind={target.silent ? "ghost" : "danger"} onClick={() => uninstall(target, false)}>
                {target.silent ? "Con su asistente" : "Desinstalar"}
              </Button>
            </>
          }
        >
          <p className="text-sm text-dim">
            {target.silent
              ? `Se puede desinstalar en silencio (${target.silentKind}), sin ventanas. Si prefieres elegir opciones (conservar configuración, etc.), usa su asistente.`
              : "Se abrirá el asistente de desinstalación del programa: sigue sus pasos. AdminOps espera a que termine."}
          </p>
          {target.size && <p className="mt-2 text-xs text-mute">Ocupa unos {bytes(target.size)}.</p>}
        </Modal>
      )}

      {batch && (
        <BatchDialog
          programs={batch.filter(canTouch)}
          onClose={(found) => {
            setBatch(null);
            setSelected(new Set());
            void load();
            if (found.length) setLeftovers(found);
          }}
        />
      )}

      {leftovers && (
        <LeftoversDialog
          groups={leftovers}
          onClose={() => setLeftovers(null)}
          onDone={(n) => {
            setLeftovers(null);
            toast("ok", `${n} restos eliminados (carpetas a la papelera, claves con copia .reg).`);
          }}
        />
      )}
      {dialog}
    </div>
  );
}

/** Desinstalación en lote: primero los silenciosos, luego los que necesitan su asistente. */
function BatchDialog({ programs, onClose }: { programs: InstalledProgram[]; onClose: (leftovers: { program: InstalledProgram; items: Leftover[] }[]) => void }) {
  const [withWizard, setWithWizard] = useState(true);
  const ordered = useMemo(() => [...programs].sort((a, b) => Number(b.silent) - Number(a.silent)), [programs]);
  const [status, setStatus] = useState<Record<string, { st: RunState; msg: string }>>({});
  const [phase, setPhase] = useState<"ask" | "run" | "done">("ask");
  const [found, setFound] = useState<{ program: InstalledProgram; items: Leftover[] }[]>([]);
  const wizards = programs.filter((p) => !p.silent).length;

  const run = async () => {
    setPhase("run");
    const all: { program: InstalledProgram; items: Leftover[] }[] = [];
    for (const p of ordered) {
      if (!p.silent && !withWizard) {
        setStatus((s) => ({ ...s, [p.id]: { st: "skipped", msg: "Necesita su asistente: no incluido." } }));
        continue;
      }
      setStatus((s) => ({ ...s, [p.id]: { st: "running", msg: p.silent ? "En silencio…" : "Sigue el asistente del programa…" } }));
      try {
        const r = await programsApi.uninstall(p.id, p.silent);
        setStatus((s) => ({ ...s, [p.id]: { st: r.removed ? "ok" : "error", msg: r.message } }));
        if (r.leftovers.length) all.push({ program: p, items: r.leftovers });
      } catch (e) {
        setStatus((s) => ({ ...s, [p.id]: { st: "error", msg: String(e) } }));
      }
    }
    setFound(all);
    setPhase("done");
  };

  const icon = (st: RunState | undefined) =>
    ({
      pending: <Circle size={14} className="text-mute" />,
      running: <Loader2 size={14} className="animate-spin text-neon" />,
      ok: <CheckCircle2 size={14} className="text-ok" />,
      error: <XCircle size={14} className="text-bad" />,
      skipped: <SkipForward size={14} className="text-mute" />,
    })[st ?? "pending"];

  return (
    <Modal
      title={`Desinstalar ${programs.length} programas`}
      onClose={phase === "run" ? () => {} : () => onClose(found)}
      width="w-[620px]"
      footer={
        phase === "ask" ? (
          <>
            {wizards > 0 && (
              <label className="mr-auto flex items-center gap-2 text-xs text-dim">
                <input type="checkbox" checked={withWizard} onChange={(e) => setWithWizard(e.target.checked)} className="accent-[var(--color-neon)]" />
                Incluir los {wizards} que necesitan su asistente (uno a uno)
              </label>
            )}
            <Button kind="ghost" onClick={() => onClose([])}>
              Cancelar
            </Button>
            <Button kind="danger" onClick={run}>
              <Trash2 size={13} /> Desinstalar
            </Button>
          </>
        ) : (
          <Button onClick={() => onClose(found)} disabled={phase === "run"}>
            {phase === "run" ? "Desinstalando…" : found.length ? "Revisar restos" : "Cerrar"}
          </Button>
        )
      }
    >
      <ol className="space-y-2">
        {ordered.map((p) => (
          <li key={p.id} className="flex items-start gap-2.5">
            <span className="mt-0.5">{icon(status[p.id]?.st)}</span>
            <span className="min-w-0 flex-1">
              <span className="text-sm text-ink">{p.name}</span>
              <span className="ml-2 text-[11px] text-mute">{p.silent ? `silencioso (${p.silentKind})` : "con asistente"}</span>
              {status[p.id]?.msg && <span className={`block text-xs ${status[p.id].st === "error" ? "text-bad" : "text-dim"}`}>{status[p.id].msg}</span>}
            </span>
          </li>
        ))}
      </ol>
    </Modal>
  );
}

const KIND = {
  folder: { icon: Folder, label: "Carpeta" },
  shortcut: { icon: Link2, label: "Acceso directo" },
  registry: { icon: KeyRound, label: "Clave del registro" },
};

function LeftoversDialog({ groups, onClose, onDone }: { groups: { program: InstalledProgram; items: Leftover[] }[]; onClose: () => void; onDone: (n: number) => void }) {
  const all = groups.flatMap((g) => g.items.map((i) => ({ ...i, pid: g.program.id })));
  const [selected, setSelected] = useState<Set<string>>(new Set(all.map((i) => `${i.pid}|${i.path}`)));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const total = all.filter((i) => selected.has(`${i.pid}|${i.path}`)).reduce((a, i) => a + i.size, 0);

  const remove = async () => {
    setBusy(true);
    setError(null);
    let n = 0;
    const errors: string[] = [];
    for (const g of groups) {
      const paths = g.items.filter((i) => selected.has(`${g.program.id}|${i.path}`)).map((i) => i.path);
      if (!paths.length) continue;
      try {
        n += await programsApi.removeLeftovers(g.program.id, paths);
      } catch (e) {
        errors.push(`${g.program.name}: ${e}`);
      }
    }
    if (errors.length) {
      setError(errors.join(" · "));
      setBusy(false);
    } else onDone(n);
  };

  const toggle = (k: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(k)) n.delete(k);
      else n.add(k);
      return n;
    });

  return (
    <Modal
      title="Restos de programas"
      onClose={busy ? () => {} : onClose}
      width="w-[680px]"
      footer={
        <>
          {error && <p className="mr-auto max-w-80 text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose} disabled={busy}>
            Dejarlos
          </Button>
          <Button kind="danger" onClick={remove} disabled={busy || !selected.size}>
            {busy ? <Loader2 size={14} className="animate-spin" /> : <PackageX size={14} />} Eliminar ({selected.size}
            {total > 0 ? ` · ${bytes(total)}` : ""})
          </Button>
        </>
      }
    >
      {groups.map((g) => (
        <div key={g.program.id} className="mb-3">
          <p className="mb-1 flex items-center gap-2 text-sm text-dim">
            <CheckCircle2 size={14} className="text-ok" /> {g.program.name}
          </p>
          <div className="space-y-0.5">
            {g.items.map((i) => {
              const K = KIND[i.kind] ?? KIND.folder;
              const k = `${g.program.id}|${i.path}`;
              return (
                <label key={k} className="flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 hover:bg-panel-2">
                  <input type="checkbox" checked={selected.has(k)} onChange={() => toggle(k)} className="accent-[var(--color-neon)]" />
                  <K.icon size={13} className="shrink-0 text-mute" />
                  <span className="min-w-0 flex-1 truncate font-mono text-xs text-ink" title={K.label}>
                    {friendlyPath(i.path)}
                  </span>
                  <span className="shrink-0 font-mono text-xs text-dim">{i.kind === "registry" ? "registro" : bytes(i.size)}</span>
                </label>
              );
            })}
          </div>
        </div>
      ))}
      <p className="text-[11px] text-mute">Carpetas y accesos directos van a la papelera; las claves del registro se copian antes en un .reg (Historial → datos del equipo).</p>
    </Modal>
  );
}
