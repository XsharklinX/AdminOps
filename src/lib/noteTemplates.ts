// Plantillas de notas: tres de fábrica y las que guarda el técnico (en un archivo de AdminOps, igual en modo técnico y usuario).
import { loadUiPref, saveUiPref } from "./api/uipref";

export interface NoteTemplate {
  id: string;
  title: string;
  text: string;
  /** Las de fábrica no se borran. */
  builtin?: boolean;
}

const KEY = "adminops.noteTemplates";

const checks = (items: string[]) => items.map((i) => `[ ] ${i}`).join("\n");

export const BUILTIN_TEMPLATES: NoteTemplate[] = [
  { id: "b-equipo", title: "Equipo nuevo", builtin: true, text: checks(["Unir al dominio", "Office y Teams", "Impresora", "Copia de datos del anterior", "Etiqueta de inventario"]) },
  { id: "b-alta", title: "Alta de usuario", builtin: true, text: checks(["Cuenta y contraseña", "Correo", "Carpetas de red", "Impresora", "Teams"]) },
  { id: "b-impresora", title: "Cambio de impresora", builtin: true, text: checks(["Driver", "Puerto o IP", "Predeterminada", "Página de prueba"]) },
];

const isList = (v: unknown): v is unknown[] => Array.isArray(v);

/** Las guardadas por el técnico; si lo guardado no vale, una lista vacía. */
export async function loadUserTemplates(): Promise<NoteTemplate[]> {
  const raw = await loadUiPref("noteTemplates", KEY, isList);
  if (!raw) return [];
  return raw
    .filter((t): t is NoteTemplate => !!t && typeof t === "object" && typeof (t as NoteTemplate).id === "string" && typeof (t as NoteTemplate).title === "string" && typeof (t as NoteTemplate).text === "string")
    .map((t) => ({ id: t.id, title: t.title, text: t.text }));
}

/** Guarda título y texto como plantilla nueva (si ya hay una con ese título, la sustituye). */
export async function saveUserTemplate(title: string, text: string): Promise<NoteTemplate[]> {
  const name = title.trim() || text.split("\n")[0]?.trim().slice(0, 60) || "Plantilla";
  const list = (await loadUserTemplates()).filter((t) => t.title !== name);
  list.push({ id: `u${Date.now()}`, title: name, text });
  await saveUiPref("noteTemplates", list);
  return list;
}

export async function deleteUserTemplate(id: string): Promise<NoteTemplate[]> {
  const list = (await loadUserTemplates()).filter((t) => t.id !== id);
  await saveUiPref("noteTemplates", list);
  return list;
}
