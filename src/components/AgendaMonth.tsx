// Agenda: vista de mes. Seis semanas de lunes a domingo, con lo de cada día
// (con el color de su tipo). Para cambiar algo de día basta con arrastrarlo
// a otro.
import { ChevronLeft, ChevronRight, Clock } from "lucide-react";
import { useState } from "react";
import type { Followup, Visit } from "../lib/api";

const DAY_MS = 86_400_000;
const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
const keyOf = (ts: number) => startOfDay(new Date(ts * 1000)).getTime();
const time = (ts: number) => new Date(ts * 1000).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });

/** Primer lunes de la cuadrícula del mes (y los 42 días que enseña). */
export function monthGrid(month: Date): number[] {
  const first = new Date(month.getFullYear(), month.getMonth(), 1);
  const offset = (first.getDay() + 6) % 7; // lunes = 0
  const start = new Date(first.getFullYear(), first.getMonth(), 1 - offset);
  return Array.from({ length: 42 }, (_, i) => new Date(start.getFullYear(), start.getMonth(), start.getDate() + i).getTime());
}

/** Días entre dos claves de día (aguanta los cambios de hora). */
export const daysBetween = (from: number, to: number) => Math.round((to - from) / DAY_MS);

const BAR: Record<string, string> = { visit: "bg-neon", task: "bg-ok", call: "bg-warn", meeting: "bg-dim", "": "bg-neon" };

export function MonthView({
  month,
  onMonth,
  visits,
  followups,
  onMove,
  onOpenDay,
  onAdd,
  onEdit,
}: {
  month: Date;
  onMonth: (d: Date) => void;
  visits: Visit[];
  followups: Followup[];
  onMove: (v: Visit, toDay: number) => void;
  onOpenDay: (day: number) => void;
  onAdd: (day: number) => void;
  onEdit: (v: Visit) => void;
}) {
  const [over, setOver] = useState<number | null>(null);
  const days = monthGrid(month);
  const today = startOfDay(new Date()).getTime();
  const byDay = (k: number) => ({
    v: visits.filter((x) => keyOf(x.start) === k).sort((a, b) => a.start - b.start),
    f: followups.filter((x) => keyOf(x.due) === k),
  });
  const title = month.toLocaleDateString("es", { month: "long", year: "numeric" });

  return (
    <div className="rounded-xl border border-line bg-panel">
      <header className="flex items-center gap-2 px-4 py-3">
        <button onClick={() => onMonth(new Date(month.getFullYear(), month.getMonth() - 1, 1))} className="rounded-md p-1 text-mute hover:bg-panel-2 hover:text-ink" title="Mes anterior">
          <ChevronLeft size={16} />
        </button>
        <h3 className="min-w-40 text-center text-sm font-semibold text-ink capitalize">{title}</h3>
        <button onClick={() => onMonth(new Date(month.getFullYear(), month.getMonth() + 1, 1))} className="rounded-md p-1 text-mute hover:bg-panel-2 hover:text-ink" title="Mes siguiente">
          <ChevronRight size={16} />
        </button>
        <button onClick={() => onMonth(new Date())} className="ml-2 rounded-md px-2 py-1 text-xs text-mute hover:bg-panel-2 hover:text-ink">
          Hoy
        </button>
        <span className="ml-auto text-[11px] text-mute">Arrastra una cosa a otro día para moverla</span>
      </header>
      <div className="grid grid-cols-7 border-t border-line text-center text-[10px] tracking-wide text-mute uppercase">
        {["lun", "mar", "mié", "jue", "vie", "sáb", "dom"].map((d) => (
          <div key={d} className="py-1.5">
            {d}
          </div>
        ))}
      </div>
      <div className="grid grid-cols-7 border-t border-line">
        {days.map((k) => {
          const d = new Date(k);
          const inMonth = d.getMonth() === month.getMonth();
          const { v, f } = byDay(k);
          const shownV = v.slice(0, 3);
          const more = v.length - shownV.length;
          return (
            <div
              key={k}
              onDragOver={(e) => {
                e.preventDefault();
                setOver(k);
              }}
              onDragLeave={() => setOver((x) => (x === k ? null : x))}
              onDrop={(e) => {
                e.preventDefault();
                setOver(null);
                const id = e.dataTransfer.getData("text/agenda-id");
                const visit = visits.find((x) => x.id === id);
                if (visit && keyOf(visit.start) !== k) onMove(visit, k);
              }}
              onDoubleClick={() => onAdd(k)}
              className={`min-h-24 min-w-0 border-r border-b border-line/60 p-1 text-left transition-colors [&:nth-child(7n)]:border-r-0 ${inMonth ? "" : "bg-void/30"} ${over === k ? "bg-neon/10" : ""}`}
              title="Doble clic: apuntar algo este día"
            >
              <div className="mb-0.5 flex items-center justify-between px-1">
                <span className={`font-mono text-[11px] ${k === today ? "grid size-5 place-items-center rounded-full bg-neon text-void" : inMonth ? "text-dim" : "text-mute/60"}`}>{d.getDate()}</span>
                {f.length > 0 && (
                  <span className="flex items-center gap-0.5 text-[10px] text-mute" title={f.map((x) => x.text).join(" · ")}>
                    <Clock size={9} /> {f.length}
                  </span>
                )}
              </div>
              <ul className="space-y-0.5">
                {shownV.map((x) => (
                  <li key={x.id}>
                    <button
                      draggable
                      onDragStart={(e) => {
                        e.dataTransfer.setData("text/agenda-id", x.id);
                        e.dataTransfer.effectAllowed = "move";
                      }}
                      onClick={() => onEdit(x)}
                      className={`flex w-full min-w-0 cursor-grab items-center gap-1 rounded px-1 py-0.5 text-left text-[11px] hover:bg-panel-2 active:cursor-grabbing ${x.status === "done" ? "line-through opacity-60" : ""}`}
                      title={`${time(x.start)} · ${x.title || x.clientName}`}
                    >
                      <span className={`size-1.5 shrink-0 rounded-full ${BAR[x.kind] ?? "bg-neon"}`} />
                      <span className="shrink-0 font-mono text-mute">{time(x.start)}</span>
                      <span className="min-w-0 truncate text-ink">{x.title || x.clientName}</span>
                    </button>
                  </li>
                ))}
                {more > 0 && (
                  <li>
                    <button onClick={() => onOpenDay(k)} className="px-1 text-[10px] text-neon hover:underline">
                      y {more} más
                    </button>
                  </li>
                )}
              </ul>
            </div>
          );
        })}
      </div>
      <p className="border-t border-line px-4 py-2 text-[11px] text-mute">
        El color del punto dice el tipo: visita, tarea, llamada o reunión. El reloj: seguimientos de ese día.
      </p>
    </div>
  );
}
