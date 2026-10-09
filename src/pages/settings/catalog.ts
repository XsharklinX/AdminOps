// Índice de los ajustes, para el buscador de la página de Ajustes: escribes
// «diagnóstico» o «contraseña» y te lleva a su sección, sin recorrer pestañas.
//
// Cada entrada apunta a la sección y, cuando es una fila concreta, a su título
// exacto (las filas se marcan con `data-setting`, así se resaltan al llegar).
import type { LucideIcon } from "lucide-react";
import { Bell, Building2, DatabaseBackup, Info, LayoutDashboard, Lock, Palette, Power, SlidersHorizontal, Ticket } from "lucide-react";

export const SECTIONS = [
  { id: "summary", group: "Uso", label: "Resumen", icon: LayoutDashboard, hint: "Lo importante de un vistazo: bloqueo, copias, versión y dónde están tus datos." },
  { id: "start", group: "Uso", label: "Inicio y ventana", icon: Power, hint: "Para quién es AdminOps, qué hace al abrirse y qué pasa al cerrar la ventana." },
  { id: "alerts", group: "Uso", label: "Avisos", icon: Bell, hint: "Qué vigila AdminOps en Windows y cuándo te avisa." },
  { id: "appearance", group: "Uso", label: "Apariencia", icon: Palette, hint: "Tema, color, tamaño, ancho de las pantallas y animaciones." },
  { id: "navigation", group: "Uso", label: "Navegación", icon: SlidersHorizontal, hint: "La barra lateral, las pestañas, las secciones y tus atajos de teclado." },
  { id: "data", group: "Datos", label: "Datos y copias", icon: DatabaseBackup, hint: "Dónde están tus datos, sus copias, la limpieza y la configuración para otro equipo." },
  { id: "security", group: "Datos", label: "Seguridad", icon: Lock, hint: "El bloqueo con PIN o contraseña, y los puntos de restauración antes de cambiar Windows." },
  { id: "reports", group: "Trabajo", label: "Informes y cobros", icon: Building2, hint: "Tu marca, firma, checklist, garantías e impuestos." },
  { id: "portals", group: "Trabajo", label: "Portales y red", icon: Ticket, hint: "Tickets, inventario web, el Correo, Teams y el dominio habitual." },
  { id: "about", group: "AdminOps", label: "Acerca de", icon: Info, hint: "Versión y actualizaciones, licencias, y lo que tarda AdminOps en abrirse." },
] as const satisfies readonly { id: string; group: string; label: string; icon: LucideIcon; hint: string }[];

/** Las secciones de antes que se pueden seguir pidiendo desde un enlace. */
export const LEGACY_SECTIONS: Record<string, SettingsSection> = { general: "start", performance: "about" };

export type SettingsSection = (typeof SECTIONS)[number]["id"];

export interface SettingEntry {
  section: SettingsSection;
  /** Título exacto de la fila (para resaltarla), o el de un bloque. */
  title: string;
  /** Palabras por las que también se encuentra. */
  keywords?: string;
}

export const SETTINGS_INDEX: SettingEntry[] = [
  // Inicio y ventana, avisos, datos
  { section: "start", title: "Para quién es AdminOps en este equipo", keywords: "modo tecnico usuario sencillo cliente interfaz simple" },
  { section: "start", title: "Página al abrir AdminOps", keywords: "inicio arranque primera pagina" },
  { section: "start", title: "Volver a ver la bienvenida", keywords: "asistente inicio primera vez tutorial modo bienvenida" },
  { section: "start", title: "Actualización del Panel", keywords: "refresco segundos cpu memoria" },
  { section: "start", title: "Diagnosticar al abrir AdminOps", keywords: "diagnostico automatico analisis" },
  { section: "alerts", title: "Vigilar errores de Windows", keywords: "avisos eventos campana pantallazos" },
  { section: "start", title: "Icono junto al reloj", keywords: "bandeja tray area de notificacion icono reloj segundo plano" },
  { section: "start", title: "Al cerrar la ventana, minimizar", keywords: "cerrar x minimizar barra de tareas salir segundo plano" },
  { section: "alerts", title: "Notificaciones de Windows de los avisos", keywords: "notificacion campana graves silenciar molestar" },
  { section: "alerts", title: "Avisar al terminar tareas largas", keywords: "notificacion tareas" },
  { section: "security", title: "Punto de restauración antes de cambiar el sistema", keywords: "restaurar seguridad deshacer" },
  { section: "start", title: "Abrir AdminOps al iniciar Windows", keywords: "arranque automatico inicio" },
  { section: "about", title: "Avisar de versiones nuevas", keywords: "actualizacion adminops" },
  { section: "data", title: "Limpieza automática al abrir", keywords: "datos antiguos historial informes" },
  { section: "data", title: "Copia de la configuración", keywords: "exportar importar respaldo ajustes" },
  { section: "data", title: "Datos de AdminOps", keywords: "carpeta espacio historial diagnosticos borrar" },
  { section: "about", title: "Actualizaciones", keywords: "version nueva adminops buscar actualizar" },
  { section: "data", title: "Seguridad de tus datos", keywords: "copia cifrada respaldo restaurar usb" },
  { section: "data", title: "Configuración de empresa", keywords: "exportar importar empresa otro tecnico equipo portales dominio" },
  { section: "data", title: "Copia automática", keywords: "onedrive respaldo copia seguridad pendrive usb automatica" },
  { section: "data", title: "Dónde se guardan tus datos", keywords: "pendrive usb portable carpeta programa datos viajan" },
  { section: "data", title: "Tus datos viajan con AdminOps", keywords: "pendrive usb portable carpeta programa navegador" },
  // Apariencia
  { section: "appearance", title: "Tema", keywords: "oscuro claro automatico" },
  { section: "appearance", title: "Color de acento", keywords: "azul verde violeta naranja" },
  { section: "appearance", title: "Tamaño de la interfaz", keywords: "zoom letra grande pequeño" },
  { section: "navigation", title: "Pestañas de las pantallas abiertas", keywords: "pestañas tabs abiertas ctrl tab cerrar" },
  { section: "appearance", title: "Ancho de las pantallas", keywords: "ancho pantalla completa monitor grande centrado espacio margen" },
  { section: "appearance", title: "Reducir animaciones", keywords: "movimiento accesibilidad" },
  // Navegación
  { section: "navigation", title: "Atajos para abrir páginas", keywords: "teclado combinacion ctrl propios" },
  { section: "navigation", title: "Estructura", keywords: "secciones paginas ocultar ordenar areas renombrar icono" },
  { section: "navigation", title: "Barra lateral", keywords: "ancho posicion iconos densidad acoplar solo areas estado marcas buscador al pulsar un area" },
  { section: "navigation", title: "Accesos rápidos en la barra", keywords: "favoritos fijados chincheta recientes" },
  // Portales y red
  { section: "portals", title: "Dominio habitual", keywords: "dominio unir equipos active directory ad" },
  { section: "portals", title: "Cómo se abren los portales", keywords: "precargar cerrar zoom tickets correo teams" },
  { section: "portals", title: "Precargar el último portal", keywords: "rapido tickets correo teams inventario" },
  { section: "portals", title: "Zoom de los portales", keywords: "tamaño letra web" },
  { section: "portals", title: "Cómo se abren Teams y el Correo", keywords: "navegador aplicacion outlook teams abrir fuera preguntar" },
  { section: "portals", title: "Dominio de la empresa", keywords: "intranet cuenta windows autenticacion integrada" },
  { section: "portals", title: "Portales configurados", keywords: "tickets inventario correo teams web sesion privada privacidad rellenar inicio de sesion cuenta guardada" },
  // Seguridad
  { section: "security", title: "Bloqueo de AdminOps", keywords: "pin contraseña bloquear" },
  { section: "security", title: "Bloquear por inactividad", keywords: "minutos tiempo automatico bloqueo" },
  // Informes
  { section: "reports", title: "Tu marca en los informes", keywords: "empresa telefono correo web logotipo" },
  { section: "reports", title: "Logo", keywords: "imagen marca" },
  { section: "reports", title: "Checklist de servicio", keywords: "visita pasos" },
  { section: "reports", title: "Tu plantilla de informe", keywords: "secciones orden pdf personalizar propia quitar informe" },
  { section: "reports", title: "Tipos de visita", keywords: "checklist propia mantenimiento instalacion sesion" },
  { section: "reports", title: "Tu firma", keywords: "firmar informe" },
  { section: "reports", title: "Presupuestos, recibos y garantías", keywords: "impuesto itbis iva numeracion mantenimiento" },
  // Acerca de
  { section: "about", title: "Acerca de", keywords: "guia ayuda manual glosario novedades version terminos de uso licencia reportar problema fallo soporte" },
  // Rendimiento (en Acerca de)
  { section: "about", title: "Arranque de AdminOps", keywords: "tiempo lento medir" },
  { section: "about", title: "Carga de páginas", keywords: "tiempo lento medir" },
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
