# AdminOps — Presentación técnica

Documento para la revisión de AdminOps antes de autorizar su uso por el equipo de soporte.

**Autor:** David Bonilla · **Versión descrita:** 1.1.5 · **Plataforma:** Windows 10 y 11

---

## 1. Qué es y qué problema resuelve

AdminOps es una aplicación de escritorio para Windows que reúne, en una sola ventana, el trabajo
diario de un técnico de soporte: diagnosticar un equipo, repararlo, documentar lo que se hizo y
entregar un informe al usuario.

Hoy ese trabajo se hace saltando entre quince sitios: el Administrador de tareas, el Visor de
eventos, `cmd`, el Panel de control, la configuración de Windows, el navegador para el sistema de
tickets, Outlook para avisar, una hoja de Excel para el inventario. Cada salto es tiempo y una
oportunidad de olvidarse de un paso.

AdminOps no inventa nada que Windows no sepa hacer: **llama a las mismas herramientas de Windows**
(PowerShell, WMI/CIM, el registro, `pnputil`, `sfc`, `dism`, `winget`) y presenta el resultado
explicado en español, con el botón que lo arregla al lado.

**Lo que cambia en la práctica**

| Antes | Con AdminOps |
|---|---|
| Revisar un equipo: 20-30 min saltando entre herramientas | Un diagnóstico que revisa ~60 puntos y ordena lo importante |
| «Está lento» → hay que investigar de cero cada vez | 24 soluciones paso a paso, con botones que las ejecutan |
| El informe al usuario se escribe a mano | PDF generado con lo que se hizo y el antes/después |
| Lo que se cambió en el equipo se olvida | Diario de cambios, cada uno reversible con un clic |
| El inventario se rellena a mano | Ficha del equipo (modelo, serie, licencia, red) lista para copiar |

---

## 2. Qué hace

Siete secciones, 34 pestañas. Lo esencial:

- **Inicio** — Panel en vivo del equipo, «Solucionar problemas» por síntoma (no hay Internet, no
  suena, va lento…) y la sesión de servicio de principio a fin.
- **Equipo** — Diagnóstico, piezas y temperaturas, seguridad, historial de lo que ha pasado en el
  equipo, procesos y espacio en disco (qué se puede liberar, medido de verdad).
- **Aplicaciones** — Actualizar con `winget` y Windows Update, instalar en lote desde un catálogo de
  113 programas, desinstalar limpiando restos, quitar bloatware.
- **Red** — Router y Wi-Fi, dispositivos conectados, test de velocidad, ping/traceroute/DNS,
  reparación de red con antes y después.
- **Administración** — Puestos de la oficina (qué responde y qué necesita atención, con acciones en
  lote), usuarios locales, cuentas y dominio, impresoras y carpetas compartidas, acceso remoto.
- **Soporte** — Tickets, Correo (Outlook), Teams, agenda de mantenimientos, clientes, contactos y
  una base de soluciones propias.
- **Datos** — Migrar los datos de un usuario a otro equipo, copia cifrada, borrado seguro,
  recuperación de archivos borrados.

### Dos modos de interfaz

- **Modo técnico:** todo.
- **Modo usuario:** solo 6 páginas (Panel, Estado del equipo, Espacio, Solucionar problemas, Acceso
  remoto y Ajustes). No se ven clientes, contactos, tickets ni el correo del técnico.

> **Importante y se dice así dentro de la propia aplicación:** el modo usuario es una simplificación
> de la interfaz, **no una barrera de seguridad**. No sustituye a los permisos de Windows.

---

## 3. Cómo está hecho

| Capa | Tecnología | Por qué |
|---|---|---|
| Núcleo | **Rust** (edición 2021) | Sin recolector de basura, sin desbordamientos de memoria por diseño. Arranca rápido y consume poco. |
| Interfaz | **React 19 + TypeScript + Tailwind 4** | Tipado estricto de extremo a extremo. |
| Contenedor | **Tauri 2** sobre **WebView2** | Usa el motor Edge que **ya viene con Windows**: no se empaqueta un navegador. El ejecutable ronda los 15 MB, no los 150 MB de una app Electron. |

**Estructura:** ~50 módulos de Rust (uno por área: red, impresoras, usuarios, dominio…) y ~45
páginas de interfaz. La comunicación entre las dos capas es una lista explícita de **323 comandos**
declarados uno a uno: la interfaz no puede llamar a nada que no esté en esa lista.

**PowerShell:** las consultas cortas van a un grupo de hasta 5 consolas reutilizadas (arrancar
PowerShell cuesta cientos de milisegundos; reutilizarlo lo evita). Lo largo va en su propio proceso
para no bloquear la cola. Lo que cambia poco se cachea.

---

## 4. Seguridad y privacidad

Esta es la sección pensada para quien lleve seguridad. Todo lo que sigue es verificable en el código.

### 4.1 Permisos: no pide administrador para nada

AdminOps **arranca como usuario normal**. Solo pide elevación cuando la acción concreta la necesita,
y lo dice antes. Todo lo que es consultar (diagnóstico, inventario, red, procesos, espacio) funciona
sin ser administrador.

Cuando la app corre elevada, los enlaces que abre (navegador, Teams, correo) se abren
**deliberadamente sin elevar**, pidiéndoselo al escritorio de Windows. Un navegador lanzado como
administrador es un riesgo conocido; AdminOps lo evita a propósito.

### 4.2 Modo auditoría

Un interruptor que **rechaza, en un único punto del código, los ~80 comandos que modifican el
equipo**. Con el modo activo la aplicación solo mira: diagnostica, inventaría y genera el informe,
pero no puede cambiar nada aunque se pulse el botón. Pensado exactamente para auditorías o para
entrar en un equipo que no se debe tocar.

### 4.3 Todo cambio es reversible

Cada modificación del sistema queda en un **diario** con su estado anterior y se puede deshacer desde
la propia aplicación. Antes de cambios de sistema se ofrece crear un **punto de restauración** de
Windows.

Esto se verifica automáticamente: en cada compilación, la integración continua **aplica y deshace
todos los ajustes del catálogo** en una máquina desechable y falla si alguno no vuelve exactamente a
su estado original.

### 4.4 Las webs incrustadas están aisladas

Los portales (sistema de tickets, inventario web, Outlook, Teams) se muestran dentro de la ventana,
pero:

- **No tienen acceso a ningún comando de AdminOps.** El permiso de comunicación se concede solo a la
  ventana principal (`capabilities/default.json`); las vistas de los portales quedan fuera. Una web
  comprometida no puede llamar a nada de la aplicación.
- **Solo navegan por los dominios de ese portal.** Cualquier otro enlace se abre en el navegador del
  usuario, fuera de la aplicación. Hay pruebas automáticas de que `teams.microsoft.com.evil.com` o
  `intranet.empresa.com.evil.com` no cuelan.
- La interfaz propia corre bajo una **política de contenido restrictiva** (`default-src 'self'`): no
  carga código de Internet.

### 4.5 Contraseñas

| Modo | Cómo se guardan |
|---|---|
| Instalado | **DPAPI de Windows**, atadas a ese usuario en ese equipo |
| Portable (USB) | **AES-256-GCM** con una clave aleatoria en el propio USB |

Nunca se guardan en claro y nunca vuelven a la interfaz. Además, la aplicación puede bloquearse con
PIN o contraseña, con bloqueo por inactividad. La propia app avisa de que, en modo portable, **quien
tenga el USB entero podría leerlas**, y por eso recomienda activar el bloqueo.

### 4.6 Qué sale del equipo

**No hay telemetría, ni analítica, ni cuenta de usuario, ni servidor propio.** AdminOps no envía
información a ninguna parte. Las únicas conexiones salientes son, todas visibles en el código:

| Destino | Cuándo | Qué envía |
|---|---|---|
| `api.github.com` | Solo si «Avisar de versiones nuevas» está activado | Nada; consulta cuál es la última versión publicada |
| `ipinfo.io` | Solo si el técnico pulsa «IP pública» | Nada; es una consulta GET |
| `speed.cloudflare.com` | Solo si el técnico ejecuta el test de velocidad | Tráfico de prueba |
| Microsoft (Outlook/Teams) | Solo si el técnico los configura | Su propia sesión, como en cualquier navegador |

### 4.7 Datos personales en pantalla

Regla del proyecto: **ninguna ruta con el nombre de un usuario aparece en la interfaz**. Todos los
mensajes de error pasan por una función que convierte `C:\Users\ana\...` en `Carpeta personal\...`.
Los informes y los paquetes de soporte siguen la misma regla.

### 4.8 Lo que AdminOps no hace, a propósito

Se descartó explícitamente durante el desarrollo:

- No ejecuta nada en otros equipos de la red sin las herramientas y permisos propios de Windows.
- No toma el control de sesiones de otros usuarios.
- No hace nada para ocultarse del antivirus ni de las herramientas de administración.
- No desactiva Defender ni el firewall por su cuenta.

Para la asistencia remota se usan las herramientas legítimas: la Asistencia rápida de Windows,
Escritorio remoto, o AnyDesk/RustDesk/TeamViewer si ya están aprobados.

---

## 5. Calidad y control de cambios

- **226 pruebas automáticas** (186 en el núcleo Rust, 40 en la interfaz).
- **Integración continua en GitHub Actions** en cada cambio: compila, tipa, prueba, analiza el código
  con `clippy` en modo estricto (cualquier aviso es un fallo) y genera el instalador y el portable.
- **Prueba de ida y vuelta del catálogo de ajustes** en cada compilación (sección 4.3).
- **Guion de pruebas manuales** documentado (`docs/PRUEBAS.md`) para lo que no se puede automatizar.
- **Hoja de ruta** pública en el repositorio con lo hecho y lo pendiente, incluida la deuda técnica
  conocida.

Código fuente completo en GitHub (repositorio privado), disponible para revisión.

---

## 6. Cómo se despliega

Dos formas, y se pueden combinar:

**a) Instalador (`.exe`)** — instalación normal en el equipo del técnico.

**b) Portable (en un USB)** — no instala nada en el equipo del cliente. Configuración, contactos,
clientes y sesiones viajan en el USB; al sacarlo no queda rastro en el equipo visitado. Es la forma
pensada para atender equipos ajenos.

### Lo que hay que saber antes de aprobarlo

> **La aplicación no está firmada digitalmente.** Un certificado de firma de código cuesta entre
> 200 y 400 USD al año y no se ha adquirido. Consecuencia práctica: al instalar, **Windows
> SmartScreen mostrará un aviso** («Windows protegió su PC») y hay que pulsar *Más información →
> Ejecutar de todas formas*. El aviso desaparece a medida que Microsoft acumula reputación del
> archivo, pero vuelve con cada versión nueva.
>
> Si la institución lo va a usar de forma habitual, hay tres caminos: aceptar y documentar ese paso,
> **añadir el ejecutable a la lista de confianza del antivirus/AppLocker corporativo** (lo más
> limpio), o financiar el certificado.

Otros límites honestos:

- **Solo español.** El inglés está planificado, no hecho.
- **Sin actualización automática** todavía: hoy avisa de que hay versión nueva y hay que descargarla.
  Con varios técnicos conviene resolverlo (ver sección 8).
- **Los datos no se comparten entre técnicos.** Cada instalación (o cada USB) tiene sus clientes,
  contactos y sesiones. Hay exportar/importar configuración, pero no una base común.
- **El informe en PDF usa Edge.** Si no estuviera disponible, el informe sale en HTML.

---

## 7. Coste

**Cero en licencias.** Todo lo que usa AdminOps es gratuito o de código abierto: Rust, Tauri, React,
Tailwind. WebView2 viene con Windows. No hay servidor, no hay suscripción, no hay cuentas.

El único coste opcional es el certificado de firma de código de la sección 6.

---

## 8. Propuesta de uso en el equipo

Un camino prudente, por fases:

1. **Prueba controlada (2-4 semanas).** Dos o tres técnicos, en equipos de trabajo, con el modo
   auditoría activado en cualquier equipo sensible. Se recogen fallos y sugerencias.
2. **Revisión de seguridad.** IT revisa el código o al menos las secciones 4.1 a 4.8 de este
   documento, y decide sobre la lista de confianza del antivirus.
3. **Despliegue al equipo de soporte.** Carpeta de red compartida con la última versión + el guion de
   instalación. Resuelve de paso el problema de las actualizaciones.
4. **Decidir sobre lo compartido.** Si interesa que todos vean los mismos clientes y contactos, hay
   que plantearlo como desarrollo: hoy no existe.

---

## 9. Preguntas frecuentes

**¿Quién lo ha hecho y quién lo mantiene?**
David Bonilla. El código es suyo y está completo en un repositorio con su historial de cambios, sus
pruebas y su hoja de ruta.

**¿Qué pasa si él no está?**
El código fuente está disponible y documentado en español, con pruebas automáticas que explican qué
debe hacer cada parte. Es un proyecto de tecnologías estándar y muy usadas (Rust, React,
TypeScript): cualquier desarrollador puede tomarlo.

**¿Puede romper un equipo?**
Puede cambiar el equipo, como cualquier herramienta de administración, y por eso: (a) todo cambio
queda registrado y es reversible, (b) ofrece punto de restauración antes de tocar el sistema, (c) el
modo auditoría impide cualquier cambio, y (d) hay una prueba automática en cada compilación que
verifica que todos los ajustes vuelven a su estado original.

**¿Envía información fuera?**
No. Ver la tabla de la sección 4.6.

**¿Necesita permisos de administrador del dominio?**
No. Funciona como usuario normal para todo lo que es consultar. Las acciones que requieren
administrador local lo piden en ese momento, como cualquier herramienta de Windows.

**¿El antivirus lo va a bloquear?**
Puede mostrar el aviso de SmartScreen por no estar firmada (sección 6). No hace nada que un
antivirus deba considerar malicioso: no se oculta, no inyecta código, no cifra nada, no se comunica
con servidores externos. Si el antivirus corporativo lo marca, lo correcto es añadirlo a la lista de
confianza tras revisarlo.

**¿Por qué no usar las herramientas de Windows directamente?**
Se usan: AdminOps las llama. Lo que aporta es tenerlas en un sitio, explicadas en español, con el
resultado interpretado y el histórico de lo que se hizo en cada equipo.

**¿Y si Microsoft cambia WebView2 o Tauri cambia su API?**
La única dependencia sobre una API marcada como inestable son las vistas web incrustadas (los
portales). Si cambiara, esos portales seguirían funcionando en ventana aparte, que ya está
implementado. Está anotado como deuda técnica conocida en la hoja de ruta.

**¿Cuánto consume?**
Ejecutable de ~15 MB. Arranca en poco más de un segundo. No deja servicios ni procesos residentes
cuando está cerrada.

---

## 10. Demostración sugerida (10 minutos)

1. **Diagnóstico** de un equipo cualquiera: ~60 comprobaciones, ordenadas por importancia, cada una
   con «Arreglar» y «Qué cambió». *(2 min)*
2. **Deshacer** un ajuste desde el diario, para que se vea que nada es irreversible. *(1 min)*
3. **Modo auditoría**: activarlo y pulsar un botón que cambia el equipo; se rechaza. *(1 min)*
4. **Espacio en disco**: qué se puede liberar, medido de verdad. *(1 min)*
5. **Puestos**: comprobar varios equipos de la oficina a la vez. *(2 min)*
6. **Informe PDF** de la sesión, con el antes y el después. *(2 min)*
7. **Portable**: enseñar el USB y explicar que en el equipo visitado no queda nada. *(1 min)*

Dejar para el final, solo si preguntan, lo de Tickets/Correo/Teams: es cómodo para el técnico, pero
lo que justifica la aprobación es el diagnóstico, la reversibilidad y el informe.
