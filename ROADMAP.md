# AdminOps — Hoja de ruta

Estado actual: **v1.1.1**. Este documento recoge lo que ya está hecho, la deuda técnica
conocida y las próximas fases en orden de prioridad. Cada fase tiene un criterio de
"terminado" para saber cuándo cerrarla.

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
- **Notificaciones en portable**: sin instalador, Windows puede mostrarlas con otro nombre de app.
- **Tickets incrustados** usan la API "unstable" de Tauri (vistas hijas); si una versión futura la
  cambia, queda la ventana aparte como alternativa.

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
  - **Contactos** (Soporte): agenda del técnico con extensión, teléfonos, correo, para qué llamarle,
    etiquetas y favoritos; importa y exporta CSV (Excel y Outlook) y se busca desde Ctrl+K. Viaja con
    AdminOps (en el USB con el portable).
  - **Portable entre equipos**: las contraseñas de routers se cifran con una clave del USB (AES-256-GCM)
    en vez de DPAPI, así funcionan en todos los equipos; las antiguas se migran solas. Ajustes explica
    qué viaja contigo y qué es de cada equipo.

### Fase 26 — Negocio (más adelante)

**Prioridad: media.** Lo que ayuda a cobrar y a que el cliente vuelva.

- **Estadísticas**: ingresos por mes, servicios más vendidos y clientes frecuentes (de los recibos).
- **Enviar el informe por WhatsApp** con el número del cliente y el mensaje listo.
- **Avisos al abrir la app**: mantenimientos vencidos y garantías que terminan.
- **Asistencia remota** apuntada en la sesión (código de Asistencia rápida o AnyDesk).

### Fase 27 — Recetas (más adelante)

**Prioridad: media.** Automatizar lo que el técnico repite en cada equipo.

- **Recetas**: secuencias guardadas de pasos (diagnóstico → limpieza → perfil → programas → copia de
  drivers → informe) que se ejecutan con un clic y muestran el progreso de cada paso.
- Recetas incluidas: "Equipo nuevo", "PC lento", "Después de un virus", "Antes de formatear".
- **Exportar e importar recetas** para compartirlas con otros técnicos.

**Terminado cuando:** preparar un equipo nuevo es elegir una receta y esperar.

### Fase 28 — Distribución (más adelante)

- **Actualización automática** con `tauri-plugin-updater` y GitHub Releases (clave de firma propia y
  gratuita: la app solo instala actualizaciones publicadas por ti). La 1.0 ya avisa de versiones nuevas.
- **Releases automáticas** desde la CI al crear una etiqueta `vX.Y.Z` (instalador + portable).
- **Inglés** completo (interfaz, catálogos e informe) y selector de idioma.
- **Página de descarga** en GitHub Pages con capturas, novedades y la nota sobre SmartScreen.

### Fase 29 — Preparación comercial (más adelante)

**Prioridad: a decidir** según cómo se quiera vender.

- **Modelo gratis + Pro**: lo esencial gratis; informes con marca propia, recetas, clientes ilimitados
  y la oficina completa en Pro.
- **Licencias sin servidor**: clave firmada (Ed25519) que la app verifica sin conexión, asociada al
  técnico y no al equipo, para que funcione en modo portable.
- **Términos de uso y privacidad** (EULA) en el instalador y en Acerca de.
- **Canal de soporte**: el paquete de soporte (Fase 14) se envía por correo con un clic.
