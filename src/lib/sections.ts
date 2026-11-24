// Las secciones (pestañas) de cada pantalla, en un solo sitio: la barra lateral
// las enseña como líneas, «Todo» las lista y la ruta de arriba dice en cuál estás.
// Los id son los de las pestañas de cada página (pages/Merged.tsx, Knowledge):
// una prueba comprueba que coinciden.
//
// `keywords`: otras formas de decirlo, para que el buscador lo encuentre aunque
// no se use el nombre exacto («AD» → Dominio, «no imprime» → Impresoras).
import { WEB_TICKETS } from "./features";
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
    { id: "connected", label: "Conectados", keywords: "dispositivos conectados administrador de dispositivos sin driver enchufado usb lector de tarjetas desconocido error codigo 28 recien conectado" },
    { id: "performance", label: "Rendimiento", keywords: "lento historial grafica procesador memoria picos va lento desde semana" },
    { id: "recorder", label: "Grabar el fallo", keywords: "grabar pantalla video grabacion fallo error aleatorio a veces se cuelga no responde se congela solo pasa cuando el usuario lo hace reproducir problema que no consigo ver captura linea de tiempo" },
    { id: "security", label: "Seguridad", keywords: "antivirus firewall bitlocker defender nota" },
    { id: "boots", label: "Arranques y cuelgues", keywords: "pantallazo azul bsod pantalla azul cuelgue reinicio apagon tarda en arrancar bugcheck minidump" },
    { id: "peripherals", label: "Probar periféricos", keywords: "teclado pixel muerto pantalla altavoz sonido microfono no me oyen camara webcam probar" },
    { id: "displays", label: "Pantallas", keywords: "monitor borroso estirado segundo monitor no detecta pantalla resolucion escala hz frecuencia hdmi displayport duplicar extender proyector" },
    { id: "keyboard", label: "Teclado e idioma", keywords: "ñ no sale arroba otro sitio lugar comillas raras teclado en ingles idioma alt mayus cambia solo distribucion" },
    { id: "history", label: "Historial del equipo", keywords: "diario deshacer cambios linea de tiempo eventos" },
    { id: "changed", label: "¿Qué cambió?", keywords: "desde el martes va mal instalado nuevo driver actualizacion servicio cambio comparar foto diaria" },
    { id: "ask", label: "Pregúntale al equipo", keywords: "por que se reinicio anoche lento quien instalo no imprime se apago se colgo pregunta" },
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
    { id: "recover", label: "Recuperar archivos", keywords: "borrados papelera recuperar excel no se guardo word se cerro sin guardar autorrecuperacion version anterior" },
    { id: "family", label: "Control parental", keywords: "menor niños hijos limite" },
  ],
  router: [
    { id: "router", label: "Red y router", keywords: "wifi router contraseña wifi conexion" },
    { id: "devices", label: "Dispositivos", keywords: "escanear red equipos conectados ip mac" },
    { id: "speed", label: "Velocidad y diagnóstico", keywords: "test velocidad lento internet reparar red" },
    { id: "usage", label: "Uso de la red", keywords: "internet va lento quien usa internet consume ancho de banda subida bajada onedrive sincronizando descarga videollamada se corta come se come comiendo pausar onedrive que programa gasta internet" },
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
    { id: "scanners", label: "Escáneres", keywords: "escanear no escanea ricoh epson fi scansnap paperstream epson scan wia twain escanear a carpeta configurar escaner ricoh libreta de direcciones carpeta compartida smb usuario contraseña escaneos" },
    { id: "drivers", label: "Drivers", keywords: "driver controlador biblioteca ricoh epson actualizar impresora pendrive" },
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
    ...(WEB_TICKETS ? [{ id: "webinventory", label: "Inventario web", keywords: "web empresa intranet" }] : []),
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
