# Pruebas manuales antes de publicar una versión

Lo que los tests automáticos no cubren: que cada pantalla se vea y funcione en un equipo real.
Marca cada punto; si algo falla, anota qué hiciste y el mensaje (y crea un paquete de soporte con
Ctrl+K → «paquete de soporte»).

Hazlo dos veces: **sin administrador** (lo que requiere permisos debe explicarlo, no fallar) y **como
administrador**. Con el **portable** en un USB, repite lo marcado con 🔌 en un segundo equipo.

## General
- [ ] Arranca sin avisos raros en la campana ni en Windows Defender.
- [ ] Ninguna tira de pestañas muestra barra de desplazamiento; con muchas pestañas se desplazan con la rueda.
- [ ] Soluciones y Contactos: con muchas etiquetas se ve una fila y «Ver todas»; la elección se recuerda.
- [ ] Diagnóstico: las tarjetas sin problemas salen plegadas con «Sin problemas» y se abren al pulsar; al desmarcar «Plegar lo que está bien» se ven todas.
- [ ] Diagnóstico: con winget lento, el análisis ya no se queda esperando (las actualizaciones llegan después).
- [ ] Correo: «Redactar» sin datos abre el borrador de Outlook con su aspecto normal; desde Contactos, con destinatario, abre una ventana aparte ya rellenada.
- [ ] Portales: con el portal abierto, desconectar la VPN → error; al reconectar se recarga solo.
- [ ] Un aviso de Windows con AdminOps de fondo: al pulsarlo, AdminOps se pone delante y abre la campana.
- [ ] Soluciones: abrir «La impresora no imprime» → «Vaciar la cola de impresión ahora» funciona; en «Sin sonido», «Reiniciar el audio ahora»; los botones de herramienta abren la ventana de Windows.
- [ ] Diagnóstico: un hallazgo ofrece «Cómo se arregla» y abre la solución correcta.
- [ ] Bienvenida (borra `onboarded` en los ajustes): elegir modo usuario → la barra lateral queda en 6 páginas y no se ven clientes ni correo; con PIN puesto, volver a modo técnico desde Ajustes lo pide.
- [ ] El «?» de cada pestaña explica esa pestaña, no solo la página.
- [ ] Instalar: sin red, avisa antes de empezar; con poco espacio, también.
- [ ] Clientes con tres equipos o más: aparece «Este equipo frente al resto de la oficina».
- [ ] Estado del equipo: las cuatro pestañas funcionan; al mirar Diagnóstico, Hardware deja de leer temperaturas (se ve en Ajustes → Rendimiento: read_sensors no crece).
- [ ] Aplicaciones → Instalar: aparece «Drivers y utilidades del fabricante»; instalar una de esa lista funciona.
- [ ] Espacio en disco: «Qué puedes liberar» muestra tamaños reales; la pestaña «Sin usar» lista archivos de más de un año; marcar dos y «A la papelera» los envía y aparecen en la papelera de Windows.
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
- [ ] Teams 🔌: Soporte → Teams → «Teams del trabajo»; entra rellenando la cuenta guardada y solo pide la verificación del móvil. Un chat se abre, una reunión pide micrófono y cámara la primera vez, un archivo compartido se abre en su ventana. «Cerrar sesión» y, al cerrar y abrir AdminOps, vuelve a pedir entrar (sesión privada). Un enlace de fuera de Teams (p. ej. una noticia que te pasan por el chat) se abre en el navegador, no dentro.
- [ ] Contactos: «Teams» con Teams configurado abre el chat dentro de AdminOps, en la página Teams, con esa persona; sin Teams configurado abre la aplicación de Teams del equipo, como antes.
- [ ] Arrastrar el borde de la ventana con un portal abierto: la vista sigue al área sin tirones y la interfaz responde mientras se arrastra.
- [ ] Sin PIN en AdminOps, al guardar una contraseña de portal aparece el aviso para activarlo.
- [ ] Contactos 🔌: crear con varias etiquetas (Enter), vistas tarjetas/tabla/directorio, agrupar, filtros, búsqueda `ext:` y `#etiqueta`, guardar búsqueda, lote, duplicados, papelera, copias, importar CSV y vCard, Teams abre la app.
- [ ] Conocimiento 🔌: solución, plantilla con `{equipo}` y `{?Nombre}`, nota de este equipo y de esta red.
- [ ] Informe: «Generar entrega de hoy».
- [ ] Clientes: con dos visitas al mismo equipo, «Qué cambió entre visitas» muestra lo que cambió (verde mejor, rojo peor); al empezar otra sesión en ese equipo aparece «Desde la última visita».

## Personas, el caso de ahora y Ctrl+K (en el dominio de la empresa)
- [ ] 🔌 Personas: buscar por nombre, por usuario y por extensión; aparece la persona con su departamento y extensión.
- [ ] 🔌 Una cuenta bloqueada sale como «Cuenta bloqueada»; «Desbloquear» la desbloquea y la ficha pasa a «Cuenta en orden».
- [ ] 🔌 «Restablecer contraseña»: sale una temporal tipo `Norte-Pino-4827!`; con ella se entra y pide cambiarla. En el diario aparece que se cambió, sin la contraseña.
- [ ] 🔌 Sin permisos de soporte sobre una cuenta: el mensaje dice que el dominio no deja, no un error en inglés.
- [ ] 🔌 Tras comprobar Puestos a fondo, la ficha dice en qué equipo tiene la sesión abierta.
- [ ] 🔌 LAPS de un equipo: «Mostrar» enseña la contraseña y se tapa sola al minuto. Sin LAPS o sin permiso, lo dice.
- [ ] 🔌 BitLocker: las claves de un equipo, y la clave buscando por el ID de 8 caracteres de la pantalla de recuperación.
- [ ] En un equipo fuera del dominio (o sin VPN), Personas lo explica en vez de fallar.
- [ ] Caso: «Nuevo caso» en la cabecera; vaciar una cola de impresión y reparar la red; la barra cuenta 2 acciones; «Cerrar caso» redacta las dos, en orden, con el tiempo.
- [ ] Caso: en el portal de Tickets, pulsar en el campo de la resolución, volver a AdminOps, «Cerrar caso» → «Pegar en Tickets»: el texto aparece en ese campo. Sin campo seleccionado, avisa de que lo ha copiado.
- [ ] Caso: abrir otro con uno abierto avisa y ofrece cerrar el actual. «Descartar» lo quita sin guardarlo.
- [ ] Caso cerrado desde la ficha de una persona: aparece en «Lo que ya se hizo con esta persona».
- [ ] Ctrl+K: `PC-CONTA-03` ofrece contraseñas, conectar y abrir caso arriba del todo; `windows update` sigue ofreciendo primero la página de Windows Update.
- [ ] Modo usuario: no se ve ni Personas, ni la barra del caso, ni «Nuevo caso».

## Pantalla dividida, nota de llamada, seguimientos y Hoy
- [ ] Tickets → «Al lado: Personas»: el portal ocupa la izquierda y Personas la derecha, las dos se usan a la vez; la vista web no tapa la mitad derecha. Personas se ve en una columna, sin apretar.
- [ ] Cambiar a otra página y volver a Tickets: la división sigue. «Sin dividir» la quita. Cerrar y abrir AdminOps: se recuerda.
- [ ] Con la ventana estrecha (1280 px), las dos mitades siguen siendo legibles.
- [ ] Con AdminOps minimizado, Ctrl+Alt+N abre la nota encima de todo. «Abrir caso» → la barra del caso aparece en AdminOps sin recargar. Esc la cierra.
- [ ] Nota → «Para mañana»: aparece mañana a las 9 en «Hoy», con aviso de Windows (una sola vez).
- [ ] Si otro programa ya usa Ctrl+Alt+N, el registro lo dice y «Nota de llamada» sigue funcionando desde Ctrl+K.
- [ ] Barra del caso → recorte: sale el recorte de Windows; lo recortado se pega en el ticket con Ctrl+V.
- [ ] Hoy: con un caso abierto, un seguimiento vencido, una visita de hoy y un aviso de Windows, salen los cuatro, el vencido primero. «Hecho» y «Mañana» funcionan.
- [ ] Modo usuario: el Panel no enseña «Hoy».

## Impresoras de red (SNMP, mDNS, WS-Discovery)
- [ ] Revisar una impresora de red con SNMP: salen los niveles de tóner con barras y el contador de páginas.
- [ ] Con el tóner por debajo del 10 %: el título dice «Tóner … al N %» en ámbar.
- [ ] Abrir la tapa o provocar un atasco y Revisar: «La impresora avisa: puerta o tapa abierta».
- [ ] Una impresora sin SNMP: Revisar funciona igual que antes, sin niveles (tarda unos segundos más como mucho).
- [ ] Buscar en la red: las impresoras salen con nombre y modelo; una que este equipo no había visto nunca también aparece.
- [ ] En una red sin impresoras: «No se ha visto ninguna», sin colgarse.

## Guía, novedades, términos y reportar fallos (1.1.10)
- [ ] Acerca de (pulsando el logo): cuatro botones. «Guía» abre la ayuda con los capítulos a la izquierda; «Abrir esta pantalla» cierra la ayuda y lleva a esa pantalla y pestaña.
- [ ] Buscar «deshacer», «pendrive», «pin»: salen apartados de la guía, preguntas y glosario. Sin tildes también encuentra.
- [ ] Con Tickets o el Correo abiertos, la ayuda se ve entera: el portal no la tapa. Esc la cierra.
- [ ] Novedades: la primera es la versión instalada, marcada «la que tienes»; baja hasta la 0.1 y no hay ninguna fecha.
- [ ] Términos de uso: se leen completos y son los mismos del instalador.
- [ ] Reportar un problema: con menos de 10 letras el botón está apagado. «Preparar el correo» abre el programa de correo con el destinatario, el asunto, el texto y el .zip adjunto; sin enviar, no sale nada. «Sin adjunto» abre el correo con el texto y la carpeta del .zip. El .zip trae `descripcion.txt`.
- [ ] En un equipo sin programa de correo: el aviso verde dice qué hacer.
- [ ] Instalador: «Instalar» está apagado hasta marcar «He leído y acepto»; «términos de uso» los abre dentro del instalador, y «Acepto» marca la casilla. El instalador clásico enseña la página de la licencia; con `/S` instala sin preguntar.
- [ ] Ya no hay nada de Microsoft 365: ni en Ajustes → Portales y correo, ni en Personas, ni el icono de Outlook en la Agenda, ni puntos de presencia en Contactos, ni «Avisar por Teams» al cerrar un caso. El Correo y Teams siguen abriendo.
- [ ] Una agenda guardada con visitas que estaban en Outlook sigue abriendo y editándose con normalidad.
- [ ] Importar un archivo de empresa antiguo (con Microsoft 365 dentro): importa lo demás sin error.

## Lavado de cara (1.1.9)
- [ ] Usuarios locales: las cuatro cifras cuadran; pulsar «Con algo que revisar» filtra. Una cuenta sin contraseña sale con el punto de color y, en la ficha, el aviso con «Cambiar contraseña». Buscar por nombre. Los botones apagados dicen por qué al pasar el ratón (único administrador, sesión iniciada…). Al borrar a alguien, la ficha pasa a otra cuenta.
- [ ] Procesos: agrupado, Chrome/Edge salen una vez con «× N»; la flecha enseña sus procesos. «Finalizar el programa» los cierra todos. Los dos avisos de arriba (CPU y memoria) seleccionan el programa. Quitar «Agrupar por programa» vuelve a la lista de siempre y se recuerda. Ordenar por cada columna.
- [ ] Acceso remoto: escribir un equipo y Enter conecta. Las fichas guardadas enseñan «Contesta · N ms» o «No contesta» con el motivo; el botón de recargar vuelve a comprobar. Con más de 6, aparece el buscador.
- [ ] Sesión de servicio: empieza en «Motivo»; con el motivo escrito, al volver abre en «Trabajo». Cada paso enseña lo que lleva (3/8, «Apuntado», «Sin cobro»…). «Firma y cierre» avisa de lo que falta y genera el informe. Descartar sigue pidiendo confirmación.
- [ ] Red: una sola entrada en la barra lateral con cuatro pestañas. Ctrl+K «Herramientas de red» y «Reparar la red» llevan a su pestaña. El panel del router se oculta al cambiar de pestaña.
- [ ] Historial: el diario sale por días con la línea a la izquierda; los filtros y el buscador funcionan; «Deshacer» sigue funcionando y la entrada queda tachada.
- [ ] Tablas: en Dispositivos, ordenar por IP (192.168.1.3 antes que 192.168.1.20) y por Ping (los que no responden, al final). En Estaciones, por «Sin actualizar». En Puertos, por programa.
- [ ] Ajustes: buscar «inactividad», «tipos de visita», «modo», «favoritos» encuentra y lleva; un bloque entero queda con el borde marcado unos segundos.
- [ ] Con el modo auditoría o sin red, las páginas que no pueden leer enseñan el recuadro rojo con «Reintentar», no un «Leyendo…» eterno.
- [ ] Compartir una carpeta con muchos archivos: no tarda minutos, y en un archivo de dentro el permiso sale como heredado (Propiedades → Seguridad → Opciones avanzadas).

## Tickets: inicio de sesión fuera del dominio
- [ ] Intranet cuyo inicio de sesión está en otro sitio (otro subdominio, IP o proveedor externo): poner usuario y contraseña entra, sin que se abra el navegador de fuera.
- [ ] Un enlace dentro de un ticket a una web ajena: se abre en el navegador de fuera y aparece la barra «…se abrió en tu navegador». «Permitir en este portal» la quita; al pulsar otra vez el enlace, abre dentro. El sitio queda en «Editar portal» → dominios.
- [ ] Correo y Teams siguen entrando igual que antes.
- [ ] En el registro técnico aparecen «se sigue a «sitio» (redirección del servidor…)» y «no es del portal y se abrió en el navegador».

## Carpetas compartidas a fondo
- [ ] Cada carpeta enseña su ruta `\\EQUIPO\Nombre`, su tamaño (primero «calculando tamaño…») y sus permisos. Copiar la ruta y copiar las instrucciones dejan el texto en el portapapeles.
- [ ] «Quién puede entrar»: cambiar a alguien de «Solo leer» a «Leer y modificar» se ve en la lista al momento y en Propiedades → Compartir de Windows. Añadir un usuario y quitarlo. Sin administrador, todo desactivado con su explicación.
- [ ] «¿Por qué no puede entrar?»: un usuario que no está en la lista → «no puede entrar» con «Darle acceso», que abre «Quién puede entrar» con él propuesto. Tras dárselo, sale que puede. Con una cuenta inventada: dice que no existe. Con la red en pública: lo dice y ofrece «Activar».
- [ ] Una carpeta compartida con «Todos: control total» sale marcada, con «Bajarlo a leer y modificar». Un disco entero o el Escritorio de alguien salen marcados. Arriba se cuenta cuántas cosas hay que revisar.
- [ ] Disco con menos del 10 % libre: aviso en la carpeta.
- [ ] Unidades de red: salen las que tiene el usuario (también con AdminOps elevado). Con el servidor apagado: «no contesta», y «Reconectar» lo explica. Conectar `\\EQUIPO\Carpeta` en una letra libre: aparece en «Este equipo» del Explorador. Quitarla: desaparece. Una ruta mal escrita o un equipo que no existe: mensaje claro.
- [ ] Un equipo que pide usuario y contraseña: el mensaje lo dice y «Abrir» lleva al Explorador, que es quien los pide.
- [ ] «Qué comparte otro equipo»: con el nombre de otro PC salen sus carpetas e impresoras; «Conectar como unidad» usa la primera letra libre y la unidad aparece arriba. Recuerda el último equipo escrito.
- [ ] Copia diaria: elegir un destino en otro disco y una hora; «Copiar ahora» crea `<destino>\<nombre>` con los archivos y un `.log` al lado. La carpeta enseña «Copia diaria a las HH:MM · última: …». Destino en el mismo disco: aviso. Con el disco de destino desconectado: «La última copia falló». «Quitar la copia» la quita del Programador de tareas (`\AdminOps\Copias`) y deja lo copiado.
- [ ] Modo auditoría: cambiar permisos, conectar o quitar unidades y programar copias se bloquean; ver, explicar y mirar otro equipo siguen funcionando.

## Historial de la Agenda, Personas y Clientes
- [ ] Agenda → Historial: salen lo hecho, lo cancelado y lo sin marcar, agrupado por mes con su total. Buscar y los filtros funcionan. «Volver a pendiente» la devuelve a la lista; «Repetir» abre el editor con una copia para mañana; borrar pide confirmación.
- [ ] Una visita de ayer sin marcar: sale arriba de la lista en «Atrasado» con su fecha; el botón del amanecer la pasa a hoy.
- [ ] Al cerrar y abrir AdminOps estando en Historial, la Agenda abre en Lista (o Mes, lo último usado).
- [ ] Soporte → «Personas y clientes»: dos pestañas. Ctrl+K «Clientes» abre la pestaña Clientes; Ctrl+K con un nombre abre Personas buscándolo.
- [ ] Clientes: la cifra «Mantenimiento cerca o vencido» filtra la lista. La ficha abre en Resumen; Equipos, Visitas y Datos enseñan lo suyo; «Nuevo» abre directamente en Datos. «Agendar» abre la Agenda con el editor y ese cliente puesto.
- [ ] Personas: tras abrir a alguien, aparece en «Recientes» al volver; «Olvidar» los quita. Una cuenta bloqueada enseña el aviso rojo con «Desbloquear»; una con la contraseña caducada, «Dar una contraseña temporal».
- [ ] Tickets con «Al lado: Personas» (pantalla dividida): se ve bien en media pantalla.

## Listo para el equipo y Agenda de mes
- [ ] Panel con datos vacíos: sale «Primeros pasos» con 5 pasos; cada botón lleva a su sitio; al hacerlo, la casilla se marca al volver a la ventana. La X lo oculta; Ajustes → General «Ver Primeros pasos» lo devuelve. En modo usuario no sale.
- [ ] Ajustes → Configuración de empresa → Exportar: el .json no contiene contraseñas, ni el nombre ni la firma del técnico. En otro equipo (o tras borrar un portal), Importar enseña qué trae, deja elegir y no duplica portales.
- [ ] Paquete de soporte: el .zip trae `resumen.txt` con los arranques y los últimos errores legibles.
- [ ] Agenda → Mes: arrastrar una visita a otro día la mueve (y el aviso lo dice); doble clic en un día abre «Apuntar» con esa fecha; «y N más» abre ese día en la lista.

## Pendrive seguro, discos a fondo e informe (1.1.8)
- [ ] Con AdminOps abierto (y Correo cargado), pasar el Setup encima: al terminar, AdminOps es el nuevo (Ajustes → Acerca de, o el registro dice «navegador» en el disco del equipo). Si algo lo impide, el instalador lo dice en vez de «listo».
- [ ] Cerrar AdminOps desde el Administrador de tareas con Correo abierto y volver a abrirlo: Correo y Teams cargan; el registro dice «Cerrados N procesos del navegador interno…».
- [ ] AdminOps instalado en el pendrive: arranca en pocos segundos; Correo y Teams cargan como en el disco (tras entrar una vez en este PC).
- [ ] El Panel enseña las temperaturas en unos segundos (no 17 s).
- [ ] Ajustes → Datos → «Guardar también el navegador interno en el pendrive»: al volver a abrir, pide entrar de nuevo en Correo (perfil en el pendrive); desmarcarlo vuelve al del disco.
- [ ] En el pendrive con bloqueo por PIN: al abrir, desbloquear con el PIN → los portales con «Entrar solo» y los routers siguen funcionando. Ajustes → Datos dice «Protegida con tu PIN».
- [ ] Abrir con la contraseña de Windows (PIN «olvidado»): AdminOps abre, pero una contraseña guardada dice que está protegida con el PIN.
- [ ] Cambiar el PIN y reiniciar: el PIN nuevo abre las contraseñas. Quitar el bloqueo: siguen funcionando sin PIN.
- [ ] Discos: la tarjeta dice «Desde hoy se apunta una foto al día…». Al día siguiente sale la evolución.
- [ ] «Medir velocidad» en un SSD, un disco mecánico y un pendrive: cifras razonables y el texto adecuado (USB 2.0, «cámbialo por un SSD»…). No queda el archivo de prueba.
- [ ] «¿Capacidad real?» en un pendrive pequeño: escribe, comprueba, dice «capacidad real» y la carpeta de prueba desaparece. Cancelar a mitad también la borra.
- [ ] «Expulsar» un pendrive con una ventana del Explorador abierta en él: lo expulsa o dice qué lo tiene abierto.
- [ ] «Formatear»: sin escribir la letra no deja; FAT32 desactivado en uno de más de 32 GB; formatea como exFAT con el nombre puesto.
- [ ] En el disco de Windows no salen expulsar ni formatear, ni en el pendrive desde el que corre AdminOps.
- [ ] Un volumen con BitLocker: chip «BitLocker activo» y «clave» enseña la clave de recuperación; el diario lo anota.
- [ ] Ajustes → Copia automática: «Usar OneDrive», contraseña, Activar, «Hacer una ahora» → aparece el .zip en OneDrive\AdminOps copias; con 6 copias y «las 5 últimas», queda en 5.
- [ ] En otro PC, la copia automática dice dónde se configuró y no se hace sola allí.
- [ ] Pendrive sin copia desde hace más de 15 días: aviso en la campana.
- [ ] Informe al cliente: la letra es la misma en otro equipo sin IBM Plex instalada; «Estado por áreas» con colores; los pendientes urgentes primero; en la página 2 arriba a la derecha salen el equipo y la fecha.
- [ ] Informe técnico: la tabla de discos dice «Sano» o el veredicto, sin «Unspecified» ni desgaste en discos mecánicos.

## AdminOps en el pendrive (1.1.7)
- [ ] En el PC donde ya estaba instalado: instalar la 1.1.7 en el pendrive. Al abrirla, Ajustes → Datos dice «está en un pendrive (E:)» y «Datos traídos del equipo …»: clientes, contactos, agenda y portales están.
- [ ] Un router o portal con contraseña guardada: funciona desde el pendrive en OTRO equipo (se cifró de nuevo con la clave del pendrive).
- [ ] En ese mismo PC, Correo y Teams siguen con la sesión iniciada (se trajo su sesión).
- [ ] En otro PC: hay que entrar una vez en Correo y Teams («Entrar solo» rellena la cuenta). Volver al primero: sigue con su sesión. Volver al segundo: también.
- [ ] Actualizar: pasar el instalador de una versión nueva sobre la carpeta del pendrive → `AdminOps-data` sigue ahí con todo.
- [ ] En un PC sin AdminOps registrado, abrir el instalador desde el pendrive: propone «Actualizar AdminOps del pendrive» con la carpeta del pendrive.
- [ ] Instalado en C:\Archivos de programa sin administrador: el botón «Guardar todo en la carpeta del programa» está desactivado y lo explica.
- [ ] Un pendrive que ya tenía datos (del portable antiguo) no se sobrescribe con los de este equipo.

## Discos: salud, reparación y rescate (1.1.7)
- [ ] La página se llama «Discos», con las pestañas Espacio y Salud y reparación. Ctrl+K y los enlaces que llevaban a «Espacio» siguen funcionando.
- [ ] Sin administrador: aviso de que faltan SMART y temperaturas; los botones de reparar desactivados; Rescatar funciona.
- [ ] Como administrador: cada disco con su veredicto; los externos salen como «Externo (USB)».
- [ ] Un disco externo quitado sin expulsar: sale «marcado como dañado» y el veredicto dice que es el sistema de archivos. «Reparar sistema de archivos» lo arregla y la marca desaparece al actualizar.
- [ ] En C: «Reparar sistema de archivos» dice que se hará al reiniciar; al reiniciar, Windows lo revisa antes de arrancar.
- [ ] «Buscar sectores dañados» en un pendrive: pide confirmación, enseña el porcentaje, se puede cancelar y la unidad vuelve a estar disponible.
- [ ] Un disco con sectores pendientes (si hay uno a mano): veredicto «Fallando», primer consejo «Rescatar archivos»; «Buscar sectores dañados» avisa en rojo.
- [ ] Rescatar de una carpeta del disco externo a otro disco: copia, dice cuántos archivos hay en el destino y lista los que no se pudieron leer (si los hay). Repetirlo no vuelve a copiar lo ya copiado.
- [ ] Rescatar con destino dentro del origen: lo impide con un mensaje claro.
- [ ] Modo auditoría: reparar, chkdsk /r y rescatar se bloquean; ver y comprobar funcionan.

## Agenda sin clientes, Contactos, recortes, avisos y Visor de eventos
- [ ] Sin ningún cliente creado: la Agenda deja apuntar. «Llamar a Contabilidad» + Mañana + Enter → aparece en «Mañana» a las 9:00 y en la semana con un punto.
- [ ] Apuntar algo para hoy sin hora: queda a la siguiente hora en punto. Con hora: a esa hora.
- [ ] Nueva → tipo Reunión, sin cliente, con título: se guarda; «Empezar» no sale (no hay cliente). Con cliente: sale.
- [ ] Una visita antigua (de antes de esta versión) sigue saliendo como visita, con su cliente.
- [ ] Pulsar un día de la semana: solo sale ese día; «Ver todo» lo quita. «Añadir» en un día abre el editor con esa fecha.
- [ ] Un seguimiento de la nota de llamada para mañana sale en «Mañana» y en su día, con «Hecho» y «Mañana».
- [ ] «Hoy» dice «En N min» con lo próximo y tacha lo que ya pasó.
- [ ] Aviso 30 minutos antes: «Llamada en menos de 30 minutos» con el título.
- [ ] Contactos: las seis cifras filtran al pulsarlas (y «Contactos» quita los filtros). «Sin completar» en ámbar si hay alguno.
- [ ] Marcación rápida: favoritos primero; al pasar el ratón, llamar, Teams, correo y copiar funcionan.
- [ ] Las tarjetas, la tabla, el directorio y la ficha muestran el avatar del mismo color para la misma persona, en claro y en oscuro.
- [ ] Ficha: la extensión en grande se copia al pulsarla; los cuatro botones funcionan y se desactivan si falta el dato.
- [ ] En ninguna parte de la app sale «Asistencia rápida»; Acceso remoto → «Abrir Asistencia remota» abre la de Windows.
- [ ] Recorte desde la barra del caso sobre una ventana con `C:\Users\<tu usuario>\…` visible: al pegarlo, la ruta sale tapada y AdminOps dice cuántos datos tapó.
- [ ] Recorte de algo sin datos personales: «sin datos personales a la vista»; la imagen se pega igual.
- [ ] Captura hecha con Win+Mayús+S fuera de AdminOps → Ctrl+K «Tapar datos personales del portapapeles»: se tapan.
- [ ] Con texto (no imagen) en el portapapeles, «Tapar datos…» dice que no hay imagen y no toca nada.
- [ ] Aviso de un seguimiento con AdminOps minimizado: tiene «Hecho» y «Mañana»; «Hecho» lo marca (sale en «Hoy» sin recargar); «Mañana» lo aplaza.
- [ ] El registro dice «Vigilancia de Windows: en tiempo real». Provocar un error de aplicación (cerrar un programa que falle): el aviso llega en segundos, no al minuto.
- [ ] Con el equipo en reposo, AdminOps no lanza PowerShell cada minuto para vigilar (Administrador de tareas).

## Agenda, Usuarios, Impresoras y Carpetas
- [ ] Agenda: crear una visita «cada mes» y marcarla como hecha → aparece la siguiente un mes después, con el mismo sitio, tipo y duración.
- [ ] Agenda: crear dos visitas que se solapen → al guardar la segunda avisa con el nombre del otro cliente, pero la guarda igual.
- [ ] Agenda: el botón de aplazar mueve la visita un día sin abrir el editor, y vuelve a avisar a los 30 minutos de la nueva hora.
- [ ] Usuarios: renombrar una cuenta sin sesión iniciada; si tiene perfil, avisa de que la carpeta personal conserva el nombre viejo. Con la sesión abierta, el botón está desactivado y dice por qué.
- [ ] Usuarios: editar solo el nombre completo y la descripción (sin cambiar el nombre de cuenta) también funciona.
- [ ] Impresoras → «Revisar» con la impresora apagada: dice que no responde y en qué dirección. Encendida pero marcada sin conexión en Windows: lo distingue.
- [ ] Impresoras → «Buscar impresoras en la red» en la oficina: encuentra las que hay y marca las que ya están instaladas.
- [ ] Carpetas compartidas: abrir un archivo de una carpeta compartida desde otro equipo → aparece en «Archivos abiertos ahora mismo» con quién lo tiene.
- [ ] Carpetas compartidas: borrar la carpeta de un recurso compartido → aparece «La carpeta ya no existe».

## Portales (sesión y velocidad)
- [ ] **MFA**: con Teams como última página usada, abrir AdminOps, esperar un par de minutos (se precarga) y entonces abrir Teams e iniciar sesión: el código llega y se acepta. Es lo que se rompió en la 1.1.5.
- [ ] **Con un portal de intranet configurado**, abrir Correo, Teams y Tickets: los tres cargan. Es lo que se rompió en la 1.1.6.
- [ ] Si un portal no llega a abrirse, a los 25 s aparece el aviso con Reintentar en vez de «Abriendo…» para siempre.
- [ ] Abrir Correo, Teams y Tickets, y luego ir a Ajustes y a otras páginas: todo se puede pulsar y desplazar. (En la 1.1.6 una vista que no arrancó se quedaba invisible encima tapándolo todo.)
- [ ] Abrir AdminOps: empieza en el Panel. Elegir otra página en Ajustes → General → «Página al abrir AdminOps», cerrar y abrir: respeta la elegida.
- [ ] 🔌 **En la red del trabajo**, con la intranet configurada como portal de Tickets: entra y carga. Es el caso que fallaba (argumentos de WebView2 distintos entre la ventana y el portal).
- [ ] Configurar el Correo, iniciar sesión con MFA, cerrar AdminOps y volver a abrir: **no debe pedir la sesión otra vez**. Igual con Teams.
- [ ] Marcar «sesión privada» en el Correo, cerrar y abrir: ahí sí debe pedirla (y la pestaña lo avisa al pasar el ratón).
- [ ] Con el Correo cargado, cambiar de página y volver antes de 3 minutos: vuelve al instante, sin recargar.
- [ ] Dejar el Correo abierto una hora sin entrar y volver: la vista sigue viva (antes se cerraba a la media hora y recargaba entero).
- [ ] Teams: primera carga lenta (es Teams), pero la segunda vez que se abre en el mismo equipo es notablemente más rápida.

## Rendimiento en equipos lentos
- [ ] En un equipo de 4 GB o menos: abrir AdminOps y mirar el Administrador de tareas. No debe precargar ningún portal, y el conjunto no debería pasar de un par de procesos de PowerShell.
- [ ] En ese mismo equipo, abrir y cerrar la app tres veces seguidas: entra siempre a la primera, sin quedarse en negro ni recargarse sola.
- [ ] Dejar el Panel abierto un minuto: el uso de CPU de AdminOps se mantiene bajo (la lista de procesos ya solo se relee cada 6 s).
- [ ] En el registro técnico aparece «N núcleos · ~N GB» al arrancar.

## Texto y espacio
- [ ] En una página con pestañas (Impresoras, Actualizaciones, Mi red…), pasar el ratón por el «?» de la derecha: sale el texto entero, dentro de la ventana, sin recortar. Con clic se queda abierto; clic fuera lo cierra.
- [ ] En una tabla con nombres largos (Procesos, Aplicaciones, Contactos), pasar el ratón por un texto cortado con «…»: se ve entero en un globo. Donde no está cortado, no aparece globo.
- [ ] Achicar la ventana al mínimo y volver a agrandarla en Herramientas de red, Espacio, Historial e Informe: las cajas con scroll se ajustan y no queda contenido metido en una rendija.

## Reparar la red
- [ ] Con todo bien: la reparación termina en verde y dice IP, router y DNS.
- [ ] Desenchufando el cable de red: dice que no hay tarjeta conectada o que el router no da dirección, no «no hay Internet».
- [ ] Poniendo un DNS que no existe (p. ej. 10.0.0.9) a mano: dice que hay Internet pero no resuelve nombres, enseña ese DNS y ofrece «Cambiar los DNS».
- [ ] Con un proxy puesto en Windows: lo detecta y lo dice aunque todo lo demás funcione.
- [ ] Los botones del diagnóstico llevan a la página correcta (router, herramientas de red, velocidad).

## Ajustes
- [ ] Usuarios → detalle de un usuario: pulsar rápido entre dos usuarios con perfiles grandes; el tamaño que se acaba viendo es el del usuario seleccionado, no el del anterior.
- [ ] Datos del equipo → Migrar: cambiar de usuario antes de que termine el cálculo; la lista que queda es la del usuario elegido.
- [ ] Portales: abrir el editor de un portal, cerrarlo y abrir el de otro enseguida; la cuenta guardada que se ve es la del portal abierto.
- [ ] Descargar varios archivos desde un portal: «Abrir» y «Mostrar en la carpeta» siguen apuntando al archivo correcto después de 20 descargas.
- [ ] Seguridad de tus datos: estado de la unidad; copia cifrada a otra unidad; restaurarla (con contraseña mala primero).
- [ ] Tipos de visita: añadir un punto automático y comprobarlo en una sesión.
- [ ] Arranque: al abrir AdminOps, la ventana aparece ya dibujada (o con «Abriendo AdminOps…»), nunca en negro. En el registro técnico hay una línea «Ventana visible a los N ms».
- [ ] Portable, primera vez en un equipo ajeno 🔌: con el USB recién preparado, abrir AdminOps: debe entrar a la primera, sin cerrar y volver a abrir. Comprobar que se creó la carpeta `webview` en el USB. Con el USB protegido contra escritura, la app abre igual (usa la carpeta del equipo) y lo dice en el registro.
- [ ] Ajustes → General → «Volver a ver la bienvenida»: abre el asistente; al terminarlo, la configuración que ya había sigue como estaba.
- [ ] Rendimiento: tras abrir varias páginas, aparecen el arranque por partes, cada página (código, pintada, lista) y las consultas más lentas; «Copiar todo» copia el texto; en el registro técnico hay una línea «Tiempos:».
- [ ] 🔌 En el segundo equipo: contactos, conocimiento, routers (contraseña visible) y ajustes están; el historial y los diagnósticos son los de ese equipo.
