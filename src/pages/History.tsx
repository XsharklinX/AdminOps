import {
  CheckCircle2,
  ExternalLink,
  History as HistoryIcon,
  LifeBuoy,
  Loader2,
  Play,
  Plus,
  ScrollText,
  Search,
  Undo2,
  Wrench,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Card, EmptyLine, ErrorState, Loading } from "../components/ui";
import { groupByDay, JOURNAL_FILTERS, matchesJournal, type JournalFilter } from "../lib/journalDays";
import { appApi, tweaksApi, type JournalEntry, type RestorePoint } from "../lib/api";
import { RestoreStorageCard } from "../components/Maintenance";
import { Timeline } from "../components/Timeline";
import type { PageId } from "../components/Sidebar";
import { useOnJournalChange } from "../lib/journalEvents";

const OP = {
  apply: { label: "Aplicado", icon: Wrench },
  revert: { label: "Deshecho", icon: Undo2 },
  run: { label: "Ejecutado", icon: Play },
  restorePoint: { label: "Punto de restauración", icon: LifeBuoy },
};

const time = (secs: number) => new Date(secs * 1000).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
/** Cuántos cambios se pintan de una vez (el diario de un equipo muy trabajado es largo). */
const PAGE = 150;

export function History({ isAdmin, onNavigate }: { isAdmin: boolean; onNavigate?: (p: PageId) => void }) {
  const [entries, setEntries] = useState<JournalEntry[] | null>(null);
  const [journalError, setJournalError] = useState<string | null>(null);
  const [filter, setFilter] = useState<JournalFilter>("all");
  const [query, setQuery] = useState("");
  const [limit, setLimit] = useState(PAGE);
  const [points, setPoints] = useState<RestorePoint[] | null>(null);
  const [pointsError, setPointsError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [reverting, setReverting] = useState<number | null>(null);
  const toast = useToast();

  const loadJournal = useCallback(
    () =>
      tweaksApi
        .journal()
        .then((j) => {
          setEntries(j);
          setJournalError(null);
        })
        .catch((e) => setJournalError(String(e))),
    [],
  );

  const shown = useMemo(() => (entries ?? []).filter((e) => matchesJournal(e, filter, query)), [entries, filter, query]);
  const days = useMemo(() => groupByDay(shown.slice(0, limit)), [shown, limit]);
  const undoable = useMemo(() => (entries ?? []).filter((e) => matchesJournal(e, "undoable", "")).length, [entries]);
  const loadPoints = useCallback(async () => {
    if (!isAdmin) return;
    try {
      setPoints(await tweaksApi.listRestorePoints());
      setPointsError(null);
    } catch (e) {
      setPointsError(String(e));
    }
  }, [isAdmin]);

  useOnJournalChange(loadJournal);
  useEffect(() => {
    void loadJournal();
    void loadPoints();
  }, [loadJournal, loadPoints]);

  const createPoint = async () => {
    setCreating(true);
    try {
      await tweaksApi.createRestorePoint();
      toast("ok", "Punto de restauración creado.");
    } catch (e) {
      toast("error", `No se pudo crear el punto: ${e}`);
    } finally {
      setCreating(false);
      void loadJournal();
      void loadPoints();
    }
  };

  const revert = async (e: JournalEntry) => {
    setReverting(e.id);
    try {
      const r = await tweaksApi.revertEntry(e.id);
      toast("ok", `${e.title}: ${r.message}`);
    } catch (err) {
      toast("error", `${e.title}: ${err}`);
    } finally {
      setReverting(null);
      void loadJournal();
    }
  };

  return (
    <div className="mx-auto grid max-w-5xl grid-cols-12 gap-4 p-6">
      <Timeline onNavigate={onNavigate} />
      <Card
        title="Puntos de restauración"
        icon={<LifeBuoy size={14} />}
        className="col-span-12 lg:col-span-5 lg:self-start"
        right={
          <button
            onClick={() => tweaksApi.openSystemRestore().catch((e) => toast("error", String(e)))}
            className="flex items-center gap-1 text-xs text-mute hover:text-ink"
            title="Abrir Restaurar sistema de Windows"
          >
            Restaurar sistema <ExternalLink size={11} />
          </button>
        }
      >
        <p className="mb-3 text-xs text-dim">
          Se crea uno automáticamente antes de aplicar ajustes de riesgo medio o alto (máximo uno cada 30 min).
        </p>
        <button
          onClick={createPoint}
          disabled={!isAdmin || creating}
          className="mb-4 flex w-full items-center justify-center gap-1.5 rounded-md border border-neon/40 py-2 text-sm font-medium text-neon transition-colors hover:bg-neon/10 disabled:cursor-not-allowed disabled:opacity-40"
        >
          {creating ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
          {creating ? "Creando punto de restauración…" : "Crear punto ahora"}
        </button>
        <TaskStatus task="restore-point" active={creating} fallback="Creando…" className="-mt-2 mb-4" />
        <RestoreStorageCard isAdmin={isAdmin} />
        {!isAdmin ? (
          <p className="text-xs text-warn">Requiere ejecutar como administrador.</p>
        ) : pointsError ? (
          <p className="text-xs break-words text-bad">{pointsError}</p>
        ) : points === null ? (
          <Loading />
        ) : points.length === 0 ? (
          <EmptyLine>No hay puntos de restauración en este equipo.</EmptyLine>
        ) : (
          <ul className="pane-md space-y-1.5 overflow-y-auto">
            {points.map((p) => (
              <li key={p.sequence} className="rounded-md border border-line bg-void/40 px-3 py-2">
                <div className="truncate text-sm text-ink">{p.description}</div>
                <div className="font-mono text-[11px] text-mute">
                  #{p.sequence} · {new Date(p.created).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" })}
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card
        title={entries?.length ? `Diario de cambios · ${entries.length}` : "Diario de cambios"}
        icon={<HistoryIcon size={14} />}
        className="col-span-12 lg:col-span-7"
        right={undoable > 0 ? <span className="text-[11px] text-mute">{undoable === 1 ? "1 cambio se puede deshacer" : `${undoable} cambios se pueden deshacer`}</span> : undefined}
      >
        {journalError ? (
          <ErrorState message={journalError} onRetry={() => void loadJournal()} />
        ) : entries === null ? (
          <Loading />
        ) : entries.length === 0 ? (
          <EmptyLine>Todavía no se ha hecho ningún cambio. Todo lo que AdminOps cambie en el equipo quedará aquí, con su «Deshacer».</EmptyLine>
        ) : (
          <>
            <div className="mb-3 flex flex-wrap items-center gap-1.5">
              <div className="relative min-w-40 flex-1">
                <Search size={13} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-mute" />
                <input
                  value={query}
                  onChange={(e) => {
                    setQuery(e.target.value);
                    setLimit(PAGE);
                  }}
                  placeholder="Buscar en el diario"
                  className="h-8 w-full rounded-md border border-line bg-void/60 pr-2 pl-7 text-xs text-ink outline-none placeholder:text-mute focus:border-neon/50"
                  aria-label="Buscar en el diario"
                />
              </div>
              {JOURNAL_FILTERS.map(([id, label]) => (
                <button
                  key={id}
                  onClick={() => {
                    setFilter(id);
                    setLimit(PAGE);
                  }}
                  className={`rounded-full border px-2.5 py-0.5 text-xs transition-colors ${filter === id ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`}
                >
                  {label}
                </button>
              ))}
            </div>
            {shown.length === 0 ? (
              <EmptyLine>Nada en el diario con ese filtro.</EmptyLine>
            ) : (
              <div className="space-y-4">
                {days.map((d) => (
                  <section key={d.day}>
                    <h3 className="mb-1 flex items-baseline gap-2 text-xs font-medium text-dim">
                      {d.label}
                      <span className="font-normal text-mute">
                        {d.entries.length} {d.entries.length === 1 ? "cambio" : "cambios"}
                      </span>
                    </h3>
                    {/* La línea vertical une los cambios del día, como una línea de tiempo. */}
                    <ol className="relative ml-[7px] space-y-0.5 border-l border-line pl-4">
                      {d.entries.map((e) => {
                        const op = OP[e.op];
                        const canUndo = e.undoable && !e.reverted;
                        return (
                          <li key={e.id} className="relative flex items-start gap-3 rounded-lg px-2 py-1.5 hover:bg-panel-2">
                            <span className={`absolute top-3 -left-[21px] size-2 rounded-full ring-2 ring-panel ${e.ok ? (e.reverted ? "bg-mute" : "bg-neon") : "bg-bad"}`} />
                            <span className="w-10 shrink-0 pt-0.5 font-mono text-[11px] text-mute">{time(e.timestamp)}</span>
                            <op.icon size={14} className={`mt-0.5 shrink-0 ${e.ok ? "text-dim" : "text-bad"}`} />
                            <div className="min-w-0 flex-1">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className={`text-sm ${e.reverted ? "text-dim line-through" : "text-ink"}`}>{e.title}</span>
                                <span className="text-[11px] text-mute">{op.label}</span>
                                {e.ok ? <CheckCircle2 size={12} className="text-ok" /> : <XCircle size={12} className="text-bad" />}
                                {e.reverted && <span className="rounded border border-line-2 px-1.5 text-[11px] text-dim">Deshecho</span>}
                              </div>
                              {e.message && <p className={`text-xs break-words select-text ${e.ok ? "text-dim" : "text-bad"}`}>{e.message}</p>}
                            </div>
                            {canUndo && (
                              <button
                                onClick={() => void revert(e)}
                                disabled={reverting !== null}
                                className="flex shrink-0 items-center gap-1 rounded-md border border-line-2 px-2.5 py-1 text-xs text-dim transition-colors hover:border-neon/40 hover:text-neon disabled:opacity-40"
                              >
                                {reverting === e.id ? <Loader2 size={12} className="animate-spin" /> : <Undo2 size={12} />}
                                Deshacer
                              </button>
                            )}
                          </li>
                        );
                      })}
                    </ol>
                  </section>
                ))}
                {shown.length > limit && (
                  <button onClick={() => setLimit(limit + PAGE)} className="text-xs text-neon hover:underline">
                    Ver más antiguos ({shown.length - limit})
                  </button>
                )}
              </div>
            )}
          </>
        )}
      </Card>

      <LogViewer />
    </div>
  );
}

function LogViewer() {
  const [text, setText] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const toast = useToast();
  const load = useCallback(() => {
    appApi.readLog(400).then(setText).catch((e) => toast("error", String(e)));
  }, [toast]);

  useEffect(() => {
    if (open) load();
  }, [open, load]);

  return (
    <Card
      title="Registro técnico"
      icon={<ScrollText size={14} />}
      className="col-span-12"
      right={
        <div className="flex gap-3 text-[11px]">
          {open && (
            <button onClick={load} className="text-mute hover:text-ink">
              Actualizar
            </button>
          )}
          <button
            onClick={() => appApi.openLogsFolder().catch((e) => toast("error", String(e)))}
            className="flex items-center gap-1 text-mute hover:text-ink"
          >
            Abrir carpeta <ExternalLink size={10} />
          </button>
          <button onClick={() => setOpen(!open)} className="text-neon hover:underline">
            {open ? "Ocultar" : "Mostrar"}
          </button>
        </div>
      }
    >
      {open ? (
        <pre className="pane-md overflow-auto rounded-md border border-line bg-void/60 p-3 font-mono text-[11px] leading-relaxed whitespace-pre-wrap text-dim select-text">
          {text === null ? "Cargando…" : text || "El registro está vacío."}
        </pre>
      ) : (
        <p className="text-xs text-dim">
          Todo lo que AdminOps ejecuta (scripts, errores, tiempos) queda registrado. Adjúntalo si algo falla.
        </p>
      )}
    </Card>
  );
}
