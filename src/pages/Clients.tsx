import { ExternalLink, Monitor, Plus, Save, Search, Trash2, UserRound } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Card } from "../components/ui";
import { diagApi, workApi, type Client } from "../lib/api";

const date = (ts: number) => new Date(ts * 1000).toLocaleDateString("es", { dateStyle: "medium" });
const EMPTY = { id: "", name: "", contact: "", phone: "", email: "", address: "", notes: "", created: 0, machines: [], sessions: [] } as Client;

export function Clients() {
  const [clients, setClients] = useState<Client[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [form, setForm] = useState<Client>(EMPTY);
  const [query, setQuery] = useState("");
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

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (clients ?? []).filter((c) => !q || [c.name, c.contact, c.phone, c.email].some((x) => x.toLowerCase().includes(q)));
  }, [clients, query]);

  const field = (key: keyof Client, label: string, wide = false) => (
    <label className={`block ${wide ? "col-span-2" : ""}`}>
      <span className="mb-1 block text-xs text-dim">{label}</span>
      <input
        value={form[key] as string}
        onChange={(e) => setForm({ ...form, [key]: e.target.value })}
        className="w-full rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none focus:border-neon/50"
      />
    </label>
  );

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <div className="col-span-12 lg:col-span-4">
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
          <button onClick={() => pick(null)} className="flex items-center gap-1 rounded-md border border-neon/50 px-3 text-xs text-neon hover:bg-neon/10">
            <Plus size={13} /> Nuevo
          </button>
        </div>
        <div className="overflow-hidden rounded-xl border border-line bg-panel">
          {visible.map((c, i) => (
            <button
              key={c.id}
              onClick={() => pick(c)}
              className={`flex w-full items-center gap-3 px-4 py-2.5 text-left ${i ? "border-t border-line/60" : ""} ${selected === c.id ? "bg-neon/10" : "hover:bg-panel-2"}`}
            >
              <UserRound size={15} className="shrink-0 text-neon/70" />
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm text-ink">{c.name}</div>
                <div className="truncate text-[11px] text-mute">
                  {c.machines.length} equipo(s) · {c.sessions.length} sesión(es)
                </div>
              </div>
            </button>
          ))}
          {clients && visible.length === 0 && (
            <p className="px-4 py-6 text-center text-sm text-mute">{clients.length ? "Sin resultados." : "Aún no hay clientes. Se crean aquí o al iniciar una sesión."}</p>
          )}
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
                  <textarea
                    value={form.notes}
                    onChange={(e) => setForm({ ...form, notes: e.target.value })}
                    rows={3}
                    className="w-full resize-y rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none focus:border-neon/50"
                  />
                </label>
              </div>
              <div className="mt-4 flex justify-between">
                {form.id ? (
                  <button onClick={remove} className="flex items-center gap-1 text-xs text-mute hover:text-bad">
                    <Trash2 size={12} /> Eliminar
                  </button>
                ) : (
                  <span />
                )}
                <button onClick={save} className="flex items-center gap-1.5 rounded-md border border-neon/50 bg-neon/10 px-4 py-1.5 text-sm text-neon hover:bg-neon/20">
                  <Save size={13} /> Guardar
                </button>
              </div>
            </Card>

            {form.id && (
              <>
                <Card title={`Equipos · ${form.machines.length}`} icon={<Monitor size={14} />}>
                  {form.machines.length === 0 ? (
                    <p className="text-sm text-mute">Los equipos se registran al finalizar una sesión de servicio.</p>
                  ) : (
                    <ul className="space-y-1 text-sm">
                      {form.machines.map((m) => (
                        <li key={m.host} className="flex items-center gap-3">
                          <span className="font-mono text-ink">{m.host}</span>
                          <span className="min-w-0 flex-1 truncate text-xs text-dim" title={m.hardware}>
                            {m.os}
                            {m.hardware && <span className="block truncate text-[11px] text-mute">{m.hardware}</span>}
                          </span>
                          <span className="ml-auto text-xs text-mute">última visita {date(m.lastSeen)}</span>
                        </li>
                      ))}
                    </ul>
                  )}
                </Card>

                <Card title={`Historial de servicio · ${form.sessions.length}`}>
                  {form.sessions.length === 0 ? (
                    <p className="text-sm text-mute">Sin sesiones todavía.</p>
                  ) : (
                    <ul className="space-y-2">
                      {form.sessions.map((s) => (
                        <li key={s.id} className="rounded-lg border border-line bg-void/40 px-3 py-2">
                          <div className="flex items-center gap-3 text-sm">
                            <span className="text-ink">{date(s.ended)}</span>
                            <span className="font-mono text-xs text-dim">{s.host}</span>
                            <span className="text-xs text-dim">{s.workItems} cambios</span>
                            <span className="ml-auto font-mono text-[11px]">
                              <span className="text-bad">{s.badBefore}→{s.badAfter}</span> · <span className="text-warn">{s.warnBefore}→{s.warnAfter}</span>
                            </span>
                            {s.report && (
                              <button
                                onClick={() => diagApi.openReport(s.report!).catch((e) => toast("error", String(e)))}
                                className="flex items-center gap-1 text-xs text-neon hover:underline"
                              >
                                Informe <ExternalLink size={11} />
                              </button>
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
      {dialog}
    </div>
  );
}
