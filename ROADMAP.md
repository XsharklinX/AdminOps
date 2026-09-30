# AdminOps — Hoja de ruta

Estado actual: **v1.1.6**. Este documento recoge el plan en curso, lo que ya está hecho y
la deuda técnica conocida. Cada fase tiene un criterio de "terminado" para saber cuándo
cerrarla.

---

## Plan actual — Profesionalizar lo que hay (v1.2)

**Nada de funciones nuevas hasta cerrar esto.** AdminOps ya hace mucho; lo que le falta es hacerlo
siempre bien. Este plan sale de lo que pasó entre la 1.1.5 y la 1.1.6, no de una lista de ideas:

- **Cinco fallos graves pasaron todas las pruebas** (240 entre Rust e interfaz) y rompieron la app
  al usarla: portales colgados en «Abriendo…», Ajustes sin poder pulsar nada, el MFA de Microsoft
  rechazado, el modo claro con el fondo negro y la ventana del portable en negro. Las pruebas miran
  las piezas por separado; **nada comprueba la aplicación en marcha**.
- **La 1.1.6 se compiló cinco veces** con contenido distinto y el mismo número de versión.
- **127 archivos sin guardar en Git**: el último commit es la 1.1.4. Todo lo de la 1.1.5 y la 1.1.6
  vive solo en este disco.
- Cada cambio de un dato (la red, una visita, una carpeta compartida) había que hacerlo **a mano en
  Rust y en TypeScript**, y el cruce de comandos se hacía a mano hasta hace poco.

Cada fase cierra un tipo de fallo de los que ya han pasado. Van en orden: la 30 es la que más
problemas evita, y conviene hacerla antes que ninguna otra.

### Fase 30 — Probar la aplicación en marcha

**Prioridad: máxima.** Es lo que habría parado los cinco fallos de arriba antes de llegarte.

- **Pruebas de humo sobre el `.exe` compilado**, con WebDriver (`tauri-driver` + `msedgedriver`):
  arranca, pinta la primera página en menos de N segundos, recorre las 34 pestañas sin errores en la
  consola, y cierra limpio.
- **Portal de pruebas propio**: un servidor web mínimo que levanta la propia prueba, para comprobar
  que un portal carga, se oculta, se vuelve a mostrar y conserva la sesión, sin depender de Microsoft
  ni de la intranet.
- **«¿Se puede pulsar?»**: después de abrir un portal, ir a Ajustes y pulsar un botón. Es exactamente
  el fallo de la capa invisible.
- **Capturas de las páginas clave en los dos temas**, comparadas con las de referencia. Es el fallo
  del modo claro. `scripts/bench.ps1 -Page` ya sabe sacar capturas; se aprovecha.
- **Arranque en frío del portable** en Windows Sandbox (`tests/sandbox` ya lo hace para la prueba de
  ida y vuelta): carpeta nueva, primera vez, sin nada guardado.
- Todo en la CI, que falla si algo de esto falla.

**Terminado cuando:** la CI no deja pasar una versión que no arranca, en la que un portal no carga, en
la que algo tapa los clics, o en la que un tema se ve roto.

### Fase 31 — Publicar con disciplina

**Prioridad: alta, y la primera parte es de hoy mismo.**

- **Guardar en Git lo que ya existe** y, a partir de ahí, un commit por cambio y una etiqueta por
  versión publicada.
- **Una versión, un contenido**: cualquier build que salga de este equipo sube el número de versión
  sola. Nunca se sobrescribe una carpeta de `release\` que ya exista.
- **Novedades de cada versión** escritas para el técnico (qué cambia para él, no qué función se tocó),
  visibles en «Acerca de» y junto al instalador.
- **Canal de prueba**: cada versión pasa un día en tu equipo antes de llegar a nadie más.

**Terminado cuando:** de cualquier AdminOps instalado se puede saber exactamente qué código lleva, y
volver a la versión anterior es copiar una carpeta.

### Fase 32 — Un solo contrato entre la interfaz y el programa

**Prioridad: alta.** Quita de raíz una clase entera de fallos.

- **Tipos generados desde Rust** (`tauri-specta`): los datos y los comandos se definen una sola vez y
  la interfaz los recibe generados. Cambiar un campo en Rust sin actualizar la interfaz deja de
  compilar, en vez de fallar en el equipo del cliente.
- **Partir los archivos enormes**, que es donde más fácil es romper algo sin darse cuenta:
  `api.ts` (2.256 líneas), `portals.rs` (1.511), `SettingsPage.tsx` (1.447), `Tickets.tsx` (893).
- **Los 9 avisos de promesas sin capturar** que quedan: decidir en cada uno qué hacer con el error.

**Terminado cuando:** no hay ningún tipo escrito dos veces y ningún archivo pasa de ~600 líneas.

### Fase 33 — Pulido de la interfaz

**Prioridad: media.** Lo que hace que parezca un producto y no un proyecto.

- **Los dos temas revisados pantalla por pantalla**, con contraste suficiente para leer sin esfuerzo.
- **Los mismos estados en todas partes**: cargando, vacío, error y «necesita administrador», con el
  mismo aspecto y el mismo tono en las 34 pestañas.
- **Cada error dice qué hacer**, no solo qué pasó. Revisar los que todavía llegan en crudo de Windows.
- **Todo se puede hacer con el teclado**, con el foco siempre visible.

**Terminado cuando:** una lista de comprobación de las 34 pestañas en los dos temas pasa entera.

### Fase 34 — Listo para que lo use el equipo

**Prioridad: media**, y es lo que pide la presentación al supervisor.

- **Manual corto del técnico**, con capturas: lo que hace cada sección y los cinco usos más comunes.
- **Guía de instalación para IT**: requisitos, qué pide permisos y por qué, qué conexiones hace
  (sección 4.6 de `docs/PRESENTACION.md`), cómo añadirlo a la lista de confianza del antivirus.
- **Revisión de dependencias en la CI** (`cargo audit`, `npm audit`) y la lista de componentes de
  terceros, que IT va a pedir.
- **Paquete de soporte más útil**: versión, equipo, últimos errores y los tiempos de arranque, en un
  clic, para que un problema en otro equipo se pueda diagnosticar sin ir hasta él.

**Terminado cuando:** otro técnico lo instala y lo usa una semana sin tener que preguntarte nada.

### Después, cuando lo anterior esté cerrado

Funciones nuevas que siguen teniendo sentido, en este orden: **avisar cuando un ajuste lo controla la
directiva del dominio** (y el cambio no va a durar), **comparar un equipo con el equipo patrón**, y
**una carpeta de red compartida como base común** de clientes, contactos y visitas entre técnicos.

---

## Hecho (v0.1 – v1.1)

| Fase | Versión | Contenido |
|---|---|---|
| 1. Base | 0.1 | Tauri 2 + React + Tailwind, tema oscuro neón, UAC, panel del sistema en vivo |
| 2. Motor de ajustes | 0.2 | Catálogo TOML declarativo, detección de estado, diario con copia exacta, deshacer, puntos de restauración |
| 3. Catálogo | 0.3 | Limpieza, privacidad, rendimiento, servicios, bloatware (Appx) e Inicio; usuario destino para HKCU |
| 4. Diagnóstico | 0.4 | Discos, BSOD, drivers, batería, seguridad, reparaciones, informe PDF antes/después, ícono |
| 5. Pulido | 0.5 | Perfiles de un clic, modo portable, firma de código preparada, versión unificada |
| 6. Robustez y calidad | 0.7 | Timeouts y Cancelar, progreso en vivo, scripts sobre el usuario destino, registro de actividad, prueba de ida y vuelta, CI, diagnóstico interactivo |
| 7. Rendimiento | 0.7 | PowerShell persistente, Inicio por COM, carga diferida de páginas, pintado más barato, WebView2 con GPU en proceso |
| 8. Herramientas | 0.9 | Procesos (finalizar proceso/árbol), red y speedtest, actualizar software (winget), analizador de espacio, copia de drivers, análisis de Defender |
| 9. Flujo del técnico | 0.9 | Sesión de servicio, clientes, perfiles propios (importar/exportar), marca en el informe, checklist, "by David Bonilla" |
| 10. Hardware | 0.10 | Inventario (placa, BIOS, RAM por ranura, GPU, monitores, clave OEM), temperaturas (LibreHardwareMonitor), SMART, prueba de RAM, speedtest con medidor animado e IP/proveedor |
| 11. Caja de herramientas | 0.11 | ~90 accesos rápidos a utilidades de Windows (favoritos y accesos propios), usuarios locales (crear, contraseña, admin/estándar, activar, eliminar con perfil), ficha rápida del equipo, redes Wi-Fi guardadas con contraseña |
| 12. Después de formatear | 0.12 | Instalar programas en lote (winget, listas propias), copia y restauración de datos del usuario, herramientas de red (ping/traza, DNS, puertos, hosts), impresoras, driver probable de los pantallazos azules |
| 13. Entorno corporativo | 0.13 | Tickets (portales web dentro de la app, incrustados o en ventana, con sesión y contraseña recordadas), dominio (estado, comprobaciones previas, unir, salir, reparar relación de confianza, renombrar), paracaídas de errores de la interfaz |
| 14. Orden y pulido | 0.14 | Búsqueda global Ctrl+K, barra lateral plegable con favoritos y grupos reordenados, asistente de primer arranque, ventana y última página recordadas, historial Alt+←/→, notificaciones al terminar tareas largas, portable sin rastro de WebView2, paquete de soporte |
| 15. Mantenimiento a fondo | 0.15 | Desinstalador con restos a la papelera, Windows Update (historial explicado, pausar, ocultar), análisis del arranque, restaurar drivers, caché de navegadores, reparaciones de Store, búsqueda, audio y perfil temporal, espacio de puntos de restauración, mantenimiento programado |
| 16. Seguridad del equipo | 0.16 | Nota 0-100 con arreglos, 8 ajustes de seguridad reversibles, BitLocker y copia de claves, programas de riesgo, elementos sospechosos, extensiones de navegador, nota en diagnóstico e informe |
| 17. Instalador y atajos | 0.17 | Instalador con interfaz propia (instalar/actualizar/reinstalar, progreso, consejos), instalador clásico para despliegues silenciosos, página de atajos de teclado con modo descubrir y prueba |
| 18. Rediseño | 0.18 | Lenguaje visual sobrio (IBM Plex, un acento, color solo para estados, sin brillos ni degradados), tema claro, navegación en 7 áreas con pestañas, Panel y Herramientas rehechos, icono nuevo, informe e instalador con el mismo estilo |
| 19. Informe y cliente | 0.19 | Informe PDF rehecho con dos plantillas (cliente y técnica), presupuesto o recibo con impuestos, firma del cliente y del técnico, envío por correo con adjunto, garantías, recordatorios de mantenimiento, evolución entre visitas, icono del medidor con llave |
| 20. Red y router | 0.20 | Mi red (conexión, router, DNS, IP pública), panel del router dentro de la app con acceso guardado y cifrado, chequeo de seguridad del router y la Wi-Fi, doble NAT/CGNAT, QR de la Wi-Fi, dispositivos en la red con fabricante, nombres propios y aviso de nuevos |
| 21. Caja fuerte y privacidad | 0.21 | Caja fuerte (.vhdx con BitLocker que se abre sin AdminOps), carpeta cifrada AES-256, borrado seguro y del espacio libre, checklist antes de vender, recuperar archivos borrados (Windows File Recovery), control parental (filtro DNS, sitios bloqueados, horario de uso) |
| 22. La oficina completa | 0.22 | Inventario de equipos con veredicto (bien/mejorar/renovar) y exportación a Excel, acceso remoto (Escritorio remoto, Asistencia rápida, Wake-on-LAN), carpetas compartidas y permisos, IP duplicadas, mapa de la red en la ficha del cliente |
| 23. Ajustes, rendimiento y red a fondo | 0.23 | Panel y diagnóstico más rápidos, Ajustes en pestañas, acento, tamaño, navegación a medida, bloqueo con PIN, copia de la configuración, identificación de dispositivos (tipo, marca, modelo), contraseña de la Wi-Fi, 26 herramientas nuevas |
| 24. Versión 1.0 | 1.0 | Las páginas conservan su contenido (Recargar/F5), atajos propios, favoritas en el Panel, punto de restauración configurable, inicio con Windows, limpieza de datos antiguos, aviso de versiones nuevas |
| 25. Pulido | 1.1 | Sin cierres por fallos internos, vigilancia de errores de Windows con explicación, acceso remoto a fondo (agenda, opciones, credenciales, AnyDesk/RustDesk/TeamViewer), navegación totalmente personalizable, 283 atajos |

### Fase 6 en detalle

- **Nada puede colgar la interfaz.** Todo proceso tiene límite de tiempo (120 s por defecto; las tareas
  declaran el suyo con `timeout`). SFC, DISM y WinSxS no tienen límite, pero se pueden **cancelar**:
  se termina el árbol de procesos completo y lo ya cambiado se deshace.
- **Progreso en vivo** (evento `task-progress`) en ajustes, perfiles ("3/10 · …"), apps y puntos de restauración.
- **Usuario destino en scripts:** todos reciben `$UserSid`, `$UserHive`, `$UserProfile`, `$UserTemp`…
  `cargo test` rechaza scripts del catálogo que usen `$env:TEMP`, `HKCU:` o `Clear-RecycleBin`.
- **Registro de actividad** con rotación (5 × 1 MB) y visor en Historial → "Registro técnico".
- **Errores legibles:** sin "At line: … ~~~~" ni CategoryInfo.
- **Apps:** las operaciones Appx van una a una y se reintentan si Windows está ocupado (corrige
  "Another operation on app packages is in progress").
- **Prueba de ida y vuelta:** `adminops.exe --roundtrip informe.json` aplica y deshace cada ajuste y
  falla si el registro o los servicios no vuelven exactamente a su estado. Corre en la CI y en
  Windows Sandbox (`tests/sandbox/run-roundtrip.ps1`).
- **CI** (`.github/workflows/ci.yml`): tipos, Clippy `-D warnings`, tests, instalador, portable y
  prueba de ida y vuelta; los artefactos quedan en cada ejecución.
- **Diagnóstico interactivo:** cada hallazgo lleva a su detalle, al ajuste que lo resuelve o a la
  herramienta de Windows (lista cerrada: Administrador de dispositivos, Confiabilidad, Update…).

### Fase 7: medido

Medido con `scripts/bench.ps1` y `cargo test --release bench -- --ignored --nocapture`
(Ryzen 5 5500, Windows 11 26200).

| Métrica | Antes | Después | Objetivo |
|---|---|---|---|
| Listar Inicio | 3 398 ms | **310 ms** | < 500 ms ✅ |
| Listar apps | 834 ms | **370 ms** | < 500 ms ✅ |
| 10 consultas PowerShell | 3 022 ms | **435 ms** | — |
| Detectar el catálogo | 329 ms | **70 ms** | — |
| Arranque hasta ventana | — | **~400–600 ms** | < 1 s ✅ |
| JS inicial | 308 KB | **253 KB** | — |
| RAM privada total | ~260 MB | **~175 MB** | ver nota |
| RAM del proceso de AdminOps | — | **~10 MB** | ✅ |
| CPU minimizada | — | **~0 %** | < 1 % ✅ |
| CPU con el Panel visible | 18 % de un núcleo | **~4–10 %** | ver nota |

**Notas honestas:**
- El objetivo de "< 80 MB" no es alcanzable con WebView2: su runtime ya ocupa ~165 MB por sí solo.
  AdminOps en sí usa ~10 MB. Bajar de ahí exigiría cambiar de motor de interfaz.
- La CPU con el Panel visible se midió con el PC en uso y varió mucho entre pasadas. Repetir la
  medición con el equipo en reposo antes de dar la cifra por buena.

---

## Deuda técnica conocida

- **La prueba de ida y vuelta aún no se ha ejecutado** en un Windows 11 cliente limpio: la CI usa
  Windows Server. Ejecutarla en Sandbox al menos una vez por versión.
- **Dependencia de Edge para el PDF.** Si falta, el informe cae a HTML.
- **Sin firma de código** (decisión: no se contempla por coste). SmartScreen avisará al instalar.
- **Solo español** (inglés en la Fase 28).
- **Notificaciones en portable**: sin instalador, Windows puede mostrarlas con otro nombre de app.
- **Tickets incrustados** usan la API "unstable" de Tauri (vistas hijas); si una versión futura la
  cambia, queda la ventana aparte como alternativa.

### v1.1.6 (segunda parte) — Portales: la raíz, no los síntomas ✅

**⚠ Diagnóstico equivocado, corregido en la cuarta parte.** Lo que sigue explica un cambio que rompió los portales; se deja escrito para que no se repita. **El fallo de Tickets en la oficina tenía causa, y era grave.** La ventana principal arranca WebView2
con `--auth-server-allowlist` cuando hay una intranet configurada; las vistas de los portales
arrancaban solo con los argumentos del archivo de configuración. WebView2 **no admite dos
configuraciones distintas en la misma carpeta de datos**, así que la vista del portal no llegaba a
crearse y la página no abría. Se daba únicamente donde hay un portal de intranet: en el trabajo, y en
ningún otro sitio. Ahora los portales leen los argumentos con los que arrancó de verdad el navegador
(`ARGS_ENV`), no una parte de ellos. Con prueba.

**La sesión ya no se pierde al salir.** Los portales de Correo y Teams se creaban con «sesión
privada» activada de fábrica, que es exactamente lo que borra la sesión al cerrar AdminOps y obliga a
repetir el inicio de sesión y la verificación del móvil cada vez. Se creaba así pensando en el equipo
del cliente, pero el caso normal es el equipo del técnico. Ahora se crean con la sesión guardada, y la
privada se marca a mano cuando toca. Los textos dicen lo que cuesta cada opción, no solo su ventaja.
De paso, con perfil persistente Teams y Outlook cachean sus archivos: la segunda carga es mucho más
rápida que la primera.

**Los portales dejan de tirarse a la basura.**
- Una vista sin usar se cerraba a la media hora, y volver a entrar costaba una carga completa de
  Teams o de Outlook. Media hora no es nada en una jornada. Ahora **4 horas** en el equipo del
  técnico; se mantienen los 30 minutos en equipos justos de recursos, que es donde la memoria pesa.
- Dormir la vista al ocultarla hacía que **ir y volver** entre el Correo y otra página costara un
  despertar cada vez. Ahora se duerme solo tras **3 minutos** sin verse: el ir y venir normal es
  instantáneo y solo se aparca lo que de verdad se dejó aparcado.

**Arranque**
- **La ventana se enseña cuando hay algo pintado, no antes.** Enseñarla a los 2,5 s pasara lo que
  pasara era lo que producía el rectángulo negro del portable: si WebView2 aún no ha pintado, lo
  único que se ve es una ventana vacía, y eso parece una aplicación colgada. Red de seguridad a los
  20 s para no quedarse nunca sin ventana, y recarga solo si WebView2 no cargó nada en 30 s.
- El registro anota ahora **cómo y cuándo se cierra la ventana**. Una aplicación que desaparece sola
  no dejaba ni rastro y no había forma de saber si la cerró el usuario, Windows, o se cayó ella.

### v1.1.6 (quinta parte) — El MFA de Microsoft vuelve a funcionar ✅

**«Sorry, we're having trouble verifying your account» al iniciar sesión en Teams o el Correo.** Lo
introdujo la 1.1.5: para ahorrar procesador, los portales ocultos se **congelaban** (`TrySuspend` de
WebView2), y uno precargado se congelaba nada más terminar su primera carga si todavía no se había
abierto. Sin sesión iniciada, esa primera carga termina en la página de inicio de sesión de
Microsoft, que se quedaba congelada ahí minutos. Esa página lleva un contexto de inicio de sesión que
se renueva mientras está viva; congelada, caduca, y al pedir el código Microsoft rechaza la
verificación.

**Se ha quitado la congelación entera**, no parcheado: los portales ocultos siguen vivos, como en la
1.1.4, que es cuando el MFA funcionaba. El propio WebView2 ya frena lo que no se ve. Se mantienen el
resto de mejoras de los portales, revisadas una a una para que ninguna toque el inicio de sesión.

**Aviso**: tras muchos inicios de sesión seguidos en poco tiempo (cada build, más la sesión privada
del Correo), Microsoft limita durante un rato el envío de códigos por SMS o llamada. Si justo después
de actualizar sigue fallando, es eso: esperar o usar Microsoft Authenticator.

### v1.1.6 (cuarta parte) — Corrección de los portales ✅

**Los portales se quedaban para siempre en «Abriendo…» (Correo, Teams y Tickets a la vez).** Lo
introdujo la propia 1.1.6. WebView2 exige que todas las vistas que comparten carpeta de datos se creen
con **opciones idénticas**. La ventana principal se crea con los argumentos del archivo de
configuración; la lista de dominios de la intranet va aparte, en una variable de entorno que WebView2
aplica por igual a todas las vistas. En la 1.1.6 se añadió esa lista también a las opciones de cada
portal, creyendo que faltaba: los portales quedaban distintos de la ventana principal y WebView2 se
negaba a crearlos, sin ningún error a la vista. Pasaba en cuanto había una intranet configurada.
El diseño original era el correcto y se ha restaurado, con una prueba de regresión que lo fija.

Además:
- **Un portal que no arranca ya no gira para siempre**: a los 25 s sin señal de vida lo dice, con
  Reintentar (que vuelve a crear la vista). Un fallo así no puede volver a pasar en silencio.
- **Ajustes no dejaba tocar nada (ni hacer scroll).** Consecuencia del mismo fallo: las vistas de los
  portales son ventanas del sistema que van por encima de la interfaz, y una vista que WebView2 no
  llegó a iniciar no pinta nada ni responde a «ocultar». Se quedaba invisible encima de la zona de
  contenido tragándose clics y scroll, y se notaba en la siguiente página que se abría. Ahora una
  vista que nunca arrancó **se destruye** al salir de su página o a los 25 s (`portal_reset`), en vez
  de intentar ocultarla. Distinguir «no arrancó» de «arrancó pero va lenta» es seguro: una vista sana
  avisa de que empieza a cargar en menos de un segundo.
- **AdminOps abre en el Panel.** «La última página que usé» venía de fábrica y quedaba guardada al
  tocar cualquier ajuste, aunque nadie la hubiera elegido. Ahora solo se respeta si el técnico la
  elige a propósito en Ajustes.

### v1.1.6 (tercera parte) — Agenda, Usuarios, Impresoras y Carpetas ✅

Cuatro apartados que se habían quedado cortos.

**Agenda** — era una lista de citas y poco más.
- **Visitas que se repiten** (semanal, quincenal, mensual, trimestral, semestral, anual). Al marcar
  una como hecha, **la siguiente se planifica sola**, contada desde la fecha prevista para que
  atender con retraso no desplace toda la serie. El mantenimiento periódico deja de depender de que
  alguien se acuerde de volver a apuntarlo.
- **Aviso de visitas que se pisan**: no impide guardar (a veces se solapan a propósito), pero se dice
  al guardar y no el día de la visita.
- **Aplazar en un clic** desde la propia fila, sin abrir el editor. Es lo que más se hace.
- **Dónde es** (sede, planta, sala) y **tipo de visita**, enlazado con los tipos de Ajustes.

**Usuarios** — **renombrar la cuenta** y editar su nombre completo y descripción, sin salir a
`lusrmgr.msc`. Con las mismas reglas de seguridad que el resto: no se renombran cuentas integradas ni
de Microsoft, ni una con la sesión abierta. Y avisa de lo que despista a todo el mundo: **Windows no
renombra la carpeta del perfil**, y cambiarla a mano lo rompe.

**Impresoras**
- **«Revisar»**: dice por qué no imprime, señalando el primer eslabón roto de la cadena (cola de
  Windows → impresora encendida y en red → estado del aparato → atascos). Distingue «Windows la tiene
  marcada como sin conexión» de «la impresora no responde en 192.168.1.30», que se arreglan de forma
  muy distinta. Con siete casos cubiertos por pruebas.
- **Buscar impresoras en la red**: prueba los puertos de impresión (RAW, IPP, LPD) sobre los equipos
  que ya responden, y marca cuáles no están instaladas aquí. Antes, saber qué impresoras había en una
  oficina era preguntar o ir mirando aparato por aparato.

**Carpetas compartidas**
- **Qué archivos están abiertos ahora mismo y quién los tiene**, con aviso de los bloqueados. Un
  archivo abierto no se puede mover, renombrar ni borrar.
- **Carpetas rotas**: se comparte una ruta que ya no existe y quien entra ve un error de Windows.
- **La trampa de los permisos**: compartida con «Todos» pero con los permisos del disco cerrados. El
  acceso real es lo que dejen los dos a la vez, y es la causa clásica de «acceso denegado» que nadie
  entiende.

**Deuda técnica**: de 178 avisos de promesas sin capturar a **9**. Los 169 arreglados eran llamadas
de usar y tirar que ya capturan por dentro; los 9 que quedan son cadenas `.then(…)` que hay que mirar
una a una y decidir qué hacer con el error.

### v1.1.6 — Que funcione en el equipo del cliente ✅

Todo esto sale de usar la 1.1.5 de verdad, en equipos que no son el de desarrollo.

**Consumo y cuelgues**
- **La precarga de portales era lo que más pesaba, y estaba mal planteada**: precargaba el último
  portal de *cada* tipo, hasta cuatro navegadores Chromium completos por detrás sin que el técnico
  hubiera pedido ninguno (y Outlook y Teams son de las webs más pesadas que existen). Ahora precarga
  **uno solo**, el de la última página usada, **ninguno en equipos justos de recursos**, y a los 8 s
  en vez de a los 4.
- **El Panel recorría los ~250 procesos del equipo cada 2 segundos** para enseñar los 10 que más
  consumen. Esa parte pasa a cada 6 s; CPU, memoria y red —lo que se ve moverse— siguen en cada tic.
- **El vigilante del arranque recargaba la interfaz a los 12 s**, y en un equipo lento la primera
  pantalla tarda más que eso: se recargaba sola y volvía a empezar. Era la causa de «deja de
  funcionar». Ahora la interfaz avisa en cuanto su código arranca (`ui_booting`) y **solo se recarga
  si WebView2 no ha cargado nada** en 25 s.
- **PowerShell**: de 5 procesos a 2 en equipos de 4 GB o ≤4 núcleos (60-100 MB cada uno).
- Criterio único de «equipo justo de recursos» (4 GB o menos, o 4 núcleos o menos) en
  `src/lib/machine.ts` y su gemelo en `pspool.rs`. El arranque lo deja en el registro.

**Asistencia rápida**
- Seguía saliendo el cuadro en inglés de Windows. La causa: en Windows 11 queda la clave del
  protocolo `ms-quick-assist` con su `URL Protocol` pero **sin ninguna subclave**, o sea sin programa
  que lo abra, y la comprobación la daba por buena. Ahora se exige `shell\open\command`, y se
  prueba primero `quickassist.exe`, que es el que nunca falla. El mismo arreglo cubre `msteams:`.

**Texto**
- **Se puede leer el texto cortado**: al pasar el ratón por algo recortado con «…» se enseña entero.
  Resuelto de una vez para toda la aplicación (`src/lib/fullTextOnHover.ts`) en lugar de añadir el
  `title` en los casi cien sitios donde pasa y olvidarlo en los siguientes.
- **Las cajas con su propio scroll crecen con la ventana** (`pane-sm`, `pane-md`, `pane-lg`). Con un
  alto fijo en píxeles, en una pantalla grande sobraba sitio y en un portátil el contenido quedaba
  metido en una rendija con dos barras compitiendo.

**Reparar la red**
- Deja de ser una lista de pasos y **dice qué pasa y qué hacer**. Señala el primer eslabón roto de la
  cadena (tarjeta → IP → router → Internet → DNS), que es distinto del último síntoma: «el router no
  te está dando dirección, reinícialo» en vez de «no hay Internet». Con botón a lo siguiente (panel
  del router, cambiar DNS, velocidad) y los datos de la conexión a la vista (IP, router, DNS, proxy,
  VPN), para no ir a buscarlos a otra página.
- Dos pasos nuevos: **vaciar la tabla ARP** (el equipo tiene IP y no llega al router porque guarda la
  MAC vieja, típico al cambiar o reiniciar el router) y, en la reparación a fondo, **quitar el proxy
  de las descargas del sistema**, que heredado de otra red deja el equipo «conectado sin Internet».

**Diagnóstico**
- Un portal que no carga deja en el registro la dirección exacta y el código de WebView2. Antes
  «No se pudo abrir la página» no se podía diagnosticar a distancia.

### Auditoría de v1.1.5: resuelto ✅

**Redes de seguridad nuevas**

1. **El cruce de comandos es una prueba**, no una comprobación a mano
   (`src/lib/commands.test.ts`): cada `invoke("x")` de la interfaz tiene que estar en
   `generate_handler![…]`. Falla nombrando el comando y el archivo. Ya había dejado pasar dos
   comandos inexistentes a producción.
2. **El modo auditoría no se puede escapar por olvido.** `audit.rs` tiene ahora las dos listas
   completas —`BLOCKED` (78 comandos que cambian el equipo) y `SAFE` (246 revisados uno a uno)— y
   tres pruebas: que todo comando registrado esté clasificado, que ninguno esté en las dos listas ni
   sobre de una, y que los bloqueados existan. Añadir un comando obliga a decidir en cuál va.
   Comprobado a mano que las pruebas fallan de verdad al quitar una entrada.
3. **ESLint con las reglas de hooks como error** (`eslint.config.js`, `npm run lint`, en la CI).
   `react-hooks/exhaustive-deps` es la regla que habría avisado del portal que se recolocaba en cada
   recarga de la lista. Las 10 excepciones que había eran todas a propósito y ahora llevan el motivo
   escrito en la línea. Las reglas del compilador de React se apagan a conciencia: dan por malos
   patrones correctos aquí.

**Fallos latentes corregidos**

4. **Respuestas que llegan tarde** (`src/lib/useLiveEffect.ts`): 21 efectos que guardaban en el
   estado el resultado de una consulta ya no pisan al nuevo cuando cambia lo que se está mirando.
   Los que se notaban: el tamaño del perfil de otro usuario (Usuarios), los datos del perfil anterior
   al cambiar de usuario en Migrar, y la cuenta guardada de otro portal en el editor de portales.
5. **Las descargas de los portales ya no crecen sin límite**: cada una lleva su número propio (antes
   era su posición en la lista, que impedía podarla) y se recuerdan las últimas 200.
6. **Los datos de un portal se olvidan al cerrarlo o borrarlo** (rectángulo, última vez visto, cuándo
   se creó y a dónde iba). Además, una primera carga que se cancele ya no deja la vista sin poder
   dormirse nunca: a los 90 segundos se da por terminada.

**Alineación de la CI**

7. La CI usa ahora `cargo clippy --all-targets` (antes `--lib --tests`, más flojo que la verificación
   local), pasa ESLint y repite clippy con la versión de Rust fijada que se verifica a mano. Ya no
   puede pasar en verde algo que falle en local.

**Pendiente**

- **178 avisos de `@typescript-eslint/no-floating-promises`**: promesas sin `await` ni `.catch()`.
  99 son llamadas a un `load()` que ya captura por dentro; el resto hay que mirarlas una a una. Queda
  como aviso (no rompe la CI) para limpiarlas por páginas, no de golpe.

---

## Fases 8 y 9: hecho

- **Procesos**: lista en vivo (CPU, RAM, disco, usuario, ruta), finalizar proceso o árbol. Los críticos
  de Windows no se pueden cerrar; los sensibles (svchost, explorer, dwm…) avisan de las consecuencias.
  Antes de cerrar se comprueba que el PID sigue siendo el mismo programa. Todo queda en el diario.
- **Red y velocidad**: adaptadores, SSID y señal, ping ICMP nativo a router/DNS/Internet, DNS,
  detección de portal cautivo. **Speedtest** contra Cloudflare: 6/4 conexiones TCP en paralelo durante
  10 s, se descartan 2 s de arranque, latencia sin el tiempo del servidor (`server-timing`), jitter y
  latencia con carga (bufferbloat). Medido en el equipo de desarrollo: 36 ms (RTT TCP del servidor: 35 ms).
- **Actualizar software** con winget (tabla interpretada por columnas: independiente del idioma).
- **Espacio en disco**: `FindFirstFileExW` + rayon; `C:\Windows` (215 000 archivos) en 2,1 s (antes 124 s).
- **Copia de drivers** (`pnputil /export-driver`) y **análisis rápido de Defender**.
- Diagnóstico e informe incluyen software pendiente, antigüedad del último análisis y el último speedtest.
- **Sesión de servicio**, **clientes** con equipos e historial, **perfiles propios**, **marca del técnico**
  (logo, empresa, contacto, condiciones) y **checklist** configurable en el informe.

### Pendiente de verificar a mano (requiere administrador o modifica el equipo)
- Finalizar procesos de otros usuarios, actualizar software con winget y la copia de drivers.
- Una sesión de servicio completa de principio a fin (inicio → trabajo → informe archivado).

---

## Plan (v0.10 – v1.4)

Objetivo: pasar de "herramienta muy completa" a **producto profesional y redondo**, sin costes
(nada de certificados de pago ni servicios de suscripción). Cada fase es una versión.

### Fase 10 — Hardware (v0.10) ✅

- Inventario completo en la página **Hardware**, en el informe y en la ficha del cliente, que avisa si el
  hardware cambió entre visitas.
- **Temperaturas** con LibreHardwareMonitor (MPL-2.0, incluido): GPU sin administrador; CPU y placa con
  administrador y el driver PawnIO, instalable desde la app.
- **SMART** de discos SATA por WMI (reasignados, pendientes, no corregibles, CRC), sin programas externos.
- Resultado de la **prueba de RAM**; hallazgos de doble canal, BIOS y drivers antiguos.
- **Speedtest** con medidor animado, fases, gráfica en vivo, "apto para", IPv4/IPv6, proveedor y ubicación.
- Pendiente de esta área: identificar el **driver culpable** de cada pantallazo azul (análisis de minidumps).

### Fase 11 — Caja de herramientas del técnico (v0.11) ✅

- **Herramientas**: unos 90 accesos rápidos en 8 grupos (consolas, administración, diagnóstico, Panel de
  control, Configuración, carpetas, arranque y asistencia remota), con buscador, favoritos y accesos
  propios (programa, carpeta o web). Catálogo en `src-tauri/tools/shortcuts.toml`. Se ocultan los que no
  existen en esa edición de Windows (gpedit en Home). Carpetas y Configuración se abren como el usuario
  del equipo; consolas y .msc, como administrador. Reiniciar a la BIOS/UEFI y al inicio avanzado.
- **Usuarios locales** (también en Windows Home, que no tiene lusrmgr): crear (estándar o administrador,
  contraseña opcional, no caduca / cambiarla al entrar), cambiar o quitar la contraseña, activar,
  desactivar, hacer administrador o estándar, eliminar con o sin su perfil (muestra cuánto ocupa).
  Nunca deja el equipo sin administrador activo ni toca la cuenta con la sesión abierta o la que ejecuta
  AdminOps. Las contraseñas no se registran.
- **Ficha del equipo** en Herramientas: modelo, serie, Windows y clave OEM, con "Copiar ficha".
- **Redes Wi-Fi guardadas** en Red: contraseña (con administrador), seguridad, conexión automática y
  olvidar red. Usa la API nativa de WLAN: las claves nunca se escriben en disco.
- Pendiente de probar en equipos reales: crear, modificar y borrar usuarios y olvidar una red Wi-Fi.

### Fase 12 — Después de formatear (v0.12) ✅

- **Instalar programas**: catálogo de ~75 programas con ids de winget verificados (`src-tauri/tools/apps.toml`),
  listas predefinidas (Básico, Oficina, Gaming, Kit del técnico) y listas propias, búsqueda en winget para
  cualquier otro programa y marca de los ya instalados (`winget export`, independiente del idioma).
- **Copia de datos**: Escritorio, Documentos, Imágenes, Música, Vídeos, Descargas (respetando las carpetas
  redirigidas a OneDrive), marcadores de Chrome/Edge/Brave/Firefox y redes Wi-Fi, a un USB con
  `adminops-backup.json`. Los archivos que solo están en la nube no se descargan. Al restaurar nunca se
  sobrescribe: lo distinto se guarda como «nombre (AdminOps)».
- **Herramientas de red**: ping y traza de ruta en vivo con ICMP nativo, DNS por adaptador (Cloudflare,
  Google, Quad9, familia o propios), puertos en uso por programa y editor del archivo hosts con copia.
- **Impresoras**: estado y errores (atasco, sin papel…), vaciar la cola, página de prueba, predeterminada,
  quitar impresoras fantasma y reiniciar la cola completa.
- **Pantallazos azules**: el diagnóstico lee los minivolcados (formato de triaje de 64 bits) y señala el
  driver probable a partir de la pila del fallo, con una pista de qué hacer.
- El informe de batería ya existía en Diagnóstico (capacidad real frente a la de fábrica).
- Pendiente de probar en equipos reales: instalar en lote, restaurar una copia real, cambiar DNS, guardar
  hosts, quitar una impresora y analizar un volcado real (NotMyFault de Sysinternals debe señalar `myfault.sys`).

### Fase 13 — Entorno corporativo (v0.13) ✅

- **Tickets**: portales web del técnico (intranet de la empresa, GLPI, osTicket…) dentro de AdminOps, en
  la ventana principal o en una ventana aparte. La sesión y las contraseñas se recuerdan (autoguardado de
  WebView2 activado). Aislados: sin acceso a las funciones de AdminOps y solo navegan dentro de sus
  dominios; el resto de enlaces se abre en el navegador.
- **Dominio**: dominio / grupo de trabajo / Entra ID, controlador, relación de confianza, diferencia de
  hora y si la sesión es de dominio. Comprobaciones previas (edición, DNS del dominio, controlador
  accesible por LDAP, hora), unir (con OU y nuevo nombre opcionales), salir (exige un administrador local
  activo), reparar la relación de confianza y cambiar el nombre del equipo. Las credenciales no se guardan.
- **Paracaídas de errores**: si una página falla muestra el error en vez de dejar la ventana en negro, y
  lo guarda en el registro técnico. Corregido el fallo que ponía la app en negro al salir de Herramientas
  de red (`scrollIntoView` devuelve una Promise en Chromium reciente).
- Pendiente de probar en equipos reales: unir, salir y reparar en el dominio de la empresa.

### Fase 14 — Orden y pulido (v0.14) ✅

**Prioridad: alta.** Con 32 secciones, lo urgente es que todo se encuentre rápido y se sienta terminado.

- **Barra lateral plegable** por grupos (se recuerda qué está abierto) y **favoritos** fijados arriba.
- **Búsqueda global (Ctrl+K)**: saltar a cualquier página, ajuste, herramienta, reparación, programa o
  proceso escribiendo. Es el atajo que más tiempo ahorra al técnico.
- **Primer arranque guiado**: nombre y logo del técnico, portal de Tickets, dominio habitual y primer
  diagnóstico, para que la app no empiece vacía.
- **La ventana recuerda** tamaño, posición y última página; **atajos de teclado** básicos.
- **Notificaciones de Windows** al terminar tareas largas (SFC, DISM, instalaciones, copias).
- **Portable sin rastro**: la caché de WebView2 también en `AdminOps-data` (hoy queda en el equipo).
- **Paquete de soporte**: un botón junta registro, último diagnóstico y versión en un .zip.
- **Revisión de textos y estados vacíos** en todas las páginas (qué hacer cuando no hay datos).
- Hecho además: grupos reordenados (Sistema, Optimizar, Soporte, Administración) y Ajustes en el pie.
- Pendiente: revisión completa de estados vacíos página por página (se irá haciendo en cada fase).

**Terminado cuando:** cualquier función se alcanza en dos pulsaciones y un usuario nuevo llega a su
primer informe sin instrucciones.

### Fase 15 — Mantenimiento a fondo (v0.15) ✅

**Prioridad: alta.** Resolver el "PC lento" completo sin salir de AdminOps.

- **Desinstalador de programas** (Win32 y Store) con desinstalación silenciosa y limpieza de restos
  (carpetas y claves huérfanas), mostrando lo que libera cada uno.
- **Limpieza de navegadores** por perfil (Chrome, Edge, Firefox, Brave, Opera): caché y descargas
  temporales; contraseñas e historial solo si se elige.
- **Windows Update**: historial, actualizaciones fallidas con su error explicado, pausar/reanudar y
  ocultar una actualización problemática.
- **Análisis del arranque**: qué programas y servicios retrasan el inicio.
- **Restaurar drivers** desde una copia de AdminOps (`pnputil /add-driver`).
- **Más reparaciones**: Microsoft Store (`wsreset`), búsqueda de Windows, audio, asociaciones de
  archivos, perfil de usuario dañado (el que inicia con perfil temporal).
- **Puntos de restauración**: ver cuánto ocupan y borrar los antiguos.
- **Mantenimiento programado** opcional (limpieza semanal/mensual como tarea del sistema).
- Hecho además: prueba que analiza la sintaxis de todos los scripts de PowerShell (catálogo e incrustados)
  sin ejecutarlos; encontró un fallo real (`Data` es palabra reservada) antes de publicarse.
- Pendiente de probar en equipos reales: desinstalar, ocultar una actualización, restaurar drivers y la
  tarea programada (se ejecuta como SYSTEM con `adminops.exe --maintenance`).

**Terminado cuando:** las tareas habituales de mantenimiento no requieren ninguna otra herramienta.

### Fase 16 — Seguridad del equipo (v0.16) ✅

**Prioridad: alta.** Muy valorado por clientes y empresas, y fácil de explicar en el informe.

- **Auditoría de seguridad con nota** (0-100) y cómo subirla: Defender y firmas, firewall, UAC,
  BitLocker, SMB1, escritorio remoto expuesto, número de administradores, cuentas sin contraseña,
  actualizaciones pendientes, arranque seguro y TPM.
- **BitLocker**: estado por unidad y **copia de la clave de recuperación** (al USB o al informe
  técnico), la causa típica de equipos bloqueados.
- **Programas vulnerables**: versiones antiguas de navegadores, Java, Adobe Reader… con actualizar en
  un clic (winget).
- **Elementos sospechosos**: tareas programadas, servicios e inicio sin firma o en carpetas temporales;
  extensiones de navegador instaladas; archivo hosts modificado.
- **Sección de seguridad en el informe** con la nota antes y después.
- Hecho además: categoría de ajustes `security` (firewall, UAC, SMB1, NLA, RDP, ejecución automática,
  Invitado, bloqueo de PUA), todos reversibles; hallazgo en el diagnóstico si la nota baja de 80.
- Pendiente de probar en equipos reales: exportar claves de BitLocker y desactivar una tarea sospechosa.

**Terminado cuando:** el técnico entrega una nota de seguridad comprensible y mejorada en cada visita.

### Fase 17 — Instalador y atajos (v0.17) ✅

- **Instalador con interfaz propia** (`installer/`): ventana con la marca, carpeta de instalación, acceso
  en el escritorio y abrir al terminar. Detecta la versión instalada (actualizar, reinstalar o bloquear si
  la instalada es más nueva) y si AdminOps está abierto. Barra de progreso con consejos. Por dentro ejecuta
  el instalador NSIS en silencio, así que conserva desinstalador, entrada en Windows y accesos.
- **Instalador clásico** junto al nuevo: con asistente y `/S` para desplegar en muchos equipos.
- **Atajos de teclado**: unos 150 en 12 categorías (incluida una de técnico y soporte), buscador, mis
  atajos, modo «descubrir» (pulsas una combinación y dice qué hace) y «Probar» para los de la tecla
  Windows (solo combinaciones Win+… de una lista cerrada; Win+L excluido).
- Pendiente de probar en equipos reales: instalar, actualizar y reinstalar con el instalador nuevo, y un
  equipo sin WebView2 (debe abrir el clásico).

### Fase 18 — Rediseño (v0.18) ✅

Objetivo: que la app se vea como una herramienta profesional y no como una interfaz «generada».
Propuesta visual: artifact «AdminOps — Propuesta de rediseño».

- **Sistema visual**: IBM Plex Sans (y Mono solo para rutas y códigos), un solo acento azul para la
  selección y la acción principal, verde/ámbar/rojo apagados solo para estados, sin brillos, degradados ni
  desenfoques, títulos en frase (fuera las mayúsculas espaciadas). Tokens en `src/index.css`.
- **Tema claro** además del oscuro (Ajustes → Apariencia).
- **Navegación**: 7 áreas en la barra lateral (Panel, Equipo, Optimizar, Programas, Red, Soporte,
  Administración) y las páginas como pestañas; cada área recuerda su última pestaña. Ajustes en el pie.
- **Panel** rehecho: banda de cifras clave, «Requiere atención» (hallazgos del último diagnóstico),
  procesos y almacenamiento sin cajas.
- **Herramientas** rehecha: filas limpias en lugar de tarjetas, filtros por grupo, ficha compacta.
- **Icono nuevo** (sustituido en la 0.19 por el medidor con llave): una tuerca con el pulso de diagnóstico dentro, plano y de una
  tinta; en la app, el ejecutable, el informe PDF y el instalador.
- Informe PDF e instalador con el mismo lenguaje (el instalador lleva sus fuentes).
- **0.18.1**: barra lateral desplegable (las secciones salen debajo de su área; solo abiertas la actual
  y las que se dejan abiertas), Panel rehecho (veredicto, acciones rápidas, equipo, salud de discos,
  actividad reciente), instalador que cierra todo lo que retiene archivos y reintenta (antes fallaba al
  actualizar con la app abierta), procesos de PowerShell ligados a la vida de la app (job object) y
  caché de iconos de Windows refrescada al instalar. Icono: cinco conceptos en el lienzo de diseño.
- Pendiente: repasar página a página los detalles que el barrido automático no cubre (espaciados y
  tarjetas internas de cada sección) y rehacer las capturas del README.

### Fase 19 — Informe y cliente (v0.19) ✅

Lo que convierte el trabajo técnico en algo que el cliente valora (y paga).

- **Informe PDF rehecho**: cabecera con la marca y número de documento correlativo por año (2026-0001),
  fichas de cliente, equipo (con nº de serie) y servicio (fecha y duración), veredicto con cifras clave
  (seguridad, resueltos, pendientes, acciones), problemas **resueltos** en la visita frente a los
  **pendientes**, antes/después, recomendaciones, firmas y pie con «Página X de Y».
- **Dos plantillas**: para el cliente (lenguaje claro, ficha resumida del equipo que también mira SMART)
  y técnica (todo el detalle: hardware, módulos, discos, SMART, estabilidad, drivers, seguridad, programas).
- **Presupuesto o recibo**: líneas de servicio y piezas, cantidades, descuento, impuesto configurable
  (ITBIS 18 % por defecto), total, forma de pago y validez del presupuesto. Catálogo de servicios y
  piezas en Ajustes para añadirlos con un clic.
- **Firma del cliente** en pantalla al finalizar la sesión (ratón, lápiz o dedo) y **firma del técnico**
  guardada en Ajustes. Sin firma, el PDF deja el espacio para firmar en papel.
- **Enviar por correo**: se abre el programa de correo con el PDF adjunto y el mensaje ya redactado
  (archivo .eml de borrador); alternativa para correo web que abre el correo y la carpeta del PDF.
- **Garantías**: mano de obra y cada pieza con su fecha de fin, en el recibo y en la ficha del cliente.
- **Próximo mantenimiento**: fecha recomendada en el informe y recordatorio en Clientes (vencidos y
  próximos 30 días, posponer o quitar).
- **Comparar visitas**: tabla de evolución en la ficha del cliente (críticos, advertencias, seguridad,
  espacio, inicio, actualizaciones, arranque, batería, memoria).
- El informe hecho a mano (Soporte → Informe) también puede elegir cliente de la ficha, plantilla,
  presupuesto o recibo, y guardar la visita en su historial.
- **Icono definitivo**: un medidor (diagnóstico) cuya aguja termina en una llave (reparación).

### Fase 20 — Red y router (v0.20) ✅

Entrar al router y saber qué hay en la red sin abrir el navegador ni buscar la IP a mano.

- **Mi red**: nombre de la red, conexión (Wi-Fi o cable y velocidad), IP del equipo (fija o DHCP),
  router, DNS e IP pública con el proveedor y la ciudad (oculta hasta pulsar el ojo). Aviso si
  Windows trata la red como pública.
- **Panel del router dentro de AdminOps** (como los portales de Tickets), en ventana aparte o en el
  navegador. Acepta el certificado propio de los routers, solo para direcciones de la red local.
- **Acceso guardado**: usuario, contraseña (cifrada con DPAPI: solo ese usuario de Windows en ese
  equipo puede leerla), dirección del panel y notas, por red. En el panel, botones para copiar el
  usuario y la contraseña. Pista de credenciales de fábrica según la marca detectada.
- **Chequeo del router**: marca (por la página del panel), puertos abiertos (Telnet y FTP como
  riesgo), panel sin HTTPS, cifrado de la Wi-Fi (abierta, WEP, WPA, WPA2, WPA3) y doble NAT o red
  del proveedor / CGNAT (por el segundo salto hacia Internet).
- **Wi-Fi**: contraseña de la red actual (mostrar y copiar) y **código QR** para conectar un móvil.
- **Dispositivos en la red**: barrido de la subred (ping y tabla de vecinos, así aparecen también
  los que no responden al ping), nombre, IP, MAC, MAC privada de móviles, fabricante (consulta
  opcional en Internet solo del prefijo, con caché), nombres propios para cada dispositivo y marca
  de **nuevo** para los que no estaban la última vez.

### Fase 21 — Caja fuerte y privacidad (v0.21) ✅

Proteger los datos del cliente con lo que ya trae Windows, para que siga funcionando aunque se
desinstale AdminOps. Nueva área **Datos y familia** en la barra lateral.

- **Caja fuerte**: un .vhdx dinámico (API de discos virtuales de Windows, sin Hyper-V) cifrado con
  BitLocker XTS-AES 256 y contraseña. Se abre con doble clic desde Windows sin AdminOps. En la app:
  crear, abrir, cerrar (bloquea antes de expulsar y avisa si hay archivos abiertos), cambiar la
  contraseña, ver la clave de recuperación y guardarla en un archivo, añadir una existente, quitar o
  eliminar. La clave de recuperación no se guarda: se muestra y hay que copiarla o guardarla.
  Requiere Windows Pro para crearla.
- **Carpeta cifrada** (cualquier Windows): .zip AES-256 que abren 7-Zip, WinRAR o AdminOps. Se
  comprueba que se descifra bien antes de borrar la original (opcional, con borrado seguro).
- **Borrado seguro** de archivos y carpetas (sobrescritura y renombrado antes de borrar; nunca
  rutas del sistema), **vaciar el espacio libre** (cipher /w, cancelable) y checklist **antes de
  vender o donar el equipo** con accesos directos a cada paso.
- **Recuperar archivos borrados** con Windows File Recovery: se instala desde la Store, se elige
  unidad, destino (siempre otra), tipos de archivo o carpeta y búsqueda rápida o a fondo.
- **Control parental**: filtro de navegación por DNS (Cloudflare for Families, CleanBrowsing) en
  todos los adaptadores, sitios bloqueados en una sección propia del archivo hosts y **horario de
  uso** por usuario local (cuadrícula de 7×24, plantillas; API de Windows, independiente del idioma).
- Las contraseñas nunca se guardan ni aparecen en el registro de actividad.

### Fase 22 — La oficina completa (v0.22) ✅

Pasar de "un equipo" a "la red del cliente".

- **Inventario de equipos** (Soporte → Inventario): cada equipo que pasa por una sesión (o que se
  añade con un clic) queda en la ficha de su cliente con fabricante, modelo, serie, CPU, memoria,
  discos, gráfica, Windows, año aproximado (BIOS), TPM, nota de seguridad, batería, IP y MAC, y un
  veredicto **Bien / Mejorar / Renovar** con los motivos (SSD, memoria, batería, disco con fallos,
  Windows 10 sin soporte o sin TPM, antigüedad). Filtros, vista de todos los clientes y **exportar
  a Excel (CSV)**.
- **Acceso remoto** (Red → Acceso remoto): conectar por Escritorio remoto, abrir Asistencia rápida,
  **encender equipos por la red** (Wake-on-LAN); para este equipo, activar/desactivar recibir
  Escritorio remoto (con autenticación de red y firewall) y **prepararlo para Wake-on-LAN**
  (paquete mágico en el adaptador por cable e inicio rápido desactivado).
- Encender y conectar con un clic desde el **Inventario**, la **ficha del cliente** y
  **Dispositivos en la red**.
- **Carpetas compartidas** (Administración): qué se comparte, con qué permisos, archivos en uso y
  quién está conectado; compartir una carpeta (todos o un usuario, lectura o escritura, también
  permisos NTFS) y dejar de compartir; aviso y arreglo si la red es pública o el firewall bloquea.
- **IP duplicadas**: conflictos que Windows detectó en los últimos 30 días, con el dispositivo que
  usaba la misma IP.
- **Mapa de la red** guardado en la ficha del cliente desde Dispositivos en la red.

### Fase 23 — Ajustes, rendimiento y red a fondo (v0.23) ✅

- **Rendimiento** (medido): lectura del Panel un 23 % más ligera (solo CPU y memoria de cada
  proceso; discos cada 10 s) y diagnóstico un 15 % más rápido (5 procesos de PowerShell en lugar
  de 3; los que sobran se cierran tras 3 minutos sin uso). Buscador, asistente y «Acerca de» se
  cargan al abrirlos.
- **Ajustes en pestañas**: General, Apariencia, Navegación, Seguridad, Informes y cobros, Acerca de.
- **Personalización**: color de acento (6), tamaño de toda la interfaz (zoom), reducir animaciones,
  página al abrir, cada cuánto se actualiza el Panel, avisos al terminar tareas largas.
- **Navegación a medida**: reordenar y renombrar secciones, cambiar su icono, mover cada página a
  otra sección, crear secciones propias, ocultar páginas (siguen en Ctrl+K) y restablecer. Las
  páginas de versiones futuras aparecen solas en su sección de fábrica.
- **Bloqueo con PIN o contraseña** al abrir y por inactividad (Ctrl+L o el candado para bloquear
  al momento). Hash PBKDF2-HMAC-SHA256 con sal; espera tras 5 fallos. Si se olvida, se desbloquea
  con la contraseña de Windows de la cuenta (máximo 3 intentos cada 15 min, para no bloquear la
  cuenta de Windows).
- **Copia de la configuración**: exportar e importar ajustes, portales de Tickets y preferencias.
- **Dispositivos en la red a fondo**: tipo (router, PC, móvil, TV, Chromecast, impresora, cámara,
  NAS, consola, altavoz, domótica…), fabricante, modelo, nombre anunciado, sistema aproximado,
  servicios y puertos abiertos (UPnP/SSDP, mDNS/Bonjour, NetBIOS, puertos, título web y TTL, todo
  en la red local), filtros por tipo, ficha de cada dispositivo y **contraseña de la Wi-Fi** de la
  red actual (oculta hasta pulsar el ojo; por cable, la Wi-Fi guardada con el mismo nombre).
- **Herramientas**: 26 accesos nuevos (variables de entorno, opciones de rendimiento, nombre del
  equipo, BitLocker, historial de archivos, copia de Windows 7, apps del firewall, calibrar color,
  licencia de Windows, informes de batería, Wi-Fi y energía, solucionadores, proxy, VPN, redes
  Wi-Fi, gráficos por app, permisos de cámara y micrófono, inicio de sesión, sensor de
  almacenamiento, accesibilidad, teclado en pantalla, lupa, mapa de caracteres, Sandbox, Hyper-V).

### Fase 24 — Versión 1.0 (v1.0) ✅

Pulido para el uso diario.

- **Las páginas conservan su contenido**: al volver a Diagnóstico, Red, Tickets, etc. no se vuelve a
  cargar ni analizar nada; cada página se mantiene viva (con su scroll y sus tareas) hasta pulsar
  **Recargar** (o F5) o cerrar la app. Las páginas ocultas pausan sus lecturas en vivo, escuchas de
  teclado y vistas web. Como mucho 12 a la vez (se descarta la menos usada).
- **Atajos de teclado propios** para abrir cualquier página (Ctrl+Alt+letra, teclas F…), sin
  interferir con AltGr ni con los atajos de la app o de Windows.
- **Herramientas favoritas en el Panel** (las marcadas con ★ en Herramientas).
- **Punto de restauración** configurable: solo antes de cambios con riesgo, antes de cualquier
  cambio o nunca.
- **Iniciar con Windows**, minimizada, mediante una tarea programada con permisos de administrador
  (sin aviso de UAC en cada inicio de sesión).
- **Datos antiguos**: uso de historial, análisis, informes y registros; limpieza manual por
  antigüedad y automática al abrir. Nunca se borra lo que aún se puede deshacer, el último análisis ni
  el de una sesión en curso; los informes van a la papelera.
- **Aviso de versiones nuevas** consultando GitHub Releases (sin descargar ni instalar nada solo).
- **1.0.1**: la identificación de dispositivos cerraba la app (esperaba un futuro desde dentro del
  runtime de Tauri); ahora usa su propio hilo. Un fallo inesperado ya no cierra la app: se anota en el
  registro («Fallo interno») y la tarea termina con error.

### Fase 25 — Pulido (v1.1) ✅

- **Revisión de fallos**: ningún error inesperado cierra la app (se anota en el registro como «Fallo
  interno»); los bloqueos internos se recuperan tras un fallo en lugar de propagarlo (44 sitios);
  el test de velocidad ya no puede fallar con una medición inválida (NaN).
- **Vigilancia de errores de Windows**: mientras AdminOps está abierta, revisa el Visor de eventos
  cada minuto y avisa (campana en la cabecera, aviso en la app y notificación de Windows si está en
  segundo plano) de pantallazos azules, apagados inesperados, discos con sectores dañados o lentos
  (con el modelo del disco), errores del sistema de archivos, errores de hardware (WHEA), driver de
  vídeo reiniciado, conflictos de IP, servicios caídos, memoria agotada, fallos de Windows Update,
  procesador limitado por temperatura, programas que se cierran o se cuelgan, amenazas de Defender y
  poco espacio en el disco del sistema. Cada aviso explica qué es y qué hacer, con un botón a la
  página donde se arregla. Agrupa repeticiones y se puede desactivar en Ajustes.
- **Acceso remoto a fondo**: agenda de conexiones (Escritorio remoto, AnyDesk, RustDesk, TeamViewer)
  con cliente y notas; Escritorio remoto con opciones (pantalla completa, monitores, portapapeles,
  unidades, impresoras, sonido) y contraseña guardada en el Administrador de credenciales de Windows;
  prueba de conexión con la causa si falla; ID de este equipo en AnyDesk/RustDesk/TeamViewer,
  instalarlos y conectar a un ID; usuarios con permiso de Escritorio remoto.
- **Navegación a medida**: arrastrar y soltar secciones y páginas, renombrar cualquier página,
  selector visual de iconos (32), barra completa o solo iconos (con pestañas), a la izquierda o a la
  derecha, tres anchos y densidades, secciones desplegadas (solo la actual / las que deje abiertas /
  todas), qué hace el clic en una sección, pestañas bajo el título, **Favoritos** (★ también desde
  la barra) y **Recientes**, y ocultar buscador, aviso de sesión o pie.
- **Atajos**: 283 (antes 155), con grupos nuevos (problemas típicos, Escritorio remoto, Word,
  Outlook, PowerPoint, reuniones de Teams/Zoom/Meet) y más en Windows, Explorador, navegador, Excel
  y técnico. Una prueba comprueba que todos los que tienen botón «Probar» se pueden simular.
- Las barras de «Guardar» ya no dependen de la posición ni del ancho de la barra lateral.
- **1.1.1 — equipos de empresa (Sophos y similares)**: PowerShell sin `-EncodedCommand` ni base64 (el
  script viaja por la entrada estándar y el canal persistente usa líneas contadas); datos como literales
  de PowerShell y contraseñas como SecureString cifrada con DPAPI (nunca legibles en el script ni en el
  registro); el script de elementos sospechosos ya no contiene palabras de malware (se comparan en
  AdminOps); mensajes claros si el antivirus bloquea una acción o PowerShell está restringido por
  directiva, y aviso al arrancar si PowerShell no está disponible. Con otro antivirus, las firmas y los
  análisis de Defender en reposo ya no generan avisos falsos. Prueba de humo con 49 lecturas reales.
- **1.1.1 — soporte del día a día**:
  - **Solucionar problemas** (Equipo): eliges el síntoma (no hay Internet, Wi-Fi, no suena,
    Bluetooth, pantalla, no imprime, va lento, Windows Update) y AdminOps revisa en orden las causas
    típicas y ofrece la reparación de cada una.
  - **Reparar la red** en un clic (rápida o a fondo) con la comprobación antes y después.
  - **Control de la Wi-Fi**: estado real de la tarjeta con el error explicado, encender/apagar,
    reiniciar la tarjeta, desactivar el ahorro de energía y quitar adaptadores fantasma. Si la tarjeta
    falla, Dispositivos explica por qué no se ve la red ni la contraseña.
  - **Aviso automático de dispositivos que dejan de funcionar** (cada 5 min, solo los nuevos), con
    enlace a su reparación.
  - **Deshacer desde el aviso**: cada cambio suelto que se puede deshacer lo ofrece en el propio aviso.
  - **Línea de tiempo del equipo** (Historial): cambios de AdminOps, avisos, diagnósticos,
    actualizaciones, drivers, programas y apagados bruscos, por días.
  - La **búsqueda** (Ctrl+K) encuentra acciones y problemas: «no suena», «encender Bluetooth»,
    «reiniciar el driver de la gráfica», páginas de Configuración de Windows…
  - **Contactos** (Soporte): agenda del técnico que viaja con AdminOps (en el USB con el portable).
    - Vistas de tarjetas, tabla (columnas elegibles y ordenables) y directorio de extensiones;
      agrupar por empresa, área, etiqueta o letra; ordenar por nombre, empresa, extensión, uso o fecha.
    - Filtros rápidos (favoritos, más usados, con extensión o correo, sin completar, recientes),
      varias etiquetas con «todas/alguna», empresa, búsqueda con prefijos (`ext:`, `empresa:`, `#tag`,
      `-excluir`) y búsquedas guardadas. La vista y los filtros se recuerdan.
    - Etiquetas en chips con autocompletado y colores; renombrar, fusionar y borrar en todos.
    - Ficha lateral: varios teléfonos y correos, disponibilidad, sustituto («si no está…»), cliente
      enlazado, llamar, correo, chat o llamada de Teams y «Copiar tarjeta».
    - «Más usados», selección múltiple (Mayús para rangos) con etiquetas, favoritos, exportar y
      papelera; duplicados con fusión; papelera de 30 días; copia automática semanal (5 últimas).
    - Importa CSV (Excel, Outlook) y vCard; exporta CSV y vCard. Se busca desde Ctrl+K.
  - **Portable entre equipos**: las contraseñas de routers se cifran con una clave del USB (AES-256-GCM)
    en vez de DPAPI, así funcionan en todos los equipos; las antiguas se migran solas. Ajustes explica
    qué viaja contigo y qué es de cada equipo.

### v1.1.2 — Herramientas del técnico ✅

- **Trabajo en el equipo del cliente**
  - **Entrega rápida** (Informes): lo hecho hoy con AdminOps pasa a las observaciones y se genera el
    informe comparando con el primer diagnóstico del día.
  - **Tipos de visita** con su checklist (Mantenimiento, Equipo nuevo, Equipo lento, Sin Internet,
    Copia de datos; editables en Ajustes → Informes). Los puntos marcados como automáticos se tachan
    solos al hacer esa tarea con AdminOps (limpieza, actualizaciones, diagnóstico, dominio…).
  - **Recetas** (Administración): punto de restauración, quitar bloatware, instalar una lista de
    programas, perfil o ajustes sueltos, crear usuario, renombrar (admite `{serie}`), unir al dominio y
    diagnóstico final, de una vez. Las contraseñas se piden al ejecutar y nunca se guardan.
  - **Ficha del equipo** (Hardware): modelo, número de serie, hardware, Windows y licencia, TPM, red y
    usuario; copiar, pegar en Excel, CSV, enlace a la garantía (Dell, Lenovo, HP…) y al inventario
    de un cliente.
- **Conocimiento** (Soporte), viaja con AdminOps:
  - **Soluciones** «problema → lo que funcionó», con etiquetas; se guardan también al terminar una
    sesión de servicio y se buscan desde Ctrl+K.
  - **Plantillas de texto** con variables automáticas ({equipo}, {usuario}, {fecha}, {tecnico}…) y
    preguntas propias ({?Nombre}).
  - **Notas por equipo y por red**: aparecen solas en el Panel al volver a ese equipo o a esa red.
- **Red y oficina**
  - **Mapa de la oficina** (Dispositivos): función, responsable (de Contactos) y notas de cada
    dispositivo, por red; las IP se actualizan en cada búsqueda.
  - **Vigilancia de dispositivos clave**: avisa (campana y notificación) si una impresora, servidor o
    NAS marcado deja de responder, y cuando vuelve. Dos fallos seguidos para evitar falsas alarmas.
  - **Comprobar puestos** (Red): lista de equipos guardada; cuáles responden y, a fondo (WinRM o DCOM),
    disco libre, días sin reiniciar, días sin actualizar y usuario conectado. CSV y conexión por
    Escritorio remoto.
- **Enlaces entre todo**: el responsable de cada tema (red, impresoras, sistemas…) aparece en los
  avisos y en Solucionar problemas; los clientes muestran sus contactos; la sesión guarda quién
  pidió el trabajo y la ficha del contacto lista sus visitas.
- **Profesionalización**
  - **Copia de seguridad cifrada** (AES-256) de todo lo que viaja contigo, a otra unidad o a una
    carpeta en la nube, y restauración (lo sobrescrito se guarda antes).
  - **Seguridad de tus datos** (Ajustes): estado de la unidad, espacio, clave del USB, copias
    recientes y avisos (FAT32, sin copia en 30 días…).
  - **Modo auditoría** («Solo mirar» en la barra superior o `--auditoria`): se bloquean en el
    despachador de comandos todas las acciones que cambian el equipo; diagnósticos, informes y datos
    propios siguen funcionando.

- **Programas**
  - Desinstalación silenciosa para muchos más programas: además de MSI y los que la declaran,
    Inno Setup y NSIS (detectados por su desinstalador), y MSI mal registrados.
  - **Desinstalar en lote**: primero los silenciosos, luego los que necesitan su asistente, y una sola
    revisión de restos al final.
  - **Restos completos**: carpetas, accesos directos del escritorio y del menú Inicio y claves del
    registro (con copia .reg). Nunca la carpeta o la clave de otro programa instalado (un plugin que
    declaraba la carpeta del programa principal la proponía entera). También para entradas huérfanas.
  - Reparar programas MSI, filtros (con actualización, grandes, recientes, del usuario, necesitan
    asistente, huérfanas), ocultar componentes y runtimes, orden por editor e inventario en CSV.
  - Las actualizaciones de winget aparecen en la propia lista, con botón para actualizar.
  - Más rápido: la lista de winget (actualizaciones e instalados) se reutiliza unos minutos y va en
    su propio proceso sin frenar al resto de AdminOps; al actualizar no se vuelve a consultar; se
    indica el origen (winget/msstore). Detección de NSIS con caché (lista en ~40 ms).
  - **Actualizaciones ignoradas**: «no actualizar este programa», recordado en todos los equipos.
- **Correcciones**: Teams desde Contactos abre la app de Teams (protocolo msteams:) en vez de una
  carpeta; guardar un contacto recién usado ya no falla; posponer un mantenimiento vencido tampoco;
  las pruebas ya no llevan textos que el antivirus confunde con malware.

- **Calidad**
  - Tests del frontend con Vitest (`npm test`, también en CI): búsqueda y filtros de contactos,
    duplicados, vCard/CSV, responsable por tema, plantillas, checklist automática, emparejar programas
    con winget y errores.
  - Errores comprensibles: todas las llamadas al sistema pasan por un traductor (permisos, archivos en
    uso, sin espacio, sin red, equipo remoto…); los fallos internos se anotan en el registro técnico.
  - Checklist de pruebas manuales antes de publicar: `docs/PRUEBAS.md`.
- **Navegación reorganizada** por lo que hace el técnico: Inicio (Panel, Solucionar problemas,
  Sesión), Equipo, Mantener, Programas, Red, Oficina, Soporte y Datos. Páginas unidas: Actualizaciones
  (programas + Windows Update), Mi red (router + dispositivos), Puestos e inventario, y las
  Reparaciones dentro de Solucionar problemas. Los enlaces antiguos llevan a la pestaña nueva y las
  navegaciones personalizadas se conservan.
- **Experiencia**
  - Panel como centro de mando: «Qué hacer ahora» reúne diagnóstico, avisos, actualizaciones, espacio,
    datos y sesión en curso, por importancia y con su botón; «Revisión completa» en el propio Panel.
  - Indicador de tareas en la barra superior: lo que está en marcha (paso actual, tiempo, cancelar) y
    lo terminado, desde cualquier página.
  - Ayuda «?» junto al título de cada página.
  - Estados de carga y listas vacías iguales en toda la app.

- **Inventario web**: pestaña en Puestos e inventario para la web de inventario de la empresa, dentro
  de AdminOps como Tickets, con el panel «Datos del equipo» (cada dato se copia con un clic para el
  formulario). El inventario manual sigue igual.
- **Intranets con dominio** (como *.pgr.gob.do): el navegador interno entra con la cuenta de Windows
  sin pedir contraseña en los dominios de los portales del técnico y su dominio habitual.
- **Cuentas** (Oficina): sesión y estado del equipo (local, Microsoft, Entra ID, dominio), cuentas
  profesionales o educativas, cuentas de Microsoft y de Office, y credenciales guardadas. Asistente
  «pasar el equipo a una cuenta local»: crear administrador local → sacar el equipo de Entra ID
  (`dsregcmd /leave`, solo si ya hay un administrador local) → cerrar sesión. Desconectar cuentas
  profesionales añadidas, cerrar sesión de Office y borrar credenciales, todo en el diario.
- **Abrir enlaces sin permisos de administrador**: Teams, correo, llamadas, webs y Configuración se
  abren a través del escritorio de Windows (como el usuario), no con Explorer, que abría Documentos.
- **Portable descargado de Internet**: el lector de temperaturas ya no falla por la marca «descargado
  de Internet» de Windows; ningún error muestra la carpeta del usuario.

- **Diagnóstico más rápido y con datos** (medido: la auditoría de seguridad pasó de 7,5 s a 2,3 s):
  - La auditoría de seguridad se reparte en tres consultas en paralelo (registro, Defender, BitLocker).
  - BitLocker no se consulta sin administrador, porque Windows tardaba 5 s en no devolver nada. Con
    administrador se reutiliza durante 10 minutos.
  - Defender y BitLocker se precargan al abrir AdminOps.
  - Inventario de hardware, SMART y prueba de memoria se reutilizan 5 minutos; la lista de winget,
    10 minutos. «A fondo» vuelve a leerlo todo, y la «Revisión completa» del Panel siempre lo hace.
  - Cada sección se pinta en cuanto termina, sin esperar a las demás.
  - Opción «Diagnosticar al abrir AdminOps» (Ajustes → General).
- **Arreglar en el sitio**: los hallazgos llevan su botón «Arreglar», también en el Panel, que
  ejecuta el ajuste o la reparación sin salir de la página. «Arreglar todo lo seguro» aplica de una
  vez solo lo que no cambia el comportamiento de Windows ni borra archivos del usuario.
- **Qué cambió desde el análisis anterior**: problemas nuevos (marcados «Nuevo») y resueltos. Se
  compara sin tener en cuenta las cifras: «poco espacio (8 %)» y «(7 %)» son el mismo problema. El
  informe antes/después usa la misma comparación.
- **Ajustes de Windows**: Limpieza, Rendimiento, Privacidad y Servicios en una página con pestañas y
  un buscador que mira en las cuatro a la vez (sin tildes: «telemetria» encuentra «Telemetría»).
- **Preparar equipos** (antes «Recetas»): pestañas Plantillas y Perfiles de ajustes.
- **Puestos en lote**: marcar varios equipos y hacer lo siguiente sobre todos, con el resultado de
  cada uno y la acción anotada en el Historial:
  - reiniciar, con aviso al usuario y 2 minutos de margen;
  - cancelar ese reinicio;
  - actualizar directivas;
  - enviar un mensaje en pantalla;
  - abrir el Escritorio remoto (hasta 6 a la vez).
- **Estado entre visitas**: en la ficha del cliente, qué cambió en cada equipo desde la visita
  anterior: problemas, nota de seguridad, espacio, programas de inicio, actualizaciones, arranque,
  RAM, batería y hardware. Se descartan las diferencias pequeñas. Al empezar una sesión se compara
  con cómo está ese equipo ahora. «Evolución entre visitas» ya no compara visitas de equipos
  distintos entre sí.
- **Rendimiento de AdminOps** (Ajustes → Rendimiento):
  - pasos del arranque del programa;
  - cuándo se abre la ventana, cuándo se pinta la primera página y cuándo está lista;
  - por cada página, en su primera visita: descarga del código, pintado y tiempo hasta que
    terminan sus consultas;
  - las consultas al backend más lentas.

  El resumen del arranque queda en el registro técnico y se puede copiar para compararlo entre
  versiones.

- **Portales más rápidos y con barra de navegador** (Tickets, Inventario web, Correo):
  - Se precarga en segundo plano el último portal usado de cada tipo (Ajustes → General → «Precargar
    los portales»). Las vistas se mantienen vivas y la barra ya no parpadea al volver.
  - Barra completa: atrás y adelante según el historial real, recargar o detener, página inicial,
    dirección editable con candado y botón Copiar, zoom recordado por portal, buscar en la página,
    imprimir o guardar como PDF, y descargas con Abrir y «Mostrar en la carpeta». De cada descarga
    solo se ve el nombre del archivo, nunca la ruta.
  - Si la web no carga, en vez de la página en blanco se explica el motivo (sin red o sin VPN,
    servidor caído, certificado no válido…) con Reintentar, Página inicial y Abrir en el navegador.
  - Barra de progreso mientras carga. El registro anota cuánto tarda cada vista en crearse y en
    cargar la primera vez.
  - Por portal:
    - inicio de sesión guardado y cifrado (viaja en el USB en portable), que se rellena solo;
    - sesión privada, que no guarda nada en el equipo y se cierra al salir;
    - ventanas emergentes en ventana aparte.
- **Correo** (Soporte): Outlook del trabajo (Microsoft 365) u Outlook.com dentro de AdminOps.
  - Con la cuenta guardada, AdminOps rellena el inicio de sesión de Microsoft; solo queda la
    verificación en el móvil.
  - Sesión privada recomendada y activada por defecto.
  - Botones Redactar y Cerrar sesión; contador de no leídos si Outlook lo pone en el título.
  - Redactar al instante: con Outlook abierto pulsa su propio botón «Correo nuevo» en vez de
    recargar toda la web con el enlace de redactar (solo se usa el enlace si el botón no aparece o
    si el mensaje va con destinatario, asunto o texto).
- **Portales: la red, no AdminOps**:
  - Sin conexión, una franja lo avisa y el portal que falló se recarga solo al volver la red.
  - Si una página tarda más de 10 segundos, se avisa de que es la conexión o el servidor de la
    web, con Recargar y Abrir en el navegador.
  - Al volver a Tickets, Inventario o Correo, la vista aparece al momento: la lista de portales
    ya leída se guarda en memoria.
  - Los mensajes y adjuntos se abren en su propia ventana.
  - «Escribir un correo» en Contactos lo abre en el Correo de AdminOps.

- **Preparar equipos** (antes «Recetas»): cada receta es ahora una plantilla de preparación.
- **Usuarios y Cuentas**: accesos directos a las herramientas de Windows que completan la página:
  Usuarios y grupos locales, Cuentas de usuario, Perfiles de usuario, Directiva de seguridad local,
  Administrador de tareas, Administración de equipos y Administrador de credenciales. Lo que ya
  había sigue igual.
- **Agenda** (Soporte): visitas de mantenimiento con día, hora, duración, equipos y notas.
  - Aviso de Windows 30 minutos antes de cada visita, y un resumen del día al abrir AdminOps.
  - Lista de clientes a los que ya les toca mantenimiento, con «Agendar».
  - Desde cada visita: «Empezar» abre la sesión con el cliente ya elegido; recordatorio al cliente
    por el Correo; marcar como hecha, reprogramar o cancelar.
  - Las visitas de hoy y los mantenimientos vencidos aparecen en «Qué hacer ahora» del Panel.
- **Plantilla de informe por cliente** (en su ficha):
  - formato (cliente o técnico), que la sesión y la página Informe proponen solos;
  - texto de presentación al principio del informe;
  - destinatarios extra y asunto y mensaje del correo, con {cliente}, {contacto}, {numero},
    {fecha}, {equipo}, {empresa} y {tecnico}.
- **Enviar el informe con el Correo de AdminOps**: deja el mensaje escrito y abre la carpeta del PDF
  para arrastrarlo (el correo web no permite adjuntar el archivo automáticamente).

- **Barra lateral**: botón para acoplarla (solo iconos) y volver a abrirla, y ancho ajustable
  arrastrando su borde (doble clic vuelve al ancho de Ajustes). El ancho se guarda.
- **Menos secciones y menos páginas** (de 8 áreas y 44 páginas a 7 y 30):
  - **Aplicaciones** (antes «Programas»): Actualizar, Windows Update, Instalar, Desinstalar y
    Bloatware en pestañas.
  - **Ajustes de Windows** se lleva «Inicio de Windows» como pestaña; el área «Mantener» desaparece
    y la página pasa a «Equipo».
  - **Herramientas y atajos**: las utilidades de Windows y los atajos de teclado, juntos.
  - **Sesión de servicio** se lleva el «Informe» como pestaña.
  - **Datos del equipo**: Copia de datos, Caja fuerte, Borrado seguro, Recuperar archivos y Control
    parental en una sola página.
  - **Cuentas y dominio**: las cuentas del equipo y el dominio de la empresa, juntos.
  - **Impresoras y carpetas**: lo que la oficina comparte, en una sola página.
  - Los enlaces antiguos siguen funcionando: cada uno lleva a su pestaña nueva.
- **Soluciones** (antes «Conocimiento»): AdminOps trae 24 soluciones probadas paso a paso para los
  problemas de soporte más habituales (sin Internet, impresora atascada, Windows Update que falla,
  perfil temporal, pantallazos, Outlook que pide la contraseña, disco al 100 %, relación de
  confianza del dominio…). Se buscan y se copian igual que las propias, salen en Ctrl+K y, para
  cambiar una, se duplica.
- **Ajustes rediseñados**:
  - buscador que encuentra cualquier ajuste por su nombre o por lo que hace, sin tildes, y lleva a
    su sección resaltando la fila;
  - secciones en una columna a la izquierda, cada una con lo que hace;
  - sección nueva **Portales y correo**: precarga, zoom por defecto, dominio de la empresa y, por
    cada portal, sesión privada y entrar solo, con aviso si ninguno usa sesión privada.
- **WebView2**: se quita `--in-process-gpu`. Se puso en la 0.7 para ahorrar un proceso al arrancar,
  cuando la única vista era la interfaz; con los portales y el Correo dentro, hacía que todo el
  trabajo gráfico pasara por el proceso principal y provocaba tirones con webs pesadas.

- **Arreglos y rendimiento con datos reales** (medido en un equipo de trabajo):
  - El arranque (1,6 s) resultó ser casi todo de WebView2: los pasos propios suman 65 ms, así que
    **no** se partió `api.ts`; no habría ganado nada.
  - **El diagnóstico esperaba a winget** (84 s en ese equipo) y por eso tardaba 82 s. Ahora usa lo
    que tenga en caché y actualiza detrás: el diagnóstico deja de depender de winget.
  - **La información de la red** (19,9 s) sale del PowerShell compartido a su propio proceso.
  - **Las temperaturas** se comparten entre el Panel y Hardware (eran 39 lecturas de 300 ms).
  - Las **vistas web sin usarse 30 minutos se cierran** (cada una es un proceso).
  - Los **portales se recargan solos al volver la red** (VPN, cable, Wi-Fi).
  - Elegir un ancho de barra lateral en Ajustes ahora manda sobre el ajustado arrastrando.
  - `npm run setversion 1.1.5` cambia la versión en los cuatro sitios a la vez.
- **Avisos de Windows que llevan a su sitio**: Windows no dice qué aviso se ha pulsado, pero al
  pulsarlo pone AdminOps delante; si el aviso es reciente, la app abre la campana con lo pendiente.
- **Barras de pestañas sin barra de desplazamiento** y barra general discreta (solo al pasar por
  encima).
- **Etiquetas plegables** (Soluciones y Contactos): con muchas, se ve una fila y «Ver todas».
- **Diagnóstico: se pliega lo que está bien.** Las tarjetas sin problemas se quedan en una línea con
  «Sin problemas» y el detalle a un clic. Se puede desactivar con «Plegar lo que está bien».
- **Redactar un correo**: sin datos que rellenar se pulsa el botón propio de Outlook (aspecto de
  siempre); con destinatario o asunto, su enlace de redactar abre en una ventana propia.

- **Estado del equipo**: Diagnóstico, Hardware, Seguridad e Historial en una página con pestañas.
  Con ello «Equipo» queda en cuatro entradas y la barra lateral en 7 secciones, en este orden:
  Inicio, Equipo, Soporte, Red, Aplicaciones, Administración (antes «Oficina») y Datos.
- **Cada pestaña sabe si se ve**: antes una página con pestañas se creía visible entera, así que lo
  que hubiera en marcha en una pestaña oculta (temperaturas, métricas) seguía trabajando. Arreglado
  en `TabPanels`, así que vale para todas las páginas con pestañas.
- **Catálogo de programas**: de 72 a 113, con categoría **Drivers y utilidades del fabricante**
  (Intel, Dell, Lenovo, MSI, ASUS, DisplayLink, Logitech, Corsair, Razer y Snappy Driver Installer)
  y dos listas nuevas. Los 74 ids añadidos se comprobaron uno a uno contra winget: 7 no existían y
  se corrigieron o se quitaron. winget no distribuye drivers sueltos: lo que se instala son las
  utilidades oficiales que los buscan.
- **Espacio en disco, para soporte**:
  - **«Qué puedes liberar»**: temporales de Windows y del usuario, caché de Windows Update,
    papelera, `Windows.old`, hibernación y Descargas, medidos de verdad, con verde (se puede borrar)
    o naranja (míralo antes).
  - **Archivos grandes sin usar en más de un año**, con cuánto hace que se tocaron.
  - **Enviar a la papelera** lo que marques, sin salir de la app y pudiendo recuperarlo; queda en el
    Historial y el modo auditoría lo bloquea.

- **Las soluciones se ejecutan, no solo se leen.** Cada paso de las 24 soluciones que trae AdminOps
  lleva su botón cuando la app sabe hacerlo: vaciar la cola de impresión, reparar Windows Update,
  limpiar el DNS, SFC y DISM, reiniciar el audio, reparar un perfil temporal, sincronizar la hora,
  analizar con Defender… También abre la herramienta de Windows o la página donde se hace. Un test
  comprueba que los 24 juegos de botones apuntan a ids que existen de verdad.
- **El diagnóstico lleva a la solución**: cada hallazgo ofrece «Cómo se arregla» y abre los pasos.
  También en «Qué hacer ahora» del Panel, para los hallazgos sin acción directa.
- **Comparar los equipos de un cliente**: «PC-CONTA arranca 3 veces más lento que el resto de la
  oficina». Compara arranque, memoria, espacio libre y nota de seguridad con la mediana de los demás
  equipos, a partir de las cifras que ya guardaba cada visita. Solo datos técnicos del equipo, y hacen
  falta al menos tres para que la mediana signifique algo.
- **Dos modos, elegidos en la bienvenida y cambiables en Ajustes**:
  - **Modo técnico**: AdminOps completo, con el aviso de que toca registro, servicios, usuarios y
    arranque, y de que para cambiar el equipo hay que abrirlo como administrador.
  - **Modo usuario**: solo Panel, Estado del equipo, Espacio, Solucionar problemas, Acceso remoto y
    Ajustes. No se ven clientes, contactos, tickets ni correo del técnico. Con PIN puesto, volver al
    modo técnico lo pide. Es un modo de la interfaz, no una barrera de seguridad, y así se dice.
- **Bienvenida nueva**: modo, datos del técnico, tickets, correo (Outlook en un clic) y dominio, con
  atrás, saltar y un resumen final de lo que conviene saber.
- **Ayuda al día**: los textos del «?» que quedaron desfasados tras las fusiones, corregidos, y
  **ayuda propia en cada una de las 34 pestañas**, que antes no tenían ninguna.
- **Comprobación antes de instalar**: red, espacio libre, winget y permisos. Avisa antes en vez de
  dejar el equipo a medio hacer.
- **Arranque instrumentado**: se mide la carpeta de datos de WebView2 y el momento justo antes de
  crear la ventana, para saber con datos cuánto es de AdminOps y cuánto de WebView2.

### v1.1.5 — Teams, arranque y portales que no estorban ✅

- **Teams dentro de AdminOps** (Soporte → Teams): chats, equipos y reuniones sin instalar Teams en el
  equipo del cliente. Se usa Teams en la web, que es la versión que funciona en cualquier equipo y no
  deja nada instalado; la cuenta se guarda cifrada como la del Correo y el inicio de sesión se rellena
  solo. Del trabajo (Microsoft 365) o personal, con sesión privada recomendada en equipos ajenos.
  Solo navega por los dominios de Teams y de Microsoft: cualquier otro enlace se abre en el navegador.
  Para hablar en una reunión, Windows pide permiso de micrófono y cámara la primera vez.
  Desde **Contactos**, «Teams» abre el chat aquí dentro si hay Teams configurado; si no, en la
  aplicación de Teams del equipo, como antes.
- **Los portales dejan de estorbar cuando no se ven.** Un portal oculto (o precargado) se duerme:
  deja de consumir procesador y batería, igual que una pestaña de fondo del navegador. Era el motivo
  de que el Correo, con Outlook abierto, pusiera lenta toda la aplicación aunque estuvieras en otra
  página. Se despierta solo al volver a él.
- **Portales más ligeros al moverse**: colocar la vista hay que pedírselo al hilo de la ventana, y al
  arrastrar el borde se mandaban decenas de peticiones por segundo. Ahora va una por fotograma como
  mucho y las que no cambian nada no se mandan. Además, el estado de la página (título, historial)
  solo repinta la interfaz cuando cambia de verdad: Outlook lo repetía sin parar.
- **La ventana ya no se abre en negro.** Se enseña cuando la interfaz tiene algo pintado, con un
  «Abriendo AdminOps…» mientras tanto, y si WebView2 no arranca en 12 segundos se recarga la vista
  sola, en vez de tener que cerrar y volver a abrir.
- **Portable: primer arranque arreglado.** La carpeta de datos de WebView2 del USB se crea y se
  comprueba antes de dársela a WebView2; si el USB viene protegido contra escritura o el antivirus
  del cliente bloquea la primera creación, se usa la del equipo en lugar de quedarse la ventana en
  negro. Era la causa de que la primera vez hubiera que cerrar y volver a abrir.
- **Arranque más despejado**: las consultas pesadas de Defender y BitLocker, la limpieza automática y
  el calentamiento de PowerShell se apartan unos segundos para no competir con la primera pantalla.
- **Asistencia rápida sin el aviso en inglés de Windows**: se comprueba antes si el equipo la tiene
  (app de la Store en Windows 11, `quickassist.exe` en Windows 10). Si no está, se abre su ficha de la
  Microsoft Store y se explica en español, en vez del «This file does not have an app associated…».
- **«Volver a ver la bienvenida»** en Ajustes → General, junto a la página de inicio.

### Ideas nuevas (propuestas, sin fecha)

**Taller y órdenes de trabajo**
- **Órdenes de taller**: al recibir un equipo, una orden con número, estado del equipo, accesorios
  entregados y lo que dice el cliente. Estados: recibido → en reparación → esperando pieza → listo →
  entregado. Etiqueta imprimible para pegar en el equipo y aviso al cliente cuando esté listo.
- **Fotos del equipo**: al recibirlo y al entregarlo (golpes, pantalla, puertos), adjuntas a la orden
  y al informe, para evitar discusiones.
- **Repuestos**: tu stock de piezas (discos, RAM, cargadores) con coste y precio. Lo que se usa en
  una visita se descuenta y pasa al recibo; aviso cuando queda poco.

**Clientes con contrato**
- **Bolsa de horas**: horas contratadas por cliente, consumo en cada visita (sale del tiempo de la
  sesión) y aviso cuando se acaban.
- **Informe mensual del cliente**: todas las visitas del mes, equipos atendidos, horas y problemas
  que se repiten, en un solo PDF con tu marca.
- **Incidencias que se repiten**: por cliente y por equipo (la impresora de recepción, el portátil
  de contabilidad), para proponer una solución de fondo o una renovación.

**Cobros y números del técnico**
- **Cobros pendientes**: estado de cada recibo (pendiente, pagado, parcial), recordatorio de pago
  por el Correo y lista de lo que te deben.
- **Resumen del mes**: visitas, horas, ingresos, clientes nuevos y lo pendiente de cobro. Se exporta
  a Excel para la contabilidad.

**Entrega al cliente**
- **Pantalla de entrega**: una vista limpia para enseñar al cliente en su propio equipo el antes y
  el después (espacio, arranque, seguridad) y que firme ahí mismo.

### Fase 26 — Negocio (aparcada)

> **Aparcada**: cobrar no es el plan ahora. Se deja escrita por si algún día lo es.

**Prioridad: media.** Lo que ayuda a cobrar y a que el cliente vuelva.

- **Estadísticas**: ingresos por mes, servicios más vendidos y clientes frecuentes (de los recibos).
- **Enviar el informe por WhatsApp** con el número del cliente y el mensaje listo.
- **Avisos al abrir la app**: mantenimientos vencidos y garantías que terminan.
- **Asistencia remota** apuntada en la sesión (código de Asistencia rápida o AnyDesk).

### Fase 27 — Plantillas de preparación (más adelante)

**Prioridad: media.** Automatizar lo que el técnico repite en cada equipo.

- **Plantillas**: secuencias guardadas de pasos (diagnóstico → limpieza → perfil → programas → copia de
  drivers → informe) que se ejecutan con un clic y muestran el progreso de cada paso.
- Plantillas incluidas: "Equipo nuevo", "PC lento", "Después de un virus", "Antes de formatear".
- **Exportar e importar plantillas** para compartirlas con otros técnicos.

**Terminado cuando:** preparar un equipo nuevo es elegir una plantilla y esperar.

### Fase 28 — Distribución (más adelante)

- **Actualización automática** con `tauri-plugin-updater` y GitHub Releases (clave de firma propia y
  gratuita: la app solo instala actualizaciones publicadas por ti). La 1.0 ya avisa de versiones nuevas.
- **Releases automáticas** desde la CI al crear una etiqueta `vX.Y.Z` (instalador + portable).
- **Inglés** completo (interfaz, catálogos e informe) y selector de idioma.
- **Página de descarga** en GitHub Pages con capturas, novedades y la nota sobre SmartScreen.

### Fase 29 — Preparación comercial (aparcada)

> **Aparcada**: cobrar no es el plan ahora. Se deja escrita por si algún día lo es.

**Prioridad: a decidir** según cómo se quiera vender.

- **Modelo gratis + Pro**: lo esencial gratis; informes con marca propia, plantillas de preparación, clientes ilimitados
  y la oficina completa en Pro.
- **Licencias sin servidor**: clave firmada (Ed25519) que la app verifica sin conexión, asociada al
  técnico y no al equipo, para que funcione en modo portable.
- **Términos de uso y privacidad** (EULA) en el instalador y en Acerca de.
- **Canal de soporte**: el paquete de soporte (Fase 14) se envía por correo con un clic.
