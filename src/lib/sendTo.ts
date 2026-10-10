// «Enviar a…»: lo que se ve en una tarjeta, convertido en lo que necesita cada
// destino (texto para un chat, tabla para Excel). Sin datos de la propia app
// (botones, menús): solo el contenido.

/** Texto de un elemento, sin lo marcado para no copiarse (botones de la cabecera). */
export function textOf(el: HTMLElement, skipAttr: string): string {
  const clone = el.cloneNode(true) as HTMLElement;
  clone.querySelectorAll(`[${skipAttr}], button, input, select, svg`).forEach((n) => n.remove());
  return (clone.innerText || clone.textContent || "")
    .split("\n")
    .map((l) => l.replace(/\s+/g, " ").trim())
    .filter((l, i, all) => l || (all[i - 1] ?? "") !== "")
    .join("\n")
    .trim();
}

/** Una tabla HTML como texto separado por tabuladores (se pega en Excel en celdas). */
export function tableToTsv(rows: string[][]): string {
  return rows.map((r) => r.map((c) => c.replace(/[\t\n\r]+/g, " ").trim()).join("\t")).join("\n");
}

/** Las celdas de la primera tabla de un elemento. */
export function tableRows(el: HTMLElement): string[][] | null {
  const t = el.querySelector("table");
  if (!t) return null;
  return Array.from(t.querySelectorAll("tr"))
    .map((tr) => Array.from(tr.querySelectorAll("th,td")).map((c) => (c as HTMLElement).innerText ?? c.textContent ?? ""))
    .filter((r) => r.some((c) => c.trim()));
}

/** Añade un bloque a las notas del caso, con su título. */
export function appendNote(notes: string, title: string, text: string): string {
  const block = `${title}\n${text}`;
  return notes.trim() ? `${notes.trimEnd()}\n\n${block}` : block;
}
