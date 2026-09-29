// Índice de los ajustes, para el buscador de la página de Ajustes: escribes
// «diagnóstico» o «contraseña» y te lleva a su sección, sin recorrer pestañas.
//
// Cada entrada apunta a la sección y, cuando es una fila concreta, a su título
// exacto (las filas se marcan con `data-setting`, así se resaltan al llegar).
import type { LucideIcon } from "lucide-react";
import { Building2, Gauge, Info, Lock, Palette, Settings2, SlidersHorizontal, Ticket } from "lucide-react";

export const SECTIONS = [
  { id: "general", label: "General", icon: Settings2, hint: "Cómo se comporta AdminOps al abrirse y mientras trabajas." },
  { id: "appearance", label: "Apariencia", icon: Palette, hint: "Tema, color, tamaño de la interfaz y animaciones." },
  { id: "navigation", label: "Navegación", icon: SlidersHorizontal, hint: "La barra lateral, las secciones y tus atajos de teclado." },
  { id: "portals", label: "Portales y correo", icon: Ticket, hint: "Tickets, inventario web y el Correo: cómo se cargan y se cierran." },
  { id: "security", label: "Seguridad", icon: Lock, hint: "Bloqueo con PIN o contraseña y modo solo mirar." },
  { id: "reports", label: "Informes y cobros", icon: Building2, hint: "Tu marca, firma, checklist, garantías e impuestos." },
  { id: "performance", label: "Rendimiento", icon: Gauge, hint: "Cuánto tarda AdminOps en abrirse y cada página en cargar." },
  { id: "about", label: "Acerca de", icon: Info, hint: "Versión, licencias y datos de la aplicación." },
] as const satisfies readonly { id: string; label: string; icon: LucideIcon; hint: string }[];

export type SettingsSection = (typeof SECTIONS)[number]["id"];

export interface SettingEntry {
  section: SettingsSection;
  /** Título exacto de la fila (para resaltarla), o el de un bloque. */
  title: string;
  /** Palabras por las que también se encuentra. */
  keywords?: string;
}

export const SETTINGS_INDEX: SettingEntry[] = [
  // General
  { section: "general", title: "Página al abrir AdminOps", keywords: "inicio arranque primera pagina" },
  { section: "general", title: "Actualización del Panel", keywords: "refresco segundos cpu memoria" },
  { section: "general", title: "Precargar los portales", keywords: "tickets inventario correo rapido" },
  { section: "general", title: "Diagnosticar al abrir AdminOps", keywords: "diagnostico automatico analisis" },
  { section: "general", title: "Vigilar errores de Windows", keywords: "avisos eventos campana pantallazos" },
  { section: "general", title: "Avisar al terminar tareas largas", keywords: "notificacion tareas" },
  { section: "general", title: "Punto de restauración antes de cambiar el sistema", keywords: "restaurar seguridad deshacer" },
  { section: "general", title: "Abrir AdminOps al iniciar Windows", keywords: "arranque automatico inicio" },
  { section: "general", title: "Avisar de versiones nuevas", keywords: "actualizacion adminops" },
  { section: "general", title: "Limpieza automática al abrir", keywords: "datos antiguos historial informes" },
  { section: "general", title: "Copia de la configuración", keywords: "exportar importar respaldo ajustes" },
  { section: "general", title: "Datos de AdminOps", keywords: "carpeta espacio historial diagnosticos borrar" },
  { section: "general", title: "Seguridad de tus datos", keywords: "copia cifrada respaldo restaurar usb" },
  // Apariencia
  { section: "appearance", title: "Tema", keywords: "oscuro claro automatico" },
  { section: "appearance", title: "Color de acento", keywords: "azul verde violeta naranja" },
  { section: "appearance", title: "Tamaño de la interfaz", keywords: "zoom letra grande pequeño" },
  { section: "appearance", title: "Reducir animaciones", keywords: "movimiento accesibilidad" },
  // Navegación
  { section: "navigation", title: "Atajos propios", keywords: "teclado combinacion ctrl" },
  { section: "navigation", title: "Barra lateral", keywords: "ancho posicion iconos densidad acoplar" },
  { section: "navigation", title: "Secciones y páginas", keywords: "ocultar ordenar areas favoritos" },
  // Portales
  { section: "portals", title: "Precargar el último portal", keywords: "rapido tickets correo inventario" },
  { section: "portals", title: "Cerrar las sesiones privadas al salir", keywords: "correo outlook privacidad cliente" },
  { section: "portals", title: "Zoom de los portales", keywords: "tamaño letra web" },
  { section: "portals", title: "Dominio de la empresa", keywords: "intranet cuenta windows autenticacion integrada" },
  { section: "portals", title: "Portales configurados", keywords: "tickets inventario correo web" },
  // Seguridad
  { section: "security", title: "Bloqueo de AdminOps", keywords: "pin contraseña bloquear inactividad" },
  { section: "security", title: "Modo solo mirar", keywords: "auditoria sin cambios" },
  // Informes
  { section: "reports", title: "Tu marca en los informes", keywords: "empresa telefono correo web logotipo" },
  { section: "reports", title: "Logo", keywords: "imagen marca" },
  { section: "reports", title: "Checklist de servicio", keywords: "visita pasos" },
  { section: "reports", title: "Tu firma", keywords: "firmar informe" },
  { section: "reports", title: "Presupuestos, recibos y garantías", keywords: "impuesto itbis iva numeracion mantenimiento" },
  // Rendimiento
  { section: "performance", title: "Arranque de AdminOps", keywords: "tiempo lento medir" },
  { section: "performance", title: "Carga de páginas", keywords: "tiempo lento medir" },
];

/** Sin tildes ni mayúsculas, para que «diagnostico» encuentre «Diagnosticar». */
const fold = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase();

/** Ajustes que coinciden con lo escrito (todas las palabras). */
export function findSettings(query: string): SettingEntry[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  const label = new Map(SECTIONS.map((s) => [s.id as string, s.label]));
  return SETTINGS_INDEX.filter((e) => {
    const hay = fold(`${e.title} ${e.keywords ?? ""} ${label.get(e.section) ?? ""}`);
    return words.every((w) => hay.includes(w));
  }).slice(0, 12);
}
