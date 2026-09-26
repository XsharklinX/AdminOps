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

## Regla de seguridad

La UI nunca envía scripts ni comandos libres al backend. Cada acción es un comando Rust
concreto (y, desde la Fase 2, un `id` de tweak del catálogo embebido en el binario).

## Roadmap

1. **Base** ✅: proyecto, tema, UAC, panel en vivo.
2. **Motor de tweaks**: formato declarativo (detect/apply/revert), journal de cambios, puntos de restauración.
3. **Catálogo**: limpieza, privacidad/telemetría, bloatware, servicios, programas de inicio.
4. **Diagnóstico e informes**: SMART, eventos/BSOD, drivers, batería, informe exportable.
5. **Pulido**: perfiles, modo portable, firma de código.
