// Soluciones que AdminOps trae de serie: los problemas de soporte que más se
// repiten, con los pasos que de verdad los resuelven, en orden de lo más rápido
// y menos invasivo a lo más pesado.
//
// No se guardan en la biblioteca del técnico: viven aquí y se mezclan con las
// suyas al mostrarlas. Así se pueden mejorar en cada versión sin tocar sus datos.
// Para cambiar una, se duplica y la copia ya es suya.
import type { Solution } from "./api";

export const BUILTIN_PREFIX = "adminops:";

/** ¿Es una solución de las que trae AdminOps (no del técnico)? */
export const isBuiltin = (id: string) => id.startsWith(BUILTIN_PREFIX);

type Builtin = { id: string; title: string; problem: string; solution: string; tags: string[] };

const CATALOG: Builtin[] = [
  {
    id: "sin-internet",
    title: "Conectado al Wi-Fi pero sin Internet",
    problem: "El icono de red muestra conexión (o el cable está puesto) pero ninguna web carga. Suele pasar tras un corte de luz, al cambiar de router o cuando el equipo se trae de otra oficina.",
    solution: `1. Comprueba si es solo este equipo: mira desde el móvil en la misma red. Si tampoco va, el problema es el router o la línea.
2. Reinicia la pila de red (AdminOps → Red → Velocidad y diagnóstico → Reparar la red). Hace lo mismo que estos comandos como administrador:
   ipconfig /release
   ipconfig /renew
   ipconfig /flushdns
   netsh winsock reset
   netsh int ip reset
3. Reinicia el equipo (los dos últimos comandos solo surten efecto tras reiniciar).
4. Si sigue igual, mira si tiene IP correcta: ipconfig. Una IP 169.254.x.x significa que no llegó respuesta del router (DHCP): reinicia el router o revisa el cable.
5. Prueba a poner DNS fijos (AdminOps → Red → DNS): 1.1.1.1 y 8.8.8.8. Si con eso funciona, el DNS del router es el que falla.
6. Si hay antivirus de empresa o VPN, desactívalo un momento: los filtros de red son causa habitual.`,
    tags: ["red", "internet", "wifi", "dns"],
  },
  {
    id: "impresora-no-imprime",
    title: "La impresora no imprime y los trabajos se quedan en la cola",
    problem: "Se manda a imprimir, el documento entra en la cola y ahí se queda. A veces pone «Error» o «Eliminando» y no se puede quitar.",
    solution: `1. Vacía la cola desde AdminOps → Oficina → Impresoras → Vaciar cola. Si no se deja:
2. Como administrador, para el servicio, borra los trabajos y vuelve a arrancarlo:
   net stop spooler
   del /Q /F /S "%systemroot%\\System32\\spool\\PRINTERS\\*.*"
   net start spooler
3. Comprueba que la impresora esté en línea y no «en pausa» ni «usar impresora sin conexión» (clic derecho sobre ella en Dispositivos e impresoras).
4. Si es de red, haz ping a su IP. Si no responde, es problema de red o está apagada; si responde pero no imprime, reinicia la impresora.
5. Si imprime desde otro equipo, el problema es el driver: quítala y vuelve a añadirla con el driver del fabricante (no el genérico).
6. Revisa que esté puesta como predeterminada y que Windows no la cambie solo (Configuración → Bluetooth y dispositivos → Impresoras → desactivar «Permitir que Windows administre mi impresora predeterminada»).`,
    tags: ["impresora", "cola", "spooler"],
  },
  {
    id: "windows-update-falla",
    title: "Windows Update da error y no instala nada",
    problem: "Las actualizaciones fallan una y otra vez con un código de error, se quedan al 0 % o vuelven a aparecer tras instalarse.",
    solution: `1. Reinicia el equipo y prueba otra vez: muchas veces solo falta terminar una instalación pendiente.
2. AdminOps → Solucionar problemas → Reparar Windows Update. Hace lo siguiente como administrador:
   net stop wuauserv & net stop bits & net stop cryptsvc
   ren %systemroot%\\SoftwareDistribution SoftwareDistribution.old
   ren %systemroot%\\System32\\catroot2 catroot2.old
   net start wuauserv & net start bits & net start cryptsvc
3. Reinicia y vuelve a buscar actualizaciones.
4. Si sigue fallando, repara los archivos del sistema (AdminOps → Solucionar problemas → Reparaciones):
   DISM /Online /Cleanup-Image /RestoreHealth
   sfc /scannow
5. Comprueba que quede espacio libre en C: (hacen falta varios GB) y que la fecha y hora sean correctas.
6. Si es una actualización concreta la que falla siempre, descárgala del Catálogo de Microsoft Update por su número KB e instálala a mano.`,
    tags: ["windows update", "actualizaciones", "error"],
  },
  {
    id: "equipo-lento",
    title: "El equipo va muy lento desde hace días",
    problem: "Tarda en arrancar, las ventanas se abren despacio y todo se queda «pensando». No hay un error concreto.",
    solution: `1. Lo primero, mira el disco: AdminOps → Diagnóstico. Si el SMART avisa o el disco es mecánico (HDD), esa es la causa casi siempre. Cambiar a SSD lo arregla de verdad; lo demás son parches.
2. Mira qué consume ahora mismo (AdminOps → Procesos): ordena por disco y por memoria. Antivirus duplicados, OneDrive sincronizando o un proceso al 100 % explican el problema.
3. Quita lo que arranca solo y no hace falta (AdminOps → Ajustes de Windows → Inicio de Windows).
4. Comprueba la memoria: si está al 90 % en reposo con 4 GB, el equipo pide más RAM.
5. Libera espacio si a C: le queda menos del 15 % (AdminOps → Ajustes de Windows → Limpieza).
6. Descarta temperatura: si el ventilador va a tope y la CPU pasa de 90 °C, hay que limpiar y cambiar la pasta térmica (AdminOps → Hardware → Temperaturas).
7. Solo después de lo anterior, aplica los ajustes de rendimiento.`,
    tags: ["lento", "rendimiento", "disco"],
  },
  {
    id: "pantallazo-azul",
    title: "Pantallazos azules que se repiten",
    problem: "El equipo se reinicia solo con una pantalla azul, a veces con un código (VIDEO_TDR_FAILURE, MEMORY_MANAGEMENT, IRQL_NOT_LESS_OR_EQUAL…).",
    solution: `1. Apunta el código: AdminOps → Diagnóstico → Estabilidad lo muestra con su explicación. El código dice por dónde empezar.
2. Si los pantallazos empezaron tras instalar algo (driver, actualización, programa), desinstálalo o usa un punto de restauración anterior.
3. Driver gráfico (VIDEO_TDR_FAILURE, nvlddmkm, atikmdag): desinstálalo por completo y pon el del fabricante, no el de Windows Update.
4. Memoria (MEMORY_MANAGEMENT, PAGE_FAULT): prueba la RAM (AdminOps → Hardware → Prueba de memoria) y, si hay dos módulos, prueba con uno cada vez.
5. Disco (CRITICAL_PROCESS_DIED, INACCESSIBLE_BOOT_DEVICE): revisa el SMART y pasa chkdsk C: /f /r.
6. Si no hay patrón claro, repara el sistema: DISM /Online /Cleanup-Image /RestoreHealth y después sfc /scannow.
7. Temperatura y fuente de alimentación: si se reinicia bajo carga (juegos, render), sospecha de calor o de la fuente.`,
    tags: ["bsod", "pantallazo", "estabilidad", "driver"],
  },
  {
    id: "perfil-temporal",
    title: "«Iniciaste sesión con un perfil temporal»",
    problem: "Al entrar, el escritorio aparece vacío, sin los documentos ni los programas del usuario, y Windows avisa de que la sesión es temporal. Al reiniciar vuelve a pasar.",
    solution: `1. No trabajes en esa sesión: lo que se guarde ahí se pierde al cerrarla.
2. Cierra sesión y vuelve a entrar. A veces basta, porque el perfil estaba en uso.
3. Si sigue, entra con otra cuenta administradora y abre el registro (regedit):
   HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\ProfileList
4. Busca la clave del SID del usuario. Si ves dos, una acabada en .bak:
   - A la que NO tiene .bak, cámbiale el nombre añadiéndole .old
   - A la que tiene .bak, quítale el .bak
5. En esa clave (ya sin .bak) comprueba: RefCount = 0 y State = 0 (ambos en decimal).
6. Reinicia y entra con el usuario: debería recuperar su perfil.
7. Si la carpeta C:\\Users\\<usuario> ya no existe o está dañada, crea un usuario nuevo y copia los datos de la carpeta antigua (AdminOps → Datos del equipo → Copia de datos).`,
    tags: ["perfil", "temporal", "usuario", "registro"],
  },
  {
    id: "outlook-pide-contrasena",
    title: "Outlook pide la contraseña una y otra vez",
    problem: "Outlook abre la ventana de la contraseña sin parar aunque se escriba bien, o dice que no puede conectar con el servidor.",
    solution: `1. Comprueba que la cuenta funciona en el correo web con la misma contraseña. Si ahí tampoco entra, la contraseña caducó o la cuenta está bloqueada.
2. Borra las credenciales guardadas: AdminOps → Oficina → Cuentas → Credenciales guardadas, y quita las que empiecen por MicrosoftOffice o MS.Outlook. También en Panel de control → Administrador de credenciales → Credenciales de Windows.
3. Cierra Outlook del todo (comprueba en el Administrador de tareas que no quede OUTLOOK.EXE) y vuelve a abrirlo: pedirá la contraseña una vez y la guardará bien.
4. Si la cuenta tiene verificación en dos pasos, hay que confirmarla en el móvil al entrar.
5. Si persiste, cierra la sesión de Office en todos los programas (AdminOps → Oficina → Cuentas → Cerrar sesión de Office) y vuelve a entrar.
6. Como último paso, crea un perfil de Outlook nuevo (Panel de control → Correo → Mostrar perfiles → Agregar).`,
    tags: ["outlook", "correo", "contraseña", "office"],
  },
  {
    id: "disco-100",
    title: "El disco está al 100 % y el equipo se congela",
    problem: "En el Administrador de tareas el disco marca 100 % constantemente, aunque no se esté haciendo nada, y el equipo responde a trompicones.",
    solution: `1. Mira qué proceso lo usa (AdminOps → Procesos, ordenado por disco). Lo más común:
   - Antimalware Service Executable: análisis del antivirus. Espera a que acabe o programa el análisis fuera del horario.
   - SearchIndexer: indexación. Si el equipo es antiguo, desactiva el indexado (AdminOps → Ajustes de Windows → Servicios).
   - SysMain / Superfetch: en discos mecánicos suele empeorar; se puede desactivar.
   - OneDrive / copias de seguridad: sincronización inicial, déjala terminar.
2. Si ningún proceso destaca pero el disco sigue al 100 %, sospecha del disco: revisa el SMART (AdminOps → Diagnóstico → Discos). Un disco con sectores pendientes da exactamente este síntoma.
3. Comprueba el espacio libre: por debajo del 10 % Windows trabaja mucho peor.
4. En discos mecánicos, comprueba que no esté en modo PIO ni con el driver genérico (Administrador de dispositivos → Controladoras IDE ATA/ATAPI).`,
    tags: ["disco", "100%", "lento", "smart"],
  },
  {
    id: "sin-sonido",
    title: "No hay sonido",
    problem: "No se oye nada por los altavoces ni por los auriculares, o el icono de volumen tiene una cruz roja.",
    solution: `1. Comprueba lo obvio: volumen, silencio y que el cable esté en el conector correcto (verde para altavoces).
2. Elige la salida correcta: clic en el icono de volumen → flecha, y prueba cada dispositivo de la lista. Con HDMI o DisplayPort conectados, Windows manda el sonido al monitor.
3. Mezclador de volumen (clic derecho en el icono): comprueba que el programa concreto no esté silenciado.
4. Reinicia el servicio de audio como administrador:
   net stop audiosrv & net start audiosrv
5. Administrador de dispositivos → Controladoras de sonido: si hay un aviso amarillo, desinstala el dispositivo marcando «Eliminar el software de controlador» y reinicia, Windows lo reinstala.
6. Si el equipo es de marca (HP, Lenovo, Dell), instala el driver de audio de su web: el genérico de Windows a veces no saca sonido por los altavoces internos.
7. Prueba unos auriculares: si por ellos se oye y por los altavoces no, el altavoz o su conector están rotos.`,
    tags: ["sonido", "audio", "driver"],
  },
  {
    id: "pantalla-negra-sesion",
    title: "Pantalla negra después de iniciar sesión",
    problem: "Se escribe la contraseña, y en vez del escritorio aparece una pantalla negra con o sin cursor del ratón.",
    solution: `1. Pulsa Ctrl+Alt+Supr → Administrador de tareas → Archivo → Ejecutar nueva tarea → escribe explorer.exe y acepta. Si aparece el escritorio, el problema es el Explorador.
2. Para que no vuelva a pasar, comprueba en el registro que la carga del Explorador esté bien:
   HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon
   El valor Shell debe ser exactamente: explorer.exe
3. Repara el sistema: DISM /Online /Cleanup-Image /RestoreHealth y luego sfc /scannow.
4. Si empezó tras una actualización o un driver gráfico, arranca en modo seguro (mantén Mayús al pulsar Reiniciar) y desinstálalo.
5. Descarta el monitor: si hay dos pantallas, pulsa Win+P y prueba las opciones; a veces el escritorio se fue al monitor apagado.
6. Si nada de lo anterior, usa un punto de restauración anterior al problema (AdminOps → Historial → Puntos de restauración).`,
    tags: ["pantalla negra", "explorer", "inicio de sesion"],
  },
  {
    id: "relacion-confianza-dominio",
    title: "«Error de relación de confianza entre esta estación de trabajo y el dominio»",
    problem: "Al iniciar sesión con la cuenta del dominio aparece ese mensaje y no deja entrar. Suele ocurrir tras restaurar el equipo de una imagen o después de mucho tiempo apagado.",
    solution: `1. Entra con una cuenta de administrador local del equipo.
2. Comprueba que el equipo ve al controlador de dominio (ping al nombre del dominio) y que la fecha y hora coinciden: una diferencia de más de 5 minutos rompe la confianza.
3. Repara la relación sin sacar el equipo del dominio (AdminOps → Oficina → Dominio → Reparar). Equivale a este comando en PowerShell como administrador:
   Test-ComputerSecureChannel -Repair -Credential (Get-Credential)
   Pide un usuario con permiso para unir equipos al dominio.
4. Reinicia e inicia sesión con la cuenta del dominio.
5. Si no se repara, saca el equipo a un grupo de trabajo, reinicia, borra su cuenta en Usuarios y equipos de Active Directory y vuelve a unirlo.
6. Antes de sacarlo, asegúrate de tener la contraseña del administrador local: si no, te quedas fuera del equipo.`,
    tags: ["dominio", "confianza", "active directory"],
  },
  {
    id: "disco-c-lleno",
    title: "El disco C: se llenó y Windows avisa",
    problem: "La barra de C: está en rojo, Windows avisa de poco espacio y ya no deja actualizar ni guardar archivos.",
    solution: `1. Mira qué ocupa de verdad (AdminOps → Equipo → Espacio en disco): ordena por tamaño antes de borrar nada.
2. Limpieza segura (AdminOps → Ajustes de Windows → Limpieza): temporales, caché de Windows Update y papelera. Suele liberar varios GB.
3. Windows.old: si hay una actualización de versión reciente, ocupa entre 10 y 25 GB y se puede borrar desde la Limpieza de disco (o esperar a que Windows lo haga a los 10 días).
4. Hibernación: si es un equipo de sobremesa que no hiberna, como administrador:
   powercfg /h off
   Libera tantos GB como RAM tenga.
5. Restauración del sistema: reduce el espacio reservado si pasa del 10 % (Propiedades del sistema → Protección del sistema → Configurar).
6. Carpetas del usuario: Descargas, Escritorio y OneDrive suelen ser lo más grande. Mueve lo que no se use a otro disco.
7. Si el disco es de 128 GB o menos, tras todo esto seguirá justo: plantea cambiarlo.`,
    tags: ["espacio", "disco lleno", "limpieza"],
  },
  {
    id: "usb-no-reconocido",
    title: "Windows no reconoce el USB (o el disco externo)",
    problem: "Se conecta un pendrive o disco externo y no aparece en el Explorador, o sale «Dispositivo USB no reconocido».",
    solution: `1. Prueba otro puerto (mejor uno trasero en sobremesa) y otro cable. Los discos externos que piden mucha corriente fallan en puertos frontales o con cables largos.
2. Administración de discos (Win+X → Administración de discos): si el disco aparece sin letra de unidad, asígnale una con el clic derecho. Si aparece «No inicializado» o «Sin asignar», tiene la tabla de particiones dañada: no lo inicialices si hay datos que recuperar.
3. Administrador de dispositivos → Controladoras USB: desinstala los dispositivos con aviso amarillo y reinicia.
4. Desactiva la suspensión selectiva de USB: Opciones de energía → Cambiar la configuración avanzada → Configuración de USB.
5. Prueba el mismo USB en otro equipo: si tampoco va, el que falla es el USB.
6. Si el disco suena (clics) o se desconecta solo, deja de usarlo y haz copia de lo que se pueda: está fallando.`,
    tags: ["usb", "disco externo", "hardware"],
  },
  {
    id: "activacion-windows",
    title: "Windows no está activado",
    problem: "Aparece la marca de agua «Activar Windows» y algunas opciones de personalización están bloqueadas.",
    solution: `1. Mira el estado real como administrador:
   slmgr /xpr
2. Si el equipo es de marca y venía con Windows, la licencia está en la BIOS: AdminOps → Hardware muestra la clave OEM. Normalmente basta con:
   Configuración → Sistema → Activación → Solucionar problemas
3. Si se cambió la placa base, la licencia digital se pierde: hay que reactivarla con la cuenta de Microsoft vinculada (Solucionar problemas → «He cambiado el hardware de este dispositivo recientemente»).
4. Con licencia por volumen (empresa), comprueba que llega al servidor KMS:
   slmgr /skms <servidor>
   slmgr /ato
5. Si el error es 0xC004F074, casi siempre es la hora del equipo o que no ve el servidor KMS.
6. Nunca uses activadores de Internet: son la vía más común de entrada de malware en equipos de empresa.`,
    tags: ["activacion", "licencia", "windows"],
  },
  {
    id: "hora-incorrecta",
    title: "La fecha y la hora se descuadran y fallan las webs",
    problem: "El reloj se va solo, y los navegadores dan errores de certificado («la conexión no es privada») en webs que funcionan bien en otros equipos.",
    solution: `1. Pon la zona horaria correcta y activa la hora automática (Configuración → Hora e idioma).
2. Fuerza la sincronización como administrador:
   w32tm /resync /force
   Si da error, arranca el servicio: net start w32time
3. Si se descuadra cada vez que se apaga el equipo, la pila de la placa (CR2032) está gastada: cámbiala. Es la causa más habitual en equipos de más de 5 años.
4. En un equipo de dominio, la hora la debe dar el controlador de dominio: no pongas servidores de Internet a mano.
5. Tras corregir la hora, los errores de certificado desaparecen solos.`,
    tags: ["hora", "fecha", "certificado", "pila"],
  },
  {
    id: "navegador-secuestrado",
    title: "El navegador abre webs raras y tiene barras que no se quitan",
    problem: "Se cambió la página de inicio y el buscador, salen anuncios en sitios donde no los había y aparecen extensiones que nadie instaló.",
    solution: `1. Revisa las extensiones del navegador y quita todo lo que no reconozcas (AdminOps → Seguridad → Extensiones del navegador las lista todas de una vez).
2. Comprueba los accesos directos del navegador: clic derecho → Propiedades → Destino. Si después de chrome.exe o msedge.exe hay una dirección web, bórrala.
3. Mira lo que arranca solo (AdminOps → Seguridad → Elementos que arrancan solos) y las tareas programadas sospechosas.
4. Desinstala programas raros instalados por esas fechas (AdminOps → Aplicaciones → Desinstalar, ordenado por fecha).
5. Pasa un análisis completo con Microsoft Defender y, además, Malwarebytes en su versión gratuita: detecta adware que el antivirus normal ignora.
6. Restablece el navegador (Configuración → Restablecer) y comprueba que no haya un proxy puesto: Configuración de Windows → Red → Proxy.
7. Si es un equipo de empresa, avisa: si entró por un correo, probablemente llegó a más usuarios.`,
    tags: ["adware", "navegador", "malware", "extensiones"],
  },
  {
    id: "escritorio-remoto-no-conecta",
    title: "El Escritorio remoto no conecta",
    problem: "Al conectar por Escritorio remoto da error de que no se puede contactar con el equipo, o pide credenciales y las rechaza.",
    solution: `1. Comprueba que el equipo de destino esté encendido y responda: ping a su nombre o IP.
2. Comprueba que el puerto 3389 esté abierto (AdminOps → Oficina → Puestos muestra los puertos abiertos de cada equipo).
3. En el equipo de destino, activa el Escritorio remoto (AdminOps → Oficina → Acceso remoto) y comprueba que el usuario esté en el grupo «Usuarios de escritorio remoto».
4. Windows Home no permite recibir conexiones de Escritorio remoto: solo salir. Usa AnyDesk o RustDesk en esos equipos.
5. Si pide credenciales y las rechaza, escribe el usuario con el dominio delante: DOMINIO\\usuario, o EQUIPO\\usuario si es una cuenta local.
6. Si conecta y se queda en negro, prueba bajando la resolución y desactivando la composición del escritorio en las opciones de la conexión.
7. Tras una actualización grande de Windows, revisa que el firewall no haya vuelto a bloquear la regla de Escritorio remoto.`,
    tags: ["escritorio remoto", "rdp", "red"],
  },
  {
    id: "unidad-red-no-conecta",
    title: "La unidad de red no se conecta al arrancar",
    problem: "La unidad mapeada (Z:, S:…) aparece con una cruz roja y hay que abrirla a mano cada mañana para que funcione.",
    solution: `1. Casi siempre es que Windows intenta conectarla antes de que la red esté lista. No es que la unidad esté mal.
2. Comprueba que se conecta bien al hacer doble clic: si al abrirla funciona, es el problema de arranque.
3. Solución fiable: crear una tarea programada al iniciar sesión, con retraso de 30 segundos, que ejecute:
   net use Z: \\\\servidor\\carpeta /persistent:yes
4. Comprueba que las credenciales estén guardadas (Administrador de credenciales → Credenciales de Windows) para que no pida usuario cada vez.
5. En redes con Wi-Fi, activa «Iniciar sesión siempre en esta red» para que la conexión suba antes de la sesión del usuario.
6. Si la unidad falla de forma intermitente durante el día, el problema es la red o el servidor, no el mapeo.`,
    tags: ["unidad de red", "mapeo", "servidor"],
  },
  {
    id: "teclado-caracteres-raros",
    title: "El teclado escribe caracteres equivocados",
    problem: "Al escribir salen símbolos que no corresponden: las comillas, la arroba o la eñe no están donde deberían.",
    solution: `1. Es la distribución del teclado, no el teclado. Mira el indicador de idioma junto al reloj (ESP, ENG…).
2. Cambia con Alt+Mayús (o Win+Espacio) y comprueba si vuelve a la normalidad.
3. Para arreglarlo de forma permanente: Configuración → Hora e idioma → Idioma y región → Español → Opciones → Teclado. Deja solo la distribución correcta (España o Latinoamérica según el teclado físico).
4. Fíjate en el teclado físico: si tiene la tecla Ñ, es un teclado en español; si no, es internacional y la distribución debe coincidir.
5. Quita los idiomas que no se usen para que el atajo no lo cambie por accidente.
6. Si solo falla alguna tecla suelta y la distribución es correcta, el teclado está sucio o roto: pruébalo con uno USB.`,
    tags: ["teclado", "idioma", "distribucion"],
  },
  {
    id: "bluetooth-no-encuentra",
    title: "Bluetooth no encuentra dispositivos",
    problem: "El equipo tiene Bluetooth activado pero no aparece el ratón, los auriculares o el móvil al buscarlos.",
    solution: `1. Comprueba que el dispositivo esté en modo emparejamiento (suele ser mantener el botón hasta que parpadee), no solo encendido.
2. Si ya estuvo emparejado antes, quítalo de la lista y vuelve a añadirlo: los dispositivos recordados a medias no aparecen.
3. Administrador de dispositivos → Bluetooth: si no aparece el adaptador, actívalo en la BIOS o instala el driver del fabricante.
4. Reinicia el servicio como administrador:
   net stop bthserv & net start bthserv
5. En portátiles, comprueba que el modo avión esté desactivado y que la tecla de función del Wi-Fi no haya apagado también el Bluetooth.
6. Los auriculares emparejados con el móvil no aparecen en el PC: desconéctalos del móvil primero.`,
    tags: ["bluetooth", "dispositivos", "driver"],
  },
  {
    id: "bateria-dura-poco",
    title: "La batería del portátil dura muy poco",
    problem: "El portátil aguanta mucho menos que antes o se apaga de golpe aunque marque carga.",
    solution: `1. Mira el desgaste real (AdminOps → Diagnóstico → Batería): compara la capacidad actual con la de fábrica. Por debajo del 60 % la batería está para cambiar y no hay ajuste que lo arregle.
2. Genera el informe detallado de Windows como administrador:
   powercfg /batteryreport
   Lo guarda en un HTML con el histórico de ciclos y capacidad.
3. Mira qué gasta: Configuración → Sistema → Energía y batería → Uso de la batería por aplicación.
4. Ajusta el plan de energía y el brillo, que es lo que más consume en un portátil.
5. Si el fabricante trae utilidad propia (Lenovo Vantage, HP, Dell Power Manager), comprueba el límite de carga: muchos equipos de empresa cargan solo hasta el 80 % a propósito.
6. Si se apaga de golpe marcando 40 %, la batería está midiendo mal: cámbiala.`,
    tags: ["bateria", "portatil", "energia"],
  },
  {
    id: "acceso-denegado-instalar",
    title: "«Acceso denegado» al instalar o al abrir una carpeta",
    problem: "Al instalar un programa o abrir una carpeta aparece acceso denegado (a veces con el código 0x80070005) aunque la cuenta sea administradora.",
    solution: `1. Prueba a ejecutar el instalador con clic derecho → Ejecutar como administrador. Ser administrador no basta: hace falta elevar.
2. Si es una carpeta, revisa el propietario: clic derecho → Propiedades → Seguridad → Opciones avanzadas → Propietario. Es habitual tras copiar datos de otro equipo o de un disco antiguo.
3. Para recuperar el control de una carpeta, como administrador:
   takeown /F "C:\\ruta" /R /D S
   icacls "C:\\ruta" /grant %USERNAME%:F /T
4. Si el bloqueo es del antivirus, mira su registro: el acceso controlado a carpetas de Defender bloquea escrituras en Documentos y Escritorio.
5. Si es un equipo de empresa, puede ser una directiva de grupo la que lo impide: compruébalo con gpresult /h informe.html.
6. Si el archivo viene de Internet y da error al abrirlo, desbloquéalo: Propiedades → Desbloquear.`,
    tags: ["permisos", "acceso denegado", "instalacion"],
  },
  {
    id: "no-arranca-windows",
    title: "Windows no arranca (reparación de inicio)",
    problem: "El equipo no llega al escritorio: se queda en el logotipo, da vueltas, reinicia en bucle o muestra un error de arranque.",
    solution: `1. Antes de nada: si hay datos importantes y no hay copia, sácalos primero con un USB de arranque. Reparar puede empeorar el disco.
2. Entra en el entorno de recuperación: apaga el equipo con el botón durante el logotipo tres veces seguidas y Windows entra solo.
3. Opciones avanzadas → Reparación de inicio. Déjala terminar aunque tarde.
4. Si no arregla nada, abre el Símbolo del sistema desde ahí y repara el arranque:
   bootrec /fixmbr
   bootrec /fixboot
   bootrec /scanos
   bootrec /rebuildbcd
5. Comprueba el disco: chkdsk C: /f /r (puede tardar horas en discos grandes).
6. Prueba un punto de restauración anterior desde Opciones avanzadas.
7. Si el disco no aparece en el entorno de recuperación, el problema es el disco o su conexión, no Windows.`,
    tags: ["no arranca", "arranque", "recuperacion"],
  },
  {
    id: "archivos-abren-programa-equivocado",
    title: "Los archivos se abren con el programa equivocado",
    problem: "Los PDF se abren con el navegador, los documentos con la aplicación que no es, o al hacer doble clic no pasa nada.",
    solution: `1. Cambia el programa de una vez: clic derecho sobre el archivo → Abrir con → Elegir otra aplicación → marca «Usar siempre esta aplicación».
2. Para todos a la vez: Configuración → Aplicaciones → Aplicaciones predeterminadas, busca el programa y asígnale sus tipos de archivo.
3. Si al hacer doble clic no pasa nada, la asociación está rota. Restáurala desde la misma pantalla con «Restablecer» en los valores predeterminados de Microsoft y vuelve a asignarla.
4. Tras cada actualización grande, Windows vuelve a poner Edge para los PDF: es normal, hay que reasignarlo.
5. En equipos de empresa esto se puede fijar por directiva para que no cambie; consúltalo con TI si pasa a menudo.`,
    tags: ["asociacion", "archivos", "predeterminado"],
  },
];

/** Las soluciones de AdminOps con el formato de la biblioteca del técnico. */
export const BUILTIN_SOLUTIONS: Solution[] = CATALOG.map((s) => ({
  id: BUILTIN_PREFIX + s.id,
  title: s.title,
  problem: s.problem,
  solution: s.solution,
  tags: s.tags,
}));

/** Copia editable de una solución de AdminOps (queda como una más del técnico). */
export function duplicateForEditing(s: Solution): Solution {
  return { id: "", title: `${s.title} (mi versión)`, problem: s.problem, solution: s.solution, tags: [...s.tags] };
}
