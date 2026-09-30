import { Copy, Mail, MessageSquare, PhoneCall, Star } from "lucide-react";
import type { MouseEvent } from "react";
import type { Contact } from "../../lib/api";
import { Avatar } from "./Avatar";
import type { ContactActions } from "./ContactDetail";

const stop = (fn: () => void) => (e: MouseEvent) => {
  e.stopPropagation();
  fn();
};

/**
 * Marcación rápida: los favoritos y los que más se usan, con la extensión en
 * grande y las acciones a un clic. Lo que se hace veinte veces al día.
 */
export function QuickDial({ items, actions: a }: { items: Contact[]; actions: ContactActions }) {
  if (items.length === 0) return null;
  return (
    <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
      {items.map((c) => {
        const number = c.extension || c.phone || c.mobile;
        return (
          <div
            key={c.id}
            onClick={() => a.open(c)}
            className="group flex cursor-pointer items-center gap-2.5 rounded-xl border border-line bg-panel px-3 py-2.5 transition-colors hover:border-neon/40"
          >
            <Avatar c={c} size={34} />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1">
                <span className="truncate text-sm font-medium text-ink">{c.name}</span>
                {c.favorite && <Star size={10} className="shrink-0 text-warn" fill="currentColor" />}
              </div>
              <div className="truncate text-[11px] text-mute">{c.role || c.company || c.reason || " "}</div>
            </div>
            <div className="flex shrink-0 flex-col items-end gap-0.5">
              {c.extension && (
                <button onClick={stop(() => a.copy(c, c.extension, "Extensión"))} className="font-mono text-base leading-none font-semibold text-ink hover:text-neon" title="Copiar extensión">
                  {c.extension}
                </button>
              )}
              <span className="flex gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                {number && (
                  <button onClick={stop(() => a.call(c, number))} className="rounded p-0.5 text-mute hover:text-neon" title={`Llamar al ${number}`}>
                    <PhoneCall size={12} />
                  </button>
                )}
                {c.email && (
                  <>
                    <button onClick={stop(() => a.teams(c, c.email, false))} className="rounded p-0.5 text-mute hover:text-neon" title="Chat de Teams">
                      <MessageSquare size={12} />
                    </button>
                    <button onClick={stop(() => a.email(c, c.email))} className="rounded p-0.5 text-mute hover:text-neon" title="Escribir un correo">
                      <Mail size={12} />
                    </button>
                  </>
                )}
                <button onClick={stop(() => a.copyCard(c))} className="rounded p-0.5 text-mute hover:text-neon" title="Copiar tarjeta">
                  <Copy size={12} />
                </button>
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}
