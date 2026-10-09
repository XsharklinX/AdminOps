// Agenda: lo que hay que hacer y cuándo.
//
// Visitas a clientes, pero también tareas, llamadas y reuniones sin cliente:
// quien trabaja en una sola empresa no tiene «clientes» y la agenda le tiene que
// servir igual. Arriba se apunta algo en una línea; la tira de la semana dice
// de un vistazo cómo viene; a la izquierda, lo de hoy y lo de mañana (con los
// seguimientos), y los clientes a los que ya les toca mantenimiento.
import {
  BellRing,
  Briefcase,
  CalendarCheck,
  CalendarDays,
  CalendarPlus,
  Check,
  Clock,
  ListTodo,
  Mail,
  MapPin,
  Monitor,
  Pencil,
  Phone,
  Play,
  Plus,
  Repeat,
  Sunrise,
  History as HistoryIcon,
  Users,
  X,
} from "lucide-react";
import { useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { AgendaHistory } from "../components/AgendaHistory";
import { MonthView, daysBetween } from "../components/AgendaMonth";
import type { PageId } from "../components/Sidebar";
import { Button, Card, inputClass, Loading, Modal, iconBtn } from "../components/ui";
import { logQuietly, agendaApi, followupsApi, portalsApi, REPEATS, workApi, type AgendaKind, type Client, type DueClient, type Followup, type Settings, type Visit } from "../lib/api";
import { useLiveEffect } from "../lib/useLiveEffect";
import { timeOfDay as time, shortDate } from "../lib/format";

const DAY_MS = 86_400_000;

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
const dayKey = (ts: number) => startOfDay(new Date(ts * 1000)).getTime();
const todayKey = () => startOfDay(new Date()).getTime();
const longDate = (ts: number) => new Date(ts * 1000).toLocaleDateString("es", { weekday: "long", day: "numeric", month: "long" });
const equipos = (n: number) => (n === 1 ? "1 equipo" : `${n} equipos`);

/** «Hoy», «Mañana», o el día de la semana y la fecha. */
function dayLabel(key: number): string {
  const today = todayKey();
  if (key === today) return "Hoy";
  if (key === today + DAY_MS) return "Mañana";
  if (key === today - DAY_MS) return "Ayer";
  const s = longDate(key / 1000);
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Fecha y hora locales para los campos del formulario. */
function toInputs(ts: number): { date: string; time: string } {
  const d = new Date(ts * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return { date: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`, time: `${p(d.getHours())}:${p(d.getMinutes())}` };
}
const fromInputs = (date: string, t: string) => Math.floor(new Date(`${date}T${t || "09:00"}`).getTime() / 1000);

/**
 * Hora propuesta para un día: hoy, la siguiente hora en punto (si aún queda
 * día de trabajo); cualquier otro día, las 9:00.
 */
function proposedStart(key: number): number {
  const d = new Date(key);
  if (key === todayKey()) {
    const h = new Date().getHours() + 1;
    d.setHours(h <= 20 ? h : 9, 0, 0, 0);
    if (h > 20) d.setDate(d.getDate() + 1);
  } else {
    d.setHours(9, 0, 0, 0);
  }
  return Math.floor(d.getTime() / 1000);
}

/** Qué es cada cosa: nombre, icono y color de la franja. */
const KINDS: Record<AgendaKind, { label: string; Icon: typeof Briefcase; bar: string; text: string }> = {
  visit: { label: "Visita", Icon: Briefcase, bar: "bg-neon", text: "text-neon" },
  task: { label: "Tarea", Icon: ListTodo, bar: "bg-ok", text: "text-ok" },
  call: { label: "Llamada", Icon: Phone, bar: "bg-warn", text: "text-warn" },
  meeting: { label: "Reunión", Icon: Users, bar: "bg-dim", text: "text-dim" },
};
const kindOf = (v: Visit): AgendaKind => (v.kind || "visit") as AgendaKind;
const labelOf = (v: Visit) => v.title || v.clientName || "Sin título";

function emptyEntry(start: number, patch: Partial<Visit> = {}): Visit {
  return {
    id: "",
    kind: "task",
    title: "",
    clientId: "",
    clientName: "",
    start,
    minutes: 60,
    machines: 0,
    notes: "",
    status: "planned",
    reminded: false,
    visitType: "",
    repeatEvery: "",
    place: "",
    ...patch,
  };
}

/** Agenda: visitas, tareas, llamadas y reuniones, con o sin cliente. */
export function Agenda({ onNavigate, focus }: { onNavigate: (page: PageId, focus?: string | null) => void; focus?: string | null }) {
  const [data, setData] = useState<{ visits: Visit[]; due: DueClient[] } | null>(null);
  const [clients, setClients] = useState<Client[]>([]);
  const [followups, setFollowups] = useState<Followup[]>([]);
  /** Todos, también los hechos: para el historial. */
  const [allFollowups, setAllFollowups] = useState<Followup[]>([]);
  const [editing, setEditing] = useState<Visit | null>(null);
  /** Día elegido en la tira de la semana (null: todos). */
  const [dayFilter, setDayFilter] = useState<number | null>(null);
  const quickRef = useRef<QuickAddHandle | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  /** Lista o mes (se recuerda). */
  const [mode, setModeState] = useState<"list" | "month" | "history">(() => {
    try {
      return localStorage.getItem("adminops.agenda.mode") === "month" ? "month" : "list";
    } catch {
      return "list";
    }
  });
  const setMode = (m: "list" | "month" | "history") => {
    setModeState(m);
    try {
      // El historial se consulta de vez en cuando: al volver se abre la lista o el mes.
      if (m !== "history") localStorage.setItem("adminops.agenda.mode", m);
    } catch {
      /* sin almacenamiento */
    }
  };
  const [month, setMonth] = useState(() => new Date());

  const load = useCallback(async () => {
    try {
      const [a, c, f] = await Promise.all([agendaApi.list(), workApi.clients().catch(() => [] as Client[]), followupsApi.list().catch(() => [] as Followup[])]);
      setData(a);
      setClients(c);
      setFollowups(f.filter((x) => !x.done));
      setAllFollowups(f);
    } catch (e) {
      toast("error", String(e));
    }
  }, [toast]);
  useEffect(() => {
    void load();
  }, [load]);

  const byId = useMemo(() => new Map(clients.map((c) => [c.id, c])), [clients]);

  // Desde la ficha de un cliente: «Agendar» abre ya el editor con ese cliente.
  const handled = useRef<string | null>(null);
  useEffect(() => {
    if (!focus?.startsWith("client:") || handled.current === focus || clients.length === 0) return;
    handled.current = focus;
    const c = byId.get(focus.slice(7));
    if (c) setEditing(emptyEntry(proposedStart(todayKey() + DAY_MS), { kind: "visit", clientId: c.id, machines: c.machines.length }));
  }, [focus, clients, byId]);

  const groups = useMemo(() => {
    const today = todayKey();
    const visits = (data?.visits ?? []).filter((v) => v.status !== "cancelled");
    const upcoming = visits.filter((v) => dayKey(v.start) >= today && v.status === "planned");
    const past = visits.filter((v) => dayKey(v.start) < today || v.status === "done").sort((a, b) => b.start - a.start);
    // Lo que se quedó sin marcar: ni hecho ni cancelado, y su día ya pasó.
    const overdue = visits.filter((v) => v.status === "planned" && dayKey(v.start) < today).sort((a, b) => a.start - b.start);
    const days = new Map<number, { visits: Visit[]; followups: Followup[] }>();
    const slot = (k: number) => {
      let d = days.get(k);
      if (!d) days.set(k, (d = { visits: [], followups: [] }));
      return d;
    };
    for (const v of upcoming) slot(dayKey(v.start)).visits.push(v);
    // Los seguimientos atrasados cuentan como de hoy: siguen pendientes.
    for (const f of followups) slot(Math.max(dayKey(f.due), today)).followups.push(f);
    return { days: [...days.entries()].sort((a, b) => a[0] - b[0]), past, overdue };
  }, [data, followups]);

  const setStatus = async (v: Visit, status: Visit["status"]) => {
    try {
      const siguiente = await agendaApi.setStatus(v.id, status);
      // Una visita que se repite deja ya puesta la próxima: el ciclo no depende
      // de que alguien se acuerde de volver a apuntarlo.
      toast(
        "ok",
        siguiente
          ? `«${labelOf(v)}» hecha. La próxima queda para el ${shortDate(siguiente.start)}.`
          : status === "done"
            ? `«${labelOf(v)}» marcada como hecha.`
            : status === "cancelled"
              ? "Cancelada."
              : "Planificada.",
      );
      void load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  /** Aplazar o adelantar sin abrir el editor: es lo que más se hace. */
  const postpone = async (v: Visit, days: number) => {
    try {
      await agendaApi.postpone(v.id, days);
      toast("ok", days > 0 ? `Aplazada ${days === 1 ? "a mañana" : `${days} días`}.` : days === -1 ? "Adelantada un día." : `Adelantada ${-days} días.`);
      void load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  const remove = async (v: Visit) => {
    if (!(await confirm({ title: "Borrar de la agenda", body: `Se borrará «${labelOf(v)}» del ${shortDate(v.start)}.`, confirmLabel: "Borrar", danger: true }))) return;
    await agendaApi.remove(v.id).catch((e) => toast("error", String(e)));
    void load();
  };

  const followup = async (f: Followup, action: "done" | "snooze") => {
    try {
      if (action === "done") await followupsApi.setDone(f.id, true);
      else await followupsApi.snooze(f.id, 1);
      toast("ok", action === "done" ? "Seguimiento hecho." : "Aplazado a mañana.");
      void load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  // Recordatorio al cliente por correo (en el Correo de AdminOps si está configurado).
  const remind = async (v: Visit) => {
    const c = byId.get(v.clientId);
    if (!c?.email) return toast("info", "Ese cliente no tiene correo en su ficha.");
    const subject = `Visita de mantenimiento: ${longDate(v.start)} a las ${time(v.start)}`;
    const body = `Hola${c.contact ? ` ${c.contact}` : ""}:\n\nTe confirmo la visita de mantenimiento el ${longDate(v.start)} a las ${time(v.start)}${v.machines ? ` para revisar ${equipos(v.machines)}` : ""}.\n\nUn saludo.`;
    try {
      const inApp = await portalsApi.compose(c.email, subject, body);
      if (inApp) onNavigate("mail");
      else toast("info", "Configura el Correo (su icono, en la barra de arriba) para escribirlo desde AdminOps.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (!data) return <Loading page />;

  const today = todayKey();
  const tomorrow = today + DAY_MS;
  const dayData = (k: number) => groups.days.find(([d]) => d === k)?.[1] ?? { visits: [], followups: [] };
  const weekCount = groups.days.filter(([k]) => k < today + 7 * DAY_MS).reduce((n, [, d]) => n + d.visits.length + d.followups.length, 0);
  const shown = dayFilter === null ? groups.days : groups.days.filter(([k]) => k === dayFilter);

  const rowProps = (v: Visit) => ({
    v,
    client: byId.get(v.clientId),
    onStart: () => onNavigate("session", v.clientId),
    onDone: () => void setStatus(v, "done"),
    onCancel: () => void setStatus(v, "cancelled"),
    onEdit: () => setEditing(v),
    onRemind: () => void remind(v),
    onPostpone: (d: number) => void postpone(v, d),
  });

  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-2">
        <Stat label="Hoy" n={dayData(today).visits.length + dayData(today).followups.length} active />
        <Stat label="Mañana" n={dayData(tomorrow).visits.length + dayData(tomorrow).followups.length} />
        <Stat label="Próximos 7 días" n={weekCount} />
        <div className="ml-auto flex items-center gap-2">
          <span className="flex overflow-hidden rounded-md border border-line text-xs" role="tablist" aria-label="Vista">
            {(["list", "month", "history"] as const).map((m) => (
              <button key={m} role="tab" aria-selected={mode === m} onClick={() => setMode(m)} className={`px-3 py-1.5 ${mode === m ? "bg-neon/15 text-neon" : "text-mute hover:text-ink"}`}>
                {m === "list" ? "Lista" : m === "month" ? "Mes" : "Historial"}
              </button>
            ))}
          </span>
          <Button onClick={() => setEditing(emptyEntry(proposedStart(dayFilter ?? today)))}>
            <CalendarPlus size={14} /> Nueva
          </Button>
        </div>
      </div>

      {mode === "month" && (
        <MonthView
          month={month}
          onMonth={setMonth}
          visits={(data.visits ?? []).filter((v) => v.status !== "cancelled")}
          followups={followups}
          onMove={(v, k) => void postpone(v, daysBetween(dayKey(v.start), k))}
          onOpenDay={(k) => {
            setMode("list");
            setDayFilter(k);
          }}
          onAdd={(k) => setEditing(emptyEntry(proposedStart(k)))}
          onEdit={(v) => setEditing(v)}
        />
      )}

      {mode === "history" && (
        <AgendaHistory
          visits={data.visits}
          followups={allFollowups}
          onReopen={(v) => void setStatus(v, "planned")}
          onDone={(v) => void setStatus(v, "done")}
          onRepeat={(v) => setEditing({ ...v, id: "", status: "planned", reminded: false, doneAt: 0, start: proposedStart(todayKey() + DAY_MS) })}
          onRemove={(v) => void remove(v)}
          onReopenFollowup={(f) =>
            void followupsApi
              .setDone(f.id, false)
              .then(() => load())
              .catch((e) => toast("error", String(e)))
          }
        />
      )}

      {mode !== "history" && <QuickAdd ref={quickRef} onAdded={() => void load()} />}

      {mode === "list" && <WeekStrip days={groups.days} selected={dayFilter} onSelect={setDayFilter} />}

      <div className={`grid grid-cols-12 gap-4 ${mode !== "list" ? "hidden" : ""}`}>
        <div className="col-span-12 space-y-4 lg:col-span-4">
          <NextUp visits={dayData(today).visits} />

          <Card
            title="Mañana"
            icon={<Sunrise size={14} />}
            right={
              <button onClick={() => quickRef.current?.focus(tomorrow)} className="flex items-center gap-1 text-xs text-neon hover:underline">
                <Plus size={12} /> Apuntar
              </button>
            }
          >
            <DayPreview data={dayData(tomorrow)} empty="Nada apuntado para mañana." />
          </Card>

          {clients.length > 0 && (
            <Card title={`Toca mantenimiento · ${data.due.length}`} icon={<BellRing size={14} />}>
              {data.due.length === 0 ? (
                <p className="text-sm text-mute">Ningún cliente pendiente de agendar. La fecha sale de la última visita (Ajustes → Informes y cobros → «Mantenimiento cada (meses)»).</p>
              ) : (
                <ul className="space-y-2.5">
                  {data.due.map((d) => {
                    const late = d.date * 1000 < Date.now();
                    return (
                      <li key={d.clientId} className="text-sm">
                        <div className="flex items-center gap-2">
                          <span className="min-w-0 flex-1 truncate text-ink">{d.name}</span>
                          <button
                            onClick={() => setEditing(emptyEntry(proposedStart(tomorrow), { kind: "visit", clientId: d.clientId, machines: d.machines }))}
                            className="flex items-center gap-1 text-xs text-neon hover:underline"
                          >
                            <CalendarPlus size={12} /> Agendar
                          </button>
                        </div>
                        <div className="flex flex-wrap items-center gap-x-2 text-[11px]">
                          <span className={late ? "text-warn" : "text-mute"}>
                            {late ? "Vencido desde" : "Toca el"} {shortDate(d.date)}
                          </span>
                          {d.machines > 0 && <span className="text-mute">· {equipos(d.machines)}</span>}
                          {d.phone && (
                            <span className="flex items-center gap-1 text-mute">
                              · <Phone size={10} /> {d.phone}
                            </span>
                          )}
                        </div>
                      </li>
                    );
                  })}
                </ul>
              )}
            </Card>
          )}

          <p className="px-1 text-[11px] text-mute">AdminOps avisa 30 minutos antes de cada cosa y, al abrirlo, de lo que hay hoy. Los seguimientos de la nota de llamada (Ctrl+Alt+N) también salen aquí.</p>
        </div>

        <div className="col-span-12 space-y-5 lg:col-span-8">
          {dayFilter !== null && (
            <div className="flex items-center gap-2 text-xs text-dim">
              Solo {dayLabel(dayFilter).toLowerCase()}
              <button onClick={() => setDayFilter(null)} className="text-neon hover:underline">
                Ver todo
              </button>
            </div>
          )}

          {groups.overdue.length > 0 && dayFilter === null && (
            <section>
              <h3 className="mb-2 flex items-baseline gap-2 text-xs font-medium text-warn">
                Atrasado
                <span className="font-normal text-mute">
                  {groups.overdue.length} sin marcar: ¿se {groups.overdue.length === 1 ? "hizo" : "hicieron"}? Márcalo como hecho, pásalo a otro día o cancélalo.
                </span>
              </h3>
              <ul className="space-y-2">
                {groups.overdue.map((v) => (
                  <EntryRow key={v.id} {...rowProps(v)} overdue />
                ))}
              </ul>
            </section>
          )}

          {shown.length === 0 ? (
            <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-line px-6 py-10 text-center">
              <CalendarDays size={22} className="text-mute" />
              <div className="text-sm font-medium text-ink">{dayFilter === null ? "No hay nada planificado" : `Nada para ${dayLabel(dayFilter).toLowerCase()}`}</div>
              <p className="max-w-md text-xs text-dim">
                Apunta tareas, llamadas, reuniones o visitas, con cliente o sin él. Te avisa 30 minutos antes y lo que se repite se vuelve a planificar solo.
              </p>
              <div className="flex flex-wrap justify-center gap-2">
                <Button onClick={() => setEditing(emptyEntry(proposedStart(dayFilter ?? today)))}>
                  <CalendarPlus size={14} /> Apuntar algo
                </Button>
                {clients.length === 0 && (
                  <Button kind="ghost" onClick={() => onNavigate("clients")}>
                    Crear un cliente
                  </Button>
                )}
              </div>
            </div>
          ) : (
            shown.map(([key, d]) => (
              <section key={key}>
                <h3 className={`mb-2 flex items-baseline gap-2 text-xs font-medium ${key === today ? "text-neon" : "text-dim"}`}>
                  {dayLabel(key)}
                  <span className="font-normal text-mute">
                    {d.visits.length > 0 && `${d.visits.length} en la agenda`}
                    {d.visits.length > 0 && d.followups.length > 0 && " · "}
                    {d.followups.length > 0 && `${d.followups.length} ${d.followups.length === 1 ? "seguimiento" : "seguimientos"}`}
                  </span>
                  <button onClick={() => setEditing(emptyEntry(proposedStart(key)))} className="ml-auto flex items-center gap-1 font-normal text-mute hover:text-neon" title="Apuntar algo este día">
                    <Plus size={12} /> Añadir
                  </button>
                </h3>
                <ul className="space-y-2">
                  {d.visits.map((v) => (
                    <EntryRow key={v.id} {...rowProps(v)} />
                  ))}
                  {d.followups.map((f) => (
                    <FollowupRow key={f.id} f={f} onDone={() => void followup(f, "done")} onSnooze={() => void followup(f, "snooze")} />
                  ))}
                </ul>
              </section>
            ))
          )}

          {groups.past.length > 0 && dayFilter === null && (
            <button onClick={() => setMode("history")} className="flex items-center gap-1.5 text-xs text-mute hover:text-ink">
              <HistoryIcon size={12} /> Ver el historial ({groups.past.length})
            </button>
          )}
        </div>
      </div>

      {editing && (
        <EntryEditor
          entry={editing}
          clients={clients}
          onClose={() => setEditing(null)}
          onDelete={
            editing.id
              ? () => {
                  setEditing(null);
                  void remove(editing);
                }
              : undefined
          }
          onSaved={(conflict: string) => {
            setEditing(null);
            // Se guarda igual (a veces se solapan a propósito), pero hay que saberlo.
            toast(conflict ? "info" : "ok", conflict ? `Guardado, pero se pisa con «${conflict}».` : "Guardado en la agenda.");
            void load();
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function Stat({ label, n, active = false }: { label: string; n: number; active?: boolean }) {
  return (
    <span className={`flex items-center gap-2 rounded-lg border px-3 py-1.5 text-xs ${active && n > 0 ? "border-neon/40 bg-neon/10 text-ink" : "border-line bg-panel text-dim"}`}>
      {label}
      <span className={`font-mono text-sm ${n > 0 ? "text-ink" : "text-mute"}`}>{n}</span>
    </span>
  );
}

// ---------- Apuntar en una línea ----------

interface QuickAddHandle {
  focus: (day: number) => void;
}

const PLACEHOLDER: Record<AgendaKind, string> = {
  task: "Apunta una tarea: «Revisar el servidor», «Renovar el antivirus»…",
  call: "Apunta una llamada: «Llamar a Contabilidad por la impresora»…",
  meeting: "Apunta una reunión: «Reunión con dirección»…",
  visit: "Apunta una visita: «Mantenimiento en la sede norte»…",
};

/** «Llamar a Contabilidad» + día + hora, Enter y listo. */
function QuickAdd({ ref, onAdded }: { ref: React.RefObject<QuickAddHandle | null>; onAdded: () => void }) {
  const [text, setText] = useState("");
  const [kind, setKind] = useState<AgendaKind>("task");
  const [day, setDay] = useState<"today" | "tomorrow" | "after" | "date">("today");
  const [date, setDate] = useState(() => toInputs(Date.now() / 1000 + 2 * 86_400).date);
  const [hour, setHour] = useState("");
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const toast = useToast();

  useImperativeHandle(ref, () => ({
    focus: (k: number) => {
      const t = todayKey();
      setDay(k === t ? "today" : k === t + DAY_MS ? "tomorrow" : k === t + 2 * DAY_MS ? "after" : "date");
      setDate(toInputs(k / 1000).date);
      input.current?.focus();
    },
  }));

  const key = () => {
    const t = todayKey();
    if (day === "today") return t;
    if (day === "tomorrow") return t + DAY_MS;
    if (day === "after") return t + 2 * DAY_MS;
    return startOfDay(new Date(`${date}T00:00`)).getTime();
  };

  const add = async () => {
    if (!text.trim()) return;
    setBusy(true);
    try {
      const k = key();
      const start = hour ? fromInputs(toInputs(k / 1000).date, hour) : proposedStart(k);
      const r = await agendaApi.save(emptyEntry(start, { kind, title: text.trim(), minutes: kind === "call" ? 30 : 60 }));
      toast(r.conflict ? "info" : "ok", r.conflict ? `Apuntado, pero se pisa con «${r.conflict}».` : `Apuntado para ${dayLabel(k).toLowerCase()} a las ${time(start)}.`);
      setText("");
      setHour("");
      onAdded();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
      input.current?.focus();
    }
  };

  const select = "rounded-md border border-line bg-void/60 px-2 py-2 text-xs text-ink outline-none focus:border-neon/50";
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-xl border border-line bg-panel p-2">
      <div className="flex shrink-0 rounded-md border border-line p-0.5" role="radiogroup" aria-label="Qué es">
        {(Object.keys(KINDS) as AgendaKind[]).map((k) => {
          const { Icon, label, text: color } = KINDS[k];
          return (
            <button
              key={k}
              onClick={() => setKind(k)}
              role="radio"
              aria-checked={kind === k}
              title={label}
              className={`rounded px-2 py-1.5 transition-colors ${kind === k ? `bg-panel-2 ${color}` : "text-mute hover:text-ink"}`}
            >
              <Icon size={14} />
            </button>
          );
        })}
      </div>
      <input
        ref={input}
        value={text}
        onChange={(e) => setText(e.target.value.slice(0, 120))}
        onKeyDown={(e) => e.key === "Enter" && void add()}
        placeholder={PLACEHOLDER[kind]}
        className="min-w-48 flex-1 bg-transparent px-2 py-2 text-sm text-ink outline-none placeholder:text-mute"
      />
      <select value={day} onChange={(e) => setDay(e.target.value as typeof day)} className={select} aria-label="Día">
        <option value="today">Hoy</option>
        <option value="tomorrow">Mañana</option>
        <option value="after">Pasado mañana</option>
        <option value="date">Otro día…</option>
      </select>
      {day === "date" && <input type="date" value={date} onChange={(e) => setDate(e.target.value)} className={select} aria-label="Fecha" />}
      <input type="time" value={hour} onChange={(e) => setHour(e.target.value)} className={select} aria-label="Hora (si no, la siguiente en punto)" title="Si no pones hora: hoy, la siguiente en punto; otro día, las 9:00" />
      <Button onClick={() => void add()} disabled={busy || !text.trim()}>
        <Plus size={14} /> Añadir
      </Button>
    </div>
  );
}

// ---------- La semana de un vistazo ----------

function WeekStrip({ days, selected, onSelect }: { days: [number, { visits: Visit[]; followups: Followup[] }][]; selected: number | null; onSelect: (k: number | null) => void }) {
  const today = todayKey();
  const week = Array.from({ length: 7 }, (_, i) => today + i * DAY_MS);
  return (
    <div className="grid grid-cols-7 gap-1.5">
      {week.map((k) => {
        const d = days.find(([x]) => x === k)?.[1];
        const n = (d?.visits.length ?? 0) + (d?.followups.length ?? 0);
        const date = new Date(k);
        const on = selected === k;
        return (
          <button
            key={k}
            onClick={() => onSelect(on ? null : k)}
            className={`flex flex-col items-center gap-1 rounded-lg border px-1 py-2 transition-colors ${
              on ? "border-neon/60 bg-neon/10" : k === today ? "border-neon/30 bg-panel" : "border-line bg-panel hover:border-line-2"
            }`}
            title={n ? `${dayLabel(k)}: ${n} ${n === 1 ? "cosa" : "cosas"}` : `${dayLabel(k)}: nada`}
          >
            <span className={`text-[10px] uppercase ${k === today ? "text-neon" : "text-mute"}`}>{k === today ? "hoy" : date.toLocaleDateString("es", { weekday: "short" }).replace(".", "")}</span>
            <span className={`font-mono text-lg leading-none ${n ? "text-ink" : "text-mute"}`}>{date.getDate()}</span>
            <span className="flex h-1.5 items-center gap-0.5">
              {(d?.visits ?? []).slice(0, 4).map((v) => (
                <span key={v.id} className={`size-1.5 rounded-full ${KINDS[kindOf(v)].bar}`} />
              ))}
              {(d?.followups.length ?? 0) > 0 && <span className="size-1.5 rounded-full border border-dim" />}
            </span>
          </button>
        );
      })}
    </div>
  );
}

// ---------- Hoy: lo próximo ----------

function NextUp({ visits }: { visits: Visit[] }) {
  const [now, setNow] = useState(() => Date.now() / 1000);
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now() / 1000), 30_000);
    return () => window.clearInterval(t);
  }, []);
  const next = visits.find((v) => v.start + v.minutes * 60 > now);
  const enCurso = next && next.start <= now;
  const falta = next ? Math.round((next.start - now) / 60) : 0;
  return (
    <Card title="Hoy" icon={<CalendarCheck size={14} />}>
      {!next ? (
        <p className="text-sm text-mute">{visits.length ? "Ya no queda nada más hoy." : "Nada en la agenda para hoy."}</p>
      ) : (
        <div className="rounded-lg border border-neon/30 bg-neon/5 px-3 py-2">
          <div className="text-[11px] text-neon">{enCurso ? "Ahora" : falta < 60 ? `En ${falta} min` : `A las ${time(next.start)}`}</div>
          <div className="truncate text-sm font-medium text-ink">{labelOf(next)}</div>
          <div className="truncate text-[11px] text-dim">{[time(next.start), next.title ? next.clientName : "", next.place].filter(Boolean).join(" · ")}</div>
        </div>
      )}
      {visits.length > 1 && (
        <ul className="mt-2 space-y-1">
          {visits
            .filter((v) => v !== next)
            .map((v) => (
              <li key={v.id} className={`flex items-center gap-2 text-xs ${v.start + v.minutes * 60 < now ? "text-mute line-through" : "text-dim"}`}>
                <span className="w-10 shrink-0 font-mono">{time(v.start)}</span>
                <span className="min-w-0 flex-1 truncate">{labelOf(v)}</span>
              </li>
            ))}
        </ul>
      )}
    </Card>
  );
}

function DayPreview({ data, empty }: { data: { visits: Visit[]; followups: Followup[] }; empty: string }) {
  if (data.visits.length + data.followups.length === 0) return <p className="text-sm text-mute">{empty}</p>;
  return (
    <ul className="space-y-1.5">
      {data.visits.map((v) => {
        const { Icon, text } = KINDS[kindOf(v)];
        return (
          <li key={v.id} className="flex items-center gap-2 text-sm">
            <span className="w-10 shrink-0 font-mono text-xs text-mute">{time(v.start)}</span>
            <Icon size={13} className={`shrink-0 ${text}`} />
            <span className="min-w-0 flex-1 truncate text-ink">{labelOf(v)}</span>
          </li>
        );
      })}
      {data.followups.map((f) => (
        <li key={f.id} className="flex items-center gap-2 text-sm">
          <span className="w-10 shrink-0 font-mono text-xs text-mute">{time(f.due)}</span>
          <Clock size={13} className="shrink-0 text-mute" />
          <span className="min-w-0 flex-1 truncate text-dim">{f.text}</span>
        </li>
      ))}
    </ul>
  );
}

// ---------- Filas ----------

function EntryRow({
  v,
  client,
  onStart,
  onDone,
  onCancel,
  onEdit,
  onRemind,
  onPostpone,
  overdue = false,
}: {
  v: Visit;
  client?: Client;
  onStart: () => void;
  onDone: () => void;
  onCancel: () => void;
  onEdit: () => void;
  onRemind: () => void;
  onPostpone: (days: number) => void;
  /** Su día ya pasó: se enseña la fecha y «a hoy» en vez de «a mañana». */
  overdue?: boolean;
}) {
  const k = KINDS[kindOf(v)];
  const place = v.place || client?.address;
  return (
    <li className="group relative flex items-start gap-3 overflow-hidden rounded-xl border border-line bg-panel py-3 pr-3 pl-4 transition-colors hover:border-line-2">
      <span className={`absolute inset-y-0 left-0 w-1 ${k.bar}`} />
      <div className="w-14 shrink-0 text-center">
        <div className="font-mono text-base text-ink">{time(v.start)}</div>
        <div className={`text-[10px] ${overdue ? "text-warn" : "text-mute"}`}>{overdue ? shortDate(v.start) : v.minutes >= 60 ? `${Math.round((v.minutes / 60) * 10) / 10} h` : `${v.minutes} min`}</div>
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <k.Icon size={13} className={`shrink-0 ${k.text}`} />
          <span className="truncate text-sm font-medium text-ink" title={labelOf(v)}>
            {labelOf(v)}
          </span>
        </div>
        <div className="mt-0.5 flex flex-wrap items-center gap-x-3 text-xs text-dim">
          {v.title && v.clientName && <span className="text-ink/80">{v.clientName}</span>}
          {v.machines > 0 && (
            <span className="flex items-center gap-1">
              <Monitor size={11} /> {equipos(v.machines)}
            </span>
          )}
          {place && (
            <span className="flex min-w-0 items-center gap-1 truncate">
              <MapPin size={11} className="shrink-0" /> {place}
            </span>
          )}
          {v.repeatEvery && (
            <span className="flex items-center gap-1 text-mute">
              <Repeat size={11} /> {REPEATS.find((r) => r.value === v.repeatEvery)?.label.toLowerCase()}
            </span>
          )}
          {client?.phone && (
            <span className="flex items-center gap-1">
              <Phone size={11} /> {client.phone}
            </span>
          )}
        </div>
        {v.notes && <p className="mt-1 text-xs whitespace-pre-line text-mute">{v.notes}</p>}
      </div>
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-0.5">
        {v.clientId && (
          <Button onClick={onStart} title="Empezar la sesión de servicio con este cliente">
            <Play size={13} /> Empezar
          </Button>
        )}
        {client?.email && (
          <button onClick={onRemind} className={iconBtn} title="Enviar recordatorio al cliente">
            <Mail size={14} />
          </button>
        )}
        <button onClick={onDone} className={`${iconBtn} hover:text-ok`} title="Hecho">
          <Check size={14} />
        </button>
        <button
          onClick={() => onPostpone(overdue ? Math.max(1, daysBetween(dayKey(v.start), todayKey())) : 1)}
          className={iconBtn}
          title={overdue ? "Pasarlo a hoy" : "Pasarlo a mañana (sin abrir el editor)"}
        >
          <Sunrise size={14} />
        </button>
        <button onClick={onEdit} className={iconBtn} title="Cambiar día, hora, sitio o repetición">
          <Pencil size={14} />
        </button>
        <button onClick={onCancel} className={`${iconBtn} hover:text-bad`} title="Cancelar">
          <X size={14} />
        </button>
      </div>
    </li>
  );
}

function FollowupRow({ f, onDone, onSnooze }: { f: Followup; onDone: () => void; onSnooze: () => void }) {
  const atrasado = f.due * 1000 < todayKey();
  return (
    <li className="relative flex items-center gap-3 overflow-hidden rounded-xl border border-dashed border-line bg-panel/60 py-2.5 pr-3 pl-4">
      <span className={`absolute inset-y-0 left-0 w-1 ${atrasado ? "bg-bad" : "bg-line-2"}`} />
      <div className="w-14 shrink-0 text-center font-mono text-xs text-mute">{atrasado ? shortDate(f.due) : time(f.due)}</div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <Clock size={13} className={`shrink-0 ${atrasado ? "text-bad" : "text-mute"}`} />
          <span className="truncate text-sm text-ink">{f.text}</span>
        </div>
        <div className="text-[11px] text-mute">{[atrasado ? "Seguimiento atrasado" : "Seguimiento", f.person, f.machine].filter(Boolean).join(" · ")}</div>
      </div>
      <button onClick={onDone} className={`${iconBtn} hover:text-ok`} title="Hecho">
        <Check size={14} />
      </button>
      <button onClick={onSnooze} className={iconBtn} title="Mañana">
        <Sunrise size={14} />
      </button>
    </li>
  );
}

// ---------- Editor ----------

function EntryEditor({ entry, clients, onClose, onSaved, onDelete }: { entry: Visit; clients: Client[]; onClose: () => void; onSaved: (conflict: string) => void; onDelete?: () => void }) {
  const [v, setV] = useState<Visit>({ ...entry, kind: entry.kind || "visit" });
  const initial = toInputs(entry.start);
  const [date, setDate] = useState(initial.date);
  const [t, setT] = useState(initial.time);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  // Los tipos de visita se configuran en Ajustes y traen su propia checklist.
  const [types, setTypes] = useState<Settings["visitTypes"]>([]);
  useLiveEffect((vigente) => {
    workApi
      .settings()
      .then((s) => vigente() && setTypes(s.visitTypes ?? []))
      .catch(logQuietly("Agenda"));
  }, []);

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const r = await agendaApi.save({ ...v, start: fromInputs(date, t) });
      onSaved(r.conflict);
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  };

  const pickClient = (id: string) => setV({ ...v, clientId: id, machines: v.machines || (clients.find((c) => c.id === id)?.machines.length ?? 0) });
  const isVisit = v.kind === "visit";
  const canSave = (!!v.clientId || !!v.title.trim()) && !!date && !saving;

  return (
    <Modal
      title={entry.id ? "Editar" : "Apuntar en la agenda"}
      onClose={onClose}
      width="w-[540px]"
      footer={
        <>
          {error && <p className="mr-auto max-w-60 text-xs text-bad">{error}</p>}
          {onDelete && !error && (
            <button onClick={onDelete} className="mr-auto text-xs text-mute hover:text-bad">
              Borrar
            </button>
          )}
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={() => void save()} disabled={!canSave}>
            Guardar
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="grid grid-cols-4 gap-1.5" role="radiogroup" aria-label="Qué es">
          {(Object.keys(KINDS) as AgendaKind[]).map((k) => {
            const { Icon, label, text } = KINDS[k];
            const on = v.kind === k;
            return (
              <button
                key={k}
                role="radio"
                aria-checked={on}
                onClick={() => setV({ ...v, kind: k })}
                className={`flex flex-col items-center gap-1 rounded-lg border py-2 text-xs transition-colors ${on ? `border-neon/50 bg-neon/10 ${text}` : "border-line text-dim hover:text-ink"}`}
              >
                <Icon size={15} /> {label}
              </button>
            );
          })}
        </div>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Qué es {v.clientId ? "(opcional)" : ""}</span>
          <input
            value={v.title}
            onChange={(e) => setV({ ...v, title: e.target.value.slice(0, 120) })}
            placeholder={isVisit ? "Mantenimiento trimestral" : v.kind === "call" ? "Llamar a Contabilidad por la impresora" : v.kind === "meeting" ? "Reunión con dirección" : "Cambiar el disco del servidor"}
            className={inputClass}
            autoFocus={!entry.id}
          />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Cliente (opcional)</span>
          <select value={v.clientId} onChange={(e) => pickClient(e.target.value)} className={inputClass} disabled={clients.length === 0}>
            <option value="">{clients.length === 0 ? "No hay clientes (no hace falta)" : "Sin cliente"}</option>
            {clients.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        <div className="grid grid-cols-3 gap-3">
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Día</span>
            <input type="date" value={date} onChange={(e) => setDate(e.target.value)} className={inputClass} />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Hora</span>
            <input type="time" value={t} onChange={(e) => setT(e.target.value)} className={inputClass} />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Duración</span>
            <select value={v.minutes} onChange={(e) => setV({ ...v, minutes: Number(e.target.value) })} className={inputClass}>
              {[15, 30, 60, 90, 120, 180, 240, 480].map((m) => (
                <option key={m} value={m}>
                  {m < 60 ? `${m} min` : `${m / 60} h`}
                </option>
              ))}
            </select>
          </label>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Dónde</span>
            <input value={v.place} onChange={(e) => setV({ ...v, place: e.target.value.slice(0, 120) })} placeholder="Sede central, planta 2" className={inputClass} />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Se repite</span>
            <select value={v.repeatEvery} onChange={(e) => setV({ ...v, repeatEvery: e.target.value })} className={inputClass}>
              {REPEATS.map((r) => (
                <option key={r.value} value={r.value}>
                  {r.label}
                </option>
              ))}
            </select>
          </label>
        </div>
        {v.repeatEvery && <p className="-mt-1 text-[11px] text-mute">Al marcarla como hecha, la siguiente se planifica sola.</p>}
        {isVisit && (
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Equipos a revisar</span>
              <input type="number" min={0} max={500} value={v.machines} onChange={(e) => setV({ ...v, machines: Math.max(0, Number(e.target.value) || 0) })} className={inputClass} />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Tipo de visita</span>
              <select value={v.visitType} onChange={(e) => setV({ ...v, visitType: e.target.value })} className={inputClass}>
                <option value="">Sin tipo</option>
                {types.map((x) => (
                  <option key={x.name} value={x.name}>
                    {x.name}
                  </option>
                ))}
              </select>
            </label>
          </div>
        )}
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Notas</span>
          <textarea value={v.notes} onChange={(e) => setV({ ...v, notes: e.target.value.slice(0, 500) })} rows={3} placeholder="Traer disco de repuesto, preguntar por la impresora…" className={inputClass} />
        </label>
      </div>
    </Modal>
  );
}
