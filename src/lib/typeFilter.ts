// «Escribe y la lista se filtra»: con una tecla suelta, sin tener el foco en un
// campo, el texto va a la lista de la pantalla (la tabla común) o a su caja de
// búsqueda. Una tabla o un campo se marcan con `data-typefilter`.
import { isValidElement, useEffect, useRef, type ReactNode } from "react";

export const TYPEFILTER_EVENT = "adminops:typefilter";

/** Lo que se manda a una tabla: un carácter o Backspace, Escape o Enter. */
export interface TypeFilterDetail {
  key: string;
}

/** Minúsculas y sin tildes: «Configuración» coincide con «configuracion». */
export function normalizeText(s: string): string {
  return s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase();
}

/** ¿Contiene el texto todas las palabras de lo escrito? (sin mayúsculas ni tildes) */
export function matchesFilter(haystack: string, query: string): boolean {
  const words = normalizeText(query).split(/\s+/).filter(Boolean);
  if (!words.length) return true;
  const h = normalizeText(haystack);
  return words.every((w) => h.includes(w));
}

/** El texto que se ve en una celda (cadenas, números y lo que lleven dentro los elementos). */
export function nodeText(n: ReactNode): string {
  if (n === null || n === undefined || typeof n === "boolean") return "";
  if (typeof n === "string" || typeof n === "number") return String(n);
  if (Array.isArray(n)) {
    return n
      .map((x) => nodeText(x).trim())
      .filter(Boolean)
      .join(" ");
  }
  if (isValidElement(n)) return nodeText((n.props as { children?: ReactNode }).children);
  return "";
}

/** ¿Es un sitio donde se escribe? */
export function isTypingPlace(el: Element | null): boolean {
  if (!el) return false;
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) return true;
  if ((el as HTMLElement).isContentEditable) return true;
  return !!el.closest('[contenteditable=""],[contenteditable="true"],[role="textbox"],[role="combobox"],[role="searchbox"]');
}

/** Teclas sueltas que alguna pantalla usa como atajo («/» lleva a la búsqueda de Contactos): no escriben en la lista. */
export const SHORTCUT_KEYS: readonly string[] = ["/"];

/** ¿Sirve esta tecla para escribir en una lista? Un carácter suelto, sin Ctrl, Alt ni Meta (Mayús vale), que no sea un atajo. */
export function isFilterKey(ev: Pick<KeyboardEvent, "key" | "ctrlKey" | "altKey" | "metaKey" | "isComposing">): boolean {
  return !ev.ctrlKey && !ev.altKey && !ev.metaKey && !ev.isComposing && ev.key.length === 1 && !SHORTCUT_KEYS.includes(ev.key);
}

function shown(el: HTMLElement): boolean {
  if (el.closest("[hidden]") || el.getClientRects().length === 0) return false;
  const r = el.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && r.bottom > 0 && r.top < window.innerHeight;
}

/** La lista o campo que recibe lo escrito: el que está bajo el puntero y, si no, el primero a la vista. */
export function pickTarget(pointer: { x: number; y: number } | null, onlyActive = false): HTMLElement | null {
  const all = [...document.querySelectorAll<HTMLElement>("[data-typefilter]")].filter((el) => shown(el) && (!onlyActive || el.dataset.tfActive === "1"));
  if (pointer) {
    const under = all.find((el) => {
      const r = el.getBoundingClientRect();
      return pointer.x >= r.left && pointer.x <= r.right && pointer.y >= r.top && pointer.y <= r.bottom;
    });
    if (under) return under;
  }
  return all[0] ?? null;
}

/** Escribe un carácter en un campo de texto de React (sin pasar por el teclado). */
function typeInto(input: HTMLInputElement, ch: string) {
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  if (!set) return;
  input.focus();
  set.call(input, input.value + ch);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

/**
 * El manejador global. `blocked()` dice si ahora no toca (bloqueo, diálogo propio
 * de la app…). Los usos que ya tiene una tecla ganan: J y K en una fila de tabla,
 * y el espacio sobre un botón o una fila.
 */
export function useTypeToFilter(blocked: () => boolean) {
  const pointer = useRef<{ x: number; y: number } | null>(null);
  const blockedRef = useRef(blocked);
  blockedRef.current = blocked;
  useEffect(() => {
    const move = (ev: PointerEvent) => (pointer.current = { x: ev.clientX, y: ev.clientY });
    const onKey = (ev: KeyboardEvent) => {
      if (ev.defaultPrevented || blockedRef.current()) return;
      if (document.body.dataset.recordingShortcut) return;
      if (ev.ctrlKey || ev.altKey || ev.metaKey || ev.isComposing) return;
      const special = ev.key === "Backspace" || ev.key === "Escape" || ev.key === "Enter";
      if (!special && !isFilterKey(ev)) return;
      const active = document.activeElement;
      if (isTypingPlace(ev.target as Element | null) || isTypingPlace(active)) return;
      // Un diálogo, la paleta o un menú abierto: la tecla es suya.
      if (document.querySelector('[role="dialog"],[aria-modal="true"],[role="menu"],[role="listbox"]')) return;
      const interactive = !!active?.closest('button,a[href],summary,[role="button"],[role="tab"],[role="menuitem"],[role="checkbox"],[role="switch"],[role="radio"],tr[tabindex]');
      if (special) {
        // Solo si hay un filtro escrito, y sin quitarle su uso a Enter en un botón o fila.
        if (ev.key === "Enter" && interactive) return;
        const t = pickTarget(pointer.current, true);
        if (!t) return;
        ev.preventDefault();
        t.dispatchEvent(new CustomEvent<TypeFilterDetail>(TYPEFILTER_EVENT, { detail: { key: ev.key } }));
        return;
      }
      if (ev.key === " " && (interactive || !pickTarget(pointer.current, true))) return;
      // En una fila de tabla, J y K mueven entre filas.
      if ((ev.key === "j" || ev.key === "k") && active?.closest("tr[tabindex]")) return;
      const t = pickTarget(pointer.current);
      if (!t) return;
      ev.preventDefault();
      if (t instanceof HTMLInputElement) typeInto(t, ev.key);
      else t.dispatchEvent(new CustomEvent<TypeFilterDetail>(TYPEFILTER_EVENT, { detail: { key: ev.key } }));
    };
    window.addEventListener("pointermove", move, { passive: true });
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("keydown", onKey);
    };
  }, []);
}
