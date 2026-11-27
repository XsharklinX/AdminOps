# Revisión de Opus · tanda 1.5.1 (27 nov)

Dos revisores de Opus. Se verifica cada hallazgo antes de arreglarlo. Tras arreglar, marcar ✔ aquí.

## A · Tanda sin commit (completa) — NO APRUEBA: 7 medios

### Medios (arreglar antes de la build)
1. ✔ (toolsRef) **Personalizar enseña la lista de fábrica**: `RocketBar.tsx:~279`, el manejador `custom` se crea al montar y cierra sobre `tools` inicial (11). Un usuario sin admin (7) o personalizado ve 11 marcadas y al guardar se le cambia la barra. Arreglo: leer `tools` por ref.
2. ✔ (tonight_due: más de 1 h tarde → se cancela y se apunta en el log) **Reinicio «esta noche» tras suspensión**: `bartools.rs:~188` solo mira `now >= at`. Portátil suspendido de 18:00 a 8:30 → reinicia por la mañana. Arreglo: si pasó más de 1–2 h de la hora, cancelar (y avisar).
3. ✔ (sin admin: sin flecha de actualizar; «Ver todas las notas» abre «Mis notas» en la barra; open_full sin elevar va por el Shell con UAC en vez de error 740; con admin abre Conocimiento → Notas) **Sin administrador piden credenciales**: (a) Notas → «Ver todas en AdminOps» (`RocketBar.tsx:~322` → `open_full` `rocketplus.rs:~622`) lanza el exe con manifiesto admin → os error 740 en inglés; tampoco abre la pestaña Notas; el usuario normal no ve su cuaderno. (b) Flecha de actualizar (`RocketBar.tsx:~454`): instalador perMachine. Arreglo: con `isAdmin` falso, esconder flecha y «Ver todas» o dar otra salida a las notas.
4. ✔ (solo cierra al guardar desde el editor; abrir otra o cancelar con cambios pregunta) **Notas pierde lo escrito**: `Notebook.tsx:35` y `:85`: guardar siempre cierra el editor; tocar otra tarjeta o el alfiler de otra nota sustituye sin preguntar. Arreglo: cerrar solo si es la nota abierta; avisar si hay cambios.
5. ✔ (radio_result en bartools.rs: error si no queda como se pidió) **Wi-Fi/Bluetooth dice «encendida» aunque no cambie**: `BarExtra.tsx:264/274` ignoran lo que devuelve `bar_set_radio`. Arreglo: comparar con lo pedido.
6. ✔ (morning_window por uptime del equipo, 90 s–3 h; espera sin marcar con llamada o pantalla completa; saludo según la hora) **«Empezar el día ligero» en plena llamada**: `rocketplus.rs:~445` no mira `fullscreen_now() || media_in_use()` como el automático; además cuenta desde que abre AdminOps («Buenos días» a las 15:00). Arreglo: misma condición, reintentar sin marcar el día; contar desde el arranque del equipo.
7. ✔ (hilo con mensajes MARK_ON/MARK_OFF: se coge al empezar y se suelta en finish) **Ctrl+Alt+M ocupado para siempre**: `faultrec.rs:~1408` nunca libera el atajo (Word lo usa). Arreglo: registrar al empezar y liberar en `finish`.

### Leves (arreglarlos también)
8. ✔ (píldora enseña el error 5 s; Personalizar enseña el fallo; «Ver mediciones anteriores» solo la lista) Errores invisibles: `RecPill.tsx:46-52` (dibujar/cámara al grabar ventana, solo log); `RocketBar.tsx:~787` fallo al guardar Personalizar; `RocketBar.tsx:~917` «Ver mediciones anteriores» enseña aguja a 0 con «Midiendo el ping…».
9. ✔ («Preparando…» y botones desactivados; GIF de los últimos 8 s) «Para enviar»/GIF en la barra (`RocketBar.tsx:~727`) sin «Preparando…» ni botón desactivado; el GIF siempre saca los primeros 8 s.
10. ✔ (segundo toque «Sí, reiniciar ya» / «No») «Reiniciar ahora» sin confirmar (`BarExtra.tsx:~431`).
11. ✔ (public_ip en vivo al abrir Datos del equipo; si falla, no sale) «IP pública» de Datos del equipo (`RocketBar.tsx:~387`) es la de la última prueba de velocidad (puede ser vieja/otra red).
12. ✔ (title; al caso y a «para mañana» va delante del texto) `QuickNote.tsx:37`: el título se manda como `person` al caso / para mañana; `machine` muerto (línea 17).
13. ✔ (también el menú del reloj; el changelog viejo se deja) Queda «Nota de llamada»: `App.tsx:761`, `TodayCard.tsx:201`, `tips.ts:13`, `Agenda.tsx:384`, `settings/General.tsx:88`, título ventana `quicknote.rs:26`.

### Posibles (comprobar)
Quedan para la prueba en el exe: mezcla voz + sistema, permiso de cámara en WebView2, Mis carpetas con unidad de red, DisplaySwitch en 24H2.
- ✔ Outlook (`rocketbar.rs:33`): ahora mira protocol_registered (outlook, ms-outlook, mailto) y si no hay, error claro: si `outlook:` no está, ShellExecute puede dar por bueno el aviso de Windows; mirar antes con `protocol_registered`.
- Mezcla voz + sistema (`capture.rs`, rama `job.sys`): microcortes/voz retrasada si el micro llega tarde. Oírlo en real.
- ✔ Carrera dibujar/cámara: tras crear la ventana se mira ACTIVE y se cierra si ya terminó justo al pararse sola la grabación: la ventana queda abierta (Esc la quita). Comprobar ACTIVE tras crearla.
- Cámara: WebView2 puede pedir permiso dentro de la ventanita pequeña.
- Mis carpetas con AdminOps elevado: unidad de red elegida no visible.
- DisplaySwitch.exe en Win11 24H2: probar que acepta /extend etc.
- ✔ notes.json dañado: load_for_write lo aparta a notas-dañado-….json antes de guardar; cifrado y bloqueado, error → al guardar se pierde el resto (mismo patrón que la app).
- ✔ (se quedan SAFE: abrir una consola o guardar un archivo pedido no cambia el equipo) Dudas de clasificación: «CMD como administrador» y «Guardar en el Escritorio» como SAFE.

### Bien (comprobado por el revisor)
TOOLS = BAR_TOOLS (20, mismo orden, valid_tools); audit.rs completo; bar_open_folder aceptable; caché de navegadores segura; pausa y tiempos; sin interbloqueos; recdraw/reccam se cierran; ajustes viejos compatibles; sin rutas personales en la UI.

### No llegó a revisar (repasar yo o en una segunda ronda)
index.css, demo.ts, faultrec.test.ts, faultrec/analysis.rs (solo vio el tipo de marca manual), resto de settings/General.tsx; FaultRecorder.tsx entero (opciones del Setup, cámara al empezar desde la página); clip.rs fuera de «Para enviar»/GIF; bucle entero de capture.rs; pruebas al final de faultrec.rs; aspecto en pantalla (píldora de 400 px con 4 botones, Personalizar ~600 px en pantallas pequeñas, altos de los paneles, CamCorner, DrawLayer); setInterval sin limpiar en la barra; panel de progreso de Rocket.tsx en la app; pestaña Notas por defecto en Knowledge.tsx. No ejecutó tests.

## B · Commit 1.5.0 — HECHA (27 nov), todo verificado y arreglado
- ✔ GRAVE: el cohete que se queda al cerrar AdminOps heredaba el administrador del técnico (elevada con otra cuenta). leave_rocket_behind: si own_sid ≠ usuario de la sesión, se abre como el usuario y sin elevar (run_as_user + RunAsInvoker).
- ✔ Medio: Asistencia rápida instalaba con el modo auditoría puesto → con auditoría, no instala (abrir sigue).
- ✔ Medio: Reparar winget restablecía los orígenes (quita repos privados) también sin red → con «network» no; con «source», primero `source update` y solo si sigue, reset diciendo que quita los añadidos.
- ✔ Medio: hibernación guardada compartida en el USB portable → archivo por equipo (COMPUTERNAME) y se borra al recuperarla.
- ✔ Leve: gadget() de audio en TS no aceptaba «Realtek(R)» → mismo recorrido que Rust, con prueba.
- ✔ Leve: USB, energía, predeterminados, audio y winget no quedaban en el diario → tweaks::record_app en los 7 comandos.
- ✔ Leve: «N tipos de archivo» contaba asociaciones → cuenta los de la lista.
- ✔ Leve: «Abrir AdminOps» del cohete fallaba sin avisar → panel de error.
- ✔ Comentario de rocket.rs: la caché de navegadores sí está (pedida por David).
- Posibles para la prueba real: USB bloqueado con AdminOps portable en el mismo USB; permiso de micro con GPO ForceDeny (éxito falso); Get-LocalGroupMember con SID huérfanos de Entra; Edge con inicio rápido en avisos de webs.

## A · Segunda ronda (27 nov) — arreglado
- ✔ Medio: tras una pausa el vídeo se congelaba lo que duró la pausa (capture.rs: `n` recalculado al seguir).
- ✔ Medio: Personalizar sin «Guardar» (filas de 28 px, alto con tope de pantalla, lista con scroll; la ventana se reajusta al cambiar el alto de la barra).
- ✔ Medio: la píldora no enseñaba el tiempo (solo el reloj, dibujar como icono, 430 px).
- ✔ Medio: «Empezar el día ligero» casi nunca saltaba (uptime no vuelve a cero con inicio rápido): ahora cuenta desde que arranca el cohete o cambia el día, solo antes de las 13:00, una vez por día también con AdminOps abierta días.
- ✔ Medio: la demo leía/escribía las notas reales → notas de ejemplo y notes_(save|delete) bloqueados.
- ✔ Leves: lápiz cortado con la flecha (barH + 38); panel de progreso 152 px y ✓ de DNS; titular «Marca» (se saltan las manuales); pista «Sonido» cuando es solo el del equipo; botón de la cabecera saca la cámara; aviso en la barra si el reinicio de esta noche se cancela; Notas se guardan solas al salir y borrar fallido no cierra; panel de velocidad 290 px.
- Posibles para la prueba real: la capa de dibujo puede subir encima de la píldora/barra al tocarla (solo queda Esc); «Red» dice «Cable» en 24H2 sin permiso de ubicación.

Comprobado tras B y la 2.ª ronda (27 nov): clippy limpio, cargo test 918, tsc, eslint, vitest 501.
