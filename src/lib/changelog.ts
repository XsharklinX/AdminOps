// Novedades de cada versión, escritas para el técnico: qué puede hacer ahora
// que antes no. Solo versiones, sin fechas. La más nueva va primero.
//
// Al publicar una versión, se añade aquí su entrada: sirve también de registro
// de mantenimiento.

export interface Release {
  version: string;
  /** De qué va la versión, en pocas palabras. */
  title: string;
  /** Lo nuevo. */
  added: string[];
  /** Lo que se arregló o cambió (si lo hay). */
  fixed?: string[];
  /** Lo que se quitó (si lo hay). */
  removed?: string[];
}

export const RELEASES: Release[] = [
  {
    version: "1.1.10",
    title: "Guía, novedades, términos de uso y reportar fallos",
    added: [
      "Guía de AdminOps (el libro de «Acerca de»): qué hace cada pantalla, cómo se usa, qué se puede deshacer y qué no, con glosario y preguntas frecuentes.",
      "Novedades: esta lista, con lo que trae cada versión desde la primera.",
      "Términos de uso, en el instalador y en «Acerca de».",
      "Reportar un problema: describes lo que pasó y AdminOps prepara el correo para el autor con el archivo de diagnóstico adjunto.",
    ],
    removed: [
      "Todo lo de Microsoft 365 que dependía de que la organización registrase una aplicación: inicios de sesión y MFA, calendario de Outlook en la Agenda, presencia y fotos de Teams, aviso por Teams al cerrar un caso. El Correo y Teams dentro de AdminOps siguen igual.",
    ],
  },
  {
    version: "1.1.9",
    title: "Lavado de cara y repaso de fallos",
    added: [
      "Usuarios locales con ficha por cuenta y avisos de lo que conviene revisar (contraseña caducada, administrador sin contraseña, cuentas sin usar).",
      "Procesos agrupados por programa, con «Finalizar el programa» entero.",
      "Acceso remoto con barra de conexión rápida y fichas que dicen si el equipo contesta.",
      "Sesión de servicio por pasos: Motivo, Trabajo, Informe, Cobro, Firma y cierre.",
      "Red en una sola página: router, dispositivos, velocidad y herramientas.",
      "Diario de cambios por días, con buscador y filtros.",
      "Tablas que se ordenan por columna en Dispositivos, Puestos, Puertos, Velocidad y discos.",
      "Carpetas compartidas a fondo: cambiar permisos sin volver a compartir, «¿por qué no puede entrar?», unidades de red, qué comparte otro equipo, tamaños y copia diaria a otro disco.",
      "Agenda con vista de mes e historial; Personas y Clientes en una misma página.",
    ],
    fixed: [
      "Tickets: el inicio de sesión ya no se corta cuando la intranet salta a otro sitio para entrar.",
      "Pantallas que se quedaban en «Leyendo…» para siempre si la primera lectura fallaba: ahora dicen qué pasó y dejan reintentar.",
      "Compartir una carpeta grande ya no tarda minutos ni deja permisos sueltos en cada archivo.",
      "El buscador de Ajustes encuentra todos los ajustes.",
    ],
  },
  {
    version: "1.1.8",
    title: "Discos y pendrive a fondo, e informe profesional",
    added: [
      "Discos: veredicto de cada disco en claro, tendencia entre lecturas, estado de BitLocker, reparar el sistema de archivos, buscar sectores dañados y rescatar los archivos de un disco que falla.",
      "Herramientas de pendrive: velocidad real, comprobar que la capacidad no es falsa, expulsar y formatear.",
      "Informe en PDF rehecho: resumen por áreas, prioridades y tipografía propia.",
      "Pendrive seguro: con bloqueo por PIN, el PIN protege también las contraseñas guardadas.",
      "Copia automática y cifrada de los datos de AdminOps a otra carpeta o a OneDrive.",
    ],
    fixed: [
      "Correo y Teams en el pendrive: el navegador interno trabaja desde el disco del equipo, que es mucho más rápido.",
      "El instalador comprueba que el programa quedó actualizado de verdad y cierra los procesos que se quedaban colgados de una sesión anterior.",
    ],
  },
  {
    version: "1.1.7",
    title: "AdminOps en el pendrive",
    added: [
      "Instalado en un pendrive, AdminOps guarda todos sus datos junto al programa y viajan contigo.",
      "La primera vez se trae lo que ya tenías en ese equipo, con las contraseñas cifradas de nuevo para el pendrive.",
      "Agenda para todo: tareas, llamadas y reuniones, con cliente o sin él.",
      "Avisos de Windows con botones para actuar sin abrir la aplicación, y aviso al momento de los errores de Windows.",
      "Tapar datos en una captura antes de pegarla en un ticket.",
    ],
    removed: ["La Asistencia rápida de Windows, que ya no viene en todos los equipos."],
  },
  {
    version: "1.1.6",
    title: "Que funcione en el equipo del cliente",
    added: ["Agenda, Usuarios, Impresoras y Carpetas compartidas repasadas a partir de usarlas en equipos reales."],
    fixed: [
      "Menos consumo: se precarga un solo portal, y ninguno en equipos justos de memoria.",
      "El Panel ya no recorre todos los procesos cada dos segundos.",
      "Portales que se quedaban en «Abriendo…», la pantalla de Ajustes que no dejaba pulsar nada y el segundo factor de Microsoft rechazado.",
      "El tema claro y la primera ventana del portable se veían en negro.",
    ],
  },
  {
    version: "1.1.5",
    title: "Teams, arranque y portales que no estorban",
    added: ["Teams dentro de AdminOps, sin instalarlo en el equipo del cliente.", "«Volver a ver la bienvenida» en Ajustes."],
    fixed: ["La ventana ya no se abre en negro.", "Los portales ocultos dejan de consumir.", "Primer arranque del portable en equipos con el pendrive protegido o un antivirus estricto."],
  },
  {
    version: "1.1.4",
    title: "Soluciones y Ajustes rehechos",
    added: [
      "Catálogo de soluciones paso a paso para los problemas más comunes.",
      "Ajustes de AdminOps por secciones, con buscador.",
      "Portales y correo con su propia sección de ajustes.",
    ],
  },
  {
    version: "1.1.3",
    title: "Agenda, cuentas y modo auditoría",
    added: [
      "Agenda de visitas con recordatorios.",
      "Cuentas de Microsoft, profesionales y de Office del equipo, y credenciales guardadas.",
      "Modo auditoría: AdminOps solo mira y no cambia nada.",
      "Contactos a fondo: importar, exportar, duplicados y copias.",
      "Copia cifrada de los datos de AdminOps y medición del tiempo de arranque.",
    ],
  },
  {
    version: "1.1.2",
    title: "Herramientas del técnico",
    added: [
      "Entrega rápida: lo hecho hoy pasa al informe.",
      "Tipos de visita con su propia lista de comprobación, que se marca sola.",
      "Plantillas para preparar un equipo de una vez: programas, ajustes, usuario, nombre y dominio.",
      "Ficha del equipo para copiar o pegar en Excel, con enlace a la garantía del fabricante.",
      "Soluciones propias, plantillas de texto y notas por equipo y por red.",
      "Mapa de la oficina y vigilancia de los dispositivos que importan.",
    ],
  },
  {
    version: "1.1.1",
    title: "Equipos de empresa y soporte del día a día",
    added: [
      "Solucionar problemas: eliges el síntoma y AdminOps revisa las causas típicas y ofrece la reparación.",
      "Reparar la red en un clic y control de la Wi‑Fi.",
      "Línea de tiempo del equipo en el Historial.",
      "Contactos: la agenda del técnico, que viaja con AdminOps.",
      "La búsqueda (Ctrl+K) encuentra acciones y problemas, no solo páginas.",
      "En el pendrive, las contraseñas guardadas funcionan en todos los equipos.",
    ],
    fixed: ["Compatibilidad con los antivirus de empresa, que bloqueaban la forma en que se lanzaban las consultas a Windows."],
  },
  {
    version: "1.1",
    title: "Pulido",
    added: [
      "Vigilancia de errores de Windows con explicación de cada uno.",
      "Acceso remoto a fondo: agenda de conexiones, opciones, contraseñas guardadas en Windows, AnyDesk, RustDesk y TeamViewer.",
      "Barra lateral totalmente personalizable.",
      "Más atajos de teclado en la página de atajos.",
    ],
    fixed: ["Un fallo interno ya no cierra la aplicación."],
  },
  {
    version: "1.0",
    title: "Versión 1.0",
    added: [
      "Las páginas conservan lo que tenían al volver a ellas.",
      "Atajos de teclado propios y páginas favoritas en el Panel.",
      "Punto de restauración configurable, inicio con Windows, limpieza de datos antiguos y aviso de versiones nuevas.",
    ],
  },
  {
    version: "0.23",
    title: "Ajustes, rendimiento y red a fondo",
    added: [
      "Ajustes en pestañas: color, tamaño de la interfaz y navegación a medida.",
      "Bloqueo de AdminOps con PIN y copia de la configuración.",
      "Identificación de los dispositivos de la red (tipo, marca y modelo) y contraseña de la Wi‑Fi.",
      "26 herramientas de Windows más.",
    ],
    fixed: ["Panel y diagnóstico más rápidos."],
  },
  {
    version: "0.22",
    title: "La oficina completa",
    added: [
      "Inventario de equipos con veredicto (bien, mejorar, renovar) y exportación a Excel.",
      "Acceso remoto: Escritorio remoto y encendido por red.",
      "Carpetas compartidas y permisos.",
      "Detección de IP duplicadas y mapa de la red en la ficha del cliente.",
    ],
  },
  {
    version: "0.21",
    title: "Caja fuerte y privacidad",
    added: [
      "Caja fuerte cifrada que se abre sin AdminOps, y carpeta cifrada con contraseña.",
      "Borrado seguro de archivos y del espacio libre, con lista de pasos antes de vender un equipo.",
      "Recuperar archivos borrados.",
      "Control parental: filtro de webs, sitios bloqueados y horario de uso.",
    ],
  },
  {
    version: "0.20",
    title: "Red y router",
    added: [
      "Mi red: conexión, router, DNS e IP pública.",
      "Panel del router dentro de la aplicación, con su acceso guardado y cifrado.",
      "Comprobación de seguridad del router y de la Wi‑Fi, y código QR para conectar un móvil.",
      "Dispositivos de la red con su fabricante, nombres propios y aviso de los nuevos.",
    ],
  },
  {
    version: "0.19",
    title: "Informe y cliente",
    added: [
      "Informe en PDF con dos plantillas: para el cliente y técnica.",
      "Presupuesto o recibo con impuestos, y firma del cliente y del técnico.",
      "Envío del informe por correo con el PDF adjunto.",
      "Garantías, recordatorios de mantenimiento y evolución del equipo entre visitas.",
    ],
  },
  {
    version: "0.18",
    title: "Rediseño",
    added: ["Aspecto nuevo, más sobrio, y tema claro.", "Navegación en siete áreas con pestañas.", "Panel y Herramientas rehechos."],
  },
  {
    version: "0.17",
    title: "Instalador y atajos",
    added: ["Instalador con interfaz propia: instala, actualiza o reinstala.", "Instalador clásico para instalar en silencio en muchos equipos.", "Página de atajos de teclado, con modo «descubrir»."],
  },
  {
    version: "0.16",
    title: "Seguridad del equipo",
    added: [
      "Nota de seguridad de 0 a 100, con cada punto explicado y un botón para arreglarlo.",
      "Ajustes de seguridad reversibles.",
      "BitLocker y copia de sus claves de recuperación.",
      "Programas de riesgo desactualizados, elementos sospechosos y extensiones de los navegadores.",
    ],
  },
  {
    version: "0.15",
    title: "Mantenimiento a fondo",
    added: [
      "Desinstalador que limpia lo que dejan los programas.",
      "Windows Update: historial con cada error explicado, pausar y ocultar una actualización.",
      "Análisis del arranque, restaurar drivers y mantenimiento programado.",
      "Reparaciones de la Tienda, la búsqueda, el audio y el perfil temporal.",
    ],
  },
  {
    version: "0.14",
    title: "Orden y pulido",
    added: [
      "Búsqueda global con Ctrl+K.",
      "Barra lateral plegable con favoritos y asistente de primer arranque.",
      "Avisos al terminar tareas largas.",
      "Paquete de soporte.",
    ],
  },
  {
    version: "0.13",
    title: "Entorno corporativo",
    added: ["Tickets: las webs de la empresa dentro de AdminOps, con la sesión recordada.", "Dominio: estado, unir, sacar, reparar la relación de confianza y cambiar el nombre del equipo."],
  },
  {
    version: "0.12",
    title: "Después de formatear",
    added: [
      "Instalar programas en lote, con listas propias.",
      "Copia y restauración de los datos del usuario.",
      "Herramientas de red: ping, traza de ruta, DNS, puertos y archivo hosts.",
      "Impresoras, y el driver probable de cada pantallazo azul.",
    ],
  },
  {
    version: "0.11",
    title: "Caja de herramientas",
    added: ["Unos 90 accesos a las utilidades de Windows, con favoritos y accesos propios.", "Usuarios locales.", "Ficha rápida del equipo y redes Wi‑Fi guardadas con su contraseña."],
  },
  {
    version: "0.10",
    title: "Hardware",
    added: ["Inventario de piezas: placa, BIOS, memoria por ranura, gráfica y monitores.", "Temperaturas en vivo y estado SMART de los discos.", "Prueba de memoria y prueba de velocidad."],
  },
  {
    version: "0.9",
    title: "Herramientas y flujo del técnico",
    added: [
      "Procesos, analizador de espacio, copia de drivers y actualización de programas.",
      "Sesión de servicio y clientes.",
      "Perfiles propios, tu marca en el informe y lista de comprobación.",
    ],
  },
  {
    version: "0.7",
    title: "Robustez y rendimiento",
    added: ["Tareas largas con progreso en vivo y botón de cancelar.", "Registro de actividad.", "Diagnóstico interactivo: cada hallazgo lleva a su arreglo."],
    fixed: ["Nada puede dejar colgada la interfaz: todo tiene límite de tiempo.", "Arranque y consultas a Windows más rápidos."],
  },
  {
    version: "0.5",
    title: "Perfiles y portable",
    added: ["Perfiles que aplican muchos ajustes de una vez.", "Modo portable para llevarlo en un pendrive."],
  },
  {
    version: "0.4",
    title: "Diagnóstico",
    added: ["Diagnóstico de discos, pantallazos azules, drivers, batería y seguridad.", "Reparaciones de Windows.", "Informe en PDF con el antes y el después."],
  },
  {
    version: "0.3",
    title: "Catálogo de ajustes",
    added: ["Limpieza, privacidad, rendimiento, servicios, aplicaciones preinstaladas e inicio de Windows."],
  },
  {
    version: "0.2",
    title: "Motor de ajustes",
    added: ["Cada ajuste detecta cómo está el equipo, guarda el valor anterior y se puede deshacer.", "Diario de cambios y puntos de restauración."],
  },
  {
    version: "0.1",
    title: "La base",
    added: ["Panel del equipo en vivo.", "Arranque con permisos de administrador cuando hacen falta."],
  },
];
