// «Enviar a…» desde cualquier tarjeta: un único botón para llevar lo que ves a
// donde haga falta: portapapeles (texto, tabla para Excel o imagen), las notas
// del caso abierto, un correo o un mensaje de Teams.
import { ClipboardCopy, FileSpreadsheet, Images, LifeBuoy, Mail, MessagesSquare, Send } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { casesApi, logQuietly, uxApi } from "../lib/api";
import { copyAsImage, NO_CAPTURE } from "../lib/copyImage";
import { openComm } from "../lib/comms";
import { caseChanged, openCase } from "../lib/currentCase";
import { appendNote, tableRows, tableToTsv, textOf } from "../lib/sendTo";
import { useToast } from "./feedback";

export function SendTo({ target, title }: { target: () => HTMLElement | null; title: string }) {
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    window.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);

  const text = () => {
    const el = target();
    return el ? textOf(el, "data-no-capture").replace(new RegExp(`^${title.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*`), "") : "";
  };
  const done = (msg: string) => {
    setOpen(false);
    toast("ok", msg);
  };
  const fail = (e: unknown) => {
    setOpen(false);
    toast("error", String(e));
  };
  const rows = target() ? tableRows(target()!) : null;

  const items: { label: string; icon: ReactNode; run: () => void | Promise<unknown> }[] = [
    { label: "Copiar como texto", icon: <ClipboardCopy size={13} />, run: () => navigator.clipboard.writeText(`${title}\n${text()}`).then(() => done("Copiado como texto."), fail) },
    ...(rows
      ? [{ label: "Copiar como tabla para Excel", icon: <FileSpreadsheet size={13} />, run: () => navigator.clipboard.writeText(tableToTsv(rows)).then(() => done("Tabla copiada: pégala en Excel."), fail) }]
      : []),
    {
      label: "Copiar como imagen",
      icon: <Images size={13} />,
      run: () => {
        const el = target();
        return el ? copyAsImage(el).then(() => done("Imagen copiada: pégala en Teams o en el correo."), fail) : undefined;
      },
    },
    {
      label: "Añadir a las notas del caso",
      icon: <LifeBuoy size={13} />,
      run: () =>
        casesApi
          .current()
          .then(async (c) => {
            if (!c) {
              setOpen(false);
              openCase({ notes: `${title}\n${text()}` });
              return;
            }
            await casesApi.update({ ...c, notes: appendNote(c.notes, title, text()) });
            caseChanged();
            done("Añadido a las notas del caso.");
          })
          .catch(fail),
    },
    {
      label: "Enviar por correo",
      icon: <Mail size={13} />,
      run: () => {
        const body = `${title}\n${text()}`;
        void navigator.clipboard.writeText(body).catch(logQuietly("SendTo"));
        return uxApi.composeMail(title, body).then(() => done("Correo preparado en tu programa de correo."), fail);
      },
    },
    {
      label: "Enviar por Teams",
      icon: <MessagesSquare size={13} />,
      run: () =>
        navigator.clipboard.writeText(`${title}\n${text()}`).then(() => {
          openComm("teams");
          done("Copiado: pégalo en el chat de Teams.");
        }, fail),
    },
  ];

  return (
    <div ref={box} className="relative" {...NO_CAPTURE}>
      <button
        onClick={() => setOpen((o) => !o)}
        title="Enviar a…"
        aria-label={`Enviar «${title}» a…`}
        aria-expanded={open}
        data-tour="sendto"
        className={`rounded p-1 transition-opacity focus:opacity-100 ${open ? "text-neon" : "text-mute opacity-0 group-hover/card:opacity-100 hover:text-ink"}`}
      >
        <Send size={13} />
      </button>
      {open && (
        <div role="menu" className="absolute top-full right-0 z-30 mt-1 w-60 rounded-lg border border-line-2 bg-panel p-1 shadow-2xl">
          <p className="px-2.5 pt-1 pb-1.5 text-[11px] text-mute">Enviar «{title}» a…</p>
          {items.map((it) => (
            <button key={it.label} role="menuitem" onClick={() => void it.run()} className="flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-[13px] text-dim hover:bg-neon/10 hover:text-ink">
              <span className="text-mute">{it.icon}</span>
              {it.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
