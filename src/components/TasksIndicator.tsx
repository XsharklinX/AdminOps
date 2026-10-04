import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, ListChecks, Loader2, Square, XCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { appApi } from "../lib/api";
import { useToast } from "./feedback";

interface Running {
  task: string;
  name: string;
  message: string;
  started: number;
}

interface Done {
  task: string;
  name: string;
  seconds: number;
  cancelled: boolean;
  at: number;
}

const human = (s: number) => (s >= 60 ? `${Math.floor(s / 60)} min ${s % 60} s` : `${s} s`);

/**
 * Tareas largas en curso desde cualquier página (instalar, desinstalar,
 * actualizar, diagnosticar…): qué hacen, cuánto llevan y cancelarlas.
 */
export function TasksIndicator() {
  const toast = useToast();
  const [running, setRunning] = useState<Record<string, Running>>({});
  const [done, setDone] = useState<Done[]>([]);
  const [open, setOpen] = useState(false);
  const [, tick] = useState(0);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const offs = [
      listen<{ task: string; name: string }>("task-started", ({ payload }) =>
        setRunning((r) => ({ ...r, [payload.task]: { task: payload.task, name: payload.name, message: r[payload.task]?.message ?? "", started: r[payload.task]?.started ?? Date.now() } })),
      ),
      listen<{ task: string; message: string }>("task-progress", ({ payload }) =>
        setRunning((r) => (r[payload.task] ? { ...r, [payload.task]: { ...r[payload.task], message: payload.message } } : r)),
      ),
      listen<{ task: string; name: string; seconds: number; cancelled: boolean }>("task-finished", ({ payload }) => {
        setRunning((r) => {
          const n = { ...r };
          delete n[payload.task];
          return n;
        });
        // Las muy cortas (buscar en la red, leer algo) no hace falta recordarlas.
        if (payload.seconds >= 3) setDone((d) => [{ ...payload, at: Date.now() }, ...d].slice(0, 6));
      }),
    ];
    return () => offs.forEach((o) => o.then((f) => f()));
  }, []);

  const list = Object.values(running).sort((a, b) => a.started - b.started);

  // Reloj mientras hay tareas y el panel está abierto.
  useEffect(() => {
    if (!open || !list.length) return;
    const t = window.setInterval(() => tick((n) => n + 1), 1000);
    return () => window.clearInterval(t);
  }, [open, list.length]);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  if (!list.length && !done.length) return null;

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className={`flex items-center gap-1.5 rounded-md px-2 py-1 text-xs transition-colors ${list.length ? "bg-neon/10 text-neon" : "text-mute hover:bg-panel-2 hover:text-ink"}`}
        title="Tareas en segundo plano"
      >
        {list.length ? <Loader2 size={13} className="animate-spin" /> : <ListChecks size={13} />}
        {list.length ? `${list.length} ${list.length === 1 ? "tarea" : "tareas"}` : "Tareas"}
      </button>
      {open && (
        <div className="absolute top-full right-0 z-40 mt-2 w-96 rounded-xl border border-line-2 bg-panel p-3 shadow-2xl">
          {list.length > 0 && (
            <>
              <p className="mb-1.5 text-[11px] font-medium tracking-wide text-mute uppercase">En curso</p>
              <ul className="mb-3 space-y-2">
                {list.map((t) => (
                  <li key={t.task} className="flex items-start gap-2.5">
                    <Loader2 size={14} className="mt-0.5 shrink-0 animate-spin text-neon" />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm text-ink">{t.name}</span>
                      <span className="block truncate text-xs text-dim">{t.message || "Trabajando…"}</span>
                      <span className="font-mono text-[11px] text-mute">{human(Math.floor((Date.now() - t.started) / 1000))}</span>
                    </span>
                    <button onClick={() => appApi.cancelTask(t.task).catch((e) => toast("error", String(e)))} className="shrink-0 rounded p-1 text-mute hover:text-bad" title="Cancelar">
                      <Square size={12} />
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
          {done.length > 0 && (
            <>
              <p className="mb-1.5 text-[11px] font-medium tracking-wide text-mute uppercase">Terminadas</p>
              <ul className="space-y-1">
                {done.map((d) => (
                  <li key={`${d.task}-${d.at}`} className="flex items-center gap-2 text-xs">
                    {d.cancelled ? <XCircle size={12} className="shrink-0 text-warn" /> : <CheckCircle2 size={12} className="shrink-0 text-ok" />}
                    <span className="min-w-0 flex-1 truncate text-dim">{d.name}</span>
                    <span className="shrink-0 font-mono text-[11px] text-mute">{d.cancelled ? "cancelada" : human(d.seconds)}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
          <p className="mt-2 border-t border-line pt-2 text-[11px] text-mute">Puedes seguir trabajando en otras páginas mientras terminan.</p>
        </div>
      )}
    </div>
  );
}
