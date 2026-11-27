// Errores comprensibles: lo que llega del sistema (inglés, códigos, errores
// internos) se traduce a algo que el técnico pueda contar y resolver.

type Rule = [RegExp, string];

const RULES: Rule[] = [
  // Antes que «acceso denegado»: lo concreto primero.
  [/os error 740\b|requires elevation|requiere elevaci[oó]n|requested registry access is not allowed/i, "Hace falta modo técnico (administrador) para esto."],
  [/os error 1223\b|operation was canceled by the user|cancelad[oa] por el usuario/i, "Cancelado en el aviso de Windows."],
  // Windows Update.
  [/0x800f081f|0x800f0906|0x800f0907|0x800f0950/i, "Windows no encuentra los archivos que necesita para instalarlo: sus componentes están dañados o no llega a Windows Update."],
  [/0x8024402c|0x80244022|0x8024401c|0x80072ee2|0x80072efd|0x80072f8f/i, "Windows Update no llega a sus servidores: sin Internet, un proxy o la hora del equipo mal."],
  [/0x800f0922/i, "Windows no pudo terminar la actualización: suele faltar espacio en la partición reservada del sistema o una VPN la bloquea."],
  [/0x80073712/i, "Faltan o están dañados archivos de Windows Update en este equipo."],
  [/0x80070643/i, "La instalación falló dentro de su instalador (a menudo .NET Framework o la actualización de Defender)."],
  [/0x80240017/i, "Esa actualización no es para este equipo: ya está instalada o es de otra versión de Windows."],
  // Cuentas y contraseñas (red, dominio, carpetas compartidas).
  [/os error 1326\b|unknown user name or bad password|nombre de usuario o contrase[ñn]a incorrectos/i, "Usuario o contraseña incorrectos."],
  [/os error 86\b|network password is not correct|contrase[ñn]a de red especificada no es correcta/i, "La contraseña de red no es correcta."],
  [/os error 1219\b|multiple connections to a server|varias conexiones a un servidor/i, "Ya hay una conexión a ese servidor con otro usuario: Windows no deja dos a la vez."],
  [/os error 1909\b|account is currently locked|cuenta (?:a la que se hace referencia )?est[aá] bloqueada/i, "La cuenta está bloqueada (demasiados intentos con la contraseña mal)."],
  [/os error 1907\b|password must be changed|debe cambiar(?:se)? la contrase[ñn]a/i, "Hay que cambiar la contraseña antes de entrar."],
  [/os error 1385\b|has not been granted the requested logon type/i, "Ese usuario no tiene permiso para iniciar sesión así en el otro equipo."],
  // Impresoras.
  [/0x00000709|\b0x709\b/i, "Windows no deja cambiar la impresora predeterminada (la tiene fijada él o lo bloquea un ajuste)."],
  [/0x0000011b|\b0x11b\b/i, "Windows bloquea la conexión a esa impresora compartida por una protección de seguridad."],
  [/0x00000bc4|\b0xbc4\b/i, "No se encuentran impresoras en la red: detección de redes apagada o la cola de impresión parada."],
  // winget.
  [/0x8a15[0-9a-f]{4}/i, "winget no pudo hacerlo: suele arreglarse con «Reparar winget» en Instalar."],
  // Generales que salen a menudo.
  [/0x8007000e|out of memory|memoria insuficiente/i, "Windows se quedó sin memoria para hacerlo: cierra programas y vuelve a probar."],
  [/0x80070570|corrupt(?:ed)? and unreadable|est[aá] da[ñn]ado y no se puede leer/i, "El archivo o la carpeta está dañado y no se puede leer: conviene revisar el disco."],
  [/is not recognized as the name of a cmdlet|no se reconoce como nombre de un cmdlet/i, "A este Windows le falta una herramienta que hace falta para esto (por su versión o su edición)."],
  [/access is denied|acceso denegado|os error 5\b|permission denied|0x80070005/i, "Windows denegó el acceso. Prueba a pasar a modo técnico (administrador)."],
  [/being used by another process|os error 32\b|está siendo utilizado por otro proceso/i, "Un archivo está en uso por otro programa. Ciérralo y vuelve a intentarlo."],
  [/not enough space|os error 112\b|no hay espacio suficiente|disk full/i, "No queda espacio suficiente en el disco."],
  [/cannot find the (file|path)|os error [23]\b|no se puede encontrar (el archivo|la ruta)/i, "No se encuentra el archivo o la carpeta (puede que se haya movido o borrado)."],
  [/rpc server is unavailable|servidor rpc no está disponible|0x800706ba/i, "El equipo remoto no responde (apagado, sin red o con el firewall cerrado)."],
  [/error sending request|dns error|failed to lookup address|connection refused|connection reset|os error 1006[01]\b/i, "No se pudo conectar: revisa la conexión a Internet o que el servidor esté disponible."],
  [/the operation timed out|operation timed out|os error 10060\b/i, "La conexión tardó demasiado y se canceló."],
  [/the network path was not found|os error 53\b|no se ha encontrado la ruta de acceso de la red/i, "No se encuentra ese equipo o carpeta en la red."],
  [/the device is not ready|os error 21\b/i, "La unidad no está lista (¿USB desconectado?)."],
];

/** ¿Es un fallo interno de AdminOps (datos mal enviados entre la interfaz y el sistema)? */
const INTERNAL = /invalid args|missing required key|invalid type:|unknown variant|command .+ not found/i;

/** Lo que delata un fallo de verdad y no una condición normal (sin permisos, sin red, falta de espacio). */
const UNEXPECTED = /panicked|os error \d+|0x8[0-9a-f]{7}|exception|timed out|tiempo agotado|se agotó|thread .* panicked/i;

/** ¿Un error de una orden merece anotarse en el registro de errores de la app? */
export function isUnexpected(error: unknown): boolean {
  const raw = typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
  if (INTERNAL.test(raw)) return false; // ya se anota por su lado (onInternalError)
  if (RULES.some(([re]) => re.test(raw))) return false; // condiciones conocidas: explicadas, no son un fallo de la app
  return UNEXPECTED.test(raw);
}

let report: ((message: string) => void) | null = null;

/** Dónde anotar los fallos internos (lo configura api.ts con el registro técnico). */
export function onInternalError(fn: (message: string) => void) {
  report = fn;
}

export function humanError(command: string, error: unknown): string {
  const raw = typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
  if (INTERNAL.test(raw)) {
    report?.(`Comando ${command}: ${raw}`);
    return "Error interno de AdminOps. Se ha anotado en el registro técnico; si se repite, crea un paquete de soporte (Ctrl+K → «paquete de soporte»).";
  }
  for (const [re, text] of RULES) if (re.test(raw)) return withCode(text, raw);
  return withoutUserPaths(raw);
}

/** El código de Windows que traía el error (0x…, «error 740»), para buscarlo o dárselo a soporte. */
export function errorCode(raw: string): string | null {
  const hex = /0x[0-9a-f]{3,8}\b/i.exec(raw);
  if (hex) return hex[0].toLowerCase();
  const os = /os error (\d{2,5})\b/i.exec(raw);
  return os ? `error ${os[1]}` : null;
}

function withCode(text: string, raw: string): string {
  const code = errorCode(raw);
  return code && !text.includes(code) ? `${text} (${code})` : text;
}

/**
 * Nunca un nombre de usuario en pantalla: «C:\Users\ana\…» (o «file:///C:/Users/ana/…»)
 * pasa a «Carpeta personal\…».
 */
export function withoutUserPaths(text: string): string {
  return text.replace(/(?:file:\/\/\/)?[a-z]:[\\/](?:users|usuarios)[\\/][^\\/'"\s]+/gi, "Carpeta personal");
}

/** Un error explicado: qué pasó, por qué suele pasar y qué probar. */
export interface Explained {
  what: string;
  why: string;
  tries: string[];
  /** Algo que AdminOps puede hacer por ti: reiniciar como administrador, reintentar… */
  fix?: "admin" | "retry" | "network" | "space" | "update" | "printer" | "winget";
}

type Explain = [RegExp, Omit<Explained, "what">];

const EXPLAIN: Explain[] = [
  [/modo técnico \(administrador\) para esto/i, { why: "Esto cambia una parte del sistema que Windows solo deja tocar a un administrador.", tries: ["Pasa a modo técnico (arriba, «Modo usuario») y vuelve a intentarlo."], fix: "admin" }],
  [/cancelado en el aviso de windows/i, { why: "Se pulsó «No» o se cerró el aviso de permisos de Windows.", tries: ["Vuelve a intentarlo y acepta el aviso."] }],
  [/windows update|componentes est[aá]n da[ñn]ados|terminar la actualizaci[oó]n|esa actualizaci[oó]n no es para este equipo|fall[oó] dentro de su instalador/i, { why: "Windows Update o sus componentes tienen un problema en este equipo.", tries: ["Abre Windows Update en AdminOps: explica el error y tiene «Reparar Windows Update».", "Si dice que falta espacio, libera espacio y vuelve a probar."], fix: "update" }],
  [/usuario o contrase[ñn]a incorrectos|contrase[ñn]a de red|cambiar la contrase[ñn]a|cuenta est[aá] bloqueada|iniciar sesi[oó]n as[ií]/i, { why: "El otro equipo o el dominio no aceptan esas credenciales.", tries: ["Comprueba el usuario (con el dominio delante: OFICINA\\usuario) y la contraseña.", "Si se bloquea sola, algo en este equipo sigue usando la contraseña vieja (unidades de red, credenciales guardadas)."] }],
  [/otro usuario: windows no deja dos/i, { why: "Windows solo admite una cuenta por servidor a la vez.", tries: ["Desconecta las unidades de red de ese servidor y vuelve a conectar con el usuario bueno.", "O usa el servidor por su IP en vez del nombre."], fix: "retry" }],
  [/impresora/i, { why: "La impresora, su controlador o la cola de impresión de Windows tienen un problema.", tries: ["Abre Impresoras en AdminOps: vaciar la cola, reinstalar el controlador o cambiar la predeterminada."], fix: "printer" }],
  [/winget/i, { why: "El instalador de programas de Windows (winget) no está bien en este equipo.", tries: ["En Instalar, «Reparar winget» lo pone al día y arregla sus orígenes."], fix: "winget" }],
  [/administrador|acceso denegado|access is denied|0x80070005/i, { why: "Windows protege esa parte del sistema y solo deja tocarla con permisos de administrador.", tries: ["Reinicia AdminOps como administrador.", "Si ya lo es, puede que un antivirus esté bloqueando la carpeta."], fix: "admin" }],
  [/en uso por otro programa|being used by another process/i, { why: "Otro programa tiene ese archivo abierto y Windows no deja cambiarlo mientras tanto.", tries: ["Cierra el programa que lo usa (Procesos lo muestra).", "Si no sabes cuál es, reinicia el equipo y vuelve a intentarlo."], fix: "retry" }],
  [/espacio suficiente|not enough space|disk full/i, { why: "El disco está lleno o casi lleno.", tries: ["Libera espacio en Espacio en disco.", "Vacía la papelera y los temporales."], fix: "space" }],
  [/no se encuentra el archivo|cannot find the (file|path)/i, { why: "El archivo o la carpeta ya no está donde se esperaba: se movió, se borró o la unidad no está conectada.", tries: ["Comprueba que la unidad (USB, red) sigue conectada.", "Vuelve a elegir el archivo."] }],
  [/conectar|internet|error sending request|dns error/i, { why: "No hay conexión con el servidor: sin Internet, un proxy o un cortafuegos que lo bloquea.", tries: ["Comprueba que hay Internet.", "Prueba Red → Reparar la red.", "Si estás tras un proxy de empresa, pregunta si deja salir a ese servidor."], fix: "network" }],
  [/tardó demasiado|timed out/i, { why: "El otro lado no contestó a tiempo: equipo ocupado, red lenta o servicio colgado.", tries: ["Vuelve a intentarlo en un momento.", "Si se repite, reinicia el equipo o el servicio."], fix: "retry" }],
  [/equipo remoto no responde|rpc server/i, { why: "El equipo de destino no contesta: está apagado, sin red o con el cortafuegos cerrado.", tries: ["Comprueba que está encendido y en la red (ping en Red → Herramientas).", "Revisa que el cortafuegos permite la administración remota."] }],
  [/ruta de acceso de la red|network path was not found|no se encuentra ese equipo/i, { why: "No se encuentra ese equipo o esa carpeta compartida en la red.", tries: ["Revisa el nombre o prueba con la IP.", "Comprueba que la carpeta sigue compartida."] }],
  [/no está lista|device is not ready/i, { why: "La unidad no responde: un USB desconectado o un lector sin disco.", tries: ["Vuelve a conectar la unidad.", "Prueba otro puerto USB."], fix: "retry" }],
  [/error interno de adminops/i, { why: "Algo falló dentro de AdminOps, no en tu equipo. Ya está anotado en el registro técnico.", tries: ["Vuelve a intentarlo.", "Si se repite, crea un paquete de soporte (Ctrl+K → «paquete de soporte»)."], fix: "retry" }],
];

/** Explica un error ya traducido (o en bruto) para la tarjeta de error. */
export function explainError(message: string): Explained {
  const what = withoutUserPaths(message.trim()) || "Algo no salió bien.";
  for (const [re, e] of EXPLAIN) if (re.test(message)) return { what, ...e };
  return { what, why: "Windows devolvió un error que AdminOps no reconoce.", tries: ["Vuelve a intentarlo.", "Si se repite, copia los detalles y busca el código en Ctrl+K."], fix: "retry" };
}

/**
 * Texto técnico para pegar en un ticket o un correo: sin rutas personales,
 * correos ni nombres de equipo de la red (\\\\SERVIDOR).
 */
export function supportDetails(message: string, where = ""): string {
  const clean = withoutUserPaths(message)
    .replace(/[\w.+-]+@[\w-]+(\.[\w-]+)+/g, "[correo]")
    .replace(/\\\\[^\\\s]+/g, "\\\\[equipo]");
  return [`AdminOps · ${new Date().toLocaleString("es-ES")}`, where && `Dónde: ${where}`, `Error: ${clean}`].filter(Boolean).join("\n");
}
