// Errores comprensibles: lo que llega del sistema (inglés, códigos, errores
// internos) se traduce a algo que el técnico pueda contar y resolver.

type Rule = [RegExp, string];

const RULES: Rule[] = [
  [/access is denied|acceso denegado|os error 5\b|permission denied|0x80070005/i, "Windows denegó el acceso. Prueba a abrir AdminOps como administrador."],
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
  for (const [re, text] of RULES) if (re.test(raw)) return text;
  return withoutUserPaths(raw);
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
  fix?: "admin" | "retry" | "network" | "space";
}

type Explain = [RegExp, Omit<Explained, "what">];

const EXPLAIN: Explain[] = [
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
