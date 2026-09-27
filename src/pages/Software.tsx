import { CheckCircle2, Download, Loader2, RefreshCw, Search, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { toolsApi, type SoftwareUpdate } from "../lib/api";

type Result = { id: string; name: string; ok: boolean; message: string };

export function Software({ isAdmin }: { isAdmin: boolean }) {
  const [updates, setUpdates] = useState<SoftwareUpdate[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [running, setRunning] = useState(false);
  const [results, setResults] = useState<Result[] | null>(null);
  const toast = useToast();

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setUpdates(await toolsApi.softwareUpdates());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (updates ?? []).filter((u) => !q || u.name.toLowerCase().includes(q) || u.id.toLowerCase().includes(q));
  }, [updates, query]);

  const toggle = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  const upgrade = async () => {
    setRunning(true);
    setResults(null);
    try {
      const r = await toolsApi.upgradeSoftware([...selected]);
      setResults(r);
      const ok = r.filter((x) => x.ok).length;
      toast(ok === r.length ? "ok" : "info", `${ok} de ${r.length} programas actualizados.`);
      setSelected(new Set(r.filter((x) => !x.ok).map((x) => x.id)));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(false);
      load();
    }
  };

  return (
    <div className="mx-auto max-w-5xl p-6">
      <div className="mb-4 flex items-center gap-3">
        <p className="text-sm text-dim">
          {updates === null ? (
            "Buscando actualizaciones con winget…"
          ) : (
            <>
              <span className="font-mono text-neon">{updates.length}</span> programas con versión nueva disponible
            </>
          )}
        </p>
        <div className="relative ml-auto w-64">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar programa…"
            className="w-full rounded-md border border-line bg-panel py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <button
          onClick={() => setSelected(new Set(visible.map((u) => u.id)))}
          disabled={!updates?.length}
          className="rounded-md border border-line-2 px-3 py-1.5 text-xs text-dim hover:text-ink disabled:opacity-40"
        >
          Seleccionar todo
        </button>
        <button onClick={load} disabled={loading || running} className="rounded-md p-1.5 text-dim hover:bg-panel-2 hover:text-ink" title="Volver a buscar">
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      {!isAdmin && (
        <p className="mb-3 rounded-lg border border-warn/30 bg-warn/5 px-3.5 py-2 text-xs text-warn">
          Sin administrador, los programas instalados para todo el equipo pueden pedir permiso (UAC) al actualizarse.
        </p>
      )}
      {error && <p className="mb-3 rounded-lg border border-bad/30 bg-bad/5 px-3.5 py-2 text-sm text-bad">{error}</p>}

      {results && (
        <div className="mb-4 rounded-xl border border-line bg-panel p-4">
          <h3 className="mb-2 text-sm font-medium">Resultado</h3>
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

      {updates === null && !error ? (
        <p className="flex items-center gap-2 text-sm text-mute">
          <Loader2 size={14} className="animate-spin" /> Puede tardar unos segundos mientras winget actualiza su catálogo.
        </p>
      ) : (
        <div className="overflow-hidden rounded-xl border border-line bg-panel">
          {visible.map((u, i) => (
            <label key={u.id} className={`flex cursor-pointer items-center gap-3 px-4 py-2 ${i ? "border-t border-line/60" : ""} hover:bg-panel-2`}>
              <input
                type="checkbox"
                checked={selected.has(u.id)}
                onChange={() => toggle(u.id)}
                disabled={running}
                className="size-4 accent-[var(--color-neon)]"
              />
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm text-ink">{u.name}</div>
                <div className="truncate font-mono text-[11px] text-mute">{u.id}</div>
              </div>
              <div className="w-56 text-right font-mono text-xs tabular">
                <span className="text-mute">{u.version}</span> <span className="text-dim">→</span> <span className="text-neon">{u.available}</span>
              </div>
            </label>
          ))}
          {updates && visible.length === 0 && <p className="px-4 py-8 text-center text-sm text-mute">Todo está al día.</p>}
        </div>
      )}

      {selected.size > 0 && (
        <div className="sticky bottom-0 z-30 -mx-6 -mb-6 mt-6 border-t border-line bg-panel px-6 py-3">
          <div className="mx-auto flex max-w-5xl items-center justify-between gap-4">
            {running ? (
              <TaskStatus task="software" active={running} fallback="Actualizando…" />
            ) : (
              <span className="text-sm text-dim">
                <span className="font-mono text-neon">{selected.size}</span> seleccionados
              </span>
            )}
            <button
              onClick={upgrade}
              disabled={running}
              className="flex shrink-0 items-center gap-1.5 rounded-md border border-neon/50 bg-neon/10 px-4 py-1.5 text-sm font-medium text-neon hover:bg-neon/20 disabled:opacity-50"
            >
              <Download size={14} /> Actualizar seleccionados
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
