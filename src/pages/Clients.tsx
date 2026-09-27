import { BellRing, ExternalLink, History, Mail, Monitor, Network, PenLine, Plus, Save, Search, ShieldCheck, Trash2, UserRound } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { inventoryLines, MachineActions, VerdictChip } from "../components/inventory";
import { SendReportModal } from "../components/service";
import { Button, Card, inputClass } from "../components/ui";
import { diagApi, workApi, type Client, type SessionRecord, type VisitMetrics } from "../lib/api";
import { bytes, money } from "../lib/format";

const DAY = 86400;
const date = (ts: number) => new Date(ts * 1000).toLocaleDateString("es", { dateStyle: "medium" });
const now = () => Date.now() / 1000;
const EMPTY = { id: "", name: "", contact: "", phone: "", email: "", address: "", notes: "", created: 0, machines: [], sessions: [], network: null } as Client;

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
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    const c = await workApi.clients();
    setClients(c);
    return c;
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const pick = (c: Client | null) => {
    setSelected(c?.id ?? "new");
    setForm(c ?? EMPTY);
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
    load();
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
    return (clients ?? []).filter((c) => !q || [c.name, c.contact, c.phone, c.email].some((x) => x.toLowerCase().includes(q)));
  }, [clients, query]);

  const due = useMemo(() => (clients ?? []).filter(dueSoon).sort((a, b) => nextOf(a)! - nextOf(b)!), [clients]);

  const field = (key: keyof Client, label: string, wide = false) => (
    <label className={`block ${wide ? "col-span-2" : ""}`}>
      <span className="mb-1 block text-xs text-dim">{label}</span>
      <input value={form[key] as string} onChange={(e) => setForm({ ...form, [key]: e.target.value })} className={inputClass} />
    </label>
  );

  const warranties = form.sessions.flatMap((s) => s.warranties.map((w) => ({ ...w, session: s }))).filter((w) => w.until > now());
  const sendingRecord = form.sessions.find((s) => s.id === sending);

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <div className="col-span-12 space-y-4 lg:col-span-4">
        {due.length > 0 && (
          <Card title={`Mantenimientos próximos · ${due.length}`} icon={<BellRing size={14} />}>
            <ul className="space-y-2">
              {due.map((c) => {
                const n = nextOf(c)!;
                const l = dueLabel(n);
                return (
                  <li key={c.id} className="text-sm">
                    <button onClick={() => pick(c)} className="block w-full truncate text-left text-ink hover:underline">
                      {c.name}
                    </button>
                    <div className="flex items-center gap-2 text-xs">
                      <span className={l.cls}>
                        {date(n)} · {l.text}
                      </span>
                      <button onClick={() => reschedule(c, Math.max(n, now()) + 30 * DAY)} className="ml-auto text-mute hover:text-ink">
                        +1 mes
                      </button>
                      <button onClick={() => reschedule(c, null)} className="text-mute hover:text-ink" title="Ya se hizo o no hace falta">
                        Quitar
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
            <p className="mt-3 text-[11px] text-mute">Llama o escribe al cliente para agendar la visita.</p>
          </Card>
        )}

        <div>
          <div className="mb-3 flex gap-2">
            <div className="relative flex-1">
              <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Buscar cliente…"
                className="w-full rounded-md border border-line bg-panel py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
              />
            </div>
            <Button onClick={() => pick(null)}>
              <Plus size={13} /> Nuevo
            </Button>
          </div>
          <div className="overflow-hidden rounded-xl border border-line bg-panel">
            {visible.map((c, i) => (
              <button
                key={c.id}
                onClick={() => pick(c)}
                className={`flex w-full items-center gap-3 px-4 py-2.5 text-left ${i ? "border-t border-line/60" : ""} ${selected === c.id ? "bg-panel-2" : "hover:bg-panel-2/60"}`}
              >
                <UserRound size={15} className="shrink-0 text-mute" />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm text-ink">{c.name}</div>
                  <div className="truncate text-[11px] text-mute">
                    {c.machines.length} equipo(s) · {c.sessions.length} visita(s)
                  </div>
                </div>
                {dueSoon(c) && <span className="size-2 shrink-0 rounded-full bg-warn" title="Mantenimiento próximo o vencido" />}
              </button>
            ))}
            {clients && visible.length === 0 && (
              <p className="px-4 py-6 text-center text-sm text-mute">{clients.length ? "Sin resultados." : "Aún no hay clientes. Se crean aquí o al iniciar una sesión."}</p>
            )}
          </div>
        </div>
      </div>

      <div className="col-span-12 space-y-4 lg:col-span-8">
        {selected === null ? (
          <p className="rounded-xl border border-dashed border-line-2 p-10 text-center text-sm text-mute">Selecciona un cliente para ver su ficha.</p>
        ) : (
          <>
            <Card title={form.id ? "Ficha del cliente" : "Nuevo cliente"} icon={<UserRound size={14} />}>
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
              <div className="mt-4 flex items-center justify-between">
                {form.id ? (
                  <button onClick={remove} className="flex items-center gap-1 text-xs text-mute hover:text-bad">
                    <Trash2 size={12} /> Eliminar
                  </button>
                ) : (
                  <span />
                )}
                <Button onClick={save}>
                  <Save size={13} /> Guardar
                </Button>
              </div>
            </Card>

            {form.id && (
              <>
                {(nextOf(form) !== null || warranties.length > 0) && (
                  <Card title="Mantenimiento y garantías" icon={<ShieldCheck size={14} />}>
                    {nextOf(form) !== null && (
                      <div className="mb-3 flex items-center gap-3 text-sm">
                        <BellRing size={14} className="text-mute" />
                        <span className="text-ink">Próximo mantenimiento: {date(nextOf(form)!)}</span>
                        <span className={`text-xs ${dueLabel(nextOf(form)!).cls}`}>{dueLabel(nextOf(form)!).text}</span>
                        <button onClick={() => reschedule(form, Math.max(nextOf(form)!, now()) + 30 * DAY)} className="ml-auto text-xs text-mute hover:text-ink">
                          +1 mes
                        </button>
                        <button onClick={() => reschedule(form, null)} className="text-xs text-mute hover:text-ink">
                          Quitar
                        </button>
                      </div>
                    )}
                    {warranties.length > 0 ? (
                      <ul className="space-y-1 text-sm">
                        {warranties.map((w, i) => (
                          <li key={i} className="flex items-center gap-3">
                            <span className="size-1.5 shrink-0 rounded-full bg-ok" />
                            <span className="min-w-0 flex-1 truncate text-ink">{w.item}</span>
                            <span className="text-xs text-mute">
                              {w.session.number ? `Nº ${w.session.number} · ` : ""}
                              {w.session.host}
                            </span>
                            <span className="w-40 text-right text-xs text-dim">hasta el {date(w.until)}</span>
                          </li>
                        ))}
                      </ul>
                    ) : (
                      <p className="text-sm text-mute">Sin garantías vigentes.</p>
                    )}
                  </Card>
                )}

                <Evolution sessions={form.sessions} />

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

                <Card title={`Historial de visitas · ${form.sessions.length}`}>
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
              </>
            )}
          </>
        )}
      </div>
      {sendingRecord?.report && <SendReportModal path={sendingRecord.report} client={form} onClose={() => setSending(null)} />}
      {dialog}
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
        <table className="w-full text-[13px]">
          <thead>
            <tr className="text-xs text-mute">
              <th className="pb-2 text-left font-medium" />
              {visits.map((v) => (
                <th key={v.id} className="pb-2 pl-3 text-right font-medium">
                  <span className="block text-dim">{date(v.ended)}</span>
                  <span className="block font-mono text-[10px]">{v.host}</span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.label} className="border-t border-line/60">
                <td className="py-1.5 text-dim">{r.label}</td>
                {visits.map((v, i) => {
                  const cur = r.get(v.metrics!);
                  const prev = i > 0 ? r.get(visits[i - 1].metrics!) : null;
                  return (
                    <td key={v.id} className={`py-1.5 pl-3 text-right font-mono text-xs ${i === 0 ? "text-ink" : tone(r, cur, prev)}`}>
                      {cur === null ? "—" : r.fmt(cur)}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="mt-2 text-[11px] text-mute">Cifras al terminar cada visita. En verde lo que mejoró respecto a la anterior; en rojo lo que empeoró.</p>
    </Card>
  );
}
