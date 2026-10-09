// Modo privacidad: tapa en pantalla lo que no debe verse al compartirla (correos,
// IP, MAC, rutas de red, carpetas de usuario, claves de recuperación) y los
// campos marcados con `data-sensitive`. No cambia ningún dato: solo cómo se pinta.
//
// El texto no se toca (React lo gestiona): se usa la API de resaltados de CSS
// (`CSS.highlights`), que pinta rangos de texto sin modificar el DOM.
import { useSyncExternalStore } from "react";

/** Lo que se tapa. Cada patrón es global; el orden no importa. */
const PATTERNS: RegExp[] = [
  /[\w.+-]+@[\w-]+(?:\.[\w-]+)+/g, // correo
  /\b(?:\d{1,3}\.){3}\d{1,3}\b/g, // IPv4
  /\b(?:[0-9a-f]{1,4}:){3,7}[0-9a-f]{1,4}\b/gi, // IPv6
  /\b(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}\b/gi, // MAC
  /\\\\[\w.$-]+(?:\\[^\s\\]+)*/g, // ruta de red \\servidor\recurso
  /\b[A-Za-z]:\\Users\\[^\\\s]+/g, // carpeta de un usuario
  /\b(?:\d{6}-){7}\d{6}\b/g, // clave de recuperación de BitLocker
];

/** Rangos [inicio, fin) del texto que hay que tapar, sin solapes y ordenados. */
export function findSensitive(text: string): [number, number][] {
  const hits: [number, number][] = [];
  for (const re of PATTERNS) {
    re.lastIndex = 0;
    for (let m = re.exec(text); m; m = re.exec(text)) {
      if (m[0].length === 0) {
        re.lastIndex++;
        continue;
      }
      hits.push([m.index, m.index + m[0].length]);
    }
  }
  hits.sort((a, b) => a[0] - b[0] || b[1] - a[1]);
  const out: [number, number][] = [];
  for (const h of hits) {
    const last = out[out.length - 1];
    if (last && h[0] <= last[1]) last[1] = Math.max(last[1], h[1]);
    else out.push([h[0], h[1]]);
  }
  return out;
}

let enabled = false;
const subscribers = new Set<() => void>();
let observer: MutationObserver | null = null;
let timer: number | null = null;

function paint() {
  timer = null;
  const reg = (CSS as unknown as { highlights?: Map<string, unknown> }).highlights;
  const HighlightCtor = (window as unknown as { Highlight?: new (...r: Range[]) => unknown }).Highlight;
  if (!reg || !HighlightCtor) return;
  const ranges: Range[] = [];
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
    acceptNode: (n) => {
      const p = n.parentElement;
      return p && !["SCRIPT", "STYLE", "TEXTAREA"].includes(p.tagName) && n.nodeValue && n.nodeValue.length > 5 ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
    },
  });
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    for (const [a, b] of findSensitive(n.nodeValue ?? "")) {
      const r = new Range();
      r.setStart(n, a);
      r.setEnd(n, b);
      ranges.push(r);
    }
  }
  reg.set("adminops-private", new HighlightCtor(...ranges));
  // Campos de texto: su contenido no es texto del DOM, así que se marca el campo entero.
  document.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("input:not([type=checkbox]):not([type=radio]):not([type=range]), textarea").forEach((el) => {
    el.toggleAttribute("data-priv-hit", findSensitive(el.value).length > 0);
  });
}

function onInput() {
  schedule();
}

function schedule() {
  if (timer === null) timer = window.setTimeout(paint, 120);
}

function apply() {
  document.documentElement.toggleAttribute("data-privacy", enabled);
  if (enabled) {
    observer ??= new MutationObserver(schedule);
    observer.observe(document.body, { childList: true, characterData: true, subtree: true });
    document.addEventListener("input", onInput, true);
    schedule();
  } else {
    observer?.disconnect();
    document.removeEventListener("input", onInput, true);
    document.querySelectorAll("[data-priv-hit]").forEach((el) => el.removeAttribute("data-priv-hit"));
    if (timer !== null) window.clearTimeout(timer);
    timer = null;
    (CSS as unknown as { highlights?: Map<string, unknown> }).highlights?.delete("adminops-private");
  }
}

export function setPrivacy(on: boolean) {
  if (on === enabled) return;
  enabled = on;
  apply();
  subscribers.forEach((f) => f());
}

export function usePrivacy(): boolean {
  return useSyncExternalStore(
    (f) => {
      subscribers.add(f);
      return () => subscribers.delete(f);
    },
    () => enabled,
  );
}
