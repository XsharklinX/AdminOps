# Contexto para continuar AdminOps en una sesión nueva

Eres el desarrollador que continúa **AdminOps**, una aplicación de escritorio para Windows hecha por
**David Bonilla** (técnico de soporte; la usa en su trabajo y la ha presentado a su supervisor).
Este documento resume todo lo que necesitas para trabajar sin romper nada. Léelo entero antes de
tocar código. Responde siempre en **español**, claro y directo.

---

## 1. Qué es AdminOps

Herramienta de soporte técnico para Windows 10/11: ver cómo está un equipo, encontrar lo que
falla, arreglarlo con cambios **reversibles**, y dejar constancia (diario de cambios, informe PDF).
Sin cuentas, sin telemetría, sin servidor: todo local. Funciona instalada o **portable en un
pendrive** (datos junto al programa, separados por equipo). Interfaz 100 % en español.

- Repositorio: `F:\Programacion\AdminOps` · GitHub `XsharklinX/AdminOps`, rama `main`
  (commits directos a `main`, mensaje con la versión, p. ej. «1.1.10: …»; etiqueta `vX.Y.Z`).
- Versión en el código: **1.2.3** (sin compilar, sin commit). Última build hecha: **1.2.2** en
  `release\v1.2.2\`. Hubo una build intermedia etiquetada 2.0.0 por un salto de numeración: la
  serie sigue en 1.2.x. Última versión guardada en Git: commit «1.1.10» (y etiqueta `v1.1.9`).
- Correo de soporte del autor (va en la app y en los términos): `Contactoyerlindavid@gmail.com`.

## 2. Reglas permanentes del autor (obligatorias)

1. **No lances builds** salvo que lo pida con esas palabras («lanza build»). No verifiques en
   máquina virtual. **No abras la app** para probar (los avisos de UAC le molestan).
2. **No hagas commit ni push** salvo que lo pida («haz commit»).
3. **Nunca muestres datos personales en la interfaz**: rutas con nombre de usuario pasan por
   `withoutUserPaths` (`src/lib/errors.ts`) / `friendlyPath`.
4. **Nada que parezca malware**: ni evasión de antivirus, ni ejecutar cosas en otros equipos de la
   red, ni controlar sesiones ajenas, ni canales de control remoto propios. Si una petición va por
   ahí (ya pasó dos veces con «acceso remoto a AdminOps desde otro PC»), dilo en una frase y ofrece
   la alternativa legítima: las herramientas de Windows (Escritorio remoto, Asistencia remota).
   No entres en detalle técnico de cómo se haría.
5. **Independencia de IT**: no propongas nada que necesite permisos de IT, registrar aplicaciones
   o consentimiento de administrador del inquilino. Por eso se **quitó Microsoft 365 (Graph)** en
   la 1.1.10; no lo reintroduzcas.
6. Ideas ya **rechazadas** (no volver a proponer): alta/baja de empleados, «esto ya pasó», SLA,
   autodiagnóstico, renovar equipos, QR, oficina en PDF, avisos de red, «¿quedó resuelto?», nota
   de salud, repaso antes de visita.
7. Los dos modos (técnico/usuario) son **un modo de interfaz, no una barrera de seguridad**;
   dilo así siempre.
8. Comentarios del código en español. Mantén el estilo del código que rodea.
9. Actualiza `ROADMAP.md`, `docs/PRUEBAS.md` y `src/lib/changelog.ts` con cada cambio.

## 3. Entorno y trampas conocidas

- Windows; hay Git Bash y PowerShell. Usa **rutas absolutas** (el directorio de trabajo cambia).
- **No edites código con heredocs de bash** (corrompen barras invertidas y tildes) **ni con
  `Get-Content`/`Set-Content` de PowerShell** (corrompen el UTF-8). Usa la herramienta de escribir
  archivos, o scripts `.py` (en el scratchpad) con cadenas literales y `io.open(..., newline="")`.
- Los archivos mezclan finales **CRLF y LF**: en los scripts, normaliza a `\n` al leer y restaura
  el original al guardar, o los reemplazos no coinciden.
- Cuando un script sustituye texto, que **falle si el texto no aparece exactamente una vez**.
- Al editar Rust con comillas escapadas, sustituye por líneas, no por bloques grandes.
- No edites fuentes mientras corre una build en segundo plano.
- PDF de documentos: se generan con Edge sin interfaz:
  `"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --headless --disable-gpu --no-pdf-header-footer --print-to-pdf="<salida.pdf>" "file:///<archivo.html>"`.

## 4. Tecnología y estructura

- **Tauri 2**: motor en **Rust** (`src-tauri/src`), interfaz **React 19 + TypeScript + Vite +
  Tailwind CSS 4** (`src/`), dibujada con **WebView2**. Iconos lucide-react, fuente IBM Plex.
- Consultas a Windows con **PowerShell/WMI** a través de `crate::pspool::query(script, timeout,
  detalle)` (proceso persistente) o `crate::ps::powershell(_opts)`. Datos en los scripts SIEMPRE con
  `crate::ps::text_var("nombre", valor)` / `ps_literal` (nunca interpolar texto). Contraseñas con
  `secret_var`.
- Sensores: LibreHardwareMonitor. Informes PDF: HTML → Edge (`diagnostics/pdf.rs`).
- Instalador propio en `installer/` (Tauri mínimo que lleva dentro el NSIS clásico; pide aceptar
  los términos) + NSIS clásico (`/S` silencioso) + zip portable.
- Cifras actuales: ~393 comandos, 274 pruebas de Rust (+41 ignoradas que usan el equipo real),
  81 de la interfaz.

### Cómo se añade un comando (no te saltes ningún paso)
1. Función `#[tauri::command]` en su módulo.
2. Registrarla en `src-tauri/src/lib.rs` (`generate_handler!`).
3. Clasificarla en `src-tauri/src/audit.rs`: en `BLOCKED` si **cambia el equipo** (el modo
   auditoría la rechaza) o en la lista `SAFE` si solo consulta o toca datos de AdminOps. Una prueba
   falla si un comando no está clasificado.
4. Si cambia el sistema: `tweaks.record(Op::Run, "título", &resultado)` para el diario, y
   comprobar administrador con `crate::elevation::is_elevated()` cuando haga falta.
5. Tareas largas: `crate::task::Task::new(&app, "clave").named("…")`, `task.step("…")`,
   `task.cancelled()`; en la interfaz `<TaskStatus task="clave" active={…} />`.
6. Scripts de PowerShell incrustados: prueba con `crate::ps::parse_errors(SCRIPT)` en
   `embedded_scripts_parse`.
7. Exponerla en `src/lib/api/<área>.ts` con su tipo (se importa siempre desde `lib/api`). Si el
   tipo es un struct de Rust con el mismo nombre, `types.test.ts` comprueba que los campos cuadran.

### Piezas comunes de la interfaz (úsalas, no reinventes)
- `src/components/ui.tsx`: `Card`, `Button` (`kind`, `size="sm"`), `IconButton`, `iconBtn`,
  `smallBtn`, `softBtn`, `Modal`, `Overlay`, `Loading`, `ErrorState` (con «Reintentar»),
  `EmptyLine`, `EmptyState`, `Tile`, `inputClass`. `NeedsAdmin` (en `AdminBanner.tsx`) para «requiere
  administrador». Fechas: `shortDate`, `fullDate`, `dateTime`, `timeOfDay` y `ago` de `lib/format.ts`.
  - **Todo diálogo usa `Modal`/`Overlay`**, que se pintan en `document.body` (portal). Un `fixed`
    dentro de una tarjeta o de una página con `@container` sale cortado o descolocado (fallo de la
    1.1.10, arreglado en la 1.1.11). Escape cierra solo la capa de arriba.
- `useConfirm()` / `useToast()` (`components/feedback.tsx`), `useLiveEffect((vigente)=>…)` para
  respuestas que llegan tarde, `DataTable` (ordenar por columna), `TaskStatus`.
- Navegación: `goToPage(página, foco)` (`lib/navigate.ts`). Barra lateral en
  `components/Sidebar.tsx` (`NAV`, `DEFAULT_AREAS`, `PAGE_ALIAS` para enlaces antiguos,
  `resolvePage`, `isPageId`). Páginas con pestañas en `pages/Merged.tsx` (`Tabbed`).
- Preferencias de interfaz: `lib/prefs.ts` (localStorage). Datos del técnico: archivos JSON en
  `paths::shared_data_dir` (viajan en el pendrive); los de cada equipo, en `machine_data_dir`.

### Navegación de 1.2 (diseño aprobado por el autor; prototipo https://claude.ai/artifact/N5gTV4XpZZuocsuyha7RYP)
- **Barra lateral** (`components/Sidebar.tsx`): columna de áreas (`DEFAULT_AREAS`) y, al lado, el
  árbol del área: cada pantalla y debajo sus secciones (`lib/sections.ts`, comprobado contra las
  pestañas reales por `sections.test.ts`). Fijados (`sidebar.favorites`, claves `page` o
  `page:sección`), marcas de estado (`lib/machineState.ts`). Modo «Solo áreas».
- Áreas: **Inicio** (Panel, Solucionar problemas, Sesión de servicio) · **Este equipo** (Estado del
  equipo, Optimizar Windows, Procesos, Discos, Datos del equipo) · **Red** · **Programas** ·
  **Administración** (Puestos, Usuarios y cuentas, Impresoras y carpetas, Acceso remoto, Preparar
  equipos) · **Soporte** (Agenda, Tickets, Inventario, Personas y clientes, Contactos, Soluciones).
  Fuera de las áreas (`OUTSIDE_AREAS`): Teams y Correo (iconos de la barra de arriba), Herramientas
  de Windows y Ajustes (abajo en la columna).
- **Barra de arriba** (`components/TopBar.tsx`): datos del equipo pulsables; se leen con
  `machine_context` e `internet_probe` (`src-tauri/src/context.rs`, sin PowerShell).
- **Todo AdminOps** (`CommandPalette.tsx`, Ctrl+K o F1): sin texto, el mapa por áreas; con texto,
  resultados con secciones y sinónimos.
- Herramientas de 1.2: Rendimiento (`perfhistory.rs`), Arranques y cuelgues (`bootlog.rs`,
  `lib/bugchecks.ts`), Probar periféricos, Vigilante de la conexión (`network/watch.rs`), mapa del
  espacio (`lib/treemap.ts`), calculadora de red (`lib/subnet.ts`). Teams y Correo: `comms.rs` y
  `CommOpener` (preferencia `prefs.comms`). Gráficas: `components/TimeChart.tsx`.
- Ir a una sección: `goTo(page, sección)` en App (foco + `requestSection` de `lib/sectionState.ts`);
  las pestañas cuentan cuál está a la vista con `reportSection`.
- Ajustes aparte. Quien personalizó la barra conserva la suya (se restablece en Ajustes →
  Navegación).

## 5. Funciones y decisiones que no se deben romper

- **Diario de cambios y «Deshacer»**: todo cambio guarda el valor anterior. Exportable a PDF
  (`export_journal_pdf`) y Excel.
- **Modo auditoría**: bloquea en un punto (`audit.rs`) todo lo que cambia el equipo.
- **Portales** (`src-tauri/src/portals/`, `pages/Tickets.tsx`): Tickets e Inventario web
  **navegan libremente** como un navegador (petición expresa del autor: no limitar); el Correo,
  Teams y los routers se quedan en sus dominios. Las páginas no tienen acceso a los comandos de la
  app. La cuenta guardada se rellena sola («Entrar solo»).
- **Informe PDF** (`diagnostics/report/`): plantillas cliente, técnica y **propia**
  (`Template::Custom`, `Settings.report_layout`; secciones en `workflow::REPORT_SECTIONS`, que una
  prueba compara con `ReportLayoutEditor.tsx`).
- **Actualizar desde la app** (`appcare::install_update`): descarga el `AdminOps-X-Setup.exe`
  (o el zip en portable) de GitHub Releases, comprueba tamaño y huella, y abre el instalador. **No
  hay ninguna versión publicada en GitHub**, así que hoy dice «no hay versiones publicadas».
- **Ayuda** (`components/HelpCenter.tsx`, `lib/help.ts`): guía (`lib/guide.ts`, 12 capítulos), glosario,
  preguntas, **novedades** (`lib/changelog.ts`), términos y **Reportar un problema** (prepara un
  correo con el paquete de soporte adjunto; no envía nada solo). Una prueba exige que
  `RELEASES[0].version` sea la versión de `package.json` y que no haya fechas: **al subir de
  versión, añade su entrada a `changelog.ts`**. Otra comprueba que cada «Abrir esta pantalla» de la
  guía lleva a una página que existe.
- **Términos de uso**: una sola fuente, `src-tauri/terminos.txt` (app, instalador propio y NSIS
  `licenseFile`).
- **Catálogo de programas** (`src-tauri/tools/apps.toml`, `pages/Install.tsx`): 147 programas;
  `home = true` = uso personal (no sale en la vista «Empresa», que es la de fábrica). Vista y
  ocultos en `app-catalog-view.json`. **Cada id nuevo se comprueba antes con
  `winget show --id <id> --exact`.**
- **Carpetas compartidas** (`shares.rs`), **Usuarios** con avisos (`lib/userIssues.ts`),
  **Procesos** agrupados (`lib/processGroups.ts`), **Sesión de servicio por pasos**, **Agenda**
  (lista, mes, historial), **Acceso remoto** (agenda RDP/AnyDesk/RustDesk/TeamViewer).
- Buscador de Ajustes: `pages/settings/catalog.ts`; una prueba exige que cada título exista en la
  pantalla.
- Datos antiguos (agendas con campos de Outlook, archivos de empresa con Microsoft 365) se siguen
  leyendo: no añadas `deny_unknown_fields` a esos tipos.

## 6. Verificación antes de dar algo por hecho

Desde `F:\Programacion\AdminOps`:
```
npx tsc --noEmit -p .
npx eslint .
npx vitest run
npx vite build
cd src-tauri
cargo clippy --all-targets -- -D warnings
cargo test --lib
CARGO_TARGET_DIR=target-ci cargo +1.98.1 clippy --all-targets -- -D warnings
```
La segunda versión de Rust (1.98.1) es la de la CI y tiene lints distintos (p. ej.
`collapsible_match`): pásala siempre. Hay **una prueba de Rust que falla de vez en cuando**; si
falla una vez y pasa al repetir, dilo, no lo escondas.

Di siempre con honestidad qué está compilado, qué está probado y qué no se ha visto en pantalla.

## 7. Versiones, build y documentación

- Cambiar versión en todos los sitios: `npm run setversion X.Y.Z` (y la insignia del README).
  Regla: **una build, una versión**; si ya se compiló una versión, lo nuevo va en la siguiente.
- Build (solo si el autor dice «lanza build»): `npm run build:release` en segundo plano (~15 min);
  deja `release\vX.Y.Z\` con `-Setup.exe`, `-instalador-clasico.exe` y `-portable.zip`.
- Documentación: `README.md` / `README.en.md`, `ROADMAP.md` (plan e historial), `docs/PRUEBAS.md`
  (lista de pruebas a mano por versión), `docs/MANUAL.md`, `docs/INSTALACION-IT.md`,
  `docs/TERCEROS.md`, `docs/documento-sistema.html` → `AdminOps-Documento-del-sistema.pdf`
  (análisis interno con FODA), `docs/presentacion.html` → `AdminOps-Presentacion.pdf` (para
  presentar la app).

## 8. Estado y pendientes al cerrar la sesión anterior

**Hecho en la 1.1.11, sin compilar ni guardar en Git:**
- Diálogos dibujados sobre la ventana entera (`Overlay`) y tarjetas que ya no recortan.
- Catálogo: vista «Empresa»/«Todo», «Personalizar» (ocultar programas y categorías), 34 programas
  de empresa nuevos, categorías «VPN y escritorios de la empresa» y «Copias de seguridad», lista
  «Puesto de empresa». WhatsApp quedó como «uso personal»: preguntar al autor si en su oficina se
  usa para trabajar.

**Hecho en 1.1.11** (lista del artifact https://claude.ai/artifact/Cep2faNQ17Aq99VQEgxyg8):
los grupos F (fallos), V (visual), M (mecánicas) y R (reorganizar), salvo lo que sigue. Detalle en
ROADMAP.md, sección v1.1.11.

**Pendiente**:
- Visual: revisar el tema claro pantalla por pantalla y hacer capturas nuevas (no entraron).
- Partir `Tickets.tsx`; tipos generados desde Rust si algún día compensa (hoy lo cubre
  `types.test.ts`).
- Acceso remoto (ofrecido, sin respuesta): ocultar las herramientas de terceros y dejar solo lo de
  Windows; «Pedir ayuda» en modo usuario (nombre del equipo e IP); botón «Conectar» (Escritorio
  remoto) en Puestos, Inventario y la ficha de una persona.
- Proceso: commit de lo pendiente (cuando lo pida), prueba que abra la app y recorra las
  pantallas, publicar versiones en GitHub Releases.

Empieza preguntando al autor qué quiere hacer primero solo si no lo ha dicho; si lo ha dicho,
ponte a ello directamente.
