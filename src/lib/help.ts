// La ventana de ayuda (guía, glosario, novedades, términos y reportar un
// problema) se puede abrir desde cualquier sitio: Acerca de, Ajustes o Ctrl+K.
import { useSyncExternalStore } from "react";
import { FAQ, GLOSSARY, GUIDE, type FaqEntry, type GlossaryEntry, type GuideChapter, type GuideTopic } from "./guide";

export type HelpTab = "guide" | "glossary" | "faq" | "news" | "terms" | "report";

let current: HelpTab | null = null;
let subs: (() => void)[] = [];

export function openHelp(tab: HelpTab = "guide") {
  current = tab;
  subs.forEach((s) => s());
}

export function closeHelp() {
  current = null;
  subs.forEach((s) => s());
}

const subscribe = (cb: () => void) => {
  subs.push(cb);
  return () => {
    subs = subs.filter((s) => s !== cb);
  };
};

/** La pestaña abierta, o null si la ayuda está cerrada. */
export const useHelp = () => useSyncExternalStore(subscribe, () => current);

/** A dónde llegan los avisos de fallos (el mismo que usa el programa al preparar el correo). */
export const SUPPORT_EMAIL = "Contactoyerlindavid@gmail.com";

/** Sin tildes ni mayúsculas, para que «diagnostico» encuentre «Diagnóstico». */
const fold = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();

export interface GuideHit {
  chapter: GuideChapter;
  topic: GuideTopic;
}

export interface HelpSearch {
  topics: GuideHit[];
  glossary: GlossaryEntry[];
  faq: FaqEntry[];
}

/** Busca en toda la ayuda. Tienen que estar todas las palabras; lo que coincide en el título va primero. */
export function searchHelp(query: string): HelpSearch {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (!words.length) return { topics: [], glossary: [], faq: [] };
  const has = (text: string) => {
    const t = fold(text);
    return words.every((w) => t.includes(w));
  };
  const topics: (GuideHit & { rank: number })[] = [];
  for (const chapter of GUIDE)
    for (const topic of chapter.topics) {
      if (has(topic.title)) topics.push({ chapter, topic, rank: 0 });
      else if (has([topic.title, topic.what, ...(topic.how ?? []), ...(topic.notes ?? [])].join(" "))) topics.push({ chapter, topic, rank: 1 });
    }
  topics.sort((a, b) => a.rank - b.rank);
  return {
    topics,
    glossary: GLOSSARY.filter((g) => has(`${g.term} ${g.def}`)),
    faq: FAQ.filter((f) => has(`${f.q} ${f.a}`)),
  };
}

export interface TermsBlock {
  heading: boolean;
  text: string;
}

/** Los términos (texto plano) en bloques: los títulos son las líneas «N. Título» y la primera. */
export function parseTerms(raw: string): TermsBlock[] {
  return raw
    .replace(/^\uFEFF/, "")
    .split(/\r?\n\s*\r?\n/)
    .map((p) => p.trim())
    .filter(Boolean)
    .map((text, i) => ({ heading: i === 0 || (/^\d+\.\s/.test(text) && text.length < 80 && !text.includes("\n")), text }));
}
