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
    version: "1.2.4",
    title: "Más a mano: duplicados, panel a tu medida, icono junto al reloj y mini monitor",
    added: [
      "Discos → Espacio → «Archivos duplicados»: encuentra el mismo archivo guardado varias veces (de 1 MB en adelante), dice cuánto se recupera y deja marcar las copias para mandarlas a la papelera. No mira dentro de Windows ni lee lo que solo está en la nube.",
      "Espacio → «Liberar lo verde»: vacía de una vez los sitios seguros de «Qué puedes liberar» (temporales y caché de Windows Update), sin tocar nada del usuario.",
      "Panel a tu medida: «Personalizar el panel» deja ordenar las tarjetas, arrastrando o con las flechas, y ocultar las que no uses. Se recuerda por técnico.",
      "Panel → «Qué frena el equipo ahora»: lo que más pesa en este momento, dicho en claro. Lo que es de Windows o del antivirus se explica; un programa del usuario se puede cerrar desde ahí.",
      "Icono junto al reloj (Ajustes → General, apagado de fábrica): clic para abrir AdminOps; clic derecho para un análisis rápido, la nota de llamada, el mini monitor, los avisos o salir. Con «Al cerrar la ventana, minimizar», la X la esconde en el icono.",
      "Mini monitor: una ventanita siempre encima con procesador, memoria, temperatura y red, para vigilar el equipo mientras pruebas otra cosa. Se abre desde Rendimiento, desde Ctrl+K o desde el icono junto al reloj.",
      "La campana tiene una pestaña «Actividad»: lo que AdminOps terminó en esta sesión (análisis, copias, instalaciones), cuánto tardó y un enlace al resultado.",
      "Ctrl+K hace más cosas al momento: vaciar la cola de impresión, reiniciar el sonido o el Explorador, vaciar la caché de DNS, limpiar temporales, sincronizar la hora, crear un punto de restauración o lanzar un análisis rápido.",
      "Copiar una tarjeta como imagen: al pasar el ratón por una tarjeta sale un botón en su esquina; la deja en el portapapeles para pegarla en Teams o en un correo.",
      "Marca «Nuevo» en la barra lateral: tras actualizar, señala las pantallas que cambiaron. Se va al entrar.",
    ],
  },
  {
    version: "1.2.3",
    title: "Avisos que se pueden ordenar y callar, y un arranque más ligero",
    added: [
      "La campana, rehecha: los avisos van por días (Hoy, Ayer…), se filtran por nivel (graves, avisos, información) o por «solo sin leer», y lo ya visto queda en una línea que se despliega al pulsar.",
      "Descartar un aviso suelto, sin tener que vaciar la lista entera.",
      "«No avisar más de esto en este equipo»: un aviso que ya conoces (ese programa viejo que se cierra siempre) se silencia y no vuelve a salir ni en la campana ni como notificación. Se deshace en «Silenciados».",
      "Ajustes → General: elegir de qué avisos salta además la notificación de Windows: de todos, solo de los graves o de ninguno. A la campana siguen llegando todos.",
      "Si la vigilancia está desactivada, la campana lo dice y lleva a Ajustes.",
      "Ajustes → General → «Al cerrar la ventana, minimizar» (apagado de fábrica): la X deja AdminOps en la barra de tareas en vez de cerrarla. Para cerrarla de verdad aparece el botón «Salir» en la barra de arriba. Junto a él sigue «Abrir AdminOps al iniciar Windows», minimizada, también apagado de fábrica.",
      "Discos → Espacio, de mirar a manejar: una barra de colores dice de qué está llena cada carpeta (vídeo, imágenes, correo guardado, instaladores…) y al pulsar un color salen los archivos más grandes de ese tipo.",
      "Espacio: un clic en una carpeta enseña su ficha (cuánto ocupa, qué parte del total, cuántos archivos, cuándo cambió por última vez) con sus acciones; doble clic entra. La lista se ordena por tamaño, antigüedad, número de archivos o nombre.",
      "Espacio: se ven los archivos de la carpeta en la que estás, con buscador por nombre o extensión. Antes una carpeta llena de archivos sueltos, como Descargas, salía casi vacía.",
      "Espacio: se pueden marcar carpetas enteras, además de archivos, y mandarlo todo a la papelera de una vez. Las cifras se actualizan solas, sin volver a analizar. Windows, los programas instalados y los perfiles de usuario quedan protegidos.",
    ],
    fixed: [
      "El instalador (Setup) decía que no había podido actualizar cuando sí lo había hecho, y mandaba al instalador clásico. Comparaba el programa instalado con una copia que difería en tres letras internas, así que la comprobación no podía salir bien nunca.",
      "Arranque: lo que AdminOps prepara por detrás (la consola de PowerShell, la seguridad) espera ahora a que la ventana esté pintada, en vez de competir con ella. Se nota sobre todo desde un pendrive o en un equipo ocupado.",
      "«Este equipo y esta red» (las notas del Panel) ya no pregunta a PowerShell en cada arranque: una red ya vista se reconoce al instante. Tardaba unos 7 s desde un pendrive.",
      "El último análisis se guarda en memoria: el Panel y la barra de arriba lo pedían cada minuto y cada vez se leía del disco.",
      "Temperaturas: si una lectura va lenta, las siguientes se espacian solas en lugar de encadenarse.",
      "La precarga del último portal y la comprobación de versión nueva se hacen más tarde, y la precarga se salta si el arranque fue lento.",
    ],
  },
  {
    version: "1.2.2",
    title: "Todo a la vista: navegación nueva, y herramientas para ver qué le pasa al equipo",
    added: [
      "Barra lateral nueva: a la izquierda la columna de áreas (Inicio, Este equipo, Red, Programas, Administración, Soporte) y, al lado, cada pantalla con sus secciones como líneas. Lo que antes estaba detrás de una pestaña (Dominio, Inicio de Windows, Carpetas compartidas, Control parental…) se ve sin abrir nada.",
      "Barra de arriba con el equipo en una línea: nombre, dominio, permisos, Internet, espacio libre y avisos del último diagnóstico. Cada dato lleva a donde se mira o se arregla: el dominio a Dominio, el disco a Espacio, los avisos al Diagnóstico.",
      "«Todo AdminOps» (botón «Todo», Ctrl+K o F1): el programa entero en una pantalla, con una tarjeta por área y sus secciones. Al escribir busca también por otros nombres: «AD» lleva a Dominio, «arranque» a Inicio de Windows, «no imprime» a Impresoras.",
      "Estado al lado de cada sección: los avisos del diagnóstico, si el equipo está en el dominio, el espacio que queda o si no hay Internet. Un punto en el área avisa de que dentro hay algo que atender.",
      "Fijar con la chincheta cualquier línea de la barra, también una sección, para tenerla arriba.",
      "Encima del título se ve dónde estás (área › pantalla › sección), y cada parte se puede pulsar.",
      "Teams y Correo se abren desde sus iconos de la barra de arriba, encima de lo que estés viendo. Herramientas de Windows, Bloquear y Ajustes están abajo en la columna de áreas.",
      "El nombre del equipo, el dominio y el espacio libre se leen directamente de Windows, sin PowerShell: la barra de arriba se rellena al momento.",
      "Rendimiento de los últimos 7 días (Estado del equipo → Rendimiento): procesador, memoria y disco minuto a minuto, y qué programa había detrás de cada pico. Para «va lento desde el martes».",
      "Arranques y cuelgues (Estado del equipo): arranques, apagones, pantallazos azules con su código explicado y qué mirar, y cuánto tarda en arrancar, de los últimos 60 días.",
      "Probar periféricos (Estado del equipo): pantalla para píxeles muertos, teclado tecla a tecla, altavoz izquierdo y derecho, micrófono y cámara.",
      "Vigilante de la conexión (Red): se deja en marcha y apunta cada corte de Internet, cuánto duró y si fallaba el router o la línea. «Copiar resumen» lo deja listo para el proveedor.",
      "Mapa del espacio en disco (Discos → Espacio): cada carpeta como un rectángulo de su tamaño; se ve de un vistazo qué se come el disco.",
      "Calculadora de red (Red → Herramientas de red): rango, máscara, cuántos equipos caben y si dos IP están en la misma red.",
      "Teams y el Correo se abren como prefieras: dentro de AdminOps, en el navegador o en su aplicación de Windows. La primera vez se pregunta; se cambia en Ajustes → Portales y correo.",
      "Siluetas mientras carga una pantalla, en lugar del círculo girando.",
      "Al volver a Desinstalar o a Inicio de Windows se ve al momento lo último leído, y se actualiza por detrás.",
      "El análisis de espacio y la prueba de velocidad avisan al terminar, como las demás tareas largas, si estás en otra ventana.",
    ],
    fixed: [
      "Quedaba el dominio de una organización real en un mensaje de error de Dominio y en ejemplos internos; ahora es empresa.local.",
      "El Panel tardaba de 3 a 6 segundos en terminar de cargar al abrir AdminOps: esperaba a identificar «esta red» y el número de serie, que se leían con PowerShell en cada arranque. Ahora se calculan en segundo plano nada más abrir y se recuerdan.",
      "Las cifras en vivo del Panel se leían en el hilo de la ventana; ahora van aparte y no la frenan.",
      "La lista de impresoras se lee directamente de Windows: unos 30 ms en lugar de casi 2 s.",
      "Los avisos de «tarea terminada» decían «Tarea» en casi todo; ahora dicen qué terminó (Diagnóstico, Borrado seguro, Copia cifrada…).",
      "Diagnóstico con menos ruido: con los mismos análisis reales, los hallazgos bajan de 66 a 41. Las apps que trae Windows ya no son un hallazgo (están en Bloatware), y los programas de inicio solo avisan si hay diez o más de terceros.",
      "Diagnóstico en tres niveles: «Urgente», «Conviene» y «Sugerencias de mejora», estas plegadas aparte (RAM en un canal, BIOS antigua…): no son problemas.",
      "«Ya lo sé»: un hallazgo que ya conoces se acepta con su motivo y deja de contar en ese equipo (diagnóstico, Panel y barra de arriba). Sigue en el informe y se deshace con «Volver a avisar».",
      "Análisis rápido, para una primera mirada en segundos, y progreso por partes: qué se está leyendo y cuánto tarda cada una.",
      "Hallazgos nuevos: Windows instalado en un disco mecánico, Windows sin soporte de Microsoft, memoria que se queda corta (según el historial de rendimiento), disco que se está llenando (comparando con análisis anteriores) y errores de disco apuntados por Windows.",
      "Las tarjetas de Discos y de Sistema y seguridad del diagnóstico son ahora un resumen con enlace a su pantalla, en vez de repetirla.",
      "De los análisis guardados de días anteriores queda el último de cada día (un equipo llegó a acumular 38 en pocos días).",
      "Diagnóstico, revisado con 51 análisis reales de 9 equipos. El teclado y el ratón PS/2 ya no salen como «faltan drivers» en equipos con teclado USB (era un puerto vacío). Un equipo sin driver gráfico sale como aviso, no como «driver antiguo». El aviso de arranque lento mira lo habitual de los últimos arranques, no solo el último.",
      "Diagnóstico: los programas con actualización pendiente se añaden solos al análisis cuando termina la búsqueda (antes el análisis se guardaba sin ellos y así salía en el informe).",
      "Diagnóstico: nuevo bloque «No se pudo comprobar», que dice qué quedó sin mirar y por qué: temperaturas sin su driver, discos que no dan SMART, falta de administrador o Windows que no respondió. Los errores de Windows salen explicados, y uno pasajero se reintenta.",
      "«Sistema y seguridad» ya no se pierde entero si Windows tarda en dar las actualizaciones instaladas: lo básico se lee aparte.",
      "Una sola pantalla para pantallazos y arranques: «Arranques y cuelgues» dice ahora el driver probable de cada pantallazo (leyendo los volcados), y la tarjeta Estabilidad del diagnóstico enlaza con ella en lugar de repetirlo.",
      "Seguridad: «Ejecución automática de USB» avisaba en todos los equipos por algo que Windows ya no hace; ahora solo avisa si alguien la abrió. Un sobremesa sin BitLocker es un dato, no un aviso que baja la nota; en un portátil sigue siendo un aviso.",
      "AdminOps ya no se cuenta a sí misma entre los programas que se cierran solos.",
      "Actualizar con AdminOps abierta podía fallar y pedir reiniciar el equipo. Ahora AdminOps se cierra sola en cuanto se abre el instalador, y el instalador pregunta a Windows qué programa tiene ocupados los archivos y lo cierra; si no puede, dice cuál es.",
    ],
    removed: [
      "La franja de «Modo de solo lectura»: lo dice ahora la barra de arriba («Solo lectura», que al pulsarlo reinicia como administrador), y cada pantalla avisa de lo que necesita.",
      "Opciones de la barra lateral que ya no tienen sentido con la nueva: secciones desplegadas, pestañas bajo el título, iconos de las secciones y el pie con el usuario. Los favoritos pasan a ser «Fijados».",
    ],
  },
  {
    version: "1.1.11",
    title: "Menú más claro, Ajustes que se guardan solos y un repaso visual completo",
    added: [
      "Instalar programas: vista «Empresa», que deja fuera juegos y programas de uso personal (Telegram, Discord, Spotify…), y vista «Todo».",
      "«Personalizar» el catálogo: ocultar programas sueltos o categorías enteras. Lo oculto sigue saliendo si lo buscas por su nombre.",
      "34 programas de empresa más: Microsoft 365 Apps, Firefox ESR, PDF24, PDF-XChange, NAPS2, Power BI, OpenVPN, Citrix Workspace, Horizon, Veeam, KeePass, SQL Server Management Studio y otros.",
      "Categorías nuevas (VPN y escritorios de la empresa, Copias de seguridad), filtro por categoría con sus cantidades, y «Ocultar los ya instalados».",
      "Lista «Puesto de empresa» para dejar un puesto de oficina listo.",
      "Menú con nombres que no se confunden: «Optimizar Windows» (antes Ajustes de Windows), «Usuarios y cuentas» (usuarios de este equipo, cuentas y dominio en una sola página), «Herramientas de Windows», «Herramientas de red» e «Historial del equipo». Preparar equipos pasa a Administración.",
      "Los Ajustes se guardan solos al cambiarlos; abajo se ve «Guardado». Ya no hay botón «Guardar ajustes» ni cambios que se pierden al salir.",
      "Ctrl+K también busca en la guía y abre el apartado que lo explica.",
      "Al abrir una versión nueva por primera vez se enseñan sus novedades (una sola vez).",
      "Aviso común de «requiere administrador» con el botón «Reiniciar como administrador», en las pantallas que lo necesitan.",
      "Impresoras, Desinstalar, Inicio de Windows y la evolución entre visitas de un cliente son tablas que se ordenan pulsando la cabecera de cada columna.",
      "Carpetas compartidas recuerda lo que ocupa cada carpeta (y dice cuándo se midió): ya no vuelve a contar los archivos cada vez que se abre.",
      "Si todavía no hay ninguna versión publicada, «Buscar actualizaciones» lo dice tal cual en lugar de dar un error.",
    ],
    fixed: [
      "Los diálogos abiertos desde una tarjeta (la copia cifrada en Ajustes, entre otros) salían recortados por el borde de la tarjeta. Ahora se abren sobre la ventana entera.",
      "En Usuarios, Acceso remoto y Personas, los diálogos podían quedar descolocados al desplazar la página.",
      "Los menús desplegables dentro de una tarjeta (el selector de icono en Navegación) ya no se cortan.",
      "Con una confirmación abierta sobre un diálogo, Escape cierra solo la confirmación.",
      "Los diálogos se pueden usar con el teclado de principio a fin: el foco entra en ellos, Tab no se escapa a la página de detrás y, al cerrar, vuelve al botón que los abrió. Los lectores de pantalla los anuncian con su título.",
      "Acceso remoto ya no comprueba todos los equipos de la agenda al abrirse (con muchos apagados llenaba la red de intentos): se comprueban al pulsar el botón, y lo comprobado se recuerda mientras AdminOps esté abierta.",
      "Las soluciones de la biblioteca indicaban rutas del menú que ya no existían; ahora llevan los nombres de hoy, y una prueba avisa si vuelven a quedarse atrás.",
      "Los ejemplos de dominio y correo usan empresa.com en lugar de un dominio real.",
      "Fallos en lecturas de fondo que antes se perdían en silencio quedan anotados en el registro técnico (va en el paquete de soporte), y las acciones que fallaban sin decir nada ahora avisan.",
      "Mismo aspecto en toda la app: anchos de página iguales, los mismos botones, las fechas escritas de una sola manera («hace 3 h», «ayer», «3 oct») y el Panel con las tarjetas del resto.",
      "Pantallas que se adaptan cuando la ventana es estrecha o está a media pantalla: buscadores, columnas y el Panel ya no se salen.",
      "En Inicio de Windows, las rutas de los programas ya no enseñan el nombre del usuario de Windows.",
      "GIMP aparecía dos veces en el catálogo; queda la versión 3.",
    ],
  },
  {
    version: "1.1.10",
    title: "Guía, novedades, términos de uso y reportar fallos",
    added: [
      "Guía de AdminOps (el libro de «Acerca de»): qué hace cada pantalla, cómo se usa, qué se puede deshacer y qué no, con glosario y preguntas frecuentes.",
      "Novedades: esta lista, con lo que trae cada versión desde la primera.",
      "Términos de uso, en el instalador y en «Acerca de».",
      "Reportar un problema: describes lo que pasó y AdminOps prepara el correo para el autor con el archivo de diagnóstico adjunto.",
      "Actualizar desde la propia aplicación: Ajustes → General descarga la versión nueva y abre su instalador.",
      "Plantilla de informe propia: eliges qué secciones lleva el PDF y en qué orden.",
      "Exportar el diario de cambios de un equipo a PDF o a Excel.",
      "Inventario, una página propia en Soporte con tu inventario y la web de inventario de la empresa.",
    ],
    fixed: [
      "Tickets e Inventario web navegan con libertad, como un navegador: la dirección guardada es solo la página de inicio.",
      "Barra lateral más corta: Red y Datos pasan a Equipo, y Soporte empieza por la Agenda.",
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
