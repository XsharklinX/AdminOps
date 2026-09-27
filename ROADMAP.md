# AdminOps — Hoja de ruta

Estado actual: **v0.11.0**. Este documento recoge lo que ya está hecho, la deuda técnica
conocida y las próximas fases en orden de prioridad. Cada fase tiene un criterio de
"terminado" para saber cuándo cerrarla.

---

## Hecho (v0.1 – v0.11)

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

- **Portable deja rastro de WebView2.** La caché del webview va a `%LOCALAPPDATA%\com.adminops.app`
  aunque el resto de datos esté en el USB.
- **SMART limitado.** `Get-StorageReliabilityCounter` no reporta temperatura/desgaste en muchos
  discos SATA; faltan los atributos SMART crudos (sectores reasignados, pendientes).
- **La prueba de ida y vuelta aún no se ha ejecutado** en un Windows 11 cliente limpio: la CI usa
  Windows Server. Ejecutarla en Sandbox al menos una vez por versión.
- **Dependencia de Edge para el PDF.** Si falta, el informe cae a HTML.
- **Sin firma de código** (decisión: no se contempla por coste). SmartScreen avisará al instalar.
- **Solo español.**

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

## Plan siguiente (v0.10 – v1.0)

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
- Pendiente de verificar a mano: crear, modificar y borrar usuarios reales y olvidar una red Wi-Fi
  (se hará en la VM de la Fase 13).

### Fase 12 — Después de formatear (v0.12)

**Prioridad: alta.** Lo que el técnico hace en cada equipo recién instalado o migrado.

- **Instalación de programas en lote** con winget: listas propias (navegador, 7-Zip, VLC, AnyDesk,
  Office…) que se instalan de una vez, con progreso y resultado por programa.
- **Copia de datos del usuario** a un USB o disco (Escritorio, Documentos, Imágenes, favoritos de los
  navegadores, perfiles Wi-Fi) y restauración en el equipo nuevo.
- **Red avanzada**: cambiar DNS con un clic (Cloudflare, Google, automático) por adaptador, editor del
  archivo hosts, ping y tracert integrados, puertos en uso por programa.
- **Impresoras**: lista, limpiar la cola atascada, página de prueba, quitar impresoras fantasma.
- **Pantallazos azules**: driver culpable a partir de los minidumps.
- **Informe de batería** en portátiles: capacidad real frente a la de fábrica.

**Terminado cuando:** dejar listo un equipo recién formateado (programas, datos y red) no requiere
salir de AdminOps.

### Fase 13 — Base sólida antes de crecer (v0.13)

**Prioridad: alta.** Cerrar lo que quedó sin comprobar y los detalles que se notan en el uso diario.

- **Verificación en limpio**: ejecutar la prueba de ida y vuelta en una VM de Windows 11
  (VirtualBox ya está instalado en el equipo de desarrollo) y una sesión de servicio completa con
  administrador: finalizar procesos, actualizar software, copia de drivers, perfiles.
- **Primer arranque guiado**: asistente de 3 pasos (tu nombre y logo, modo instalado/portable,
  diagnóstico inicial) para que la app no empiece "vacía".
- **Portable sin rastro**: llevar también la caché de WebView2 a `AdminOps-data` (hoy queda en
  `%LOCALAPPDATA%` del cliente).
- **Paquete de soporte**: un botón que junta registro, último diagnóstico y versión en un .zip para
  cuando algo falle.
- **Medición de CPU en reposo** con el equipo sin carga y fijar la cifra definitiva.

**Terminado cuando:** todo lo marcado como "pendiente de verificar" esté probado y un usuario nuevo
llegue a su primer informe sin instrucciones.

### Fase 14 — Más mantenimiento y reparación (v0.14)

**Prioridad: media.**

- **Desinstalador de programas** (Win32 y Store) con desinstalación silenciosa y limpieza de restos
  (carpetas y claves huérfanas), mostrando el tamaño que libera cada uno.
- **Limpieza de navegadores** (Chrome, Edge, Firefox, Opera) por perfil del usuario destino: caché,
  sin tocar contraseñas ni historial salvo que se elija.
- **Restaurar drivers** desde una copia hecha con AdminOps (`pnputil /add-driver`).
- **Más reparaciones**: restablecer Microsoft Store (`wsreset`), reparar la búsqueda de Windows,
  reiniciar el audio, reparar asociaciones de archivos.
- **Mantenimiento programado**: limpieza semanal o mensual como tarea programada del sistema,
  opcional y reversible desde la app.
- **Gestión de puntos de restauración**: ver el espacio que ocupan y borrar los antiguos.

**Terminado cuando:** las tareas habituales de "PC lento" se resuelven sin salir de AdminOps.

### Fase 15 — Experiencia profesional (v0.15)

**Prioridad: media.** Que se sienta como un producto terminado.

- **Búsqueda global (Ctrl+K)**: saltar a cualquier página, ajuste, reparación o proceso escribiendo.
- **Notificaciones de Windows** al terminar tareas largas (SFC, DISM, speedtest, sesión).
- **Tema claro** opcional y tamaño de texto ajustable; la ventana recuerda tamaño y posición.
- **Atajos de teclado** y navegación completa sin ratón.
- **Panel más útil**: tendencia de temperaturas y alertas en vivo (RAM llena, disco al 100 %).
- **Comparar visitas** en la ficha del cliente: cómo evolucionó el equipo entre sesiones.

### Fase 16 — Informe y relación con el cliente (v0.16)

**Prioridad: media.**

- **Dos plantillas de informe**: resumido para el cliente (lenguaje sencillo) y técnico detallado.
- **Presupuesto o recibo** opcional en la sesión: líneas de servicio, precios y total en el PDF.
- **Conformidad del cliente**: firma a mano en pantalla al cerrar la sesión.
- **Enviar el informe**: abrir el correo con el PDF adjunto y los datos del cliente ya puestos.
- **Recordatorios**: "próximo mantenimiento" por cliente, visibles en Clientes.

### Fase 17 — Distribución gratuita (v1.0)

**Prioridad: media.**

- **Actualización automática** con `tauri-plugin-updater` y GitHub Releases. Usa una clave propia
  gratuita (no un certificado): la app solo instala actualizaciones firmadas por ti.
- **Releases automáticas** desde la CI al crear una etiqueta `vX.Y.Z` (instalador + portable).
- **Inglés** como segundo idioma (interfaz, catálogo e informe).
- **Página de descarga** en GitHub Pages con capturas y novedades.
- **Nota sobre SmartScreen**: sin certificado, Windows avisará al abrir el instalador las primeras
  veces; el aviso se reduce solo a medida que el archivo acumula descargas. Documentarlo en la página.

**v1.0 cuando:** actualizaciones automáticas funcionando, inglés completo y dos semanas de uso real
en clientes sin fallos graves.
