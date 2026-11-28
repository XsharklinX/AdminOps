// «Notas»: el cuaderno (notebook.rs).
import { invoke } from "./core";

export interface Note {
  id: string;
  title: string;
  text: string;
  created: number;
  updated: number;
  pinned: boolean;
  /** Imágenes pegadas (capturas), como data: URL. */
  images: string[];
  /** Hora (ms desde 1970) a la que AdminOps avisa de la nota; vacía si no hay aviso. */
  remindAt?: number | null;
}

const atHour = (d: Date, h: number) => {
  const x = new Date(d);
  x.setHours(h, 0, 0, 0);
  return x;
};

/** Los avisos que se ofrecen en el editor, a partir de `now`. */
export function remindOptions(now: Date = new Date()): { label: string; at: number }[] {
  const today15 = atHour(now, 15);
  const tomorrow = atHour(new Date(now.getTime() + 86_400_000), 9);
  const monday = atHour(now, 9);
  monday.setDate(monday.getDate() + (((8 - monday.getDay()) % 7) || 7));
  return [
    today15 > now ? { label: "Hoy 15:00", at: today15.getTime() } : { label: "En 1 hora", at: now.getTime() + 3_600_000 },
    { label: "Mañana 9:00", at: tomorrow.getTime() },
    { label: "El lunes 9:00", at: monday.getTime() },
  ];
}

/** «hoy 15:00», «mañana 09:00» o «lun 3 nov 09:00». */
export function fmtRemind(ms: number, now: Date = new Date()): string {
  const d = new Date(ms);
  const hm = d.toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(d) - day(now)) / 86_400_000);
  if (diff === 0) return `hoy ${hm}`;
  if (diff === 1) return `mañana ${hm}`;
  return `${d.toLocaleDateString("es", { weekday: "short", day: "numeric", month: "short" })} ${hm}`;
}

/** «[ ] algo» / «[x] algo»: una casilla de la nota. */
export const CHECK_RE = /^(\s*)\[( |x|X)\]\s?(.*)$/;

/** Cuántas casillas hay y cuántas marcadas. */
export function checkCount(text: string): { done: number; total: number } {
  let done = 0;
  let total = 0;
  for (const l of text.split("\n")) {
    const m = CHECK_RE.exec(l);
    if (!m) continue;
    total++;
    if (m[2] !== " ") done++;
  }
  return { done, total };
}

/** Marca o desmarca la casilla de la línea `line`. */
export function toggleCheck(text: string, line: number): string {
  const lines = text.split("\n");
  const m = CHECK_RE.exec(lines[line] ?? "");
  if (!m) return text;
  lines[line] = `${m[1]}[${m[2] === " " ? "x" : " "}] ${m[3]}`;
  return lines.join("\n");
}

export const NOTES_CHANGED_EVENT = "notes-changed";

export const notesApi = {
  list: () => invoke<Note[]>("notes_list"),
  /** Sin imágenes: para buscar en el texto (Ctrl+K). */
  index: () => invoke<Note[]>("notes_index"),
  save: (note: Note) => invoke<Note>("notes_save", { note }),
  remove: (id: string) => invoke<void>("notes_delete", { id }),
};
