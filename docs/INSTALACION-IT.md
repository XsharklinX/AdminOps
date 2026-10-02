# AdminOps — Guía de instalación para IT

Lo que IT necesita para aprobar y desplegar AdminOps. El detalle técnico y de seguridad está en
`docs/PRESENTACION.md`; aquí, lo práctico.

## Requisitos

- Windows 10 (21H2 o posterior) o Windows 11, 64 bits.
- **WebView2 Runtime** (viene con Windows 11 y con Edge actualizado; si falta, el instalador
  clásico lo instala).
- 300 MB de disco. Sin servicios, sin controladores propios, sin tareas programadas salvo las que el
  técnico active (mantenimiento programado).

## Formas de instalarlo

| Archivo | Para |
|---|---|
| `AdminOps-x.y.z-Setup.exe` | Lo normal. Pide administrador (escribe en Archivos de programa o en la carpeta elegida, por ejemplo un pendrive). |
| `AdminOps-x.y.z-instalador-clasico.exe` | Equipos sin WebView2, o despliegue silencioso: `/S /D=C:\Ruta\AdminOps`. |
| `AdminOps-x.y.z-portable.zip` | Sin instalar: se descomprime y se ejecuta. No deja nada en el equipo (todo en `AdminOps-data` junto al programa). |

Instalado en una **unidad extraíble**, AdminOps guarda sus datos junto al programa (modo portable)
y el navegador interno en el disco de cada equipo. La firma del ejecutable no está incluida:
SmartScreen avisará la primera vez.

## Permisos

**No necesita administrador para consultar.** Las acciones que cambian el equipo (reparaciones,
ajustes, discos, usuarios) piden elevación en ese momento, como cualquier herramienta de Windows,
y quedan en un diario con «Deshacer». El **modo auditoría** bloquea todas en un único punto.

Para el dominio usa los permisos de la cuenta del técnico (ADSI, sin RSAT): si la cuenta no puede
desbloquear usuarios o leer LAPS, AdminOps lo dice y no hace nada.

## Conexiones de red

| Destino | Cuándo | Qué envía |
|---|---|---|
| `api.github.com` | Solo con «Avisar de versiones nuevas» activado | Nada (consulta la última versión) |
| `ipinfo.io` | Solo al pulsar «IP pública» | Nada (GET) |
| `speed.cloudflare.com` | Solo al hacer el test de velocidad | Tráfico de prueba |
| `login.microsoftonline.com`, `graph.microsoft.com` | Solo si se conecta Microsoft 365 | Las consultas del técnico con su cuenta |
| Portales configurados (Outlook, Teams, Tickets…) | Al abrirlos | Lo que enviaría el navegador |
| Red local: SNMP (UDP 161), mDNS (224.0.0.251:5353), WS-Discovery (239.255.255.250:3702) | Solo al revisar o buscar impresoras | Consultas de solo lectura |

Sin telemetría, sin analítica, sin servidor propio.

## Microsoft 365 (opcional)

Para las funciones de Microsoft 365 (inicios de sesión, MFA, estado del servicio, calendario,
Teams, presencia y fotos), IT registra **una aplicación** en Entra ID:

1. *Registros de aplicaciones* → Nuevo registro → «Solo este directorio» → plataforma
   *Aplicaciones móviles y de escritorio*.
2. *Autenticación* → activar **Permitir flujos de clientes públicos** (inicio de sesión por código).
3. Permisos **delegados** de Microsoft Graph con consentimiento de administrador: `User.Read`,
   `AuditLog.Read.All`, `Directory.Read.All`, `UserAuthenticationMethod.ReadWrite.All`,
   `User.RevokeSessions.All`, `ServiceHealth.Read.All`, `Calendars.ReadWrite`, `Chat.Create`,
   `ChatMessage.Send`, `Presence.Read.All`, `User.ReadBasic.All`. Los que no se den, esa función
   avisa y el resto funciona.
4. Dar al técnico el **inquilino** y el **id de la aplicación** (o ponerlos en el archivo de
   configuración de empresa). Cada técnico inicia sesión con su cuenta y su MFA.

Al ser permisos delegados, AdminOps nunca puede más de lo que puede la cuenta del técnico por su
rol (por ejemplo, quitar el MFA de otro exige *Administrador de autenticación*).

## Antivirus

AdminOps usa PowerShell y WMI para consultar el equipo (como las herramientas de Windows). Si el
antivirus corporativo lo bloquea, añadir a la lista de confianza la **carpeta de instalación** (o
la del pendrive) tras revisarlo. Si PowerShell está restringido por AppLocker/WDAC (modo de lenguaje
restringido), AdminOps lo detecta al arrancar y lo dice; buena parte de las funciones no
trabajarán en ese equipo.

## Componentes de terceros y vulnerabilidades

La lista completa con licencias está en `docs/TERCEROS.md` (ninguna GPL/AGPL). La CI ejecuta
`cargo audit` y `npm audit` en cada cambio y falla si aparece una vulnerabilidad conocida.

## Desinstalar

Desde *Aplicaciones instaladas* de Windows. El desinstalador quita los archivos del programa; los
datos (`%APPDATA%\com.adminops.app`, o `AdminOps-data` junto al programa) se conservan salvo que se
marque borrarlos.
