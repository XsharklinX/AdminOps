// Ir a otra página desde un componente que no recibe la navegación (diálogos
// compartidos): App escucha este evento.
export const NAVIGATE_EVENT = "adminops:navigate";

export function goToPage(page: string, focus: string | null = null) {
  window.dispatchEvent(new CustomEvent(NAVIGATE_EVENT, { detail: { page, focus } }));
}
