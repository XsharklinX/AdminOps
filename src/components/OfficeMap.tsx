import { Eye, MapPinned, Phone, Save, UserRound } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "./feedback";
import { Button, Card, inputClass } from "./ui";
import { contactsApi, lanApi, officeMapApi, type Contact, type DeviceMeta, type LanDevice } from "../lib/api";

export const macKey = (mac: string) => mac.trim().toLowerCase().replace(/-/g, ":");

export type WatchMap = Record<string, { up: boolean; since: number; checked: number }>;

/** Datos del mapa de la oficina de la red actual (se recargan al guardar). */
export function useOfficeMap() {
  const [key, setKeyState] = useState<string>("");
  // El intervalo lee la clave actual, no la del primer render.
  const keyRef = useRef("");
  const setKey = useCallback((k: string) => {
    keyRef.current = k;
    setKeyState(k);
  }, []);
  const [meta, setMeta] = useState<Record<string, DeviceMeta>>({});
  const [watch, setWatch] = useState<WatchMap>({});
  const [contacts, setContacts] = useState<Contact[]>([]);

  const reload = useCallback(async (k?: string) => {
    const net = k ?? keyRef.current;
    if (!net) return;
    const [m, w] = await Promise.all([officeMapApi.get(net).catch(() => ({})), officeMapApi.watchStatus(net).catch(() => ({}))]);
    setMeta(m);
    setWatch(w);
  }, []);

  useEffect(() => {
    lanApi
      .info()
      .then((i) => {
        if (i?.key) {
          setKey(i.key);
          reload(i.key);
        }
      })
      .catch(() => {});
    contactsApi
      .list()
      .then((l) => setContacts(l.filter((c) => !c.deleted)))
      .catch(() => {});
    // Estado de la vigilancia: se refresca cada minuto mientras la página está abierta.
    const t = window.setInterval(() => reload(), 60_000);
    return () => window.clearInterval(t);
  }, [reload, setKey]);

  return { key, setKey, meta, watch, contacts, reload };
}

function contactLine(c: Contact | undefined) {
  if (!c) return null;
  return `${c.name}${c.extension ? ` · ext. ${c.extension}` : c.phone ? ` · ${c.phone}` : ""}`;
}

/** Lo anotado de la red actual: qué es cada dispositivo, quién se encarga y si está vigilado. */
export function OfficeMapCard({
  meta,
  watch,
  contacts,
  scanned,
  onOpen,
}: {
  meta: Record<string, DeviceMeta>;
  watch: WatchMap;
  contacts: Contact[];
  scanned: LanDevice[] | null;
  onOpen: (mac: string) => void;
}) {
  const toast = useToast();
  const entries = Object.entries(meta).sort((a, b) => Number(b[1].watch) - Number(a[1].watch) || (a[1].role || a[1].name).localeCompare(b[1].role || b[1].name, "es"));
  if (!entries.length) return null;
  const byId = new Map(contacts.map((c) => [c.id, c]));
  return (
    <Card title="Mapa de la oficina" icon={<MapPinned size={14} />}>
      <p className="mb-3 text-xs text-dim">Lo que has anotado de esta red. Los marcados con el ojo se vigilan: si dejan de responder, AdminOps avisa.</p>
      <div className="grid gap-2 md:grid-cols-2">
        {entries.map(([mac, m]) => {
          const w = watch[mac];
          const seen = scanned?.find((d) => macKey(d.mac) === mac);
          const contact = byId.get(m.contactId);
          const state = m.watch ? (w ? (w.up ? "up" : "down") : "pending") : seen ? "up" : scanned ? "absent" : "unknown";
          const dot = { up: "bg-ok", down: "bg-bad", pending: "bg-mute", absent: "bg-warn", unknown: "bg-line-2" }[state];
          const label = { up: "Responde", down: "No responde", pending: "Comprobando…", absent: "No apareció en la búsqueda", unknown: "" }[state];
          return (
            <button key={mac} onClick={() => onOpen(mac)} className="flex items-start gap-2.5 rounded-lg border border-line bg-void/30 px-3 py-2 text-left hover:border-line-2">
              <span className={`mt-1.5 size-2 shrink-0 rounded-full ${dot}`} title={label} />
              <span className="min-w-0 flex-1">
                <span className="flex items-center gap-1.5 text-sm text-ink">
                  {m.role || m.name || mac}
                  {m.watch && <Eye size={11} className="text-neon" />}
                </span>
                <span className="block font-mono text-[11px] text-mute">
                  {m.ip || "—"}
                  {m.role && m.name ? ` · ${m.name}` : ""}
                </span>
                {contact && (
                  <span
                    role="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      const v = contact.extension || contact.phone || contact.mobile || contact.email;
                      if (v) navigator.clipboard.writeText(v).then(() => toast("ok", `${contact.name}: copiado.`));
                    }}
                    className="mt-0.5 flex items-center gap-1 text-[11px] text-neon hover:underline"
                    title="Copiar su extensión o teléfono"
                  >
                    <UserRound size={10} /> {contactLine(contact)}
                  </span>
                )}
                {m.notes && <span className="mt-0.5 line-clamp-2 block text-[11px] text-dim">{m.notes}</span>}
              </span>
              {label && state !== "up" && <span className={`shrink-0 text-[10px] ${state === "down" ? "text-bad" : "text-mute"}`}>{label}</span>}
            </button>
          );
        })}
      </div>
    </Card>
  );
}

/** En la ficha de un dispositivo: función, responsable, notas y vigilancia. */
export function DeviceOfficeForm({
  netKey,
  device,
  initial,
  contacts,
  onSaved,
}: {
  netKey: string;
  device: { mac: string; ip: string; name: string };
  initial?: DeviceMeta;
  contacts: Contact[];
  onSaved: () => void;
}) {
  const [role, setRole] = useState(initial?.role ?? "");
  const [contactId, setContactId] = useState(initial?.contactId ?? "");
  const [notes, setNotes] = useState(initial?.notes ?? "");
  const [watch, setWatch] = useState(initial?.watch ?? false);
  const toast = useToast();
  const contact = contacts.find((c) => c.id === contactId);

  if (!device.mac) return <p className="text-xs text-mute">Sin MAC no se puede anotar (no se podría reconocer en otra búsqueda).</p>;
  if (!netKey) return null;

  const save = async () => {
    try {
      await officeMapApi.save(netKey, device.mac, { role, contactId, notes, watch, ip: device.ip, name: device.name });
      toast("ok", watch ? "Guardado. AdminOps avisará si deja de responder." : "Guardado en el mapa de la oficina.");
      onSaved();
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <div className="space-y-2 rounded-lg border border-line p-3">
      <p className="flex items-center gap-1.5 text-xs font-medium text-ink">
        <MapPinned size={12} className="text-neon" /> En la oficina
      </p>
      <div className="grid grid-cols-2 gap-2">
        <input value={role} onChange={(e) => setRole(e.target.value)} placeholder="Para qué sirve: impresora de contabilidad…" className={inputClass} />
        <select value={contactId} onChange={(e) => setContactId(e.target.value)} className={inputClass}>
          <option value="">Responsable: nadie</option>
          {contacts.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
              {c.extension ? ` (ext. ${c.extension})` : ""}
            </option>
          ))}
        </select>
      </div>
      <textarea value={notes} onChange={(e) => setNotes(e.target.value)} rows={2} placeholder="Notas: clave de administración en la pegatina, tóner compatible…" className={inputClass} />
      {contact && (
        <p className="flex items-center gap-1 text-[11px] text-dim">
          <Phone size={10} /> {contactLine(contact)}
        </p>
      )}
      <div className="flex items-center justify-between">
        <label className="flex items-center gap-2 text-xs text-dim">
          <input type="checkbox" checked={watch} onChange={(e) => setWatch(e.target.checked)} className="accent-[var(--color-neon)]" />
          Vigilar: avisar si deja de responder (cada minuto, con AdminOps abierta)
        </label>
        <Button kind="ghost" onClick={save}>
          <Save size={13} /> Guardar
        </Button>
      </div>
    </div>
  );
}
