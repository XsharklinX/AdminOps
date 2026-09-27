import { FileWarning, RefreshCw, Search } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useToast } from "../components/feedback";
import { Switch } from "../components/TweakCard";
import { systemApi, type StartupItem } from "../lib/api";
import { BootCard } from "../components/Maintenance";

const SOURCE_LABEL = { registry: "Registro", folder: "Carpeta", task: "Tarea" };

export function Startup({ isAdmin }: { isAdmin: boolean }) {
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [hideMicrosoft, setHideMicrosoft] = useState(false);
  const toast = useToast();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await systemApi.listStartup());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    load();
  }, [load]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (items ?? []).filter(
      (i) =>
        (!hideMicrosoft || !i.publisher?.startsWith("Microsoft")) &&
        (!q || [i.name, i.description, i.publisher, i.command].some((s) => s?.toLowerCase().includes(q))),
    );
  }, [items, query, hideMicrosoft]);

  const toggle = async (i: StartupItem) => {
    setBusy(i.id);
    try {
      await systemApi.setStartupEnabled(i.id, !i.enabled);
      // Actualización optimista: volver a listar lanza PowerShell y tarda ~1 s.
      setItems((all) => all?.map((x) => (x.id === i.id ? { ...x, enabled: !i.enabled } : x)) ?? null);
      toast("ok", `${i.description ?? i.name}: ${i.enabled ? "no arrancará" : "arrancará"} con Windows.`);
    } catch (e) {
      toast("error", `${i.name}: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  if (!items) return <p className="p-8 font-mono text-sm text-mute">Leyendo programas de inicio…</p>;

  const enabled = items.filter((i) => i.enabled).length;

  return (
    <div className="mx-auto max-w-5xl p-6">
      <BootCard />
      <div className="mb-4 flex items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{enabled}</span> de {items.length} arrancan con Windows
        </p>
        <div className="relative ml-auto w-72">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar programa, editor o ruta…"
            className="w-full rounded-md border border-line bg-panel py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <label className="flex items-center gap-1.5 text-xs text-dim">
          <input
            type="checkbox"
            checked={hideMicrosoft}
            onChange={(e) => setHideMicrosoft(e.target.checked)}
            className="accent-[var(--color-neon)]"
          />
          Ocultar Microsoft
        </label>
        <button
          onClick={load}
          disabled={loading}
          className="rounded-md p-1.5 text-dim transition-colors hover:bg-panel-2 hover:text-ink"
          title="Volver a leer"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      <div className="overflow-hidden rounded-xl border border-line bg-panel">
        {visible.map((i, idx) => {
          const blocked = i.needsAdmin && !isAdmin;
          return (
            <div
              key={i.id}
              className={`flex items-center gap-4 px-4 py-2.5 ${idx > 0 ? "border-t border-line/70" : ""} ${
                i.enabled ? "" : "opacity-60"
              }`}
            >
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-baseline gap-x-2">
                  <span className="text-sm font-medium text-ink">{i.description ?? i.name}</span>
                  {i.description && i.description !== i.name && (
                    <span className="truncate text-[11px] text-mute">{i.name}</span>
                  )}
                  {!i.target && (
                    <span
                      className="flex items-center gap-1 text-[10px] text-warn"
                      title="El ejecutable ya no existe: probablemente restos de un programa desinstalado"
                    >
                      <FileWarning size={11} /> Archivo no encontrado
                    </span>
                  )}
                </div>
                <div className="truncate font-mono text-[11px] text-mute select-text" title={i.command}>
                  {i.command}
                </div>
              </div>
              <div className="w-44 shrink-0 truncate text-xs text-dim" title={i.publisher ?? undefined}>
                {i.publisher ?? <span className="text-mute">Editor desconocido</span>}
              </div>
              <div className="w-48 shrink-0 truncate text-[11px] text-mute" title={i.location}>
                <span className="mr-1.5 rounded bg-line px-1.5 py-px font-mono text-[9px]">{SOURCE_LABEL[i.source]}</span>
                {i.location}
              </div>
              <div title={blocked ? "Requiere administrador" : undefined}>
                <Switch on={i.enabled} disabled={blocked || busy === i.id} onClick={() => toggle(i)} />
              </div>
            </div>
          );
        })}
        {visible.length === 0 && <p className="px-4 py-8 text-center text-sm text-mute">Sin resultados.</p>}
      </div>
      <p className="mt-3 text-xs text-mute">
        Desactivar no borra nada: usa el mismo mecanismo que el Administrador de tareas y se puede deshacer desde el
        Historial.
      </p>
    </div>
  );
}
