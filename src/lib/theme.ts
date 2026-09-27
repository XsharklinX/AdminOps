/** Tema de la interfaz (oscuro o claro). Se guarda por equipo en el almacenamiento del webview. */
export type Theme = "dark" | "light";

const KEY = "adminops.theme";

export function getTheme(): Theme {
  try {
    return localStorage.getItem(KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}

export function setTheme(theme: Theme) {
  applyTheme(theme);
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    /* sin almacenamiento: dura esta sesión */
  }
}
