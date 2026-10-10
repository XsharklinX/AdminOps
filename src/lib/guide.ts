// La guía de AdminOps: qué hace cada pantalla, cómo se usa y qué hay que saber
// antes de tocar. Está escrita para quien no la conoce. Lo que aquí se dice que
// se puede deshacer es lo que el programa deshace de verdad: si una pantalla
// cambia, su apartado se actualiza con ella.
import { GLOSSARY as TECH_GLOSSARY } from "./glossary";
import type { PageId } from "../components/Sidebar";

export interface GuideTopic {
  id: string;
  title: string;
  /** Qué es y para qué sirve. */
  what: string;
  /** Cómo se usa, paso a paso. */
  how?: string[];
  /** Lo que conviene saber: avisos, límites, lo que no se deshace. */
  notes?: string[];
  /** Página (y pestaña) a la que lleva «Abrir esta pantalla». */
  go?: [PageId, string?];
}

export interface GuideChapter {
  id: string;
  title: string;
  intro: string;
  topics: GuideTopic[];
}

export const GUIDE: GuideChapter[] = [
  {
    id: "start",
    title: "Antes de empezar",
    intro: "Lo mínimo para moverse por AdminOps con seguridad.",
    topics: [
      {
        id: "what-is",
        title: "Qué es AdminOps",
        what: "Un programa para dar soporte técnico a equipos con Windows 10 y 11. Junta en una ventana lo que normalmente se reparte entre muchas herramientas: ver cómo está el equipo, encontrar lo que falla, arreglarlo, y dejar por escrito lo que se hizo.",
        notes: [
          "No sustituye a Windows: usa sus propios mecanismos y los explica. Casi todo lo que AdminOps hace se puede comprobar después en Windows.",
          "Trabaja sobre el equipo en el que está abierto. No vigila ni controla otros equipos a distancia.",
        ],
      },
      {
        id: "admin",
        title: "Abrirlo como administrador",
        what: "Sin permisos de administrador AdminOps puede leer casi todo, pero no cambiar el sistema. Cuando falta el permiso, arriba aparece un aviso con el botón «Reiniciar como admin», y los botones que lo necesitan lo dicen al pasar el ratón.",
        how: ["Pulsa «Reiniciar como admin» en el aviso de arriba, o busca «administrador» con Ctrl+K.", "Windows pedirá confirmación (o la contraseña de un administrador). Es el aviso normal de Windows."],
        notes: ["Si entras con la cuenta de administrador de otra persona, los ajustes que son «del usuario» se aplican al usuario que tiene la sesión abierta, no al administrador."],
      },
      {
        id: "modes",
        title: "Modo técnico y modo usuario",
        what: "El modo técnico enseña todo. El modo usuario enseña solo el estado del equipo, la solución de problemas, los discos y el acceso remoto: es para dejar AdminOps en el equipo de alguien que no es técnico.",
        how: ["Ajustes → Inicio y ventana → «Para quién es AdminOps en este equipo».", "Para volver al modo técnico desde el modo usuario se pide el PIN, si hay uno puesto."],
        notes: ["Es un modo de interfaz: esconde opciones para no confundir. No es una barrera de seguridad; alguien con conocimientos puede cambiarlo."],
        go: ["settings", "start"],
      },
      {
        id: "portable",
        title: "Instalado o en un pendrive",
        what: "Instalado, AdminOps guarda sus datos en la carpeta del usuario de Windows. En un pendrive (o con la versión portable) los guarda junto al programa, y viajan contigo: clientes, contactos, agenda, ajustes y contraseñas guardadas.",
        notes: [
          "En el pendrive, lo que es de cada equipo (su diario de cambios, sus diagnósticos) se guarda separado por nombre de equipo.",
          "En el pendrive no queda nada en el equipo del cliente al irte, salvo los cambios que hayas hecho en él.",
          "Pon un PIN si llevas AdminOps en un pendrive: protege la aplicación y también las contraseñas guardadas. Si olvidas el PIN, esas contraseñas no se pueden recuperar.",
        ],
        go: ["settings", "data"],
      },
      {
        id: "moving",
        title: "Moverse por la aplicación",
        what: "A la izquierda, la columna de áreas (Inicio, Este equipo, Red, Programas, Administración, Soporte). Al lado, las pantallas del área elegida y, debajo de cada una, sus secciones: lo que hay dentro se ve sin abrir nada. Arriba, el equipo en una línea: nombre, dominio, permisos, Internet, disco y avisos; cada dato lleva a donde se mira o se arregla.",
        how: [
          "«Todo» (arriba en la columna de áreas, o Ctrl+K o F1) enseña el programa entero en una pantalla, por áreas. Escribiendo, busca cualquier cosa: una sección («dominio», «AD»), un problema («no imprime»), una herramienta, un ajuste o un contacto.",
          "Encima del título se ve dónde estás: área › pantalla › sección. Cada parte se puede pulsar.",
          "La chincheta, al pasar el ratón por una línea de la barra, la fija arriba (también una sección, como Dominio).",
          "Al lado de algunas secciones sale su estado: los avisos del último diagnóstico, si el equipo está en el dominio, el espacio que queda.",
          "Teams y Correo se abren desde sus iconos de la barra de arriba. Herramientas de Windows y Ajustes están abajo en la columna de áreas.",
          "Alt+← y Alt+→ vuelven a la pantalla anterior y a la siguiente. Ctrl+, abre Ajustes. Ctrl+L bloquea AdminOps si hay PIN.",
        ],
        notes: ["La barra lateral se puede reorganizar entera en Ajustes → Navegación, y dejar solo la columna de áreas para ganar espacio."],
      },
      {
        id: "first-steps",
        title: "Los primeros cinco minutos",
        what: "Lo que conviene dejar puesto la primera vez. El Panel lo recuerda en «Primeros pasos» hasta que esté hecho.",
        how: [
          "Tus datos y los de tu empresa (Ajustes → Informes y cobros): salen en los informes.",
          "Los portales que uses: tickets, correo, Teams (Ajustes → Portales y correo).",
          "Un PIN de bloqueo (Ajustes → Seguridad), sobre todo en un pendrive.",
          "La copia automática de tus datos (Ajustes → Datos y copias).",
        ],
        go: ["dashboard"],
      },
    ],
  },
  {
    id: "safety",
    title: "Cómo cuida AdminOps el equipo",
    intro: "Las reglas con las que trabaja el programa. Conocerlas evita sustos.",
    topics: [
      {
        id: "journal",
        title: "El diario y «Deshacer»",
        what: "Cada cambio que AdminOps hace en el equipo se anota en el diario con el valor que había antes. Por eso un ajuste se puede deshacer días después y queda exactamente como estaba.",
        how: [
          "Estado del equipo → Historial → Diario de cambios.",
          "Busca el cambio (hay buscador y filtros) y pulsa «Deshacer».",
          "El filtro «Se pueden deshacer» enseña solo lo que aún tiene vuelta atrás.",
          "«Exportar» guarda el diario en PDF o para Excel, como constancia de lo hecho. Con un filtro puesto, exporta solo lo filtrado.",
        ],
        notes: ["Las acciones (una limpieza, una reparación, una instalación) se anotan pero no se deshacen: lo borrado no vuelve.", "El diario es de cada equipo. En el pendrive hay uno por cada equipo en el que has trabajado."],
        go: ["history"],
      },
      {
        id: "restore-point",
        title: "Puntos de restauración",
        what: "Antes de aplicar un ajuste de riesgo medio o alto, AdminOps pide a Windows un punto de restauración: una foto del sistema a la que se puede volver si algo sale mal. Como mucho se crea uno cada 30 minutos.",
        how: ["Se crean solos. También puedes crear uno a mano en Historial → Puntos de restauración.", "Para volver a un punto: «Restaurar sistema», en esa misma tarjeta, abre la herramienta de Windows."],
        notes: ["Un punto de restauración recupera la configuración de Windows y los programas, no tus documentos.", "Necesita que «Protección del sistema» esté activada en Windows; AdminOps avisa si no lo está."],
        go: ["history"],
      },
      {
        id: "audit",
        title: "Modo auditoría (solo mirar)",
        what: "Con el modo auditoría activado, AdminOps no cambia nada en el equipo: cualquier acción que modifique algo se rechaza con un mensaje. Sirve para revisar un equipo ajeno sin riesgo, o para enseñar la aplicación.",
        how: ["Se activa y desactiva desde la barra de arriba.", "Dura mientras AdminOps está abierta: al volver a abrirla empieza desactivado."],
        notes: ["Lo que es de AdminOps (contactos, notas, informes, diagnósticos) sigue funcionando en este modo."],
      },
      {
        id: "irreversible",
        title: "Lo que no se puede deshacer",
        what: "La mayoría de los cambios tienen vuelta atrás, pero no todos. Estos no:",
        notes: [
          "Borrado seguro de archivos y del espacio libre: es justamente para que no se puedan recuperar.",
          "Formatear una unidad.",
          "Eliminar un usuario marcando «borrar también su carpeta de perfil».",
          "Desinstalar un programa (habría que volver a instalarlo) y quitar aplicaciones preinstaladas que no son de la Tienda.",
          "Limpiezas: temporales, cachés, papelera.",
          "Sacar un equipo del dominio sin tener a mano una cuenta local de administrador.",
          "AdminOps avisa antes de cada una y, en las más graves, pide escribir un nombre para confirmar.",
        ],
      },
      {
        id: "data",
        title: "Dónde están tus datos y tus contraseñas",
        what: "Todo lo que guardas se queda en el equipo o en el pendrive. No hay cuentas, ni servidor, ni telemetría. Las contraseñas que decides guardar (routers, portales) se cifran: instalado, con el mecanismo de Windows para tu usuario; en el pendrive, con una clave propia que el PIN protege.",
        how: ["Ajustes → Datos y copias → «Dónde se guardan tus datos» dice qué viaja contigo y qué es de cada equipo.", "Ajustes → Datos y copias → «Seguridad de tus datos» hace una copia cifrada de todo, para restaurarla en otro equipo."],
        notes: ["Las contraseñas de Escritorio remoto no las guarda AdminOps: las guarda Windows, en su Administrador de credenciales."],
        go: ["settings", "data"],
      },
    ],
  },
  {
    id: "home",
    title: "Inicio",
    intro: "Lo primero que se ve y el trabajo de una visita.",
    topics: [
      {
        id: "dashboard",
        title: "Panel",
        what: "El equipo en vivo: procesador, memoria, discos, red y temperaturas, con lo que conviene hacer ahora ordenado por importancia. Arriba, lo de hoy: visitas, seguimientos, avisos de Windows y el caso abierto.",
        how: ["«Revisar el equipo» lanza un diagnóstico.", "Las acciones rápidas y tus herramientas favoritas están a un clic.", "Si este equipo o esta red tienen notas tuyas, salen aquí al volver."],
        go: ["dashboard"],
      },
      {
        id: "troubleshoot",
        title: "Solucionar problemas",
        what: "Eliges el síntoma (no hay Internet, no suena, no imprime, va lento, Wi‑Fi, Bluetooth, pantalla, Windows Update) y AdminOps revisa en orden las causas típicas y ofrece la reparación de cada una.",
        how: ["Elige el síntoma.", "Lee lo que encontró: cada causa dice si está bien o mal.", "Pulsa la reparación que propone. Al terminar vuelve a comprobar."],
        notes: ["Debajo están las reparaciones de Windows: comprobar archivos del sistema (SFC), reparar la imagen (DISM), reiniciar la red, Windows Update, la cola de impresión. Son largas y se pueden cancelar."],
        go: ["troubleshoot"],
      },
      {
        id: "session",
        title: "Sesión de servicio",
        what: "Una visita registrada de principio a fin. Al empezar se guarda un diagnóstico del equipo tal como llega; al terminar, otro. El informe compara los dos y lleva lo que se hizo, el cobro y la firma.",
        how: [
          "Elige el cliente (o escribe uno nuevo), el tipo de visita y quién pidió el trabajo. «Iniciar sesión».",
          "Motivo: lo que cuenta el cliente.",
          "Trabajo: la lista de comprobación. Los puntos con varita se marcan solos al hacer esa tarea con AdminOps. Trabaja con normalidad en el resto de la aplicación.",
          "Informe: tus observaciones y recomendaciones.",
          "Cobro: presupuesto o recibo, con impuestos y descuento.",
          "Firma y cierre: garantía, próximo mantenimiento y firma del cliente en pantalla. «Finalizar y generar informe».",
        ],
        notes: ["Todo se guarda solo mientras escribes. Puedes cerrar AdminOps y seguir después.", "«Descartar» borra la sesión sin informe; los cambios hechos en el equipo se quedan."],
        go: ["session"],
      },
      {
        id: "report",
        title: "Informe",
        what: "El PDF que se entrega. Hay dos plantillas de fábrica: para el cliente (clara, sin tecnicismos) y técnica (con todo el detalle). Y la tuya: en Ajustes → Informes y cobros eliges qué secciones lleva y en qué orden. Lleva tu marca, el estado del equipo, lo resuelto, lo pendiente y, si quieres, presupuesto o recibo.",
        how: ["Sin sesión: Sesión de servicio → pestaña Informe. «Entrega rápida» pasa al informe lo hecho hoy.", "«Enviar por correo» abre tu programa de correo con el PDF adjunto. No se envía nada sin que tú lo envíes."],
        notes: ["Los informes se guardan en una carpeta por cliente; se abre desde la ficha del cliente o desde Acerca de."],
        go: ["report"],
      },
    ],
  },
  {
    id: "machine",
    title: "Este equipo",
    intro: "Cómo está este equipo y qué se le puede hacer.",
    topics: [
      {
        id: "diagnostics",
        title: "Diagnóstico",
        what: "Un repaso del equipo en menos de un minuto: salud de los discos, pantallazos azules y apagados bruscos, programas que fallan, drivers con error, batería, antivirus, actualizaciones, activación y arranque. Cada hallazgo va ordenado por gravedad y trae el botón que lo resuelve o lo explica.",
        how: [
          "«Analizar» hace el análisis completo. «Rápido» da una primera mirada en segundos (sin actualizaciones, SMART, piezas, temperaturas ni seguridad) y no se guarda. «A fondo» vuelve a leer también el hardware y las actualizaciones.",
          "Lo que encuentra va en tres niveles: «Urgente» (hay que hacerlo), «Conviene» (mejor arreglarlo) y «Sugerencias», plegadas aparte: mejoras posibles, no problemas.",
          "«Ya lo sé», al pasar el ratón por un hallazgo: si ya lo conoces y no vas a arreglarlo ahora (un disco pendiente de cambio), apúntale el motivo. Pasa a «Aceptados» y deja de contar en este equipo, también en el Panel y en la barra de arriba. Se deshace con «Volver a avisar».",
          "Mira «No se pudo comprobar», debajo de los hallazgos: dice qué quedó sin mirar y por qué (sin administrador, falta un driver para las temperaturas, discos que no dan SMART). Que no haya aviso de algo no significa que esté bien si no se pudo leer.",
          "Los análisis se guardan: se puede comparar el de hoy con uno anterior.",
        ],
        notes: [
          "Un diagnóstico es una ayuda, no un dictamen. Un disco «saludable» según sus propios datos puede fallar igualmente.",
          "Los programas con actualización pendiente pueden tardar más de un minuto en buscarse la primera vez: se añaden solos al análisis cuando llegan.",
          "El detalle de pantallazos azules, apagones y arranques está en «Arranques y cuelgues»; el de discos, en Discos; y cada punto de la nota, en Seguridad. Aquí va un resumen con su enlace.",
          "Un hallazgo aceptado sigue saliendo en el informe del cliente: aceptarlo es una nota tuya, no un arreglo.",
          "De los análisis de días anteriores se guarda el último de cada día.",
        ],
        go: ["diagnostics"],
      },
      {
        id: "hardware",
        title: "Hardware",
        what: "Las piezas del equipo: placa, BIOS, procesador, memoria por ranura, gráfica, monitores y discos, con las temperaturas en vivo. Incluye la ficha del equipo (modelo, número de serie, licencia) para copiarla o pegarla en Excel.",
        notes: ["Las temperaturas necesitan administrador. Las lee LibreHardwareMonitor, un componente de código abierto que viene con AdminOps."],
        go: ["hardware"],
      },
      {
        id: "performance",
        title: "Rendimiento (7 días)",
        what: "Procesador, memoria y disco del sistema minuto a minuto, de las últimas 6 horas, 24 horas o 7 días, y qué programa había detrás de cada pico. Para «va lento desde el martes»: se ve cuándo empezó y qué coincidía.",
        how: ["Elige el tramo arriba a la derecha.", "Pasa el ratón por la gráfica: dice la hora, las cifras y el programa que más gastaba.", "«Qué había detrás de los picos» resume los programas que más aparecen cuando el procesador pasa del 60 %."],
        notes: ["Se mide solo mientras AdminOps está abierta: los huecos de la gráfica son ratos con AdminOps cerrada. Se guarda una semana, en un archivo pequeño de este equipo."],
        go: ["machine", "performance"],
      },
      {
        id: "boots",
        title: "Arranques y cuelgues",
        what: "Lo que dice el registro de Windows de los últimos 60 días: cuántas veces arrancó, cuántas se apagó mal (corte de luz, botón, cuelgue), los pantallazos azules con su código explicado y qué mirar, y cuánto tarda en arrancar.",
        notes: [
          "Cuánto tarda cada arranque solo se puede leer como administrador; lo demás, sin él.",
          "Como administrador, AdminOps lee los volcados de Windows y dice qué driver estaba probablemente detrás de cada pantallazo. Es una pista para empezar, no un veredicto.",
          "El aviso de arranque lento del diagnóstico mira lo habitual de los últimos arranques, no solo el último.",
        ],
        go: ["machine", "boots"],
      },
      {
        id: "peripherals",
        title: "Probar periféricos",
        what: "Pantalla de un color para ver píxeles muertos, teclado que marca cada tecla que responde, pitido por el altavoz izquierdo y el derecho, nivel del micrófono y la imagen de la cámara.",
        notes: ["El micrófono y la cámara se ven en vivo y se apagan al salir de la pestaña; no se graba nada. Si Windows no deja usarlos, se dice dónde permitirlo (Configuración → Privacidad y seguridad)."],
        go: ["machine", "peripherals"],
      },
      {
        id: "security",
        title: "Seguridad",
        what: "Una nota de 0 a 100 con cada punto explicado: antivirus, cortafuegos, control de cuentas, cifrado del disco, protocolos antiguos, escritorio remoto, cuentas y programas de riesgo sin actualizar. Cada punto flojo tiene su botón.",
        how: ["Revisa los puntos en rojo y ámbar.", "BitLocker: desde aquí se guardan las claves de recuperación en un pendrive. Hazlo antes de tocar nada del arranque."],
        notes: ["«Elementos sospechosos» señala tareas, servicios y programas de inicio con rasgos típicos de software no deseado. Es una pista para mirar, no una condena."],
        go: ["security"],
      },
      {
        id: "history",
        title: "Historial",
        what: "Todo lo que ha pasado en el equipo: la línea de tiempo (cambios, avisos, diagnósticos, actualizaciones, apagados bruscos), los puntos de restauración, el diario de cambios con su «Deshacer» y el registro técnico de AdminOps.",
        go: ["history"],
      },
      {
        id: "processes",
        title: "Procesos",
        what: "Qué está usando procesador, memoria y disco ahora mismo. Agrupado por programa: un navegador cuenta como uno, con la suma de todos sus procesos.",
        how: ["Arriba, los dos que más pesan ahora: púlsalos para ir a ellos.", "Pulsa un programa para ver sus datos; la flecha enseña sus procesos.", "«Finalizar el programa» cierra todos sus procesos. «Pausar» congela la lista para elegir con calma."],
        notes: ["Finalizar un programa pierde lo que no esté guardado en él.", "Los procesos de Windows marcados con candado no se pueden finalizar desde aquí: cerrarlos puede colgar el equipo."],
        go: ["processes"],
      },
      {
        id: "space",
        title: "Discos: espacio",
        what: "Qué ocupa el disco, carpeta por carpeta y por tipo de archivo (vídeo, correo guardado, instaladores…), con los archivos más grandes y los grandes que nadie toca hace más de un año. Y lo que se puede liberar sin riesgo.",
        how: [
          "Elige la unidad y «Analizar» (o doble clic en la unidad).",
          "En «Carpetas», el mapa enseña cada carpeta como un rectángulo de su tamaño. Un clic la señala y enseña su ficha: cuánto ocupa, qué parte es de la carpeta y del total, cuántos archivos y cuándo cambió por última vez. Doble clic entra.",
          "La barra de colores dice de qué está llena la carpeta en la que estás. Pulsa un color para ver los archivos más grandes de ese tipo.",
          "«Archivos» tiene cuatro listas: los más grandes, los que nadie toca hace un año, los de la carpeta en la que estás y los del tipo elegido. Se filtran por nombre o extensión.",
          "Marca carpetas y archivos de cualquier lista: abajo sale lo marcado y cuánto suma. «A la papelera» lo manda todo de una vez, y las cifras se ponen al día sin volver a analizar.",
          "«Archivos duplicados» busca el mismo archivo guardado varias veces. De cada grupo, «Marcar las copias» deja la primera y marca las demás; mira antes dónde está cada una.",
          "«Liberar lo verde», en «Qué puedes liberar», vacía de una vez los sitios seguros.",
        ],
        notes: [
          "Desde aquí no se pueden borrar Windows, un programa instalado entero ni un perfil de usuario: cada cosa se quita desde su sitio (Limpieza, Programas, Usuarios).",
          "Lo que sea demasiado grande para la papelera, Windows lo borra del todo.",
          "Los duplicados de más de 256 MB se comparan por muestras (principio, final y dieciséis trozos), no enteros: lo dice cada grupo.",
        ],
        go: ["space", "space"],
      },
      {
        id: "disk-health",
        title: "Discos: salud y reparación",
        what: "El estado de cada disco dicho en claro (bien, vigilar, cambiar ya), su evolución entre lecturas, si está cifrado, y las reparaciones de Windows: reparar el sistema de archivos y buscar sectores dañados. Para un disco que falla, «Rescatar archivos» copia lo que se pueda leer a otro disco.",
        how: ["Si el veredicto es «cambiar ya»: primero rescata los datos, después repara.", "«Reparar disco» puede pedir reiniciar si es el disco de Windows.", "Las herramientas de pendrive miden su velocidad real y comprueban que la capacidad no es falsa."],
        notes: ["La prueba de capacidad y el formateo borran lo que haya en la unidad.", "Buscar sectores dañados en un disco grande puede tardar horas. Se puede cancelar."],
        go: ["space", "health"],
      },
      {
        id: "tweaks",
        title: "Optimizar Windows",
        what: "Limpieza, rendimiento, privacidad, servicios e inicio de Windows en un solo sitio, con buscador. Cada ajuste dice qué hace, qué riesgo tiene y cómo está ahora en el equipo.",
        how: ["Busca o recorre la categoría.", "Lee la explicación y el nivel de riesgo antes de aplicar.", "«Aplicar» lo cambia; «Deshacer» lo devuelve a como estaba."],
        notes: [
          "Limpieza borra archivos: eso no se deshace.",
          "Inicio de Windows activa o desactiva lo que arranca con el equipo, igual que el Administrador de tareas: no borra nada.",
          "Si no sabes para qué sirve un servicio, no lo desactives.",
        ],
        go: ["tweaks"],
      },
    ],
  },
  {
    id: "support",
    title: "Soporte",
    intro: "El trabajo diario con personas, clientes y tickets.",
    topics: [
      {
        id: "tickets",
        title: "Tickets",
        what: "La web de tickets o la intranet de tu empresa dentro de AdminOps, con la sesión recordada. Funciona como un navegador: la dirección guardada es solo la página de inicio, y desde ella puedes ir a cualquier otra.",
        how: [
          "«Añadir portal»: nombre y dirección. Puedes guardar varios y cambiar de uno a otro.",
          "La barra de direcciones admite cualquier dirección; atrás, adelante, recargar e inicio funcionan como en un navegador.",
          "«Entrar solo» rellena el usuario y la contraseña guardados; en las páginas normales no envía el formulario, lo deja listo.",
        ],
        notes: ["Las páginas de un portal no tienen acceso a AdminOps ni al equipo: están aisladas.", "«Sesión privada» no guarda nada en el equipo y cierra la sesión al salir. Úsala en equipos ajenos."],
        go: ["tickets"],
      },
      {
        id: "mail-teams",
        title: "Correo y Teams",
        what: "Outlook y Teams en su versión web, dentro de AdminOps: no hay que instalarlos ni configurarlos en el equipo del cliente. Sirven la cuenta del trabajo y la personal. Se abren desde sus iconos de la barra de arriba, estés donde estés.",
        how: ["La primera vez se pregunta cómo abrirlos: dentro de AdminOps, en el navegador o en la aplicación de Windows (si está instalada).", "Se cambia en Ajustes → Portales y correo → «Cómo se abren Teams y el Correo»."],
        notes: ["Para hablar en una reunión, Windows pide permiso de micrófono y cámara la primera vez.", "Si tardan en abrir, es la web de Microsoft cargando: AdminOps lo dice en una barra y ofrece recargar."],
        go: ["mail"],
      },
      {
        id: "inventory",
        title: "Inventario",
        what: "Todo el inventario en un sitio. «Mi inventario»: el parque de equipos con la ficha de cada uno y un veredicto (seguir, mejorar, renovar), exportable a Excel. «Inventario web»: la web de inventario de tu empresa dentro de AdminOps, con los datos de este equipo a mano para rellenar el formulario.",
        notes: ["La web de inventario navega con libertad, como Tickets."],
        go: ["inventory"],
      },
      {
        id: "people",
        title: "Personas",
        what: "La ficha de alguien del dominio de la empresa: si su cuenta está bloqueada, desactivada o con la contraseña caducada, en qué equipo está, y los casos que ya tuvo. Desde ahí se desbloquea o se le da una contraseña temporal.",
        how: ["Busca por nombre, usuario, correo o extensión.", "Si la cuenta tiene un problema, sale arriba con el botón que lo arregla.", "Las últimas personas abiertas quedan en «Recientes»."],
        notes: ["Funciona con tus propios permisos en el dominio: puedes ver y cambiar lo mismo que en la consola de Active Directory, ni más ni menos.", "Solo tiene sentido en un equipo unido a un dominio."],
        go: ["people", "people"],
      },
      {
        id: "clients",
        title: "Clientes",
        what: "Las fichas de tus clientes: sus datos, sus equipos, las visitas con sus informes, las garantías vigentes y cuándo les toca mantenimiento.",
        how: ["Las cifras de arriba resumen; «Mantenimiento cerca o vencido» filtra la lista.", "En la ficha: «Empezar sesión» abre una sesión de servicio con ese cliente; «Agendar» apunta una visita."],
        go: ["people", "clients"],
      },
      {
        id: "agenda",
        title: "Agenda",
        what: "Visitas, tareas, llamadas y reuniones, con cliente o sin él. Tres vistas: lista (hoy, mañana, la semana), mes e historial. Windows avisa 30 minutos antes.",
        how: [
          "La línea de arriba apunta algo rápido escribiéndolo.",
          "En la vista de mes, arrastra una cosa a otro día para moverla.",
          "«Atrasado» enseña lo que pasó sin marcarse: márcalo como hecho, pásalo a hoy o cancélalo.",
          "Historial: lo hecho y lo cancelado, por meses, con «Repetir».",
        ],
        go: ["agenda"],
      },
      {
        id: "contacts",
        title: "Contactos",
        what: "A quién llamar y para qué: extensiones, correos, quién sustituye a quién. Viaja contigo en el pendrive.",
        how: ["Busca con prefijos: «ext:», «empresa:», «#etiqueta».", "Importa desde Excel (CSV) o vCard; exporta a los mismos formatos.", "Hay papelera de 30 días y copias automáticas."],
        go: ["contacts"],
      },
      {
        id: "knowledge",
        title: "Soluciones",
        what: "Qué hacer ante cada problema: soluciones paso a paso (las que trae AdminOps y las tuyas), plantillas de texto para responder al cliente, y notas de cada equipo y de cada red.",
        how: ["Al cerrar una sesión de servicio puedes guardar lo que funcionó como solución.", "Las plantillas rellenan solas el nombre del equipo, el usuario, la fecha y tu nombre."],
        go: ["knowledge"],
      },
    ],
  },
  {
    id: "network",
    title: "Red",
    intro: "Todo lo de la red, en una pantalla con cuatro secciones. Tiene su propia área: Red.",
    topics: [
      {
        id: "router",
        title: "Red y router",
        what: "La red en la que está el equipo: dirección, puerta de enlace, DNS e IP pública. El panel del router se abre dentro de AdminOps, con su acceso guardado y cifrado. Comprueba la seguridad del router y de la Wi‑Fi y genera un código QR para conectar un móvil.",
        notes: ["Los routers usan un certificado propio que los navegadores no reconocen. AdminOps lo acepta solo para direcciones de la red local."],
        go: ["router", "router"],
      },
      {
        id: "devices",
        title: "Dispositivos",
        what: "Todo lo que está conectado a la red: IP, MAC, fabricante y, cuando se puede saber, tipo y modelo. Puedes ponerles nombre, apuntar su función y su responsable, y marcar los que importan para que AdminOps avise si dejan de responder.",
        how: ["«Buscar» recorre la red.", "Los nuevos desde la última búsqueda salen marcados.", "Ordena por cualquier columna; las IP se ordenan como números."],
        notes: ["Los móviles modernos usan una MAC distinta en cada red: por eso a veces no se sabe el fabricante.", "Busca solo en la red local en la que estás. No explora otras redes."],
        go: ["router", "devices"],
      },
      {
        id: "speed",
        title: "Velocidad y diagnóstico",
        what: "Prueba de velocidad (bajada, subida y latencia) con su historial, diagnóstico de la conexión, «Reparar la red» con la comprobación antes y después, y el estado de la tarjeta Wi‑Fi.",
        notes: ["«Reparar la red a fondo» reinicia la configuración de red de Windows y puede pedir reiniciar el equipo. Las redes Wi‑Fi guardadas se conservan."],
        go: ["router", "speed"],
      },
      {
        id: "netwatch",
        title: "Vigilante de la conexión",
        what: "Para «se me corta Internet a ratos». Se deja en marcha y cada 5 segundos hace un ping al router y a Internet; apunta cada corte con su hora, cuánto duró y de quién era la culpa: del router (no contestaba ni él) o de Internet (el router sí, fuera no).",
        how: ["«Poner en marcha» y deja AdminOps abierta el tiempo que haga falta (minimizada vale).", "La gráfica enseña la última hora y media, con los cortes en rojo.", "«Copiar resumen» deja un texto con todos los cortes listo para el proveedor de Internet."],
        notes: ["Un ping perdido suelto no cuenta como corte: hacen falta dos comprobaciones fallidas seguidas.", "Los cortes se guardan aunque se cierre AdminOps; «Borrar» los quita."],
        go: ["router", "watch"],
      },
      {
        id: "nettools",
        title: "Herramientas de red",
        what: "Ping y traza de ruta en vivo, puertos abiertos por programa, cambio de DNS con un clic, editor del archivo hosts y calculadora de red (rango, máscara, cuántos equipos caben y si dos IP están en la misma red).",
        notes: ["Un cambio de DNS se puede deshacer desde el diario."],
        go: ["router", "tools"],
      },
    ],
  },
  {
    id: "apps",
    title: "Programas",
    intro: "Los programas del equipo y cómo dejar uno nuevo listo.",
    topics: [
      {
        id: "update",
        title: "Actualizar programas y Windows",
        what: "Programas con versión nueva (usa winget, la herramienta de Windows) para actualizarlos en lote, y Windows Update: qué falta, el historial con cada error explicado, pausar y ocultar una actualización que da problemas.",
        go: ["apps", "update"],
      },
      {
        id: "install",
        title: "Instalar en lote",
        what: "Tras formatear: marcas los programas de un catálogo comprobado, o una lista tuya guardada, y se instalan todos solos.",
        how: [
          "«Empresa» enseña solo lo que se usa en una oficina; «Todo» añade juegos y programas de uso personal.",
          "«Personalizar» deja ocultar los programas o las categorías que no usas. Se guarda y viaja con tus datos.",
          "Marca los programas o carga una lista. Las categorías de arriba filtran.",
          "«Instalar». Se ve el progreso de cada uno y se puede cancelar.",
        ],
        notes: ["Necesita Internet y winget, que viene con Windows 11 y con las versiones recientes de Windows 10."],
        go: ["apps", "install"],
      },
      {
        id: "uninstall",
        title: "Desinstalar y quitar preinstaladas",
        what: "Desinstala uno o varios programas, en silencio cuando se puede, y manda a la papelera lo que dejan atrás. «Bloatware» lista las aplicaciones preinstaladas con una recomendación para cada una.",
        notes: ["Desinstalar no se deshace. Las aplicaciones de la Tienda se pueden volver a instalar desde ella."],
        go: ["apps", "uninstall"],
      },
      {
        id: "recipes",
        title: "Preparar equipos",
        what: "Plantillas: todo lo que haces a un equipo nuevo (quitar preinstaladas, instalar programas, aplicar ajustes, crear el usuario, ponerle nombre, unirlo al dominio), de una vez y siempre igual. Perfiles: grupos de ajustes que se aplican juntos (Oficina, Gaming, Equipo viejo, Privacidad máxima, o los tuyos).",
        notes: ["Las contraseñas se piden al ejecutar la plantilla y nunca se guardan.", "Un perfil se puede deshacer entero o ajuste por ajuste."],
        go: ["recipes"],
      },
    ],
  },
  {
    id: "admin",
    title: "Administración",
    intro: "Usuarios, cuentas, lo que la oficina comparte y cómo llegar a otros equipos.",
    topics: [
      {
        id: "users",
        title: "Usuarios y cuentas: usuarios de este equipo",
        what: "Las cuentas de este equipo: crear, cambiar la contraseña, hacer administrador o estándar, desactivar, renombrar y eliminar. La ficha de cada cuenta avisa de lo que conviene revisar: contraseña caducada, administrador sin contraseña, cuentas integradas activas o cuentas que nadie usa.",
        notes: [
          "AdminOps no deja el equipo sin un administrador activo, ni permite borrar la cuenta que tiene la sesión abierta.",
          "Cambiar la contraseña de otro usuario le hace perder sus archivos cifrados con EFS y las contraseñas que Windows le tenía guardadas.",
          "Al renombrar una cuenta, su carpeta personal conserva el nombre antiguo. Es normal; no la renombres a mano.",
        ],
        go: ["users", "users"],
      },
      {
        id: "accounts",
        title: "Usuarios y cuentas: cuentas y dominio",
        what: "Con qué cuentas entra este equipo (Microsoft, profesionales, de Office) y las credenciales que Windows recuerda. Y el dominio de la empresa: estado, unir, sacar, reparar la relación de confianza y cambiar el nombre del equipo.",
        notes: ["Antes de sacar un equipo del dominio, comprueba que hay una cuenta local de administrador con contraseña conocida: si no, nadie podrá entrar.", "Borrar una credencial guardada arregla el típico «pide la contraseña una y otra vez»."],
        go: ["users", "accounts"],
      },
      {
        id: "printers",
        title: "Impresoras",
        what: "Impresoras instaladas con su estado, la cola de impresión (vaciarla cuando se atasca), página de prueba, elegir la predeterminada y quitar las que sobran.",
        go: ["printers", "printers"],
      },
      {
        id: "shares",
        title: "Carpetas compartidas",
        what: "Lo que este equipo comparte en la red: quién puede entrar y con qué permiso, cuánto ocupa cada carpeta y quién tiene archivos abiertos. Y el otro lado: las unidades de red de este equipo y lo que comparten otros.",
        how: [
          "«Quién puede entrar» cambia permisos sin volver a compartir.",
          "«¿Por qué no puede entrar?»: eliges carpeta y persona y dice qué se lo impide.",
          "El icono de mensaje copia las instrucciones para mandárselas a quien tiene que entrar.",
          "«Copia diaria» programa una copia de la carpeta a otro disco.",
        ],
        notes: [
          "El acceso real es lo que permitan la compartición y los permisos del disco a la vez: manda lo más restrictivo.",
          "La copia diaria nunca borra nada en la copia. No sustituye a una copia de seguridad fuera del equipo.",
          "AdminOps no guarda contraseñas de otros equipos: si las piden, las pide y las guarda Windows.",
        ],
        go: ["printers", "shares"],
      },
      {
        id: "remote",
        title: "Acceso remoto",
        what: "Tu agenda de equipos para conectarte: Escritorio remoto con sus opciones, o el identificador de AnyDesk, RustDesk o TeamViewer. Cada ficha dice si el equipo contesta. También enciende equipos por la red.",
        notes: ["Para recibir Escritorio remoto, el equipo necesita Windows Pro; Windows Home solo puede conectarse a otros.", "No abras el puerto de Escritorio remoto del router a Internet."],
        go: ["remote"],
      },
      {
        id: "stations",
        title: "Puestos",
        what: "Tus listas de equipos de la oficina, para comprobar de una vez cuáles responden y cuáles necesitan atención (disco, reinicios, actualizaciones).",
        go: ["stations"],
      },
      {
        id: "tools",
        title: "Herramientas de Windows",
        what: "Las herramientas de Windows de siempre a un clic (servicios, conexiones de red, visor de eventos, registro, BIOS…), tus propios accesos directos, y los atajos de teclado de Windows y de los programas habituales. Está abajo en la columna de áreas.",
        go: ["tools"],
      },
    ],
  },
  {
    id: "data",
    title: "Datos del equipo",
    intro: "Los archivos del usuario: moverlos, protegerlos, borrarlos y recuperarlos. Está en el área Este equipo.",
    topics: [
      {
        id: "migrate",
        title: "Copia de datos",
        what: "Copia el Escritorio, Documentos, Imágenes, los marcadores de los navegadores y las redes Wi‑Fi de un usuario a un disco externo, y los restaura en el equipo nuevo sin sobrescribir lo que ya haya.",
        go: ["data", "migrate"],
      },
      {
        id: "vault",
        title: "Caja fuerte",
        what: "Una unidad cifrada con contraseña que se guarda en un solo archivo y se abre con doble clic, aunque AdminOps no esté. En Windows Home, una carpeta cifrada en un archivo comprimido.",
        notes: ["Si se pierde la contraseña, lo de dentro no se puede recuperar. No hay puerta trasera."],
        go: ["data", "vault"],
      },
      {
        id: "wipe",
        title: "Borrado seguro",
        what: "Borra archivos sobrescribiéndolos para que no se puedan recuperar, limpia el espacio libre de un disco, y guía para dejar un equipo listo antes de venderlo o donarlo.",
        notes: ["No tiene vuelta atrás. Comprueba dos veces lo que has elegido.", "En discos SSD el borrado por sobrescritura no garantiza lo mismo que en discos mecánicos; para vender un equipo, lo más seguro es cifrar el disco entero y después restablecer Windows."],
        go: ["data", "wipe"],
      },
      {
        id: "recover",
        title: "Recuperar archivos",
        what: "Intenta recuperar archivos borrados con la herramienta de Microsoft para eso, sin tocar la consola.",
        how: ["Elige el disco donde estaban y otro disco distinto donde guardar lo recuperado.", "Cuanto antes, mejor: deja de usar el disco en cuanto notes la pérdida."],
        notes: ["No siempre se puede. En discos SSD, lo borrado suele desaparecer de verdad en poco tiempo."],
        go: ["data", "recover"],
      },
      {
        id: "family",
        title: "Control parental",
        what: "Filtro de webs para adultos y de malware, sitios bloqueados y horario de uso para una cuenta del equipo.",
        notes: ["El menor debe usar una cuenta estándar, no de administrador: con una de administrador puede quitarlo."],
        go: ["data", "family"],
      },
    ],
  },
  {
    id: "settings",
    title: "Ajustes de AdminOps",
    intro: "Lo que se puede personalizar. Tiene buscador arriba.",
    topics: [
      {
        id: "settings-all",
        title: "Las secciones de Ajustes",
        what: "General (modo, arranque, copias, dónde se guardan los datos), Apariencia (tema, color, tamaño), Navegación (barra lateral y atajos), Portales y correo, Seguridad (bloqueo con PIN), Informes y cobros (tu marca, firma, impuestos, tipos de visita), Rendimiento y Acerca de.",
        how: ["Escribe en el buscador lo que quieres cambiar: «zoom», «firma», «inactividad».", "Los cambios de la sección Informes se guardan con el botón de abajo; los demás, al momento."],
        go: ["settings"],
      },
      {
        id: "lock",
        title: "Bloqueo con PIN",
        what: "Protege AdminOps con un PIN o una contraseña: se pide al abrir y tras un tiempo sin usarla. En el pendrive protege además las contraseñas guardadas.",
        notes: ["Si olvidas el PIN, puedes abrir AdminOps con la contraseña de Windows, pero en el pendrive las contraseñas guardadas no se recuperan: habrá que volver a guardarlas."],
        go: ["settings", "security"],
      },
      {
        id: "backup",
        title: "Copias de tus datos",
        what: "Tres maneras: la copia automática a otra carpeta cada pocos días, la copia cifrada a mano («Seguridad de tus datos») y la «Configuración de empresa», que exporta los datos de la empresa y los portales para que otro técnico empiece con todo listo.",
        notes: ["La configuración de empresa no lleva contraseñas, sesiones, tu nombre ni tu firma."],
        go: ["settings", "data"],
      },
    ],
  },
  {
    id: "responsibility",
    title: "Responsabilidad y buenas prácticas",
    intro: "AdminOps es una herramienta potente. Usarla bien es cosa de quien la maneja.",
    topics: [
      {
        id: "authorization",
        title: "Trabaja solo en equipos autorizados",
        what: "Usa AdminOps en equipos tuyos o en los que tengas permiso expreso de su dueño o de la organización. Ver las contraseñas de las Wi‑Fi guardadas, las cuentas o los archivos de un equipo sin permiso no es soporte técnico.",
        notes: ["En una empresa, sigue sus normas: si exigen pasar por su departamento de sistemas para instalar un programa, pásalo. La guía de instalación para sistemas explica qué permisos pide AdminOps y qué conexiones hace."],
      },
      {
        id: "before",
        title: "Antes de tocar",
        what: "Tres costumbres que evitan casi todos los disgustos.",
        how: [
          "Copia de seguridad de lo que no se pueda perder, antes de reparar discos, cambiar cuentas o formatear.",
          "Un diagnóstico antes y otro después: sabrás qué cambió y podrás demostrarlo.",
          "Las claves de recuperación de BitLocker a un pendrive antes de tocar nada del arranque o de la BIOS.",
        ],
      },
      {
        id: "judgement",
        title: "El criterio es tuyo",
        what: "AdminOps propone y explica; no decide. Un ajuste recomendado para un equipo de oficina puede no convenir a otro. Lee la explicación y el nivel de riesgo, y aplica solo lo que entiendas.",
        notes: ["Ante la duda, usa el modo auditoría para mirar sin tocar, o consulta antes de aplicar.", "El programa se entrega sin garantía. Los términos de uso, en esta misma ventana, lo detallan."],
      },
      {
        id: "antivirus",
        title: "Avisos de Windows y del antivirus",
        what: "AdminOps no está firmado digitalmente, así que Windows muestra «Windows protegió su PC» al instalarlo: «Más información» → «Ejecutar de todas formas». Un antivirus de empresa puede pedir confirmación, porque AdminOps cambia la configuración del sistema.",
        notes: ["Descárgalo solo de su origen oficial.", "AdminOps no intenta esquivar al antivirus. Si bloquea una acción, lo dice; la solución es que el responsable del antivirus lo autorice."],
      },
    ],
  },
  {
    id: "help",
    title: "Si algo falla",
    intro: "Qué hacer cuando AdminOps no hace lo que esperabas.",
    topics: [
      {
        id: "report-problem",
        title: "Reportar un problema",
        what: "Describes lo que pasó y AdminOps prepara un correo para el autor con un archivo de diagnóstico adjunto. El correo se abre en tu programa de correo: lo revisas y lo envías tú.",
        how: ["Esta ventana → «Reportar un problema».", "Cuenta qué hacías, qué esperabas y qué pasó. Si se repite siempre, dilo.", "«Preparar el correo»."],
        notes: ["El archivo lleva el registro de actividad, el último diagnóstico y la versión. Puede contener el nombre del equipo y del usuario: ábrelo antes si quieres ver qué va."],
      },
      {
        id: "log",
        title: "El registro técnico",
        what: "Todo lo que AdminOps ejecuta queda anotado, con sus errores y lo que tardó. Está en Historial → Registro técnico.",
        go: ["history"],
      },
      {
        id: "stuck",
        title: "Una pantalla no carga",
        what: "Si una pantalla no puede leer algo, lo dice en un recuadro rojo con el motivo y un botón «Reintentar». F5 recarga la página que estás viendo.",
        notes: ["Si pasa en todas las pantallas, cierra AdminOps y vuelve a abrirlo. Si sigue, repórtalo."],
      },
    ],
  },
];

export interface GlossaryEntry {
  term: string;
  def: string;
}

export const GLOSSARY: GlossaryEntry[] = [
  { term: "Administrador", def: "Cuenta de Windows con permiso para cambiar el sistema. AdminOps necesita abrirse con una para aplicar cambios." },
  { term: "Ajuste", def: "Un cambio concreto de la configuración de Windows que AdminOps sabe aplicar y deshacer." },
  { term: "BitLocker", def: "El cifrado de discos de Windows. Si el disco está cifrado y se pierde la clave de recuperación, los datos no se pueden leer." },
  { term: "Bloatware", def: "Aplicaciones que vienen preinstaladas con el equipo y que casi nadie pidió." },
  { term: "Caso", def: "Una incidencia de una persona, con lo que se hizo para resolverla. Al cerrarlo queda en su ficha." },
  { term: "Checklist", def: "La lista de comprobación de una visita. Algunos puntos se marcan solos al hacer la tarea con AdminOps." },
  { term: "Clave de recuperación", def: "El número largo que abre un disco cifrado con BitLocker cuando Windows no puede abrirlo solo." },
  { term: "Credencial guardada", def: "Un usuario y contraseña que Windows recuerda para una carpeta de red, una impresora o un escritorio remoto." },
  { term: "Diagnóstico", def: "El repaso automático del equipo. Se guarda para comparar el antes y el después." },
  { term: "Diario de cambios", def: "El registro de todo lo que AdminOps ha cambiado en un equipo, con el valor anterior de cada cosa." },
  { term: "DISM", def: "Herramienta de Windows que repara la imagen del sistema. Se usa cuando SFC no basta." },
  { term: "DNS", def: "El servicio que traduce nombres de webs a direcciones. Si falla, «no hay Internet» aunque la conexión esté bien." },
  { term: "Dominio", def: "La red de una empresa en la que las cuentas y los equipos se gestionan desde un servidor central (Active Directory)." },
  { term: "Driver", def: "El programa que permite a Windows usar una pieza del equipo. Un driver defectuoso causa pantallazos azules." },
  { term: "Escritorio remoto", def: "La forma que trae Windows de manejar un equipo desde otro." },
  { term: "Hallazgo", def: "Cada cosa que encuentra el diagnóstico, con su gravedad." },
  { term: "Hosts", def: "Un archivo de Windows que fuerza a qué dirección va un nombre. Se usa para bloquear sitios, y el malware también lo usa." },
  { term: "IP", def: "La dirección de un equipo en la red. La IP pública es la que se ve desde Internet." },
  { term: "MAC", def: "El identificador de fábrica de una tarjeta de red." },
  { term: "Modo auditoría", def: "Estado en el que AdminOps solo mira: no cambia nada en el equipo." },
  { term: "Modo usuario", def: "La interfaz reducida de AdminOps para quien no es técnico." },
  { term: "NTFS (permisos del disco)", def: "Los permisos de una carpeta en el disco. En una carpeta compartida se combinan con los de la compartición." },
  { term: "Pantallazo azul", def: "El error grave de Windows que detiene el equipo. El diagnóstico señala el driver probable." },
  { term: "Perfil (de ajustes)", def: "Un grupo de ajustes que se aplican juntos." },
  { term: "Perfil (de usuario)", def: "La carpeta personal de una cuenta de Windows: Escritorio, Documentos, configuración de sus programas." },
  { term: "PIN", def: "El código con el que se bloquea AdminOps." },
  { term: "Plantilla", def: "Una secuencia guardada de pasos para preparar un equipo, o un texto con huecos que se rellenan solos." },
  { term: "Portable", def: "La versión que no se instala: se ejecuta desde una carpeta o un pendrive y guarda ahí sus datos." },
  { term: "Portal", def: "Una web (tickets, correo, Teams, el panel de un router) abierta dentro de AdminOps." },
  { term: "Punto de restauración", def: "Una foto de la configuración de Windows a la que se puede volver." },
  { term: "Registro (de Windows)", def: "La base de datos donde Windows y los programas guardan su configuración." },
  { term: "Registro técnico", def: "El archivo donde AdminOps anota lo que ejecuta y sus errores." },
  { term: "Sector dañado", def: "Una zona del disco que ya no guarda bien los datos. Si aparecen más con el tiempo, el disco se está muriendo." },
  { term: "Seguimiento", def: "Un recordatorio para volver a mirar algo otro día." },
  { term: "Servicio", def: "Un programa de Windows que trabaja en segundo plano, sin ventana." },
  { term: "Sesión de servicio", def: "Una visita registrada de principio a fin, que termina en un informe." },
  { term: "SFC", def: "Herramienta de Windows que comprueba y repara los archivos del sistema." },
  { term: "SMART", def: "Los datos de salud que un disco da de sí mismo: horas de uso, errores, temperatura." },
  { term: "SmartScreen", def: "El filtro de Windows que avisa al ejecutar programas que no reconoce o que no están firmados." },
  { term: "Unidad de red", def: "Una letra (Z:, por ejemplo) que apunta a una carpeta compartida de otro equipo." },
  { term: "Wake-on-LAN", def: "Encender un equipo apagado mandándole una señal por la red. Requiere cable y tenerlo activado en la BIOS." },
  { term: "WebView2", def: "El componente de Microsoft Edge, incluido en Windows, con el que AdminOps dibuja su ventana." },
  { term: "winget", def: "El instalador de programas de Windows. AdminOps lo usa para instalar y actualizar." },
];

// Los términos técnicos del glosario de bolsillo (el subrayado de puntos) también
// salen aquí, con qué hacer. Los que ya tenían definición propia se quedan con ella.
for (const g of TECH_GLOSSARY)
  if (!GLOSSARY.some((x) => x.term.toLowerCase() === g.term.toLowerCase())) GLOSSARY.push({ term: g.term, def: `${g.what} ${g.worry === "—" ? "" : `${g.worry} `}${g.todo}` });
GLOSSARY.sort((a, b) => a.term.localeCompare(b.term, "es"));

export interface FaqEntry {
  q: string;
  a: string;
}

export const FAQ: FaqEntry[] = [
  { q: "¿AdminOps envía mis datos a algún sitio?", a: "No. No tiene cuentas, ni servidor, ni telemetría. Solo se conecta a Internet para lo que tú le pides: comprobar si hay versión nueva, la prueba de velocidad, instalar programas y las webs que abres dentro." },
  { q: "Apliqué un ajuste y algo va peor. ¿Qué hago?", a: "Estado del equipo → Historial → Diario de cambios, busca el ajuste y pulsa «Deshacer». Si fueron varios, deshaz de uno en uno empezando por el último. Si el equipo no arranca bien, usa «Restaurar sistema» con el punto de restauración que se creó antes." },
  { q: "¿Por qué muchos botones están apagados?", a: "Porque AdminOps no está abierto como administrador, o porque el modo auditoría está activo. Al pasar el ratón por el botón dice el motivo." },
  { q: "¿Puedo usar el mismo pendrive en varios equipos?", a: "Sí, para eso está. Tus datos van en el pendrive; en cada equipo tendrás que entrar una vez en los portales (correo, tickets), porque Windows guarda esas sesiones por equipo." },
  { q: "Windows dice que el programa no es de confianza.", a: "AdminOps no está firmado digitalmente. En el aviso, «Más información» → «Ejecutar de todas formas». Descárgalo siempre de su origen oficial." },
  { q: "El antivirus bloqueó una acción.", a: "AdminOps cambia la configuración del sistema, y algunos antivirus de empresa lo vigilan. AdminOps no intenta saltárselo: quien administra el antivirus tiene que autorizarlo." },
  { q: "¿El modo usuario impide que alguien cambie cosas?", a: "No. Esconde opciones para no confundir, pero no es una protección. Para impedir cambios, la persona debe usar una cuenta de Windows estándar." },
  { q: "Olvidé el PIN.", a: "Puedes abrir AdminOps con la contraseña de tu cuenta de Windows. En el pendrive, las contraseñas guardadas quedan ilegibles sin el PIN: tendrás que guardarlas de nuevo." },
  { q: "¿Una limpieza se puede deshacer?", a: "No. Lo que se limpia son archivos temporales y cachés que Windows y los programas vuelven a crear, pero lo borrado no se recupera." },
  { q: "¿Sirve en Windows Home?", a: "Sí. Algunas cosas dependen de la edición: recibir Escritorio remoto, BitLocker completo y unir el equipo a un dominio necesitan Windows Pro. AdminOps lo indica en cada pantalla." },
  { q: "¿Cómo paso mis datos a otro equipo?", a: "Ajustes → Datos y copias → «Seguridad de tus datos» hace una copia cifrada. En el otro equipo, el mismo sitio, «Restaurar una copia»." },
  { q: "¿Cómo actualizo AdminOps?", a: "Ajustes → Acerca de → Actualizaciones → «Buscar ahora»; si hay una versión nueva publicada, «Descargar e instalar» la baja y abre su instalador, que conserva tus datos. También puedes pasar a mano el instalador de la versión nueva por encima." },
];
