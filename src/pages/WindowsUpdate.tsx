import { CircleCheck, CircleX, Eye, EyeOff, Loader2, Pause, Play, RefreshCw, Search, TriangleAlert, Wrench } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, Loading } from "../components/ui";
import { bytes } from "../lib/format";
import { toolboxApi, tweaksApi, updateApi, type PendingUpdate, type UpdateHistoryEntry } from "../lib/api";

const RESULT = {
  ok: { label: "Instalada", icon: CircleCheck, color: "text-ok" },
  partial: { label: "Con errores", icon: TriangleAlert, color: "text-warn" },
  failed: { label: "Falló", icon: CircleX, color: "text-bad" },
  aborted: { label: "Cancelada", icon: CircleX, color: "text-mute" },
  progress: { label: "En curso", icon: Loader2, color: "text-neon" },
};

const when = (iso: string | null) => (iso ? new Date(iso).toLocaleDateString("es", { day: "numeric", month: "short", year: "numeric" }) : "—");

export function WindowsUpdate({ isAdmin }: { isAdmin: boolean }) {
  const [history, setHistory] = useState<UpdateHistoryEntry[] | null>(null);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [paused, setPaused] = useState<string | null>(null);
  const [pending, setPending] = useState<PendingUpdate[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [onlyFailed, setOnlyFailed] = useState(false);
  const toast = useToast();

  const load = useCallback(() => {
    updateApi.history().then(setHistory).catch((e) => setHistoryError(String(e)));
    void updateApi.pauseState().then((s) => setPaused(s.pausedUntil));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const pause = async (days: number) => {
    setBusy("pause");
    try {
      await updateApi.pause(days);
      toast("ok", days ? `Actualizaciones pausadas ${days} días.` : "Actualizaciones reanudadas.");
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const search = async () => {
    setSearching(true);
    try {
      setPending(await updateApi.pending());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setSearching(false);
    }
  };

  const toggleHidden = async (u: PendingUpdate) => {
    setBusy(u.id);
    try {
      await updateApi.setHidden(u.id, u.title, !u.hidden);
      setPending((p) => p?.map((x) => (x.id === u.id ? { ...x, hidden: !u.hidden } : x)) ?? null);
      toast("ok", u.hidden ? "La actualización vuelve a estar disponible." : "Actualización ocultada: Windows no la instalará.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const repair = async () => {
    setBusy("repair");
    try {
      const r = await tweaksApi.run("repair.windows-update");
      toast("ok", r.message || "Windows Update reparado.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const failed = history?.filter((h) => h.result === "failed") ?? [];
  const shown = (history ?? []).filter((h) => !onlyFailed || h.result === "failed" || h.result === "partial");

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <Card title="Estado" className="col-span-12 lg:col-span-5">
        {paused ? (
          <p className="flex items-center gap-2 text-sm text-warn">
            <Pause size={14} /> Pausadas hasta el {when(paused)}
          </p>
        ) : (
          <p className="flex items-center gap-2 text-sm text-ok">
            <Play size={14} /> Actualizaciones activas
          </p>
        )}
        {failed.length > 0 && <p className="mt-1 text-xs text-bad">{failed.length} actualizaciones fallidas en el historial reciente.</p>}
        <div className="mt-3 flex flex-wrap gap-2">
          {paused ? (
            <Button onClick={() => pause(0)} disabled={!isAdmin || busy !== null}>
              <Play size={13} /> Reanudar
            </Button>
          ) : (
            [7, 14, 35].map((d) => (
              <Button key={d} kind="ghost" onClick={() => pause(d)} disabled={!isAdmin || busy !== null}>
                <Pause size={13} /> Pausar {d} días
              </Button>
            ))
          )}
        </div>
        <div className="mt-3 flex flex-wrap gap-2 border-t border-line/60 pt-3">
          <Button kind="ghost" onClick={repair} disabled={!isAdmin || busy !== null}>
            {busy === "repair" ? <Loader2 size={13} className="animate-spin" /> : <Wrench size={13} />} Reparar Windows Update
          </Button>
          <Button kind="ghost" onClick={() => toolboxApi.launch("set-update")}>
            Abrir Windows Update
          </Button>
        </div>
        {!isAdmin && <p className="mt-2 text-[11px] text-warn">Pausar, ocultar y reparar requieren administrador.</p>}
      </Card>

      <Card
        title="Pendientes"
        className="col-span-12 lg:col-span-7"
        right={
          <Button kind="ghost" onClick={search} disabled={searching}>
            {searching ? <Loader2 size={13} className="animate-spin" /> : <Search size={13} />} Buscar
          </Button>
        }
      >
        {searching ? (
          <TaskStatus task="wu-search" active fallback="Buscando actualizaciones…" />
        ) : pending === null ? (
          <p className="text-sm text-mute">
            Pulsa <span className="text-ink">Buscar</span> para consultar a Microsoft (tarda uno o dos minutos). Aquí puedes ocultar una actualización que
            da problemas para que Windows no la vuelva a instalar.
          </p>
        ) : pending.length === 0 ? (
          <p className="flex items-center gap-2 text-sm text-ok">
            <CircleCheck size={14} /> No hay actualizaciones pendientes.
          </p>
        ) : (
          <div className="max-h-72 space-y-1 overflow-y-auto">
            {pending.map((u) => (
              <div key={u.id} className={`flex items-center gap-3 rounded-md px-2 py-1.5 ${u.hidden ? "opacity-50" : ""}`}>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm text-ink" title={u.title}>
                    {u.title}
                  </div>
                  <div className="text-[11px] text-mute">
                    {[u.kb, u.driver ? "driver" : null, u.size ? bytes(u.size) : null, u.hidden ? "oculta" : null].filter(Boolean).join(" · ")}
                  </div>
                </div>
                <button
                  onClick={() => toggleHidden(u)}
                  disabled={!isAdmin || busy !== null}
                  className="rounded p-1.5 text-mute hover:text-ink disabled:opacity-30"
                  title={u.hidden ? "Volver a mostrar" : "Ocultar: Windows no la instalará"}
                >
                  {busy === u.id ? <Loader2 size={14} className="animate-spin" /> : u.hidden ? <Eye size={14} /> : <EyeOff size={14} />}
                </button>
              </div>
            ))}
          </div>
        )}
      </Card>

      <Card
        title="Historial"
        className="col-span-12"
        right={
          <div className="flex items-center gap-3">
            <label className="flex items-center gap-1.5 text-[11px] text-dim">
              <input type="checkbox" checked={onlyFailed} onChange={(e) => setOnlyFailed(e.target.checked)} className="accent-[var(--color-neon)]" />
              Solo con errores
            </label>
            <button onClick={load} className="text-mute hover:text-ink" title="Volver a leer">
              <RefreshCw size={12} />
            </button>
          </div>
        }
      >
        {historyError ? (
          <p className="text-sm text-bad">{historyError}</p>
        ) : !history ? (
          <Loading text="Leyendo historial…" />
        ) : shown.length === 0 ? (
          <p className="text-sm text-mute">{onlyFailed ? "Ninguna actualización ha fallado recientemente." : "Sin historial."}</p>
        ) : (
          <div className="divide-y divide-line/60">
            {shown.map((h, i) => {
              const r = RESULT[h.result];
              return (
                <div key={i} className="py-2">
                  <div className="flex items-center gap-3">
                    <r.icon size={14} className={`shrink-0 ${r.color}`} />
                    <span className="min-w-0 flex-1 truncate text-sm text-ink" title={h.title}>
                      {h.title}
                    </span>
                    <span className="shrink-0 text-xs text-mute">{when(h.date)}</span>
                    <span className={`w-20 shrink-0 text-right text-xs ${r.color}`}>{r.label}</span>
                  </div>
                  {h.result !== "ok" && h.code && (
                    <p className="mt-0.5 ml-6 text-xs text-dim">
                      <span className="font-mono text-mute">{h.code}</span> {h.explanation ?? "Error sin explicación conocida: busca el código en la web de Microsoft."}
                    </p>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </Card>
    </div>
  );
}
