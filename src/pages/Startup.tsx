import { FileWarning, RefreshCw, Search } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useToast } from "../components/feedback";
import { Switch } from "../components/TweakCard";
import { systemApi, type StartupItem } from "../lib/api";
import { BootCard } from "../components/Maintenance";
import { useOnJournalChange } from "../lib/journalEvents";
import { ErrorState, Loading, iconBtn } from "../components/ui";
import { DataTable, type Column } from "../components/DataTable";
import { withoutUserPaths } from "../lib/errors";
import { readCached, remember } from "../lib/cachedRead";

const SOURCE_LABEL = { registry: "Registro", folder: "Carpeta", task: "Tarea" };

export function Startup({ isAdmin }: { isAdmin: boolean }) {
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [hideMicrosoft, setHideMicrosoft] = useState(false);
  const toast = useToast();

  const [failed, setFailed] = useState<string | null>(null);
  /** `fromMemory`: al abrir la pantalla, lo último leído al momento (luego se lee de nuevo). */
  const load = useCallback(async (fromMemory = false) => {
    setFailed(null);
    setLoading(true);
    try {
      if (fromMemory) await readCached("startup", systemApi.listStartup, (v) => setItems(v));
      else {
        const v = await systemApi.listStartup();
        setItems(v);
        remember("startup", v);
      }
    } catch (e) {
      setFailed(String(e));
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useOnJournalChange(load);
  useEffect(() => {
    void load(true);
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

  if (!items) return failed ? <ErrorState page message={failed} onRetry={() => void load()} /> : <Loading page text="Leyendo programas de inicio…" />;

  const enabled = items.filter((i) => i.enabled).length;

  const columns: Column<StartupItem>[] = [
    {
      id: "name",
      header: "Programa",
      sortBy: (i) => i.description ?? i.name,
      className: "max-w-0 w-full",
      cell: (i) => (
        <div className="min-w-0 py-1">
          <div className="flex flex-wrap items-baseline gap-x-2">
            <span className="font-medium text-ink">{i.description ?? i.name}</span>
            {i.description && i.description !== i.name && <span className="truncate text-[11px] text-mute">{i.name}</span>}
            {!i.target && (
              <span className="flex items-center gap-1 text-[11px] text-warn" title="El ejecutable ya no existe: probablemente restos de un programa desinstalado">
                <FileWarning size={11} /> Archivo no encontrado
              </span>
            )}
          </div>
          <div className="truncate font-mono text-[11px] text-mute select-text" title={withoutUserPaths(i.command)}>
            {withoutUserPaths(i.command)}
          </div>
        </div>
      ),
    },
    {
      id: "publisher",
      header: "Editor",
      sortBy: (i) => i.publisher,
      className: "max-w-44 truncate text-xs text-dim",
      cell: (i) => <span title={i.publisher ?? undefined}>{i.publisher ?? <span className="text-mute">Editor desconocido</span>}</span>,
    },
    {
      id: "source",
      header: "Desde",
      sortBy: (i) => SOURCE_LABEL[i.source],
      className: "max-w-48 truncate text-[11px] text-mute",
      cell: (i) => (
        <span title={withoutUserPaths(i.location)}>
          <span className="mr-1.5 rounded bg-line px-1.5 py-px font-mono text-[10px]">{SOURCE_LABEL[i.source]}</span>
          {withoutUserPaths(i.location)}
        </span>
      ),
    },
    {
      id: "enabled",
      header: "Arranca",
      align: "right",
      sortBy: (i) => (i.enabled ? 1 : 0),
      stopClick: true,
      cell: (i) => {
        const blocked = i.needsAdmin && !isAdmin;
        return (
          <span className="inline-flex" title={blocked ? "Requiere administrador" : undefined}>
            <Switch on={i.enabled} disabled={blocked || busy === i.id} onClick={() => void toggle(i)} />
          </span>
        );
      },
    },
  ];

  return (
    <div className="mx-auto max-w-(--page-max) p-6">
      <BootCard />
      <div className="mb-4 flex items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{enabled}</span> de {items.length} arrancan con Windows
        </p>
        <div className="relative ml-auto w-72 min-w-40 shrink">
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
        <button onClick={() => void load()} disabled={loading} className={iconBtn} title="Volver a leer" aria-label="Volver a leer">
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      <div className="overflow-x-auto rounded-xl border border-line bg-panel">
        <DataTable
          padded
          rows={visible}
          rowKey={(i) => i.id}
          rowClass={(i) => (i.enabled ? "" : "opacity-60")}
          empty={query ? `Nada coincide con «${query}».` : "No hay programas de inicio que enseñar."}
          columns={columns}
        />
      </div>
      <p className="mt-3 text-xs text-mute">
        Desactivar no borra nada: usa el mismo mecanismo que el Administrador de tareas y se puede deshacer desde el
        Historial.
      </p>
    </div>
  );
}
