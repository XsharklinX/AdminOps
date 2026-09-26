<div align="center">

<img src="src/assets/logo.svg" alt="AdminOps" width="96" />

# AdminOps

**Diagnóstico, optimización y servicio técnico para Windows 10/11, en una sola app.**

🇪🇸 **Español** · [🇬🇧 English](README.en.md)

[![Versión](https://img.shields.io/badge/versión-0.10.0-22d3ee?style=flat-square)](https://github.com/XsharklinX/AdminOps/releases)
[![Plataforma](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4?style=flat-square&logo=windows)](#requisitos)
[![Hecho con Tauri](https://img.shields.io/badge/Tauri%202-Rust%20%2B%20React-a855f7?style=flat-square&logo=tauri)](docs/DEVELOPMENT.md)
[![Descargar](https://img.shields.io/badge/descargar-instalador%20%7C%20portable-22c55e?style=flat-square)](https://github.com/XsharklinX/AdminOps/releases/latest)

<img src="docs/screenshots/dashboard.png" alt="Panel de AdminOps" width="860" />

</div>

---

## ¿Qué es AdminOps?

AdminOps es una herramienta de escritorio pensada para **técnicos de soporte** y usuarios avanzados.
Reúne en una sola ventana lo que normalmente exige una docena de programas: ver el estado del equipo
en vivo, encontrar lo que falla, aplicar optimizaciones seguras **que se pueden deshacer** y entregar
al cliente un **informe PDF** profesional del trabajo realizado.

- ⚡ **Ligera y rápida**: hecha en Rust + WebView2, arranca en segundos y consume poca memoria.
- 🛟 **Segura**: cada cambio guarda el valor anterior exacto y se puede deshacer; los cambios de riesgo crean un punto de restauración.
- 🔒 **Privada**: sin cuentas, sin telemetría. Todos los datos se quedan en el equipo.
- 🧳 **Portable**: llévala en un USB y úsala en cada cliente sin instalar nada.

## Funciones

### 📊 Panel en vivo
CPU por núcleo, memoria, discos, red, temperaturas y los procesos que más consumen, actualizados cada 2 segundos.

### 🩺 Diagnóstico
Un análisis completo en segundos: salud de discos (SMART), pantallazos azules y apagados inesperados,
aplicaciones que fallan, drivers con error, batería, Defender, Windows Update, activación y TPM.
Cada hallazgo viene priorizado y con **un botón para resolverlo**. Los análisis se guardan para comparar el antes y el después.

<img src="docs/screenshots/diagnostics.png" alt="Diagnóstico" width="860" />

### 🖥️ Hardware
Inventario completo (placa base, BIOS, CPU, RAM por módulo, GPU, discos, monitores), **sensores en vivo**
(temperaturas, ventiladores, voltajes) con LibreHardwareMonitor, estado SMART detallado de cada disco y resultado de la prueba de memoria de Windows.

<img src="docs/screenshots/hardware.png" alt="Hardware" width="860" />

### 🌐 Red y velocidad
Test de velocidad profesional (descarga, subida, latencia y jitter con varias conexiones en paralelo),
datos de la conexión (proveedor, IP pública con opción de ocultarla, servidor) y diagnóstico de red: adaptadores, puerta de enlace, DNS y conectividad.

<img src="docs/screenshots/network.png" alt="Red y velocidad" width="860" />

### ⚙️ Procesos
Lista en vivo con CPU, memoria y disco por proceso. Busca por nombre, PID, ruta o usuario y **finaliza procesos**
(o el árbol completo) desde la propia app. Los procesos críticos del sistema están marcados para no cerrarlos por error.

<img src="docs/screenshots/processes.png" alt="Procesos" width="860" />

### 🎛️ Perfiles
Aplica decenas de ajustes de una vez: **Oficina**, **Gaming**, **Equipo viejo** y **Privacidad máxima**,
o crea los tuyos. Un solo punto de restauración por perfil y se puede deshacer entero o ajuste por ajuste.

<img src="docs/screenshots/profiles.png" alt="Perfiles" width="860" />

### 🧹 Optimizar
| Sección | Qué hace |
| --- | --- |
| **Limpieza** | Temporales, caché de Windows Update, papelera, miniaturas, registros y más. |
| **Rendimiento** | Plan de energía, efectos visuales, indexación, juegos y otros ajustes. |
| **Privacidad** | Telemetría, publicidad, Copilot, historial de actividad… |
| **Bloatware** | Quita apps preinstaladas con recomendación por app; las de la Store se pueden reinstalar. |
| **Servicios** | Desactiva servicios innecesarios con explicación y reversión. |
| **Inicio** | Controla qué arranca con Windows (igual que el Administrador de tareas, sin borrar nada). |
| **Actualizar software** | Detecta programas desactualizados y los actualiza con winget. |
| **Espacio en disco** | Analiza qué carpetas ocupan más, muy rápido incluso en discos grandes. |

### 🧰 Servicio técnico
| Sección | Qué hace |
| --- | --- |
| **Sesión de servicio** | Registra el trabajo en un equipo: diagnóstico inicial, cambios, checklist y cierre. |
| **Clientes** | Fichas de clientes y sus equipos, con historial de cada visita. |
| **Reparaciones** | SFC, DISM, reinicio de red, Windows Update, cola de impresión, Explorador, hora, caché de iconos y prueba de RAM. |
| **Informe** | PDF profesional listo para entregar: estado del equipo, hardware, hallazgos, cambios y velocidad. |
| **Historial** | Todo lo aplicado, con opción de deshacer, y el registro técnico de actividad. |

## Descarga e instalación

Descarga la última versión desde **[Releases](https://github.com/XsharklinX/AdminOps/releases/latest)**. Hay dos opciones:

| Archivo | Para qué |
| --- | --- |
| `AdminOps_x.y.z_x64-setup.exe` | **Instalador**: para tu propio equipo. Crea accesos directos y se desinstala desde Windows. |
| `AdminOps-x.y.z-portable.zip` | **Portable**: descomprímelo en un USB. Guarda los datos al lado del ejecutable (`AdminOps-data\`), separados por equipo. |

> [!NOTE]
> AdminOps todavía no está firmado digitalmente, así que Windows SmartScreen puede mostrar
> *"Windows protegió su PC"*. Pulsa **Más información → Ejecutar de todas formas**.

### Requisitos
- Windows 10 u 11 de 64 bits.
- Microsoft Edge WebView2 (ya incluido en Windows 11 y en Windows 10 actualizado).
- **Permisos de administrador** para aplicar cambios. Sin ellos la app funciona en modo solo lectura y ofrece *Reiniciar como admin*.
- Opcional: el driver **PawnIO** para leer temperaturas de CPU y placa base (la app ofrece instalarlo desde la pestaña Hardware).

## Cómo usarla

1. **Abre AdminOps como administrador** (o pulsa *Reiniciar como admin* en el aviso superior).
2. Revisa el **Panel** para ver el estado general del equipo.
3. Ejecuta un **Diagnóstico** y resuelve los hallazgos con sus botones de acción.
4. Aplica un **Perfil** o los ajustes que quieras en **Optimizar**. Todo queda en el **Historial** y se puede deshacer.
5. Si atiendes a un cliente: abre una **Sesión de servicio**, asígnala al cliente y al terminar genera el **Informe PDF**.

> [!TIP]
> Antes de cambios importantes, AdminOps crea un punto de restauración de Windows automáticamente.
> Si algo no te convence, ve a **Historial** y pulsa *Deshacer*.

## Privacidad

- No hay cuentas, anuncios ni telemetría. Nada sale del equipo.
- Los datos (historial, clientes, informes) se guardan en local: `%APPDATA%` con el instalador o `AdminOps-data\` en modo portable.
- Conexiones externas solo cuando las pides: el test de velocidad (Cloudflare y `ipinfo.io` para mostrar el proveedor) y las actualizaciones de software (winget).

## Compilar desde el código

```bash
npm install
npm run tauri dev        # modo desarrollo
npm run build:release    # instalador + portable en release/v<versión>/
```

Requiere Node.js 22+, Rust estable y las herramientas de compilación de C++ de Visual Studio.
Arquitectura, catálogo de ajustes y pruebas: **[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)**.
Próximas versiones: **[ROADMAP.md](ROADMAP.md)**.

## Componentes de terceros

AdminOps incluye [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) (MPL-2.0)
y sus dependencias para la lectura de sensores. Licencias completas en
[`src-tauri/resources/lhm/THIRD-PARTY-NOTICES.txt`](src-tauri/resources/lhm/THIRD-PARTY-NOTICES.txt)
(también accesibles desde la pestaña *Hardware* de la app).

## Autor

Creado por **David Bonilla**.

© 2026 David Bonilla. Todos los derechos reservados.
