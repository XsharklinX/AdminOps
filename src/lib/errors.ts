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
