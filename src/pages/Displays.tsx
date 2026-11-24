import { listen } from "@tauri-apps/api/event";
import { Monitor as MonitorIcon, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { ViewCommand } from "../components/ActionCommand";
import { DataTable, type Column } from "../components/DataTable";
import { useToast } from "../components/feedback";
import { Button, Card, ErrorState, Loading, iconBtn } from "../components/ui";
import { displaysApi, type DisplayList, type DisplayMonitor } from "../lib/api";
import { layoutBoxes, monitorLine, monitorState, nativeFixLabel, nativeSize, secondsLeft, stateSentence, STATE_LABEL, summary, type MonitorState } from "../lib/displays";
import { usePageActive } from "../lib/pageActive";
import { AUDIT_TITLE, useAuditOn } from "../lib/useAudit";

const STATE_CLASS: Record<MonitorState, string> = { ok: "text-ok", blurry: "text-warn", oddScale: "text-warn", lowHz: "text-warn" };
const BOX_CLASS: Record<MonitorState, string> = { ok: "border-line-2", blurry: "border-warn", oddScale: "border-warn", lowHz: "border-warn" };

/** El dibujo de la disposición: un rectángulo por monitor, a escala y en su sitio. */
function Layout({ monitors }: { monitors: DisplayMonitor[] }) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setWidth(el.clientWidth);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const boxes = layoutBoxes(monitors, width, 176);
  return (
    <div ref={ref} className="relative h-44 w-full overflow-hidden rounded-lg bg-void/60" role="img" aria-label="Disposición de las pantallas">
      {boxes.map((b) => (
        <div
          key={b.id}
          className={`absolute flex flex-col items-center justify-center overflow-hidden rounded-md border bg-panel-2 text-ink ${b.primary ? "border-2 border-neon" : BOX_CLASS[b.state]}`}
          style={{ left: b.left, top: b.top, width: b.width, height: b.height }}
          title={monitors.find((m) => m.id === b.id)?.name}
        >
          <span className="text-lg font-semibold">{b.number || ""}</span>
          {b.primary && <span className="text-[11px] text-neon">Principal</span>}
        </div>
      ))}
    </div>
  );
}

/** Estado de pantallas: cómo va cada monitor y los arreglos con red (15 s para confirmar). */
export function Displays() {
  const [list, setList] = useState<DisplayList | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<{ untilMs: number; title: string } | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const active = usePageActive();
  const audit = useAuditOn();
  const refreshTimer = useRef(0);

  const load = useCallback(async () => {
    try {
      const l = await displaysApi.list();
      setList(l);
      setPending(l.pending);
      setError(null);
      setNow(Date.now());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Al volver a la pestaña la lista puede haber cambiado sin que nadie la mirara.
  useEffect(() => {
    if (active) void load();
  }, [active, load]);

  // Windows avisa de que cambió la pantalla; y el programa avisa si tuvo que deshacer un cambio solo.
  useEffect(() => {
    const offs = [
      listen("displays-changed", () => {
        window.clearTimeout(refreshTimer.current);
        refreshTimer.current = window.setTimeout(() => void load(), 500);
      }),
      listen<{ ok: boolean; message: string }>("display-reverted", (e) => {
        toast(e.payload.ok ? "info" : "error", e.payload.message);
        setPending(null);
        void load();
      }),
    ];
    return () => {
      window.clearTimeout(refreshTimer.current);
      for (const off of offs) void off.then((f) => f());
    };
  }, [load, toast]);

  // La cuenta atrás se dibuja aquí, pero la que manda corre en Rust. Si llegó a cero y el aviso del
  // programa no se vio, se vuelve a leer el estado (una sola vez).
  useEffect(() => {
    if (!pending || !active) return;
    const t = window.setInterval(() => {
      const at = Date.now();
      setNow(at);
      if (at > pending.untilMs + 1500) {
        window.clearInterval(t);
        void load();
      }
    }, 250);
    return () => window.clearInterval(t);
  }, [pending, active, load]);

  const run = async (what: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    try {
      await what();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
      await load();
    }
  };

  const change = (action: "native" | "extend" | "clone", id?: string) =>
    run(async () => {
      const c = await displaysApi.apply(action, id);
      setPending({ untilMs: c.untilMs, title: c.message });
      setNow(Date.now());
    });
  const confirm = () =>
    run(async () => {
      toast("ok", await displaysApi.confirm());
      setPending(null);
    });
  const undo = () =>
    run(async () => {
      toast("ok", await displaysApi.undo());
      setPending(null);
    });
  const restartDriver = () =>
    run(async () => {
      toast("ok", await displaysApi.restartDriver());
    });

  if (error && !list) return <ErrorState message={error} onRetry={() => void load()} where="Estado del equipo · Pantallas" />;
  if (!list) return <Loading text="Leyendo las pantallas del equipo…" page />;

  const locked = audit || busy || !!pending;
  const lockTitle = audit ? AUDIT_TITLE : undefined;

  const columns: Column<DisplayMonitor>[] = [
    {
      id: "monitor",
      header: "Monitor",
      sortBy: (m) => m.number,
      cell: (m) => {
        const sentence = stateSentence(m);
        return (
          <div className="min-w-0">
            <div className="text-ink">{monitorLine(m)}</div>
            {sentence && <div className="text-xs text-dim">{sentence}</div>}
          </div>
        );
      },
    },
    {
      id: "state",
      header: "Estado",
      sortBy: (m) => monitorState(m),
      className: (m) => `whitespace-nowrap ${STATE_CLASS[monitorState(m)]}`,
      cell: (m) => STATE_LABEL[monitorState(m)],
    },
    {
      id: "actions",
      header: "",
      align: "right",
      stopClick: true,
      cell: (m) => {
        const label = nativeFixLabel(m);
        const size = nativeSize(m);
        if (!label || !size) return null;
        const first = list.monitors.find((x) => nativeFixLabel(x))?.id === m.id;
        return (
          <div className="flex items-center justify-end gap-1">
            <Button kt={first ? "P" : undefined} size="sm" disabled={locked} title={lockTitle} onClick={() => change("native", m.id)}>
              {label}
            </Button>
            <ViewCommand id="display.native" context={size} icon />
          </div>
        );
      },
    },
  ];

  const secs = pending ? secondsLeft(pending.untilMs, now) : 0;
  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="text-sm text-dim">{summary(list.monitors, list.duplicated)}</p>
        <div className="ml-auto">
          <button onClick={() => void load()} className={iconBtn} title="Volver a leer" aria-label="Volver a leer">
            <RefreshCw size={14} />
          </button>
        </div>
      </div>

      {pending && (
        <div className="flex flex-wrap items-center gap-3 rounded-lg border border-neon/40 bg-neon/5 px-3 py-2 text-sm" role="alert">
          <span className="text-ink">
            ¿Se ve bien? Si no confirmas, vuelve como estaba en {secs} s.
          </span>
          <div className="ml-auto flex items-center gap-1">
            <Button kt="M" size="sm" disabled={busy || audit} title={lockTitle} onClick={confirm}>
              Mantener
            </Button>
            <Button kt="D" kind="secondary" size="sm" disabled={busy || audit} title={lockTitle} onClick={undo}>
              Deshacer
            </Button>
          </div>
        </div>
      )}

      <Card title="Disposición" icon={<MonitorIcon size={14} />}>
        <Layout monitors={list.monitors} />
      </Card>

      <DataTable padded live rows={list.monitors} rowKey={(m) => m.id} columns={columns} empty="Windows no tiene ninguna pantalla activa." />

      <Card title="Arreglos">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
          {list.monitors.length > 1 && (
            <>
              <span className="flex items-center gap-1">
                <Button kt="E" kind="secondary" size="sm" disabled={locked} title={lockTitle} onClick={() => change("extend")}>
                  Extender
                </Button>
                <ViewCommand id="display.extend" icon />
              </span>
              <span className="flex items-center gap-1">
                <Button kt="U" kind="secondary" size="sm" disabled={locked} title={lockTitle} onClick={() => change("clone")}>
                  Duplicar
                </Button>
                <ViewCommand id="display.clone" icon />
              </span>
            </>
          )}
          <span className="flex items-center gap-1">
            <Button kt="R" kind="secondary" size="sm" disabled={locked} title={lockTitle} onClick={restartDriver}>
              Reiniciar el driver gráfico
            </Button>
            <ViewCommand id="display.restart-driver" icon />
          </span>
        </div>
      </Card>
    </div>
  );
}
