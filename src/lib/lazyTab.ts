// Cada pestaña de una página con pestañas (pages/Merged.tsx) carga su código
// solo cuando se abre, en vez de venir todas juntas en un solo trozo.
import { lazy } from "react";

/** Cualquier componente de React (el tipo que `lazy` acepta, con sus props propias). */
type AnyComponent = Parameters<typeof lazy>[0] extends () => Promise<{ default: infer T }> ? T : never;

/** Una pantalla exportada por nombre desde su módulo, cargada al usarla. */
export function lazyTab<K extends string, M extends Record<K, AnyComponent>>(load: () => Promise<M>, name: K) {
  return lazy<M[K]>(() => load().then((m) => ({ default: m[name] })));
}
