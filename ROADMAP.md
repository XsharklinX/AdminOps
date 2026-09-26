# AdminOps — Hoja de ruta

Estado actual: **v0.5.0**. Este documento recoge lo que ya está hecho, la deuda técnica
conocida y las próximas fases en orden de prioridad. Cada fase tiene un criterio de
"terminado" para saber cuándo cerrarla.

---

## Hecho (v0.1 – v0.5)

| Fase | Versión | Contenido |
|---|---|---|
| 1. Base | 0.1 | Tauri 2 + React + Tailwind, tema oscuro neón, UAC, panel del sistema en vivo |
| 2. Motor de ajustes | 0.2 | Catálogo TOML declarativo, detección de estado, diario con copia exacta, deshacer, puntos de restauración |
| 3. Catálogo | 0.3 | Limpieza, privacidad, rendimiento, servicios, bloatware (Appx) e Inicio; usuario destino para HKCU |
| 4. Diagnóstico | 0.4 | Discos, BSOD, drivers, batería, seguridad, reparaciones, informe PDF antes/después, ícono |
| 5. Pulido | 0.5 | Perfiles de un clic, modo portable, firma de código preparada, versión unificada |

---

## Deuda técnica conocida

Cosas que funcionan pero tienen límites o riesgos. Las marcadas ⚠️ conviene resolverlas pronto.

- ⚠️ **PowerShell sin timeout.** `ps::powershell` espera indefinidamente; un script colgado
  deja la tarjeta en "Trabajando…" para siempre. No se puede cancelar SFC/DISM.
- ⚠️ **Scripts y usuario destino.** La redirección de HKCU al usuario con sesión abierta solo cubre
  las acciones de registro. Los scripts que usan `$env:LOCALAPPDATA`, `$env:TEMP` o `HKCU:`
  (caché de iconos, temporales, papelera) siguen actuando sobre la cuenta que elevó AdminOps.
- ⚠️ **Sin CI ni pruebas de ida y vuelta.** Los tests cubren el catálogo, el registro y el formato
  de fechas, pero nadie verifica automáticamente que *cada* ajuste se aplique y se deshaga bien.
- **Un PowerShell por consulta.** Cada detección por script arranca un proceso (~300 ms).
  Listar apps o el Inicio tarda 1–2 s.
- **Portable deja rastro de WebView2.** La caché del webview va a `%LOCALAPPDATA%\com.adminops.app`
  aunque el resto de datos esté en el USB.
- **SMART limitado.** `Get-StorageReliabilityCounter` no reporta temperatura/desgaste en muchos
  discos SATA; faltan los atributos SMART crudos (sectores reasignados, pendientes).
- **Punto de restauración sin progreso.** Crearlo puede tardar más de un minuto sin feedback.
- **Dependencia de Edge para el PDF.** Si falta, el informe cae a HTML.
- **Solo español.**

---

## Fase 6 — Robustez y calidad

**Prioridad: alta.** Antes de añadir funciones, que lo existente sea fiable en equipos de clientes.

- Timeout configurable por script y **botón Cancelar** en tareas largas (matar el proceso de PowerShell).
- Redirección completa al usuario destino en scripts: inyectar `$UserProfile`, `$UserTemp` y
  `$UserHive` y reescribir los scripts del catálogo para usarlos.
- **Registro de actividad** a archivo (`tauri-plugin-log`), con rotación y un visor en la app
  para adjuntarlo cuando algo falle.
- **Pruebas de ida y vuelta** en Windows Sandbox: script que aplica y deshace todo el catálogo
  y compara el registro/servicios antes y después.
- **CI con GitHub Actions**: `cargo test`, `cargo clippy -D warnings`, `tsc`, compilación y
  artefactos (instalador + zip portable) en cada push a `main`.
- Progreso visible al crear puntos de restauración.

**Terminado cuando:** ninguna operación puede colgar la interfaz, la prueba de ida y vuelta pasa
en limpio sobre Windows 10 22H2 y Windows 11 24H2, y la CI está en verde.

---

## Fase 7 — Rendimiento de la propia app

**Prioridad: alta.** Una app que optimiza el PC tiene que ser ligera.

- Sustituir PowerShell por APIs nativas donde sea posible: WMI (`wmi` crate) para discos,
  drivers y batería; `windows` crate para Appx y tareas programadas.
- Para lo que siga en PowerShell: un proceso persistente reutilizado (runspace) en vez de uno por consulta.
- Caché de detección con invalidación al aplicar/deshacer.
- Carga diferida de páginas (`React.lazy`) y medición del tamaño del bundle.

**Objetivos medibles:** arranque < 1 s, < 80 MB de RAM y < 1 % de CPU en reposo,
listas de Apps e Inicio en < 500 ms.

---

## Fase 8 — Nuevas herramientas de diagnóstico y mantenimiento

**Prioridad: media.** Lo que más tiempo ahorra en el día a día de soporte.

- **Actualizar software** con `winget upgrade`: lista de apps desactualizadas y actualización por lotes.
- **Analizador de espacio**: carpetas y archivos más grandes por unidad, con acceso directo a limpiarlos.
- **SMART completo** con `smartctl` opcional (sectores reasignados/pendientes, CRC), con alertas.
- **Temperaturas** de CPU y GPU (LibreHardwareMonitor) en el panel y en el informe.
- **Diagnóstico de red**: adaptadores, ping a puerta de enlace/DNS/Internet, resolución DNS, velocidad.
- **Copia de drivers** (`pnputil /export-driver`) antes de formatear o actualizar.
- **Escaneo rápido de Defender** desde la app, con resultado en el informe.
- **Benchmark antes/después**: tiempo de arranque, lectura/escritura de disco y latencia de apertura de apps.

**Terminado cuando:** cada herramienta aparece también en el diagnóstico o en el informe, no solo en su página.

---

## Fase 9 — Flujo de trabajo del técnico

**Prioridad: media.** Convertir AdminOps en la herramienta de "sesión de servicio".

- **Sesión de trabajo**: al empezar hace el diagnóstico "antes"; al terminar genera el informe
  con comparación y trabajo realizado, en un solo flujo.
- **Perfiles personalizados**: crear, editar, exportar e importar (TOML/JSON) los tuyos.
- **Fichas de cliente y equipo**: nombre, contacto, equipos atendidos y el historial de informes de cada uno.
- **Informe con marca propia**: logo, datos de contacto y condiciones del técnico o empresa.
- **Checklist de servicio** configurable (copia de seguridad hecha, antivirus revisado…) que se incluye en el informe.

**Terminado cuando:** una visita completa (diagnóstico → trabajo → informe) se hace sin salir de la app.

---

## Fase 10 — Distribución

**Prioridad: media-baja** (pasa a alta al empezar a repartir la app a otros).

- **Firma de código real**: Azure Trusted Signing o certificado OV. El script `scripts/sign.ps1`
  ya está conectado: basta con definir las variables de entorno.
- **Actualización automática** con `tauri-plugin-updater` y GitHub Releases.
- **Releases automáticas** desde la CI al crear una etiqueta `vX.Y.Z`.
- Publicación en **winget** (`winget install AdminOps`).
- **Inglés** como segundo idioma (i18n de la interfaz, el catálogo y el informe).

**Terminado cuando:** instalar y actualizar AdminOps no muestra avisos de SmartScreen y no requiere pasos manuales.
