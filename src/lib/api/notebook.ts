// «Notas»: el cuaderno (notebook.rs).
import { invoke } from "./core";

export interface Note {
  id: string;
  title: string;
  text: string;
  created: number;
  updated: number;
  pinned: boolean;
}

export const NOTES_CHANGED_EVENT = "notes-changed";

export const notesApi = {
  list: () => invoke<Note[]>("notes_list"),
  save: (note: Note) => invoke<Note>("notes_save", { note }),
  remove: (id: string) => invoke<void>("notes_delete", { id }),
};
