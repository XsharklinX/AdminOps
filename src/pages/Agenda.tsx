import { BellRing, CalendarCheck, CalendarDays, CalendarPlus, Check, Mail, MapPin, Monitor, Pencil, Phone, Play, Repeat, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { useLiveEffect } from "../lib/useLiveEffect";
import { OutlookButton } from "../components/M365";
import type { PageId } from "../components/Sidebar";
import { Button, Card, EmptyState, inputClass, Loading, Modal } from "../components/ui";
import { agendaApi, portalsApi, REPEATS, workApi, type Client, type DueClient, type Settings, type Visit } from "../lib/api";

const DAY_MS = 86_400_000;

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
const dayKey = (ts: number) => startOfDay(new Date(ts * 1000)).getTime();
const time = (ts: number) => new Date(ts * 1000).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
const longDate = (ts: number) => new Date(ts * 1000).toLocaleDateString("es", { weekday: "long", day: "numeric", month: "long" });
const shortDate = (ts: number) => new Date(ts * 1000).toLocaleDateString("es", { day: "numeric", month: "short" });

/** «Hoy», «Mañana», o el día de la semana y la fecha. */
function dayLabel(key: number): string {
  const today = startOfDay(new Date()).getTime();
  if (key === today) return "Hoy";
  if (key === today + DAY_MS) return "Mañana";
  if (key === today - DAY_MS) return "Ayer";
  const s = longDate(key / 1000);
  return s.charAt(0).toUpperCase() + s.slice(1);
}

const equipos = (n: number) => (n === 1 ? "1 equipo" : `${n} equipos`);

/** Fecha y hora locales para los campos del formulario. */
function toInputs(ts: number): { date: string; time: string } {
  const d = new Date(ts * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return { date: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`, time: `${p(d.getHours())}:${p(d.getMinutes())}` };
}
const fromInputs = (date: string, t: string) => Math.floor(new Date(`${date}T${t || "09:00"}`).getTime() / 1000);

/** Mañana a las 9:00: la hora que se propone por defecto. */
function defaultStart(): number {
  const d = startOfDay(new Date(Date.now() + DAY_MS));
  d.setHours(9);
  return Math.floor(d.getTime() / 1000);
}

/** Agenda de mantenimientos: qué visitas tocan y a quién le toca ya. */
export function Agenda({ onNavigate }: { onNavigate: (page: PageId, focus?: string | null) => void }) {
  const [data, setData] = useState<{ visits: Visit[]; due: DueClient[] } | null>(null);
  const [clients, setClients] = useState<Client[]>([]);
  const [editing, setEditing] = useState<Visit | null>(null);
  const [showPast, setShowPast] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    try {
      const [a, c] = await Promise.all([agendaApi.list(), workApi.clients()]);
      setData(a);
      setClients(c);
    } catch (e) {
      toast("error", String(e));
    }
  }, [toast]);
  useEffect(() => {
    void load();
  }, [load]);

  const byId = useMemo(() => new Map(clients.map((c) => [c.id, c])), [clients]);

  const groups = useMemo(() => {
    const today = startOfDay(new Date()).getTime();
    const visits = (data?.visits ?? []).filter((v) => v.status !== "cancelled");
    const upcoming = visits.filter((v) => dayKey(v.start) >= today && v.status === "planned");
    const past = visits.filter((v) => dayKey(v.start) < today || v.status === "done").sort((a, b) => b.start - a.start);
    const days = new Map<number, Visit[]>();
    for (const v of upcoming) days.set(dayKey(v.start), [...(days.get(dayKey(v.start)) ?? []), v]);
    return { days: [...days.entries()].sort((a, b) => a[0] - b[0]), past };
  }, [data]);

  const newVisit = (clientId = "", machines = 0): Visit => ({
    id: "",
    clientId,
    clientName: "",
    start: defaultStart(),
    minutes: 60,
    machines,
    notes: "",
    status: "planned",
    reminded: false,
    visitType: "",
    repeatEvery: "",
    place: "",
  });

  const setStatus = async (v: Visit, status: Visit["status"]) => {
    try {
      const siguiente = await agendaApi.setStatus(v.id, status);
      // Una visita que se repite deja ya puesta la próxima: el ciclo no depende
      // de que alguien se acuerde de volver a apuntarlo.
      toast(
        "ok",
        siguiente
          ? `Visita a ${v.clientName} hecha. La próxima queda planificada para el ${shortDate(siguiente.start)}.`
          : status === "done"
            ? `Visita a ${v.clientName} marcada como hecha.`
            : status === "cancelled"
              ? "Visita cancelada."
              : "Visita planificada.",
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
      toast("ok", days > 0 ? `Aplazada ${days === 1 ? "un día" : `${days} días`}.` : "Adelantada un día.");
      void load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  const remove = async (v: Visit) => {
    if (!(await confirm({ title: "Borrar visita", body: `Se borrará la visita a «${v.clientName}» del ${shortDate(v.start)}.`, confirmLabel: "Borrar", danger: true }))) return;
    await agendaApi.remove(v.id).catch((e) => toast("error", String(e)));
    void load();
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
      else toast("info", "Configura el Correo (Soporte → Correo) para escribirlo desde AdminOps.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (!data) return <Loading page />;

  const todayKey = startOfDay(new Date()).getTime();
  const todays = groups.days.find(([k]) => k === todayKey)?.[1] ?? [];

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <div className="col-span-12 space-y-4 lg:col-span-4">
        <Card title="Hoy" icon={<CalendarCheck size={14} />}>
          {todays.length === 0 ? (
            <p className="text-sm text-mute">Nada planificado para hoy.</p>
          ) : (
            <p className="text-sm text-ink">
              {todays.length} {todays.length === 1 ? "visita" : "visitas"} ·{" "}
              {equipos(todays.reduce((n, v) => n + v.machines, 0))}
            </p>
          )}
          <p className="mt-2 text-[11px] text-mute">AdminOps te avisa 30 minutos antes de cada visita y, al abrirlo, de las visitas del día.</p>
        </Card>

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
                      <button onClick={() => setEditing(newVisit(d.clientId, d.machines))} className="flex items-center gap-1 text-xs text-neon hover:underline">
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
      </div>

      <div className="col-span-12 space-y-4 lg:col-span-8">
        <div className="flex items-center gap-3">
          <h2 className="flex items-center gap-2 text-sm font-medium text-ink">
            <CalendarDays size={15} /> Próximas visitas
          </h2>
          <div className="ml-auto">
            <Button onClick={() => setEditing(newVisit())} disabled={clients.length === 0} title={clients.length ? undefined : "Crea antes un cliente (Soporte → Clientes)"}>
              <CalendarPlus size={14} /> Nueva visita
            </Button>
          </div>
        </div>

        {groups.days.length === 0 ? (
          <EmptyState
            icon={<CalendarDays size={22} />}
            title="No hay visitas planificadas"
            action={
              clients.length > 0 ? (
                <Button onClick={() => setEditing(newVisit())}>
                  <CalendarPlus size={14} /> Planificar una visita
                </Button>
              ) : (
                <Button onClick={() => onNavigate("clients")}>Crear un cliente</Button>
              )
            }
          >
            Planifica aquí los mantenimientos: te avisará el día y 30 minutos antes, y desde la visita empiezas la sesión de servicio.
          </EmptyState>
        ) : (
          groups.days.map(([key, visits]) => (
            <section key={key}>
              <h3 className={`mb-2 text-xs font-medium ${key === todayKey ? "text-neon" : "text-dim"}`}>
                {dayLabel(key)}
                <span className="ml-2 text-mute">
                  {visits.length} {visits.length === 1 ? "visita" : "visitas"} · {equipos(visits.reduce((n, v) => n + v.machines, 0))}
                </span>
              </h3>
              <ul className="space-y-2">
                {visits.map((v) => (
                  <VisitRow
                    key={v.id}
                    v={v}
                    client={byId.get(v.clientId)}
                    onStart={() => onNavigate("session", v.clientId)}
                    onDone={() => setStatus(v, "done")}
                    onCancel={() => setStatus(v, "cancelled")}
                    onEdit={() => setEditing(v)}
                    onRemind={() => remind(v)}
                    onPostpone={(d) => postpone(v, d)}
                    onChanged={() => void load()}
                  />
                ))}
              </ul>
            </section>
          ))
        )}

        {groups.past.length > 0 && (
          <section>
            <button onClick={() => setShowPast(!showPast)} className="text-xs text-mute hover:text-ink">
              {showPast ? "Ocultar" : "Ver"} visitas pasadas ({groups.past.length})
            </button>
            {showPast && (
              <ul className="mt-2 divide-y divide-line/60 rounded-xl border border-line bg-panel">
                {groups.past.slice(0, 50).map((v) => (
                  <li key={v.id} className="flex items-center gap-3 px-4 py-2 text-sm">
                    <span className="w-24 shrink-0 text-xs text-mute">{shortDate(v.start)}</span>
                    <span className="min-w-0 flex-1 truncate text-dim">{v.clientName}</span>
                    <span className={`text-xs ${v.status === "done" ? "text-ok" : "text-warn"}`}>{v.status === "done" ? "Hecha" : "Sin marcar"}</span>
                    {v.status !== "done" && (
                      <button onClick={() => setStatus(v, "done")} className="text-xs text-neon hover:underline">
                        Marcar hecha
                      </button>
                    )}
                    <button onClick={() => remove(v)} className="text-mute hover:text-bad" title="Borrar">
                      <Trash2 size={13} />
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}
      </div>

      {editing && (
        <VisitEditor
          visit={editing}
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
            toast(conflict ? "info" : "ok", conflict ? `Visita guardada, pero se pisa con la de ${conflict}.` : "Visita guardada.");
            void load();
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function VisitRow({
  v,
  client,
  onStart,
  onDone,
  onCancel,
  onEdit,
  onRemind,
  onPostpone,
  onChanged,
}: {
  v: Visit;
  client?: Client;
  onStart: () => void;
  onDone: () => void;
  onCancel: () => void;
  onEdit: () => void;
  onRemind: () => void;
  onPostpone: (days: number) => void;
  onChanged: () => void;
}) {
  const btn = "rounded-md p-1.5 text-dim transition-colors hover:bg-panel-2 hover:text-ink";
  return (
    <li className="flex items-start gap-4 rounded-xl border border-line bg-panel px-4 py-3">
      <div className="w-14 shrink-0 text-center">
        <div className="font-mono text-base text-ink">{time(v.start)}</div>
        <div className="text-[10px] text-mute">{v.minutes >= 60 ? `${Math.round((v.minutes / 60) * 10) / 10} h` : `${v.minutes} min`}</div>
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium text-ink">{v.clientName}</div>
        <div className="flex flex-wrap items-center gap-x-3 text-xs text-dim">
          {v.machines > 0 && (
            <span className="flex items-center gap-1">
              <Monitor size={11} /> {equipos(v.machines)}
            </span>
          )}
          {(v.place || client?.address) && (
            <span className="flex items-center gap-1 truncate">
              <MapPin size={11} className="shrink-0" /> {v.place || client?.address}
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
      <div className="flex shrink-0 items-center gap-0.5">
        <Button onClick={onStart} title="Empezar la sesión de servicio con este cliente">
          <Play size={13} /> Empezar
        </Button>
        {client?.email && (
          <button onClick={onRemind} className={btn} title="Enviar recordatorio al cliente">
            <Mail size={14} />
          </button>
        )}
        <button onClick={onDone} className={btn} title="Marcar como hecha">
          <Check size={14} />
        </button>
        <button onClick={() => onPostpone(1)} className={btn} title="Aplazar un día (sin abrir el editor)">
          <CalendarPlus size={14} />
        </button>
        <OutlookButton visitId={v.id} inOutlook={!!v.outlookEvent} onDone={onChanged} className={btn} />
        <button onClick={onEdit} className={btn} title="Cambiar día, hora, sitio o repetición">
          <Pencil size={14} />
        </button>
        <button onClick={onCancel} className={`${btn} hover:text-bad`} title="Cancelar visita">
          <X size={14} />
        </button>
      </div>
    </li>
  );
}

function VisitEditor({ visit, clients, onClose, onSaved, onDelete }: { visit: Visit; clients: Client[]; onClose: () => void; onSaved: (conflict: string) => void; onDelete?: () => void }) {
  const [v, setV] = useState(visit);
  const initial = toInputs(visit.start);
  const [date, setDate] = useState(initial.date);
  const [t, setT] = useState(initial.time);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  // Los tipos de visita se configuran en Ajustes y traen su propia checklist.
  const [types, setTypes] = useState<Settings["visitTypes"]>([]);
  useLiveEffect((vigente) => {
    workApi.settings().then((s) => vigente() && setTypes(s.visitTypes ?? [])).catch(() => {});
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

  return (
    <Modal
      title={visit.id ? "Editar visita" : "Nueva visita"}
      onClose={onClose}
      width="w-[520px]"
      footer={
        <>
          {error && <p className="mr-auto max-w-60 text-xs text-bad">{error}</p>}
          {onDelete && !error && (
            <button onClick={onDelete} className="mr-auto text-xs text-mute hover:text-bad">
              Borrar visita
            </button>
          )}
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save} disabled={!v.clientId || !date || saving}>
            Guardar
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Cliente</span>
          <select value={v.clientId} onChange={(e) => pickClient(e.target.value)} className={inputClass}>
            <option value="">Elige un cliente…</option>
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
              {[30, 60, 90, 120, 180, 240, 480].map((m) => (
                <option key={m} value={m}>
                  {m < 60 ? `${m} min` : `${m / 60} h`}
                </option>
              ))}
            </select>
          </label>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Equipos a revisar</span>
            <input type="number" min={0} max={500} value={v.machines} onChange={(e) => setV({ ...v, machines: Math.max(0, Number(e.target.value) || 0) })} className={inputClass} />
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
        {v.repeatEvery && (
          <p className="-mt-1 text-[11px] text-mute">Al marcarla como hecha, la siguiente se planifica sola. No hay que volver a apuntarla.</p>
        )}
        <div className="grid grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Dónde</span>
            <input value={v.place} onChange={(e) => setV({ ...v, place: e.target.value.slice(0, 120) })} placeholder="Sede central, planta 2" className={inputClass} />
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
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Notas</span>
          <textarea value={v.notes} onChange={(e) => setV({ ...v, notes: e.target.value.slice(0, 500) })} rows={3} placeholder="Traer disco de repuesto, preguntar por la impresora…" className={inputClass} />
        </label>
      </div>
    </Modal>
  );
}
