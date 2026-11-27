// «Notas»: la ventanita que sale con Ctrl+Alt+N o el botón de la barra del cohete; se guarda en el cuaderno.
//
// Suena el teléfono mientras estás con otra cosa: quién llama, desde qué equipo
// y qué le pasa, y con un clic se convierte en un caso (que empieza a apuntar lo
// que hagas) o en un seguimiento para más tarde. Vive en su propia ventana, siempre
// encima, así que no hace falta cambiar de lo que estabas haciendo.
import { emit } from "@tauri-apps/api/event";
import { CalendarClock, Loader2, TicketPlus, StickyNote } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { casesApi, followupsApi, noteApi, tomorrowMorning, notesApi } from "../lib/api";
import { Notice } from "./ui";

const input =
  "w-full rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50";

export function QuickNote() {
  const [title, setTitle] = useState("");
  const [text, setText] = useState("");
  const [busy, setBusy] = useState<"case" | "later" | "save" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => {
    first.current?.focus();
    // Esc cierra la nota sin guardar.
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && void noteApi.close();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const empty = !title.trim() && !text.trim();
  // El título es libre (no siempre es quién llama): al caso o a «para mañana» va delante del texto, no como persona.
  const joined = [title.trim(), text.trim()].filter(Boolean).join(": ");

  const asCase = async () => {
    setBusy("case");
    setError(null);
    try {
      await casesApi.open({ notes: [title.trim(), text.trim()].filter(Boolean).join("\n") });
      // La ventana principal enseña la barra del caso en cuanto se entera.
      await emit("case-changed");
      await noteApi.close();
    } catch (e) {
      setError(String(e));
      setBusy(null);
    }
  };

  const later = async () => {
    setBusy("later");
    setError(null);
    try {
      await followupsApi.add({ text: joined || "Nota pendiente", due: tomorrowMorning() });
      await emit("followups-changed");
      await noteApi.close();
    } catch (e) {
      setError(String(e));
      setBusy(null);
    }
  };

  // Lo normal: guardarla en el cuaderno (Conocimiento → Notas). Caso y «para mañana» siguen a mano.
  const save = async () => {
    setBusy("save");
    setError(null);
    try {
      await notesApi.save({ id: "", title: title.trim(), text, created: 0, updated: 0, pinned: false, images: [] });
      await emit("notes-changed");
      await noteApi.close();
    } catch (e) {
      setError(String(e));
      setBusy(null);
    }
  };

  return (
    <div className="flex h-full flex-col gap-3 bg-void p-4 text-ink">
      <div className="flex items-center gap-2 text-sm font-semibold">
        <StickyNote size={15} className="text-neon" /> Nota
      </div>
      <label className="block">
        <span className="mb-1 block text-xs text-dim">Título (opcional)</span>
        <input ref={first} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Laura · contabilidad, impresora, contraseña wifi…" className={input} />
      </label>
      <label className="block flex-1">
        <span className="mb-1 block text-xs text-dim">Nota</span>
        <textarea value={text} onChange={(e) => setText(e.target.value)} placeholder="Escribe lo que quieras apuntar…" className={`${input} h-36 resize-none`} />
      </label>
      {error && <Notice tone="error" where="Nota rápida">{error}</Notice>}
      <button
        onClick={() => void save()}
        disabled={empty || busy !== null}
        className="flex h-9 items-center justify-center gap-1.5 rounded-lg bg-neon px-3 text-[13px] font-medium text-on-neon hover:brightness-110 disabled:opacity-40"
      >
        {busy === "save" ? <Loader2 size={14} className="animate-spin" /> : <StickyNote size={14} />} Guardar nota
      </button>
      <div className="flex gap-2">
        <button onClick={() => void asCase()} disabled={empty || busy !== null} className="flex h-8 flex-1 items-center justify-center gap-1.5 rounded-lg border border-line-2 px-3 text-[12px] text-ink hover:bg-panel-2 disabled:opacity-40">
          {busy === "case" ? <Loader2 size={13} className="animate-spin" /> : <TicketPlus size={13} />} Abrir caso
        </button>
        <button onClick={() => void later()} disabled={empty || busy !== null} className="flex h-8 flex-1 items-center justify-center gap-1.5 rounded-lg border border-line-2 px-3 text-[12px] text-ink hover:bg-panel-2 disabled:opacity-40" title="Aparecerá mañana a las 9 en «Hoy», con un aviso">
          {busy === "later" ? <Loader2 size={13} className="animate-spin" /> : <CalendarClock size={13} />} Para mañana
        </button>
      </div>
      <p className="text-center text-[11px] text-mute">Todas las notas: Conocimiento → Notas · Ctrl+Alt+N · Esc cierra</p>
    </div>
  );
}
