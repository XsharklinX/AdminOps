// Agenda: historial. Todo lo que ya pasó (hecho, cancelado o sin marcar), con
// búsqueda, filtros y totales por mes: qué se hizo, cuándo y cuánto tiempo.
import { Briefcase, Check, Clock, CopyPlus, ListTodo, Phone, RotateCcw, Search, Trash2, Users } from "lucide-react";
import { useMemo, useState } from "react";
import type { Followup, Visit } from "../lib/api";
import { iconBtn } from "./ui";

const KIND = {
  visit: { label: "Visita", Icon: Briefcase, text: "text-neon" },
  task: { label: "Tarea", Icon: ListTodo, text: "text-ok" },
  call: { label: "Llamada", Icon: Phone, text: "text-warn" },
  meeting: { label: "Reunión", Icon: Users, text: "text-dim" },
} as const;
type KindId = keyof typeof KIND;

type State = "done" | "cancelled" | "missed";
type Filter = "all" | State;

interface Item {
  key: string;
  /** Cuándo era (segundos). */
  at: number;
  title: string;
  detail: string;
  minutes: number;
  state: State;
  kind: KindId | "followup";
  visit?: Visit;
  followup?: Followup;
}

const startOfToday = () => {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime() / 1000;
};
const day = (ts: number) => new Date(ts * 1000).toLocaleDateString("es", { weekday: "short", day: "numeric", month: "short" });
const monthKey = (ts: number) => {
  const d = new Date(ts * 1000);
  return d.getFullYear() * 12 + d.getMonth();
};
const monthLabel = (k: number) => {
  const s = new Date(Math.floor(k / 12), k % 12, 1).toLocaleDateString("es", { month: "long", year: "numeric" });
  return s.charAt(0).toUpperCase() + s.slice(1);
};
const hours = (m: number) => (m >= 60 ? `${Math.round((m / 60) * 10) / 10} h` : `${m} min`);

/** Lo que entra en el historial: lo hecho, lo cancelado y lo que pasó sin marcarse. */
export function historyItems(visits: Visit[], followups: Followup[], today = startOfToday()): Item[] {
  const out: Item[] = [];
  for (const v of visits) {
    const state: State | null = v.status === "done" ? "done" : v.status === "cancelled" ? "cancelled" : v.start < today ? "missed" : null;
    if (!state) continue;
    out.push({
      key: `v-${v.id}`,
      at: v.start,
      title: v.title || v.clientName || "Sin título",
      detail: [v.title ? v.clientName : "", v.place].filter(Boolean).join(" · "),
      minutes: v.minutes,
      state,
      kind: (v.kind || "visit") as KindId,
      visit: v,
    });
  }
  for (const f of followups.filter((x) => x.done)) {
    out.push({ key: `f-${f.id}`, at: f.due, title: f.text, detail: [f.person, f.machine].filter(Boolean).join(" · "), minutes: 0, state: "done", kind: "followup", followup: f });
  }
  return out.sort((a, b) => b.at - a.at);
}

export function AgendaHistory({
  visits,
  followups,
  onReopen,
  onDone,
  onRepeat,
  onRemove,
  onReopenFollowup,
}: {
  visits: Visit[];
  followups: Followup[];
  /** Volver a ponerla como pendiente. */
  onReopen: (v: Visit) => void;
  onDone: (v: Visit) => void;
  /** Apuntar otra igual (abre el editor con una copia). */
  onRepeat: (v: Visit) => void;
  onRemove: (v: Visit) => void;
  onReopenFollowup: (f: Followup) => void;
}) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [kind, setKind] = useState<KindId | "followup" | "all">("all");
  const [limit, setLimit] = useState(150);

  const all = useMemo(() => historyItems(visits, followups), [visits, followups]);
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return all.filter((i) => (filter === "all" || i.state === filter) && (kind === "all" || i.kind === kind) && (!q || `${i.title} ${i.detail}`.toLowerCase().includes(q)));
  }, [all, query, filter, kind]);

  const now = new Date();
  const thisMonth = now.getFullYear() * 12 + now.getMonth();
  const doneThisMonth = all.filter((i) => i.state === "done" && monthKey(i.at) === thisMonth);
  const stats = [
    { label: "Hechas este mes", value: String(doneThisMonth.length) },
    { label: "Tiempo este mes", value: hours(doneThisMonth.reduce((n, i) => n + i.minutes, 0)) },
    { label: "Hechas este año", value: String(all.filter((i) => i.state === "done" && new Date(i.at * 1000).getFullYear() === now.getFullYear()).length) },
    { label: "Sin marcar", value: String(all.filter((i) => i.state === "missed").length), warn: true },
  ];

  const groups: [number, Item[]][] = [];
  for (const i of shown.slice(0, limit)) {
    const k = monthKey(i.at);
    const last = groups[groups.length - 1];
    if (last && last[0] === k) last[1].push(i);
    else groups.push([k, [i]]);
  }

  const chip = (on: boolean) => `rounded-full border px-2.5 py-0.5 text-xs transition-colors ${on ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`;

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {stats.map((s) => (
          <div key={s.label} className="rounded-xl border border-line bg-panel px-3 py-2.5">
            <div className={`font-mono text-xl leading-none font-semibold ${s.warn && s.value !== "0" ? "text-warn" : "text-ink"}`}>{s.value}</div>
            <div className="mt-1 text-[11px] text-mute">{s.label}</div>
          </div>
        ))}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-56 flex-1">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar en el historial…"
            className="w-full rounded-md border border-line bg-panel py-2 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        {(
          [
            ["all", "Todo"],
            ["done", "Hechas"],
            ["missed", "Sin marcar"],
            ["cancelled", "Canceladas"],
          ] as const
        ).map(([id, label]) => (
          <button key={id} onClick={() => setFilter(id)} className={chip(filter === id)}>
            {label}
          </button>
        ))}
        <select value={kind} onChange={(e) => setKind(e.target.value as typeof kind)} className="h-8 rounded-md border border-line bg-void/60 px-2 text-xs text-ink outline-none focus:border-neon/50" aria-label="Tipo">
          <option value="all">Todos los tipos</option>
          {(Object.keys(KIND) as KindId[]).map((k) => (
            <option key={k} value={k}>
              {KIND[k].label}s
            </option>
          ))}
          <option value="followup">Seguimientos</option>
        </select>
      </div>

      {shown.length === 0 ? (
        <p className="rounded-xl border border-dashed border-line px-6 py-10 text-center text-sm text-mute">
          {all.length === 0 ? "Aún no hay nada en el historial. Lo que marques como hecho o canceles aparecerá aquí." : "Nada coincide con el filtro."}
        </p>
      ) : (
        groups.map(([k, items]) => {
          const done = items.filter((i) => i.state === "done");
          return (
            <section key={k}>
              <h3 className="mb-2 flex items-baseline gap-2 text-xs font-medium text-dim">
                {monthLabel(k)}
                <span className="font-normal text-mute">
                  {done.length} {done.length === 1 ? "hecha" : "hechas"}
                  {done.some((i) => i.minutes) && ` · ${hours(done.reduce((n, i) => n + i.minutes, 0))}`}
                </span>
              </h3>
              <ul className="divide-y divide-line/60 rounded-xl border border-line bg-panel">
                {items.map((i) => {
                  const meta = i.kind === "followup" ? { Icon: Clock, text: "text-mute", label: "Seguimiento" } : KIND[i.kind];
                  return (
                    <li key={i.key} className="flex items-center gap-3 px-4 py-2">
                      <span className="w-24 shrink-0 text-xs text-mute">{day(i.at)}</span>
                      <meta.Icon size={13} className={`shrink-0 ${meta.text}`} />
                      <span className="min-w-0 flex-1">
                        <span className={`block truncate text-sm ${i.state === "cancelled" ? "text-mute line-through" : "text-ink"}`} title={i.title}>
                          {i.title}
                        </span>
                        {i.detail && <span className="block truncate text-[11px] text-mute">{i.detail}</span>}
                      </span>
                      {i.minutes > 0 && i.state === "done" && <span className="hidden shrink-0 text-[11px] text-mute sm:block">{hours(i.minutes)}</span>}
                      <span
                        className={`shrink-0 rounded-full border px-2 py-0.5 text-[10px] ${
                          i.state === "done" ? "border-ok/40 text-ok" : i.state === "missed" ? "border-warn/50 text-warn" : "border-line text-mute"
                        }`}
                        title={i.visit?.doneAt ? `Marcada como hecha el ${new Date(i.visit.doneAt * 1000).toLocaleString("es")}` : undefined}
                      >
                        {i.state === "done" ? "Hecha" : i.state === "missed" ? "Sin marcar" : "Cancelada"}
                      </span>
                      <span className="flex shrink-0 items-center">
                        {i.visit && i.state === "missed" && (
                          <button onClick={() => onDone(i.visit!)} className={`${iconBtn} hover:text-ok`} title="Se hizo: marcarla como hecha">
                            <Check size={14} />
                          </button>
                        )}
                        {i.visit && i.state !== "missed" && (
                          <button onClick={() => onReopen(i.visit!)} className={iconBtn} title="Volver a ponerla como pendiente">
                            <RotateCcw size={14} />
                          </button>
                        )}
                        {i.visit && (
                          <button onClick={() => onRepeat(i.visit!)} className={iconBtn} title="Apuntar otra igual">
                            <CopyPlus size={14} />
                          </button>
                        )}
                        {i.visit && (
                          <button onClick={() => onRemove(i.visit!)} className={`${iconBtn} hover:text-bad`} title="Borrar del historial">
                            <Trash2 size={14} />
                          </button>
                        )}
                        {i.followup && (
                          <button onClick={() => onReopenFollowup(i.followup!)} className={iconBtn} title="Volver a dejarlo pendiente">
                            <RotateCcw size={14} />
                          </button>
                        )}
                      </span>
                    </li>
                  );
                })}
              </ul>
            </section>
          );
        })
      )}
      {shown.length > limit && (
        <button onClick={() => setLimit(limit + 150)} className="text-xs text-neon hover:underline">
          Ver más antiguas ({shown.length - limit})
        </button>
      )}
    </div>
  );
}
