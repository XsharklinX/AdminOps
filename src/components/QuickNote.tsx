// La nota de llamada: la ventanita que sale con Ctrl+Alt+N.
//
// Suena el teléfono mientras estás con otra cosa: quién llama, desde qué equipo
// y qué le pasa, y con un clic se convierte en un caso (que empieza a apuntar lo
// que hagas) o en un seguimiento para más tarde. Vive en su propia ventana, siempre
// encima, así que no hace falta cambiar de lo que estabas haciendo.
import { emit } from "@tauri-apps/api/event";
import { CalendarClock, Loader2, PhoneCall, TicketPlus } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { casesApi, followupsApi, noteApi, tomorrowMorning } from "../lib/api";

const input =
  "w-full rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50";

export function QuickNote() {
  const [person, setPerson] = useState("");
  const [machine, setMachine] = useState("");
  const [text, setText] = useState("");
  const [busy, setBusy] = useState<"case" | "later" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => {
    first.current?.focus();
    // Esc cierra la nota sin guardar.
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && void noteApi.close();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const empty = !person.trim() && !machine.trim() && !text.trim();

  const asCase = async () => {
    setBusy("case");
    setError(null);
    try {
      await casesApi.open({ person, machine, notes: text });
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
      const quien = person.trim() ? `${person.trim()}: ` : "";
      await followupsApi.add({ text: `${quien}${text.trim() || "Llamada pendiente"}`, due: tomorrowMorning(), person, machine });
      await emit("followups-changed");
      await noteApi.close();
    } catch (e) {
      setError(String(e));
      setBusy(null);
    }
  };

  return (
    <div className="flex h-full flex-col gap-3 bg-void p-4 text-ink">
      <div className="flex items-center gap-2 text-sm font-semibold">
        <PhoneCall size={15} className="text-neon" /> Nota de llamada
      </div>
      <label className="block">
        <span className="mb-1 block text-xs text-dim">Quién llama</span>
        <input ref={first} value={person} onChange={(e) => setPerson(e.target.value)} placeholder="María Pérez · ext. 4512" className={input} />
      </label>
      <label className="block">
        <span className="mb-1 block text-xs text-dim">Equipo</span>
        <input value={machine} onChange={(e) => setMachine(e.target.value)} placeholder="PC-CONTA-03" className={`${input} font-mono`} />
      </label>
      <label className="block flex-1">
        <span className="mb-1 block text-xs text-dim">Qué le pasa</span>
        <textarea value={text} onChange={(e) => setText(e.target.value)} placeholder="No imprime desde esta mañana…" className={`${input} h-28 resize-none`} />
      </label>
      {error && <p className="text-xs text-bad">{error}</p>}
      <div className="flex gap-2">
        <button
          onClick={() => void asCase()}
          disabled={empty || busy !== null}
          className="flex h-9 flex-1 items-center justify-center gap-1.5 rounded-lg bg-neon px-3 text-[13px] font-medium text-on-neon hover:brightness-110 disabled:opacity-40"
        >
          {busy === "case" ? <Loader2 size={14} className="animate-spin" /> : <TicketPlus size={14} />} Abrir caso
        </button>
        <button
          onClick={() => void later()}
          disabled={empty || busy !== null}
          className="flex h-9 flex-1 items-center justify-center gap-1.5 rounded-lg border border-line-2 px-3 text-[13px] text-ink hover:bg-panel-2 disabled:opacity-40"
          title="Aparecerá mañana a las 9 en «Hoy», con un aviso"
        >
          {busy === "later" ? <Loader2 size={14} className="animate-spin" /> : <CalendarClock size={14} />} Para mañana
        </button>
      </div>
      <p className="text-center text-[11px] text-mute">Ctrl+Alt+N la abre desde cualquier sitio · Esc la cierra</p>
    </div>
  );
}
