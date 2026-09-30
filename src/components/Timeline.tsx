import { AlertTriangle, ArrowRight, CalendarClock, Loader2, MonitorCog, RefreshCw, Stethoscope, Wrench } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Card } from "./ui";
import { isPageId, pageLabel, type PageId } from "./Sidebar";
import { timelineApi, type TimelineEvent } from "../lib/api";

const KINDS = {
  change: { label: "Cambios de AdminOps", icon: Wrench },
  alert: { label: "Avisos", icon: AlertTriangle },
  scan: { label: "Diagnósticos", icon: Stethoscope },
  windows: { label: "Windows", icon: MonitorCog },
} as const;

type Kind = keyof typeof KINDS;

const COLOR = { ok: "text-ok", info: "text-neon", warn: "text-warn", bad: "text-bad" };
const RANGES = [7, 30, 90];
const PAGE = 250;

const isPage = isPageId;
const dayKey = (t: number) => new Date(t * 1000).toLocaleDateString("es", { weekday: "long", day: "numeric", month: "long", year: "numeric" });
const hour = (t: number) => new Date(t * 1000).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
const isBoot = (e: TimelineEvent) => e.kind === "windows" && (e.title === "Windows arrancó" || e.title === "Windows se apagó");

/** Todo lo que pasó en el equipo, por días: responde a «¿qué cambió desde que funcionaba?». */
export function Timeline({ onNavigate }: { onNavigate?: (p: PageId) => void }) {
  const [days, setDays] = useState(30);
  const [events, setEvents] = useState<TimelineEvent[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [kinds, setKinds] = useState<Set<Kind>>(new Set(Object.keys(KINDS) as Kind[]));
  const [boots, setBoots] = useState(false);
  const [limit, setLimit] = useState(PAGE);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setEvents(await timelineApi.list(days));
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [days]);

  useEffect(() => {
    void load();
  }, [load]);

  const shown = useMemo(() => (events ?? []).filter((e) => kinds.has(e.kind) && (boots || !isBoot(e))), [events, kinds, boots]);
  const groups = useMemo(() => {
    const out: { day: string; items: TimelineEvent[] }[] = [];
    for (const e of shown.slice(0, limit)) {
      const d = dayKey(e.time);
      let last = out[out.length - 1];
      if (last?.day !== d) out.push((last = { day: d, items: [] }));
      last.items.push(e);
    }
    return out;
  }, [shown, limit]);

  const count = (k: Kind) => (events ?? []).filter((e) => e.kind === k && (boots || !isBoot(e))).length;
  const toggle = (k: Kind) =>
    setKinds((s) => {
      const n = new Set(s);
      if (n.has(k)) n.delete(k);
      else n.add(k);
      return n;
    });

  return (
    <Card
      title="Línea de tiempo del equipo"
      icon={<CalendarClock size={14} />}
      className="col-span-12"
      right={
        <div className="flex items-center gap-3 text-[11px]">
          <span className="flex overflow-hidden rounded-md border border-line">
            {RANGES.map((r) => (
              <button key={r} onClick={() => setDays(r)} className={`px-2 py-0.5 ${days === r ? "bg-neon/15 text-neon" : "text-mute hover:text-ink"}`}>
                {r} días
              </button>
            ))}
          </span>
          <button onClick={load} disabled={loading} className="flex items-center gap-1 text-mute hover:text-ink">
            <RefreshCw size={11} className={loading ? "animate-spin" : ""} /> Actualizar
          </button>
        </div>
      }
    >
      <p className="mb-3 text-xs text-dim">
        Cambios hechos con AdminOps, avisos, diagnósticos y lo que pasó en Windows (actualizaciones, drivers, programas, apagados bruscos). Útil para saber qué cambió desde
        que funcionaba.
      </p>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        {(Object.keys(KINDS) as Kind[]).map((k) => {
          const K = KINDS[k];
          const on = kinds.has(k);
          return (
            <button
              key={k}
              onClick={() => toggle(k)}
              className={`flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs transition-colors ${on ? "border-neon/50 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`}
            >
              <K.icon size={11} /> {K.label} <span className="text-mute">{count(k)}</span>
            </button>
          );
        })}
        <label className="ml-auto flex items-center gap-1.5 text-xs text-dim">
          <input type="checkbox" checked={boots} onChange={(e) => setBoots(e.target.checked)} className="accent-[var(--color-neon)]" />
          Arranques y apagados
        </label>
      </div>

      {error ? (
        <p className="text-sm text-bad">{error}</p>
      ) : !events ? (
        <p className="flex items-center gap-2 text-sm text-mute">
          <Loader2 size={13} className="animate-spin" /> Reuniendo lo que pasó en el equipo…
        </p>
      ) : shown.length === 0 ? (
        <p className="py-6 text-center text-sm text-mute">Nada en este periodo con estos filtros.</p>
      ) : (
        <div className="max-pane-lg space-y-4 overflow-y-auto pr-1">
          {groups.map((g) => (
            <div key={g.day}>
              <h4 className="sticky top-0 z-10 mb-1 bg-panel py-1 text-[11px] font-medium tracking-wide text-mute uppercase">{g.day}</h4>
              <ol className="space-y-0.5 border-l border-line pl-3">
                {g.items.map((e, i) => {
                  const K = KINDS[e.kind];
                  return (
                    <li key={`${e.time}-${i}`} className="group flex items-start gap-2.5 rounded-md px-2 py-1.5 hover:bg-panel-2">
                      <span className="w-10 shrink-0 pt-0.5 font-mono text-[11px] text-mute">{hour(e.time)}</span>
                      <K.icon size={13} className={`mt-0.5 shrink-0 ${COLOR[e.level]}`} />
                      <span className="min-w-0 flex-1">
                        <span className="text-sm text-ink">{e.title}</span>
                        {e.detail && <span className="block text-xs break-words text-dim select-text">{e.detail}</span>}
                      </span>
                      {onNavigate && isPage(e.page) && e.page !== "history" && (
                        <button onClick={() => onNavigate(e.page as PageId)} className="flex shrink-0 items-center gap-1 text-[11px] text-mute opacity-0 group-hover:opacity-100 hover:text-neon">
                          {pageLabel(e.page as PageId)} <ArrowRight size={10} />
                        </button>
                      )}
                    </li>
                  );
                })}
              </ol>
            </div>
          ))}
          {shown.length > limit && (
            <button onClick={() => setLimit((l) => l + PAGE)} className="w-full py-2 text-xs text-neon hover:underline">
              Mostrar más ({shown.length - limit} restantes)
            </button>
          )}
        </div>
      )}
    </Card>
  );
}
