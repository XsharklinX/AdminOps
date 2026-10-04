// Lo que la pantalla de Espacio calcula sin preguntar al equipo: nombres y
// colores de los tipos de archivo, el orden de las listas y las migas.
import type { KindSize, SpaceEntry, SpaceFolder } from "./api";

/** Tipos de archivo, con el nombre que ve el técnico. Las claves las da el análisis. */
export const KINDS: Record<string, { label: string; color: string }> = {
  video: { label: "Vídeo", color: "#a78bfa" },
  image: { label: "Imágenes", color: "#34d399" },
  audio: { label: "Música y sonido", color: "#f472b6" },
  doc: { label: "Documentos", color: "#60a5fa" },
  mail: { label: "Correo guardado", color: "#fbbf24" },
  archive: { label: "Comprimidos", color: "#fb923c" },
  disk: { label: "Instaladores e imágenes de disco", color: "#f87171" },
  program: { label: "Programas y sistema", color: "#94a3b8" },
  other: { label: "Otros", color: "#64748b" },
};

export const kindLabel = (id: string) => KINDS[id]?.label ?? id;
export const kindColor = (id: string) => KINDS[id]?.color ?? "#64748b";

/** Los tipos de mayor a menor, cada uno con su parte del total. */
export function kindShares(kinds: KindSize[]): (KindSize & { share: number })[] {
  const total = kinds.reduce((a, k) => a + k.size, 0);
  if (!total) return [];
  return [...kinds].sort((a, b) => b.size - a.size).map((k) => ({ ...k, share: k.size / total }));
}

/** Clave del rectángulo que agrupa lo que no es una subcarpeta con nombre. */
export const LOOSE = "\u0000sueltos";

/**
 * Lo que hay en una carpeta que no sale como subcarpeta: sus archivos sueltos y
 * las carpetas de menos de 1 MB. Sin esto, el mapa de una carpeta llena de
 * archivos (Descargas) salía casi vacío.
 */
export function looseSize(folder: SpaceFolder): number {
  return Math.max(0, folder.size - folder.children.reduce((a, c) => a + c.size, 0));
}

export type FolderSort = "size" | "name" | "old" | "files";

export function sortEntries(entries: SpaceEntry[], by: FolderSort): SpaceEntry[] {
  const list = [...entries];
  switch (by) {
    case "name":
      return list.sort((a, b) => a.name.localeCompare(b.name, "es", { sensitivity: "base" }));
    case "files":
      return list.sort((a, b) => b.files - a.files);
    case "old":
      // Lo que lleva más tiempo sin tocarse, primero; lo que no tiene fecha, al final.
      return list.sort((a, b) => (a.modified ?? Infinity) - (b.modified ?? Infinity));
    default:
      return list.sort((a, b) => b.size - a.size);
  }
}

/** Filtra por texto, sin distinguir mayúsculas ni tildes. */
export function matchesText(name: string, query: string): boolean {
  const plain = (s: string) =>
    s
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLowerCase();
  const q = plain(query.trim());
  return !q || plain(name).includes(q);
}

/** Las migas desde la raíz del análisis hasta `path`. */
export function crumbsOf(root: string, path: string): { name: string; path: string }[] {
  const out = [{ name: root, path: root }];
  if (!path.toLowerCase().startsWith(root.toLowerCase())) return out;
  let at = root.replace(/\\+$/, "");
  for (const part of path.slice(root.length).split("\\").filter(Boolean)) {
    at = `${at}\\${part}`;
    out.push({ name: part, path: at });
  }
  return out;
}

/** La carpeta que contiene a `path` (para «Subir» y para enseñar dónde está un archivo). */
export function parentOf(path: string): string {
  const clean = path.replace(/\\+$/, "");
  const i = clean.lastIndexOf("\\");
  if (i < 0) return clean;
  // «C:\algo» sube a «C:\», no a «C:».
  return i <= 2 ? clean.slice(0, i + 1) : clean.slice(0, i);
}

/** Tanto por ciento legible: «<1 %» en vez de «0 %» para lo que existe pero es poco. */
export function percent(part: number, total: number): string {
  if (!total || !part) return "0 %";
  const p = (part / total) * 100;
  return p < 1 ? "<1 %" : `${Math.round(p)} %`;
}
