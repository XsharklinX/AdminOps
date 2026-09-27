import { listen } from "@tauri-apps/api/event";
import { Bell, RotateCw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { alertsApi, type WindowsAlert } from "../lib/api";
import { useToast } from "./feedback";
import type { PageId } from "./Sidebar";
import { Button, Modal } from "./ui";

const DOT = { bad: "bg-bad", warn: "bg-warn", info: "bg-mute" } as Record<string, string>;

function when(ts: number) {
  const d = new Date(ts * 1000);
  const mins = Math.floor((Date.now() - d.getTime()) / 60000);
  if (mins < 1) return "ahora";
  if (mins < 60) return `hace ${mins} min`;
  if (mins < 24 * 60) return `hace ${Math.floor(mins / 60)} h`;
  return d.toLocaleString("es", { dateStyle: "short", timeStyle: "short" });
}

/** Campana de la cabecera: errores de Windows detectados mientras AdminOps está abierta. */
export function AlertCenter({ onNavigate, onOpenChange }: { onNavigate: (p: PageId) => void; onOpenChange?: (open: boolean) => void }) {
  const [alerts, setAlerts] = useState<WindowsAlert[]>([]);
  const [open, setOpen] = useState(false);
  const [checking, setChecking] = useState(false);
  const toast = useToast();

  const load = useCallback(() => {
    alertsApi.list().then(setAlerts).catch(() => {});
  }, []);

  useEffect(() => {
    load();
    // Varios a la vez (la primera pasada mira las últimas 24 h): un solo mensaje.
    let batch: WindowsAlert[] = [];
    let timer: number | undefined;
    const un = listen<WindowsAlert>("windows-alert", (e) => {
      batch.push(e.payload);
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        load();
        const worst = batch.find((a) => a.level === "bad") ?? batch[0];
        const more = batch.length > 1 ? ` (y ${batch.length - 1} más en la campana)` : "";
        toast(worst.level === "bad" ? "error" : "info", `${worst.title}${worst.detail ? ` · ${worst.detail}` : ""}${more}`);
        batch = [];
      }, 1500);
    });
    return () => {
      window.clearTimeout(timer);
      un.then((f) => f());
    };
  }, [load, toast]);

  const show = (v: boolean) => {
    setOpen(v);
    onOpenChange?.(v);
    if (!v && alerts.some((a) => !a.read)) alertsApi.markRead().then(load);
  };

  const unread = alerts.filter((a) => !a.read).length;
  const worst = alerts.find((a) => !a.read && a.level === "bad") ? "bad" : unread ? "warn" : null;

  return (
    <>
      <button
        onClick={() => show(true)}
        className="relative grid size-8 place-items-center rounded-md text-mute transition-colors hover:bg-panel-2 hover:text-ink"
        title="Errores de Windows detectados"
        aria-label={`Avisos de Windows${unread ? `: ${unread} sin leer` : ""}`}
      >
        <Bell size={16} strokeWidth={1.6} />
        {unread > 0 && (
          <span className={`absolute -top-0.5 -right-0.5 grid min-w-4 place-items-center rounded-full px-1 text-[10px] font-semibold text-white ${worst === "bad" ? "bg-bad" : "bg-warn"}`}>
            {unread > 9 ? "9+" : unread}
          </span>
        )}
      </button>
      {open && (
        <Modal
          title="Errores de Windows detectados"
          onClose={() => show(false)}
          width="w-[640px]"
          footer={
            <>
              <Button
                kind="ghost"
                onClick={() => {
                  setChecking(true);
                  alertsApi
                    .checkNow()
                    .then((n) => {
                      load();
                      toast("ok", n.length ? `${n.length} avisos nuevos.` : "Nada nuevo.");
                    })
                    .catch((e) => toast("error", String(e)))
                    .finally(() => setChecking(false));
                }}
                disabled={checking}
              >
                <RotateCw size={14} className={checking ? "animate-spin" : ""} /> Comprobar ahora
              </Button>
              <Button kind="ghost" onClick={() => alertsApi.clear().then(load)} disabled={!alerts.length}>
                <Trash2 size={14} /> Vaciar
              </Button>
            </>
          }
        >
          {alerts.length === 0 ? (
            <p className="py-6 text-center text-sm text-mute">
              Sin avisos. Mientras AdminOps está abierta, revisa cada minuto el Visor de eventos: pantallazos azules, discos con fallos, programas que se
              cierran, drivers de vídeo, falta de memoria o de espacio, amenazas, Windows Update…
            </p>
          ) : (
            <ul className="space-y-3">
              {alerts.map((a) => (
                <li key={a.id} className={`rounded-lg border px-3.5 py-3 ${a.read ? "border-line" : "border-line-2 bg-panel-2/40"}`}>
                  <div className="flex items-start gap-2.5">
                    <span className={`mt-1.5 size-2 shrink-0 rounded-full ${DOT[a.level] ?? "bg-mute"}`} />
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-baseline gap-x-2">
                        <span className="text-sm font-medium text-ink">{a.title}</span>
                        {a.count > 1 && <span className="text-xs text-mute">×{a.count}</span>}
                        <span className="ml-auto text-xs text-mute">{when(a.time)}</span>
                      </div>
                      {a.detail && <div className="truncate font-mono text-[11px] text-dim">{a.detail}</div>}
                      <p className="mt-1 text-[13px] text-dim">{a.explanation}</p>
                      <p className="mt-1 text-[13px] text-ink">
                        <span className="text-mute">Qué hacer: </span>
                        {a.advice}
                      </p>
                      {a.page && (
                        <button
                          onClick={() => {
                            show(false);
                            onNavigate(a.page as PageId);
                          }}
                          className="mt-1.5 text-xs text-neon hover:underline"
                        >
                          Ir a revisarlo →
                        </button>
                      )}
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Modal>
      )}
    </>
  );
}
