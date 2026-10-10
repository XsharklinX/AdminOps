<div align="center">

<img src="src/assets/logo.svg" alt="AdminOps" width="96" />

# AdminOps

**Diagnóstico, mantenimiento y soporte técnico para Windows 10 y 11, en una sola aplicación.**

🇪🇸 **Español** · [🇬🇧 English](README.en.md)

[![Versión](https://img.shields.io/badge/versión-1.2.9-22d3ee?style=flat-square)](https://github.com/XsharklinX/AdminOps/releases)
[![Plataforma](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4?style=flat-square&logo=windows)](#requisitos)
[![Hecho con Tauri](https://img.shields.io/badge/Tauri%202-Rust%20%2B%20React-a855f7?style=flat-square&logo=tauri)](docs/DEVELOPMENT.md)

</div>

---

## Qué es

AdminOps es un programa de escritorio para quien da **soporte técnico**: llega a un equipo que va
lento, que no imprime o que no entra a la red, y tiene que averiguar qué pasa, arreglarlo y dejar
constancia de lo que hizo.

Reúne en una ventana lo que normalmente se reparte entre una docena de herramientas, lo explica en
español claro y añade lo que Windows no trae: **memoria** de qué se cambió, cuándo, en qué equipo y
para qué cliente.

- **Se puede deshacer.** Cada cambio guarda el valor anterior exacto. Los de riesgo crean antes un punto de restauración.
- **Privado.** Sin cuentas, sin telemetría y sin servidor. Los datos se quedan en el equipo o en tu pendrive.
- **Portable.** Va en un pendrive, con tus clientes, contactos y ajustes, y no deja nada en el equipo del cliente.
- **Ligero.** Rust y WebView2: instalador de unos 11 MB.
- **Explica.** Cada error dice qué pasó y qué hacer; cada hallazgo trae el botón que lo resuelve.
- **Dos modos.** Técnico (todo) y usuario (lo esencial, para dejarlo en el equipo de alguien). Es un modo de interfaz, no una barrera de seguridad.

Dentro de la aplicación, **Acerca de → Guía** explica cada pantalla con detalle, con glosario y
preguntas frecuentes.

## Qué hace

| Área | Lo principal |
| --- | --- |
| **Inicio** | Panel del equipo en vivo. Solución guiada de problemas (no hay Internet, no suena, no imprime, va lento…). Sesión de servicio por pasos: motivo, trabajo, informe, cobro y firma del cliente, con el **informe en PDF** que compara el antes y el después, con tu propia plantilla si quieres. |
| **Este equipo** | Diagnóstico con hallazgos por gravedad. Hardware y temperaturas. Seguridad con nota de 0 a 100. Rendimiento de los últimos 7 días, arranques y pantallazos azules explicados, y prueba de periféricos. Optimizar Windows con ajustes reversibles. Procesos agrupados por programa. Discos: espacio (con mapa), salud, reparación y rescate de archivos. **Datos**: copia a otro equipo, caja fuerte cifrada, borrado seguro, recuperar archivos y control parental. Historial con el diario de cambios, exportable a PDF y Excel. |
| **Red** | Router y Wi‑Fi, dispositivos de la red, velocidad, reparar la red, vigilante de cortes de la conexión, ping, traza de ruta, puertos, DNS, hosts y calculadora de red. |
| **Programas** | Actualizar, instalar en lote y desinstalar programas; Windows Update; quitar aplicaciones preinstaladas. |
| **Administración** | Puestos de la oficina. Usuarios y cuentas: los usuarios de este equipo con avisos de lo que conviene revisar, las cuentas y el dominio. Impresoras. Carpetas compartidas: permisos, «¿por qué no puede entrar?», unidades de red y copia diaria. Acceso remoto. Plantillas para preparar un equipo nuevo de una vez. |
| **Soporte** | Agenda con vista de mes e historial. Tickets e Inventario web, que se usan como un navegador. Inventario propio de equipos. Personas del dominio y Clientes con sus equipos, visitas y garantías. Contactos. Soluciones paso a paso y plantillas de texto. |

La barra lateral enseña cada pantalla con sus secciones, sin nada escondido en pestañas; la barra de
arriba, el equipo en una línea (nombre, dominio, permisos, Internet, disco y avisos), con Teams y
Correo a un clic; y **Todo AdminOps** (Ctrl+K o F1) el programa entero en una pantalla, con un
buscador que entiende otras formas de decirlo («AD», «no imprime»).

Dos piezas atraviesan todo: el **diario de cambios**, que anota lo que el programa modifica y
permite deshacerlo, y el **modo auditoría**, en el que AdminOps solo mira y no cambia nada.

La lista de lo añadido en cada versión está en la aplicación (**Acerca de → Novedades**) y en
[ROADMAP.md](ROADMAP.md).

## Descarga e instalación

Descarga la última versión desde **[Releases](https://github.com/XsharklinX/AdminOps/releases/latest)**.

| Archivo | Para qué |
| --- | --- |
| `AdminOps-x.y.z-Setup.exe` | **Instalador.** Detecta si ya tienes AdminOps y lo actualiza conservando tus datos. Si lo instalas en un pendrive, los datos se guardan junto al programa. |
| `AdminOps-x.y.z-instalador-clasico.exe` | **Instalador clásico.** El mismo, con el asistente de siempre. Admite `/S` para instalar en silencio en muchos equipos. |
| `AdminOps-x.y.z-portable.zip` | **Portable.** Se descomprime en una carpeta o un pendrive y guarda ahí sus datos, separados por equipo. |

> [!NOTE]
> AdminOps no está firmado digitalmente, así que Windows SmartScreen puede mostrar
> *«Windows protegió su PC»*. Pulsa **Más información → Ejecutar de todas formas**.
> Descárgalo solo desde esta página.

### Requisitos

- Windows 10 u 11 de 64 bits.
- Microsoft Edge WebView2 (incluido en Windows 11 y en Windows 10 actualizado).
- **Permisos de administrador** para aplicar cambios. Sin ellos la aplicación lee, pero no cambia, y ofrece *Reiniciar como admin*.

## Cómo empezar

1. Abre AdminOps como administrador (o pulsa *Reiniciar como admin* en el aviso de arriba).
2. Mira el **Panel** y lanza un **Diagnóstico**.
3. Resuelve los hallazgos con sus botones. Todo queda en **Historial → Diario de cambios**, con su *Deshacer*.
4. Si atiendes a un cliente, abre una **Sesión de servicio** y al terminar genera el informe.
5. `Ctrl+K` busca cualquier página, herramienta, ajuste o problema.

## Privacidad y responsabilidad

- No hay cuentas, anuncios ni telemetría.
- Los datos se guardan en local: en la carpeta del usuario si está instalado, o junto al programa en un pendrive. Las contraseñas que decidas guardar se cifran.
- Solo se conecta a Internet para lo que le pides: comprobar si hay versión nueva, la prueba de velocidad, instalar o actualizar programas con winget y las webs que abras dentro.
- **Reportar un problema** prepara un correo para el autor con un archivo de diagnóstico; no sale nada si tú no lo envías.
- Usa AdminOps solo en equipos tuyos o en los que tengas autorización. El programa se entrega sin garantía: los **[términos de uso](src-tauri/terminos.txt)** se aceptan al instalar y están en *Acerca de*.

## Documentación

| Documento | Para quién |
| --- | --- |
| [Manual del técnico](docs/MANUAL.md) | Quien usa AdminOps. |
| [Documento del sistema](docs/AdminOps-Documento-del-sistema.pdf) | Qué es, cómo funciona, cómo está hecho y análisis FODA. |
| [Guía de instalación para sistemas](docs/INSTALACION-IT.md) | Qué permisos pide y qué conexiones hace. |
| [Guía de desarrollo](docs/DEVELOPMENT.md) | Compilar y extender AdminOps. |
| [Lista de pruebas](docs/PRUEBAS.md) | Lo que hay que comprobar a mano antes de publicar. |
| [Componentes de terceros](docs/TERCEROS.md) | Licencias de lo que AdminOps incluye. |

## Compilar desde el código

```bash
npm install
npm run tauri dev        # modo desarrollo
npm run build:release    # instalador + portable en release/v<versión>/
```

Requiere Node.js 22 o posterior, Rust estable y las herramientas de compilación de C++ de Visual Studio.

**Tecnologías:** Tauri 2 · Rust · React 19 · TypeScript · Vite · Tailwind CSS 4 · WebView2 ·
PowerShell y WMI para las consultas a Windows · LibreHardwareMonitor para los sensores.

## Autor

Creado por **David Bonilla**. Para informar de un fallo, usa *Acerca de → Reportar un problema*
dentro de la aplicación, o escribe a Contactoyerlindavid@gmail.com.

© 2026 David Bonilla. Todos los derechos reservados.
