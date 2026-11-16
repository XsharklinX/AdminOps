# AdminOps — tanda 1.3 (empezada el 2026-11-16)

Leer esto primero en cada sesión nueva. Junta tres listas aprobadas por el autor:
- **Rapidez** (artifact QEtoJJr1BUUxwDnf7aa9gW): R1–R5, F1, F2, N1, N3, N5, N6, P1. Detalle en ROADMAP.md, «v1.2.11».
- **Un día con AdminOps** (artifact NHa3f6FC9vmqrSXEeSLnRg): A2, B1, B2, B3, C1, C2, C3, D1, D2, F1, G1, G2.
- **Impresoras RICOH y EPSON** (pedido directo, 2026-11-16).

Lo que pidió el autor: «nuestra app debe ser lo más profesional y útil posible para que cualquier
persona que la utilice tenga la capacidad de dar mantenimiento y hacer funcionar y mejorar una PC».
Va a enseñársela a su supervisor en unos días. App de **un solo técnico**: nada de varios usuarios.
Las ideas le parecieron «raras o básicas»: hacerlas a fondo y que se noten, no la versión mínima.

Reglas: sin build ni commit sin que lo pida; nada que necesite permisos de IT; sin datos personales en
la interfaz; textos en español. Antes de cambiar algo visual nuevo (la pantalla de drivers), enseñar
el antes y el después.

## Estado
- 1.2.10 guardada en el commit 577029e (rama claude/great-mayer-m7kpmy), sin lo de la Fase 1.
- 2026-11-16: **Fase 1 hecha** (sin commit; David solo pidió el de la 1.2.10). Detalle abajo.
- Fase 2: David eligió las seis partes (P1–P6); plan de implementación abajo, en curso. Investigado: RICOH (PCL6 Driver for Universal Print 4.42.0.0, jul 2025) y EPSON (Universal Print Driver) solo se bajan desde la página de soporte de cada modelo, sin enlace directo estable; por eso las fuentes son Windows Update → biblioteca local → abrir la página oficial y recoger la descarga de Descargas. Generador de la maqueta: gen3.py en el scratchpad de la sesión 01e269f2. Idea de la sesión anterior: módulo `printvendor.rs` (PnP + colas + Windows Update
  con ServerSelection=2 + biblioteca de INF + `Set-Printer -DriverName` con deshacer).

## Orden propuesto (el autor puede cambiarlo)

### Fase 0 — Guardar la 1.2.10
- [x] Commit 577029e (David dijo «sí, haz commit», 2026-11-16).

### Fase 1 — Red de seguridad (primero, para que lo demás no rompa nada)
- [x] **G2** `tests/exe/exe.mjs` (`npm run test:exe`; `--exe ruta.exe`, `--quick`). Arranca el .exe en
      una carpeta portable limpia, se conecta a WebView2 por CDP, recorre las pantallas en los dos temas,
      portal de prueba propio (no se recarga al volver y conserva la cookie), clic de ratón de verdad en
      Ajustes tras el portal y cierre sin procesos colgados. Enganchada al final de `build-release.ps1`
      (si falla, la carpeta pasa a `v<ver>-NO-DISTRIBUIR`; `ADMINOPS_SKIP_EXE_TEST=1` la salta).
      Arreglos a la prueba en esta sesión: ajustes `{"onboarded":true}` antes de arrancar (si no, el
      portable «vacío» se trae los datos del equipo con `closeMinimizes` y portales, y sale el asistente);
      recargar tras marcar la versión vista (las Novedades tapaban el portal); pantallas que son pestaña
      de otra según `PAGE_ALIAS`; captura que se atasca = fallo, no caída; si revienta, cierra igual;
      Alt + SetForegroundWindow antes del clic y dice qué ventana había debajo (otro programa ≠ fallo);
      el portal escucha el clic (si le llega, sigue encima: el fallo de la 1.1.6); espera hasta 10 s a
      que WebView2 cierre sus procesos. Lint: `tests/exe` y `tests/perf` con globals de Node + navegador.
      **Comprobado**: .exe de depuración actual (`cargo build --features tauri/custom-protocol`),
      prueba completa pasa (arranque ~2 s). Ojo: el clic mueve el ratón de verdad; no tocar nada
      mientras corre o falla con «el clic cayó en otro programa».
- [x] **P1** `tests/perf/budget.mjs` (`npm run test:perf`), referencia `tests/perf/baseline.json`
      (1977 KB, 49 trozos; Merged.js 764 KB e index.js 584 KB: R1/R5 los bajarán). Pasa.
- [x] **Fallo real encontrado por G2 y arreglado**: en el programa compilado la vista previa del informe
      (iframe `srcDoc`, `ReportPreview.tsx`) salía sin estilos: Tauri añade un nonce a `style-src` y eso
      anula `'unsafe-inline'`. Arreglo: `"dangerousDisableAssetCspModification": ["style-src"]` en
      `tauri.conf.json` (los scripts siguen con nonce). Comprobado con el .exe: sin errores de CSP.

### Fase 2 — Impresoras y escáneres RICOH y EPSON (elegidas P1–P6 el 2026-11-16)
Maqueta aprobada: https://claude.ai/artifact/GFCqRBdNeQP9uyDBw7mmGV (las seis partes: Sí).
Fuentes de drivers, en este orden: **Windows Update** (WUA con `ServerSelection = 2`, oficiales y
firmados) → **biblioteca local** (carpeta o pendrive) → **página oficial** de RICOH o EPSON abierta por
AdminOps con el modelo copiado al portapapeles, y **recogida automática** del archivo que llegue a
Descargas. RICOH (PCL6 Driver for Universal Print 4.42.0.0, jul 2025) y EPSON no tienen enlaces
directos estables: nunca se baja nada a ciegas ni de webs de terceros.

Base que ya existe: `printers.rs` (`list_printers` por spooler, `on_printer` + `tweaks.record`,
`print_test_page`, `check_printer`, `find_network_printers`), `snmp.rs` (`printer_identity`,
`printer_status`), `fixes/printdeep.rs` (`brand`, `generic_driver`, reinstalación limpia),
`drivers.rs` (copia con pnputil), `paths::drivers_backup_dir`, `audit.rs` (`BLOCKED` = comandos que
cambian el equipo; la prueba exige que cada comando esté en `SAFE` o en `BLOCKED`).
Interfaz: `src/pages/Printers.tsx`; pestañas en `src/pages/Merged.tsx` (`SharedTab`, hoy
`printers | shares`); alias en `Sidebar.tsx` (`PAGE_ALIAS`); API en `src/lib/api/install.ts`.

**Encargo A — motor (Rust, módulo nuevo `printvendor.rs`)**
- [ ] `printer_inventory()`: por cada impresora instalada (no virtual): marca (RICOH/EPSON/otra),
      modelo limpio, conexión (USB / red + IP del puerto / WSD), driver (nombre, versión, fecha,
      fabricante, inf de `Get-PrinterDriver`), tipo de driver (fabricante / genérico de Microsoft /
      de otra marca). En las de red, por SNMP: modelo (`hrDeviceDescr` 1.3.6.1.2.1.25.3.2.1.3.1),
      serie (`prtGeneralSerialNumber` 1.3.6.1.2.1.43.5.1.1.17.1) y páginas (`prtMarkerLifeCount`
      1.3.6.1.2.1.43.10.2.1.4.1.1), en paralelo y con tiempo límite corto. Además, los aparatos
      conectados **sin instalar**: PnP con VID 05CA (RICOH) o 04B8 (EPSON), o clase Printer con
      código de problema 28.
- [ ] `scanner_inventory()`: dispositivos de imagen (clase PnP «Image» / WIA) de RICOH, EPSON y
      PFU/Fujitsu (fi y ScanSnap, ahora RICOH): driver (`Win32_PnPSignedDriver`: versión y fecha),
      tipo (WIA, TWAIN, PaperStream IP), estado (falta / funciona) y si están instalados los
      programas de escanear (Epson Scan 2, PaperStream Capture/IP, ScanSnap Home; claves Uninstall).
- [ ] **Biblioteca**: carpeta en `Settings` (por defecto `<datos>/drivers`, viaja en el portable).
      `driver_library()` recorre la carpeta, lee los `.inf` (`[Version] DriverVer`, `Class`,
      `Provider` y los modelos de las secciones de fabricante), solo x64, y guarda un índice json.
      `driver_library_add(path)`: copia un paquete (carpeta, `.zip`, o `.exe` autoextraíble que se
      pueda abrir como zip; si no se puede abrir, se guarda tal cual como «instalador del fabricante»).
- [ ] `driver_candidates(device)`: lo que hay para ese modelo en la biblioteca y en Windows Update
      (búsqueda WUA `IsInstalled=0 and Type='Driver'`, filtrada por ID de hardware o modelo;
      tarda: se hace bajo demanda, con caché de la última búsqueda). Estado final: al día / viejo
      (con la versión nueva y de dónde sale) / genérico / falta / de otra marca / **sin comparar**
      (nunca «al día» si no hay con qué comparar).
- [ ] `printer_driver_update(printer, candidate)` (BLOCKED, requiere administrador): pasos con
      progreso: 1) copia del driver actual (`pnputil /export-driver` a `drivers_backup_dir`);
      2) instalar (`pnputil /add-driver <inf> /install` o la actualización de Windows Update);
      3) `Add-PrinterDriver` con el nombre de modelo del inf; 4) `Set-Printer -DriverName`
      conservando nombre, puerto, compartida y predeterminada; 5) página de prueba. Devuelve un id de
      deshacer. `printer_driver_undo(id)` (BLOCKED): vuelve al driver anterior (no se desinstala el
      viejo, así deshacer es inmediato). Todo con `tweaks.record`, también los fallos.
- [ ] `open_vendor_page(brand, model)`: abre la página oficial de soporte (RICOH / EPSON) y copia
      el modelo al portapapeles. `watch_downloads(minutes)`: vigila la carpeta Descargas y avisa por
      evento de los paquetes nuevos (`.exe`, `.zip`) con su versión y modelos si se pueden leer.
- [ ] `scanner_test(device)` (solo lectura del escáner): escaneo de prueba por WIA a un archivo
      temporal; devuelve la imagen para enseñarla.
- [ ] Pruebas de la lógica pura: lectura de `.inf` (DriverVer, modelos), comparación de versiones
      con 4 números, marca y modelo desde nombre/driver/ID de hardware, estado del driver.

**Encargo B — interfaz**
- [ ] Impresoras: marca (insignia RICOH / EPSON), modelo, conexión, serie y páginas; chip del estado
      del driver con el motivo en `HoverCard`; aparatos sin instalar con «Instalarla».
- [ ] Ficha del driver (Drawer): ahora / nuevo / de dónde sale; «Actualizar driver» con los pasos
      en vivo; «¿Salió bien la página de prueba?» → Sí / Deshacer.
- [ ] «Actualizar las RICOH» / «las EPSON»: en fila, resultado por impresora, resumen para copiar.
- [ ] Pestañas nuevas en «Impresoras y carpetas»: **Escáneres** (con «Probar escaneo» y
      «Instalar Epson Scan 2 / PaperStream») y **Drivers** (biblioteca: paquetes, carpeta, «Añadir»,
      «Bajar de RICOH / EPSON» con la recogida desde Descargas). Alias en `PAGE_ALIAS`.
- [ ] Guía (`guide.ts`), glosario si hace falta, novedades (`changelog.ts`) y la prueba de pantallas.

**Por comprobar a mano (no hay RICOH ni EPSON en este equipo)**: con David y sus impresoras reales.

### Fase 3 — Rapidez (base de la fluidez)
- [ ] R1 partir `Merged` · R2 temporizadores en pausa con la ventana oculta · R3 última lectura con su
      hora en las pantallas de consulta · R4 precargar al pasar el ratón · R5 abrir antes ·
      F1 listas virtuales · F2 las últimas pantallas siguen vivas.

### Fase 4 — Diagnóstico de experto
- [ ] **C1** ¿Por qué no responde?: Wait Chain Traversal (`GetThreadWaitChain`) + archivos y rutas de
      red abiertas por el proceso; explicación en palabras y acciones (reconectar unidad, esperar, cerrar).
- [ ] **C2** ¿Quién tiene ese archivo?: Restart Manager (`RmStartSession`/`RmGetList`) para archivo,
      carpeta o unidad; por la red, `Get-SmbOpenFile` (ya en `office.rs`). Soltar archivos ya existe.
- [ ] **D1** Encontrar el cambio que lo rompió: búsqueda por mitades sobre el diario y el deshacer.
- [ ] **D2** ¿Perfil o equipo?: usuario local temporal con contraseña dictable, borrado con su carpeta
      al terminar o a las 24 h (tarea programada).
- [ ] **B1** Lector de errores: `ocr.rs` + `errcodes.rs` + soluciones; soltar o pegar imagen.

### Fase 5 — El día a día
- [ ] **A2** Puestos por gravedad: `check_stations` manda cada equipo al terminarlo (eventos).
- [ ] **B2** Ctrl+K que ejecuta acciones con confirmación en la misma caja.
- [ ] **B3** Su equipo, desde su ficha (Personas → revisar y arreglar a distancia).
- [ ] **C3** AdminOps en miniatura (segunda ventana pequeña, como la nota de Ctrl+Alt+N).
- [ ] **N1** ¿Qué cambió? · **N3** Pregúntale al equipo · **N5** arreglos desde la bandeja ·
      **N6** salir sin dejar rastro.

### Fase 6 — Lo que enseñas
- [ ] **F1** Tu mes, medido: PDF con cifras que enlazan a su prueba (diario, casos, arranque medido).

### Fase 7 — Antes de la presentación
- [ ] **G1** Repaso de las 52 pantallas en los dos temas a 1366×768, con capturas antes/después.

## Decisiones del autor
- 2026-11-16: N2 y N4 no. Todo lo de varios técnicos, no. Del catálogo «Un día», no: A1, A3, A4, B4,
  C4, D3, E1–E4, F2, F3.
