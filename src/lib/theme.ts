/** Tema de la interfaz (oscuro o claro). Se guarda por equipo en el almacenamiento del webview. */
export type Theme = "dark" | "light";

/**
 * Lo que eligió el técnico: un tema fijo, el de Windows («auto») o por horario
 * (claro de día y oscuro desde la tarde).
 */
export type ThemeMode = Theme | "auto" | "schedule";

const KEY = "adminops.theme";
const SCHEDULE_KEY = "adminops.themeSchedule";

/** Horas del horario: desde cuándo es claro y desde cuándo oscuro. */
export interface ThemeSchedule {
  lightFrom: number;
  darkFrom: number;
}

const DEFAULT_SCHEDULE: ThemeSchedule = { lightFrom: 8, darkFrom: 19 };

export function getThemeMode(): ThemeMode {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "auto" || v === "schedule" ? v : "dark";
  } catch {
    return "dark";
  }
}

export function getSchedule(): ThemeSchedule {
  try {
    const raw = JSON.parse(localStorage.getItem(SCHEDULE_KEY) ?? "null");
    if (raw && Number.isInteger(raw.lightFrom) && Number.isInteger(raw.darkFrom)) return raw;
  } catch {
    /* sin almacenamiento o dañado */
  }
  return DEFAULT_SCHEDULE;
}

export function setSchedule(s: ThemeSchedule) {
  try {
    localStorage.setItem(SCHEDULE_KEY, JSON.stringify(s));
  } catch {
    /* dura esta sesión */
  }
  syncTheme();
}

/** El tema que toca a esta hora con ese horario. */
export function scheduledTheme(hour: number, s: ThemeSchedule): Theme {
  const { lightFrom, darkFrom } = s;
  if (lightFrom === darkFrom) return "dark";
  const light = lightFrom < darkFrom ? hour >= lightFrom && hour < darkFrom : hour >= lightFrom || hour < darkFrom;
  return light ? "light" : "dark";
}

function systemTheme(): Theme {
  try {
    return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  } catch {
    return "dark";
  }
}

/** El tema que se ve ahora mismo. */
export function getTheme(): Theme {
  const m = getThemeMode();
  if (m === "auto") return systemTheme();
  if (m === "schedule") return scheduledTheme(new Date().getHours(), getSchedule());
  return m;
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}

/** Cambia el tema con un fundido (si WebView2 lo permite y no se han reducido las animaciones). */
function fadeTo(theme: Theme, after?: () => void) {
  if (document.documentElement.dataset.theme === theme) return;
  const doc = document as Document & { startViewTransition?: (f: () => void) => unknown };
  const calm = document.documentElement.classList.contains("reduce-motion");
  const run = () => {
    applyTheme(theme);
    after?.();
  };
  if (doc.startViewTransition && !calm) doc.startViewTransition(run);
  else run();
}

let onChange: (() => void) | null = null;

/** Quién recalcula lo que depende del tema (el acento): lo pone prefs. */
export function onThemeApplied(f: () => void) {
  onChange = f;
}

/** Vuelve a mirar qué tema toca (cambió Windows, pasó la hora del horario…). */
export function syncTheme() {
  fadeTo(getTheme(), () => onChange?.());
}

export function setThemeMode(mode: ThemeMode) {
  try {
    localStorage.setItem(KEY, mode);
  } catch {
    /* sin almacenamiento: dura esta sesión */
  }
  syncTheme();
}

/** Compatibilidad: fijar un tema concreto. */
export function setTheme(theme: Theme) {
  setThemeMode(theme);
}

let watching = false;

/** Sigue a Windows y al reloj mientras AdminOps está abierta. */
export function watchTheme() {
  if (watching) return;
  watching = true;
  try {
    window.matchMedia("(prefers-color-scheme: light)").addEventListener("change", () => getThemeMode() === "auto" && syncTheme());
  } catch {
    /* sin matchMedia */
  }
  window.setInterval(() => getThemeMode() === "schedule" && syncTheme(), 60_000);
}
