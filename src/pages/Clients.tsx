import { BellRing, CalendarPlus, Copy, ExternalLink, History, Mail, Monitor, Network, PenLine, Phone, Play, Plus, Save, Search, ShieldCheck, Trash2, UserRound } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { inventoryLines, MachineActions, VerdictChip } from "../components/inventory";
import { SendReportModal } from "../components/service";
import { Button, Card, inputClass, Tile } from "../components/ui";
import { VisitChanges } from "../components/VisitChanges";
import { MachineCompare } from "../components/MachineCompare";
import { logQuietly, contactsApi, diagApi, EMPTY_CLIENT_REPORT, workApi, type Client, type ClientReport, type Contact, type SessionRecord, type VisitMetrics } from "../lib/api";
import { bytes, money, fullDate as date } from "../lib/format";
import { goToPage } from "../lib/navigate";
import { Avatar } from "../components/contacts/Avatar";
import { DataTable, type Column } from "../components/DataTable";

const DAY = 86400;
// Segundos enteros: el backend guarda las fechas como u64.
const now = () => Math.floor(Date.now() / 1000);
const EMPTY = { id: "", name: "", contact: "", phone: "", email: "", address: "", notes: "", created: 0, machines: [], sessions: [], network: null, report: EMPTY_CLIENT_REPORT } as Client;

/** Próximo mantenimiento del cliente (el de su última visita). */
const nextOf = (c: Client) => c.sessions[0]?.nextMaintenance ?? null;
/** Vencido o en los próximos 30 días. */
const dueSoon = (c: Client) => {
  const n = nextOf(c);
  return n !== null && n <= now() + 30 * DAY;
};

function dueLabel(ts: number) {
  const days = Math.round((ts - now()) / DAY);
  if (days < 0) return { text: `vencido hace ${-days} días`, cls: "text-bad" };
  if (days === 0) return { text: "hoy", cls: "text-warn" };
  return { text: `en ${days} días`, cls: days <= 7 ? "text-warn" : "text-dim" };
}

const DOC = { none: "", quote: "Presupuesto", receipt: "Recibo" } as const;

export function Clients() {
  const [clients, setClients] = useState<Client[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [form, setForm] = useState<Client>(EMPTY);
  const [query, setQuery] = useState("");
  const [sending, setSending] = useState<string | null>(null);
  const [contacts, setContacts] = useState<Contact[]>([]);
  /** Parte de la ficha que se ve. */
  const [tab, setTab] = useState<"summary" | "machines" | "visits" | "data">("summary");
  /** Solo los que tienen el mantenimiento cerca o vencido. */
  const [onlyDue, setOnlyDue] = useState(false);
  const toast = useToast();
  useEffect(() => {
    contactsApi
      .list()
      .then((l) => setContacts(l.filter((c) => !c.deleted)))
      .catch(logQuietly("Clients"));
  }, []);
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    const c = await workApi.clients();
    setClients(c);
    return c;
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const pick = (c: Client | null) => {
    setSelected(c?.id ?? "new");
    setForm(c ?? EMPTY);
    // Uno nuevo empieza por sus datos; uno que ya existe, por el resumen.
    setTab(c ? "summary" : "data");
  };

  const save = async () => {
    try {
      const saved = await workApi.saveClient(form);
      toast("ok", "Cliente guardado.");
      await load();
      setSelected(saved.id);
      setForm(saved);
    } catch (e) {
      toast("error", String(e));
    }
  };

  const remove = async () => {
    const ok = await confirm({
      title: `¿Eliminar a ${form.name}?`,
      danger: true,
      confirmLabel: "Eliminar",
      body: <p>Se borra la ficha y su historial de sesiones. Los informes PDF ya generados no se eliminan.</p>,
    });
    if (!ok) return;
    await workApi.deleteClient(form.id);
    setSelected(null);
    void load();
  };

  const reschedule = async (c: Client, date: number | null) => {
    try {
      await workApi.setNextMaintenance(c.id, date);
      const all = await load();
      const fresh = all.find((x) => x.id === c.id);
      if (fresh && form.id === c.id) setForm(fresh);
      toast("ok", date ? `Mantenimiento movido al ${new Date(date * 1000).toLocaleDateString("es")}.` : "Recordatorio quitado.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (clients ?? []).filter((c) => (!onlyDue || dueSoon(c)) && (!q || [c.name, c.contact, c.phone, c.email].some((x) => x.toLowerCase().includes(q))));
  }, [clients, query, onlyDue]);

  const due = useMemo(() => (clients ?? []).filter(dueSoon).sort((a, b) => nextOf(a)! - nextOf(b)!), [clients]);

  const field = (key: keyof Client, label: string, wide = false) => (
    <label className={`block ${wide ? "col-span-2" : ""}`}>
      <span className="mb-1 block text-xs text-dim">{label}</span>
      <input value={form[key] as string} onChange={(e) => setForm({ ...form, [key]: e.target.value })} className={inputClass} />
    </label>
  );

  const warranties = form.sessions.flatMap((s) => s.warranties.map((w) => ({ ...w, session: s }))).filter((w) => w.until > now());
  const sendingRecord = form.sessions.find((s) => s.id === sending);

  const all = clients ?? [];
  const totals = {
    clients: all.length,
    machines: all.reduce((n, c) => n + c.machines.length, 0),
    due: due.length,
    warranties: all.reduce((n, c) => n + c.sessions.flatMap((x) => x.warranties).filter((w) => w.until > now()).length, 0),
  };
  const copy = (text: string, what: string) => navigator.clipboard.writeText(text).then(() => toast("ok", `${what} copiado.`), () => toast("error", "No se pudo copiar."));
  const last = form.sessions[0];
  const next = nextOf(form);

  return (
    <div className="mx-auto max-w-6xl space-y-4 p-6">
      {/* De un vistazo */}
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <Tile label="Clientes" value={totals.clients} />
        <Tile label="Equipos a tu cargo" value={totals.machines} />
        <Tile label="Mantenimiento cerca o vencido" value={totals.due} warn active={onlyDue} onClick={() => setOnlyDue(!onlyDue)} />
        <Tile label="Garantías vigentes" value={totals.warranties} />
      </div>

      <div className="grid grid-cols-12 gap-4">
        {/* Lista */}
        <div className="col-span-12 lg:col-span-4">
          <div className="mb-3 flex gap-2">
            <div className="relative flex-1">
              <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Buscar cliente…"
                className="w-full rounded-md border border-line bg-panel py-2 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
              />
            </div>
            <Button onClick={() => pick(null)}>
              <Plus size={13} /> Nuevo
            </Button>
          </div>
          {onlyDue && (
            <p className="mb-2 flex items-center gap-2 text-xs text-dim">
              Solo con el mantenimiento cerca o vencido
              <button onClick={() => setOnlyDue(false)} className="text-neon hover:underline">
                Ver todos
              </button>
            </p>
          )}
          <div className="overflow-hidden rounded-xl border border-line bg-panel">
            {visible.map((c, i) => {
              const n = nextOf(c);
              return (
                <button
                  key={c.id}
                  onClick={() => pick(c)}
                  className={`flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors ${i ? "border-t border-line/60" : ""} ${selected === c.id ? "bg-neon/10" : "hover:bg-panel-2/60"}`}
                >
                  <Avatar c={{ name: c.name, favorite: false }} size={34} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm text-ink">{c.name}</div>
                    <div className="truncate text-[11px] text-mute">
                      {c.machines.length} {c.machines.length === 1 ? "equipo" : "equipos"} · {c.sessions.length} {c.sessions.length === 1 ? "visita" : "visitas"}
                      {c.sessions[0] && ` · última ${date(c.sessions[0].ended)}`}
                    </div>
                  </div>
                  {n !== null && dueSoon(c) && <span className={`shrink-0 text-[11px] ${dueLabel(n).cls}`}>{dueLabel(n).text}</span>}
                </button>
              );
            })}
            {clients && visible.length === 0 && (
              <p className="px-4 py-6 text-center text-sm text-mute">{clients.length ? "Sin resultados." : "Aún no hay clientes. Se crean aquí o al iniciar una sesión."}</p>
            )}
          </div>
        </div>

        {/* Ficha */}
        <div className="col-span-12 space-y-4 lg:col-span-8">
          {selected === null ? (
            <div className="rounded-xl border border-dashed border-line-2 p-10 text-center">
              <UserRound size={26} className="mx-auto text-mute" />
              <p className="mt-2 text-sm text-ink">Elige un cliente</p>
              <p className="mx-auto mt-1 max-w-sm text-xs text-dim">Su resumen, sus equipos y sus visitas, con agendar y empezar la sesión a un clic.</p>
              {due.length > 0 && (
                <ul className="mx-auto mt-4 max-w-sm space-y-1 text-left">
                  {due.slice(0, 5).map((c) => (
                    <li key={c.id}>
                      <button onClick={() => pick(c)} className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm hover:bg-panel-2">
                        <BellRing size={13} className="shrink-0 text-warn" />
                        <span className="min-w-0 flex-1 truncate text-ink">{c.name}</span>
                        <span className={`text-[11px] ${dueLabel(nextOf(c)!).cls}`}>{dueLabel(nextOf(c)!).text}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          ) : (
            <>
              {form.id && (
                <section className="rounded-xl border border-line bg-panel p-4">
                  <div className="flex flex-wrap items-center gap-3">
                    <Avatar c={{ name: form.name, favorite: false }} size={48} />
                    <div className="min-w-0 flex-1">
                      <h2 className="truncate text-base font-semibold text-ink">{form.name}</h2>
                      <p className="truncate text-xs text-dim">{[form.contact, form.address].filter(Boolean).join(" · ") || "Sin persona de contacto"}</p>
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      <Button onClick={() => goToPage("session", form.id)} title="Empezar la sesión de servicio con este cliente">
                        <Play size={13} /> Empezar sesión
                      </Button>
                      <Button kind="ghost" onClick={() => goToPage("agenda", `client:${form.id}`)} title="Apuntar una visita en la Agenda con este cliente">
                        <CalendarPlus size={13} /> Agendar
                      </Button>
                    </div>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-x-5 gap-y-1 text-xs text-dim">
                    {form.phone && (
                      <button onClick={() => void copy(form.phone, "Teléfono")} className="flex items-center gap-1.5 font-mono hover:text-neon" title="Copiar">
                        <Phone size={12} className="text-mute" /> {form.phone} <Copy size={10} className="text-mute" />
                      </button>
                    )}
                    {form.email && (
                      <button onClick={() => void copy(form.email, "Correo")} className="flex items-center gap-1.5 hover:text-neon" title="Copiar">
                        <Mail size={12} className="text-mute" /> {form.email} <Copy size={10} className="text-mute" />
                      </button>
                    )}
                  </div>
                  <div className="mt-3 grid grid-cols-2 gap-2 sm:grid-cols-4">
                    <Kpi label="Equipos" value={String(form.machines.length)} />
                    <Kpi label="Visitas" value={String(form.sessions.length)} />
                    <Kpi label="Última visita" value={last ? date(last.ended) : "—"} />
                    <Kpi label="Próximo mantenimiento" value={next !== null ? date(next) : "Sin fecha"} tone={next !== null ? dueLabel(next).cls : undefined} />
                  </div>
                </section>
              )}

              {form.id && (
                <div className="flex gap-1 border-b border-line" role="tablist">
                  {(
                    [
                      ["summary", "Resumen"],
                      ["machines", `Equipos · ${form.machines.length}`],
                      ["visits", `Visitas · ${form.sessions.length}`],
                      ["data", "Datos y plantilla"],
                    ] as const
                  ).map(([id, label]) => (
                    <button
                      key={id}
                      role="tab"
                      aria-selected={tab === id}
                      onClick={() => setTab(id)}
                      className={`-mb-px border-b-2 px-3 py-2 text-sm transition-colors ${tab === id ? "border-neon font-medium text-ink" : "border-transparent text-dim hover:text-ink"}`}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              )}

              {(tab === "data" || !form.id) && (
                <Card title={form.id ? "Datos del cliente" : "Nuevo cliente"} icon={<UserRound size={14} />}>
                  <div className="grid grid-cols-2 gap-3">
                    {field("name", "Nombre o empresa", true)}
                    {field("contact", "Persona de contacto")}
                    {field("phone", "Teléfono")}
                    {field("email", "Correo")}
                    {field("address", "Dirección")}
                    <label className="col-span-2 block">
                      <span className="mb-1 block text-xs text-dim">Notas</span>
                      <textarea value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} rows={3} className={`${inputClass} resize-y`} />
                    </label>
                  </div>
                  <ClientReportFields value={form.report ?? EMPTY_CLIENT_REPORT} onChange={(report) => setForm({ ...form, report })} />
                  <div className="mt-4 flex items-center justify-between">
                    {form.id ? (
                      <button onClick={remove} className="flex items-center gap-1 text-xs text-mute hover:text-bad">
                        <Trash2 size={12} /> Eliminar
                      </button>
                    ) : (
                      <span />
                    )}
                    <Button onClick={save} disabled={!form.name.trim()}>
                      <Save size={13} /> Guardar
                    </Button>
                  </div>
                </Card>
              )}

              {form.id && tab === "summary" && (
                <>
                  {form.notes.trim() && <p className="rounded-xl border border-line bg-panel px-4 py-3 text-sm whitespace-pre-line text-dim">{form.notes}</p>}
                  {(next !== null || warranties.length > 0) && (
                    <Card title="Mantenimiento y garantías" icon={<ShieldCheck size={14} />}>
                      {next !== null && (
                        <div className="mb-3 flex flex-wrap items-center gap-3 text-sm">
                          <BellRing size={14} className="text-mute" />
                          <span className="text-ink">Próximo mantenimiento: {date(next)}</span>
                          <span className={`text-xs ${dueLabel(next).cls}`}>{dueLabel(next).text}</span>
                          <button onClick={() => reschedule(form, Math.max(next, now()) + 30 * DAY)} className="ml-auto text-xs text-mute hover:text-ink">
                            +1 mes
                          </button>
                          <button onClick={() => reschedule(form, null)} className="text-xs text-mute hover:text-ink" title="Ya se hizo o no hace falta">
                            Quitar
                          </button>
                        </div>
                      )}
                      {warranties.length > 0 ? (
                        <ul className="space-y-1 text-sm">
                          {warranties.map((w, i) => (
                            <li key={i} className="flex flex-wrap items-center gap-x-3">
                              <span className="size-1.5 shrink-0 rounded-full bg-ok" />
                              <span className="min-w-0 flex-1 truncate text-ink">{w.item}</span>
                              <span className="text-xs text-mute">
                                {w.session.number ? `Nº ${w.session.number} · ` : ""}
                                {w.session.host}
                              </span>
                              <span className="text-right text-xs text-dim">hasta el {date(w.until)}</span>
                            </li>
                          ))}
                        </ul>
                      ) : (
                        <p className="text-sm text-mute">Sin garantías vigentes.</p>
                      )}
                    </Card>
                  )}
                  <ClientContacts people={contacts.filter((c) => c.clientId === form.id)} />
                  <Evolution sessions={form.sessions} />
                  <VisitChanges clientId={form.id} refresh={form.sessions.length} />
                  <MachineCompare clientId={form.id} refresh={form.sessions.length} />
                  {form.network && form.network.devices.length > 0 && (
                    <Card title={`Red de la oficina · ${form.network.devices.length} dispositivos`} icon={<Network size={14} />}>
                      <p className="mb-2 text-xs text-mute">
                        {form.network.name && `${form.network.name} · `}router {form.network.gateway} · guardado el {date(form.network.saved)}
                      </p>
                      <ul className="grid grid-cols-1 gap-x-6 gap-y-0.5 text-xs md:grid-cols-2">
                        {form.network.devices.map((d) => (
                          <li key={d.ip + d.mac} className="flex gap-2">
                            <span className="w-24 shrink-0 font-mono text-ink">{d.ip}</span>
                            <span className="truncate text-dim">{d.alias || d.name || d.vendor || d.mac || "—"}</span>
                          </li>
                        ))}
                      </ul>
                    </Card>
                  )}
                  {form.sessions.length === 0 && next === null && (
                    <p className="rounded-xl border border-dashed border-line px-4 py-6 text-center text-sm text-mute">
                      Aún no hay visitas con este cliente. Al terminar la primera sesión de servicio aparecerán aquí sus equipos, la evolución y las garantías.
                    </p>
                  )}
                </>
              )}

              {form.id && tab === "machines" && (
                <Card title={`Equipos · ${form.machines.length}`} icon={<Monitor size={14} />}>
                  {form.machines.length === 0 ? (
                    <p className="text-sm text-mute">Los equipos se registran al finalizar una sesión de servicio o desde Soporte → Inventario.</p>
                  ) : (
                    <ul className="divide-y divide-line/60 text-sm">
                      {form.machines.map((m) => (
                        <li key={m.host} className="flex items-start gap-3 py-2">
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                              <span className="font-mono text-ink">{m.host}</span>
                              <VerdictChip inv={m.inventory} />
                            </div>
                            {m.inventory ? (
                              <>
                                {inventoryLines(m.inventory).map((l) => (
                                  <div key={l} className="truncate text-xs text-dim">
                                    {l}
                                  </div>
                                ))}
                                {m.inventory.reasons.map((r) => (
                                  <div key={r} className="text-xs text-ink">
                                    → {r}
                                  </div>
                                ))}
                              </>
                            ) : (
                              <div className="truncate text-xs text-dim" title={m.hardware}>
                                {m.os}
                                {m.hardware && ` · ${m.hardware}`}
                              </div>
                            )}
                          </div>
                          <div className="flex shrink-0 flex-col items-end gap-1">
                            <span className="text-xs text-mute">última visita {date(m.lastSeen)}</span>
                            <MachineActions machine={m} />
                          </div>
                        </li>
                      ))}
                    </ul>
                  )}
                </Card>
              )}

              {form.id && tab === "visits" && (
                <Card title={`Historial de visitas · ${form.sessions.length}`} icon={<History size={14} />}>
                  {form.sessions.length === 0 ? (
                    <p className="text-sm text-mute">Sin visitas todavía.</p>
                  ) : (
                    <ul className="space-y-2">
                      {form.sessions.map((s) => (
                        <li key={s.id} className="rounded-lg border border-line px-3 py-2">
                          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
                            <span className="text-ink">{date(s.ended)}</span>
                            {s.number && <span className="font-mono text-[11px] text-mute">Nº {s.number}</span>}
                            <span className="font-mono text-xs text-dim">{s.host}</span>
                            <span className="text-xs text-dim">{s.workItems} cambios</span>
                            {s.visitType && <span className="text-xs text-dim">{s.visitType}</span>}
                            {s.contactId && contacts.find((c) => c.id === s.contactId) && (
                              <span className="flex items-center gap-1 text-xs text-dim" title="Quién pidió el trabajo">
                                <UserRound size={11} /> {contacts.find((c) => c.id === s.contactId)!.name}
                              </span>
                            )}
                            {s.docKind !== "none" && (
                              <span className="text-xs text-dim">
                                {DOC[s.docKind]} · {money(s.total, s.currency)}
                              </span>
                            )}
                            {s.signed && (
                              <span className="flex items-center gap-1 text-xs text-ok" title="Firmado por el cliente en pantalla">
                                <PenLine size={11} /> firmado
                              </span>
                            )}
                            <span className="ml-auto font-mono text-[11px]" title="Críticos y advertencias: antes → después">
                              <span className="text-bad">
                                {s.badBefore}→{s.badAfter}
                              </span>{" "}
                              ·{" "}
                              <span className="text-warn">
                                {s.warnBefore}→{s.warnAfter}
                              </span>
                            </span>
                            {s.report && (
                              <>
                                <button
                                  onClick={() => diagApi.openReport(s.report!).catch((e) => toast("error", String(e)))}
                                  className="flex items-center gap-1 text-xs text-neon hover:underline"
                                >
                                  Informe <ExternalLink size={11} />
                                </button>
                                <button onClick={() => setSending(s.id)} className="text-mute hover:text-ink" title="Enviar el informe por correo">
                                  <Mail size={13} />
                                </button>
                              </>
                            )}
                          </div>
                          {s.hardwareChange && <p className="mt-1 text-xs text-warn">Hardware cambiado: {s.hardwareChange}</p>}
                          {s.notes && <p className="mt-1 line-clamp-2 text-xs text-dim">{s.notes}</p>}
                        </li>
                      ))}
                    </ul>
                  )}
                </Card>
              )}
            </>
          )}
        </div>
      </div>
      {sendingRecord?.report && <SendReportModal path={sendingRecord.report} client={form} onClose={() => setSending(null)} />}
      {dialog}
    </div>
  );
}

function Kpi({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return (
    <div className="rounded-lg bg-panel-2 px-3 py-2">
      <div className="text-[10px] tracking-wide text-mute uppercase">{label}</div>
      <div className={`truncate text-sm ${tone ?? "text-ink"}`}>{value}</div>
    </div>
  );
}

type Row = { label: string; get: (m: VisitMetrics) => number | null; fmt: (v: number) => string; upBetter: boolean | null };

const ROWS: Row[] = [
  { label: "Problemas críticos", get: (m) => m.bad, fmt: String, upBetter: false },
  { label: "Advertencias", get: (m) => m.warn, fmt: String, upBetter: false },
  { label: "Nota de seguridad", get: (m) => m.security, fmt: (v) => `${v}/100`, upBetter: true },
  { label: "Libre en el disco del sistema", get: (m) => m.sysFree, fmt: (v) => bytes(v), upBetter: true },
  { label: "Programas de inicio", get: (m) => m.startup, fmt: String, upBetter: false },
  { label: "Programas por actualizar", get: (m) => m.updates, fmt: String, upBetter: false },
  { label: "Último arranque", get: (m) => m.bootMs, fmt: (v) => `${(v / 1000).toFixed(1)} s`, upBetter: false },
  { label: "Batería", get: (m) => m.batteryHealth, fmt: (v) => `${Math.round(v)}%`, upBetter: true },
  { label: "Memoria", get: (m) => m.ramTotal || null, fmt: (v) => bytes(v, 0), upBetter: null },
];

/** Personas de la agenda enlazadas a este cliente (se enlazan desde Contactos). */
function ClientContacts({ people }: { people: Contact[] }) {
  const toast = useToast();
  if (!people.length) return null;
  return (
    <Card title={`Contactos · ${people.length}`} icon={<UserRound size={14} />}>
      <ul className="grid gap-x-6 gap-y-1 md:grid-cols-2">
        {people.map((c) => {
          const reach = c.extension ? `ext. ${c.extension}` : c.phone || c.mobile || c.email;
          return (
            <li key={c.id} className="flex items-center gap-2 py-1 text-sm">
              <span className="min-w-0 flex-1">
                <span className="text-ink">{c.name}</span>
                {(c.role || c.reason) && <span className="block truncate text-[11px] text-mute">{[c.role, c.reason && `para: ${c.reason}`].filter(Boolean).join(" · ")}</span>}
              </span>
              {reach && (
                <button
                  onClick={() => navigator.clipboard.writeText(c.extension || c.phone || c.mobile || c.email).then(() => toast("ok", "Copiado."))}
                  className="font-mono text-xs text-dim hover:text-neon"
                  title="Copiar"
                >
                  {reach}
                </button>
              )}
            </li>
          );
        })}
      </ul>
    </Card>
  );
}

/** Plantilla de informe de este cliente: formato, presentación y el correo con el que se le envía. */
function ClientReportFields({ value, onChange }: { value: ClientReport; onChange: (v: ClientReport) => void }) {
  const custom = !!(value.template || value.intro || value.to || value.subject || value.body);
  return (
    <details className="mt-4 rounded-lg border border-line px-3 py-2" open={custom}>
      <summary className="cursor-pointer text-xs text-dim select-none">
        Plantilla de informe de este cliente {custom ? <span className="text-neon">· personalizada</span> : <span className="text-mute">· la de siempre</span>}
      </summary>
      <div className="mt-3 space-y-3">
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Formato</span>
          <select value={value.template ?? ""} onChange={(e) => onChange({ ...value, template: (e.target.value || null) as ClientReport["template"] })} className={inputClass}>
            <option value="">Para el cliente (resumen claro)</option>
            <option value="technical">Técnico (todo el detalle)</option>
          </select>
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Presentación (va al principio del informe)</span>
          <textarea
            value={value.intro}
            onChange={(e) => onChange({ ...value, intro: e.target.value })}
            rows={3}
            placeholder="Mantenimiento trimestral según el contrato de soporte. Equipos revisados de la oficina de {cliente}."
            className={`${inputClass} resize-y`}
          />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Enviar también a</span>
          <input value={value.to} onChange={(e) => onChange({ ...value, to: e.target.value })} placeholder="gerencia@cliente.com, contabilidad@cliente.com" className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Asunto del correo</span>
          <input value={value.subject} onChange={(e) => onChange({ ...value, subject: e.target.value })} placeholder="Informe Nº {numero} · Mantenimiento de {cliente}" className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Mensaje del correo</span>
          <textarea
            value={value.body}
            onChange={(e) => onChange({ ...value, body: e.target.value })}
            rows={5}
            placeholder={"Hola {contacto}:\n\nAdjunto el informe del mantenimiento del {fecha}.\n\nUn saludo,\n{tecnico}"}
            className={`${inputClass} resize-y`}
          />
        </label>
        <p className="text-[11px] text-mute">
          Campos que se rellenan solos: {"{cliente} {contacto} {numero} {fecha} {empresa} {tecnico}"}. Vacío = el texto de siempre. Se guarda con «Guardar».
        </p>
      </div>
    </details>
  );
}

/** Cómo ha evolucionado el equipo del cliente de una visita a otra. */
function Evolution({ sessions }: { sessions: SessionRecord[] }) {
  const visits = sessions.filter((s) => s.metrics).slice(0, 5).reverse();
  if (visits.length < 2) return null;
  const rows = ROWS.filter((r) => visits.some((v) => r.get(v.metrics!) !== null));
  const tone = (r: Row, cur: number | null, prev: number | null) => {
    if (cur === null || prev === null || r.upBetter === null) return "text-ink";
    const delta = r.label === "Libre en el disco del sistema" && Math.abs(cur - prev) < 1024 ** 3 ? 0 : cur - prev;
    if (delta === 0) return "text-ink";
    return delta > 0 === r.upBetter ? "text-ok" : "text-bad";
  };
  return (
    <Card title="Evolución entre visitas" icon={<History size={14} />}>
      <div className="overflow-x-auto">
        <DataTable
          rows={rows}
          rowKey={(r) => r.label}
          columns={[
            { id: "label", header: "", cell: (r) => r.label, className: "text-dim" },
            ...visits.map((v, i): Column<Row> => {
              // Se compara con la visita anterior del mismo equipo, no con la de otro.
              const before = visits.slice(0, i).reverse().find((x) => x.host.toLowerCase() === v.host.toLowerCase());
              const prevOf = (r: Row) => (before ? r.get(before.metrics!) : null);
              return {
                id: v.id,
                align: "right",
                header: (
                  <span className="flex flex-col items-end">
                    <span className="text-dim">{date(v.ended)}</span>
                    <span className="font-mono text-[10px]">{v.host}</span>
                  </span>
                ),
                cell: (r) => {
                  const cur = r.get(v.metrics!);
                  return cur === null ? "—" : r.fmt(cur);
                },
                className: (r) => `font-mono text-xs ${tone(r, r.get(v.metrics!), prevOf(r))}`,
              };
            }),
          ]}
        />
      </div>
      <p className="mt-2 text-[11px] text-mute">Cifras al terminar cada visita. En verde lo que mejoró respecto a la visita anterior del mismo equipo; en rojo lo que empeoró.</p>
    </Card>
  );
}
