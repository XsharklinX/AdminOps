// Las secciones (pestañas) de cada pantalla, en un solo sitio: la barra lateral
// las enseña como líneas, «Todo» las lista y la ruta de arriba dice en cuál estás.
// Los id son los de las pestañas de cada página (pages/Merged.tsx, Knowledge):
// una prueba comprueba que coinciden.
//
// `keywords`: otras formas de decirlo, para que el buscador lo encuentre aunque
// no se use el nombre exacto («AD» → Dominio, «no imprime» → Impresoras).
import type { PageId } from "../components/Sidebar";

export interface Section {
  id: string;
  label: string;
  keywords?: string;
}

export const SECTIONS: Partial<Record<PageId, Section[]>> = {
  session: [
    { id: "session", label: "Sesión", keywords: "visita mantenimiento checklist" },
    { id: "report", label: "Informe", keywords: "pdf entregar cliente firma cobro" },
  ],
  machine: [
    { id: "diagnostics", label: "Diagnóstico", keywords: "problemas hallazgos revisar salud" },
    { id: "hardware", label: "Hardware", keywords: "piezas temperatura cpu ram placa bios ficha serie modelo" },
    { id: "performance", label: "Rendimiento", keywords: "lento historial grafica procesador memoria picos va lento desde semana" },
    { id: "security", label: "Seguridad", keywords: "antivirus firewall bitlocker defender nota" },
    { id: "boots", label: "Arranques y cuelgues", keywords: "pantallazo azul bsod pantalla azul cuelgue reinicio apagon tarda en arrancar bugcheck minidump" },
    { id: "peripherals", label: "Probar periféricos", keywords: "teclado pixel muerto pantalla altavoz sonido microfono no me oyen camara webcam probar" },
    { id: "history", label: "Historial del equipo", keywords: "diario deshacer cambios linea de tiempo eventos" },
  ],
  tweaks: [
    { id: "cleanup", label: "Limpieza", keywords: "temporales basura liberar caché" },
    { id: "performance", label: "Rendimiento", keywords: "rapido lento acelerar" },
    { id: "privacy", label: "Privacidad", keywords: "telemetria rastreo anuncios" },
    { id: "services", label: "Servicios", keywords: "servicio windows" },
    { id: "startup", label: "Inicio de Windows", keywords: "arranque programas al iniciar startup" },
  ],
  space: [
    { id: "space", label: "Espacio", keywords: "liberar lleno ocupa carpetas grandes" },
    { id: "health", label: "Salud y reparación", keywords: "smart chkdsk sectores dañado rescatar pendrive clonar imagen superficie autoprueba vigilante crystaldiskinfo" },
    { id: "partitions", label: "Particiones y arranque", keywords: "particion perdida tabla gpt mbr testdisk sin formato arranque bcd bootmgr disco sin asignar" },
  ],
  data: [
    { id: "migrate", label: "Copia de datos", keywords: "migrar pasar a otro pc perfil marcadores" },
    { id: "backups", label: "Copias de seguridad", keywords: "backup copia seguridad comprobar restaurar reciente otro disco outlook documentos" },
    { id: "vault", label: "Caja fuerte", keywords: "cifrar contraseñas secreto" },
    { id: "wipe", label: "Borrado seguro", keywords: "borrar sin recuperar destruir" },
    { id: "recover", label: "Recuperar archivos", keywords: "borrados papelera recuperar" },
    { id: "family", label: "Control parental", keywords: "menor niños hijos limite" },
  ],
  router: [
    { id: "router", label: "Red y router", keywords: "wifi router contraseña wifi conexion" },
    { id: "devices", label: "Dispositivos", keywords: "escanear red equipos conectados ip mac" },
    { id: "speed", label: "Velocidad y diagnóstico", keywords: "test velocidad lento internet reparar red" },
    { id: "watch", label: "Vigilante de la conexión", keywords: "cortes se corta internet intermitente se cae la conexion proveedor ping continuo" },
    { id: "tools", label: "Herramientas de red", keywords: "ping traceroute puertos dns hosts nslookup calculadora subred mascara cidr" },
  ],
  apps: [
    { id: "update", label: "Actualizar", keywords: "winget update versiones nuevas" },
    { id: "winupdate", label: "Windows Update", keywords: "parches kb actualizaciones windows" },
    { id: "install", label: "Instalar", keywords: "descargar instalar programa catalogo" },
    { id: "uninstall", label: "Desinstalar", keywords: "quitar eliminar programa restos" },
    { id: "bloatware", label: "Bloatware", keywords: "apps preinstaladas basura store" },
  ],
  users: [
    { id: "users", label: "Usuarios de este equipo", keywords: "usuario local contraseña administrador crear cuenta" },
    { id: "accounts", label: "Cuentas", keywords: "microsoft office entra azure credenciales guardadas" },
    { id: "domain", label: "Dominio", keywords: "ad active directory unir al dominio salir relacion de confianza renombrar equipo controlador" },
  ],
  printers: [
    { id: "printers", label: "Impresoras", keywords: "imprimir no imprime cola atascada toner escaner" },
    { id: "shares", label: "Carpetas compartidas", keywords: "compartir permisos unidad de red recurso no puede entrar" },
  ],
  recipes: [
    { id: "recipes", label: "Plantillas", keywords: "receta equipo nuevo preparar" },
    { id: "profiles", label: "Perfiles de ajustes", keywords: "perfil ajustes" },
  ],
  tools: [
    { id: "tools", label: "Herramientas", keywords: "regedit eventos msconfig panel de control bios servicios" },
    { id: "shortcuts", label: "Atajos de teclado", keywords: "teclas atajos combinaciones" },
  ],
  inventory: [
    { id: "inventory", label: "Mi inventario", keywords: "equipos ficha veredicto renovar excel" },
    { id: "webinventory", label: "Inventario web", keywords: "web empresa intranet" },
  ],
  people: [
    { id: "people", label: "Personas", keywords: "dominio empleados quien bloqueado" },
    { id: "clients", label: "Clientes", keywords: "cliente garantia visitas equipos" },
  ],
  knowledge: [
    { id: "solutions", label: "Soluciones", keywords: "conocimiento pasos problema" },
    { id: "templates", label: "Plantillas de texto", keywords: "respuestas correos textos" },
    { id: "notes", label: "Notas", keywords: "notas equipos redes apuntar" },
  ],
};

/** Otras formas de llamar a cada pantalla (además de su nombre y su ayuda). */
export const PAGE_KEYWORDS: Partial<Record<PageId, string>> = {
  dashboard: "inicio resumen panel en vivo",
  troubleshoot: "no hay internet no imprime no suena va lento pantalla sintoma arreglar reparar sfc dism",
  processes: "administrador de tareas cerrar programa colgado cpu memoria",
  remote: "rdp escritorio remoto conectar anydesk rustdesk",
  stations: "equipos oficina estaciones puestos",
  agenda: "calendario cita visita recordatorio seguimiento",
  tickets: "incidencias glpi helpdesk",
  contacts: "telefono extension agenda llamar",
  mail: "outlook email correo",
  teams: "chat llamada reunion",
  settings: "configuracion preferencias",
};

export const sectionsOf = (page: PageId): Section[] => SECTIONS[page] ?? [];

export const sectionLabel = (page: PageId, id: string | null | undefined): string | null =>
  (id && SECTIONS[page]?.find((s) => s.id === id)?.label) || null;

/** Clave de una línea de la barra: "page" o "page:sección" (también la de los fijados). */
export const navKey = (page: PageId, section?: string | null) => (section ? `${page}:${section}` : page);

export function parseNavKey(key: string): { page: PageId; section: string | null } {
  const i = key.indexOf(":");
  return i < 0 ? { page: key as PageId, section: null } : { page: key.slice(0, i) as PageId, section: key.slice(i + 1) };
}
