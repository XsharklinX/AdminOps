// Texto cortado que se puede leer entero.
//
// En las tablas y listas hay mucho texto recortado con «…» (nombres de
// programas, rutas, asuntos de correo, nombres de equipos). Hasta ahora, si no
// cabía, no había forma de leerlo: ni pasando el ratón por encima.
//
// En vez de añadir a mano el `title` en los casi cien sitios donde pasa —y
// olvidarlo en los siguientes—, se resuelve una vez aquí: al pasar el ratón por
// un texto que de verdad está cortado, se le pone su contenido completo como
// `title` y Windows enseña el globo de siempre. Lo que no está cortado no se
// toca, así que no aparecen globos donde no hacen falta.

/** Marca los `title` que ponemos nosotros, para distinguirlos de los del código. */
const CHECKED = "adminopsFullText";

/** ¿Está este elemento recortando su texto (con «…» o por líneas)? */
function isClipped(el: HTMLElement): boolean {
  // Primero lo barato: si cabe, no hay nada que hacer.
  const overflowsX = el.scrollWidth > el.clientWidth + 1;
  const overflowsY = el.scrollHeight > el.clientHeight + 1;
  if (!overflowsX && !overflowsY) return false;
  // Y solo si además lo está recortando, no si es una caja con su propio scroll.
  const s = getComputedStyle(el);
  if (overflowsX && s.textOverflow === "ellipsis" && s.overflowX !== "auto" && s.overflowX !== "scroll") return true;
  return overflowsY && s.webkitLineClamp !== "none";
}

function onPointerOver(e: Event) {
  const el = e.target;
  if (!(el instanceof HTMLElement)) return;
  const nuestro = el.dataset[CHECKED] === "1";
  // Un `title` puesto a mano explica algo que el texto no dice: no se pisa.
  if (el.title && !nuestro) return;
  // Se vuelve a medir en cada pasada: la misma fila puede traer otro texto
  // después de recargar, o dejar de estar cortada al agrandar la ventana.
  const text = isClipped(el) ? el.textContent?.trim() : "";
  if (text) {
    el.title = text;
    el.dataset[CHECKED] = "1";
  } else if (nuestro) {
    el.removeAttribute("title");
    delete el.dataset[CHECKED];
  }
}

/** Se engancha una vez al abrir la aplicación. */
export function showFullTextOnHover() {
  document.addEventListener("pointerover", onPointerOver, { passive: true, capture: true });
}
