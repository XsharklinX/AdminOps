# AdminOps — Manual del técnico

Para empezar a usar AdminOps sin que te lo expliquen. Cinco minutos de lectura; lo demás se
aprende usándolo (cada página tiene un «?» junto al título que dice para qué sirve).

## 1. El primer día

1. **Instálalo** con `AdminOps-x.y.z-Setup.exe`. Puede ir en el disco del equipo o en un
   **pendrive**: instalado en un pendrive, tus datos (clientes, contactos, agenda, contraseñas
   guardadas) viajan con él a cualquier equipo. Para actualizar, vuelve a pasar el instalador
   sobre la misma carpeta: los datos no se tocan.
2. **Si te pasaron el archivo de la empresa** (`AdminOps-empresa-….json`): Panel → *Primeros
   pasos* → «Importar la configuración de la empresa». Trae el nombre y logo de la empresa, las
   condiciones, los precios, los tipos de visita, los portales (Tickets, Correo, Teams) y el
   dominio. No trae contraseñas ni sesiones: cada uno entra con su cuenta.
3. Sigue **Primeros pasos** en el Panel: tus datos, los portales, el
   bloqueo con PIN y la copia automática. Cada paso se marca solo al hacerlo.

## 2. Dos modos

- **Técnico** (por defecto): todo AdminOps.
- **Usuario**: una interfaz sencilla para dejar en el equipo de un cliente. Es un **modo de
  interfaz, no una barrera de seguridad**: no impide nada que Windows no impida ya.

El **modo auditoría** (barra superior) hace que AdminOps solo mire: bloquea en un único punto todo
lo que cambia el equipo. Útil para revisar un equipo ajeno sin tocar nada.

## 3. Los cinco usos de cada día

**«No me funciona X».** Inicio → *Solucionar problemas*: eliges el síntoma (no hay Internet, no
imprime, va lento…), AdminOps revisa las causas típicas en orden y ofrece la reparación. Abajo,
las reparaciones de Windows (SFC, DISM, red, Windows Update).

**Una persona llama.** Ctrl+Alt+N abre la **nota de llamada** aunque AdminOps esté minimizado:
quién, qué equipo, qué pasa. Desde ahí, «Abrir caso» o «Para mañana». Con un **caso abierto**, todo
lo que hagas en AdminOps queda apuntado y al cerrarlo se redacta la resolución para el ticket
(«Pegar en Tickets»). En *Personas* (dominio): desbloquear la cuenta, contraseña temporal, y la clave
de BitLocker o LAPS de su equipo.

**Un equipo va mal.** Equipo → *Estado del equipo*: diagnóstico con lo que hay que arreglar,
piezas y temperaturas, seguridad e historial. Equipo → *Discos*: el espacio y la **salud** de cada
disco (si falla, qué hacer; rescatar archivos de uno que no copia; pendrives falsos).

**Una visita de mantenimiento.** Inicio → *Sesión de servicio*: diagnóstico al llegar, checklist,
trabajo, diagnóstico al irse, presupuesto o recibo y firma. Al final, el **informe PDF** con el
antes y después, el estado por áreas y lo pendiente por prioridad.

**Organizar el día.** Soporte → *Agenda*: tareas, llamadas, reuniones y visitas, con o sin cliente;
vista de lista o de **mes** (arrastra para cambiar de día) e **historial**. *Hoy*, en el Panel, junta lo
urgente: caso abierto, seguimientos, visitas y avisos de Windows.

## 4. Atajos

| Atajo | Qué hace |
|---|---|
| Ctrl+K | Buscar cualquier cosa: páginas, acciones, contactos, síntomas, equipos |
| Ctrl+Alt+N | Nota de llamada (aunque AdminOps esté minimizado) |
| Ctrl+L | Bloquear AdminOps |
| / | Buscar en la página (Contactos, listas) |

## 5. Lo que conviene saber

- **Todo cambio es reversible**: cada ajuste queda en el *historial* con «Deshacer», y antes de lo
  arriesgado se crea un punto de restauración.
- **Nada sale del equipo** salvo lo que pidas: comprobar versiones nuevas, el test de velocidad,
  instalar programas y las webs que abras dentro.
- **Datos personales**: AdminOps nunca enseña rutas con el nombre de usuario; los recortes de
  pantalla tapan solos rutas, usuario y equipo antes de pegarlos en un ticket.
- **Si algo falla**: Ajustes → *Datos de AdminOps* → «Paquete de soporte». Es un .zip con un
  `resumen.txt` (versión, cuánto tarda en arrancar, últimos errores) y el registro técnico.
