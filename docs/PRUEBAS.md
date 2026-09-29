# Pruebas manuales antes de publicar una versión

Lo que los tests automáticos no cubren: que cada pantalla se vea y funcione en un equipo real.
Marca cada punto; si algo falla, anota qué hiciste y el mensaje (y crea un paquete de soporte con
Ctrl+K → «paquete de soporte»).

Hazlo dos veces: **sin administrador** (lo que requiere permisos debe explicarlo, no fallar) y **como
administrador**. Con el **portable** en un USB, repite lo marcado con 🔌 en un segundo equipo.

## General
- [ ] Arranca sin avisos raros en la campana ni en Windows Defender.
- [ ] Barra lateral: 7 secciones (Inicio, Equipo, Aplicaciones, Red, Oficina, Soporte, Datos); Oficina tiene 6 páginas.
- [ ] Barra lateral: el botón de la esquina la acopla a solo iconos y otro la devuelve; arrastrar su borde cambia el ancho y se mantiene al cerrar y abrir; doble clic en el borde lo devuelve al de Ajustes.
- [ ] Cuentas y dominio tiene las dos pestañas; Impresoras y carpetas también. Desde Ctrl+K, «Unir el equipo a un dominio» abre Cuentas en la pestaña Dominio.
- [ ] Aplicaciones: pestañas Actualizar, Windows Update, Instalar, Desinstalar y Bloatware. Ajustes de Windows tiene además la pestaña Inicio de Windows. Herramientas tiene Herramientas y Atajos. Sesión de servicio tiene Sesión e Informe. Datos del equipo tiene las cinco pestañas, con Copia de datos primero.
- [ ] Soluciones: con la lista vacía ya aparecen las de AdminOps; buscar «impresora» encuentra la suya; «Duplicar para editarla» crea una copia tuya que sí se puede cambiar y borrar; Ctrl+K encuentra las de AdminOps y abre la correcta.
- [ ] Ajustes: buscar «diagnostico» (sin tilde) lleva a General y resalta la fila; la sección Portales y correo lista los portales y permite marcar sesión privada; el zoom por defecto se aplica a un portal nuevo.
- [ ] El «?» junto al título de cada página explica qué hace.
- [ ] Ctrl+K encuentra páginas, acciones («no suena», «encender Bluetooth»), contactos, soluciones y plantillas.
- [ ] Enlaces antiguos: desde un aviso o Ctrl+K, «Windows Update», «Dispositivos», «Inventario», «Reparaciones», «Limpieza», «Privacidad», «Servicios», «Rendimiento» y «Perfiles» abren su pestaña nueva.
- [ ] Indicador de tareas (barra superior): aparece al diagnosticar o desinstalar, muestra el paso, se puede cancelar y lista las terminadas.
- [ ] Modo «Solo mirar»: con él activo, aplicar un ajuste da el mensaje de auditoría; diagnosticar y guardar un contacto sí funcionan.
- [ ] Un error del sistema se lee en español claro (prueba: desinstalar algo sin administrador).
- [ ] «Deshacer» en el aviso tras aplicar un ajuste suelto; la página del ajuste se actualiza.

## Inicio
- [ ] Panel: «Revisión completa» diagnostica en el sitio y rellena «Qué hacer ahora» con diagnóstico, avisos, actualizaciones y datos, cada uno con su botón; un «Arreglar» del Panel avisa del resultado.
- [ ] Notas de este equipo/red aparecen arriba si las hay.
- [ ] Solucionar problemas: cada síntoma comprueba y ofrece reparaciones; «Responsable» aparece si hay contactos con esa etiqueta. Abajo, las Reparaciones de Windows.
- [ ] Sesión: tipo de visita y contacto al empezar; la checklist se marca sola (limpia temporales y comprueba); al terminar, «Guardar como solución».

## Equipo
- [ ] Diagnóstico: las secciones aparecen una a una mientras analiza (las pendientes dicen «Analizando…»); la segunda vez es más rápido; «A fondo» vuelve a leerlo todo.
- [ ] Diagnóstico: tras un segundo análisis, la línea «Desde el análisis del…» cuenta nuevos y resueltos; los nuevos llevan «Nuevo».
- [ ] Diagnóstico: con poco espacio, «Limpiar temporales» funciona desde el hallazgo; «Arreglar todo lo seguro» solo aparece si hay algo seguro que arreglar.
- [ ] Ajustes → General → «Diagnosticar al abrir AdminOps»: cerrar y abrir; al entrar en Diagnóstico ya está hecho o en marcha.
- [ ] Hardware → Ficha del equipo: copiar, «Copiar para Excel» (pegar en Excel en una fila), CSV, garantía (Dell/Lenovo/HP) y «Añadir al inventario de…».
- [ ] Historial: línea de tiempo con filtros y 7/30/90 días.

## Mantener
- [ ] Ajustes de Windows: cuatro pestañas; buscar «telemetria» (sin tilde) encuentra el ajuste en Privacidad; borrar la búsqueda vuelve a las pestañas; aplicar y deshacer desde la búsqueda.

## Programas
- [ ] Actualizaciones: pestañas Programas y Windows Update; «ignorar» una actualización y verla con «(ver)».
- [ ] Desinstalar: filtros, «Ocultar componentes», CSV; desinstalar en silencio uno NSIS; lote de 2-3 programas; revisar restos (carpetas, accesos, registro) y que ninguna ruta muestre el nombre de usuario.
- [ ] Entrada huérfana → «Quitar y limpiar restos».
- [ ] Preparar equipos: crear la plantilla de ejemplo y usarla en un equipo de pruebas; la pestaña Perfiles de ajustes funciona igual que antes.

## Red
- [ ] Mi red: pestañas Red y router / Dispositivos; el portal del router no se queda superpuesto al cambiar de pestaña.
- [ ] Dispositivos: anotar función, responsable y «Vigilar» en una impresora; desenchufarla → aviso en ~2 min; enchufarla → «vuelve a responder».
- [ ] Velocidad y diagnóstico: Reparar la red (rápida) con antes/después; Estado de la Wi-Fi.

## Oficina
- [ ] Usuarios y Cuentas: cada acceso de «Más opciones en Windows» abre su herramienta; en Windows Home, los que no existen aparecen desactivados con el motivo.
- [ ] Puestos: guardar una lista, comprobar rápido y a fondo; CSV; «Conectar» por Escritorio remoto.
- [ ] Puestos en lote (en un dominio, como administrador): marcar 2 equipos → Mensaje (aparece en su pantalla) → Reiniciar (el usuario ve el aviso) → Cancelar reinicio → Actualizar directivas; cada uno con su resultado; queda en el Historial. Con un equipo apagado, explica por qué falla.
- [ ] Inventario web: añadir la web del inventario (p. ej. https://inventario.pgr.gob.do), cerrar y abrir AdminOps, comprobar que entra con la cuenta de Windows; «Datos del equipo» copia cada dato.
- [ ] Cuentas: la sesión y el equipo se describen bien; en un equipo unido a Entra ID, crear el administrador local → sacar el equipo → cerrar sesión y entrar con la cuenta local; cerrar sesión de Office; borrar una credencial guardada.

## Soporte
- [ ] Agenda: crear una visita para dentro de 35 minutos → a los 5 minutos llega el aviso de Windows; al cerrar y abrir AdminOps llega «Hoy tienes 1 visita». «Empezar» abre la sesión con el cliente elegido. Recordatorio por correo; marcar como hecha.
- [ ] Agenda: un cliente con el mantenimiento vencido aparece en «Toca mantenimiento» y en «Qué hacer ahora» del Panel; al agendarlo desaparece de la lista.
- [ ] Plantilla de informe de un cliente: formato Técnico y presentación con {cliente}; al generar el informe, sale la presentación rellenada; al enviarlo, el asunto y el mensaje son los de la plantilla y los destinatarios extra están puestos.
- [ ] Enviar el informe «Con el Correo de AdminOps»: el mensaje queda escrito en el Correo y se abre la carpeta del PDF; arrastrarlo al mensaje.
- [ ] Tickets e Inventario web: al abrir AdminOps y entrar tras unos segundos, el último portal ya está cargado. Atrás y adelante se activan según el historial. Escribir una dirección del portal navega; una de fuera se abre en el navegador. El zoom se recuerda al cerrar y abrir. Buscar, Imprimir y una descarga (Abrir y Mostrar en la carpeta, sin ver la ruta).
- [ ] Sin red o sin VPN: el portal muestra «No se pudo abrir…» con el motivo, y Reintentar funciona al volver la red.
- [ ] Desconectar la red con un portal abierto: aparece la franja «sin conexión»; al reconectar, el portal que falló se recarga solo.
- [ ] Portal lento (p. ej. limitar la red): a los 10 segundos cargando aparece «está tardando en responder… no AdminOps», con Recargar y Abrir en el navegador; desaparece al terminar de cargar.
- [ ] Correo abierto y ya cargado: Redactar abre el mensaje nuevo al instante, sin recargar Outlook. Con el correo recién abierto y aún cargando, espera y lo abre igual. Desde Contactos (con destinatario) sigue abriéndolo con el destinatario puesto.
- [ ] Portal con inicio de sesión guardado y «Rellenar»: al cerrar la sesión de la web, los campos se rellenan solos.
- [ ] Correo 🔌: «Outlook del trabajo» con la cuenta y la contraseña guardadas. Entra rellenando solo; solo pide la verificación en el móvil. Redactar abre un mensaje nuevo; un adjunto se abre en su ventana; «Cerrar sesión» y, al cerrar y abrir AdminOps, pide entrar otra vez (sesión privada). En otro equipo con el USB, la cuenta sigue guardada.
- [ ] Contactos: «Escribir un correo» abre el Correo de AdminOps con el destinatario puesto (y, sin correo configurado, el programa de Windows).
- [ ] Sin PIN en AdminOps, al guardar una contraseña de portal aparece el aviso para activarlo.
- [ ] Contactos 🔌: crear con varias etiquetas (Enter), vistas tarjetas/tabla/directorio, agrupar, filtros, búsqueda `ext:` y `#etiqueta`, guardar búsqueda, lote, duplicados, papelera, copias, importar CSV y vCard, Teams abre la app.
- [ ] Conocimiento 🔌: solución, plantilla con `{equipo}` y `{?Nombre}`, nota de este equipo y de esta red.
- [ ] Informe: «Generar entrega de hoy».
- [ ] Clientes: con dos visitas al mismo equipo, «Qué cambió entre visitas» muestra lo que cambió (verde mejor, rojo peor); al empezar otra sesión en ese equipo aparece «Desde la última visita».

## Ajustes
- [ ] Seguridad de tus datos: estado de la unidad; copia cifrada a otra unidad; restaurarla (con contraseña mala primero).
- [ ] Tipos de visita: añadir un punto automático y comprobarlo en una sesión.
- [ ] Rendimiento: tras abrir varias páginas, aparecen el arranque por partes, cada página (código, pintada, lista) y las consultas más lentas; «Copiar todo» copia el texto; en el registro técnico hay una línea «Tiempos:».
- [ ] 🔌 En el segundo equipo: contactos, conocimiento, routers (contraseña visible) y ajustes están; el historial y los diagnósticos son los de ese equipo.
