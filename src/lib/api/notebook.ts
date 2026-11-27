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
