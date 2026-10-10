// Adelantarse al clic: al pasar el ratón por una pantalla de la barra lateral se
// empieza a cargar su código, para que al pulsar ya esté listo.
import type { PageId } from "../components/Sidebar";

let loader: ((p: PageId) => void) | null = null;
const done = new Set<PageId>();

/** La aplicación dice cómo se carga cada pantalla. */
export function setPrefetcher(f: (p: PageId) => void) {
  loader = f;
}

export function prefetchPage(p: PageId) {
  if (done.has(p) || !loader) return;
  done.add(p);
  loader(p);
}
