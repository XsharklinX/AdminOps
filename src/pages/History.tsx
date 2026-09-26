import {
  CheckCircle2,
  ExternalLink,
  History as HistoryIcon,
  LifeBuoy,
  Loader2,
  Play,
  Plus,
  ScrollText,
  Undo2,
  Wrench,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Card } from "../components/ui";
import { appApi, tweaksApi, type JournalEntry, type RestorePoint } from "../lib/api";

const OP = {
  apply: { label: "Aplicado", icon: Wrench },
  revert: { label: "Deshecho", icon: Undo2 },
  run: { label: "Ejecutado", icon: Play },
  restorePoint: { label: "Punto de restauración", icon: LifeBuoy },
};

const when = (secs: number) =>
  new Date(secs * 1000).toLocaleString("es", { dateStyle: "medium", timeStyle: "short" });

export function History({ isAdmin }: { isAdmin: boolean }) {
  const [entries, setEntries] = useState<JournalEntry[]>([]);
  const [points, setPoints] = useState<RestorePoint[] | null>(null);
  const [pointsError, setPointsError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [reverting, setReverting] = useState<number | null>(null);
  const toast = useToast();

  const loadJournal = useCallback(() => tweaksApi.journal().then(setEntries), []);
  const loadPoints = useCallback(async () => {
    if (!isAdmin) return;
    try {
      setPoints(await tweaksApi.listRestorePoints());
      setPointsError(null);
    } catch (e) {
      setPointsError(String(e));
    }
  }, [isAdmin]);

  useEffect(() => {
    loadJournal();
    loadPoints();
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
      loadJournal();
      loadPoints();
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
      loadJournal();
    }
  };

  return (
    <div className="mx-auto grid max-w-5xl grid-cols-12 gap-4 p-6">
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
        {!isAdmin ? (
          <p className="text-xs text-warn">Requiere ejecutar como administrador.</p>
        ) : pointsError ? (
          <p className="text-xs break-words text-bad">{pointsError}</p>
        ) : points === null ? (
          <p className="font-mono text-xs text-mute">Cargando…</p>
        ) : points.length === 0 ? (
          <p className="text-xs text-mute">No hay puntos de restauración en este equipo.</p>
        ) : (
          <ul className="max-h-80 space-y-1.5 overflow-y-auto">
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

      <Card title="Diario de cambios" icon={<HistoryIcon size={14} />} className="col-span-12 lg:col-span-7">
        {entries.length === 0 ? (
          <p className="py-6 text-center text-sm text-mute">Todavía no se ha hecho ningún cambio.</p>
        ) : (
          <ol className="space-y-1">
            {entries.map((e) => {
              const op = OP[e.op];
              const canUndo = e.undoable && !e.reverted;
              return (
                <li key={e.id} className="flex items-start gap-3 rounded-lg px-2 py-2 hover:bg-panel-2">
                  <op.icon size={15} className={`mt-0.5 shrink-0 ${e.ok ? "text-neon" : "text-bad"}`} />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="text-sm text-ink">{e.title}</span>
                      <span className="text-[11px] text-mute">{op.label}</span>
                      {e.ok ? (
                        <CheckCircle2 size={12} className="text-ok" />
                      ) : (
                        <XCircle size={12} className="text-bad" />
                      )}
                      {e.reverted && (
                        <span className="rounded border border-line-2 px-1.5 text-[10px] text-dim">Deshecho</span>
                      )}
                    </div>
                    {e.message && (
                      <p className={`text-xs break-words select-text ${e.ok ? "text-dim" : "text-bad"}`}>{e.message}</p>
                    )}
                    <p className="font-mono text-[11px] text-mute">{when(e.timestamp)}</p>
                  </div>
                  {canUndo && (
                    <button
                      onClick={() => revert(e)}
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
        <pre className="max-h-96 overflow-auto rounded-md border border-line bg-void/60 p-3 font-mono text-[11px] leading-relaxed whitespace-pre-wrap text-dim select-text">
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
