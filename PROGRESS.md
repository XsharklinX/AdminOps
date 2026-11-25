# AdminOps — tanda 1.4 (elegida el 2026-11-20)

Leer esto primero en cada sesión nueva. El plan de la 1.3 está en `docs/PROGRESS-1.3.md`.

Sale del catálogo «Ideas para AdminOps 1.4» (https://claude.ai/artifact/WrP5AEk8vTcUuCJmmvGnfj,
30 ideas). David eligió **21**; las maquetas del artifact son lo aprobado: hay que hacerlas así y a fondo,
no la versión mínima.

Reglas de siempre: sin build ni commit sin que lo pida; nada que necesite permisos de IT (administrador
local sí); un solo técnico; sin datos personales en la interfaz; textos en español. Cada fase termina
con lint, typecheck, pruebas de Rust y de interfaz, y `npm run test:exe` + `npm run test:perf` con un
.exe de depuración. Antes de cambiar algo visual que no esté ya en la maqueta, enseñar el antes y el después.

## 1.4.1 — BUILD EN CURSO (2026-11-25) · leer esto primero

David decidió el número: **1.4.1** (no 1.5.0). Lleva la tanda 1.5 entera, lo de los juegos y la
**actualización automática** que pidió hoy: «cuando haya actualización, que muestre una ventana para
actualizar y que se actualice ahí automáticamente, que no te lleve a GitHub».

### Hecho
- Actualizar sin salir — comprobado con cargo test 868, vitest 487, tsc, eslint y clippy (app e
  instalador); el script del portable probado de verdad con exes de prueba y ruta con comilla.
  Sin comprobar: una actualización real de principio a fin (necesita la 1.4.1 publicada y otra después).
  - `src/components/UpdateDialog.tsx` (nuevo, + test): novedades, «Actualizar ahora», barra de progreso,
    «Cancelar», «Seguir en segundo plano». Siempre montada (no pierde la descarga al cerrar o bloquear).
    App.tsx la abre sola si `installable`; el aviso de la barra lateral la reabre.
  - `appcare.rs install_update`: instalador con `run_with_args(out, "--actualizar")` (hereda la
    elevación, sin UAC); portable: `stage_portable` + `swap_portable_on_exit` (PowerShell oculto espera
    al PID, copia exe y lhm, relanza; si no puede, enseña el .zip → "portable-manual"). Candado
    `UpdateBusy`, `read_timeout` 30 s.
  - `installer`: `setup_info.auto` (`--actualizar`, hay instalación, no baja de versión) → la UI instala
    sola y reabre AdminOps; `stop_app` desde AdminOps espera 20 s a que se cierre y no usa `taskkill /T`.
  - `ps.rs`/`task.rs`: las tareas sin procesos (descargas) ya se pueden cancelar (LIVE).
- Revisión de Opus hecha (5 hallazgos + 2 posibles), todos arreglados y repasadas las pruebas.
- Versión 1.4.1 en package.json, tauri.conf.json, Cargo.toml y badges del README; entrada en
  `src/lib/changelog.ts`; notas `docs/notas-1.4.1.md` (433 ES / 421 EN caracteres).
- Ojo: de la 1.4.0 a la 1.4.1 aún se actualiza con el instalador de siempre (lo hace el código de la
  1.4.0); lo automático y «volver a la anterior» valen desde la 1.4.1 → siguiente. Ya lo dicen los textos.

### Pendiente (en orden)
1. Ver si terminó la build lanzada a las 7:55 (`npm run build:release`, log en el scratchpad de la sesión
   10d1a6dd: `build141.log`). Si no terminó o falló: relanzarla desde `F:\Programacion\AdminOps`
   (con `$env:SHARKCOST_PEDIDA=1`, David la pidió). Comprobar `release/v1.4.1` con sus 4 archivos.
2. Publicar: `npm run publish:release` (commit «Versión 1.4.1», push, release v1.4.1). Decisión de David:
   tras cada build pedida se publica sin volver a preguntar.
3. David la prueba en el PC; las «sin comprobar» de la tanda 1.5 siguen (abrir a salvo con dos cierres
   reales, volver a la anterior con dos versiones).

### Para retomar
Las pruebas de vitest hay que lanzarlas desde `F:\` en mayúscula (PowerShell `Set-Location 'F:\Programacion\AdminOps'`):
con `f:` en minúscula fallan todas por cargar dos copias de vitest. No es fallo del código.

## Tanda 1.5 (elegida el 2026-11-24, SIN EMPEZAR)
Catálogo «Ideas para AdminOps 1.5»: https://claude.ai/artifact/LeSCHP4JFfEi5fdJcW4kVp (fuente
`ideas-adminops-15.html` y `build15.py` en el scratchpad de la sesión 6e25bd71). Las ventanas
antes/ahora del artifact son lo aprobado: hacerlas así y a fondo. Base: la 1.4.0 publicada (2f34258).

**SÍ (7):**
- [ ] **1 «Ver el comando» en todas partes**: lo que faltaba de B1. Limpiadores de Espacio (cada fila
      con «Ver el comando ▾» y «Copiar el comando»), el interruptor de cada programa de Inicio y los
      arreglos de la bandeja (`tray.rs`).
- [ ] **2 Las vistas guardadas recuerdan las columnas**: unir A4 (columnas a tu gusto, DataTable) con
      SavedViews. Cada vista guarda columnas visibles, orden, ordenación y filtro. Las vistas ya
      guardadas, sin columnas, siguen abriendo con las de siempre.
- [ ] **4 Ctrl+K abre con «Lo de siempre»**: sin escribir, las 5 acciones más usadas en esa franja
      del día (mañana/tarde), contadas en este equipo, con «N veces este mes» y Alt+1…5. Al escribir,
      Ctrl+K como ahora. Solo acciones que funcionan de verdad (igual que el resto de Ctrl+K).
- [ ] **5 Teams y Correo con su icono también fuera de la barra**: `BrandIcons.tsx` (TeamsIcon,
      OutlookIcon) en CommOpener («Enviar a…»), ficha de Personas y Ajustes → Portales y correo.
- [x] **10 Si se cierra mal dos veces, abre a salvo** — HECHA 2026-11-24 (sin commit, sin verla en la app).
      `safestart.rs` + `SafeStart.tsx`; `session.json` con abierta/pid/arranque/pantalla/actividades; un cierre
      malo no cuenta si Windows se reinició después ni si aquella AdminOps sigue viva. A salvo se saltan:
      winwatch, officemap, agenda, disks watch, backupio, alertrules, perfhistory, whatchanged, nightjobs,
      appbackup, connected, auto_cleanup y security::warm_up. Actividades: grabar, vigilar, tarea nocturna.
      Errorlog con origen «cierre». Antes:: marcar «abierta» al arrancar y «cerrada bien»
      al salir; dos cierres malos seguidos → abrir sin vigilar (faultrec), sin monitor en vivo ni tareas
      nocturnas, con la banda «se cerró de golpe dos veces… estaba haciendo X» y «Seguir a salvo /
      Abrir normal / Copiar el informe del fallo» (registro de errores, `errorlog.rs`).
- [x] **11 Volver a la versión anterior** — HECHA 2026-11-24 (sin commit, sin probar con dos versiones).
      `install_update` guarda antes el Setup de la versión instalada (descargado de su release y comprobado
      con SHA256) en `version-anterior/`; `DATA_FORMAT` en appcare.rs (súbelo si una versión cambia datos
      que la anterior no lee). El instalador acepta `--volver` (deja bajar de versión); tauri.conf
      `allowDowngrades: true`. Solo funciona desde 1.5 → siguiente (la 1.4.0 no guarda nada). Antes:: el actualizador (`appcare.rs`) guarda el Setup de la versión
      instalada antes de actualizar (comprobado con SHA256). Ajustes → Actualizaciones: «Volver a la
      X» y aviso si la nueva cambió datos que la anterior no sabe leer.
- [x] **12 La prueba del .exe, completa en cada build** — HECHA el 2026-11-24, sin commit.
      En vez de tocar el manifiesto: el .exe se abre con `__COMPAT_LAYER=RunAsInvoker` (solo ese
      proceso, el .exe no cambia; `--admin` para probarlo elevado con UAC). Recorre las pantallas y
      cada sección de `src/lib/sections.ts` en los dos temas (98 por tema), comprueba que la pestaña
      visible cambia, graba 5 s (`faultrec_start/stop`, y borra el vídeo) y al cerrar mira que no
      quede ningún AdminOps.exe ni WebView2 de la carpeta de prueba. `build-release.ps1` la pasa
      siempre: ADMINOPS_SKIP_EXE_TEST ya no salta nada (solo avisa).
      **Comprobado con**: `node tests/exe/exe.mjs` sobre el portable 1.4.0 desde una terminal sin
      administrador: arranque 2,0 s, 98+98, grabación de 5,0 s, «✓ Todo bien», ningún proceso vivo.
      Dos falsos fallos de la propia prueba arreglados (pestañas de pantallas/secciones ocultas).

**NO (no volver a proponerlas):** 3 caso en espera, 6 directivas explicadas, 7 certificados que caducan,
8 recorrido Wi-Fi, 9 programa que pide administrador, 13 capturas comparadas, 14 el plan tras unirse al
dominio (el fallo conocido de B3 queda como está: el plan se recoge al abrir AdminOps).

Orden propuesto: 12 primero (así todo lo demás se prueba con el .exe), luego 10 y 11, y al final 1, 2,
4 y 5. Versión: decidir con David al pedir la build.

**Para retomar:** las 7 de la tanda 1.5 HECHAS (2026-11-24), sin commit y sin verlas en la app. Dos revisiones
de Opus: todos sus hallazgos arreglados (vistas guardadas ya en Programas, Dispositivos, Inicio y Conectados;
«Lo de siempre» respeta el modo; Alt+1..5 con e.code y sin chocar; huella de la versión anterior en HKLM
y archivo bloqueado al ejecutar; «Abrir normal» con request_restart; lanzadores Epic/Rockstar no son juego).
Comprobado: cargo test 867, clippy limpio, vitest 485, tsc, instalador 4/4, test:exe --quick sobre el 1.4.0.
Falta: pasada de Opus sobre los últimos arreglos antes de la build, novedades (changelog.ts) y guía al
decidir la versión, y la build cuando David la pida (test:exe completo ya corre en ella).

## Estado
- 2026-11-20: **Fases 0, 1, 2 y 3 hechas, en la carpeta principal SIN commit** (sobre badb39d).
  Hecho por oficiales en worktrees y pasado como parche; revisado por Opus (11 fallos + 3 posibles,
  todos arreglados: el grave, B3 con RunOnce no abría la versión con administrador → tarea programada
  «AdminOps seguir plan» AtLogOn + RunLevel Highest).
  **Comprobado con**: tsc, eslint, vitest 335/335, cargo test 628 (54 ignoradas), clippy sin avisos,
  `npm run build`, `test:perf` (referencia actualizada a 2198 KB: +8 %, lo nuevo de Ctrl+K, DataTable
  y «Ver el comando»).
  **Sin comprobar**: nada visto en la app abierta; `test:exe` no se lanzó (mueve el ratón de verdad);
  la tarea programada tras un reinicio real; letras con Alt, arrastrar columnas, botones del ratón,
  zoom por monitor.
- Detalles que decidieron los oficiales (revisar con David si no le gustan):
  - Letras con Alt (A2): «Liberar espacio» no existe como botón → L en «Liberar lo verde»; «Renovar la
    IP» → R en «Reparación rápida».
  - Filtro al escribir (A3): solo en tablas de 8 filas o más.
  - Zoom (A8): elegir tamaño en Ajustes lo pone como general; Ctrl+/− lo guarda para el monitor actual.
  - B3: cerrar el plan a medias pregunta «¿Parar el plan?».
  - B4: las acciones de riesgo medio o alto salen desmarcadas y piden confirmación al ejecutar.
- Pendiente de esta parte: «Ver el comando» no está en los limpiadores de Espacio, en el interruptor
  de cada fila de Inicio ni en la bandeja. Ninguna acción usa todavía la medida «memoria». A4 sin
  integrar con las vistas guardadas. Tras unirse al dominio con otra cuenta, la tarea de seguir el
  plan no salta (es del usuario que la creó): el plan se recoge al abrir AdminOps.
- 2026-11-21: **Fase 4 (C2, C4, C5) hecha por oficiales en la carpeta principal, sin commit**
  (punto de restauración ccb2311 justo antes). C5 va en una pestaña nueva Hardware → «Conectados»
  (la pantalla Dispositivos es la de equipos de la red); «sin driver» solo con el código 28 de Windows.
  Revisión de Opus: NO PASA (8 fallos + 6 posibles; el grave: `[uint32]` de un HResult negativo
  cortaba la instalación de Windows Update; el medio: «Cerrar y seguir» mataba powershell.exe).
  Los 14 arreglados. **Comprobado con**: tsc, eslint, vitest 355/355, cargo test 676 (58 ignoradas),
  clippy, `npm run build`; la función Emit ejecutada en PowerShell con -2145124329 → 0x80240017.
  **Sin comprobar**: la app abierta, DISM real (pide administrador), EFI real, driver y WU reales.
- 2026-11-21: Fase 5 (D3, D5, D8) lanzada a un oficial en la carpeta principal. Punto de restauración
  `b3b3baea` (git stash create) + copia de los archivos sin seguimiento en el scratchpad (pre-f5).
  Hecha: Hardware/«Este equipo» → «Pantallas» y «Teclado e idioma»; Recuperar archivos → pestaña
  «Sin guardar de Office». **Comprobado con**: tsc, eslint, vitest 397/397, cargo test 707 (61
  ignoradas), clippy, build; lecturas reales (1 monitor DELL por HDMI, 3 idiomas, 0 sin guardar).
  **Sin comprobar**: ningún cambio real de pantalla ni de idioma, la cuenta atrás de 15 s, «Abrir una
  copia», versiones anteriores (VSS), la app abierta.
  Decisiones del oficial (revisar con David): nativa según el driver y si no el EDID; cambio temporal
  y «Mantener» lo fija; «Hz bajos» solo avisa; `unsaved_copy` bloqueado en auditoría; ~DF*.tmp fuera.
  Revisión de Opus: NO PASA (10 fallos + 7 posibles; los altos: con AdminOps elevado con otra cuenta
  D5/D8 miraban al técnico, y cerrar AdminOps en los 15 s dejaba el cambio de pantalla puesto).
  Los 17 arreglados (con otra cuenta, D8 solo lee el hive del cliente y no cambia nada; .asd/.xar
  conservan extensión y se abren con su app de Office). **Comprobado con**: tsc, eslint, vitest
  402/402, cargo test 718 (61 ignoradas), clippy, build, lecturas reales.
  **Sin comprobar**: otra cuenta de administrador, cambios reales de pantalla/idioma, SPI_SETLANGTOGGLE,
  cierre con cambio pendiente, «Abrir una copia» con Office real, la app abierta.
- 2026-11-22: Fase 6 (D6, D7) lanzada a un oficial en la carpeta principal. Punto de restauración
  `37188c37` (git stash create) + copia de los sin seguimiento en el scratchpad de la sesión
  340a88aa (`pre-f6`). Encargo: `scratchpad/encargo-F6.md` de esa sesión. El enlace del artifact de
  ideas ya no se puede leer; la copia local del HTML está en el scratchpad de la sesión bb5d87a2
  (`ideas-adminops-14.html`, D6/D7 en las líneas 569-586).
  Decisiones mías en el encargo (revisar con David): carpeta `C:\Escaneos`; usuario `escaner` oculto y
  contraseña dictable que no se guarda («Contraseña nueva» la cambia); con red Pública no se abre el
  cortafuegos sin un paso aparte; SMB1 solo se explica, nunca se activa; quitar la firma SMB
  obligatoria como acción de riesgo alto; D7 con ETW de Kernel-Network (TCP y UDP) y plan B
  `GetPerTcpConnectionEStats`; «Hoy» = lo medido mientras AdminOps miraba; «Pausar 2 h» cierra
  OneDrive y lo reabre con una tarea programada de un solo uso.
- 2026-11-22: **Fase 6 hecha, sin commit.** D6 = tarjeta «Escanear a carpeta» en Escáneres
  (`scanfolder.rs`, `ScanToFolder.tsx`); D7 = pestaña Red → «Uso de la red» (`netusage.rs`,
  `onedrive.rs`, `NetUsage.tsx`, `ProgramIcon.tsx`). Cortafuegos rehecho por mí: regla propia
  `AdminOps-Escaneos-SMB` (TCP 445, solo desde la IP de la impresora, perfil actual, nunca Público);
  las de Windows no se tocan porque la de SMB cubre Privada+Pública a la vez.
  Revisión de Opus: NO PASA (2 altos: el deshacer borraba un recurso «Escaneos» previo; «Reanudar
  OneDrive» no abría estando elevado; + 2 medios, 6 bajos, 8 posibles). Todos arreglados por el oficial;
  los dos altos mirados por mí.
  **Comprobado con**: tsc, eslint, vitest 430/430, cargo test 759 (65 ignoradas), clippy sin avisos.
  **Sin comprobar**: `npm run build` tras los arreglos (lo corté; antes de los arreglos pasaba); sin
  segunda revisión tras los arreglos; ETW real, plan B, registros SMB (piden administrador); nada real
  de crear usuario/recurso/cortafuegos ni pausar OneDrive; la app abierta; `test:ui`/`test:exe`/`test:perf`.
  Decisiones del oficial (revisar con David): siempre «X MB subidos en N min» (no «subiendo 38 GB»,
  no se puede saber); «Subida libre» = línea − lo que suben todos (demo 19,4 en vez de 1,6); la espera
  del escaneo arranca sola al preparar; contraseña de hasta 18 caracteres.
- OJO vitest: lanzarlo desde `F:\` (PowerShell `Set-Location F:\Programacion\AdminOps`). Con la ruta
  en minúscula `f:` fallan los 81 archivos con «Cannot read properties of undefined (reading 'config')».
- OpenCode Go sin cuota mensual hasta ~2026-11-27: revisiones con Opus mientras tanto.
- 2026-11-23: segunda revisión de Opus de los arreglos: NO PASA (4 medios, 6 bajos, 4 posibles). Todos
  arreglados por un oficial y el diff mirado por mí: deshacer tolerante a fallos (acaba todo y avisa al
  final); el recurso lleva la marca de AdminOps y no cuenta como previo; usuario `escaner` previo vuelve a
  su estado (activado y UserList); el permiso NTFS del cliente se quita si no lo tenía; contraseña con
  mínimo alto elige palabras por longitud (máximo real 18); contador para `AuditSmb1Access`; OneDrive
  lanzado con `cmd /c start` (la tarea acaba y ningún límite lo mata), tarea de un momento con ajustes de
  batería, procesos solo del usuario (CIM + GetOwner, sin administrador), «Reanudar» comprueba que se
  abrió antes de borrar la tarea; ETW: un solo reintento tras pánico y luego error; plan B apaga todas.
  **Comprobado con**: cargo test 767 (65 ignoradas, lanzado por mí), clippy, tsc, eslint, vitest 430
  (del oficial). **Sin comprobar**: los scripts en Windows real (solo con el parser), `npm run build`
  (bloqueado: build solo cuando David la pida), la app abierta.
  Límite conocido: si se prepara dos veces, la segunda entrada del diario no quita el permiso NTFS del
  cliente (ya lo tenía, se lo dio la primera); lo quita deshacer la primera.
- 2026-11-23: David pide Fase 7 y Fase 8 seguidas. Fase 7 lanzada a un oficial en la carpeta principal.
  Punto de restauración `67fd89d` (git stash create) + copia de los sin seguimiento en el scratchpad de
  la sesión df0d25e6 (`pre-f7`). Encargo: `scratchpad/encargo-F7.md` de esa sesión.
  Decisiones mías en el encargo (revisar con David): pestaña nueva Equipo → «Grabar el fallo»; MP4 H.264
  con Media Foundation, 10 fps, hasta 1920 de ancho, sin audio; se deja el borde amarillo de Windows;
  píldora «● Grabando» siempre encima y excluida de la captura; marcas CPU ≥ 90 % y disco ≥ 95 % durante
  3 s, «no responde» con el nombre del programa (nunca el título de la ventana); «Causa probable» = el
  pico que empezó en los 10 s antes de un cuelgue o error; vídeos en machine_data_dir, se borran al
  cerrar o descartar el caso (sin caso, al cerrar AdminOps); comandos SAFE en auditoría.
- 2026-11-23: **Fase 7 hecha por el oficial, sin commit y SIN REVISAR todavía.** Nuevos:
  `src-tauri/src/faultrec.rs` + `faultrec/{capture,watch,analysis}.rs`, `src/lib/api/faultrec.ts`,
  `src/lib/faultrec.ts(+test)`, `src/components/FaultRecorder.tsx`, `src/components/RecPill.tsx`.
  Cambiados: lib.rs, audit.rs, cases.rs (borra al cerrar/descartar), perfhistory.rs (DiskBusy pub(crate) +
  Drop), waitchain.rs (`hung_pids`), netusage.rs (`Idents`), eventdigest.rs (`known_title`), Cargo.toml
  (protocol-asset + features de windows), tauri.conf.json (assetProtocol, CSP media-src),
  capabilities/default.json (ventana `recpill`), main.tsx, Merged.tsx, sections.ts, demo.ts.
  **Comprobado con**: captura real (`cargo test faultrec -- --ignored`: 3 s → MP4 1920×1080 de ~0,7 MB,
  duración 3200 ms, orientación bien), cargo test 790 (67 ignoradas), clippy, tsc, eslint, vitest 445,
  scan:secrets (todo del oficial). **Sin comprobar**: la app abierta (píldora, minimizar, vídeo por el
  protocolo asset con la CSP, borrado al cerrar un caso real), monitor secundario, cambio de resolución,
  cerrar AdminOps grabando, `test:ui`.
  Decisiones del oficial (revisar con David): botón rojo = `Button kind="danger"`; eventos de ruido
  (DCOM 10016…) no son marcas; las marcas de error no guardan el texto del evento («Error 1002» + frase
  conocida); grabar sin caso → se borra al cerrar AdminOps aunque luego se abra un caso; cerrar el caso
  grabando → no se guarda y avisa. Yo moví la marca de disco de la demo de 0:13 a 0:16 para que cumpla la
  regla de 10 s de «Causa probable».
- 2026-11-23: Revisión de Opus de la Fase 7: NO PASA (1 medio + 7 leves + 4 posibles). Arreglados:
  cerrar AdminOps grabando dejaba la píldora abierta y el proceso vivo (ahora se cierra siempre en
  `finish` y en `shutdown`); los avisos (Level 3) salían como «Error N» (ahora solo Level 1, 2); al
  guardar cerrando AdminOps, nota «faltan las marcas de errores»; «Parar» antes del primer fotograma
  (`STOP_EARLY`, mirado con `ACTIVE` bloqueado; la interfaz lo enseña como aviso, no como error);
  `FINISHING` se pone antes de soltar `ACTIVE`; `<Result key>`; la consulta de respaldo sigue mientras
  `finishing` y enseña `error` (sin repetir el aviso); monitor desconectado → para con nota
  (`Done.lost`, mira `monitor_handle` tras 3 s sin fotogramas); si el codificador de la tarjeta falla en
  el primer fotograma, se cambia al de programa; ventanas colgadas escondidas tras la «fantasma»
  (`hung_window`: visible, o sin dueño y con barra de título; también en `is_hung`); `faultrec_delete`
  avisa si el vídeo sigue ahí (lo borra la limpieza al cerrar). Sin tocar: el protocolo asset también
  existe en los portales de Tickets (alcance solo a la carpeta, ruta difícil de adivinar, el vídeo no
  se puede leer desde otro origen).
  **Comprobado con**: clippy -D warnings, cargo test 790 (67 ignoradas), captura real `--ignored` (2/2),
  tsc, eslint, vitest 445, sin tildes rotas. **Sin comprobar**: un programa colgado de verdad, monitor
  desconectado, codificador de tarjeta que falle al escribir, la app abierta.
- 2026-11-23: **David: fuera de la 1.4.0 la pantalla Tickets y la pestaña «Inventario web»** (los
  portales web de tickets/inventario de la empresa; «no funciona como me gustaría»). Hecho con un
  interruptor `WEB_TICKETS = false` en `src/lib/features.ts` (allí, la lista de lo que hay que devolver
  si vuelve). `DISABLED_PAGES` en Sidebar.tsx entra en `allowedInMode` (barra, Ctrl+K, historial,
  fijados, pantalla al abrir); sin el paso «Tus tickets» del Onboarding, sin «Pegar en Tickets» en el
  caso, sin precargar esos portales, y en Ajustes → Portales solo se ven los de Correo y Teams (los ya
  guardados no se borran). Textos reescritos sin nombrarlos. `test:exe` prueba los portales con Correo.
  Correo y Teams siguen igual. La guía de la Fase 8 y el changelog de 1.4.0 NO deben mencionar Tickets.
  **Comprobado con**: tsc, eslint, vitest 445, `vite build`. **Sin comprobar**: la app abierta.
  Nota: vitest falla entero («reading 'config'») si se lanza desde `f:\` en minúscula: usar `F:\`.
- 2026-11-23: **Fase 8 hecha, sin commit.** changelog.ts (entrada 1.4.0, con «removed»: Tickets e
  Inventario web apagados), whatsNew.ts (NEW_IN de las pantallas nuevas), guide.ts (temas connected,
  recorder, displays, keyboard, netusage, see-command; ampliados moving, update, install, recipes,
  scanners, recover; glosario +6), README.md y README.en.md (este venía de 1.1.11), versión 1.4.0 con
  `npm run setversion` + package-lock.json (venía de 1.2.9), notas en `docs/notas-1.4.0.md` (ES 489,
  EN 486 caracteres). **Comprobado con**: tsc, eslint, vitest 445, cargo test 790, clippy, vite build,
  `tests/ui/smoke.mjs` con `CHROMIUM_PATH` = Edge (55 pantallas, 57 secciones, 0 problemas; el
  Chromium de Playwright no está instalado). **Sin comprobar**: `test:exe` y `test:perf` (necesitan
  un .exe), la app abierta. Revisión de Opus previa a la build lanzada.
- 2026-11-23: Revisión de Opus previa a la build: NO PASA (1 medio + 5 leves + 2 posibles), todo
  arreglado (ejemplo de Ctrl+K que no existía → «liberar espacio y vaciar la cola»; Tickets fuera de
  ResumeRibbon, ShortcutEditor y NavEditor; company.rs solo importa portales mail/teams; showError en
  FaultRecorder; frase de Pantallas; «no responde» con ventana escondida solo si hay una «Ghost» visible;
  shutdown espera 45 s si FINISHING).
- 2026-11-23: **David pide análisis de Espacio rápido «como WizTree» y luego la build.** Medido: C: (1,9 M
  archivos, SSD) tarda 257 s carpeta a carpeta sin administrador (el registro de la app tenía 7–11 min).
  Hecho `src-tauri/src/space/mft.rs`: con administrador y NTFS lee la MFT de `\\.\X:` (solo lectura,
  hilo lector + sync_channel, fixup, runlists, registros de extensión) y monta el mismo Node/Tops que
  `walk`. Igual que walk: fuera todo lo que tiene reparse (uniones, OneDrive), System Volume Information y
  los archivos de NTFS. Enlaces duros una vez. Disco entero siempre; subcarpeta solo sin seek penalty (SSD).
  Cae a `walk` si: sin admin, no NTFS, sin letra, serie de volumen distinta (enlace a otro disco), MFT muy
  fragmentada, destino que no es carpeta en uso. `SpaceView.fast`/`elevated` → línea del resultado en
  Space.tsx. Guía, changelog, NEW_IN space:space, README y notas actualizados (ES 499, EN 500).
  2.ª revisión de Opus: NO PASA hasta probarlo de verdad; sus 5 fallos y 5 posibles arreglados.
  **Comprobado con**: 7 pruebas de mft.rs con registros fabricados, cargo test, clippy, tsc, eslint,
  vitest 445. **Sin comprobar**: la lectura real como administrador →
  `$env:SPACE_PATH='C:\'; cargo test --release --lib space_mft_real -- --ignored --nocapture` en una
  consola de administrador (compara MFT con carpeta a carpeta). Build 1.4.0 lanzada con
  ADMINOPS_SKIP_EXE_TEST=1.
- 2026-11-23: 3.ª revisión de Opus: PASA (2 leves + 1 posible arreglados: NavEditor no enseña secciones
  con solo páginas apagadas, README.en, «C:» → «C:\»). **BUILD 1.4.0 HECHA** en `release/v1.4.0`
  (Setup 13,4 MB, clásico 10,0 MB, portable 14,0 MB, SHA256SUMS), sin rutas del equipo, SIN `test:exe`
  ni `test:perf` (ADMINOPS_SKIP_EXE_TEST=1). Sin commit. Falta: que David la pruebe (primero Espacio como
  administrador en C:, «leyendo el índice del disco»), commit y publicar.
- 2026-11-23: el instalador propio (installer/) tenía el icono y el logo viejos: copiados de
  src-tauri/icons/icon.ico y src/assets/logo.svg, recompilado y reempaquetado (David no lo había pedido;
  no volver a compilar sin que lo diga). Nuevo `npm run publish:release` (scripts/publish-release.ps1): commit
  + push + versión en GitHub. **Decisión de David:** tras cada build que pida, publicar sin preguntar. La
  1.4.0 se publica cuando él la pruebe y lo diga.
- 2026-11-23: David: «se ven demasiadas cosas, es lioso» + rehacer la grabación. Hecho (sin compilar, sin
  revisar): botón de grabar en la barra de arriba junto a Teams y Correo (`src/components/RecButton.tsx`:
  cámara gris parado; grabando, pastilla verde con punto que late y el tiempo; clic para; clic derecho abre
  Grabar el fallo; al terminar navega a machine/recorder), guía y changelog al día. Artifact de ideas
  «AdminOps más claro» https://claude.ai/artifact/A4eEutFgCGn7krrrqHsdPj (16: 7 para despejar, 9 de
  grabación, la 8 es el botón ya hecho). **Espera su elección.** Pendiente pedido por David: Teams y Correo
  con su icono oficial en la barra de arriba (en las maquetas ya sale así).
## Tanda «más claro + grabación» (elegida el 2026-11-23, tras la build 1.4.0 sin publicar)
Artifact https://claude.ai/artifact/A4eEutFgCGn7krrrqHsdPj. David: SÍ a 1, 2, 4, 7, 8–16. **NO (2026-11-24:
lo que deja sin decidir es un no; no volver a proponerlas): 3 resumen único arriba, 5 Inicio «¿Qué le pasa?»,
6 lo avanzado aparte.** Las maquetas del artifact son lo aprobado. Número de versión: decidir con David al pedir la build
(la 1.4.0 compilada aún no está publicada). Añadidos suyos:
- Grabación automática (10) **desactivada de serie** y nada molesta: «No volver a grabar por esto» (por
  programa/código de error), límite por programa y tiempo, un solo aviso que agrupa, nunca 10 notificaciones.
- Icono de la cámara con color, acoplado a Teams y Correo. Teams y Correo con su icono oficial.
- La píldora «Grabando» se puede mover.

Fases:
- **G0 — HECHA (sin compilar):** `BrandIcons.tsx` (TeamsIcon, OutlookIcon, RecordIcon verde con la misma
  composición) en TopBar y RecButton; píldora arrastrable (`data-tauri-drag-region` + capacidad
  `capabilities/recpill.json` con `core:window:allow-start-dragging`), punto verde que late y asa. tsc,
  eslint, vitest. **Sin comprobar:** arrastrar de verdad (el clic en el texto/punto/fondo debe arrastrar,
  «Parar» no). Falta: iconos oficiales también donde salgan Teams/Correo fuera de la barra (CommOpener,
  Ajustes → Portales), si David lo quiere.
- **G1 — HECHA 2026-11-23 (sin compilar, sin revisión de Opus):** cabecera en una línea (App.tsx: atrás + título +
  aviso «N avisos» de QuickRead.tsx + hueco `lib/tabsSlot.ts` donde PageTabs se pinta por portal; sin hueco
  —vista dividida, derecha— se pinta dentro como antes); icono ↻ solo fuera de NO_RELOAD; barra lateral solo con
  pantallas (resume avisos y «Nuevo» de sus secciones); chincheta de fijar en cada pestaña y píldora (TabPin,
  en la esquina); Estado del equipo en 4 grupos (MACHINE_GROUPS + GroupTabs en Merged.tsx, recuerda la píldora
  por grupo); `Intro` en ui.tsx (1.ª vez entero, luego «Cómo funciona ▾»; localStorage adminops.introSeen)
  aplicado por un oficial a 14 pantallas; Ajustes → Inicio y ventana → «Ver otra vez las explicaciones»; guía,
  NavEditor y changelog al día. **Comprobado con:** tsc, eslint, vitest 445/445, test:ui con Edge
  (CHROMIUM_PATH=msedge.exe; 55 pantallas + 57 secciones, 0 problemas) y capturas de machine-*.
  **Sin comprobar:** app compilada, barra en modo «solo áreas», arrastre con la vista dividida.
  Plan original: Estado del equipo en 4 grupos (Salud: Diagnóstico, Seguridad,
  Rendimiento, Arranques · Piezas: Hardware, Conectados, Pantallas, Teclado, Periféricos · Qué pasó:
  Historial, ¿Qué cambió?, Pregúntale · Grabar) con fila de botones dentro; barra lateral solo con
  pantallas (las secciones solo arriba; ojo a fijados de secciones, Ctrl+K, KeyTips, NEW_IN, sections.ts y
  sus pruebas); cabecera en una línea (título + aviso pequeño + pestañas; «Recargar» solo donde relee);
  explicaciones plegadas en «Cómo funciona ▾» a partir de la 2.ª visita (por pantalla, localStorage).
- **G2 — HECHA 2026-11-23 (sin compilar, sin revisión de Opus):** Rust por un oficial de Sonnet (encargo en el
  scratchpad de la sesión 8e6f9dbc, `encargo-g2.md`): `faultrec_start(monitor, opts{window, region, input, mic})`,
  `faultrec_windows`, `faultrec_pick_region` + `faultrec_region_done` (ventana transparente `recarea`, evento
  `faultrec://region`), módulos nuevos `faultrec/input.rs` (sondeo con GetAsyncKeyState cada 8 ms, SIN ganchos;
  letras siempre «•••»), `mic.rs` (WASAPI 48 kHz mono → pista AAC; relleno de silencio si se para), `win.rs`; Meta
  gana target/inputOn/input/mic/voice y Status gana mic. Interfaz mía: Setup en FaultRecorder.tsx (Pantalla entera /
  Una ventana / Una zona, interruptores de clics y teclas —sí de serie— y voz —no de serie—, guardados en
  localStorage `adminops.faultrec.options`), AreaPicker.tsx (capa para dibujar), círculos de clic y teclas encima del
  vídeo + pistas de clics y voz bajo la línea de tiempo, micrófono en la píldora, RecButton usa las opciones
  guardadas (siempre pantalla entera), demo con ventanas, clics y voz. **Arreglado de paso:** cierre con 0xC0000005
  al parar (el vigilante usaba el COM del hilo de captura; ahora `watch.rs` inicia el suyo): antes 3 de 6 pasadas
  fallaban, después 8 de 8 bien. **Comprobado con:** clippy, cargo test 817 (71 ignoradas), pruebas reales de
  ventana, zona, orientación y voz (pista de audio presente; el micro de aquí dio silencio), tsc, eslint, vitest
  450/450, test:ui con Edge 0 problemas, captura de la pantalla de preparar. **Sin comprobar:** la capa de la zona
  abierta de verdad, un clic o una tecla apuntados de verdad, voz con sonido, todo desde la app compilada.
  Plan original: pantalla / una ventana (WGC CreateForWindow, sigue a la ventana) /
  una zona (recorte del monitor); clics (ondas) y teclas («Ctrl + P»; en campos de contraseña «•••»: hook
  de bajo nivel o GetAsyncKeyState, nunca guardar texto tecleado) pintados en el vídeo o como pista; voz
  por micrófono opcional (apagado de serie, pista de audio en el MP4).
- **G3 — HECHA 2026-11-23 (sin compilar, sin revisión de Opus):** Rust por un oficial de Sonnet (cortado por el
  límite de 5 h y reanudado): `faultrec/vigil.rs` (sesión: captura 5 fps ≤1280 → H.264 por MFT síncrono →
  anillo de 75 s en memoria, vigilante y clics en anillo, Ctrl+Alt+G solo mientras se vigila, avisos agrupados,
  pausa durante la grabación a mano y vuelta), `mfenc.rs` (MFT + MP4 sin recodificar), `ring.rs`, `auto.rs`
  (disparadores, límites 2/clave/hora y 10/día, EvtQuery cada 10 s, ajustes en `grabacion-auto.json`). Comandos
  faultrec_watch_start/stop/status, faultrec_just_happened, faultrec_auto_get/set/ignore/unignore; eventos
  faultrec://auto y faultrec://open (clic en el aviso). Interfaz mía: `FaultWatch.tsx` (barra del último minuto,
  «¡Acaba de pasar!», automática apagada de serie con sus 3 disparadores e ignorados), origen en la lista y en la
  grabación, «No volver a grabar por esto», punto verde y clic = guardar el último minuto en el botón de arriba,
  demo. **Arreglado por mí:** el cierre con 0xC0000005 al parar seguía (3 de 20): `capture::keep_mta_alive`
  (`CoIncrementMTAUsage` una vez) → 0 de 20, y las 8 pruebas reales ya pasan seguidas en un proceso.
  **Consumo medido (optimizado, no release):** 0,8 % del equipo con la pantalla quieta, 1,6 % moviéndose; 76 s en
  5,6 MB. **Comprobado con:** clippy, cargo test 848 (74 ignoradas), las 8 reales seguidas, tsc, eslint, vitest
  453/453, test:ui 0 problemas. **Sin comprobar:** con la app compilada: Ctrl+Alt+G, avisos de Windows y su clic,
  un disparador automático real, la vuelta a vigilar tras grabar a mano, la interfaz.
  Plan original: búfer en memoria del último minuto («¡Acaba de pasar!» + atajo global);
  disparadores automáticos (no responde, error de aplicación, CPU/disco al 100 % > 20 s) guardando 30 s
  antes + 15 s después; desactivado de serie; ignorar por programa/código; límite; aviso agrupado.
- **G4 — HECHA 2026-11-23 por mí (sin compilar, sin revisión de Opus):** Rust: Tick gana mem y net (sysinfo) y
  Meta gana `samples` (pistas por segundo); `faultrec/clip.rs` (fotogramas con Media Foundation → JPEG con el
  BitmapEncoder de WinRT; exportar un trozo sin recodificar, vídeo + voz); comandos faultrec_thumb (cacheada en
  `<id>.thumb.jpg`), faultrec_to_report (`<id>.clip`, 3 fotogramas: marcas del trozo o principio/mitad/final),
  faultrec_report_remove, faultrec_export (pregunta dónde con el diálogo); el informe mete «Grabación del fallo»
  antes de las firmas (report_clips del caso abierto); limpieza de .jpg/.clip. Interfaz: TrackPanel (pistas
  alineadas, clic = ir, arrastrar = recortar, lectura del segundo), ClipBar (Al informe / Al caso → nota del
  caso / Exportar MP4), lista con miniaturas (RecList) como pantalla de inicio con «Grabar» y «Vigilar»,
  «Todas las grabaciones» para volver; demo con pistas; guion de capturas `tests/ui/g4shots.mjs`.
  **Comprobado con:** clippy, cargo test 851 (75 ignoradas), prueba real fotogramas + exportar (3 JPEG, trozo de
  2,1 s), tsc, eslint, vitest 457/457, test:ui 0 problemas, capturas de lista, reproductor y recorte.
  **Sin comprobar:** el PDF con la sección de la grabación, «Al caso» con un caso abierto, el diálogo de exportar,
  todo desde la app compilada.
  Plan original: reproductor con pistas (CPU, disco, red, memoria, errores, voz)
  alineadas; recortar; «Al informe» (3 fotogramas con marcas en el PDF), «Al caso», «Exportar MP4» (solo
  si lo exporta él); pantalla como lista con miniaturas y un botón grande «Grabar» + «Vigilar».
- **G5 — Cierre:** guía, glosario, changelog, NEW_IN, revisión de Opus. Build solo cuando la pida; tras
  la build, commit + `npm run publish:release` sin preguntar.

- Sigue, en orden: (1) HECHO: revisión de Opus de la Fase 7 con el diff desde `67fd89d` + los archivos nuevos
  (rúbrica: COM/MF emparejados y sin fugas, MP4 cerrado bien al parar/cerrar/pánico, alcance del
  protocolo asset solo a `recordings`, ids sin rutas, sin títulos de ventana, borrado por caso, píldora
  excluida de la captura); (2) HECHO: arreglar lo que salga; (3) Fase 8 (guía, glosario —hay dos: guide.ts:768 y
  glossary.ts—, changelog.ts, prueba de pantallas, `scripts/version.ps1` a 1.4.0, README) y revisión de
  Opus antes de la build; (4) `npm run build` / build solo cuando David la pida.

## Fases (orden propuesto; David puede cambiarlo)

### Fase 0 — Icono «Equipo vivo» (A10) — HECHA
- [ ] Afinar el boceto elegido (ventana azul con pulso verde y pie): SVG maestro, legible a 16 px,
      una tinta y negativo, `svg_audit.py` y la hoja a tamaños reales (skill logo-design).
- [ ] Enseñar a David el icono afinado junto al actual antes de cambiar nada.
- [ ] `npx tauri icon` desde el maestro, `src/assets/logo.svg`, el logo de la barra de título, del
      informe y del instalador; copia en `perfil/diseno/iconos/` y ficha `apps/adminops.md`.

### Fase 1 — Moverse y listas (A1, A3, A4, A5, A8) — HECHA
- [ ] **A1** Atrás y adelante: pila de navegación con el estado de cada pantalla (pestaña, filtros,
      scroll); Alt+←/→, botones laterales del ratón (button 3/4) y flecha en la cabecera con el nombre
      de dónde vienes; mantener pulsada → las 10 últimas.
- [ ] **A3** Escribir para filtrar en cualquier `DataTable` (letras y números con el foco fuera de un
      campo); Esc borra; Enter abre si queda uno.
- [ ] **A4** Columnas a tu gusto: clic derecho en la cabecera (ver/ocultar), arrastrar para ordenar,
      doble clic ajusta el ancho; se guarda por pantalla y entra en `SavedViews`.
- [ ] **A5** Listas en vivo quietas bajo el ratón: se congela el orden, no los datos; aviso «Orden en
      pausa»; al salir, se recoloca con `useAnimatedList`. Procesos, Puestos, Dispositivos, Vigilante.
- [ ] **A8** Ctrl + / Ctrl − / Ctrl+0: zoom del webview entre 80 % y 150 %, porcentaje un segundo en
      pantalla, recordado por monitor. Probar a 1366×768 y en 4K.

### Fase 2 — Teclado y Ctrl+K (A2, A9) — HECHA
- [ ] **A2** Letras con Alt: tabla fija de letras por pantalla (no al azar), capa que las pinta con
      Alt pulsado, barra lateral incluida; no pisar los atajos de Windows con Alt.
- [ ] **A9** Ctrl+K con varias órdenes: partir por «y» y comas sobre el reconocedor actual; una
      confirmación para todas; en serie por la cola de tareas; resultado de cada paso; Tab completa,
      ↑ trae las últimas.

### Fase 3 — Acciones que se explican, se prueban y se repiten (B1, B2, B3, B4) — HECHA
- [ ] Base común: cada acción declara su comando equivalente (B1) y qué mide (B2). Revisar primero
      cómo están registradas las acciones (`quickActions`, `fixLadder`, `tweaks.record`) para hacerlo
      en un solo sitio.
- [ ] **B1** «Ver el comando»: PowerShell/CMD exacto y comentado en español, «Copiar» y «Guardar
      como .ps1»; si la acción usa la API de Windows, el equivalente más cercano y avisándolo.
- [ ] **B2** Antes y después medidos en el aviso («C: 3,1 GB → 24,8 GB libres»), en el caso y en el
      informe. El arranque se mide en el siguiente reinicio y se añade solo.
- [ ] **B3** El plan sigue tras reiniciar: entrada en RunOnce del usuario con el id del plan; al
      entrar, abre el plan, comprueba el paso y sigue. Banner «Seguido tras reiniciar · paso 3 de 5».
- [ ] **B4** «Guardar como plantilla» al cerrar un caso: pasos sacados del diario, sin lo que falló,
      desmarcar lo que era de ese equipo, nombre; aparece en Plantillas.

### Fase 4 — Las pantallas de siempre (C2, C4, C5)
- [ ] **C2** Windows Update: en la fila del fallo, el código en una frase, la causa comprobada en este
      equipo (partición reservada, servicio, componentes) y «Probar» con el siguiente escalón
      (`CodeLinks` + `fixLadder`); progreso real por actualización.
- [ ] **C4** Instalar: una fila por programa (descargando %, instalando, listo); salida de winget
      línea a línea y códigos de salida traducidos; «Cerrar y seguir», «Reintentar», «Saltar»; resumen.
- [ ] **C5** Dispositivos: `CM_Register_Notification` en Rust → evento; lo recién conectado arriba y
      resaltado con su estado y acción; aviso desde la bandeja si está minimizado.

### Fase 5 — Diagnóstico nuevo, primera parte (D3, D5, D8)
- [ ] **D3** Pantallas y monitores (página nueva): `QueryDisplayConfig` + EDID (marca, modelo,
      nativa), escala, Hz, conexión; dibujo de la disposición; arreglos con `SetDisplayConfig` y
      deshacer solo a los 15 s; reiniciar el driver gráfico.
- [ ] **D5** Lo que no se guardó: carpetas de autorrecuperación de Office (registro), documentos sin
      guardar, temporales y versiones anteriores (VSS); vista previa; «Abrir una copia» sin tocar el
      original. Pestaña en Recuperar archivos.
- [ ] **D8** El teclado escribe raro: idiomas de teclado, activo, atajo de cambio; prueba «ñ @ ´»;
      «Dejar solo Español» y quitar el atajo, con deshacer.

### Fase 6 — Diagnóstico nuevo, segunda parte (D6, D7)
- [ ] **D6** Escanear a carpeta: asistente en Escáneres (carpeta, recurso compartido, usuario local
      con contraseña dictable, cortafuegos), panel grande con lo que se escribe en la RICOH/EPSON,
      vigilar la carpeta y decir «Llegó…» o por qué no (SMB1, firma SMB). Pide administrador local.
- [ ] **D7** ¿Quién se come internet?: subida y bajada por programa en vivo
      (`GetPerTcpConnectionEStats` o ETW de red del kernel), a dónde se conecta, total del día;
      «Pausar 2 h» para OneDrive.

### Fase 7 — Grabar el fallo (D2)
- [ ] Captura con `Windows.Graphics.Capture` y el codificador de Windows (sin ffmpeg), hasta 2 min,
      solo en este equipo; línea de tiempo con errores del registro, programas colgados y picos de
      CPU/disco; cada marca salta al segundo del vídeo; se borra al cerrar el caso.

### Fase 8 — Cierre de la 1.4.0
- [ ] Guía (`guide.ts`), glosario, novedades (`changelog.ts`), prueba de pantallas, subir a 1.4.0
      (package.json, tauri.conf.json, Cargo.toml, README). Revisión de Opus antes de la build. Build
      solo cuando David la pida.

## Decisiones del autor
- 2026-11-20: Sí a A1, A2, A3, A4, A5, A8, A9, A10, B1, B2, B3, B4, C2, C4, C5, D2, D3, D5, D6, D7, D8.
- A10: el icono **«El equipo vivo»** (ventana con pulso), de los tres bocetos.
- NO (2026-11-24: lo que deja sin elegir es un no; no volver a proponerlas): A6 ventana que recuerda su
  sitio, A7 sacar a otra ventana, B5 scripts con formulario, B6 pendrive al día, C1 lo que creció, C3
  informe con portada, D1 mapa Wi-Fi, D4 certificados y hora, D9 quién no deja dormir.
