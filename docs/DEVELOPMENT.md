# AdminOps — Guía de desarrollo

> Documentación técnica para compilar y extender AdminOps. La presentación del proyecto está en el [README](../README.md).

**Stack:** Tauri 2 (Rust) + React 19 + Vite + Tailwind CSS 4.

## Desarrollo

```bash
npm install
npm run tauri dev      # corre como usuario normal; la UI ofrece "Reiniciar como admin"
npm run tauri build    # instalador NSIS en src-tauri/target/release/bundle/
npm run build:release  # instalador + zip portable juntos en release/v<versión>/
```

## Instalador personalizado

`installer/` es una app Tauri mínima con interfaz propia (`installer/ui/index.html`) que lleva dentro el
instalador NSIS de AdminOps (`installer/build.rs` lo incrusta) y lo ejecuta en silencio (`/S /D=carpeta`).
`npm run build:release` la compila después de AdminOps y deja en `release/v<versión>/` el instalador
nuevo (`-Setup.exe`), el clásico (`-instalador-clasico.exe`, admite `/S` para despliegues) y el portable.
Si el equipo no tiene WebView2, el instalador nuevo abre directamente el clásico.

## Modo portable

Si junto a `AdminOps.exe` hay un archivo `AdminOps.portable`, todos los datos van a
`AdminOps-data\` al lado del ejecutable: `equipos\<NOMBRE-PC>\` (diario y análisis de cada equipo)
e `Informes\` (los PDF de todos). `npm run build:release` deja el zip (listo para un USB) junto al instalador en `release\v<versión>\`.

## Perfiles

`src-tauri/tweaks/profiles.toml`: Oficina, Gaming, Equipo viejo y Privacidad máxima. Un perfil
aplica sus ajustes con un único punto de restauración; cada ajuste queda en el diario, así que
se puede deshacer el perfil entero o ajuste por ajuste. `cargo test` verifica que los perfiles
solo usen ids existentes del catálogo.

## Ícono

Fuente única: `src/assets/logo.svg` (también se usa en la barra lateral y en el informe PDF).
Para regenerar todos los tamaños (ventana, barra de tareas, .exe, instalador):

```bash
# exportar el SVG a PNG de 1024 px como src-tauri/icons/icon-source.png, luego:
npx tauri icon src-tauri/icons/icon-source.png
```

## Estructura

```
src/                     Frontend React
  lib/api.ts             Únicos comandos que la UI puede invocar
  hooks/useLiveMetrics   Sondeo de métricas (se pausa con la ventana oculta)
  pages/Dashboard.tsx    Panel en vivo
src-tauri/src/
  metrics.rs             CPU, RAM, discos, red y procesos (sysinfo)
  elevation.rs           Detección de admin y relanzamiento con UAC
```

## Añadir un ajuste

Los ajustes viven en `src-tauri/tweaks/*.toml` y se incrustan en el binario al compilar
(esquema completo en `src-tauri/src/tweaks/model.rs`). El motor detecta, respalda y revierte
solo las acciones de registro y servicios; los scripts necesitan `detect`, `apply` y `revert`.

```toml
[[tweak]]
id = "privacy.ejemplo"          # debe empezar por la categoría
name = "Nombre visible"
description = "Qué hace"
category = "privacy"
risk = "low"                    # low | medium | high (medium+ crea punto de restauración)
note = "Efectos secundarios"   # opcional

[[tweak.registry]]
path = 'HKCU\Software\...'
name = "Valor"
type = "dword"                  # dword | qword | string | expand
value = 0
default = 1                     # de fábrica; si falta, revertir sin historial borra el valor

[[tweak.service]]
name = "DiagTrack"
startup = "disabled"            # automatic | delayed | manual | disabled
default = "automatic"
stop = true
```

Los scripts `apply` pueden imprimir un valor (p. ej. el plan de energía activo); `revert` lo recibe
como `$Previous` para volver exactamente al estado anterior.

Tareas puntuales: `kind = "action"` con `[tweak.script] run = '''...'''`.
`cargo test` valida el catálogo (ids únicos, campos obligatorios).

## Bloatware e Inicio

- `src-tauri/tweaks/bloatware.toml`: apps conocidas con recomendación (`remove`/`optional`/`keep`) y
  `store_id` verificado con `winget show --id <id> --source msstore` para poder reinstalarlas.
  Runtimes, códecs, idiomas, Store/winget y componentes del shell están protegidos (`appx.rs`).
- Inicio: claves Run, carpetas Inicio y tareas programadas de inicio de sesión. Desactivar usa
  `Explorer\StartupApproved`, igual que el Administrador de tareas: no se borra nada.

## Usuario destino

Si AdminOps se eleva con otra cuenta de administrador, los ajustes de HKCU se aplican al usuario con
la sesión abierta (dueño de `explorer.exe`, vía `HKEY_USERS\<SID>`), no a la cuenta del técnico.

## Diagnóstico e informe

- `src-tauri/src/diagnostics/collect.rs`: recolectores en PowerShell (discos físicos y SMART,
  pantallazos azules y apagados inesperados, apps que fallan, drivers con error, batería,
  Defender/actualizaciones/activación/TPM). Corren en paralelo; si uno falla, el resto sigue.
- `diagnostics/mod.rs` convierte los datos en hallazgos priorizados y guarda cada análisis en
  `%APPDATA%\com.adminops.app\snapshots\` para comparar antes/después.
- `diagnostics/report.rs` arma el informe y `diagnostics/pdf.rs` lo convierte a PDF (A4) con Microsoft
  Edge en modo headless (incluido en Windows 10/11). Se guarda en `Documentos\AdminOps\Informes` y se
  abre vía `explorer.exe` para que el visor no herede los permisos de administrador. Si Edge no está,
  se guarda como HTML.
- Reparaciones (`tweaks/repair.toml`): SFC, DISM, red, Windows Update, cola de impresión,
  Explorador, hora, caché de iconos y prueba de RAM.

## Deshacer

Cada aplicación guarda el valor exacto anterior (tipo + bytes del registro, tipo de inicio y
estado del servicio) en `%APPDATA%\com.adminops.app\journal.json`. "Deshacer" restaura eso; si el
ajuste ya venía aplicado de antes, usa los valores `default` del catálogo. Si algo falla a mitad
de un ajuste, lo ya cambiado se revierte antes de devolver el error.

## Regla de seguridad

La UI nunca envía scripts ni comandos libres al backend. Cada acción es un comando Rust
concreto (y, desde la Fase 2, un `id` de tweak del catálogo embebido en el binario).

## Calidad y rendimiento

- **CI** (`.github/workflows/ci.yml`): tipos, Clippy, tests, instalador, portable y prueba de ida y vuelta.
- **Prueba de ida y vuelta**: `adminops.exe --roundtrip informe.json [--only id1,id2]` aplica y deshace
  cada ajuste y comprueba que todo vuelve exactamente a su estado. **Modifica el sistema**: úsala en la
  CI o en Windows Sandbox (`tests/sandbox/run-roundtrip.ps1`), no en un PC real.
- **Registro de actividad**: `%LOCALAPPDATA%\com.adminops.app\logs\adminops.log` (o `AdminOps-data\equipos\<PC>\logs`
  en portable). Visible en Historial → Registro técnico.
- **Rendimiento**: `cargo test --release bench -- --ignored --nocapture` (consultas) y `scripts/bench.ps1`
  (arranque, RAM y CPU; requiere compilar con `ADMINOPS_ASINVOKER=1`, solo para medir).
- Las consultas a PowerShell usan procesos persistentes (`src-tauri/src/pspool.rs`); lo que modifica el
  sistema o se puede cancelar va en procesos aislados (`ps.rs`). `ADMINOPS_NO_PS_POOL=1` lo desactiva.


## Roadmap

Fases 1–18 completadas (v0.18.0). Lo siguiente, la deuda técnica conocida y los criterios de cada
fase están en [ROADMAP.md](../ROADMAP.md).
