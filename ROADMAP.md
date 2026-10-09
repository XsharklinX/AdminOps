# AdminOps — Hoja de ruta

Estado actual: **v1.1.6**. Este documento recoge el plan en curso, lo que ya está hecho y
la deuda técnica conocida. Cada fase tiene un criterio de "terminado" para saber cuándo
cerrarla.

---

## Plan actual — Profesionalizar lo que hay (v1.2)

**Nada de funciones nuevas hasta cerrar esto.** AdminOps ya hace mucho; lo que le falta es hacerlo
siempre bien. Este plan sale de lo que pasó entre la 1.1.5 y la 1.1.6, no de una lista de ideas:

- **Cinco fallos graves pasaron todas las pruebas** (240 entre Rust e interfaz) y rompieron la app
  al usarla: portales colgados en «Abriendo…», Ajustes sin poder pulsar nada, el MFA de Microsoft
  rechazado, el modo claro con el fondo negro y la ventana del portable en negro. Las pruebas miran
  las piezas por separado; **nada comprueba la aplicación en marcha**.
- **La 1.1.6 se compiló cinco veces** con contenido distinto y el mismo número de versión.
- **127 archivos sin guardar en Git**: el último commit es la 1.1.4. Todo lo de la 1.1.5 y la 1.1.6
  vive solo en este disco.
- Cada cambio de un dato (la red, una visita, una carpeta compartida) había que hacerlo **a mano en
  Rust y en TypeScript**, y el cruce de comandos se hacía a mano hasta hace poco.

Cada fase cierra un tipo de fallo de los que ya han pasado. Van en orden: la 30 es la que más
problemas evita, y conviene hacerla antes que ninguna otra.

### Una sola estación: las tres primeras (hecho, sin probar en un dominio real)

Adelantadas a la Fase 30 por decisión del técnico. Están verificadas con pruebas y el compilador,
pero **no se han ejecutado contra el dominio de la empresa**: eso solo se puede probar allí.

- **Personas** (Soporte → Personas). La ficha de alguien del dominio buscando por nombre, usuario,
  correo o extensión: cuenta bloqueada, desactivada o con la contraseña caducada o a punto; su
  departamento, extensión y responsable; en qué equipo tiene la sesión abierta (según la última
  comprobación a fondo de Puestos); y lo que ya se hizo con ella en casos anteriores. Acciones:
  **desbloquear**, **contraseña temporal** (fácil de dictar, obliga a cambiarla y desbloquea a la vez;
  se enseña una vez y no se guarda en ningún sitio), **Teams** y **abrir un caso**. Además, las
  **contraseñas de un equipo**: la **LAPS** del administrador local (Windows LAPS, también cifrada, y la
  versión anterior; se tapa sola al minuto) y las **claves de recuperación de BitLocker** guardadas en
  el dominio, por equipo o por el **ID que enseña la pantalla de recuperación**.
  Por ADSI, que viene con Windows (no hace falta RSAT), y **con los permisos del técnico**: si el dominio
  no le deja, se lo dice. Desbloquear y cambiar la contraseña quedan en el diario y los bloquea el modo
  auditoría; consultar LAPS o BitLocker también queda en el diario (qué equipo, nunca la contraseña).
- **El caso de ahora**. Una barra arriba, en cualquier página, con el ticket, la persona, el equipo, el
  tiempo y lo que va quedando apuntado. **No hay que registrar nada**: se lee del diario, que ya guarda
  cada cambio. Al cerrarlo se redacta la resolución con lo que se hizo de verdad (lo que falló, aparte y
  con el motivo), editable, para **copiarla** o **pegarla en el campo del ticket** que tengas seleccionado
  en el portal (AdminOps comprueba si la escribió; si no, la deja copiada y lo dice). Se abre desde la
  cabecera, desde la ficha de una persona o desde Ctrl+K. Queda en la ficha de la persona.
- **Ctrl+K reconoce lo que escribes**: un equipo (`PC-CONTA-03`), una persona (`maria.perez`), una
  extensión, una IP, una impresora o un ticket (`#4521`), y ofrece lo que se hace con cada uno. Lo
  inconfundible va arriba; dos palabras sueltas (que pueden ser un nombre o «windows update») van al
  final, sin tapar lo que Ctrl+K ya encontraba. Solo ofrece acciones que funcionan de verdad.

### Una sola estación: las tres siguientes (hecho)

- **Pantalla dividida.** En Tickets, Correo y Teams, un selector «Al lado» pone otra página de
  AdminOps a la derecha: Personas, Solucionar problemas, Impresoras, Contactos o Soluciones. Se lee el
  ticket mientras se resuelve. Solo páginas que se leen bien en media pantalla, y ningún portal (dos
  vistas web nativas no se reparten bien la ventana). Personas usa *container queries*: sus columnas
  dependen del hueco que tiene, no del ancho de la ventana, así que se ve bien a pantalla completa y
  en media. Se recuerda la elección.
- **Nota de llamada con Ctrl+Alt+N**, aunque AdminOps esté minimizado: una ventana pequeña, siempre
  encima, para apuntar quién llama, su equipo y qué le pasa, y convertirlo en un caso o en un
  seguimiento para mañana. El atajo usa `RegisterHotKey` de Windows (sin plugins nuevos); si otro
  programa ya lo tiene, se dice en el registro y la nota se abre desde Ctrl+K. La ventana se crea con
  las mismas opciones de navegador que la principal.
- **Seguimientos**: «volver a mirar esto el jueves», con fecha. Aparecen en «Hoy» cuando tocan y avisan
  por Windows una sola vez; se marcan hechos o se aplazan. Nunca se borra uno pendiente al podar.
- **Recorte de pantalla** desde la barra del caso y desde Ctrl+K: abre el recorte de Windows y lo
  recortado queda en el portapapeles para pegarlo en el ticket. *No tapa datos personales*: eso sobre
  una captura cualquiera necesitaría leer el texto de la imagen (OCR), y no se promete lo que no hace.
- **Hoy**, arriba del Panel: el caso abierto, los seguimientos que tocan o se pasaron, las visitas de
  hoy, los clientes con el mantenimiento vencido y los avisos de Windows sin leer, ordenados por lo que
  corre más prisa, con la acción de cada cosa. No sale en modo usuario (ahí el Panel lo ve el cliente).

### Microsoft 365 e impresoras que hablan (lo de Microsoft 365 se quitó en la 1.1.10)

- **Microsoft 365 con Graph**, con la cuenta del técnico. Se conecta en Ajustes → Portales y correo
  con el inicio de sesión **por código** (microsoft.com/devicelogin en el navegador de siempre, con
  su MFA): no pasa por ninguna vista web integrada. La sesión se guarda cifrada; el token de acceso,
  solo en memoria. Permisos **delegados** y `.default`: AdminOps solo puede lo que IT concedió a la
  aplicación *y* lo que el rol del técnico permite; si falta un permiso, esa función lo dice.
  - **Personas → Microsoft 365**: los últimos inicios de sesión con el motivo en español (50126
    contraseña, 50053 bloqueo, 500121 MFA no aprobado, 53003 acceso condicional…; los que no se
    conocen, con el texto de Microsoft y el código), **métodos de MFA** con «Quitar» (el móvil
    perdido: al quedarse sin métodos, Microsoft le pide registrarlos de nuevo), **cerrar todas sus
    sesiones** y **mensaje por Teams**. Quitar MFA y cerrar sesiones van al diario y el modo
    auditoría los bloquea.
  - **Hoy**: las incidencias abiertas de Microsoft 365 (cada 10 minutos, no cada minuto).
  - **Agenda**: cada visita se pone en el calendario de Outlook del técnico (y se actualiza si ya
    estaba; si la borraron en Outlook, se crea otra).
  - **Cerrar caso → «Avisar por Teams»** a la persona de que ya está resuelto.
- **Impresoras por SNMP** (Printer-MIB, v2c con v1 de reserva, comunidad `public`, solo lectura):
  «Revisar» dice ahora lo que cuenta el propio aparato — atasco, puerta abierta, sin papel — con
  prioridad sobre lo que sabe Windows, y **«Tóner negro al 8 %»** cuando no hay nada peor. Muestra
  los niveles de cada consumible y el contador de páginas. Cliente SNMP propio (unas 200 líneas de
  BER probadas byte a byte) en vez de una dependencia.
- **Descubrir impresoras por su nombre**: mDNS (`_ipp`, `_pdl-datastream`, `_printer`) y
  WS-Discovery a la vez que la tabla de vecinos. Salen también las que este equipo nunca ha visto,
  con su nombre y modelo (mDNS, o SNMP si no se anuncian). Si el cortafuegos no deja pasar las
  respuestas, la búsqueda por puertos sigue igual.
- **Deuda cerrada**: los 9 avisos de ESLint `no-floating-promises` (promesas sin gestionar en
  Knowledge, Migrate, Report, Ajustes → Bloqueo, Cuentas, Dispositivos, Plan de acción e informe de
  servicio). `npx eslint .` queda sin avisos.
- **La ayuda «?» de las pestañas salía vacía** (una raya bajo el icono) en todas las páginas con
  pestañas: la tira tiene scroll horizontal y eso recortaba el recuadro. Ahora se coloca fijo en la
  ventana, hacia la izquierda si no cabe, y se cierra al hacer scroll o cambiar el tamaño.

### v1.2.6 — Ajustes ordenado, vista previa del informe y batería (hecho, sin build)

- **Ajustes**: `SECTIONS` lleva `group`; `General.tsx` exporta `StartWindow`, `AlertSettings`,
  `DataSettings`, `SystemChanges` (en Seguridad) y `DomainCard` (en Portales y red). `DataCare`
  tiene `part` («data» en Datos y copias, «updates» en Acerca de); `PerfPanel` va en Acerca de.
  `LEGACY_SECTIONS` lleva «general» y «performance» (pestaña guardada, enlaces) a su sitio nuevo.
  Fuera la fila repetida «Precargar los portales» (sigue en Portales).
- **Vista previa del informe**: `report_html` sale de `create_report`; `preview_report` arma la
  misma entrada (`report_input`) y devuelve el HTML con el número «BORRADOR», sin `next_number`.
  `ReportPreview.tsx` lo pinta en un `iframe` con `sandbox` vacío. CSP: `font-src 'self' data:`
  (la fuente del informe va incrustada).
- **Batería** (`hardware/battery.rs`, `BatteryCard.tsx`, `lib/batteryChart.ts`): historial
  semanal de `powercfg /batteryreport /xml`, limpiado (`clean`) y con recta del último año
  (`trend`) para la pérdida al mes y los meses hasta el 50 %. **Sin probar en un portátil real**:
  la estructura del XML del historial se lee igual que la del resto del informe, pero no se ha
  visto con datos de verdad.
- Pasado a esta versión desde la 1.2.5 (hecho después de su build): el ancho de cada tarjeta del Panel.

### v1.2.5 — Ajustes, pestañas y Ctrl+F (compilada)

Elegidas sobre el catálogo «AdminOps, ideas para la 1.2.5».

- **Resumen** (`pages/settings/Summary.tsx`): sección nueva y la primera al entrar. Solo lee; la
  versión nueva se consulta al pulsar.
- **Interruptores y filas compactas**: `Toggle` en `components/ui.tsx` (con `ToggleLabelCtx`, que
  `Row` rellena con su título para los lectores de pantalla). `Row` esconde `sub` tras «?» y la
  enseña sola si el buscador lleva a esa fila. Las casillas de tabla (catálogo de Informes) siguen
  siendo casillas.
- **Vista previa** (`pages/settings/AppPreview.tsx`): se pinta con los mismos tokens de color.
- **Deshacer en Ajustes** (`SettingsPage.tsx`): guarda el estado de antes del último cambio, sea de
  `Settings` (backend) o de `Prefs` (`setPrefs` emite ahora `adminops-prefs` con `detail.before`).
  Cambios seguidos del mismo campo en menos de 3 s cuentan como uno.
- **Ancho de cada tarjeta del Panel**: rejilla de seis columnas, `PanelPrefs.widths` y `spanOf`
  (un ancho no válido vuelve al de fábrica). Cada tarjeta es un `@container`, y sus columnas
  internas (`@3xl:`, `@2xl:`) siguen a su ancho, no al de la ventana. El alto no se elige: lo da
  el contenido.
- **Pestañas** (`components/OpenTabs.tsx`): las pantallas vivas de App en el orden de apertura;
  cerrar una la desmonta. Preferencia `pageTabs` (encendida de fábrica).
- **Ctrl+F** (`lib/pageFind.ts`, `components/PageFind.tsx`): CSS Custom Highlight API sobre el
  contenedor `[data-page]` de la pantalla actual; vuelve a buscar cada 2 s mientras está abierta.
- **No elegidas** (no volver a proponer tal cual): secciones nuevas de Ajustes, ver lo cambiado y
  restablecer, etiquetas de cuándo surte efecto, dos pantallas lado a lado (ya existe la pantalla
  dividida de los portales), modo compacto, tablas con columnas y CSV, hoja de atajos, acciones
  fijadas arriba.
- Pasados a esta versión desde la 1.2.4 (hechos después de su build): arrastrar arreglado, fuera el
  botón «Todo», pantallas a todo el ancho, aviso del icono junto al reloj.

### v1.2.4 — Once ideas elegidas por el autor (compilada)

Elegidas sobre un catálogo con maquetas (artifact «AdminOps, ideas para la 1.2.4»).

- **Duplicados** (`space.rs`): `space_duplicates` recorre la raíz del último análisis (archivos de
  1 MB o más), agrupa por tamaño, descarta con una huella de muestras y confirma leyendo el archivo
  entero; por encima de 256 MB se queda en las muestras y el grupo lo dice (`sampled`). No entra en
  Windows, la papelera ni `System Volume Information`, y no lee lo que solo está en la nube
  (`RawEntry.is_cloud`). Tarea «duplicates», cancelable. **Sin tratar**: dos enlaces duros al mismo
  archivo saldrían como duplicados sin que borrar uno libere nada (fuera de Windows es raro).
- **Liberar lo verde** (`Space.tsx`): ejecuta `cleanup.windows-temp`, `cleanup.windows-update-cache`
  y `cleanup.user-temp` según lo que haya medido `space_freeable`.
- **Panel a tu medida**: `lib/panelLayout.ts` (bloques, `arrange`, `shift`, `dropOn`),
  `components/PanelGrid.tsx`, preferencia `panel {order, hidden}`. El veredicto de arriba es fijo.
- **Qué frena el equipo** (`lib/slowdown.ts`, `components/Slowdown.tsx`): sustituye a «Procesos con
  más carga». Lista de procesos de Windows que se explican y no se ofrece cerrar.
- **Icono junto al reloj** (`tray.rs`, característica `tray-icon` de Tauri): ajuste `tray_icon`,
  apagado de fábrica. Con `close_minimizes`, la X esconde la ventana en vez de minimizarla.
  Las acciones que hace la interfaz llegan por el evento `tray-action`.
- **Mini monitor** (`minimon.rs`, `components/MiniMonitor.tsx`): ventana «monitor», siempre encima
  y fuera de la barra de tareas; se cierra con la principal.
- **Actividad** (`lib/activity.ts`): tareas terminadas de la sesión, en la campana.
- **Ctrl+K**: `DO_NOW` en `App.tsx` (acciones que ejecutan un ajuste al momento).
- **Copiar tarjeta** (`lib/copyImage.ts`, botón en `Card`): dependencia nueva `html-to-image`.
  **Sin probar en la app**: que WebView2 deje escribir una imagen en el portapapeles desde aquí.
- **«Nuevo»** (`lib/whatsNew.ts`): `NEW_IN` se rellena al publicar; una prueba comprueba que cada
  clave existe y que su versión está en las novedades.
- **Deshacer en el momento**: ya existía (`undoable-change` → aviso con «Deshacer» 10 s).
- **Arrastrar estaba roto en toda la app**: Tauri captura por defecto el arrastre de archivos del
  sistema y WebView2 deja entonces de dar el de la página. `dragDropEnabled: false` en la ventana
  principal y `disable_drag_drop_handler()` en la nota y el monitor. Si algún día se quiere soltar
  archivos desde el Explorador, habrá que hacerlo con el evento de Tauri y no con el de la página.
- **Ancho**: las pantallas usan `max-w-(--page-max)`; `applyAppearance` lo pone a `none` o `72rem`
  según la preferencia `pageWidth` (de fábrica, toda la ventana).
- Fuera el botón «Todo» de la columna de áreas (repetía el buscador).
- **Quedan en «quizá»** (no hacer sin que el autor lo pida): qué ha crecido desde el último
  análisis de espacio, rutinas de varios pasos, arrastrar una carpeta a la ventana.

### v1.2.3 — Avisos y arranque (compilada)

**Avisos (campana)**
- `AlertCenter.tsx` rehecho sobre `lib/alerts.ts` (con pruebas): por días, filtro por nivel y «solo
  sin leer», avisos leídos plegados, descartar uno, silenciar un tipo.
- `winwatch.rs`: `Store.muted` (`MutedAlert`: clave, título, detalle, fecha), por equipo. `merge`
  ignora lo silenciado, así que no llega ni a la campana ni a la notificación. Comandos
  `dismiss_windows_alert`, `mute_windows_alert`, `unmute_windows_alert`, `muted_windows_alerts`.
  Los de la campana pasan a `async` (leían un archivo en el hilo principal).
- Ajuste `notify_alerts` (all | bad | none): de qué avisos salta notificación de Windows.

**Arranque** (medido en un arranque de 46 s desde un pendrive)
- `window_state::wait_until_ready`: la consola de PowerShell (`pspool::warm_up`) y la seguridad
  (`security::warm_up`) esperan a que la interfaz esté pintada; antes usaban esperas fijas de 3 y
  12 s desde el inicio del programa, que en un arranque lento caían en plena carga de WebView2.
- `library::this_place`: fuera el precalentado al segundo cero. El número de serie y las redes ya
  vistas se guardan en `lugar.json` (por equipo); la red se reconoce con `lan::fingerprint` (puerta
  de enlace y su MAC, sin PowerShell). Solo la primera vez en cada red se pregunta a Windows.
- `diagnostics::latest_snapshot`: último análisis en memoria, renovado en `save_snapshot`.
- `useSensors`: `nextSensorWait` espacia las lecturas tras una lenta (hasta 60 s).
- `App.tsx`: precarga de portal a los 20 s (no si el arranque fue lento); versión nueva a los 30 s.
- **Sin tocar, a decisión del autor**: el perfil del navegador en el pendrive (Ajustes → General).
  Es lo que más pesa en el arranque desde USB, y está así a propósito: no deja sesiones en el
  equipo del cliente.

**Setup: la comprobación final no podía salir bien**
- `installer/build.rs` calculaba la huella de `target/release/adminops.exe`, pero Tauri marca el
  ejecutable que empaqueta (`__TAURI_BUNDLE_TYPE_VAR_NSS`) y deja el de `target` como estaba
  (`…_UNK`): tres bytes distintos, y `exe_is_current` fallaba siempre aunque la instalación hubiera
  ido bien (código 2, «usa el instalador clásico»). `installer/src/fingerprint.rs` calcula la huella
  sin esa marca; lo usan `build.rs` y el programa. Comprobado con los dos ejecutables de la 1.2.3.
  **Sin confirmar en un equipo**: que no hubiera además un bloqueo real de archivos.

**Cerrar minimizando**
- Ajuste `close_minimizes` (apagado de fábrica). `window_state`: `minimizes_on_close`, comando
  `quit_app`; en `CloseRequested` se llama a `prevent_close` y se minimiza. `QuitButton.tsx` solo
  aparece con el ajuste activo. La salida por actualización (`app.exit`) no pasa por ahí.
- El arranque con Windows ya existía (tarea programada con `--minimized`, apagado de fábrica).

**Discos → Espacio, de mirar a manejar**
- `space.rs`: cada carpeta del análisis guarda lo que ocupa cada tipo de archivo (`KINDS`, por
  extensión) y su cambio más reciente; se apartan además los más grandes de cada tipo (`Tops`).
  El recorrido cuesta lo mismo: todo sale del mismo listado.
- Comandos: `space_folder` (sustituye a `space_children`: subcarpetas y tipos de una carpeta),
  `space_files` (archivos sueltos de una carpeta, leídos al momento) y `space_kind_files`.
- `space_recycle` acepta carpetas. `Protected` se niega con la unidad entera, Windows, los archivos
  de paginación e hibernación, Archivos de programa y ProgramData (ellos y su primer nivel) y la
  carpeta de perfiles (ella y cada perfil). Tras borrar, `forget` descuenta lo que se fue en toda
  la rama (tamaño, archivos, tipos, listas): no hace falta volver a analizar.
- `pages/Space.tsx` rehecha sobre `lib/spaceView.ts` (con pruebas): barra de tipos por carpeta,
  ficha de la carpeta señalada, lista ordenable, archivos de la carpeta actual, buscador, y una
  sola selección para carpetas y archivos con su barra fija abajo.
- Los duplicados se hicieron en la 1.2.4; «qué ha crecido» quedó en «quizá».

### v1.2.2 — Todo a la vista: navegación nueva (compilada)

Diseñada con el autor sobre un prototipo (artifact «AdminOps, todo a la vista»). El problema: con
unas 80 pestañas, muchas funciones (Dominio era la tercera pestaña de «Usuarios y cuentas») solo
existían para quien ya sabía dónde estaban.

- **Barra lateral en dos columnas**: áreas a la izquierda; al lado, cada pantalla con sus secciones
  como líneas. Registro único de secciones en `lib/sections.ts`, con sinónimos para el buscador; una
  prueba lo compara con las pestañas de `Merged.tsx` y `Knowledge.tsx`. Las pestañas anuncian cuál
  está a la vista (`reportSection`) y obedecen a la barra (`requestSection`).
- **Áreas nuevas**: Inicio, Este equipo, Red, Programas, Administración, Soporte. Teams y Correo
  pasan a la barra de arriba; Herramientas de Windows, Bloquear y Ajustes, al pie de la columna.
- **Barra de arriba**: nombre del equipo, dominio o grupo de trabajo, permisos, Internet, espacio en
  el disco del sistema y avisos del último diagnóstico, cada uno con su destino. Comando nuevo
  `context.rs` (`NetGetJoinInformation`, `GetComputerNameExW`, `GetDiskFreeSpaceExW` y una conexión
  TCP de prueba con tope de 2 s): milisegundos, sin PowerShell. Se refresca cada minuto con la
  ventana a la vista.
- **Todo AdminOps** (Ctrl+K o F1): mapa del programa por áreas (tarjetas con pantallas y secciones),
  fijados y «siempre a mano»; al escribir, resultados con secciones y sinónimos.
- **Marcas de estado** junto a las secciones (avisos, dominio, espacio, Internet) y un punto en el
  área si dentro hay algo que atender. **Fijados** de pantallas o secciones. **Ruta** área › pantalla
  › sección sobre el título.
- Fuera: la franja de solo lectura (ahora es un dato de la barra de arriba) y las opciones de la
  barra que ya no aplican (secciones desplegadas, pestañas bajo el título, iconos, pie de usuario).
- Arreglado: el dominio de una organización real que quedaba en un mensaje de Dominio y en pruebas
  de Rust.

**Segunda tanda de 1.2.2** (lista de mejoras de interfaz, gráficas, herramientas y rendimiento):

- **Rendimiento (7 días)**: `perfhistory.rs` toma una muestra por minuto (procesador, memoria, disco
  del sistema y el programa que más gasta) mientras AdminOps está abierta; `perf-history.json` en los
  datos del equipo, recortado a 7 días. Pantalla en Estado del equipo → Rendimiento, con `TimeChart`
  (gráfica SVG propia, sin librerías, que corta la línea en los huecos).
- **Arranques y cuelgues**: `bootlog.rs` lee el registro de Windows (eventos 12, 13, 41 y 1001, y el
  100 de Diagnostics-Performance para la duración). Sin administrador Windows no da error al leer la
  duración: dice que no hay eventos; por eso, sin administrador y vacío se trata como «hace falta
  administrador». Códigos de pantallazo explicados en `lib/bugchecks.ts`.
- **Probar periféricos**: pantalla, teclado (por posición física, distribución española), altavoces
  con `StereoPanner`, micrófono y cámara con `getUserMedia` (en vivo; se apagan al salir).
- **Vigilante de la conexión**: `network/watch.rs`, ping ICMP cada 5 s al router (`GetBestRoute`) y a
  1.1.1.1/8.8.8.8; un corte empieza tras dos fallos seguidos; culpa router/Internet/sin red. Cada
  puesta en marcha lleva su número para que un hilo anterior no siga en paralelo.
- **Mapa del espacio**: `lib/treemap.ts` (squarified) en Discos → Espacio → Carpetas.
- **Calculadora de red**: `lib/subnet.ts`, en Herramientas de red.
- **Siluetas** en `Loading page` (todas las pantallas). **Memoria de lecturas lentas**:
  `lib/cachedRead.ts`, en Desinstalar e Inicio de Windows.
- **Sin PowerShell**: impresoras con `EnumPrintersW` (31 ms frente a ~1,9 s en este equipo; una
  prueba manual compara con WMI), además de la barra de arriba (`context.rs`).
- **Arranque medido** con el registro real del equipo: ventana ~0,9 s y primera página ~1,2 s, pero
  el Panel tardaba 3-6 s en quedar listo esperando «este equipo y esta red» (PowerShell de red de
  1-4 s y número de serie por CIM). Ahora se calcula en segundo plano al abrir y se recuerda
  (`library::warm_up`, `lan::current_recent`). Las métricas en vivo pasan a comando asíncrono
  (antes iban en el hilo de la ventana). La barra de arriba espera 1,2 s a su primera lectura.
- **Teams y Correo**: `comms.rs` (`open_comm`, `comm_apps`) y `CommOpener`: la primera vez pregunta
  (AdminOps, navegador o aplicación) y se recuerda en `prefs.comms`; se cambia en Ajustes → Portales.
- **Avisos al terminar**: nombres de todas las tareas en `task.rs` y `notify_if_long` para el análisis
  de espacio y la prueba de velocidad.

**Diagnóstico, revisado con datos reales** (51 análisis de 9 equipos guardados en el pendrive)

Lo que se vio: casi todo lo que señalaba salía en todos los equipos. Diez fallos arreglados:

1. Puertos PS/2 vacíos (código 24, ACPI) contados como «faltan drivers» en 4 de 9: se filtran al
   recoger (`collect::is_empty_port`).
2. «Adaptador de pantalla básico de Microsoft» salía como driver antiguo (informativo): ahora es
   el aviso «La tarjeta gráfica no tiene su driver instalado».
3. El análisis se guardaba con «se están buscando las actualizaciones…» (5 de 9) y así se quedaba:
   `complete_later` espera a winget, completa el análisis guardado (hallazgos y nota de seguridad)
   y emite `diagnostics-updated`.
4. SMART «Incompatible»/«Not supported» (5 de 9) es una lista vacía, no un error.
5. Temperaturas ausentes en 7 de 9 sin explicación.
6. Errores en crudo y secciones enteras perdidas: `explain_error`, reintento de los fallos
   pasajeros (RPC), y «Sistema y seguridad» en dos lecturas (lo básico no depende de lo lento).
   Los puntos 4 a 6 se ven en un bloque nuevo, **«No se pudo comprobar»** (`Diagnostics.unchecked`).
7. Arranque lento: mediana de los arranques apuntados, no el último.
8. «Arranques y cuelgues» duplicaba Estabilidad: una sola tabla de códigos (`bugcheck_info`), el
   análisis de volcados (driver probable) también allí, y la tarjeta Estabilidad enlaza en vez de
   repetir. Se quitó `lib/bugchecks.ts`.
9. AdminOps y su instalador no se cuentan entre los programas que fallan (quedan en el registro).
10. Seguridad: «Ejecución automática de USB» avisaba en 9 de 9 (medía el valor 255, no lo que
    Windows hace desde la versión 7); BitLocker en un sobremesa pasa a estado «info», que no cuenta
    en la nota (en portátil sigue siendo aviso).

Y uno más encontrado al leer: sin administrador, la tarjeta Estabilidad decía «sin registros» de
arranque en vez de «requiere administrador».

Medido volviendo a pasar las reglas por los análisis guardados (`cargo test recalibrate`, prueba
manual nueva): de 66 hallazgos a 57; los 8 de PS/2 desaparecen; el arranque lento pasa de 5 equipos
a 4.

**Diagnóstico: ruido fuera, niveles y lo que faltaba** (segunda parte del repaso)

- **Ruido**: fuera el hallazgo de apps promocionales (salía en 9 de 9); programas de inicio solo si
  hay 10 o más de terceros (`startup::enabled_overview`, `Diagnostics.startup_third_party`). Con los
  análisis reales: de 66 hallazgos a 41 (`cargo test recalibrate`).
- **Tres niveles** en la pantalla: Urgente (`Bad`), Conviene (`Warn`) y Sugerencias (`Info`), estas
  en un bloque plegado. El modelo no cambia: informe, Panel y análisis guardados siguen igual.
- **«Ya lo sé»**: `accepted-findings.json` por equipo (clave del problema sin cifras, título, motivo,
  fecha); comandos `diag_accepted`, `diag_accept`, `diag_unaccept`. `latest_findings` ya no los
  devuelve (Panel y barra de arriba dejan de contarlos); el informe sí los lleva. `Finding.key`.
- **Análisis rápido** (`run_diagnostics(quick)`): discos, estabilidad, drivers, batería y sistema; no
  se guarda ni se compara. **Progreso por partes** con lo que tarda cada una (interfaz).
- **Hallazgos nuevos**: Windows en disco mecánico (`PhysicalDisk.is_system`), Windows sin soporte
  (`support_end`, tabla de fechas por compilación y edición; LTSC y Server no se opinan), memoria
  corta (`perfhistory::memory_pressure`: media ≥ 85 % o más del 25 % del tiempo sobre el 90 %), disco
  que se llena (`disk_trend`: frente al análisis más antiguo de las últimas tres semanas; avisa si a
  ese ritmo quedan 45 días o menos) y errores de disco del registro (sucesos 7, 55 graves; 11, 51,
  153 a partir de cinco).
- **Tarjetas**: Discos y Sistema y seguridad pasan a resumen con enlace. Drivers se queda entera:
  es su único sitio.
- **Análisis guardados**: `to_prune` deja el último de cada día para lo anterior a 48 horas; no toca
  lo reciente ni el punto de partida de una sesión en curso.
- **Calibración**: `a_healthy_office_pc_raises_no_warnings` fija el presupuesto de ruido con un
  equipo inventado pero típico (sin datos reales): un PC de oficina sano no da ningún aviso.
  **Por comprobar**: las fechas de fin de soporte de Windows están escritas a mano en `support_end`;
  revisarlas contra la página de ciclo de vida de Microsoft al añadir versiones.

### v1.1.11 — Catálogo a medida, diálogos, y repaso de fallos, aspecto y orden (compilada)

**Instalar programas**, a partir de lo que pidió el supervisor: en una empresa no se instalan
Telegram ni juegos, y el catálogo tiene que poder ajustarse.

- **Vista «Empresa»** (la de fábrica) y **«Todo»**. 30 programas están marcados como de uso
  personal u ocio (`home = true` en `apps.toml`): mensajería personal, juegos, torrents,
  periféricos de jugador, edición de vídeo doméstica. En «Empresa» no salen.
- **Personalizar**: ocultar programas sueltos o categorías enteras. Se guarda en
  `app-catalog-view.json` con los datos del técnico (viaja en el pendrive). Lo oculto sigue
  apareciendo al buscarlo por su nombre, en un bloque aparte.
- **Catálogo**: de 113 a 147 programas. 34 nuevos de empresa, cada id comprobado con
  `winget show`: Microsoft 365 Apps, Firefox ESR, PDF24, PDF-XChange, PDF Arranger, NAPS2,
  draw.io, Power BI, Nextcloud, OpenVPN, Citrix Workspace, Horizon, Windows App, mRemoteNG,
  Remote Desktop Manager, Veeam, Duplicati, SyncBackFree, KeePass, PowerShell 7, SSMS, Postman,
  Java 21, .NET 10, y utilidades de HP, Brother, Jabra y Plantronics. Dos categorías nuevas: VPN y
  escritorios de la empresa, y Copias de seguridad. Lista «Puesto de empresa».
- **Vista**: filtro por categoría con sus cantidades, «Ocultar los ya instalados», y cuántos
  programas hay a la vista y cuántos ocultos.

**Diálogos**

Visto al usar la 1.1.10: el diálogo «Copia de seguridad cifrada» de Ajustes salía cortado por el
borde de su tarjeta.

- **Causa**: las tarjetas (`Card`) llevaban `contain: layout paint`, puesto en su día para pintar
  más barato. Con eso, un diálogo `fixed` que nace dentro de una tarjeta se coloca respecto a la
  tarjeta y se recorta en su borde. Lo mismo pasa dentro de una página con `@container` (Usuarios,
  Acceso remoto, Personas): el diálogo se centraba en la página entera, no en lo que se ve.
- **Arreglo de raíz**: una capa común (`Overlay` en `ui.tsx`) que se pinta en el `body`, fuera de
  quien la abre. La usan `Modal` (49 pantallas), las confirmaciones, el editor de perfiles y su
  importación. Las tarjetas dejan de recortar, así que tampoco se cortan los menús desplegables.
- Escape cierra solo la capa de arriba; los diálogos no se salen de una ventana estrecha.

**Repaso completo** (la lista del artifact de mejoras, grupos F, V, M y R)

- **Fallos (F)**: ejemplos con `empresa.com` en vez de un dominio real; las rutas de
  `solutionsCatalog.ts` con los nombres de hoy y una prueba (`solutionPaths.test.ts`) que lo vigila;
  GIMP solo una vez; los diálogos con el foco dentro (Tab no se escapa, vuelve al cerrar,
  `role="dialog"`); unos 66 `catch` vacíos pasan a `logQuietly` (registro técnico) o a un aviso;
  las nueve pruebas que leen el equipo real pasan a `#[ignore]` (se lanzan a mano: eran las
  intermitentes); «Buscar actualizaciones» sin versiones publicadas lo dice en claro
  (`UpdateInfo.published`).
- **Visual (V)**: anchos de página iguales (`max-w-6xl`); `iconBtn`, `smallBtn`, `softBtn`,
  `IconButton` y `Button size="sm"` en `ui.tsx` en lugar de botones a mano; fechas en
  `lib/format.ts` (`shortDate`, `fullDate`, `dateTime`, `timeOfDay`, `ago`) en lugar de 18
  ayudantes sueltos; el Panel con `Card` y `Button`, y su rejilla se adapta al ancho; buscadores y
  columnas que encogen en ventanas estrechas.
- **Mecánicas (M)**: Ajustes con guardado automático (600 ms tras el último cambio, y al salir);
  Acceso remoto comprueba los equipos al pulsar, no al abrir, y recuerda lo comprobado; Ctrl+K
  busca en la guía (`openHelpTopic`); Novedades se abren una vez por versión nueva; tamaños de
  carpetas compartidas recordados un día; `NeedsAdmin` con «Reiniciar como administrador» en ocho
  pantallas; Impresoras, Desinstalar, Inicio de Windows y la evolución de un cliente con
  `DataTable`.
- **Reorganizar (R)**: nombres del menú sin choques (Optimizar Windows, Usuarios y cuentas con
  tres pestañas, Herramientas de Windows, Herramientas de red, Historial del equipo; Preparar
  equipos en Administración). Archivos grandes partidos sin cambiar nada de lo que hacen:
  `lib/api.ts` → `lib/api/` (17 áreas e `index.ts`; los `import` siguen igual),
  `portals.rs` → `portals/` (navegación, correo, inicio de sesión, vistas),
  `diagnostics/report.rs` → `report/` (formato, estilo, comparación, secciones, correo) y
  `SettingsPage.tsx` → una sección por archivo en `pages/settings/`. Tipos: `types.test.ts`
  compara campo a campo las interfaces de `lib/api` con los structs de Rust del mismo nombre.
  No se generan con `tauri-specta`/`ts-rs`: habría que derivar en unos 300 structs y cambiar
  cómo se registran los comandos; la prueba caza el mismo fallo sin tocar el programa.

### v1.1.10 — Guía, novedades, términos, reportar fallos, y fuera Microsoft 365 (compilada)

- **Todo guardado en Git y subido**, con una etiqueta por versión (`v1.1.9`). Primera parte de la
  Fase 31.
- **Ayuda de AdminOps**, una ventana con seis apartados que se abre desde Acerca de (los cuatro
  botones bajo el logo), desde Ajustes → Acerca de y desde Ctrl+K:
  - **Guía**: 12 capítulos y 62 apartados. De cada pantalla, qué es, cómo se usa paso a paso y qué
    hay que tener en cuenta (lo que no se deshace, lo que necesita administrador), con «Abrir esta
    pantalla». Incluye cómo cuida AdminOps el equipo y un capítulo de responsabilidad y buenas
    prácticas. Una prueba comprueba que cada enlace lleva a una pantalla que existe.
  - **Glosario** (43 términos) y **preguntas frecuentes** (12). El buscador busca en los tres.
  - **Novedades**: lo que trae cada versión desde la 0.1, sin fechas (`lib/changelog.ts`). Una
    prueba exige que la primera entrada sea la versión actual: no se puede publicar sin escribirla.
  - **Términos de uso** (`src-tauri/terminos.txt`, una sola fuente): licencia, autorización sobre
    los equipos, qué no se deshace, sin garantía, limitación de responsabilidad y datos. En la
    ayuda, en el **instalador** (no instala sin aceptarlos; se leen dentro) y en el instalador
    clásico (página de licencia).
  - **Reportar un problema**: qué pasó, cómo se repite y cómo contestar. Prepara un correo para el
    autor con el paquete de soporte adjunto (ahora lleva `descripcion.txt`) y lo abre en el
    programa de correo; o sin adjunto, para correo web. AdminOps no envía nada por su cuenta.
- **Fuera Microsoft 365 (Graph)**: dependía de que la organización registrase una aplicación.
  Se quitan `graph.rs` y sus 17 comandos, la tarjeta de Ajustes, los inicios de sesión y el MFA en
  Personas, el calendario de Outlook en la Agenda, la presencia y las fotos en Contactos y
  Personas, las incidencias en «Hoy», el aviso por Teams al cerrar un caso y su parte de la
  configuración de empresa. El Correo y Teams (las webs dentro de AdminOps) siguen igual. Los
  datos guardados antes (visitas enlazadas, archivos de empresa) se siguen leyendo.
- **README** al día en español e inglés: ya no enseña capturas de la 0.10 ni dice 1.1.2.
- **Tickets e Inventario web navegan con libertad**, como un navegador: la dirección guardada es
  solo la página de inicio. Limitar la navegación al sitio del portal rompía intranets reales (el
  inicio de sesión en otro servidor, enlaces entre sistemas). El Correo, Teams y los routers sí se
  quedan en sus sitios. Las páginas siguen sin acceso a las operaciones de AdminOps.
- **Barra lateral**: cinco áreas en vez de siete. Red y Datos pasan a **Equipo**. **Inventario**
  es una página propia en Soporte (el inventario propio y la web de inventario); «Puestos» se queda
  en Administración. Soporte va en este orden: Agenda, Teams, Correo, Contactos, Tickets,
  Inventario, Personas, Soluciones. Quien personalizó la barra conserva la suya (se restablece en
  Ajustes → Navegación).
- **Actualizar desde la aplicación** (Ajustes → General → Actualizaciones): descarga el instalador
  de la versión nueva desde las versiones publicadas del repositorio, comprueba tamaño y huella, y
  lo abre; en el portable, deja el .zip en Descargas. Nada se instala en silencio. **Requiere
  publicar cada versión en GitHub Releases con sus tres archivos** y que el repositorio sea
  público: a día de hoy no hay ninguna publicada, así que la función dice «todavía no hay ninguna
  versión publicada».
- **Plantilla de informe propia** (Ajustes → Informes y cobros): qué secciones lleva el PDF y en
  qué orden, con o sin detalle técnico. Aparece como tercera opción al generar un informe.
- **Exportar el diario de cambios** a PDF o a Excel desde Historial; con un filtro puesto, exporta
  lo filtrado.
- En la vista de mes, «y N más» contaba también los seguimientos, que ya salen con su reloj.

### v1.1.9 — Lavado de cara y repaso de fallos

Nueve pantallas repasadas, por lotes, y una revisión de lo que fallaba en silencio. Incluye lo de
abajo que estaba «sin build» (Tickets, Carpetas compartidas, Agenda, Personas y clientes).

**Pantallas**

- **Usuarios locales**, como Clientes: cifras arriba (usuarios, administradores activos,
  desactivadas, con algo que revisar; las tres últimas filtran), buscador, lista con avatar y
  última entrada, y una ficha por cuenta con sus datos y sus acciones con nombre. **Lo que conviene
  revisar** de cada cuenta sale arriba de la ficha con el botón que lo arregla: contraseña caducada
  o a punto, administrador sin contraseña, «Administrador» o invitado integrados activos, y cuentas
  que nadie usa desde hace seis meses (`lib/userIssues.ts`, con sus pruebas).
- **Procesos**: agrupados por programa (un navegador cuenta como uno, con la suma de todos; se
  despliega para ver cada proceso), barras de CPU y memoria, «lo que más pesa ahora» arriba,
  filtros «de quien usa el equipo / de Windows» y **finalizar el programa entero**. Un fallo al
  releer ya no saca un aviso cada dos segundos.
- **Acceso remoto**: barra de conexión rápida (un nombre y Conectar; usuario y opciones a un
  clic), conexiones guardadas como fichas ordenadas por uso, con buscador y un punto que dice si
  el equipo **contesta** (se comprueba al abrir, de dos en dos), y por qué no cuando no.
- **Sesión de servicio por pasos**: Motivo → Trabajo → Informe → Cobro → Firma y cierre, con lo que
  lleva cada paso a la vista y pudiendo saltar a cualquiera. El cierre ya no es una ventana
  aparte: avisa de lo que falta (motivo, checklist, observaciones) antes de generar el informe.
- **Red, una sola página** con cuatro pestañas: Red y router · Dispositivos · Velocidad y
  diagnóstico · Herramientas. Los enlaces antiguos llevan a su pestaña.
- **Diario de cambios** como línea de tiempo: por días (Hoy, Ayer, fecha), con buscador, filtros
  (ajustes, acciones, deshechos, con error, se pueden deshacer) y cuántos cambios quedan por poder
  deshacer.
- **Tabla común** (`components/DataTable.tsx`): misma cabecera y densidad, y **ordenar por
  columna**, en Dispositivos, Estaciones, Herramientas de red (ping/traceroute y puertos),
  Velocidad (historial y diagnóstico), SMART de Hardware y discos del Diagnóstico. Las IP se
  ordenan como números y lo vacío va siempre al final.
- **Cargas, vacíos y errores iguales en toda la app**: `Loading`, `EmptyLine`/`EmptyState` y
  `ErrorState` (con «Reintentar»).
- **Ajustes**: ya tenía buscador y secciones, así que lo que se hizo fue arreglar el buscador:
  8 ajustes que no se encontraban, 4 entradas que apuntaban a nombres que ya no existen, y ahora
  lleva y marca también los bloques enteros, no solo las filas. Una prueba compara el índice con la
  pantalla para que no se vuelva a desfasar.

**Fallos corregidos**

- **Páginas que se quedaban en «Leyendo…» para siempre** si la primera lectura fallaba: Usuarios,
  Procesos, Impresoras, Perfiles, Inicio de Windows, Ajustes de Windows, Desinstalar, Bloatware,
  Instalar, Herramientas, Carpetas compartidas, Sesión de servicio, Ajustes y la lista de copias
  de drivers. Ahora dicen qué pasó y dejan reintentar.
- **Carpetas compartidas**: los permisos del disco se ponían archivo por archivo (`icacls /T`):
  lento en carpetas grandes (podía agotar el tiempo y dejarlo a medias) y, al quitar a la persona
  después, los permisos sueltos de cada archivo se quedaban. Ahora es un solo permiso heredable en
  la carpeta. Al compartir o cambiar permisos se da más tiempo.
- **Tickets**: los botones «Entrar con Microsoft / Google / Okta…» se admiten de fábrica (son un
  enlace que pulsa el usuario y no se distinguían de un enlace a otro sitio); al guardar un portal
  se olvidan los sitios aprendidos en la sesión.
- **Copiar al portapapeles** sin avisar si fallaba, en 8 sitios.
- Diario, Informe, Panel y editor de perfiles: lecturas sin gestionar el error.

### Tickets: el inicio de sesión ya no se corta al salir del dominio (hecho, sin build)

Un portal solo navegaba por su propio nombre y sus subdominios; cualquier otra dirección se
cancelaba y se abría en el navegador de fuera. Eso rompía el inicio de sesión de las intranets que
lo tienen en otro sitio (`intranet.empresa.com` → `sso.empresa.com`, nombre corto → nombre largo o
IP, un proveedor de identidad externo): al enviar las credenciales, la página de destino se abría
fuera, sin la sesión ni los datos del formulario, y solo enseñaba un error. Además no quedaba
rastro en el registro.

- **Misma casa**: valen también los hermanos del dominio (`*.empresa.com`, sin abrir terminaciones
  que no son de nadie como `com.do` o `gob.do`), y para una intranet por nombre corto o IP privada,
  su nombre largo y el resto de la red interna.
- **Lo que es parte de entrar pasa siempre** (`guard_navigation`, con lo que WebView2 sabe de cada
  navegación): redirecciones del servidor, envío de formularios y navegación de la propia página.
  El sitio al que se llega así queda admitido para ese portal mientras AdminOps esté abierta.
  Solo un enlace que pulsa el usuario hacia otro sitio se abre fuera.
- **Aviso con arreglo**: cuando algo se abre fuera, una barra lo dice con el nombre del sitio y
  «Permitir en este portal» (queda guardado en sus dominios, sin recargar la vista).
- Todo se anota en el registro técnico (solo el nombre del sitio, nunca la dirección entera).

### Carpetas compartidas a fondo (hecho, sin build)

Módulo nuevo `shares.rs` y la pestaña «Carpetas compartidas» rehecha. Todo con los mecanismos de
Windows y el usuario de quien usa el equipo: AdminOps no guarda contraseñas ni toca otros equipos.

- **Quién puede entrar, sin volver a compartir**: por carpeta, cambiar el permiso de cada cuenta
  (leer / leer y modificar / control total), quitarla o añadir otra. Al dar acceso se ajustan
  también los permisos del disco (nunca en un disco entero ni en carpetas de Windows).
- **«¿Por qué no puede entrar?»**: carpeta + persona → lo que deja la compartición, lo que deja el
  disco y lo que puede de verdad (la intersección), con cada causa en claro: red pública, firewall,
  carpeta que ya no existe, cuenta que no existe, desactivada o sin contraseña, cuenta de
  Microsoft, un «Denegar», no estar en la lista. Con el botón que lo arregla al lado.
- **Unidades de red de este equipo**: las que Windows recuerda, con si el equipo que las sirve
  contesta; reconectar, abrir, quitar y conectar una nueva. Elevado, se le pide al escritorio de
  Windows que ejecute `net use`, porque las unidades son de la sesión del usuario.
- **Qué comparte otro equipo**: nombre o IP → sus carpetas e impresoras (lo mismo que enseña el
  Explorador en `\\EQUIPO`), conectar como unidad, copiar la ruta o abrir.
- **Copiar la ruta y las instrucciones**: `\\EQUIPO\Carpeta` y un texto listo para mandar.
- **Revisión de riesgos**: «Todos: control total» (con «Bajarlo a leer y modificar»), disco
  entero, carpetas personales (perfil, Escritorio, Documentos, Descargas) y carpetas de Windows.
- **Tamaño y espacio**: cuánto ocupa cada carpeta y aviso si al disco le queda menos del 10 % o
  de 10 GB.
- **Copia diaria a otro disco**: tarea programada de Windows con robocopy (`/E`: lo nuevo y lo
  cambiado, nunca borra en la copia), con hora, última copia, si falló y «Copiar ahora».

### Historial de la Agenda, y Personas y Clientes juntos y rediseñados (hecho, sin build)

- **Agenda → Historial** (tercera vista, junto a Lista y Mes): todo lo hecho, lo cancelado y lo que
  pasó sin marcarse, con los seguimientos hechos. Buscar, filtrar por estado y por tipo, y por cada
  mes cuántas se hicieron y cuánto tiempo. Arriba: hechas este mes, tiempo este mes, hechas este
  año y sin marcar. Cada cosa se puede volver a dejar pendiente, marcar como hecha, **repetir**
  (abre el editor con una copia) o borrar. Se guarda cuándo se marcó como hecha.
- **«Atrasado»** arriba de la lista: lo que se quedó sin marcar (su día pasó y no está ni hecho ni
  cancelado) ya no se esconde en «lo pasado»: sale primero, con su fecha, para marcarlo, pasarlo a
  hoy o cancelarlo.
- **Personas y Clientes, una sola página** con dos pestañas (lo que abría «Clientes» lleva a su
  pestaña). **Clientes**: cuatro cifras arriba (clientes, equipos, mantenimiento cerca o vencido
  —filtra al pulsarla— y garantías vigentes), lista con avatar y última visita, y la ficha con
  cabecera (Empezar sesión, Agendar —abre la Agenda con ese cliente—, teléfono y correo para
  copiar, y cuatro datos clave) y pestañas **Resumen · Equipos · Visitas · Datos y plantilla**, en
  vez de todo apilado con el formulario delante. **Personas**: buscador con el botón dentro,
  **recientes** (las últimas 8 personas abiertas, a un clic), resultados con avatar y estado, y un
  **aviso arriba de la ficha** cuando la cuenta tiene un problema (bloqueada, contraseña caducada,
  desactivada, caduca pronto) con el botón que lo arregla ahí mismo.

### Listo para el equipo y Agenda de mes (Outlook y la presencia de Teams se quitaron en la 1.1.10)

- **Fase 34, cerrada en lo que depende del código**:
  - **Primeros pasos** en el Panel (modo técnico): tus datos y los de la empresa, portales,
    Microsoft 365, bloqueo con PIN y copia automática, cada uno con su botón y su casilla que se
    marca sola. Se oculta al completarlo; vuelve desde Ajustes → General.
  - **Configuración de empresa exportable** (Ajustes → General, y en Primeros pasos): datos de la
    empresa (logo, condiciones, precios, impuestos, tipos de visita, checklist), portales, dominio y
    la aplicación de Microsoft 365, en un .json. Al importar se elige qué aplicar; los portales que
    ya existen no se duplican. **Nunca** lleva contraseñas, sesiones, el nombre ni la firma del técnico.
  - **Paquete de soporte** con `resumen.txt`: versión, modo (pendrive/instalado), dónde va el
    navegador, Microsoft 365, los últimos arranques con sus tiempos y los últimos avisos y errores.
  - **Manual del técnico** (`docs/MANUAL.md`), **guía de instalación para IT**
    (`docs/INSTALACION-IT.md`: requisitos, permisos, conexiones, Microsoft 365, antivirus) y **lista
    de componentes de terceros** (`docs/TERCEROS.md`, generada con `scripts/terceros.py`: 535 de
    Rust y 7 de la interfaz, ninguno GPL). La CI ejecuta `cargo audit` y `npm audit`.
  - Falta lo que no es código: las capturas del manual y que otro técnico lo use una semana.
- **Agenda: vista de mes** (seis semanas, lunes a domingo) con **arrastrar para cambiar de día**,
  doble clic para apuntar, «y N más» que abre ese día en la lista. **Outlook en los dos sentidos**:
  al abrir la Agenda se trae lo que se movió en Outlook (hora, duración, sitio) de lo que salió de
  aquí, y si se borró allí la visita se queda sin enlace y se dice; lo que se mueve o edita aquí se
  actualiza solo en Outlook; y lo demás de tu calendario de Outlook se ve en gris, en la lista y en
  el mes, para tener un solo calendario. Se puede mover hasta un año antes o después.
- **Presencia de Teams y fotos de Microsoft 365** en Contactos (tarjetas, tabla, directorio,
  marcación rápida y ficha) y en Personas: punto de color (disponible, ocupado, ausente, no
  molestar…) y «Teams: Ocupado · en una reunión». La presencia se refresca cada 2 minutos con la
  página a la vista, en lotes de 20 ($batch); las fotos se guardan una semana (y quien no tiene,
  también, para no volver a pedirla). Permisos nuevos: `Presence.Read.All`, `User.ReadBasic.All`.

### v1.1.8 — Discos y pendrive a fondo, e informe profesional (hecho)

- **Correo y Teams «no llegó a abrirse» sesión tras sesión: la causa de verdad.** Al cerrarse
  AdminOps de golpe (el instalador lo cierra para actualizar, un cuelgue), sus procesos de WebView2
  seguían vivos con el perfil abierto; el AdminOps siguiente se enganchaba a ellos y las vistas no
  arrancaban nunca. Ahora, al abrir, antes de crear ninguna vista, se cierran los procesos de
  WebView2 con perfil de AdminOps si no hay otro AdminOps abierto (y se anota en el registro); el
  instalador también los cierra. Además, la vista que tarda en arrancar ya no se destruye a los
  25 s (en un equipo cargado se destruía justo antes de llegar y así nunca llegaba): se espera un
  minuto.
- **El instalador comprueba que de verdad actualizó.** Si NSIS no podía sobrescribir
  `adminops.exe` (algo lo tenía abierto), terminaba sin error; y como las dos builds tenían la misma
  versión, se daba por actualizado y se seguía usando el programa anterior (pasó con la corrección
  del navegador). Ahora compara la huella SHA-256 del `adminops.exe` instalado con la del que lleva
  dentro; si no coincide, cierra todo, reintenta y, si sigue igual, lo dice.
- **Corrección tras probar la build en el pendrive**: Correo y Teams se quedaban en blanco y
  AdminOps tardaba 38 s en abrir. El perfil del navegador interno estaba en el pendrive, y Teams y
  Outlook escriben miles de archivos pequeños: en un pendrive se arrastran. Ahora, con AdminOps
  instalado en el pendrive, el navegador va al disco de cada equipo (donde sus sesiones valen de
  todas formas: Windows las cifra por equipo). Los datos siguen en el pendrive. El portable (.zip,
  para equipos de clientes) sigue guardándolo en el pendrive para no dejar rastro; se cambia en
  Ajustes → Datos. Las DLL de temperaturas se copian una vez al disco del equipo (la primera lectura
  tardaba 17 s desde el pendrive) y el tamaño de los datos ya no recorre el perfil del navegador.

- **La clave del pendrive, con tu PIN.** Con el bloqueo de AdminOps activado, la clave que cifra
  las contraseñas guardadas (portales, routers, Microsoft 365) se guarda a su vez cifrada con el
  PIN (PBKDF2 de 210 000 vueltas + AES-256-GCM). Al desbloquear se descifra y queda solo en
  memoria; quien se lleve el pendrive no puede leerlas. Las claves que ya existían se protegen la
  primera vez que se desbloquea con el PIN. Si se olvida el PIN, esas contraseñas no se recuperan
  (se dice en Ajustes → Seguridad).
- **Tendencia de cada disco**: una foto diaria de sus cifras de desgaste (sectores apartados,
  pendientes, no corregibles, errores de conexión y de lectura, desgaste). La tarjeta enseña su
  evolución; si algo sube, el veredicto pasa a «Va a peor» aunque las cifras sean bajas, y con
  AdminOps abierto se revisa una vez al día y avisa.
- **Medir velocidad** de cada disco: 256 MB escritos y leídos sin caché de Windows y 3 s de
  lecturas aleatorias, con lo que es normal para su tipo (USB 2.0, disco mecánico → «cámbialo por
  un SSD», NVMe lento…).
- **¿Capacidad real?** (pendrives falsos, como H2testw): llena el espacio libre con datos que
  dependen de su posición y los vuelve a leer sin caché. Dice si la capacidad es real o desde qué GB
  se pierde lo escrito. No toca lo que había y borra la prueba al acabar; cancelable.
- **Expulsar** (dice qué programa lo tiene abierto si no se puede) y **formatear** (exFAT
  recomendado, FAT32 solo hasta 32 GB, NTFS; hay que escribir la letra para confirmar) unidades
  extraíbles y discos USB; nunca la de Windows ni aquella desde la que corre AdminOps.
- **BitLocker en cada volumen** (activo, en pausa, cifrando…) y su **clave de recuperación**, con
  constancia en el diario de que se consultó.
- **Copia automática** de los datos a OneDrive u otro disco cada N días (cifrada, con las últimas N
  copias), solo en el equipo donde se configuró. **Salud del pendrive** una vez al día: sistema de
  archivos dañado, FAT32, poco espacio o más de 15 días sin copia → aviso.
- **Informe profesional**: la tipografía va dentro del PDF (se ve igual en cualquier equipo),
  **estado por áreas** (discos, espacio, seguridad, actualizaciones, estabilidad, dispositivos,
  memoria, temperatura, batería) con semáforo, la nota de seguridad en un anillo, los pendientes con
  su prioridad (Urgente / Recomendado) y los urgentes primero, el veredicto de cada disco con las
  mismas palabras que Discos, cabecera (equipo y fecha) y pie en todas las páginas desde la segunda,
  y una nota final con de dónde salen los datos. Lo que no se pudo medir sale «Sin datos», nunca
  «Bien».

### v1.1.7 — AdminOps en el pendrive y Discos (hecho, sin probar en otro equipo)

- **Instalado en el pendrive, los datos viajan con él.** Si el programa está en una unidad
  extraíble, AdminOps se pone solo en modo portable (todo en `AdminOps-data` junto al programa); en
  cualquier otra carpeta, Ajustes → Datos → «Guardar todo en la carpeta del programa». Actualizar es
  volver a pasar el instalador: su desinstalador solo borra los archivos que instaló (se comprobó
  en el script NSIS generado), así que `AdminOps-data` se conserva.
- **La primera vez se trae lo de este equipo**, al arrancar y antes de abrir ninguna vista web:
  ajustes, clientes, contactos, agenda, portales, Microsoft 365…; lo propio del equipo (diario,
  diagnósticos, ventana) a `equipos\<equipo>`. Las contraseñas cifradas con DPAPI (solo legibles en
  este equipo) se cifran de nuevo con la clave del pendrive; también la sesión de los portales de
  este equipo. Queda anotado (registro y Ajustes).
- **Un perfil del navegador por equipo dentro del pendrive.** Windows cifra las sesiones de los
  portales para cada equipo: con una sola carpeta compartida, cada cambio de PC obligaba a entrar de
  nuevo en todo (y otra vez al volver). Ahora se entra una vez en cada PC y se mantiene. Llevarse
  la sesión de un equipo a otro no se hace: es lo que Microsoft detecta como robo de sesión. Las
  cuentas guardadas para «Entrar solo» sí viajan (cifradas con la clave del pendrive).
- **El instalador encuentra el AdminOps del pendrive** en un equipo que no lo tiene registrado (o
  si el pendrive cambió de letra) y lo actualiza en esa misma carpeta.
- **Discos** (antes «Espacio en disco»), con una pestaña nueva, **Salud y reparación**: un veredicto
  por disco con lo que se estropea primero (superficie: sectores pendientes o no corregibles,
  errores de lectura, aviso del propio disco → «copia ya y no pases chkdsk /r antes»; conexión: CRC
  → cable, puerto o caja USB; sistema de archivos marcado como dañado → el «Reparar disco» de
  Windows; sectores ya apartados; desgaste de SSD; temperatura). Por volumen: **Comprobar** (sin
  cambiar nada, `Repair-Volume -Scan`), **Reparar sistema de archivos** (`-OfflineScanAndFix`; en el
  de Windows se programa al reiniciar) y **Buscar sectores dañados** (chkdsk /r con progreso y
  cancelable; no en el de Windows). **Rescatar archivos**: copia a otro disco todo lo que se lee
  (robocopy con un reintento de 1 s en vez de un millón) y lista lo que se quedó. El estado sucio se
  lee con `FSCTL_IS_VOLUME_DIRTY` (sin depender del idioma). Reparar, chkdsk /r y rescatar van al
  diario y el modo auditoría los bloquea.

### Agenda para todos, Contactos, fuera la Asistencia rápida y cuatro APIs de Windows (hecho)

- **La Agenda ya no depende de tener clientes.** Cada cosa es una **tarea, llamada, reunión o
  visita**, con cliente o sin él (quien trabaja en una sola empresa no tiene «clientes»): basta un
  título. Arriba, **apuntar en una línea** (qué + hoy/mañana/pasado/otro día + hora, Enter); si no
  se pone hora, hoy la siguiente en punto y otro día las 9:00. **La semana de un vistazo** (siete
  días con un punto de color por cosa; al pulsar uno, solo ese día). A la izquierda, **lo próximo de
  hoy** («En 25 min: …») y **lo de mañana**, con los **seguimientos** (los de la nota de llamada)
  metidos en su día, con «Hecho» y «Mañana». «Toca mantenimiento» solo sale si hay clientes. En
  cada fila, una franja del color del tipo; «Empezar» (sesión de servicio) solo con cliente. Los
  avisos y el evento de Outlook usan el título. Las visitas antiguas siguen igual (sin tipo =
  visita).
- **Contactos, más útil de un vistazo**: seis cifras arriba (contactos, favoritos, con extensión,
  empresas, usados este mes, sin completar) que filtran al pulsarlas; **marcación rápida** con los
  favoritos y los más usados (extensión en grande, llamar, Teams, correo y copiar al pasar el
  ratón); **avatares** con las iniciales y un color estable por persona en tarjetas, tabla y
  directorio; tarjetas con la disponibilidad y una barra de acciones; la ficha con una cabecera del
  color de la persona, la extensión en grande y cuatro botones (Llamar, Correo, Chat, Tarjeta).
- **Fuera la Asistencia rápida** de toda la app (Microsoft la retiró): el botón de Acceso remoto
  abre ahora la **Asistencia remota de Windows** (`msra.exe`); fuera también su atajo del catálogo
  de herramientas, su ficha en «Apps preinstaladas», sus atajos de teclado y las menciones en los
  textos de ayuda y documentación.
- **Tapar datos personales en los recortes, con el OCR de Windows** (`Windows.Media.Ocr`, sin nube).
  Al recortar desde AdminOps (barra del caso o Ctrl+K), en cuanto la imagen llega al portapapeles se
  leen sus palabras y se tapan las rutas de perfil (`C:\Users\…`) y las que llevan el usuario, la
  carpeta del perfil, el equipo o el dominio; la imagen tapada vuelve al portapapeles y se dice
  cuántas se taparon. Para capturas hechas fuera: Ctrl+K → «Tapar datos personales del
  portapapeles». El portapapeles se lee y escribe con Win32; WinRT solo para el OCR. Si Windows no
  tiene el reconocimiento de texto del idioma, se dice y el recorte queda como estaba.
- **El Visor de eventos, en tiempo real**: la vigilancia de errores se suscribe (`EvtSubscribe`) a
  los eventos que vigila y hace la pasada en cuanto Windows escribe uno (esperando 3 s a que lleguen
  los que van juntos), en vez de preguntar cada minuto con PowerShell. El espacio libre y los
  dispositivos, que no dejan evento, cada 10 minutos. Si Windows no deja suscribirse, cada minuto
  como antes (se dice en el registro).
- **Avisos de seguimientos con botones**: «Hecho» y «Mañana» en el propio aviso de Windows, sin
  abrir AdminOps; pulsar el aviso trae AdminOps delante. Uno por seguimiento si son hasta tres; más,
  un resumen. Si Windows no deja mostrar botones, el aviso de siempre.
- **API COM de winget: no se hizo.** Windows solo registra los intermediarios COM de winget
  (Microsoft.Management.Deployment) para aplicaciones empaquetadas (MSIX); desde AdminOps falla con
  «Failed to find proxy registration» (0x80073D54), y además Microsoft no la admite desde procesos
  elevados. Se probó con enlaces generados del propio `.winmd` y se quitó para no dejar código
  muerto. Alternativas: distribuir el paquete de interoperabilidad COM de Microsoft junto a
  AdminOps (más peso y atado a sus versiones), o usar el módulo Microsoft.WinGet.Client de
  PowerShell cuando esté instalado. Mientras tanto, winget sigue por la consola (los ids instalados
  ya salen del JSON de `winget export`, que no depende del idioma ni del formato de la tabla).

### Fase 30 — Probar la aplicación en marcha

**Prioridad: máxima.** Es lo que habría parado los cinco fallos de arriba antes de llegarte.

- **Pruebas de humo sobre el `.exe` compilado**, con WebDriver (`tauri-driver` + `msedgedriver`):
  arranca, pinta la primera página en menos de N segundos, recorre las 34 pestañas sin errores en la
  consola, y cierra limpio.
- **Portal de pruebas propio**: un servidor web mínimo que levanta la propia prueba, para comprobar
  que un portal carga, se oculta, se vuelve a mostrar y conserva la sesión, sin depender de Microsoft
  ni de la intranet.
- **«¿Se puede pulsar?»**: después de abrir un portal, ir a Ajustes y pulsar un botón. Es exactamente
  el fallo de la capa invisible.
- **Capturas de las páginas clave en los dos temas**, comparadas con las de referencia. Es el fallo
  del modo claro. `scripts/bench.ps1 -Page` ya sabe sacar capturas; se aprovecha.
- **Arranque en frío del portable** en Windows Sandbox (`tests/sandbox` ya lo hace para la prueba de
  ida y vuelta): carpeta nueva, primera vez, sin nada guardado.
- Todo en la CI, que falla si algo de esto falla.

**Terminado cuando:** la CI no deja pasar una versión que no arranca, en la que un portal no carga, en
la que algo tapa los clics, o en la que un tema se ve roto.

### Fase 31 — Publicar con disciplina

**Prioridad: alta, y la primera parte es de hoy mismo.**

- **Guardar en Git lo que ya existe** y, a partir de ahí, un commit por cambio y una etiqueta por
  versión publicada.
- **Una versión, un contenido**: cualquier build que salga de este equipo sube el número de versión
  sola. Nunca se sobrescribe una carpeta de `release\` que ya exista.
- **Novedades de cada versión** escritas para el técnico (qué cambia para él, no qué función se tocó),
  visibles en «Acerca de» y junto al instalador.
- **Canal de prueba**: cada versión pasa un día en tu equipo antes de llegar a nadie más.

**Terminado cuando:** de cualquier AdminOps instalado se puede saber exactamente qué código lleva, y
volver a la versión anterior es copiar una carpeta.

### Fase 32 — Un solo contrato entre la interfaz y el programa

**Prioridad: alta.** Quita de raíz una clase entera de fallos.

- **Tipos generados desde Rust** (`tauri-specta`): los datos y los comandos se definen una sola vez y
  la interfaz los recibe generados. *En 1.1.11, en su lugar, `types.test.ts` compara los campos de
  los dos lados.*
- **Partir los archivos enormes**: *hecho en 1.1.11* con `api.ts`, `portals.rs`, `report.rs` y
  `SettingsPage.tsx`. Queda `Tickets.tsx`.
- **Los 9 avisos de promesas sin capturar** que quedan: decidir en cada uno qué hacer con el error.

**Terminado cuando:** no hay ningún tipo escrito dos veces y ningún archivo pasa de ~600 líneas.

### Fase 33 — Pulido de la interfaz

**Prioridad: media.** Lo que hace que parezca un producto y no un proyecto.

- **Los dos temas revisados pantalla por pantalla**, con contraste suficiente para leer sin esfuerzo.
- **Los mismos estados en todas partes**: cargando, vacío, error y «necesita administrador», con el
  mismo aspecto y el mismo tono en las 34 pestañas.
- **Cada error dice qué hacer**, no solo qué pasó. Revisar los que todavía llegan en crudo de Windows.
- **Todo se puede hacer con el teclado**, con el foco siempre visible.

**Terminado cuando:** una lista de comprobación de las 34 pestañas en los dos temas pasa entera.

### Fase 34 — Listo para que lo use el equipo

**Prioridad: media**, y es lo que pide la presentación al supervisor.

- **Manual corto del técnico**, con capturas: lo que hace cada sección y los cinco usos más comunes.
- **Guía de instalación para IT**: requisitos, qué pide permisos y por qué, qué conexiones hace
  (sección 4.6 de `docs/PRESENTACION.md`), cómo añadirlo a la lista de confianza del antivirus.
- **Revisión de dependencias en la CI** (`cargo audit`, `npm audit`) y la lista de componentes de
  terceros, que IT va a pedir.
- **Paquete de soporte más útil**: versión, equipo, últimos errores y los tiempos de arranque, en un
  clic, para que un problema en otro equipo se pueda diagnosticar sin ir hasta él.

**Terminado cuando:** otro técnico lo instala y lo usa una semana sin tener que preguntarte nada.

### Después, cuando lo anterior esté cerrado

Funciones nuevas que siguen teniendo sentido, en este orden: **avisar cuando un ajuste lo controla la
directiva del dominio** (y el cambio no va a durar), **comparar un equipo con el equipo patrón**, y
**una carpeta de red compartida como base común** de clientes, contactos y visitas entre técnicos.

---

## Hecho (v0.1 – v1.1)

| Fase | Versión | Contenido |
|---|---|---|
| 1. Base | 0.1 | Tauri 2 + React + Tailwind, tema oscuro neón, UAC, panel del sistema en vivo |
| 2. Motor de ajustes | 0.2 | Catálogo TOML declarativo, detección de estado, diario con copia exacta, deshacer, puntos de restauración |
| 3. Catálogo | 0.3 | Limpieza, privacidad, rendimiento, servicios, bloatware (Appx) e Inicio; usuario destino para HKCU |
| 4. Diagnóstico | 0.4 | Discos, BSOD, drivers, batería, seguridad, reparaciones, informe PDF antes/después, ícono |
| 5. Pulido | 0.5 | Perfiles de un clic, modo portable, firma de código preparada, versión unificada |
| 6. Robustez y calidad | 0.7 | Timeouts y Cancelar, progreso en vivo, scripts sobre el usuario destino, registro de actividad, prueba de ida y vuelta, CI, diagnóstico interactivo |
| 7. Rendimiento | 0.7 | PowerShell persistente, Inicio por COM, carga diferida de páginas, pintado más barato, WebView2 con GPU en proceso |
| 8. Herramientas | 0.9 | Procesos (finalizar proceso/árbol), red y speedtest, actualizar software (winget), analizador de espacio, copia de drivers, análisis de Defender |
| 9. Flujo del técnico | 0.9 | Sesión de servicio, clientes, perfiles propios (importar/exportar), marca en el informe, checklist, "by David Bonilla" |
| 10. Hardware | 0.10 | Inventario (placa, BIOS, RAM por ranura, GPU, monitores, clave OEM), temperaturas (LibreHardwareMonitor), SMART, prueba de RAM, speedtest con medidor animado e IP/proveedor |
| 11. Caja de herramientas | 0.11 | ~90 accesos rápidos a utilidades de Windows (favoritos y accesos propios), usuarios locales (crear, contraseña, admin/estándar, activar, eliminar con perfil), ficha rápida del equipo, redes Wi-Fi guardadas con contraseña |
| 12. Después de formatear | 0.12 | Instalar programas en lote (winget, listas propias), copia y restauración de datos del usuario, herramientas de red (ping/traza, DNS, puertos, hosts), impresoras, driver probable de los pantallazos azules |
| 13. Entorno corporativo | 0.13 | Tickets (portales web dentro de la app, incrustados o en ventana, con sesión y contraseña recordadas), dominio (estado, comprobaciones previas, unir, salir, reparar relación de confianza, renombrar), paracaídas de errores de la interfaz |
| 14. Orden y pulido | 0.14 | Búsqueda global Ctrl+K, barra lateral plegable con favoritos y grupos reordenados, asistente de primer arranque, ventana y última página recordadas, historial Alt+←/→, notificaciones al terminar tareas largas, portable sin rastro de WebView2, paquete de soporte |
| 15. Mantenimiento a fondo | 0.15 | Desinstalador con restos a la papelera, Windows Update (historial explicado, pausar, ocultar), análisis del arranque, restaurar drivers, caché de navegadores, reparaciones de Store, búsqueda, audio y perfil temporal, espacio de puntos de restauración, mantenimiento programado |
| 16. Seguridad del equipo | 0.16 | Nota 0-100 con arreglos, 8 ajustes de seguridad reversibles, BitLocker y copia de claves, programas de riesgo, elementos sospechosos, extensiones de navegador, nota en diagnóstico e informe |
| 17. Instalador y atajos | 0.17 | Instalador con interfaz propia (instalar/actualizar/reinstalar, progreso, consejos), instalador clásico para despliegues silenciosos, página de atajos de teclado con modo descubrir y prueba |
| 18. Rediseño | 0.18 | Lenguaje visual sobrio (IBM Plex, un acento, color solo para estados, sin brillos ni degradados), tema claro, navegación en 7 áreas con pestañas, Panel y Herramientas rehechos, icono nuevo, informe e instalador con el mismo estilo |
| 19. Informe y cliente | 0.19 | Informe PDF rehecho con dos plantillas (cliente y técnica), presupuesto o recibo con impuestos, firma del cliente y del técnico, envío por correo con adjunto, garantías, recordatorios de mantenimiento, evolución entre visitas, icono del medidor con llave |
| 20. Red y router | 0.20 | Mi red (conexión, router, DNS, IP pública), panel del router dentro de la app con acceso guardado y cifrado, chequeo de seguridad del router y la Wi-Fi, doble NAT/CGNAT, QR de la Wi-Fi, dispositivos en la red con fabricante, nombres propios y aviso de nuevos |
| 21. Caja fuerte y privacidad | 0.21 | Caja fuerte (.vhdx con BitLocker que se abre sin AdminOps), carpeta cifrada AES-256, borrado seguro y del espacio libre, checklist antes de vender, recuperar archivos borrados (Windows File Recovery), control parental (filtro DNS, sitios bloqueados, horario de uso) |
| 22. La oficina completa | 0.22 | Inventario de equipos con veredicto (bien/mejorar/renovar) y exportación a Excel, acceso remoto (Escritorio remoto, Asistencia rápida, Wake-on-LAN), carpetas compartidas y permisos, IP duplicadas, mapa de la red en la ficha del cliente |
| 23. Ajustes, rendimiento y red a fondo | 0.23 | Panel y diagnóstico más rápidos, Ajustes en pestañas, acento, tamaño, navegación a medida, bloqueo con PIN, copia de la configuración, identificación de dispositivos (tipo, marca, modelo), contraseña de la Wi-Fi, 26 herramientas nuevas |
| 24. Versión 1.0 | 1.0 | Las páginas conservan su contenido (Recargar/F5), atajos propios, favoritas en el Panel, punto de restauración configurable, inicio con Windows, limpieza de datos antiguos, aviso de versiones nuevas |
| 25. Pulido | 1.1 | Sin cierres por fallos internos, vigilancia de errores de Windows con explicación, acceso remoto a fondo (agenda, opciones, credenciales, AnyDesk/RustDesk/TeamViewer), navegación totalmente personalizable, 283 atajos |

### Fase 6 en detalle

- **Nada puede colgar la interfaz.** Todo proceso tiene límite de tiempo (120 s por defecto; las tareas
  declaran el suyo con `timeout`). SFC, DISM y WinSxS no tienen límite, pero se pueden **cancelar**:
  se termina el árbol de procesos completo y lo ya cambiado se deshace.
- **Progreso en vivo** (evento `task-progress`) en ajustes, perfiles ("3/10 · …"), apps y puntos de restauración.
- **Usuario destino en scripts:** todos reciben `$UserSid`, `$UserHive`, `$UserProfile`, `$UserTemp`…
  `cargo test` rechaza scripts del catálogo que usen `$env:TEMP`, `HKCU:` o `Clear-RecycleBin`.
- **Registro de actividad** con rotación (5 × 1 MB) y visor en Historial → "Registro técnico".
- **Errores legibles:** sin "At line: … ~~~~" ni CategoryInfo.
- **Apps:** las operaciones Appx van una a una y se reintentan si Windows está ocupado (corrige
  "Another operation on app packages is in progress").
- **Prueba de ida y vuelta:** `adminops.exe --roundtrip informe.json` aplica y deshace cada ajuste y
  falla si el registro o los servicios no vuelven exactamente a su estado. Corre en la CI y en
  Windows Sandbox (`tests/sandbox/run-roundtrip.ps1`).
- **CI** (`.github/workflows/ci.yml`): tipos, Clippy `-D warnings`, tests, instalador, portable y
  prueba de ida y vuelta; los artefactos quedan en cada ejecución.
- **Diagnóstico interactivo:** cada hallazgo lleva a su detalle, al ajuste que lo resuelve o a la
  herramienta de Windows (lista cerrada: Administrador de dispositivos, Confiabilidad, Update…).

### Fase 7: medido

Medido con `scripts/bench.ps1` y `cargo test --release bench -- --ignored --nocapture`
(Ryzen 5 5500, Windows 11 26200).

| Métrica | Antes | Después | Objetivo |
|---|---|---|---|
| Listar Inicio | 3 398 ms | **310 ms** | < 500 ms ✅ |
| Listar apps | 834 ms | **370 ms** | < 500 ms ✅ |
| 10 consultas PowerShell | 3 022 ms | **435 ms** | — |
| Detectar el catálogo | 329 ms | **70 ms** | — |
| Arranque hasta ventana | — | **~400–600 ms** | < 1 s ✅ |
| JS inicial | 308 KB | **253 KB** | — |
| RAM privada total | ~260 MB | **~175 MB** | ver nota |
| RAM del proceso de AdminOps | — | **~10 MB** | ✅ |
| CPU minimizada | — | **~0 %** | < 1 % ✅ |
| CPU con el Panel visible | 18 % de un núcleo | **~4–10 %** | ver nota |

**Notas honestas:**
- El objetivo de "< 80 MB" no es alcanzable con WebView2: su runtime ya ocupa ~165 MB por sí solo.
  AdminOps en sí usa ~10 MB. Bajar de ahí exigiría cambiar de motor de interfaz.
- La CPU con el Panel visible se midió con el PC en uso y varió mucho entre pasadas. Repetir la
  medición con el equipo en reposo antes de dar la cifra por buena.

---

## Deuda técnica conocida

- **La prueba de ida y vuelta aún no se ha ejecutado** en un Windows 11 cliente limpio: la CI usa
  Windows Server. Ejecutarla en Sandbox al menos una vez por versión.
- **Dependencia de Edge para el PDF.** Si falta, el informe cae a HTML.
- **Sin firma de código** (decisión: no se contempla por coste). SmartScreen avisará al instalar.
- **Solo español** (inglés en la Fase 28).
- **Microsoft 365 y SNMP sin probar contra un inquilino y una impresora reales.** Los mensajes se
  prueban con respuestas de ejemplo; falta verlo con los permisos de la empresa y una impresora de
  red de la oficina.
- **Descubrimiento en la red y cortafuegos**: las respuestas de mDNS y WS-Discovery llegan por UDP a
  un puerto efímero; si el cortafuegos de Windows las filtra, esas impresoras solo salen por puertos.
- **Notificaciones en portable**: sin instalador, Windows puede mostrarlas con otro nombre de app.
- **Tickets incrustados** usan la API "unstable" de Tauri (vistas hijas); si una versión futura la
  cambia, queda la ventana aparte como alternativa.

### v1.1.6 (segunda parte) — Portales: la raíz, no los síntomas ✅

**⚠ Diagnóstico equivocado, corregido en la cuarta parte.** Lo que sigue explica un cambio que rompió los portales; se deja escrito para que no se repita. **El fallo de Tickets en la oficina tenía causa, y era grave.** La ventana principal arranca WebView2
con `--auth-server-allowlist` cuando hay una intranet configurada; las vistas de los portales
arrancaban solo con los argumentos del archivo de configuración. WebView2 **no admite dos
configuraciones distintas en la misma carpeta de datos**, así que la vista del portal no llegaba a
crearse y la página no abría. Se daba únicamente donde hay un portal de intranet: en el trabajo, y en
ningún otro sitio. Ahora los portales leen los argumentos con los que arrancó de verdad el navegador
(`ARGS_ENV`), no una parte de ellos. Con prueba.

**La sesión ya no se pierde al salir.** Los portales de Correo y Teams se creaban con «sesión
privada» activada de fábrica, que es exactamente lo que borra la sesión al cerrar AdminOps y obliga a
repetir el inicio de sesión y la verificación del móvil cada vez. Se creaba así pensando en el equipo
del cliente, pero el caso normal es el equipo del técnico. Ahora se crean con la sesión guardada, y la
privada se marca a mano cuando toca. Los textos dicen lo que cuesta cada opción, no solo su ventaja.
De paso, con perfil persistente Teams y Outlook cachean sus archivos: la segunda carga es mucho más
rápida que la primera.

**Los portales dejan de tirarse a la basura.**
- Una vista sin usar se cerraba a la media hora, y volver a entrar costaba una carga completa de
  Teams o de Outlook. Media hora no es nada en una jornada. Ahora **4 horas** en el equipo del
  técnico; se mantienen los 30 minutos en equipos justos de recursos, que es donde la memoria pesa.
- Dormir la vista al ocultarla hacía que **ir y volver** entre el Correo y otra página costara un
  despertar cada vez. Ahora se duerme solo tras **3 minutos** sin verse: el ir y venir normal es
  instantáneo y solo se aparca lo que de verdad se dejó aparcado.

**Arranque**
- **La ventana se enseña cuando hay algo pintado, no antes.** Enseñarla a los 2,5 s pasara lo que
  pasara era lo que producía el rectángulo negro del portable: si WebView2 aún no ha pintado, lo
  único que se ve es una ventana vacía, y eso parece una aplicación colgada. Red de seguridad a los
  20 s para no quedarse nunca sin ventana, y recarga solo si WebView2 no cargó nada en 30 s.
- El registro anota ahora **cómo y cuándo se cierra la ventana**. Una aplicación que desaparece sola
  no dejaba ni rastro y no había forma de saber si la cerró el usuario, Windows, o se cayó ella.

### v1.1.6 (quinta parte) — El MFA de Microsoft vuelve a funcionar ✅

**«Sorry, we're having trouble verifying your account» al iniciar sesión en Teams o el Correo.** Lo
introdujo la 1.1.5: para ahorrar procesador, los portales ocultos se **congelaban** (`TrySuspend` de
WebView2), y uno precargado se congelaba nada más terminar su primera carga si todavía no se había
abierto. Sin sesión iniciada, esa primera carga termina en la página de inicio de sesión de
Microsoft, que se quedaba congelada ahí minutos. Esa página lleva un contexto de inicio de sesión que
se renueva mientras está viva; congelada, caduca, y al pedir el código Microsoft rechaza la
verificación.

**Se ha quitado la congelación entera**, no parcheado: los portales ocultos siguen vivos, como en la
1.1.4, que es cuando el MFA funcionaba. El propio WebView2 ya frena lo que no se ve. Se mantienen el
resto de mejoras de los portales, revisadas una a una para que ninguna toque el inicio de sesión.

**Aviso**: tras muchos inicios de sesión seguidos en poco tiempo (cada build, más la sesión privada
del Correo), Microsoft limita durante un rato el envío de códigos por SMS o llamada. Si justo después
de actualizar sigue fallando, es eso: esperar o usar Microsoft Authenticator.

### v1.1.6 (cuarta parte) — Corrección de los portales ✅

**Los portales se quedaban para siempre en «Abriendo…» (Correo, Teams y Tickets a la vez).** Lo
introdujo la propia 1.1.6. WebView2 exige que todas las vistas que comparten carpeta de datos se creen
con **opciones idénticas**. La ventana principal se crea con los argumentos del archivo de
configuración; la lista de dominios de la intranet va aparte, en una variable de entorno que WebView2
aplica por igual a todas las vistas. En la 1.1.6 se añadió esa lista también a las opciones de cada
portal, creyendo que faltaba: los portales quedaban distintos de la ventana principal y WebView2 se
negaba a crearlos, sin ningún error a la vista. Pasaba en cuanto había una intranet configurada.
El diseño original era el correcto y se ha restaurado, con una prueba de regresión que lo fija.

Además:
- **Un portal que no arranca ya no gira para siempre**: a los 25 s sin señal de vida lo dice, con
  Reintentar (que vuelve a crear la vista). Un fallo así no puede volver a pasar en silencio.
- **Ajustes no dejaba tocar nada (ni hacer scroll).** Consecuencia del mismo fallo: las vistas de los
  portales son ventanas del sistema que van por encima de la interfaz, y una vista que WebView2 no
  llegó a iniciar no pinta nada ni responde a «ocultar». Se quedaba invisible encima de la zona de
  contenido tragándose clics y scroll, y se notaba en la siguiente página que se abría. Ahora una
  vista que nunca arrancó **se destruye** al salir de su página o a los 25 s (`portal_reset`), en vez
  de intentar ocultarla. Distinguir «no arrancó» de «arrancó pero va lenta» es seguro: una vista sana
  avisa de que empieza a cargar en menos de un segundo.
- **AdminOps abre en el Panel.** «La última página que usé» venía de fábrica y quedaba guardada al
  tocar cualquier ajuste, aunque nadie la hubiera elegido. Ahora solo se respeta si el técnico la
  elige a propósito en Ajustes.

### v1.1.6 (tercera parte) — Agenda, Usuarios, Impresoras y Carpetas ✅

Cuatro apartados que se habían quedado cortos.

**Agenda** — era una lista de citas y poco más.
- **Visitas que se repiten** (semanal, quincenal, mensual, trimestral, semestral, anual). Al marcar
  una como hecha, **la siguiente se planifica sola**, contada desde la fecha prevista para que
  atender con retraso no desplace toda la serie. El mantenimiento periódico deja de depender de que
  alguien se acuerde de volver a apuntarlo.
- **Aviso de visitas que se pisan**: no impide guardar (a veces se solapan a propósito), pero se dice
  al guardar y no el día de la visita.
- **Aplazar en un clic** desde la propia fila, sin abrir el editor. Es lo que más se hace.
- **Dónde es** (sede, planta, sala) y **tipo de visita**, enlazado con los tipos de Ajustes.

**Usuarios** — **renombrar la cuenta** y editar su nombre completo y descripción, sin salir a
`lusrmgr.msc`. Con las mismas reglas de seguridad que el resto: no se renombran cuentas integradas ni
de Microsoft, ni una con la sesión abierta. Y avisa de lo que despista a todo el mundo: **Windows no
renombra la carpeta del perfil**, y cambiarla a mano lo rompe.

**Impresoras**
- **«Revisar»**: dice por qué no imprime, señalando el primer eslabón roto de la cadena (cola de
  Windows → impresora encendida y en red → estado del aparato → atascos). Distingue «Windows la tiene
  marcada como sin conexión» de «la impresora no responde en 192.168.1.30», que se arreglan de forma
  muy distinta. Con siete casos cubiertos por pruebas.
- **Buscar impresoras en la red**: prueba los puertos de impresión (RAW, IPP, LPD) sobre los equipos
  que ya responden, y marca cuáles no están instaladas aquí. Antes, saber qué impresoras había en una
  oficina era preguntar o ir mirando aparato por aparato.

**Carpetas compartidas**
- **Qué archivos están abiertos ahora mismo y quién los tiene**, con aviso de los bloqueados. Un
  archivo abierto no se puede mover, renombrar ni borrar.
- **Carpetas rotas**: se comparte una ruta que ya no existe y quien entra ve un error de Windows.
- **La trampa de los permisos**: compartida con «Todos» pero con los permisos del disco cerrados. El
  acceso real es lo que dejen los dos a la vez, y es la causa clásica de «acceso denegado» que nadie
  entiende.

**Deuda técnica**: de 178 avisos de promesas sin capturar a **9**. Los 169 arreglados eran llamadas
de usar y tirar que ya capturan por dentro; los 9 que quedan son cadenas `.then(…)` que hay que mirar
una a una y decidir qué hacer con el error.

### v1.1.6 — Que funcione en el equipo del cliente ✅

Todo esto sale de usar la 1.1.5 de verdad, en equipos que no son el de desarrollo.

**Consumo y cuelgues**
- **La precarga de portales era lo que más pesaba, y estaba mal planteada**: precargaba el último
  portal de *cada* tipo, hasta cuatro navegadores Chromium completos por detrás sin que el técnico
  hubiera pedido ninguno (y Outlook y Teams son de las webs más pesadas que existen). Ahora precarga
  **uno solo**, el de la última página usada, **ninguno en equipos justos de recursos**, y a los 8 s
  en vez de a los 4.
- **El Panel recorría los ~250 procesos del equipo cada 2 segundos** para enseñar los 10 que más
  consumen. Esa parte pasa a cada 6 s; CPU, memoria y red —lo que se ve moverse— siguen en cada tic.
- **El vigilante del arranque recargaba la interfaz a los 12 s**, y en un equipo lento la primera
  pantalla tarda más que eso: se recargaba sola y volvía a empezar. Era la causa de «deja de
  funcionar». Ahora la interfaz avisa en cuanto su código arranca (`ui_booting`) y **solo se recarga
  si WebView2 no ha cargado nada** en 25 s.
- **PowerShell**: de 5 procesos a 2 en equipos de 4 GB o ≤4 núcleos (60-100 MB cada uno).
- Criterio único de «equipo justo de recursos» (4 GB o menos, o 4 núcleos o menos) en
  `src/lib/machine.ts` y su gemelo en `pspool.rs`. El arranque lo deja en el registro.

**Asistencia rápida**
- Seguía saliendo el cuadro en inglés de Windows. La causa: en Windows 11 queda la clave del
  protocolo `ms-quick-assist` con su `URL Protocol` pero **sin ninguna subclave**, o sea sin programa
  que lo abra, y la comprobación la daba por buena. Ahora se exige `shell\open\command`, y se
  prueba primero `quickassist.exe`, que es el que nunca falla. El mismo arreglo cubre `msteams:`.

**Texto**
- **Se puede leer el texto cortado**: al pasar el ratón por algo recortado con «…» se enseña entero.
  Resuelto de una vez para toda la aplicación (`src/lib/fullTextOnHover.ts`) en lugar de añadir el
  `title` en los casi cien sitios donde pasa y olvidarlo en los siguientes.
- **Las cajas con su propio scroll crecen con la ventana** (`pane-sm`, `pane-md`, `pane-lg`). Con un
  alto fijo en píxeles, en una pantalla grande sobraba sitio y en un portátil el contenido quedaba
  metido en una rendija con dos barras compitiendo.

**Reparar la red**
- Deja de ser una lista de pasos y **dice qué pasa y qué hacer**. Señala el primer eslabón roto de la
  cadena (tarjeta → IP → router → Internet → DNS), que es distinto del último síntoma: «el router no
  te está dando dirección, reinícialo» en vez de «no hay Internet». Con botón a lo siguiente (panel
  del router, cambiar DNS, velocidad) y los datos de la conexión a la vista (IP, router, DNS, proxy,
  VPN), para no ir a buscarlos a otra página.
- Dos pasos nuevos: **vaciar la tabla ARP** (el equipo tiene IP y no llega al router porque guarda la
  MAC vieja, típico al cambiar o reiniciar el router) y, en la reparación a fondo, **quitar el proxy
  de las descargas del sistema**, que heredado de otra red deja el equipo «conectado sin Internet».

**Diagnóstico**
- Un portal que no carga deja en el registro la dirección exacta y el código de WebView2. Antes
  «No se pudo abrir la página» no se podía diagnosticar a distancia.

### Auditoría de v1.1.5: resuelto ✅

**Redes de seguridad nuevas**

1. **El cruce de comandos es una prueba**, no una comprobación a mano
   (`src/lib/commands.test.ts`): cada `invoke("x")` de la interfaz tiene que estar en
   `generate_handler![…]`. Falla nombrando el comando y el archivo. Ya había dejado pasar dos
   comandos inexistentes a producción.
2. **El modo auditoría no se puede escapar por olvido.** `audit.rs` tiene ahora las dos listas
   completas —`BLOCKED` (78 comandos que cambian el equipo) y `SAFE` (246 revisados uno a uno)— y
   tres pruebas: que todo comando registrado esté clasificado, que ninguno esté en las dos listas ni
   sobre de una, y que los bloqueados existan. Añadir un comando obliga a decidir en cuál va.
   Comprobado a mano que las pruebas fallan de verdad al quitar una entrada.
3. **ESLint con las reglas de hooks como error** (`eslint.config.js`, `npm run lint`, en la CI).
   `react-hooks/exhaustive-deps` es la regla que habría avisado del portal que se recolocaba en cada
   recarga de la lista. Las 10 excepciones que había eran todas a propósito y ahora llevan el motivo
   escrito en la línea. Las reglas del compilador de React se apagan a conciencia: dan por malos
   patrones correctos aquí.

**Fallos latentes corregidos**

4. **Respuestas que llegan tarde** (`src/lib/useLiveEffect.ts`): 21 efectos que guardaban en el
   estado el resultado de una consulta ya no pisan al nuevo cuando cambia lo que se está mirando.
   Los que se notaban: el tamaño del perfil de otro usuario (Usuarios), los datos del perfil anterior
   al cambiar de usuario en Migrar, y la cuenta guardada de otro portal en el editor de portales.
5. **Las descargas de los portales ya no crecen sin límite**: cada una lleva su número propio (antes
   era su posición en la lista, que impedía podarla) y se recuerdan las últimas 200.
6. **Los datos de un portal se olvidan al cerrarlo o borrarlo** (rectángulo, última vez visto, cuándo
   se creó y a dónde iba). Además, una primera carga que se cancele ya no deja la vista sin poder
   dormirse nunca: a los 90 segundos se da por terminada.

**Alineación de la CI**

7. La CI usa ahora `cargo clippy --all-targets` (antes `--lib --tests`, más flojo que la verificación
   local), pasa ESLint y repite clippy con la versión de Rust fijada que se verifica a mano. Ya no
   puede pasar en verde algo que falle en local.

**Pendiente**

- **178 avisos de `@typescript-eslint/no-floating-promises`**: promesas sin `await` ni `.catch()`.
  99 son llamadas a un `load()` que ya captura por dentro; el resto hay que mirarlas una a una. Queda
  como aviso (no rompe la CI) para limpiarlas por páginas, no de golpe.

---

## Fases 8 y 9: hecho

- **Procesos**: lista en vivo (CPU, RAM, disco, usuario, ruta), finalizar proceso o árbol. Los críticos
  de Windows no se pueden cerrar; los sensibles (svchost, explorer, dwm…) avisan de las consecuencias.
  Antes de cerrar se comprueba que el PID sigue siendo el mismo programa. Todo queda en el diario.
- **Red y velocidad**: adaptadores, SSID y señal, ping ICMP nativo a router/DNS/Internet, DNS,
  detección de portal cautivo. **Speedtest** contra Cloudflare: 6/4 conexiones TCP en paralelo durante
  10 s, se descartan 2 s de arranque, latencia sin el tiempo del servidor (`server-timing`), jitter y
  latencia con carga (bufferbloat). Medido en el equipo de desarrollo: 36 ms (RTT TCP del servidor: 35 ms).
- **Actualizar software** con winget (tabla interpretada por columnas: independiente del idioma).
- **Espacio en disco**: `FindFirstFileExW` + rayon; `C:\Windows` (215 000 archivos) en 2,1 s (antes 124 s).
- **Copia de drivers** (`pnputil /export-driver`) y **análisis rápido de Defender**.
- Diagnóstico e informe incluyen software pendiente, antigüedad del último análisis y el último speedtest.
- **Sesión de servicio**, **clientes** con equipos e historial, **perfiles propios**, **marca del técnico**
  (logo, empresa, contacto, condiciones) y **checklist** configurable en el informe.

### Pendiente de verificar a mano (requiere administrador o modifica el equipo)
- Finalizar procesos de otros usuarios, actualizar software con winget y la copia de drivers.
- Una sesión de servicio completa de principio a fin (inicio → trabajo → informe archivado).

---

## Plan (v0.10 – v1.4)

Objetivo: pasar de "herramienta muy completa" a **producto profesional y redondo**, sin costes
(nada de certificados de pago ni servicios de suscripción). Cada fase es una versión.

### Fase 10 — Hardware (v0.10) ✅

- Inventario completo en la página **Hardware**, en el informe y en la ficha del cliente, que avisa si el
  hardware cambió entre visitas.
- **Temperaturas** con LibreHardwareMonitor (MPL-2.0, incluido): GPU sin administrador; CPU y placa con
  administrador y el driver PawnIO, instalable desde la app.
- **SMART** de discos SATA por WMI (reasignados, pendientes, no corregibles, CRC), sin programas externos.
- Resultado de la **prueba de RAM**; hallazgos de doble canal, BIOS y drivers antiguos.
- **Speedtest** con medidor animado, fases, gráfica en vivo, "apto para", IPv4/IPv6, proveedor y ubicación.
- Pendiente de esta área: identificar el **driver culpable** de cada pantallazo azul (análisis de minidumps).

### Fase 11 — Caja de herramientas del técnico (v0.11) ✅

- **Herramientas**: unos 90 accesos rápidos en 8 grupos (consolas, administración, diagnóstico, Panel de
  control, Configuración, carpetas, arranque y asistencia remota), con buscador, favoritos y accesos
  propios (programa, carpeta o web). Catálogo en `src-tauri/tools/shortcuts.toml`. Se ocultan los que no
  existen en esa edición de Windows (gpedit en Home). Carpetas y Configuración se abren como el usuario
  del equipo; consolas y .msc, como administrador. Reiniciar a la BIOS/UEFI y al inicio avanzado.
- **Usuarios locales** (también en Windows Home, que no tiene lusrmgr): crear (estándar o administrador,
  contraseña opcional, no caduca / cambiarla al entrar), cambiar o quitar la contraseña, activar,
  desactivar, hacer administrador o estándar, eliminar con o sin su perfil (muestra cuánto ocupa).
  Nunca deja el equipo sin administrador activo ni toca la cuenta con la sesión abierta o la que ejecuta
  AdminOps. Las contraseñas no se registran.
- **Ficha del equipo** en Herramientas: modelo, serie, Windows y clave OEM, con "Copiar ficha".
- **Redes Wi-Fi guardadas** en Red: contraseña (con administrador), seguridad, conexión automática y
  olvidar red. Usa la API nativa de WLAN: las claves nunca se escriben en disco.
- Pendiente de probar en equipos reales: crear, modificar y borrar usuarios y olvidar una red Wi-Fi.

### Fase 12 — Después de formatear (v0.12) ✅

- **Instalar programas**: catálogo de ~75 programas con ids de winget verificados (`src-tauri/tools/apps.toml`),
  listas predefinidas (Básico, Oficina, Gaming, Kit del técnico) y listas propias, búsqueda en winget para
  cualquier otro programa y marca de los ya instalados (`winget export`, independiente del idioma).
- **Copia de datos**: Escritorio, Documentos, Imágenes, Música, Vídeos, Descargas (respetando las carpetas
  redirigidas a OneDrive), marcadores de Chrome/Edge/Brave/Firefox y redes Wi-Fi, a un USB con
  `adminops-backup.json`. Los archivos que solo están en la nube no se descargan. Al restaurar nunca se
  sobrescribe: lo distinto se guarda como «nombre (AdminOps)».
- **Herramientas de red**: ping y traza de ruta en vivo con ICMP nativo, DNS por adaptador (Cloudflare,
  Google, Quad9, familia o propios), puertos en uso por programa y editor del archivo hosts con copia.
- **Impresoras**: estado y errores (atasco, sin papel…), vaciar la cola, página de prueba, predeterminada,
  quitar impresoras fantasma y reiniciar la cola completa.
- **Pantallazos azules**: el diagnóstico lee los minivolcados (formato de triaje de 64 bits) y señala el
  driver probable a partir de la pila del fallo, con una pista de qué hacer.
- El informe de batería ya existía en Diagnóstico (capacidad real frente a la de fábrica).
- Pendiente de probar en equipos reales: instalar en lote, restaurar una copia real, cambiar DNS, guardar
  hosts, quitar una impresora y analizar un volcado real (NotMyFault de Sysinternals debe señalar `myfault.sys`).

### Fase 13 — Entorno corporativo (v0.13) ✅

- **Tickets**: portales web del técnico (intranet de la empresa, GLPI, osTicket…) dentro de AdminOps, en
  la ventana principal o en una ventana aparte. La sesión y las contraseñas se recuerdan (autoguardado de
  WebView2 activado). Aislados: sin acceso a las funciones de AdminOps y solo navegan dentro de sus
  dominios; el resto de enlaces se abre en el navegador.
- **Dominio**: dominio / grupo de trabajo / Entra ID, controlador, relación de confianza, diferencia de
  hora y si la sesión es de dominio. Comprobaciones previas (edición, DNS del dominio, controlador
  accesible por LDAP, hora), unir (con OU y nuevo nombre opcionales), salir (exige un administrador local
  activo), reparar la relación de confianza y cambiar el nombre del equipo. Las credenciales no se guardan.
- **Paracaídas de errores**: si una página falla muestra el error en vez de dejar la ventana en negro, y
  lo guarda en el registro técnico. Corregido el fallo que ponía la app en negro al salir de Herramientas
  de red (`scrollIntoView` devuelve una Promise en Chromium reciente).
- Pendiente de probar en equipos reales: unir, salir y reparar en el dominio de la empresa.

### Fase 14 — Orden y pulido (v0.14) ✅

**Prioridad: alta.** Con 32 secciones, lo urgente es que todo se encuentre rápido y se sienta terminado.

- **Barra lateral plegable** por grupos (se recuerda qué está abierto) y **favoritos** fijados arriba.
- **Búsqueda global (Ctrl+K)**: saltar a cualquier página, ajuste, herramienta, reparación, programa o
  proceso escribiendo. Es el atajo que más tiempo ahorra al técnico.
- **Primer arranque guiado**: nombre y logo del técnico, portal de Tickets, dominio habitual y primer
  diagnóstico, para que la app no empiece vacía.
- **La ventana recuerda** tamaño, posición y última página; **atajos de teclado** básicos.
- **Notificaciones de Windows** al terminar tareas largas (SFC, DISM, instalaciones, copias).
- **Portable sin rastro**: la caché de WebView2 también en `AdminOps-data` (hoy queda en el equipo).
- **Paquete de soporte**: un botón junta registro, último diagnóstico y versión en un .zip.
- **Revisión de textos y estados vacíos** en todas las páginas (qué hacer cuando no hay datos).
- Hecho además: grupos reordenados (Sistema, Optimizar, Soporte, Administración) y Ajustes en el pie.
- Pendiente: revisión completa de estados vacíos página por página (se irá haciendo en cada fase).

**Terminado cuando:** cualquier función se alcanza en dos pulsaciones y un usuario nuevo llega a su
primer informe sin instrucciones.

### Fase 15 — Mantenimiento a fondo (v0.15) ✅

**Prioridad: alta.** Resolver el "PC lento" completo sin salir de AdminOps.

- **Desinstalador de programas** (Win32 y Store) con desinstalación silenciosa y limpieza de restos
  (carpetas y claves huérfanas), mostrando lo que libera cada uno.
- **Limpieza de navegadores** por perfil (Chrome, Edge, Firefox, Brave, Opera): caché y descargas
  temporales; contraseñas e historial solo si se elige.
- **Windows Update**: historial, actualizaciones fallidas con su error explicado, pausar/reanudar y
  ocultar una actualización problemática.
- **Análisis del arranque**: qué programas y servicios retrasan el inicio.
- **Restaurar drivers** desde una copia de AdminOps (`pnputil /add-driver`).
- **Más reparaciones**: Microsoft Store (`wsreset`), búsqueda de Windows, audio, asociaciones de
  archivos, perfil de usuario dañado (el que inicia con perfil temporal).
- **Puntos de restauración**: ver cuánto ocupan y borrar los antiguos.
- **Mantenimiento programado** opcional (limpieza semanal/mensual como tarea del sistema).
- Hecho además: prueba que analiza la sintaxis de todos los scripts de PowerShell (catálogo e incrustados)
  sin ejecutarlos; encontró un fallo real (`Data` es palabra reservada) antes de publicarse.
- Pendiente de probar en equipos reales: desinstalar, ocultar una actualización, restaurar drivers y la
  tarea programada (se ejecuta como SYSTEM con `adminops.exe --maintenance`).

**Terminado cuando:** las tareas habituales de mantenimiento no requieren ninguna otra herramienta.

### Fase 16 — Seguridad del equipo (v0.16) ✅

**Prioridad: alta.** Muy valorado por clientes y empresas, y fácil de explicar en el informe.

- **Auditoría de seguridad con nota** (0-100) y cómo subirla: Defender y firmas, firewall, UAC,
  BitLocker, SMB1, escritorio remoto expuesto, número de administradores, cuentas sin contraseña,
  actualizaciones pendientes, arranque seguro y TPM.
- **BitLocker**: estado por unidad y **copia de la clave de recuperación** (al USB o al informe
  técnico), la causa típica de equipos bloqueados.
- **Programas vulnerables**: versiones antiguas de navegadores, Java, Adobe Reader… con actualizar en
  un clic (winget).
- **Elementos sospechosos**: tareas programadas, servicios e inicio sin firma o en carpetas temporales;
  extensiones de navegador instaladas; archivo hosts modificado.
- **Sección de seguridad en el informe** con la nota antes y después.
- Hecho además: categoría de ajustes `security` (firewall, UAC, SMB1, NLA, RDP, ejecución automática,
  Invitado, bloqueo de PUA), todos reversibles; hallazgo en el diagnóstico si la nota baja de 80.
- Pendiente de probar en equipos reales: exportar claves de BitLocker y desactivar una tarea sospechosa.

**Terminado cuando:** el técnico entrega una nota de seguridad comprensible y mejorada en cada visita.

### Fase 17 — Instalador y atajos (v0.17) ✅

- **Instalador con interfaz propia** (`installer/`): ventana con la marca, carpeta de instalación, acceso
  en el escritorio y abrir al terminar. Detecta la versión instalada (actualizar, reinstalar o bloquear si
  la instalada es más nueva) y si AdminOps está abierto. Barra de progreso con consejos. Por dentro ejecuta
  el instalador NSIS en silencio, así que conserva desinstalador, entrada en Windows y accesos.
- **Instalador clásico** junto al nuevo: con asistente y `/S` para desplegar en muchos equipos.
- **Atajos de teclado**: unos 150 en 12 categorías (incluida una de técnico y soporte), buscador, mis
  atajos, modo «descubrir» (pulsas una combinación y dice qué hace) y «Probar» para los de la tecla
  Windows (solo combinaciones Win+… de una lista cerrada; Win+L excluido).
- Pendiente de probar en equipos reales: instalar, actualizar y reinstalar con el instalador nuevo, y un
  equipo sin WebView2 (debe abrir el clásico).

### Fase 18 — Rediseño (v0.18) ✅

Objetivo: que la app se vea como una herramienta profesional y no como una interfaz «generada».
Propuesta visual: artifact «AdminOps — Propuesta de rediseño».

- **Sistema visual**: IBM Plex Sans (y Mono solo para rutas y códigos), un solo acento azul para la
  selección y la acción principal, verde/ámbar/rojo apagados solo para estados, sin brillos, degradados ni
  desenfoques, títulos en frase (fuera las mayúsculas espaciadas). Tokens en `src/index.css`.
- **Tema claro** además del oscuro (Ajustes → Apariencia).
- **Navegación**: 7 áreas en la barra lateral (Panel, Equipo, Optimizar, Programas, Red, Soporte,
  Administración) y las páginas como pestañas; cada área recuerda su última pestaña. Ajustes en el pie.
- **Panel** rehecho: banda de cifras clave, «Requiere atención» (hallazgos del último diagnóstico),
  procesos y almacenamiento sin cajas.
- **Herramientas** rehecha: filas limpias en lugar de tarjetas, filtros por grupo, ficha compacta.
- **Icono nuevo** (sustituido en la 0.19 por el medidor con llave): una tuerca con el pulso de diagnóstico dentro, plano y de una
  tinta; en la app, el ejecutable, el informe PDF y el instalador.
- Informe PDF e instalador con el mismo lenguaje (el instalador lleva sus fuentes).
- **0.18.1**: barra lateral desplegable (las secciones salen debajo de su área; solo abiertas la actual
  y las que se dejan abiertas), Panel rehecho (veredicto, acciones rápidas, equipo, salud de discos,
  actividad reciente), instalador que cierra todo lo que retiene archivos y reintenta (antes fallaba al
  actualizar con la app abierta), procesos de PowerShell ligados a la vida de la app (job object) y
  caché de iconos de Windows refrescada al instalar. Icono: cinco conceptos en el lienzo de diseño.
- Pendiente: repasar página a página los detalles que el barrido automático no cubre (espaciados y
  tarjetas internas de cada sección) y rehacer las capturas del README.

### Fase 19 — Informe y cliente (v0.19) ✅

Lo que convierte el trabajo técnico en algo que el cliente valora (y paga).

- **Informe PDF rehecho**: cabecera con la marca y número de documento correlativo por año (2026-0001),
  fichas de cliente, equipo (con nº de serie) y servicio (fecha y duración), veredicto con cifras clave
  (seguridad, resueltos, pendientes, acciones), problemas **resueltos** en la visita frente a los
  **pendientes**, antes/después, recomendaciones, firmas y pie con «Página X de Y».
- **Dos plantillas**: para el cliente (lenguaje claro, ficha resumida del equipo que también mira SMART)
  y técnica (todo el detalle: hardware, módulos, discos, SMART, estabilidad, drivers, seguridad, programas).
- **Presupuesto o recibo**: líneas de servicio y piezas, cantidades, descuento, impuesto configurable
  (ITBIS 18 % por defecto), total, forma de pago y validez del presupuesto. Catálogo de servicios y
  piezas en Ajustes para añadirlos con un clic.
- **Firma del cliente** en pantalla al finalizar la sesión (ratón, lápiz o dedo) y **firma del técnico**
  guardada en Ajustes. Sin firma, el PDF deja el espacio para firmar en papel.
- **Enviar por correo**: se abre el programa de correo con el PDF adjunto y el mensaje ya redactado
  (archivo .eml de borrador); alternativa para correo web que abre el correo y la carpeta del PDF.
- **Garantías**: mano de obra y cada pieza con su fecha de fin, en el recibo y en la ficha del cliente.
- **Próximo mantenimiento**: fecha recomendada en el informe y recordatorio en Clientes (vencidos y
  próximos 30 días, posponer o quitar).
- **Comparar visitas**: tabla de evolución en la ficha del cliente (críticos, advertencias, seguridad,
  espacio, inicio, actualizaciones, arranque, batería, memoria).
- El informe hecho a mano (Soporte → Informe) también puede elegir cliente de la ficha, plantilla,
  presupuesto o recibo, y guardar la visita en su historial.
- **Icono definitivo**: un medidor (diagnóstico) cuya aguja termina en una llave (reparación).

### Fase 20 — Red y router (v0.20) ✅

Entrar al router y saber qué hay en la red sin abrir el navegador ni buscar la IP a mano.

- **Mi red**: nombre de la red, conexión (Wi-Fi o cable y velocidad), IP del equipo (fija o DHCP),
  router, DNS e IP pública con el proveedor y la ciudad (oculta hasta pulsar el ojo). Aviso si
  Windows trata la red como pública.
- **Panel del router dentro de AdminOps** (como los portales de Tickets), en ventana aparte o en el
  navegador. Acepta el certificado propio de los routers, solo para direcciones de la red local.
- **Acceso guardado**: usuario, contraseña (cifrada con DPAPI: solo ese usuario de Windows en ese
  equipo puede leerla), dirección del panel y notas, por red. En el panel, botones para copiar el
  usuario y la contraseña. Pista de credenciales de fábrica según la marca detectada.
- **Chequeo del router**: marca (por la página del panel), puertos abiertos (Telnet y FTP como
  riesgo), panel sin HTTPS, cifrado de la Wi-Fi (abierta, WEP, WPA, WPA2, WPA3) y doble NAT o red
  del proveedor / CGNAT (por el segundo salto hacia Internet).
- **Wi-Fi**: contraseña de la red actual (mostrar y copiar) y **código QR** para conectar un móvil.
- **Dispositivos en la red**: barrido de la subred (ping y tabla de vecinos, así aparecen también
  los que no responden al ping), nombre, IP, MAC, MAC privada de móviles, fabricante (consulta
  opcional en Internet solo del prefijo, con caché), nombres propios para cada dispositivo y marca
  de **nuevo** para los que no estaban la última vez.

### Fase 21 — Caja fuerte y privacidad (v0.21) ✅

Proteger los datos del cliente con lo que ya trae Windows, para que siga funcionando aunque se
desinstale AdminOps. Nueva área **Datos y familia** en la barra lateral.

- **Caja fuerte**: un .vhdx dinámico (API de discos virtuales de Windows, sin Hyper-V) cifrado con
  BitLocker XTS-AES 256 y contraseña. Se abre con doble clic desde Windows sin AdminOps. En la app:
  crear, abrir, cerrar (bloquea antes de expulsar y avisa si hay archivos abiertos), cambiar la
  contraseña, ver la clave de recuperación y guardarla en un archivo, añadir una existente, quitar o
  eliminar. La clave de recuperación no se guarda: se muestra y hay que copiarla o guardarla.
  Requiere Windows Pro para crearla.
- **Carpeta cifrada** (cualquier Windows): .zip AES-256 que abren 7-Zip, WinRAR o AdminOps. Se
  comprueba que se descifra bien antes de borrar la original (opcional, con borrado seguro).
- **Borrado seguro** de archivos y carpetas (sobrescritura y renombrado antes de borrar; nunca
  rutas del sistema), **vaciar el espacio libre** (cipher /w, cancelable) y checklist **antes de
  vender o donar el equipo** con accesos directos a cada paso.
- **Recuperar archivos borrados** con Windows File Recovery: se instala desde la Store, se elige
  unidad, destino (siempre otra), tipos de archivo o carpeta y búsqueda rápida o a fondo.
- **Control parental**: filtro de navegación por DNS (Cloudflare for Families, CleanBrowsing) en
  todos los adaptadores, sitios bloqueados en una sección propia del archivo hosts y **horario de
  uso** por usuario local (cuadrícula de 7×24, plantillas; API de Windows, independiente del idioma).
- Las contraseñas nunca se guardan ni aparecen en el registro de actividad.

### Fase 22 — La oficina completa (v0.22) ✅

Pasar de "un equipo" a "la red del cliente".

- **Inventario de equipos** (Soporte → Inventario): cada equipo que pasa por una sesión (o que se
  añade con un clic) queda en la ficha de su cliente con fabricante, modelo, serie, CPU, memoria,
  discos, gráfica, Windows, año aproximado (BIOS), TPM, nota de seguridad, batería, IP y MAC, y un
  veredicto **Bien / Mejorar / Renovar** con los motivos (SSD, memoria, batería, disco con fallos,
  Windows 10 sin soporte o sin TPM, antigüedad). Filtros, vista de todos los clientes y **exportar
  a Excel (CSV)**.
- **Acceso remoto** (Red → Acceso remoto): conectar por Escritorio remoto, abrir Asistencia rápida,
  **encender equipos por la red** (Wake-on-LAN); para este equipo, activar/desactivar recibir
  Escritorio remoto (con autenticación de red y firewall) y **prepararlo para Wake-on-LAN**
  (paquete mágico en el adaptador por cable e inicio rápido desactivado).
- Encender y conectar con un clic desde el **Inventario**, la **ficha del cliente** y
  **Dispositivos en la red**.
- **Carpetas compartidas** (Administración): qué se comparte, con qué permisos, archivos en uso y
  quién está conectado; compartir una carpeta (todos o un usuario, lectura o escritura, también
  permisos NTFS) y dejar de compartir; aviso y arreglo si la red es pública o el firewall bloquea.
- **IP duplicadas**: conflictos que Windows detectó en los últimos 30 días, con el dispositivo que
  usaba la misma IP.
- **Mapa de la red** guardado en la ficha del cliente desde Dispositivos en la red.

### Fase 23 — Ajustes, rendimiento y red a fondo (v0.23) ✅

- **Rendimiento** (medido): lectura del Panel un 23 % más ligera (solo CPU y memoria de cada
  proceso; discos cada 10 s) y diagnóstico un 15 % más rápido (5 procesos de PowerShell en lugar
  de 3; los que sobran se cierran tras 3 minutos sin uso). Buscador, asistente y «Acerca de» se
  cargan al abrirlos.
- **Ajustes en pestañas**: General, Apariencia, Navegación, Seguridad, Informes y cobros, Acerca de.
- **Personalización**: color de acento (6), tamaño de toda la interfaz (zoom), reducir animaciones,
  página al abrir, cada cuánto se actualiza el Panel, avisos al terminar tareas largas.
- **Navegación a medida**: reordenar y renombrar secciones, cambiar su icono, mover cada página a
  otra sección, crear secciones propias, ocultar páginas (siguen en Ctrl+K) y restablecer. Las
  páginas de versiones futuras aparecen solas en su sección de fábrica.
- **Bloqueo con PIN o contraseña** al abrir y por inactividad (Ctrl+L o el candado para bloquear
  al momento). Hash PBKDF2-HMAC-SHA256 con sal; espera tras 5 fallos. Si se olvida, se desbloquea
  con la contraseña de Windows de la cuenta (máximo 3 intentos cada 15 min, para no bloquear la
  cuenta de Windows).
- **Copia de la configuración**: exportar e importar ajustes, portales de Tickets y preferencias.
- **Dispositivos en la red a fondo**: tipo (router, PC, móvil, TV, Chromecast, impresora, cámara,
  NAS, consola, altavoz, domótica…), fabricante, modelo, nombre anunciado, sistema aproximado,
  servicios y puertos abiertos (UPnP/SSDP, mDNS/Bonjour, NetBIOS, puertos, título web y TTL, todo
  en la red local), filtros por tipo, ficha de cada dispositivo y **contraseña de la Wi-Fi** de la
  red actual (oculta hasta pulsar el ojo; por cable, la Wi-Fi guardada con el mismo nombre).
- **Herramientas**: 26 accesos nuevos (variables de entorno, opciones de rendimiento, nombre del
  equipo, BitLocker, historial de archivos, copia de Windows 7, apps del firewall, calibrar color,
  licencia de Windows, informes de batería, Wi-Fi y energía, solucionadores, proxy, VPN, redes
  Wi-Fi, gráficos por app, permisos de cámara y micrófono, inicio de sesión, sensor de
  almacenamiento, accesibilidad, teclado en pantalla, lupa, mapa de caracteres, Sandbox, Hyper-V).

### Fase 24 — Versión 1.0 (v1.0) ✅

Pulido para el uso diario.

- **Las páginas conservan su contenido**: al volver a Diagnóstico, Red, Tickets, etc. no se vuelve a
  cargar ni analizar nada; cada página se mantiene viva (con su scroll y sus tareas) hasta pulsar
  **Recargar** (o F5) o cerrar la app. Las páginas ocultas pausan sus lecturas en vivo, escuchas de
  teclado y vistas web. Como mucho 12 a la vez (se descarta la menos usada).
- **Atajos de teclado propios** para abrir cualquier página (Ctrl+Alt+letra, teclas F…), sin
  interferir con AltGr ni con los atajos de la app o de Windows.
- **Herramientas favoritas en el Panel** (las marcadas con ★ en Herramientas).
- **Punto de restauración** configurable: solo antes de cambios con riesgo, antes de cualquier
  cambio o nunca.
- **Iniciar con Windows**, minimizada, mediante una tarea programada con permisos de administrador
  (sin aviso de UAC en cada inicio de sesión).
- **Datos antiguos**: uso de historial, análisis, informes y registros; limpieza manual por
  antigüedad y automática al abrir. Nunca se borra lo que aún se puede deshacer, el último análisis ni
  el de una sesión en curso; los informes van a la papelera.
- **Aviso de versiones nuevas** consultando GitHub Releases (sin descargar ni instalar nada solo).
- **1.0.1**: la identificación de dispositivos cerraba la app (esperaba un futuro desde dentro del
  runtime de Tauri); ahora usa su propio hilo. Un fallo inesperado ya no cierra la app: se anota en el
  registro («Fallo interno») y la tarea termina con error.

### Fase 25 — Pulido (v1.1) ✅

- **Revisión de fallos**: ningún error inesperado cierra la app (se anota en el registro como «Fallo
  interno»); los bloqueos internos se recuperan tras un fallo en lugar de propagarlo (44 sitios);
  el test de velocidad ya no puede fallar con una medición inválida (NaN).
- **Vigilancia de errores de Windows**: mientras AdminOps está abierta, revisa el Visor de eventos
  cada minuto y avisa (campana en la cabecera, aviso en la app y notificación de Windows si está en
  segundo plano) de pantallazos azules, apagados inesperados, discos con sectores dañados o lentos
  (con el modelo del disco), errores del sistema de archivos, errores de hardware (WHEA), driver de
  vídeo reiniciado, conflictos de IP, servicios caídos, memoria agotada, fallos de Windows Update,
  procesador limitado por temperatura, programas que se cierran o se cuelgan, amenazas de Defender y
  poco espacio en el disco del sistema. Cada aviso explica qué es y qué hacer, con un botón a la
  página donde se arregla. Agrupa repeticiones y se puede desactivar en Ajustes.
- **Acceso remoto a fondo**: agenda de conexiones (Escritorio remoto, AnyDesk, RustDesk, TeamViewer)
  con cliente y notas; Escritorio remoto con opciones (pantalla completa, monitores, portapapeles,
  unidades, impresoras, sonido) y contraseña guardada en el Administrador de credenciales de Windows;
  prueba de conexión con la causa si falla; ID de este equipo en AnyDesk/RustDesk/TeamViewer,
  instalarlos y conectar a un ID; usuarios con permiso de Escritorio remoto.
- **Navegación a medida**: arrastrar y soltar secciones y páginas, renombrar cualquier página,
  selector visual de iconos (32), barra completa o solo iconos (con pestañas), a la izquierda o a la
  derecha, tres anchos y densidades, secciones desplegadas (solo la actual / las que deje abiertas /
  todas), qué hace el clic en una sección, pestañas bajo el título, **Favoritos** (★ también desde
  la barra) y **Recientes**, y ocultar buscador, aviso de sesión o pie.
- **Atajos**: 283 (antes 155), con grupos nuevos (problemas típicos, Escritorio remoto, Word,
  Outlook, PowerPoint, reuniones de Teams/Zoom/Meet) y más en Windows, Explorador, navegador, Excel
  y técnico. Una prueba comprueba que todos los que tienen botón «Probar» se pueden simular.
- Las barras de «Guardar» ya no dependen de la posición ni del ancho de la barra lateral.
- **1.1.1 — equipos de empresa (Sophos y similares)**: PowerShell sin `-EncodedCommand` ni base64 (el
  script viaja por la entrada estándar y el canal persistente usa líneas contadas); datos como literales
  de PowerShell y contraseñas como SecureString cifrada con DPAPI (nunca legibles en el script ni en el
  registro); el script de elementos sospechosos ya no contiene palabras de malware (se comparan en
  AdminOps); mensajes claros si el antivirus bloquea una acción o PowerShell está restringido por
  directiva, y aviso al arrancar si PowerShell no está disponible. Con otro antivirus, las firmas y los
  análisis de Defender en reposo ya no generan avisos falsos. Prueba de humo con 49 lecturas reales.
- **1.1.1 — soporte del día a día**:
  - **Solucionar problemas** (Equipo): eliges el síntoma (no hay Internet, Wi-Fi, no suena,
    Bluetooth, pantalla, no imprime, va lento, Windows Update) y AdminOps revisa en orden las causas
    típicas y ofrece la reparación de cada una.
  - **Reparar la red** en un clic (rápida o a fondo) con la comprobación antes y después.
  - **Control de la Wi-Fi**: estado real de la tarjeta con el error explicado, encender/apagar,
    reiniciar la tarjeta, desactivar el ahorro de energía y quitar adaptadores fantasma. Si la tarjeta
    falla, Dispositivos explica por qué no se ve la red ni la contraseña.
  - **Aviso automático de dispositivos que dejan de funcionar** (cada 5 min, solo los nuevos), con
    enlace a su reparación.
  - **Deshacer desde el aviso**: cada cambio suelto que se puede deshacer lo ofrece en el propio aviso.
  - **Línea de tiempo del equipo** (Historial): cambios de AdminOps, avisos, diagnósticos,
    actualizaciones, drivers, programas y apagados bruscos, por días.
  - La **búsqueda** (Ctrl+K) encuentra acciones y problemas: «no suena», «encender Bluetooth»,
    «reiniciar el driver de la gráfica», páginas de Configuración de Windows…
  - **Contactos** (Soporte): agenda del técnico que viaja con AdminOps (en el USB con el portable).
    - Vistas de tarjetas, tabla (columnas elegibles y ordenables) y directorio de extensiones;
      agrupar por empresa, área, etiqueta o letra; ordenar por nombre, empresa, extensión, uso o fecha.
    - Filtros rápidos (favoritos, más usados, con extensión o correo, sin completar, recientes),
      varias etiquetas con «todas/alguna», empresa, búsqueda con prefijos (`ext:`, `empresa:`, `#tag`,
      `-excluir`) y búsquedas guardadas. La vista y los filtros se recuerdan.
    - Etiquetas en chips con autocompletado y colores; renombrar, fusionar y borrar en todos.
    - Ficha lateral: varios teléfonos y correos, disponibilidad, sustituto («si no está…»), cliente
      enlazado, llamar, correo, chat o llamada de Teams y «Copiar tarjeta».
    - «Más usados», selección múltiple (Mayús para rangos) con etiquetas, favoritos, exportar y
      papelera; duplicados con fusión; papelera de 30 días; copia automática semanal (5 últimas).
    - Importa CSV (Excel, Outlook) y vCard; exporta CSV y vCard. Se busca desde Ctrl+K.
  - **Portable entre equipos**: las contraseñas de routers se cifran con una clave del USB (AES-256-GCM)
    en vez de DPAPI, así funcionan en todos los equipos; las antiguas se migran solas. Ajustes explica
    qué viaja contigo y qué es de cada equipo.

### v1.1.2 — Herramientas del técnico ✅

- **Trabajo en el equipo del cliente**
  - **Entrega rápida** (Informes): lo hecho hoy con AdminOps pasa a las observaciones y se genera el
    informe comparando con el primer diagnóstico del día.
  - **Tipos de visita** con su checklist (Mantenimiento, Equipo nuevo, Equipo lento, Sin Internet,
    Copia de datos; editables en Ajustes → Informes). Los puntos marcados como automáticos se tachan
    solos al hacer esa tarea con AdminOps (limpieza, actualizaciones, diagnóstico, dominio…).
  - **Recetas** (Administración): punto de restauración, quitar bloatware, instalar una lista de
    programas, perfil o ajustes sueltos, crear usuario, renombrar (admite `{serie}`), unir al dominio y
    diagnóstico final, de una vez. Las contraseñas se piden al ejecutar y nunca se guardan.
  - **Ficha del equipo** (Hardware): modelo, número de serie, hardware, Windows y licencia, TPM, red y
    usuario; copiar, pegar en Excel, CSV, enlace a la garantía (Dell, Lenovo, HP…) y al inventario
    de un cliente.
- **Conocimiento** (Soporte), viaja con AdminOps:
  - **Soluciones** «problema → lo que funcionó», con etiquetas; se guardan también al terminar una
    sesión de servicio y se buscan desde Ctrl+K.
  - **Plantillas de texto** con variables automáticas ({equipo}, {usuario}, {fecha}, {tecnico}…) y
    preguntas propias ({?Nombre}).
  - **Notas por equipo y por red**: aparecen solas en el Panel al volver a ese equipo o a esa red.
- **Red y oficina**
  - **Mapa de la oficina** (Dispositivos): función, responsable (de Contactos) y notas de cada
    dispositivo, por red; las IP se actualizan en cada búsqueda.
  - **Vigilancia de dispositivos clave**: avisa (campana y notificación) si una impresora, servidor o
    NAS marcado deja de responder, y cuando vuelve. Dos fallos seguidos para evitar falsas alarmas.
  - **Comprobar puestos** (Red): lista de equipos guardada; cuáles responden y, a fondo (WinRM o DCOM),
    disco libre, días sin reiniciar, días sin actualizar y usuario conectado. CSV y conexión por
    Escritorio remoto.
- **Enlaces entre todo**: el responsable de cada tema (red, impresoras, sistemas…) aparece en los
  avisos y en Solucionar problemas; los clientes muestran sus contactos; la sesión guarda quién
  pidió el trabajo y la ficha del contacto lista sus visitas.
- **Profesionalización**
  - **Copia de seguridad cifrada** (AES-256) de todo lo que viaja contigo, a otra unidad o a una
    carpeta en la nube, y restauración (lo sobrescrito se guarda antes).
  - **Seguridad de tus datos** (Ajustes): estado de la unidad, espacio, clave del USB, copias
    recientes y avisos (FAT32, sin copia en 30 días…).
  - **Modo auditoría** («Solo mirar» en la barra superior o `--auditoria`): se bloquean en el
    despachador de comandos todas las acciones que cambian el equipo; diagnósticos, informes y datos
    propios siguen funcionando.

- **Programas**
  - Desinstalación silenciosa para muchos más programas: además de MSI y los que la declaran,
    Inno Setup y NSIS (detectados por su desinstalador), y MSI mal registrados.
  - **Desinstalar en lote**: primero los silenciosos, luego los que necesitan su asistente, y una sola
    revisión de restos al final.
  - **Restos completos**: carpetas, accesos directos del escritorio y del menú Inicio y claves del
    registro (con copia .reg). Nunca la carpeta o la clave de otro programa instalado (un plugin que
    declaraba la carpeta del programa principal la proponía entera). También para entradas huérfanas.
  - Reparar programas MSI, filtros (con actualización, grandes, recientes, del usuario, necesitan
    asistente, huérfanas), ocultar componentes y runtimes, orden por editor e inventario en CSV.
  - Las actualizaciones de winget aparecen en la propia lista, con botón para actualizar.
  - Más rápido: la lista de winget (actualizaciones e instalados) se reutiliza unos minutos y va en
    su propio proceso sin frenar al resto de AdminOps; al actualizar no se vuelve a consultar; se
    indica el origen (winget/msstore). Detección de NSIS con caché (lista en ~40 ms).
  - **Actualizaciones ignoradas**: «no actualizar este programa», recordado en todos los equipos.
- **Correcciones**: Teams desde Contactos abre la app de Teams (protocolo msteams:) en vez de una
  carpeta; guardar un contacto recién usado ya no falla; posponer un mantenimiento vencido tampoco;
  las pruebas ya no llevan textos que el antivirus confunde con malware.

- **Calidad**
  - Tests del frontend con Vitest (`npm test`, también en CI): búsqueda y filtros de contactos,
    duplicados, vCard/CSV, responsable por tema, plantillas, checklist automática, emparejar programas
    con winget y errores.
  - Errores comprensibles: todas las llamadas al sistema pasan por un traductor (permisos, archivos en
    uso, sin espacio, sin red, equipo remoto…); los fallos internos se anotan en el registro técnico.
  - Checklist de pruebas manuales antes de publicar: `docs/PRUEBAS.md`.
- **Navegación reorganizada** por lo que hace el técnico: Inicio (Panel, Solucionar problemas,
  Sesión), Equipo, Mantener, Programas, Red, Oficina, Soporte y Datos. Páginas unidas: Actualizaciones
  (programas + Windows Update), Mi red (router + dispositivos), Puestos e inventario, y las
  Reparaciones dentro de Solucionar problemas. Los enlaces antiguos llevan a la pestaña nueva y las
  navegaciones personalizadas se conservan.
- **Experiencia**
  - Panel como centro de mando: «Qué hacer ahora» reúne diagnóstico, avisos, actualizaciones, espacio,
    datos y sesión en curso, por importancia y con su botón; «Revisión completa» en el propio Panel.
  - Indicador de tareas en la barra superior: lo que está en marcha (paso actual, tiempo, cancelar) y
    lo terminado, desde cualquier página.
  - Ayuda «?» junto al título de cada página.
  - Estados de carga y listas vacías iguales en toda la app.

- **Inventario web**: pestaña en Puestos e inventario para la web de inventario de la empresa, dentro
  de AdminOps como Tickets, con el panel «Datos del equipo» (cada dato se copia con un clic para el
  formulario). El inventario manual sigue igual.
- **Intranets con dominio** (como *.pgr.gob.do): el navegador interno entra con la cuenta de Windows
  sin pedir contraseña en los dominios de los portales del técnico y su dominio habitual.
- **Cuentas** (Oficina): sesión y estado del equipo (local, Microsoft, Entra ID, dominio), cuentas
  profesionales o educativas, cuentas de Microsoft y de Office, y credenciales guardadas. Asistente
  «pasar el equipo a una cuenta local»: crear administrador local → sacar el equipo de Entra ID
  (`dsregcmd /leave`, solo si ya hay un administrador local) → cerrar sesión. Desconectar cuentas
  profesionales añadidas, cerrar sesión de Office y borrar credenciales, todo en el diario.
- **Abrir enlaces sin permisos de administrador**: Teams, correo, llamadas, webs y Configuración se
  abren a través del escritorio de Windows (como el usuario), no con Explorer, que abría Documentos.
- **Portable descargado de Internet**: el lector de temperaturas ya no falla por la marca «descargado
  de Internet» de Windows; ningún error muestra la carpeta del usuario.

- **Diagnóstico más rápido y con datos** (medido: la auditoría de seguridad pasó de 7,5 s a 2,3 s):
  - La auditoría de seguridad se reparte en tres consultas en paralelo (registro, Defender, BitLocker).
  - BitLocker no se consulta sin administrador, porque Windows tardaba 5 s en no devolver nada. Con
    administrador se reutiliza durante 10 minutos.
  - Defender y BitLocker se precargan al abrir AdminOps.
  - Inventario de hardware, SMART y prueba de memoria se reutilizan 5 minutos; la lista de winget,
    10 minutos. «A fondo» vuelve a leerlo todo, y la «Revisión completa» del Panel siempre lo hace.
  - Cada sección se pinta en cuanto termina, sin esperar a las demás.
  - Opción «Diagnosticar al abrir AdminOps» (Ajustes → General).
- **Arreglar en el sitio**: los hallazgos llevan su botón «Arreglar», también en el Panel, que
  ejecuta el ajuste o la reparación sin salir de la página. «Arreglar todo lo seguro» aplica de una
  vez solo lo que no cambia el comportamiento de Windows ni borra archivos del usuario.
- **Qué cambió desde el análisis anterior**: problemas nuevos (marcados «Nuevo») y resueltos. Se
  compara sin tener en cuenta las cifras: «poco espacio (8 %)» y «(7 %)» son el mismo problema. El
  informe antes/después usa la misma comparación.
- **Ajustes de Windows**: Limpieza, Rendimiento, Privacidad y Servicios en una página con pestañas y
  un buscador que mira en las cuatro a la vez (sin tildes: «telemetria» encuentra «Telemetría»).
- **Preparar equipos** (antes «Recetas»): pestañas Plantillas y Perfiles de ajustes.
- **Puestos en lote**: marcar varios equipos y hacer lo siguiente sobre todos, con el resultado de
  cada uno y la acción anotada en el Historial:
  - reiniciar, con aviso al usuario y 2 minutos de margen;
  - cancelar ese reinicio;
  - actualizar directivas;
  - enviar un mensaje en pantalla;
  - abrir el Escritorio remoto (hasta 6 a la vez).
- **Estado entre visitas**: en la ficha del cliente, qué cambió en cada equipo desde la visita
  anterior: problemas, nota de seguridad, espacio, programas de inicio, actualizaciones, arranque,
  RAM, batería y hardware. Se descartan las diferencias pequeñas. Al empezar una sesión se compara
  con cómo está ese equipo ahora. «Evolución entre visitas» ya no compara visitas de equipos
  distintos entre sí.
- **Rendimiento de AdminOps** (Ajustes → Rendimiento):
  - pasos del arranque del programa;
  - cuándo se abre la ventana, cuándo se pinta la primera página y cuándo está lista;
  - por cada página, en su primera visita: descarga del código, pintado y tiempo hasta que
    terminan sus consultas;
  - las consultas al backend más lentas.

  El resumen del arranque queda en el registro técnico y se puede copiar para compararlo entre
  versiones.

- **Portales más rápidos y con barra de navegador** (Tickets, Inventario web, Correo):
  - Se precarga en segundo plano el último portal usado de cada tipo (Ajustes → General → «Precargar
    los portales»). Las vistas se mantienen vivas y la barra ya no parpadea al volver.
  - Barra completa: atrás y adelante según el historial real, recargar o detener, página inicial,
    dirección editable con candado y botón Copiar, zoom recordado por portal, buscar en la página,
    imprimir o guardar como PDF, y descargas con Abrir y «Mostrar en la carpeta». De cada descarga
    solo se ve el nombre del archivo, nunca la ruta.
  - Si la web no carga, en vez de la página en blanco se explica el motivo (sin red o sin VPN,
    servidor caído, certificado no válido…) con Reintentar, Página inicial y Abrir en el navegador.
  - Barra de progreso mientras carga. El registro anota cuánto tarda cada vista en crearse y en
    cargar la primera vez.
  - Por portal:
    - inicio de sesión guardado y cifrado (viaja en el USB en portable), que se rellena solo;
    - sesión privada, que no guarda nada en el equipo y se cierra al salir;
    - ventanas emergentes en ventana aparte.
- **Correo** (Soporte): Outlook del trabajo (Microsoft 365) u Outlook.com dentro de AdminOps.
  - Con la cuenta guardada, AdminOps rellena el inicio de sesión de Microsoft; solo queda la
    verificación en el móvil.
  - Sesión privada recomendada y activada por defecto.
  - Botones Redactar y Cerrar sesión; contador de no leídos si Outlook lo pone en el título.
  - Redactar al instante: con Outlook abierto pulsa su propio botón «Correo nuevo» en vez de
    recargar toda la web con el enlace de redactar (solo se usa el enlace si el botón no aparece o
    si el mensaje va con destinatario, asunto o texto).
- **Portales: la red, no AdminOps**:
  - Sin conexión, una franja lo avisa y el portal que falló se recarga solo al volver la red.
  - Si una página tarda más de 10 segundos, se avisa de que es la conexión o el servidor de la
    web, con Recargar y Abrir en el navegador.
  - Al volver a Tickets, Inventario o Correo, la vista aparece al momento: la lista de portales
    ya leída se guarda en memoria.
  - Los mensajes y adjuntos se abren en su propia ventana.
  - «Escribir un correo» en Contactos lo abre en el Correo de AdminOps.

- **Preparar equipos** (antes «Recetas»): cada receta es ahora una plantilla de preparación.
- **Usuarios y Cuentas**: accesos directos a las herramientas de Windows que completan la página:
  Usuarios y grupos locales, Cuentas de usuario, Perfiles de usuario, Directiva de seguridad local,
  Administrador de tareas, Administración de equipos y Administrador de credenciales. Lo que ya
  había sigue igual.
- **Agenda** (Soporte): visitas de mantenimiento con día, hora, duración, equipos y notas.
  - Aviso de Windows 30 minutos antes de cada visita, y un resumen del día al abrir AdminOps.
  - Lista de clientes a los que ya les toca mantenimiento, con «Agendar».
  - Desde cada visita: «Empezar» abre la sesión con el cliente ya elegido; recordatorio al cliente
    por el Correo; marcar como hecha, reprogramar o cancelar.
  - Las visitas de hoy y los mantenimientos vencidos aparecen en «Qué hacer ahora» del Panel.
- **Plantilla de informe por cliente** (en su ficha):
  - formato (cliente o técnico), que la sesión y la página Informe proponen solos;
  - texto de presentación al principio del informe;
  - destinatarios extra y asunto y mensaje del correo, con {cliente}, {contacto}, {numero},
    {fecha}, {equipo}, {empresa} y {tecnico}.
- **Enviar el informe con el Correo de AdminOps**: deja el mensaje escrito y abre la carpeta del PDF
  para arrastrarlo (el correo web no permite adjuntar el archivo automáticamente).

- **Barra lateral**: botón para acoplarla (solo iconos) y volver a abrirla, y ancho ajustable
  arrastrando su borde (doble clic vuelve al ancho de Ajustes). El ancho se guarda.
- **Menos secciones y menos páginas** (de 8 áreas y 44 páginas a 7 y 30):
  - **Aplicaciones** (antes «Programas»): Actualizar, Windows Update, Instalar, Desinstalar y
    Bloatware en pestañas.
  - **Ajustes de Windows** se lleva «Inicio de Windows» como pestaña; el área «Mantener» desaparece
    y la página pasa a «Equipo».
  - **Herramientas y atajos**: las utilidades de Windows y los atajos de teclado, juntos.
  - **Sesión de servicio** se lleva el «Informe» como pestaña.
  - **Datos del equipo**: Copia de datos, Caja fuerte, Borrado seguro, Recuperar archivos y Control
    parental en una sola página.
  - **Cuentas y dominio**: las cuentas del equipo y el dominio de la empresa, juntos.
  - **Impresoras y carpetas**: lo que la oficina comparte, en una sola página.
  - Los enlaces antiguos siguen funcionando: cada uno lleva a su pestaña nueva.
- **Soluciones** (antes «Conocimiento»): AdminOps trae 24 soluciones probadas paso a paso para los
  problemas de soporte más habituales (sin Internet, impresora atascada, Windows Update que falla,
  perfil temporal, pantallazos, Outlook que pide la contraseña, disco al 100 %, relación de
  confianza del dominio…). Se buscan y se copian igual que las propias, salen en Ctrl+K y, para
  cambiar una, se duplica.
- **Ajustes rediseñados**:
  - buscador que encuentra cualquier ajuste por su nombre o por lo que hace, sin tildes, y lleva a
    su sección resaltando la fila;
  - secciones en una columna a la izquierda, cada una con lo que hace;
  - sección nueva **Portales y correo**: precarga, zoom por defecto, dominio de la empresa y, por
    cada portal, sesión privada y entrar solo, con aviso si ninguno usa sesión privada.
- **WebView2**: se quita `--in-process-gpu`. Se puso en la 0.7 para ahorrar un proceso al arrancar,
  cuando la única vista era la interfaz; con los portales y el Correo dentro, hacía que todo el
  trabajo gráfico pasara por el proceso principal y provocaba tirones con webs pesadas.

- **Arreglos y rendimiento con datos reales** (medido en un equipo de trabajo):
  - El arranque (1,6 s) resultó ser casi todo de WebView2: los pasos propios suman 65 ms, así que
    **no** se partió `api.ts`; no habría ganado nada.
  - **El diagnóstico esperaba a winget** (84 s en ese equipo) y por eso tardaba 82 s. Ahora usa lo
    que tenga en caché y actualiza detrás: el diagnóstico deja de depender de winget.
  - **La información de la red** (19,9 s) sale del PowerShell compartido a su propio proceso.
  - **Las temperaturas** se comparten entre el Panel y Hardware (eran 39 lecturas de 300 ms).
  - Las **vistas web sin usarse 30 minutos se cierran** (cada una es un proceso).
  - Los **portales se recargan solos al volver la red** (VPN, cable, Wi-Fi).
  - Elegir un ancho de barra lateral en Ajustes ahora manda sobre el ajustado arrastrando.
  - `npm run setversion 1.1.5` cambia la versión en los cuatro sitios a la vez.
- **Avisos de Windows que llevan a su sitio**: Windows no dice qué aviso se ha pulsado, pero al
  pulsarlo pone AdminOps delante; si el aviso es reciente, la app abre la campana con lo pendiente.
- **Barras de pestañas sin barra de desplazamiento** y barra general discreta (solo al pasar por
  encima).
- **Etiquetas plegables** (Soluciones y Contactos): con muchas, se ve una fila y «Ver todas».
- **Diagnóstico: se pliega lo que está bien.** Las tarjetas sin problemas se quedan en una línea con
  «Sin problemas» y el detalle a un clic. Se puede desactivar con «Plegar lo que está bien».
- **Redactar un correo**: sin datos que rellenar se pulsa el botón propio de Outlook (aspecto de
  siempre); con destinatario o asunto, su enlace de redactar abre en una ventana propia.

- **Estado del equipo**: Diagnóstico, Hardware, Seguridad e Historial en una página con pestañas.
  Con ello «Equipo» queda en cuatro entradas y la barra lateral en 7 secciones, en este orden:
  Inicio, Equipo, Soporte, Red, Aplicaciones, Administración (antes «Oficina») y Datos.
- **Cada pestaña sabe si se ve**: antes una página con pestañas se creía visible entera, así que lo
  que hubiera en marcha en una pestaña oculta (temperaturas, métricas) seguía trabajando. Arreglado
  en `TabPanels`, así que vale para todas las páginas con pestañas.
- **Catálogo de programas**: de 72 a 113, con categoría **Drivers y utilidades del fabricante**
  (Intel, Dell, Lenovo, MSI, ASUS, DisplayLink, Logitech, Corsair, Razer y Snappy Driver Installer)
  y dos listas nuevas. Los 74 ids añadidos se comprobaron uno a uno contra winget: 7 no existían y
  se corrigieron o se quitaron. winget no distribuye drivers sueltos: lo que se instala son las
  utilidades oficiales que los buscan.
- **Espacio en disco, para soporte**:
  - **«Qué puedes liberar»**: temporales de Windows y del usuario, caché de Windows Update,
    papelera, `Windows.old`, hibernación y Descargas, medidos de verdad, con verde (se puede borrar)
    o naranja (míralo antes).
  - **Archivos grandes sin usar en más de un año**, con cuánto hace que se tocaron.
  - **Enviar a la papelera** lo que marques, sin salir de la app y pudiendo recuperarlo; queda en el
    Historial y el modo auditoría lo bloquea.

- **Las soluciones se ejecutan, no solo se leen.** Cada paso de las 24 soluciones que trae AdminOps
  lleva su botón cuando la app sabe hacerlo: vaciar la cola de impresión, reparar Windows Update,
  limpiar el DNS, SFC y DISM, reiniciar el audio, reparar un perfil temporal, sincronizar la hora,
  analizar con Defender… También abre la herramienta de Windows o la página donde se hace. Un test
  comprueba que los 24 juegos de botones apuntan a ids que existen de verdad.
- **El diagnóstico lleva a la solución**: cada hallazgo ofrece «Cómo se arregla» y abre los pasos.
  También en «Qué hacer ahora» del Panel, para los hallazgos sin acción directa.
- **Comparar los equipos de un cliente**: «PC-CONTA arranca 3 veces más lento que el resto de la
  oficina». Compara arranque, memoria, espacio libre y nota de seguridad con la mediana de los demás
  equipos, a partir de las cifras que ya guardaba cada visita. Solo datos técnicos del equipo, y hacen
  falta al menos tres para que la mediana signifique algo.
- **Dos modos, elegidos en la bienvenida y cambiables en Ajustes**:
  - **Modo técnico**: AdminOps completo, con el aviso de que toca registro, servicios, usuarios y
    arranque, y de que para cambiar el equipo hay que abrirlo como administrador.
  - **Modo usuario**: solo Panel, Estado del equipo, Espacio, Solucionar problemas, Acceso remoto y
    Ajustes. No se ven clientes, contactos, tickets ni correo del técnico. Con PIN puesto, volver al
    modo técnico lo pide. Es un modo de la interfaz, no una barrera de seguridad, y así se dice.
- **Bienvenida nueva**: modo, datos del técnico, tickets, correo (Outlook en un clic) y dominio, con
  atrás, saltar y un resumen final de lo que conviene saber.
- **Ayuda al día**: los textos del «?» que quedaron desfasados tras las fusiones, corregidos, y
  **ayuda propia en cada una de las 34 pestañas**, que antes no tenían ninguna.
- **Comprobación antes de instalar**: red, espacio libre, winget y permisos. Avisa antes en vez de
  dejar el equipo a medio hacer.
- **Arranque instrumentado**: se mide la carpeta de datos de WebView2 y el momento justo antes de
  crear la ventana, para saber con datos cuánto es de AdminOps y cuánto de WebView2.

### v1.1.5 — Teams, arranque y portales que no estorban ✅

- **Teams dentro de AdminOps** (Soporte → Teams): chats, equipos y reuniones sin instalar Teams en el
  equipo del cliente. Se usa Teams en la web, que es la versión que funciona en cualquier equipo y no
  deja nada instalado; la cuenta se guarda cifrada como la del Correo y el inicio de sesión se rellena
  solo. Del trabajo (Microsoft 365) o personal, con sesión privada recomendada en equipos ajenos.
  Solo navega por los dominios de Teams y de Microsoft: cualquier otro enlace se abre en el navegador.
  Para hablar en una reunión, Windows pide permiso de micrófono y cámara la primera vez.
  Desde **Contactos**, «Teams» abre el chat aquí dentro si hay Teams configurado; si no, en la
  aplicación de Teams del equipo, como antes.
- **Los portales dejan de estorbar cuando no se ven.** Un portal oculto (o precargado) se duerme:
  deja de consumir procesador y batería, igual que una pestaña de fondo del navegador. Era el motivo
  de que el Correo, con Outlook abierto, pusiera lenta toda la aplicación aunque estuvieras en otra
  página. Se despierta solo al volver a él.
- **Portales más ligeros al moverse**: colocar la vista hay que pedírselo al hilo de la ventana, y al
  arrastrar el borde se mandaban decenas de peticiones por segundo. Ahora va una por fotograma como
  mucho y las que no cambian nada no se mandan. Además, el estado de la página (título, historial)
  solo repinta la interfaz cuando cambia de verdad: Outlook lo repetía sin parar.
- **La ventana ya no se abre en negro.** Se enseña cuando la interfaz tiene algo pintado, con un
  «Abriendo AdminOps…» mientras tanto, y si WebView2 no arranca en 12 segundos se recarga la vista
  sola, en vez de tener que cerrar y volver a abrir.
- **Portable: primer arranque arreglado.** La carpeta de datos de WebView2 del USB se crea y se
  comprueba antes de dársela a WebView2; si el USB viene protegido contra escritura o el antivirus
  del cliente bloquea la primera creación, se usa la del equipo en lugar de quedarse la ventana en
  negro. Era la causa de que la primera vez hubiera que cerrar y volver a abrir.
- **Arranque más despejado**: las consultas pesadas de Defender y BitLocker, la limpieza automática y
  el calentamiento de PowerShell se apartan unos segundos para no competir con la primera pantalla.
- **Asistencia rápida sin el aviso en inglés de Windows**: se comprueba antes si el equipo la tiene
  (app de la Store en Windows 11, `quickassist.exe` en Windows 10). Si no está, se abre su ficha de la
  Microsoft Store y se explica en español, en vez del «This file does not have an app associated…».
- **«Volver a ver la bienvenida»** en Ajustes → General, junto a la página de inicio.

### Ideas nuevas (propuestas, sin fecha)

**Taller y órdenes de trabajo**
- **Órdenes de taller**: al recibir un equipo, una orden con número, estado del equipo, accesorios
  entregados y lo que dice el cliente. Estados: recibido → en reparación → esperando pieza → listo →
  entregado. Etiqueta imprimible para pegar en el equipo y aviso al cliente cuando esté listo.
- **Fotos del equipo**: al recibirlo y al entregarlo (golpes, pantalla, puertos), adjuntas a la orden
  y al informe, para evitar discusiones.
- **Repuestos**: tu stock de piezas (discos, RAM, cargadores) con coste y precio. Lo que se usa en
  una visita se descuenta y pasa al recibo; aviso cuando queda poco.

**Clientes con contrato**
- **Bolsa de horas**: horas contratadas por cliente, consumo en cada visita (sale del tiempo de la
  sesión) y aviso cuando se acaban.
- **Informe mensual del cliente**: todas las visitas del mes, equipos atendidos, horas y problemas
  que se repiten, en un solo PDF con tu marca.
- **Incidencias que se repiten**: por cliente y por equipo (la impresora de recepción, el portátil
  de contabilidad), para proponer una solución de fondo o una renovación.

**Cobros y números del técnico**
- **Cobros pendientes**: estado de cada recibo (pendiente, pagado, parcial), recordatorio de pago
  por el Correo y lista de lo que te deben.
- **Resumen del mes**: visitas, horas, ingresos, clientes nuevos y lo pendiente de cobro. Se exporta
  a Excel para la contabilidad.

**Entrega al cliente**
- **Pantalla de entrega**: una vista limpia para enseñar al cliente en su propio equipo el antes y
  el después (espacio, arranque, seguridad) y que firme ahí mismo.

### Fase 26 — Negocio (aparcada)

> **Aparcada**: cobrar no es el plan ahora. Se deja escrita por si algún día lo es.

**Prioridad: media.** Lo que ayuda a cobrar y a que el cliente vuelva.

- **Estadísticas**: ingresos por mes, servicios más vendidos y clientes frecuentes (de los recibos).
- **Enviar el informe por WhatsApp** con el número del cliente y el mensaje listo.
- **Avisos al abrir la app**: mantenimientos vencidos y garantías que terminan.
- **Asistencia remota** apuntada en la sesión (invitación de Asistencia remota o id de AnyDesk/RustDesk).

### Fase 27 — Plantillas de preparación (más adelante)

**Prioridad: media.** Automatizar lo que el técnico repite en cada equipo.

- **Plantillas**: secuencias guardadas de pasos (diagnóstico → limpieza → perfil → programas → copia de
  drivers → informe) que se ejecutan con un clic y muestran el progreso de cada paso.
- Plantillas incluidas: "Equipo nuevo", "PC lento", "Después de un virus", "Antes de formatear".
- **Exportar e importar plantillas** para compartirlas con otros técnicos.

**Terminado cuando:** preparar un equipo nuevo es elegir una plantilla y esperar.

### Fase 28 — Distribución (más adelante)

- **Actualización automática** con `tauri-plugin-updater` y GitHub Releases (clave de firma propia y
  gratuita: la app solo instala actualizaciones publicadas por ti). La 1.0 ya avisa de versiones nuevas.
- **Releases automáticas** desde la CI al crear una etiqueta `vX.Y.Z` (instalador + portable).
- **Inglés** completo (interfaz, catálogos e informe) y selector de idioma.
- **Página de descarga** en GitHub Pages con capturas, novedades y la nota sobre SmartScreen.

### Fase 29 — Preparación comercial (aparcada)

> **Aparcada**: cobrar no es el plan ahora. Se deja escrita por si algún día lo es.

**Prioridad: a decidir** según cómo se quiera vender.

- **Modelo gratis + Pro**: lo esencial gratis; informes con marca propia, plantillas de preparación, clientes ilimitados
  y la oficina completa en Pro.
- **Licencias sin servidor**: clave firmada (Ed25519) que la app verifica sin conexión, asociada al
  técnico y no al equipo, para que funcione en modo portable.
- **Términos de uso y privacidad** (EULA) en el instalador y en Acerca de.
- **Canal de soporte**: el paquete de soporte (Fase 14) se envía por correo con un clic.
