import { listen } from "@tauri-apps/api/event";
import { Loader2, Moon, Play, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { logQuietly } from "../lib/api";
import { goToPage } from "../lib/navigate";
import { nightApi, type NightState } from "../lib/nightJobs";
import { useToast } from "./feedback";
import { api } from "../lib/api";
import { Card, Notice } from "./ui";

const DOT = { ok: "bg-ok", warn: "bg-warn", bad: "bg-bad" } as const;

/**
 * El Panel: lo que pasó esta noche (si hay algo sin ver) y lo apuntado para la
 * próxima. Sin nada apuntado ni resultados, no ocupa sitio.
 */
export function NightCard() {
  const [s, setS] = useState<NightState | null>(null);
  const toast = useToast();
  const load = useCallback(() => {
    nightApi.get().then(setS).catch(logQuietly("NightCard"));
  }, []);
  useEffect(() => {
    load();
    const un = listen("night-jobs", load);
    return () => void un.then((f) => f());
  }, [load]);

  const [elevated, setElevated] = useState<boolean | null>(null);
  useEffect(() => {
    api.isAdmin().then(setElevated, () => setElevated(null));
  }, []);
  if (!s || (s.queue.length === 0 && s.unseen === 0 && !s.running)) return null;
  // Necesitan administrador: en modo usuario esperan a una noche con AdminOps en modo técnico.
  const waiting = s.queue.length > 0 && elevated === false;
  const fresh = s.results.slice(0, s.unseen);

  return (
    <Card title={fresh.length ? "Esto pasó esta noche" : "Para esta noche"} icon={<Moon size={14} />}>
      {waiting && (
        <Notice tone="warn" className="mb-3">
          Se hacen de noche con AdminOps abierta en modo técnico (necesitan administrador). En modo usuario esperan.
        </Notice>
      )}
      {fresh.length > 0 && (
        <ul className="mb-3 space-y-2">
          {fresh.map((r) => (
            <li key={r.id} className={`rounded-xl border border-line/70 px-3.5 py-2.5 ${r.level === "ok" ? "" : `tone-${r.level}`}`}>
              <div className="flex items-center gap-2">
                <span className={`size-2 shrink-0 rounded-full ${DOT[r.level]}`} />
                <span className="min-w-0 flex-1 truncate text-[13.5px] font-medium text-ink">{r.title}</span>
                <button onClick={() => goToPage(r.page, r.section)} className="text-xs text-neon hover:underline">
                  Ver
                </button>
              </div>
              <p className="mt-0.5 pl-4 text-xs text-dim">{r.summary}</p>
            </li>
          ))}
          <li>
            <button onClick={() => void nightApi.seen().then(load)} className="text-xs text-mute hover:text-ink">
              Visto
            </button>
          </li>
        </ul>
      )}
      {s.running && (
        <p className="mb-2 flex items-center gap-2 text-[13px] text-neon">
          <Loader2 size={13} className="animate-spin" /> Ahora: {s.running}
        </p>
      )}
      {s.queue.length > 0 && (
        <>
          <div className="mb-1.5 flex flex-wrap items-center gap-2 text-xs text-mute">
            <span>Empieza a las</span>
            <input
              type="time"
              value={s.at}
              onChange={(e) => void nightApi.setTime(e.target.value).then(setS).catch((err) => toast("error", String(err)))}
              className="rounded-md border border-line bg-void/60 px-1.5 py-0.5 font-mono text-xs text-ink"
              aria-label="Hora a la que empiezan los trabajos de noche"
            />
            <span>· AdminOps tiene que quedarse abierta; el equipo no se suspenderá mientras trabaja.</span>
            <button
              onClick={() => void nightApi.runNow().then(() => toast("info", "Empezando ahora."), (e) => toast("error", String(e)))}
              className="ml-auto flex items-center gap-1 rounded-md border border-line-2 px-2 py-0.5 text-dim hover:text-ink"
            >
              <Play size={11} /> Empezar ya
            </button>
          </div>
          <ul className="divide-y divide-line/60">
            {s.queue.map((q) => (
              <li key={q.id} className="flex items-center gap-2 py-1.5 text-[13px]">
                <Moon size={12} className="shrink-0 text-mute" />
                <span className="min-w-0 flex-1 truncate text-ink">{q.title}</span>
                <button onClick={() => void nightApi.remove(q.id).then(setS)} className="rounded p-0.5 text-mute hover:text-ink" title="Quitar" aria-label={`Quitar «${q.title}»`}>
                  <X size={13} />
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </Card>
  );
}
