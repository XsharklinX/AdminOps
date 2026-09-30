import { Mail, Phone, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { Button, inputClass, Modal } from "../ui";
import type { Client, Contact, ContactChannel } from "../../lib/api";
import { TagInput, type TagColors } from "./Tags";

/** Alta y edición de un contacto. */
export function ContactEditor({
  initial,
  contacts,
  clients,
  tagSuggestions,
  colors,
  onSave,
  onClose,
}: {
  initial: Contact;
  contacts: Contact[];
  clients: Client[];
  tagSuggestions: string[];
  colors: TagColors;
  onSave: (c: Contact) => Promise<boolean>;
  onClose: () => void;
}) {
  const [c, setC] = useState<Contact>(initial);
  const [saving, setSaving] = useState(false);
  const set = (patch: Partial<Contact>) => setC((x) => ({ ...x, ...patch }));

  const field = (key: keyof Contact, label: string, placeholder = "", span = "") => (
    <label className={`block ${span}`}>
      <span className="mb-1 block text-[11px] text-mute">{label}</span>
      <input value={c[key] as string} onChange={(e) => set({ [key]: e.target.value } as Partial<Contact>)} placeholder={placeholder} className={inputClass} />
    </label>
  );

  const setChannel = (i: number, patch: Partial<ContactChannel>) => set({ channels: c.channels.map((x, j) => (j === i ? { ...x, ...patch } : x)) });

  const save = async () => {
    setSaving(true);
    const ok = await onSave(c);
    setSaving(false);
    if (ok) onClose();
  };

  return (
    <Modal
      title={c.id ? `Editar · ${initial.name}` : "Nuevo contacto"}
      onClose={onClose}
      width="w-[760px] max-w-[95vw]"
      footer={
        <>
          <span className="mr-auto text-[11px] text-mute">Ctrl+Enter para guardar</span>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save} disabled={saving}>
            Guardar
          </Button>
        </>
      }
    >
      <div
        className="grid grid-cols-2 gap-3 md:grid-cols-4"
        onKeyDown={(e) => {
          if (e.key === "Enter" && e.ctrlKey) {
            e.preventDefault();
            void save();
          }
        }}
      >
        <label className="col-span-2 block">
          <span className="mb-1 block text-[11px] text-mute">Nombre</span>
          <input autoFocus value={c.name} onChange={(e) => set({ name: e.target.value })} placeholder="Ana López" className={inputClass} />
        </label>
        {field("role", "Cargo o área", "Sistemas")}
        {field("company", "Empresa", "Proveedor, cliente…")}
        {field("extension", "Extensión", "2104")}
        {field("phone", "Teléfono")}
        {field("mobile", "Móvil")}
        {field("email", "Correo")}

        <div className="col-span-2 md:col-span-4">
          {c.channels.map((ch, i) => (
            <div key={i} className="mb-2 flex items-center gap-2">
              {ch.kind === "email" ? <Mail size={13} className="shrink-0 text-mute" /> : <Phone size={13} className="shrink-0 text-mute" />}
              <input value={ch.label} onChange={(e) => setChannel(i, { label: e.target.value })} placeholder="Guardia, personal…" className={`${inputClass} w-40`} />
              <input value={ch.value} onChange={(e) => setChannel(i, { value: e.target.value })} placeholder={ch.kind === "email" ? "correo@…" : "número"} className={inputClass} />
              <button onClick={() => set({ channels: c.channels.filter((_, j) => j !== i) })} className="p-1 text-mute hover:text-bad" title="Quitar">
                <Trash2 size={13} />
              </button>
            </div>
          ))}
          <div className="flex gap-3 text-xs">
            <button onClick={() => set({ channels: [...c.channels, { kind: "phone", label: "", value: "" }] })} className="flex items-center gap-1 text-neon hover:underline">
              <Plus size={11} /> Otro teléfono
            </button>
            <button onClick={() => set({ channels: [...c.channels, { kind: "email", label: "", value: "" }] })} className="flex items-center gap-1 text-neon hover:underline">
              <Plus size={11} /> Otro correo
            </button>
          </div>
        </div>

        {field("reason", "Para qué llamarle", "Altas de usuarios, impresoras, la fibra…", "col-span-2")}
        {field("availability", "Disponibilidad", "Solo mañanas, guardia fines de semana…", "col-span-2")}

        <label className="col-span-2 block">
          <span className="mb-1 block text-[11px] text-mute">Si no está, llamar a</span>
          <select value={c.substituteId} onChange={(e) => set({ substituteId: e.target.value })} className={inputClass}>
            <option value="">— Nadie —</option>
            {contacts
              .filter((x) => x.id !== c.id && !x.deleted)
              .map((x) => (
                <option key={x.id} value={x.id}>
                  {x.name}
                  {x.extension ? ` (ext. ${x.extension})` : ""}
                </option>
              ))}
          </select>
        </label>
        <label className="col-span-2 block">
          <span className="mb-1 block text-[11px] text-mute">Cliente de AdminOps</span>
          <select value={c.clientId} onChange={(e) => set({ clientId: e.target.value })} className={inputClass}>
            <option value="">— Ninguno —</option>
            {clients.map((x) => (
              <option key={x.id} value={x.id}>
                {x.name}
              </option>
            ))}
          </select>
        </label>

        <div className="col-span-2 md:col-span-4">
          <span className="mb-1 block text-[11px] text-mute">Etiquetas (Enter, coma o Tab para añadir otra)</span>
          <TagInput value={c.tags} onChange={(tags) => set({ tags })} suggestions={tagSuggestions} colors={colors} />
        </div>
        <label className="col-span-2 block md:col-span-4">
          <span className="mb-1 block text-[11px] text-mute">Notas</span>
          <textarea value={c.notes} onChange={(e) => set({ notes: e.target.value })} rows={3} className={inputClass} />
        </label>
      </div>
    </Modal>
  );
}
