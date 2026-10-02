// La barra del caso de ahora.
//
// Mientras hay un caso abierto se ve arriba en cualquier página: a quién se
// atiende, qué equipo, cuánto lleva y cuántas acciones van apuntadas. Todo lo
// que se hace en AdminOps queda en el diario, y al cerrar el caso se redacta la
// resolución con eso, para pegarla en el ticket en vez de escribirla de memoria.
import { ClipboardCheck, Copy, Crop, Loader2, Pencil, Send, TicketPlus, Trash2, X } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { casesApi, noteApi, portalsApi, type Case, type ClipRedacted } from "../lib/api";
import { CASE_CHANGED_EVENT, caseChanged, elapsed, OPEN_CASE_EVENT } from "../lib/currentCase";
import { goToPage } from "../lib/navigate";
import { lastPortalKey } from "../lib/portalState";
import { usePrefs } from "../lib/prefs";
import { useLiveEffect } from "../lib/useLiveEffect";
import { useToast } from "./feedback";
import { Button, inputClass, Modal } from "./ui";

/** Botón de la cabecera para abrir un caso cuando no hay ninguno. */
export function NewCaseButton({ hidden }: { hidden: boolean }) {
  const prefs = usePrefs();
  if (hidden || prefs.mode === "user") return null;
  return (
    <button
      onClick={() => window.dispatchEvent(new CustomEvent(OPEN_CASE_EVENT, { detail: {} }))}
      className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-mute transition-colors hover:bg-panel-2 hover:text-ink"
      title="Abrir un caso: lo que hagas queda apuntado y la resolución se redacta sola"
    >
      <TicketPlus size={13} /> Nuevo caso
    </button>
  );
}

/** Lo que se tapó en un recorte, dicho en una línea. */
export function clipMessage(r: ClipRedacted): { kind: "ok" | "info" | "error"; text: string } {
  if (r.error) return { kind: "error", text: `Recorte en el portapapeles, pero sin tapar: ${r.error}` };
  if (r.covered > 0) return { kind: "ok", text: `Recorte listo: ${r.covered} ${r.covered === 1 ? "dato personal tapado" : "datos personales tapados"} (rutas, usuario o equipo). Pégalo con Ctrl+V.` };
  return { kind: "info", text: r.words > 0 ? "Recorte listo, sin datos personales a la vista. Pégalo con Ctrl+V." : "Recorte listo. Pégalo con Ctrl+V." };
}

export function CaseBar({ onOpenChange }: { onOpenChange?: (open: boolean) => void }) {
  const prefs = usePrefs();
  const [current, setCurrent] = useState<Case | null>(null);
  const [count, setCount] = useState(0);
  const [last, setLast] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now() / 1000);
  const [dialog, setDialog] = useState<{ kind: "new" | "edit"; prefill: Partial<Case> } | { kind: "close" } | null>(null);
  const toast = useToast();

  const reload = useCallback(() => {
    void casesApi
      .current()
      .then((c) => setCurrent(c))
      .catch(() => {});
  }, []);

  useEffect(() => {
    reload();
    window.addEventListener(CASE_CHANGED_EVENT, reload);
    // Un caso abierto desde la nota de llamada, que es otra ventana.
    const off = listen("case-changed", reload);
    return () => {
      window.removeEventListener(CASE_CHANGED_EVENT, reload);
      void off.then((f) => f());
    };
  }, [reload]);

  // Abrir un caso desde cualquier sitio (ficha de la persona, Ctrl+K, cabecera).
  useEffect(() => {
    const onOpen = (e: Event) => {
      const prefill = (e as CustomEvent<Partial<Case>>).detail ?? {};
      if (current) {
        toast("info", "Ya tienes un caso abierto. Ciérralo antes de abrir otro.");
        setDialog({ kind: "close" });
        return;
      }
      setDialog({ kind: "new", prefill });
    };
    window.addEventListener(OPEN_CASE_EVENT, onOpen);
    return () => window.removeEventListener(OPEN_CASE_EVENT, onOpen);
  }, [current, toast]);

  // El recorte de pantalla avisa de lo que tapó (el OCR va por detrás, en Rust).
  useEffect(() => {
    const show = (r: ClipRedacted) => {
      const m = clipMessage(r);
      toast(m.kind, m.text);
    };
    const off = listen<ClipRedacted>("screen-clip", (e) => show(e.payload));
    // «Tapar datos personales del portapapeles», desde Ctrl+K.
    const onClip = (e: Event) => show((e as CustomEvent<ClipRedacted>).detail);
    window.addEventListener("adminops:clip", onClip);
    return () => {
      window.removeEventListener("adminops:clip", onClip);
      void off.then((f) => f());
    };
  }, [toast]);

  // Los diálogos tapan los portales (sus vistas van por encima de la interfaz).
  useEffect(() => onOpenChange?.(dialog !== null), [dialog, onOpenChange]);

  // Reloj del caso y acciones apuntadas, solo mientras hay uno abierto.
  useLiveEffect(
    (vigente) => {
      if (!current) return;
      const tick = window.setInterval(() => setNow(Date.now() / 1000), 1000);
      const leer = () =>
        void casesApi
          .actions()
          .then((a) => {
            if (!vigente()) return;
            setCount(a.length);
            setLast(a.length ? a[a.length - 1].title : null);
          })
          .catch(() => {});
      leer();
      const poll = window.setInterval(leer, 10_000);
      window.addEventListener("focus", leer);
      return () => {
        window.clearInterval(tick);
        window.clearInterval(poll);
        window.removeEventListener("focus", leer);
      };
    },
    [current],
  );

  if (prefs.mode === "user") return null;

  return (
    <>
      {current && (
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 border-b border-neon/25 bg-neon/[0.07] px-8 py-2 text-[13px]">
          <span className="flex items-center gap-1.5 rounded-full border border-neon/40 px-2 py-0.5 text-xs text-neon">
            <span className="size-1.5 rounded-full bg-neon" />
            {current.ticket ? `Caso ${current.ticket}` : "Caso abierto"}
          </span>
          {current.person && <span className="font-medium text-ink">{current.person}</span>}
          {current.machine && <span className="font-mono text-xs text-dim">{current.machine}</span>}
          <span className="font-mono text-xs text-neon tabular">{elapsed(now - current.started)}</span>
          <span className="min-w-0 flex-1 truncate text-xs text-mute" title={last ?? undefined}>
            {count === 0 ? "Nada apuntado todavía: lo que hagas en AdminOps aparecerá solo." : `${count} ${count === 1 ? "acción apuntada" : "acciones apuntadas"} · última: ${last}`}
          </span>
          <button
            onClick={() => void noteApi.screenClip().catch((e) => toast("error", String(e)))}
            className="rounded-md p-1 text-mute hover:bg-panel-2 hover:text-ink"
            title="Recorte de pantalla: se tapan solas las rutas, el usuario y el equipo, y queda en el portapapeles para pegarlo en el ticket"
          >
            <Crop size={13} />
          </button>
          <button onClick={() => setDialog({ kind: "edit", prefill: current })} className="rounded-md p-1 text-mute hover:bg-panel-2 hover:text-ink" title="Editar el caso">
            <Pencil size={13} />
          </button>
          <Button onClick={() => setDialog({ kind: "close" })}>
            <ClipboardCheck size={14} /> Cerrar caso
          </Button>
        </div>
      )}

      {dialog && dialog.kind !== "close" && (
        <CaseEditor
          mode={dialog.kind}
          prefill={dialog.prefill}
          onClose={() => setDialog(null)}
          onSaved={() => {
            setDialog(null);
            caseChanged();
          }}
        />
      )}
      {dialog?.kind === "close" && current && (
        <CloseDialog
          onClose={() => setDialog(null)}
          onClosed={() => {
            setDialog(null);
            setCount(0);
            setLast(null);
            caseChanged();
          }}
        />
      )}
    </>
  );
}

/** Abrir un caso nuevo o cambiar los datos del abierto. */
function CaseEditor({ mode, prefill, onClose, onSaved }: { mode: "new" | "edit"; prefill: Partial<Case>; onClose: () => void; onSaved: () => void }) {
  const [c, setC] = useState<Partial<Case>>({ ticket: "", person: "", sam: "", machine: "", notes: "", ...prefill });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      if (mode === "new") await casesApi.open(c);
      else await casesApi.update(c);
      onSaved();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  const field = (key: "ticket" | "person" | "machine", label: string, placeholder: string, mono = false) => (
    <label className="block">
      <span className="mb-1 block text-xs text-dim">{label}</span>
      <input value={c[key] ?? ""} onChange={(e) => setC({ ...c, [key]: e.target.value })} placeholder={placeholder} className={`${inputClass} ${mono ? "font-mono" : ""}`} />
    </label>
  );

  return (
    <Modal
      title={mode === "new" ? "Nuevo caso" : "Editar el caso"}
      onClose={onClose}
      width="w-[520px]"
      footer={
        <>
          {error && <p className="mr-auto max-w-64 text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={() => void save()} disabled={busy}>
            {busy && <Loader2 size={14} className="animate-spin" />} {mode === "new" ? "Abrir caso" : "Guardar"}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          {field("ticket", "Ticket", "#4521")}
          {field("machine", "Equipo", "PC-CONTA-03", true)}
        </div>
        {field("person", "Persona", "María Pérez")}
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Qué le pasa</span>
          <textarea value={c.notes ?? ""} onChange={(e) => setC({ ...c, notes: e.target.value })} rows={3} placeholder="No imprime desde esta mañana…" className={inputClass} />
        </label>
        {mode === "new" && (
          <p className="text-[11px] text-mute">
            Todo es opcional. Mientras el caso esté abierto, lo que hagas en AdminOps queda apuntado solo, y al cerrarlo se redacta la resolución para el ticket.
          </p>
        )}
      </div>
    </Modal>
  );
}

/** Cerrar el caso: la resolución redactada, editable, lista para el ticket. */
function CloseDialog({ onClose, onClosed }: { onClose: () => void; onClosed: () => void }) {
  const [text, setText] = useState<string | null>(null);
  const [busy, setBusy] = useState<"close" | "paste" | "discard" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const toast = useToast();

  useLiveEffect((vigente) => {
    void casesApi
      .draft()
      .then((t) => vigente() && setText(t))
      .catch((e) => vigente() && setError(String(e)));
  }, []);

  const copy = () =>
    navigator.clipboard.writeText(text ?? "").then(
      () => true,
      () => false,
    );

  const paste = async () => {
    if (!text) return;
    setBusy("paste");
    try {
      await copy();
      let id: string | null = null;
      try {
        id = localStorage.getItem(lastPortalKey(""));
      } catch {
        /* sin almacenamiento */
      }
      const escrito = id ? await portalsApi.insertText(id, text) : false;
      toast(
        escrito ? "ok" : "info",
        escrito
          ? "Escrito en el campo del ticket. Revísalo y guárdalo en el portal."
          : "Copiado. En el portal de Tickets, pulsa en el campo de la resolución y pega con Ctrl+V.",
      );
      goToPage("tickets");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const close = async () => {
    setBusy("close");
    try {
      await casesApi.close(text ?? "");
      toast("ok", "Caso cerrado. Queda en la ficha de la persona.");
      onClosed();
    } catch (e) {
      setError(String(e));
      setBusy(null);
    }
  };

  const discard = async () => {
    setBusy("discard");
    try {
      await casesApi.discard();
      onClosed();
    } catch (e) {
      setError(String(e));
      setBusy(null);
    }
  };

  return (
    <Modal
      title="Cerrar el caso"
      onClose={onClose}
      width="w-[620px]"
      footer={
        <>
          {confirmDiscard ? (
            <span className="mr-auto flex items-center gap-2 text-xs text-bad">
              ¿Descartarlo sin guardar?
              <button onClick={() => void discard()} className="font-medium underline" disabled={busy !== null}>
                Sí, descartar
              </button>
              <button onClick={() => setConfirmDiscard(false)} className="text-mute hover:text-ink">
                No
              </button>
            </span>
          ) : (
            <button onClick={() => setConfirmDiscard(true)} className="mr-auto flex items-center gap-1 text-xs text-mute hover:text-bad" title="Para un caso abierto por error">
              <Trash2 size={12} /> Descartar
            </button>
          )}
          <Button kind="ghost" onClick={() => void copy().then((ok) => toast(ok ? "ok" : "error", ok ? "Resolución copiada." : "No se pudo copiar."))} disabled={!text}>
            <Copy size={14} /> Copiar
          </Button>
          <Button kind="ghost" onClick={() => void paste()} disabled={!text || busy !== null} title="Escribe la resolución en el campo del ticket que tengas seleccionado en el portal">
            {busy === "paste" ? <Loader2 size={14} className="animate-spin" /> : <Send size={14} />} Pegar en Tickets
          </Button>
          <Button onClick={() => void close()} disabled={text === null || busy !== null}>
            {busy === "close" ? <Loader2 size={14} className="animate-spin" /> : <ClipboardCheck size={14} />} Cerrar caso
          </Button>
        </>
      }
    >
      {error && <p className="mb-3 flex items-center gap-2 text-xs text-bad"><X size={12} />{error}</p>}
      {text === null && !error ? (
        <p className="flex items-center gap-2 text-sm text-mute">
          <Loader2 size={14} className="animate-spin" /> Redactando la resolución…
        </p>
      ) : (
        <>
          <textarea value={text ?? ""} onChange={(e) => setText(e.target.value)} rows={13} className={`${inputClass} font-mono text-[12.5px] leading-relaxed`} aria-label="Resolución del caso" />
          <p className="mt-2 text-[11px] text-mute">
            Sale del diario: cada línea es algo que se hizo de verdad en AdminOps. Puedes retocarla antes de pegarla. Para «Pegar en Tickets», deja antes seleccionado en el portal el
            campo donde va la resolución.
          </p>
        </>
      )}
    </Modal>
  );
}
