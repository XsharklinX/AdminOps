import { listen } from "@tauri-apps/api/event";
import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { ViewCommand } from "../components/ActionCommand";
import { NeedsAdmin } from "../components/AdminBanner";
import { DataTable, type Column } from "../components/DataTable";
import { useToast } from "../components/feedback";
import { SavedViews } from "../components/SavedViews";
import { Button, ErrorState, Loading, Toggle, iconBtn } from "../components/ui";
import { connectedApi, type ConnectedDevice, type ConnectedList, type RecentChange } from "../lib/api";
import { ACTION_LABEL, actionsFor, ago, buildRows, summary, type ConnectedRow, type DeviceAction } from "../lib/connected";
import { usePageActive } from "../lib/pageActive";

const STATUS = {
  ok: { label: "Bien", cls: "text-ok" },
  noDriver: { label: "Sin driver", cls: "text-warn" },
  error: { label: "Con error", cls: "text-bad" },
} as const;

/** Hardware → Conectados: lo que Windows tiene presente en este equipo y si funciona. */
export function Connected({ isAdmin }: { isAdmin: boolean }) {
  const [list, setList] = useState<ConnectedList | null>(null);
  const [recent, setRecent] = useState<RecentChange[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);
  // Dispositivos con una acción en marcha: sus botones quedan desactivados (el pulsado enseña el spinner).
  const [busy, setBusy] = useState<ReadonlySet<string>>(new Set());
  const [now, setNow] = useState(() => Date.now());
  const toast = useToast();
  const active = usePageActive();
  const showAllRef = useRef(showAll);
  showAllRef.current = showAll;
  const busyRef = useRef<ReadonlySet<string>>(new Set());

  const load = useCallback(async () => {
    try {
      const [l, r] = await Promise.all([connectedApi.list(showAllRef.current), connectedApi.recent()]);
      setList(l);
      setRecent(r);
      setError(null);
      setNow(Date.now());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, showAll]);

  // Al enchufar o quitar algo, la lista se vuelve a leer sola.
  useEffect(() => {
    const offs = (["device-arrived", "device-removed"] as const).map((ev) => listen(ev, () => void load()));
    return () => {
      for (const off of offs) void off.then((f) => f());
    };
  }, [load]);

  // Al volver a la pestaña la lista puede haber cambiado sin que nadie la mirara.
  useEffect(() => {
    if (active) void load();
  }, [active, load]);

  const rows = list ? buildRows(list.devices, recent, now) : [];
  const hasFresh = rows.some((r) => r.fresh);
  // El «hace n s» corre mientras haya algo resaltado.
  useEffect(() => {
    if (!active || !hasFresh) return;
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, [active, hasFresh]);

  const act = async (action: DeviceAction, d: ConnectedDevice) => {
    if (busyRef.current.has(d.id)) return;
    busyRef.current = new Set(busyRef.current).add(d.id);
    setBusy(busyRef.current);
    try {
      toast("ok", await connectedApi.action(action, d.id));
    } catch (e) {
      toast("error", String(e));
    } finally {
      const next = new Set(busyRef.current);
      next.delete(d.id);
      busyRef.current = next;
      setBusy(next);
      await load();
    }
  };

  if (error && !list) return <ErrorState message={error} onRetry={() => void load()} where="Hardware · Conectados" />;
  if (!list) return <Loading text="Leyendo los dispositivos del equipo…" page />;

  const columns: Column<ConnectedRow>[] = [
    {
      id: "name",
      header: "Dispositivo",
      sortBy: (r) => r.device.name,
      cell: ({ device: d, fresh }) => (
        <div className="min-w-0">
          <div className="truncate text-ink" title={d.id}>
            {d.name}
          </div>
          {fresh?.kind === "arrived" && <div className="text-xs text-neon">Recién conectado · {ago(now - fresh.at)}</div>}
          {fresh?.kind === "removed" && <div className="text-xs text-mute">Recién quitado · {ago(now - fresh.at)}</div>}
          {fresh?.kind !== "removed" && d.status !== "ok" && <div className="text-xs text-dim">{d.problem}</div>}
        </div>
      ),
    },
    { id: "class", header: "Clase", sortBy: (r) => r.device.class, className: "whitespace-nowrap text-dim", cell: (r) => r.device.class },
    { id: "maker", header: "Fabricante", hidden: true, sortBy: (r) => r.device.manufacturer, className: "text-dim", cell: (r) => r.device.manufacturer || "—" },
    {
      id: "driver",
      header: "Driver",
      sortBy: (r) => r.device.driverDate ?? "",
      className: "whitespace-nowrap font-mono text-xs text-dim",
      cell: ({ device: d }) => (d.driverVersion ? `${d.driverVersion}${d.driverDate ? ` · ${d.driverDate}` : ""}` : "—"),
    },
    {
      id: "status",
      header: "Estado",
      sortBy: (r) => ({ error: 0, noDriver: 1, ok: 2 })[r.device.status],
      className: (r) => `whitespace-nowrap ${STATUS[r.device.status].cls}`,
      cell: (r) => STATUS[r.device.status].label,
    },
    {
      id: "actions",
      header: "",
      align: "right",
      stopClick: true,
      cell: ({ device: d, fresh }) => {
        if (fresh?.kind === "removed") return null;
        const actions = actionsFor(d);
        return (
          <div className="flex items-center justify-end gap-1">
            {actions.map((a, i) => (
              <Button key={a} kind={i === 0 ? "primary" : "secondary"} size="sm" disabled={!isAdmin || busy.has(d.id)} title={isAdmin ? undefined : "Requiere administrador"} onClick={() => act(a, d)}>
                {ACTION_LABEL[a]}
              </Button>
            ))}
            {actions[0] && <ViewCommand id={`device.${actions[0]}`} context={d.id} icon />}
          </div>
        );
      },
    },
  ];

  const problems = list.devices.some((d) => d.status !== "ok");
  return (
    <div className="mx-auto max-w-(--page-max) p-6">
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <p className="text-sm text-dim">{summary(list.devices)}</p>
        <SavedViews scope="connected" table="connected" current={{ showAll }} isDefault={!showAll} onApply={(v) => setShowAll(v.showAll)} />
        <div className="ml-auto flex flex-wrap items-center justify-end gap-3">
          <Toggle checked={showAll} onChange={setShowAll} label="Ver todos" />
          <button onClick={() => void load()} className={iconBtn} title="Volver a leer" aria-label="Volver a leer">
            <RefreshCw size={14} />
          </button>
        </div>
      </div>
      {problems && !isAdmin && <NeedsAdmin className="mb-3">Buscar el driver, volver a detectar y habilitar un dispositivo necesitan administrador.</NeedsAdmin>}
      {!showAll && list.hidden > 0 && <p className="mb-2 text-xs text-mute">Hay {list.hidden} dispositivos del sistema ocultos (raíz, ACPI, volúmenes…). «Ver todos» los enseña.</p>}
      <DataTable
        padded
        live
        resizable="connected"
        rows={rows}
        rowKey={(r) => r.device.id}
        columns={columns}
        rowClass={(r) => (r.fresh?.kind === "arrived" ? "bg-neon/5" : r.fresh?.kind === "removed" ? "opacity-50" : "")}
        empty="No hay dispositivos que enseñar."
      />
    </div>
  );
}
