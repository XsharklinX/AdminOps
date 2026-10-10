// Copiar con un clic (1.2.9): lo que lleve `data-copy` se copia al pulsarlo,
// destella y dice «Copiado» un instante. `data-copy="valor"` copia ese valor;
// vacío, el texto del elemento. Un solo oyente para toda la ventana.

let on = false;

export function watchCopy() {
  if (on || typeof document === "undefined") return;
  on = true;
  document.addEventListener("click", (e) => {
    const el = (e.target as Element | null)?.closest<HTMLElement>("[data-copy]");
    if (!el) return;
    // Si la persona está seleccionando texto con el ratón, no se le copia otra cosa.
    if (window.getSelection()?.toString()) return;
    const text = (el.dataset.copy || el.textContent || "").trim();
    if (!text) return;
    navigator.clipboard?.writeText(text).then(
      () => {
        el.classList.remove("copied");
        void el.offsetWidth;
        el.classList.add("copied");
        window.setTimeout(() => el.classList.remove("copied"), 1100);
      },
      () => {
        /* sin permiso del portapapeles: no se promete nada */
      },
    );
  });
}
