import { PhoneCall } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "./feedback";
import { logQuietly, contactsApi, type Contact } from "../lib/api";
import { responsibleFor, TOPIC_OF } from "../lib/contacts";

let cache: { at: number; list: Contact[] } | null = null;

/** La agenda, con una caché corta (se usa en varios sitios a la vez). */
function useContacts() {
  const [list, setList] = useState<Contact[]>(cache?.list ?? []);
  useEffect(() => {
    if (cache && Date.now() - cache.at < 30_000) return;
    contactsApi
      .list()
      .then((l) => {
        cache = { at: Date.now(), list: l };
        setList(l);
      })
      .catch(logQuietly("Responsible"));
  }, []);
  return list;
}

/** «Si no se arregla, llamar a…»: el contacto que se encarga de ese tema. */
export function Responsible({ topic, className = "" }: { topic: string; className?: string }) {
  const contacts = useContacts();
  const toast = useToast();
  const who = responsibleFor(TOPIC_OF[topic] ?? topic, contacts);
  if (!who.length) return null;
  return (
    <div className={`flex flex-wrap items-center gap-2 text-xs ${className}`}>
      <span className="flex items-center gap-1 text-mute">
        <PhoneCall size={11} /> Responsable:
      </span>
      {who.map((c) => {
        const reach = c.extension ? `ext. ${c.extension}` : c.phone || c.mobile || c.email;
        return (
          <button
            key={c.id}
            onClick={() => {
              if (!reach) return;
              void navigator.clipboard.writeText(c.extension || c.phone || c.mobile || c.email).then(() => toast("ok", `${c.name}: ${reach} copiado.`), () => toast("error", "No se pudo copiar."));
              contactsApi.touch(c.id).catch(() => {});
            }}
            className="rounded-md border border-line px-2 py-0.5 text-dim hover:border-neon/50 hover:text-neon"
            title={c.reason ? `Para: ${c.reason}` : "Copiar"}
          >
            {c.name}
            {reach && <span className="ml-1 font-mono text-mute">{reach}</span>}
          </button>
        );
      })}
    </div>
  );
}
