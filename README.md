# AdminOps

Herramienta de escritorio para técnicos de soporte: diagnóstico y optimización de Windows 10/11.

**Stack:** Tauri 2 (Rust) + React 19 + Vite + Tailwind CSS 4.

## Desarrollo

```bash
npm install
npm run tauri dev      # corre como usuario normal; la UI ofrece "Reiniciar como admin"
npm run tauri build    # instalador NSIS en src-tauri/target/release/bundle/
```

El ejecutable de **release** lleva un manifiesto `requireAdministrator` y pide UAC al abrirse.
En **debug** usa `asInvoker` para que `tauri dev` funcione sin una terminal elevada
(ver `src-tauri/build.rs` y `src-tauri/manifests/`).

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

## Deshacer

Cada aplicación guarda el valor exacto anterior (tipo + bytes del registro, tipo de inicio y
estado del servicio) en `%APPDATA%\com.adminops.app\journal.json`. "Deshacer" restaura eso; si el
ajuste ya venía aplicado de antes, usa los valores `default` del catálogo. Si algo falla a mitad
de un ajuste, lo ya cambiado se revierte antes de devolver el error.

## Regla de seguridad

La UI nunca envía scripts ni comandos libres al backend. Cada acción es un comando Rust
concreto (y, desde la Fase 2, un `id` de tweak del catálogo embebido en el binario).

## Roadmap

1. **Base** ✅: proyecto, tema, UAC, panel en vivo.
2. **Motor de tweaks** ✅: formato declarativo (detect/apply/revert), journal de cambios, puntos de restauración.
3. **Catálogo** ✅: limpieza, privacidad/telemetría, bloatware, servicios, programas de inicio.
4. **Diagnóstico e informes**: SMART, eventos/BSOD, drivers, batería, informe exportable.
5. **Pulido**: perfiles, modo portable, firma de código.
