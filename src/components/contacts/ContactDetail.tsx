import { Briefcase, Clock, History, ClipboardCopy, Copy, Mail, MessageSquare, Pencil, Phone, PhoneCall, Smartphone, Star, Trash2, UserRound, Users, X } from "lucide-react";
import type { ReactNode } from "react";
import type { Client, Contact } from "../../lib/api";
import { colorOf, TagChip, type TagColors } from "./Tags";

/** Lo que se puede hacer con un contacto (lo implementa la página: cuenta el uso). */
export interface ContactActions {
  copy: (c: Contact, text: string, what: string) => void;
  call: (c: Contact, number: string) => void;
  email: (c: Contact, address: string) => void;
  teams: (c: Contact, address: string, call: boolean) => void;
  copyCard: (c: Contact) => void;
  edit: (c: Contact) => void;
  trash: (c: Contact) => void;
  star: (c: Contact) => void;
  open: (c: Contact) => void;
}

function Line({ icon, label, value, children }: { icon: ReactNode; label: string; value: ReactNode; children?: ReactNode }) {
  return (
    <div className="group flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-panel-2">
      <span className="shrink-0 text-mute">{icon}</span>
      <span className="min-w-0 flex-1">
        <span className="block text-[10px] tracking-wide text-mute uppercase">{label}</span>
        <span className="block truncate text-sm text-ink select-text">{value}</span>
      </span>
      <span className="flex shrink-0 gap-0.5 opacity-60 group-hover:opacity-100">{children}</span>
    </div>
  );
}

const Btn = ({ title, onClick, children }: { title: string; onClick: () => void; children: ReactNode }) => (
  <button onClick={onClick} title={title} className="rounded p-1.5 text-mute hover:bg-neon/10 hover:text-neon">
    {children}
  </button>
);

/** Ficha del contacto en un panel lateral. */
export function ContactDetail({
  contact: c,
  byId,
  clients,
  colors,
  actions: a,
  onClose,
}: {
  contact: Contact;
  byId: Map<string, Contact>;
  clients: Client[];
  colors: TagColors;
  actions: ContactActions;
  onClose: () => void;
}) {
  const sub = c.substituteId ? byId.get(c.substituteId) : undefined;
  const client = c.clientId ? clients.find((x) => x.id === c.clientId) : undefined;
  const phone = (label: string, value: string, icon: ReactNode) => (
    <Line key={label + value} icon={icon} label={label} value={<span className="font-mono">{value}</span>}>
      <Btn title="Llamar" onClick={() => a.call(c, value)}>
        <PhoneCall size={13} />
      </Btn>
      <Btn title="Copiar" onClick={() => a.copy(c, value, label)}>
        <Copy size={13} />
      </Btn>
    </Line>
  );
  const mail = (label: string, value: string) => (
    <Line key={label + value} icon={<Mail size={14} />} label={label} value={value}>
      <Btn title="Escribir un correo" onClick={() => a.email(c, value)}>
        <Mail size={13} />
      </Btn>
      <Btn title="Chat de Teams" onClick={() => a.teams(c, value, false)}>
        <MessageSquare size={13} />
      </Btn>
      <Btn title="Llamada de Teams" onClick={() => a.teams(c, value, true)}>
        <PhoneCall size={13} />
      </Btn>
      <Btn title="Copiar" onClick={() => a.copy(c, value, "Correo")}>
        <Copy size={13} />
      </Btn>
    </Line>
  );
  // Referencia de los usuarios que usan esta ficha: el sustituto de otros.
  const substituteOf = [...byId.values()].filter((x) => x.substituteId === c.id && !x.deleted);
  // Visitas que pidió esta persona (sesiones de servicio con ella como contacto).
  const visits = clients
    .flatMap((cl) => cl.sessions.filter((v) => v.contactId === c.id).map((v) => ({ ...v, client: cl.name })))
    .sort((a, b) => b.ended - a.ended);

  return (
    <aside className="fixed top-0 right-0 bottom-0 z-30 flex w-[400px] max-w-[92vw] flex-col border-l border-line-2 bg-panel shadow-2xl">
      <header className="flex items-start gap-3 border-b border-line px-5 py-4">
        <span className="grid size-10 shrink-0 place-items-center rounded-full bg-neon/15 text-sm font-semibold text-neon">
          {c.name
            .split(/\s+/)
            .slice(0, 2)
            .map((w) => w.charAt(0).toUpperCase())
            .join("")}
        </span>
        <div className="min-w-0 flex-1">
          <h3 className="text-base font-semibold break-words text-ink">{c.name}</h3>
          {(c.role || c.company) && <p className="text-xs text-dim">{[c.role, c.company].filter(Boolean).join(" · ")}</p>}
        </div>
        <button onClick={() => a.star(c)} className={c.favorite ? "text-warn" : "text-mute hover:text-warn"} title="Favorito">
          <Star size={16} fill={c.favorite ? "currentColor" : "none"} />
        </button>
        <button onClick={onClose} className="text-mute hover:text-ink" title="Cerrar (Esc)">
          <X size={16} />
        </button>
      </header>

      <div className="flex-1 space-y-4 overflow-y-auto px-3 py-3">
        {c.reason && <p className="mx-2 rounded-md border border-neon/30 bg-neon/5 px-3 py-2 text-sm text-ink">Para: {c.reason}</p>}
        <div>
          {c.extension && phone("Extensión", c.extension, <Phone size={14} />)}
          {c.phone && phone("Teléfono", c.phone, <Phone size={14} />)}
          {c.mobile && phone("Móvil", c.mobile, <Smartphone size={14} />)}
          {c.channels.filter((x) => x.kind === "phone").map((x) => phone(x.label || "Teléfono", x.value, <Phone size={14} />))}
          {c.email && mail("Correo", c.email)}
          {c.channels.filter((x) => x.kind === "email").map((x) => mail(x.label || "Correo", x.value))}
        </div>
        {c.availability && <Line icon={<Clock size={14} />} label="Disponibilidad" value={c.availability} />}
        {sub && (
          <Line icon={<UserRound size={14} />} label="Si no está, llamar a" value={`${sub.name}${sub.extension ? ` · ext. ${sub.extension}` : ""}`}>
            <button onClick={() => a.open(sub)} className="rounded px-1.5 py-0.5 text-[11px] text-neon hover:bg-neon/10">
              Ver
            </button>
          </Line>
        )}
        {substituteOf.length > 0 && (
          <Line icon={<Users size={14} />} label="Sustituye a" value={substituteOf.map((x) => x.name).join(", ")} />
        )}
        {client && <Line icon={<Briefcase size={14} />} label="Cliente" value={client.name} />}
        {c.tags.length > 0 && (
          <div className="flex flex-wrap gap-1 px-2">
            {c.tags.map((t) => (
              <TagChip key={t} name={t} color={colorOf(colors, t)} />
            ))}
          </div>
        )}
        {c.notes && <p className="mx-2 text-xs whitespace-pre-wrap text-dim select-text">{c.notes}</p>}
        {visits.length > 0 && (
          <div className="px-2">
            <p className="mb-1 flex items-center gap-1 text-[10px] tracking-wide text-mute uppercase">
              <History size={11} /> Visitas que pidió · {visits.length}
            </p>
            <ul className="space-y-1">
              {visits.slice(0, 10).map((v) => (
                <li key={v.id} className="text-xs text-dim">
                  <span className="text-ink">{new Date(v.ended * 1000).toLocaleDateString("es")}</span> · {v.client} · {v.host}
                  {v.visitType && ` · ${v.visitType}`}
                  {v.workItems > 0 && ` · ${v.workItems} cambios`}
                </li>
              ))}
            </ul>
          </div>
        )}
        <p className="px-2 text-[11px] text-mute">
          {c.uses > 0 ? `Usado ${c.uses} ${c.uses === 1 ? "vez" : "veces"}` : "Aún no usado"}
          {c.lastUsed > 0 && ` · última el ${new Date(c.lastUsed * 1000).toLocaleDateString("es")}`}
        </p>
      </div>

      <footer className="flex gap-2 border-t border-line px-5 py-3">
        <button onClick={() => a.copyCard(c)} className="flex flex-1 items-center justify-center gap-1.5 rounded-md border border-neon/40 py-1.5 text-sm text-neon hover:bg-neon/10">
          <ClipboardCopy size={14} /> Copiar tarjeta
        </button>
        <button onClick={() => a.edit(c)} className="flex items-center gap-1.5 rounded-md border border-line-2 px-3 py-1.5 text-sm text-dim hover:text-ink">
          <Pencil size={13} /> Editar
        </button>
        <button onClick={() => a.trash(c)} className="rounded-md border border-line-2 px-2.5 py-1.5 text-mute hover:border-bad/50 hover:text-bad" title="A la papelera">
          <Trash2 size={13} />
        </button>
      </footer>
    </aside>
  );
}
