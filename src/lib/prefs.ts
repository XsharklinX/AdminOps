/** Preferencias de la interfaz: apariencia, página de inicio, refresco del Panel y
 * estructura de la barra lateral. Se guardan por equipo en el almacenamiento del
 * webview (en modo portable, junto a AdminOps.exe) y se aplican antes de pintar. */
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { PageId } from "../components/Sidebar";
import { getTheme } from "./theme";

export type Accent = "blue" | "teal" | "violet" | "orange" | "green" | "graphite";

export interface NavArea {
  id: string;
  label: string;
  icon: string;
  pages: PageId[];
}

export interface NavLayout {
  areas: NavArea[];
  /** Páginas que no aparecen en la barra lateral (siguen en Ctrl+K). */
  hidden: PageId[];
}

/** Aspecto y comportamiento de la barra lateral. */
export interface SidebarPrefs {
  /** full: las áreas y, al lado, sus pantallas y secciones; mini: solo la columna de áreas
   *  (las pantallas del área van en pestañas bajo el título). */
  mode: "full" | "mini";
  position: "left" | "right";
  width: "narrow" | "normal" | "wide";
  /** Ancho exacto en píxeles si se ajustó arrastrando el borde; `null`: el de `width`. */
  widthPx: number | null;
  density: "compact" | "normal" | "comfortable";
  /** Al pulsar un área: su última pantalla usada o la primera. */
  areaClick: "last" | "first";
  /** El buscador en la barra de arriba (Ctrl+K y F1 funcionan igual). */
  showSearch: boolean;
  showSession: boolean;
  /** El estado al lado de cada sección («3» en Diagnóstico, «unido» en Dominio). */
  showBadges: boolean;
  /** Fijados arriba de la barra: pantallas («users») o secciones («users:domain»). */
  favorites: string[];
  /** Cuántas páginas recientes mostrar (0: sección oculta). */
  recents: number;
}

export const DEFAULT_SIDEBAR: SidebarPrefs = {
  mode: "full",
  position: "left",
  width: "normal",
  widthPx: null,
  density: "normal",
  areaClick: "last",
  showSearch: true,
  showSession: true,
  showBadges: true,
  favorites: [],
  recents: 0,
};

/**
 * Para quién es esta instalación.
 *  - `admin`: el técnico. Todo AdminOps, y para cambiar el equipo hace falta
 *    abrirlo como administrador.
 *  - `user`: la persona que usa el equipo. Solo ver cómo está y dejar entrar al
 *    técnico; no se muestran ni los datos del técnico ni lo que puede romper algo.
 *
 * Es un modo de la interfaz para evitar accidentes y no enseñar datos de otros
 * clientes, no una barrera de seguridad: quien tenga el equipo y sepa, puede
 * cambiarlo (por eso conviene poner un PIN en Ajustes → Seguridad).
 */
export type AppMode = "admin" | "user";

/** Cómo se abren Teams y el Correo: preguntar (la primera vez), dentro de AdminOps, en el navegador o en su aplicación. */
export type CommMode = "ask" | "adminops" | "browser" | "app";

export interface Prefs {
  mode: AppMode;
  accent: Accent;
  /** Tamaño de toda la interfaz (zoom de la ventana). */
  zoom: number;
  reduceMotion: boolean;
  /** "last": la última página visitada. */
  startPage: "last" | PageId;
  /** La página de inicio la eligió el técnico en Ajustes (y no es la de fábrica). */
  startPageChosen?: boolean;
  /** Cada cuánto se actualiza el Panel (ms). */
  refreshMs: number;
  layout: NavLayout | null;
  /** Atajos propios: combinación ("Ctrl+Alt+D") → página. */
  shortcuts: Record<string, PageId>;
  sidebar: SidebarPrefs;
  /** Nombres propios de las páginas. */
  pageLabels: Partial<Record<PageId, string>>;
  /** Lanzar el diagnóstico en segundo plano al abrir AdminOps. */
  diagnoseOnOpen: boolean;
  /** Cargar en segundo plano el último portal usado (Tickets, inventario, correo). */
  preloadPortals: boolean;
  /** Zoom con el que se abren los portales nuevos (1 = 100 %). */
  portalZoom: number;
  /** Cómo se abren Teams y el Correo desde la barra de arriba. */
  comms: { teams: CommMode; mail: CommMode };
}

const KEY = "adminops.prefs";
const DEFAULTS: Prefs = { mode: "admin", accent: "blue", zoom: 1, reduceMotion: false, startPage: "dashboard", refreshMs: 2000, layout: null, shortcuts: {}, sidebar: DEFAULT_SIDEBAR, pageLabels: {}, diagnoseOnOpen: false, preloadPortals: true, portalZoom: 1, comms: { teams: "ask", mail: "ask" } };

export const ACCENTS: Record<Accent, { label: string; dark: string; light: string }> = {
  blue: { label: "Azul", dark: "#5b8def", light: "#2459c9" },
  teal: { label: "Turquesa", dark: "#2bb3a3", light: "#0f7f72" },
  violet: { label: "Violeta", dark: "#8f7cf6", light: "#6546d8" },
  orange: { label: "Naranja", dark: "#e8914a", light: "#b85a14" },
  green: { label: "Verde", dark: "#4fb477", light: "#1f7f47" },
  graphite: { label: "Grafito", dark: "#a3abb7", light: "#434b57" },
};

export const ZOOMS = [
  { value: 0.9, label: "Pequeño" },
  { value: 1, label: "Normal" },
  { value: 1.1, label: "Grande" },
  { value: 1.25, label: "Muy grande" },
];

let cache: Prefs | null = null;

export function getPrefs(): Prefs {
  if (cache) return cache;
  let p: Prefs;
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    p = { ...DEFAULTS, ...(raw && typeof raw === "object" ? raw : {}) };
  } catch {
    p = { ...DEFAULTS };
  }
  if (!ACCENTS[p.accent]) p.accent = "blue";
  if (!ZOOMS.some((z) => z.value === p.zoom)) p.zoom = 1;
  if (![1000, 2000, 5000].includes(p.refreshMs)) p.refreshMs = 2000;
  if (!p.shortcuts || typeof p.shortcuts !== "object") p.shortcuts = {};
  p.sidebar = { ...DEFAULT_SIDEBAR, ...(p.sidebar && typeof p.sidebar === "object" ? p.sidebar : {}) };
  if (!Array.isArray(p.sidebar.favorites)) p.sidebar.favorites = [];
  p.sidebar.favorites = p.sidebar.favorites.filter((f): f is string => typeof f === "string");
  if (!p.pageLabels || typeof p.pageLabels !== "object") p.pageLabels = {};
  const modes = ["ask", "adminops", "browser", "app"];
  const c = (p.comms && typeof p.comms === "object" ? p.comms : {}) as Partial<Prefs["comms"]>;
  p.comms = { teams: modes.includes(c.teams as string) ? c.teams! : "ask", mail: modes.includes(c.mail as string) ? c.mail! : "ask" };
  // AdminOps abre siempre en el Panel salvo que el técnico haya elegido otra
  // página. Antes venía «la última que usé» de fábrica, y como las preferencias
  // se guardan enteras al tocar cualquier ajuste, ese valor quedaba escrito
  // aunque nadie lo hubiera elegido. Solo se respeta si se eligió a propósito.
  if (!p.startPageChosen && p.startPage === "last") p.startPage = "dashboard";
  cache = p;
  return p;
}

/** Acento, animaciones y zoom según las preferencias y el tema actual. */
export function applyAppearance(p: Prefs = getPrefs()) {
  const root = document.documentElement;
  const a = ACCENTS[p.accent] ?? ACCENTS.blue;
  const light = getTheme() === "light";
  if (p.accent === "blue") {
    root.style.removeProperty("--color-neon");
    root.style.removeProperty("--color-on-neon");
  } else {
    root.style.setProperty("--color-neon", light ? a.light : a.dark);
    root.style.setProperty("--color-on-neon", light ? "#ffffff" : "#0c1422");
  }
  root.classList.toggle("reduce-motion", p.reduceMotion);
  invoke("set_ui_zoom", { scale: p.zoom }).catch(() => {});
}

export function setPrefs(patch: Partial<Prefs>) {
  cache = { ...getPrefs(), ...patch };
  try {
    localStorage.setItem(KEY, JSON.stringify(cache));
  } catch {
    /* sin almacenamiento: dura esta sesión */
  }
  applyAppearance(cache);
  window.dispatchEvent(new Event("adminops-prefs"));
}

export function usePrefs(): Prefs {
  const [p, set] = useState(getPrefs);
  useEffect(() => {
    const on = () => set(getPrefs());
    window.addEventListener("adminops-prefs", on);
    return () => window.removeEventListener("adminops-prefs", on);
  }, []);
  return p;
}

/** Rectángulo de un elemento en píxeles de la ventana (con el zoom aplicado), para las vistas web incrustadas. */
export function windowRect(el: HTMLElement) {
  const r = el.getBoundingClientRect();
  const z = getPrefs().zoom;
  return { x: r.left * z, y: r.top * z, width: r.width * z, height: r.height * z };
}

/** Copia de seguridad de las preferencias (para exportar la configuración). */
export function exportPrefs(): Prefs {
  return getPrefs();
}

export function importPrefs(p: unknown) {
  if (p && typeof p === "object") setPrefs({ ...DEFAULTS, ...(p as Partial<Prefs>) });
}

/** Combinaciones que ya usa AdminOps o Windows: no se pueden asignar. */
export const RESERVED = new Set(["Ctrl+K", "Ctrl+,", "Ctrl+L", "Ctrl+C", "Ctrl+V", "Ctrl+X", "Ctrl+A", "Ctrl+Z", "Ctrl+Y", "Ctrl+F", "Ctrl+P", "Ctrl+R", "Alt+F4", "F5", "Alt+Tab"]);

/** "Ctrl+Alt+D" a partir de una pulsación; null si no es una combinación válida para un atajo. */
export function comboOf(e: KeyboardEvent): string | null {
  const k = e.key;
  if (["Control", "Alt", "Shift", "Meta", "AltGraph"].includes(k)) return null;
  // AltGr (Ctrl+Alt en Windows) escribe @, #, €…: nunca es un atajo.
  if (e.getModifierState?.("AltGraph")) return null;
  let key: string;
  if (/^F([1-9]|1[0-2])$/.test(k)) key = k;
  else if (/^[a-z0-9]$/i.test(k)) key = k.toUpperCase();
  else if (e.code.startsWith("Digit")) key = e.code.slice(5);
  else if (e.code.startsWith("Key")) key = e.code.slice(3);
  else return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean) as string[];
  // Sin modificadores solo valen las teclas F (las letras se escriben en los campos de texto).
  if (!mods.length && !key.startsWith("F")) return null;
  return [...mods, key].join("+");
}

/** Cambia solo algunas opciones de la barra lateral. */
export function setSidebar(patch: Partial<SidebarPrefs>) {
  setPrefs({ sidebar: { ...getPrefs().sidebar, ...patch } });
}
