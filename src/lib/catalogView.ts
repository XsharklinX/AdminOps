// Qué programas del catálogo se enseñan. En una empresa no se instalan juegos
// ni mensajería personal: la vista «Empresa» los deja fuera, y el técnico puede
// ocultar además programas sueltos o categorías enteras.
import type { CatalogApp, CatalogView } from "./api";

/** ¿Se enseña este programa con lo que el técnico tiene elegido? */
export function isVisible(a: Pick<CatalogApp, "id" | "category" | "home">, view: CatalogView): boolean {
  return !(view.business && a.home) && !view.hiddenApps.includes(a.id) && !view.hiddenCategories.includes(a.category);
}

/** Cuántos se ven y cuántos no (y, de estos, cuántos solo por ser de uso personal). */
export function catalogCounts(apps: CatalogApp[], view: CatalogView): { visible: number; hidden: number; home: number } {
  const visible = apps.filter((a) => isVisible(a, view)).length;
  const home = apps.filter((a) => view.business && a.home && !view.hiddenApps.includes(a.id) && !view.hiddenCategories.includes(a.category)).length;
  return { visible, hidden: apps.length - visible, home };
}
