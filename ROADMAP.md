# AdminOps — Hoja de ruta

Estado actual: **v0.16.0**. Este documento recoge lo que ya está hecho, la deuda técnica
conocida y las próximas fases en orden de prioridad. Cada fase tiene un criterio de
"terminado" para saber cuándo cerrarla.

---

## Hecho (v0.1 – v0.16)

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
- **Solo español** (inglés en la Fase 20).
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

## Plan (v0.10 – v1.1)

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

### Fase 17 — Informe y cliente (v0.17)

**Prioridad: media.** Lo que convierte el trabajo técnico en algo que el cliente valora (y paga).

- **Dos plantillas de informe**: resumen en lenguaje sencillo para el cliente y detalle técnico.
- **Presupuesto y recibo** en la sesión: líneas de servicio, precios, impuestos y total en el PDF.
- **Firma del cliente** en pantalla al cerrar la sesión.
- **Enviar el informe**: correo con el PDF adjunto y los datos del cliente ya puestos.
- **Comparar visitas** en la ficha del cliente y **recordatorios** de próximo mantenimiento.
- **Garantías**: fecha de fin de garantía del trabajo y de las piezas cambiadas.

**Terminado cuando:** una sesión termina con informe, recibo y firma sin salir de la app.

### Fase 18 — La oficina completa (v0.18)

**Prioridad: media.** Pasar de "un equipo" a "la red del cliente".

- **Escáner de red local**: equipos, impresoras y dispositivos con IP, MAC, fabricante y nombre;
  detectar IP duplicadas y dispositivos desconocidos.
- **Wake-on-LAN** y **conexión remota** (escritorio remoto, Asistencia rápida) desde la ficha del
  equipo o del cliente.
- **Carpetas compartidas y permisos** del equipo: qué se comparte y con quién.
- **Inventario de la oficina**: cada equipo diagnosticado queda en la ficha del cliente con su
  hardware, para ver el parque completo y qué equipos conviene renovar.

**Terminado cuando:** el técnico puede describir y mantener la red entera de un cliente pequeño.

### Fase 19 — Recetas (v0.19)

**Prioridad: media.** Automatizar lo que el técnico repite en cada equipo.

- **Recetas**: secuencias guardadas de pasos (diagnóstico → limpieza → perfil → programas → copia de
  drivers → informe) que se ejecutan con un clic y muestran el progreso de cada paso.
- Recetas incluidas: "Equipo nuevo", "PC lento", "Después de un virus", "Antes de formatear".
- **Exportar e importar recetas** para compartirlas con otros técnicos.

**Terminado cuando:** preparar un equipo nuevo es elegir una receta y esperar.

### Fase 20 — Versión 1.0 y distribución (v1.0)

**Prioridad: alta cuando lo anterior esté probado en uso real.**

- **Actualización automática** con `tauri-plugin-updater` y GitHub Releases (clave de firma propia y
  gratuita: la app solo instala actualizaciones publicadas por ti).
- **Releases automáticas** desde la CI al crear una etiqueta `vX.Y.Z` (instalador + portable).
- **Inglés** completo (interfaz, catálogos e informe) y selector de idioma.
- **Página de descarga** en GitHub Pages con capturas, novedades y la nota sobre SmartScreen.

**v1.0 cuando:** actualizaciones automáticas funcionando, inglés completo y dos semanas de uso real sin
fallos graves.

### Fase 21 — Preparación comercial (v1.1)

**Prioridad: a decidir** según cómo se quiera vender.

- **Modelo gratis + Pro**: lo esencial gratis; informes con marca propia, recetas, clientes ilimitados
  y la oficina completa en Pro.
- **Licencias sin servidor**: clave firmada (Ed25519) que la app verifica sin conexión, asociada al
  técnico y no al equipo, para que funcione en modo portable.
- **Términos de uso y privacidad** (EULA) en el instalador y en Acerca de.
- **Canal de soporte**: el paquete de soporte (Fase 14) se envía por correo con un clic.
