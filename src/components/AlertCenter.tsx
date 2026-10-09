import { listen } from "@tauri-apps/api/event";
import { Responsible } from "./Responsible";
import { Bell, BellOff, CheckCircle2, ChevronDown, RotateCw, Trash2, X, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { logQuietly, alertsApi, workApi, type MutedAlert, type WindowsAlert } from "../lib/api";
import { type AlertFilter, countByLevel, filterAlerts, groupByDay, LEVEL_LABEL, worstUnread } from "../lib/alerts";
import { clearActivity, markActivitySeen, resultPage, took, useActivity } from "../lib/activity";
import { useToast } from "./feedback";
import type { PageId } from "./Sidebar";
import { Button, iconBtn, Modal } from "./ui";
import { ago as when, shortDate, timeOfDay } from "../lib/format";

const DOT = { bad: "bg-bad", warn: "bg-warn", info: "bg-mute" } as Record<string, string>;
const FILTERS: AlertFilter[] = ["all", "bad", "warn", "info"];

/** Campana de la cabecera: lo que AdminOps ha detectado en Windows mientras está abierta. */
export function AlertCenter({
  onNavigate,
  onOpenChange,
  openSignal = 0,
}: {
  onNavigate: (p: PageId, section?: string | null) => void;
  onOpenChange?: (open: boolean) => void;
  /** Al cambiar (y no ser 0), abre el panel: lo usa el aviso de Windows. */
  openSignal?: number;
}) {
  const [alerts, setAlerts] = useState<WindowsAlert[]>([]);
  const [muted, setMuted] = useState<MutedAlert[]>([]);
  const [open, setOpen] = useState(false);
  const [checking, setChecking] = useState(false);
  const [filter, setFilter] = useState<AlertFilter>("all");
  const [onlyUnread, setOnlyUnread] = useState(false);
  const [showMuted, setShowMuted] = useState(false);
  // Avisos del equipo, o lo que AdminOps ha terminado en esta sesión.
  const [tab, setTab] = useState<"alerts" | "activity">("alerts");
  const activity = useActivity();
  const activityNew = activity.filter((a) => !a.seen).length;
  // Plegados o desplegados a mano; lo demás sigue la regla de `isOpen`.
  const [toggled, setToggled] = useState<Record<string, boolean>>({});
  // ¿Está la vigilancia activada en Ajustes? (null: aún no se sabe)
  const [watching, setWatching] = useState<boolean | null>(null);

  // Vuelta desde un aviso de Windows: se abre el panel con lo que hay pendiente.
  useEffect(() => {
    if (openSignal) show(true);
    // `show` cambia en cada render: solo interesa la señal.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [openSignal]);
  const toast = useToast();

  const load = useCallback(() => {
    alertsApi.list().then(setAlerts).catch(logQuietly("AlertCenter"));
  }, []);
  const loadMuted = useCallback(() => {
    alertsApi.muted().then(setMuted).catch(logQuietly("AlertCenter"));
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
      void un.then((f) => f());
    };
  }, [load, toast]);

  const show = (v: boolean) => {
    // Se abre por lo que haya pendiente: si solo hay tareas terminadas, por ahí.
    if (v) setTab(alerts.some((a) => !a.read) || !activityNew ? "alerts" : "activity");
    setOpen(v);
    onOpenChange?.(v);
    if (v) {
      loadMuted();
      workApi
        .settings()
        .then((s) => setWatching(s.watchWindows))
        .catch(logQuietly("AlertCenter"));
    } else {
      setToggled({});
      if (alerts.some((a) => !a.read)) void alertsApi.markRead().then(load);
      markActivitySeen();
    }
  };

  const unread = alerts.filter((a) => !a.read).length;
  const worst = worstUnread(alerts);
  const counts = useMemo(() => countByLevel(alerts), [alerts]);
  const groups = useMemo(() => groupByDay(filterAlerts(alerts, filter, onlyUnread)), [alerts, filter, onlyUnread]);

  // Sin leer o grave: desplegado. Lo ya visto queda en una línea.
  const isOpen = (a: WindowsAlert) => toggled[a.id] ?? (!a.read || a.level === "bad");

  const dismiss = (a: WindowsAlert) => {
    setAlerts((l) => l.filter((x) => x.id !== a.id));
    alertsApi
      .dismiss(a.id)
      .catch((e) => toast("error", String(e)))
      .finally(load);
  };
  const mute = (a: WindowsAlert) => {
    setAlerts((l) => l.filter((x) => x.key !== a.key));
    alertsApi
      .mute(a.id)
      .then(() => toast("ok", `«${a.title}» no volverá a avisar en este equipo. Se deshace en «Silenciados».`))
      .catch((e) => toast("error", String(e)))
      .finally(() => {
        load();
        loadMuted();
      });
  };
  const unmute = (m: MutedAlert) => {
    alertsApi
      .unmute(m.key)
      .then(loadMuted)
      .catch((e) => toast("error", String(e)));
  };

  return (
    <>
      <button
        onClick={() => show(true)}
        className="relative grid size-8 place-items-center rounded-md text-mute transition-colors hover:bg-panel-2 hover:text-ink"
        title={unread ? `Avisos: ${unread} sin leer` : activityNew ? `${activityNew} ${activityNew === 1 ? "tarea terminada" : "tareas terminadas"}` : "Avisos y actividad"}
        aria-label={`Avisos${unread ? `: ${unread} sin leer` : ""}${activityNew ? `. ${activityNew} tareas terminadas` : ""}`}
      >
        <Bell size={16} strokeWidth={1.6} />
        {unread === 0 && activityNew > 0 && <span className="absolute top-1 right-1 size-2 rounded-full bg-ok" />}
        {unread > 0 && (
          <span className={`absolute -top-0.5 -right-0.5 grid min-w-4 place-items-center rounded-full px-1 text-[10px] font-semibold text-white ${worst === "bad" ? "bg-bad" : "bg-warn"}`}>
            {unread > 9 ? "9+" : unread}
          </span>
        )}
      </button>
      {open && (
        <Modal
          title={
            <span className="flex items-center gap-1" role="tablist">
              {(
                [
                  ["alerts", "Avisos", unread],
                  ["activity", "Actividad", activityNew],
                ] as const
              ).map(([id, label, n]) => (
                <button
                  key={id}
                  role="tab"
                  aria-selected={tab === id}
                  onClick={() => setTab(id)}
                  className={`flex items-center gap-1.5 rounded-md px-2.5 py-1 text-sm transition-colors ${tab === id ? "bg-neon/10 text-neon" : "font-normal text-mute hover:text-ink"}`}
                >
                  {label}
                  {n > 0 && <span className="rounded-full bg-panel-2 px-1.5 text-[10.5px] text-dim">{n}</span>}
                </button>
              ))}
            </span>
          }
          onClose={() => show(false)}
          width="w-[680px]"
          footer={
            tab === "activity" ? (
              <Button kind="ghost" onClick={clearActivity} disabled={!activity.length}>
                <Trash2 size={14} /> Vaciar
              </Button>
            ) : (
            <>
              {muted.length > 0 && (
                <button onClick={() => setShowMuted((v) => !v)} className="mr-auto flex items-center gap-1.5 text-xs text-mute hover:text-ink">
                  <BellOff size={13} /> Silenciados ({muted.length})
                </button>
              )}
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
            )
          }
        >
          {tab === "activity" &&
            (activity.length === 0 ? (
              <p className="py-6 text-center text-sm text-mute">
                Todavía no ha terminado nada. Aquí queda lo que AdminOps acaba mientras miras otra cosa: análisis, copias, instalaciones… con lo que tardó y dónde ver
                el resultado. Solo de esta sesión.
              </p>
            ) : (
              <ul className="space-y-2">
                {activity.map((a) => {
                  const where = resultPage(a.task);
                  return (
                    <li key={`${a.task}-${a.at}`} className={`flex items-start gap-2.5 rounded-lg border px-3.5 py-2.5 ${a.seen ? "border-line" : "border-line-2 bg-panel-2/40"}`}>
                      {a.cancelled ? <XCircle size={15} className="mt-0.5 shrink-0 text-warn" /> : <CheckCircle2 size={15} className="mt-0.5 shrink-0 text-ok" />}
                      <div className="min-w-0 flex-1">
                        <div className="flex items-baseline gap-2">
                          <span className={`min-w-0 flex-1 truncate text-sm text-ink ${a.seen ? "" : "font-medium"}`}>{a.name}</span>
                          <span className="shrink-0 font-mono text-[11px] text-mute">{a.cancelled ? "cancelada" : took(a.seconds)}</span>
                        </div>
                        <div className="flex items-center gap-3 text-[11px] text-mute">
                          <span>{timeOfDay(a.at / 1000)}</span>
                          {where && !a.cancelled && (
                            <button
                              onClick={() => {
                                show(false);
                                onNavigate(where.page, where.section);
                              }}
                              className="text-neon hover:underline"
                            >
                              Ver resultado →
                            </button>
                          )}
                        </div>
                      </div>
                    </li>
                  );
                })}
              </ul>
            ))}
          {tab === "alerts" && (
          <>
          {watching === false && (
            <p className="mb-3 rounded-lg border border-warn/40 bg-warn/10 px-3 py-2 text-[13px] text-ink">
              La vigilancia está desactivada: no llegarán avisos nuevos.{" "}
              <button
                onClick={() => {
                  show(false);
                  onNavigate("settings", "alerts");
                }}
                className="text-neon hover:underline"
              >
                Activarla en Ajustes → Avisos
              </button>
            </p>
          )}

          {showMuted && muted.length > 0 && (
            <div className="mb-4 rounded-lg border border-line bg-panel-2/40 px-3.5 py-3">
              <div className="mb-1.5 text-xs font-medium text-mute">Silenciados en este equipo: no avisan aunque vuelvan a pasar</div>
              <ul className="divide-y divide-line">
                {muted.map((m) => (
                  <li key={m.key} className="flex items-center gap-3 py-1.5">
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] text-ink">{m.title}</div>
                      <div className="truncate text-[11px] text-mute">
                        {m.detail ? `${m.detail} · ` : ""}desde el {shortDate(m.at)}
                      </div>
                    </div>
                    <button onClick={() => unmute(m)} className="shrink-0 text-xs text-neon hover:underline">
                      Volver a avisar
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {alerts.length === 0 ? (
            <p className="py-6 text-center text-sm text-mute">
              Sin avisos. Mientras AdminOps está abierta, vigila el Visor de eventos: pantallazos azules, discos con fallos, programas que se cierran,
              drivers de vídeo, falta de memoria o de espacio, amenazas, Windows Update…
            </p>
          ) : (
            <>
              <div className="mb-3 flex flex-wrap items-center gap-1.5">
                {FILTERS.filter((f) => f === "all" || counts[f] > 0).map((f) => (
                  <button
                    key={f}
                    onClick={() => setFilter(f)}
                    aria-pressed={filter === f}
                    className={`flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs transition-colors ${
                      filter === f ? "border-neon/50 bg-neon/10 text-neon" : "border-line-2 text-dim hover:text-ink"
                    }`}
                  >
                    {f !== "all" && <span className={`size-1.5 rounded-full ${DOT[f]}`} />}
                    {f === "all" ? "Todos" : LEVEL_LABEL[f]} <span className="text-mute">{counts[f]}</span>
                  </button>
                ))}
                <label className="ml-auto flex cursor-pointer items-center gap-1.5 text-xs text-dim">
                  <input type="checkbox" checked={onlyUnread} onChange={(e) => setOnlyUnread(e.target.checked)} className="size-3.5 accent-[var(--color-neon)]" />
                  Solo sin leer
                </label>
              </div>

              {groups.length === 0 && <p className="py-6 text-center text-sm text-mute">Nada con este filtro.</p>}

              {groups.map((g) => (
                <section key={g.label} className="mb-4 last:mb-0">
                  <h4 className="mb-1.5 text-[11px] font-medium tracking-wide text-mute uppercase">{g.label}</h4>
                  <ul className="space-y-2">
                    {g.items.map((a) => {
                      const expanded = isOpen(a);
                      return (
                        <li key={a.id} className={`group rounded-lg border ${a.read ? "border-line" : "border-line-2 bg-panel-2/40"}`}>
                          <div className="flex items-start gap-2.5 px-3.5 py-2.5">
                            <span className={`mt-1.5 size-2 shrink-0 rounded-full ${DOT[a.level] ?? "bg-mute"}`} />
                            <button
                              onClick={() => setToggled((t) => ({ ...t, [a.id]: !expanded }))}
                              aria-expanded={expanded}
                              className="min-w-0 flex-1 text-left"
                            >
                              <div className="flex flex-wrap items-baseline gap-x-2">
                                <span className={`text-sm text-ink ${a.read ? "" : "font-medium"}`}>{a.title}</span>
                                {a.count > 1 && <span className="text-xs text-mute">×{a.count}</span>}
                                <span className="ml-auto text-xs text-mute" title={when(a.time)}>
                                  {timeOfDay(a.time)}
                                </span>
                              </div>
                              {a.detail && <div className="truncate font-mono text-[11px] text-dim">{a.detail}</div>}
                            </button>
                            <div className="flex shrink-0 items-center opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
                              <button onClick={() => mute(a)} className={iconBtn} title="No avisar más de esto en este equipo" aria-label="No avisar más de esto en este equipo">
                                <BellOff size={14} />
                              </button>
                              <button onClick={() => dismiss(a)} className={iconBtn} title="Descartar" aria-label="Descartar">
                                <X size={14} />
                              </button>
                            </div>
                            <ChevronDown size={14} className={`mt-1 shrink-0 text-mute transition-transform ${expanded ? "rotate-180" : ""}`} />
                          </div>
                          {expanded && (
                            <div className="border-t border-line px-3.5 py-2.5 pl-[34px]">
                              <p className="text-[13px] text-dim">{a.explanation}</p>
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
                              {a.level !== "info" && a.page && <Responsible topic={a.page} className="mt-1.5" />}
                            </div>
                          )}
                        </li>
                      );
                    })}
                  </ul>
                </section>
              ))}
            </>
          )}
          </>
          )}
        </Modal>
      )}
    </>
  );
}
