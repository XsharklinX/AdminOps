import { editDistance, tolerance } from "./palette";

// Qué es lo que alguien acaba de escribir en Ctrl+K.
//
// «PC-CONTA-03», «maria.perez», «4512», «192.168.10.45», «impresora recepción»
// o «#4521» no son páginas: son un equipo, una persona, una extensión, una
// dirección, una impresora y un ticket. Reconocerlos permite ofrecer
// directamente lo que se hace con cada uno, en vez de obligar a recordar en qué
// página está cada cosa.
//
// Sin adivinar de más. Lo que tiene una forma inconfundible (una IP, un nombre
// de equipo, un usuario con punto) es «seguro» y se ofrece arriba. Dos palabras
// sueltas pueden ser un nombre («María Pérez») o una búsqueda normal («windows
// update»): eso es «dudoso» y se ofrece al final, sin tapar lo que ya
// encontraba Ctrl+K.

export type RecognizedKind = "ticket" | "ip" | "extension" | "printer" | "computer" | "person";

export interface Recognized {
  kind: RecognizedKind;
  value: string;
  /** Forma inconfundible: se ofrece antes que el resto de resultados. */
  sure: boolean;
}

const IP = /^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$/;
/** Nombre de equipo: letras, un guion o subrayado, y al menos un número (PC-CONTA-03, LAP_RRHH2). */
const COMPUTER = /^[a-z][a-z0-9]{1,6}[-_][a-z0-9-_]*\d[a-z0-9-_]*$/i;
/** Usuario del dominio (maria.perez, m_perez) o con el dominio delante (EMPRESA\maria.perez). */
const SAM = /^(?:[a-z0-9-]+\\)?[a-záéíóúñü]+[._][a-záéíóúñü0-9]+$/i;
/** Dos palabras o más con letras: puede ser un nombre y apellido. */
const FULL_NAME = /^[a-záéíóúñü]{2,}(\s+[a-záéíóúñü]{2,})+$/i;
const PRINTER = /\b(impresora|impresoras|printer|hp|epson|ricoh|brother|kyocera|canon|xerox|lexmark)\b/i;

/**
 * Palabras de un problema («monitor borroso», «excel no se guardó»): con alguna, dos palabras sueltas
 * son una búsqueda y no el nombre de una persona.
 */
const PROBLEM_WORDS = new Set([
  "monitor", "monitores", "pantalla", "pantallas", "borroso", "borrosa", "estirado", "estirada", "resolucion", "detecta", "teclado", "arroba", "sitio",
  "ingles", "idioma", "excel", "word", "powerpoint", "guardo", "guardar", "guardado", "cerro", "recuperar", "autorrecuperacion",
  "escanear", "escaner", "escaneo", "escanea", "carpeta", "configurar", "internet", "lento", "lenta", "subida", "bajada", "onedrive", "come", "consume", "usa", "quien", "ancho", "banda", "sincronizando",
]);

/** Hablar de escanear: con una marca de impresora de por medio («configurar escaner ricoh») no es «una impresora». */
const SCANNING = /\b(escaner|escaneres|escanear|escanea|escaneo|scanner)\b/;

const plain = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export function recognize(raw: string): Recognized | null {
  const v = raw.trim();
  if (v.length < 2 || v.length > 64) return null;
  if (/^#\d{2,}$/.test(v)) return { kind: "ticket", value: v, sure: true };
  if (IP.test(v)) return { kind: "ip", value: v, sure: true };
  // Extensión: 3 a 5 cifras. Más largo es un teléfono o un número de serie.
  if (/^\d{3,5}$/.test(v)) return { kind: "extension", value: v, sure: true };
  if (PRINTER.test(v) && !SCANNING.test(plain(v))) return { kind: "printer", value: v, sure: true };
  if (COMPUTER.test(v)) return { kind: "computer", value: v.toUpperCase(), sure: true };
  if (SAM.test(v)) return { kind: "person", value: v.split("\\").pop() ?? v, sure: true };
  if (FULL_NAME.test(v) && !plain(v).split(/\s+/).some((w) => PROBLEM_WORDS.has(w))) return { kind: "person", value: v, sure: false };
  return null;
}

// ---------- Frases que hacen algo ----------
//
// «desbloquear maria», «reiniciar PC-CONTA-03», «cola recepción», «laps legal-04»,
// «liberar espacio». Se escriben como se hablan: perdonan erratas y tildes que
// faltan. Esto solo entiende la frase; lo que se hace (los mismos comandos que
// los botones de cada pantalla) está en lib/quickActions.ts.

export type ActionKind = "unlock" | "restart" | "queue" | "laps" | "space" | "explorer";

export interface ActionIntent {
  action: ActionKind;
  /** Lo que sigue al verbo, tal como se escribió (el nombre de la persona, del equipo o de la impresora). */
  arg: string;
}

const VERBS: Record<ActionKind, string[]> = {
  unlock: ["desbloquear", "desbloquea", "desbloqueo", "unlock"],
  restart: ["reiniciar", "reinicia", "reboot", "restart"],
  queue: ["cola", "colas", "vaciar", "vacia"],
  laps: ["laps"],
  space: ["liberar", "libera"],
  // Sin verbo propio: sale de «reiniciar explorador».
  explorer: [],
};

/** Palabras de enlace que se saltan entre el verbo y el nombre. */
const LINK = new Set(["a", "al", "de", "del", "la", "el", "los", "las", "en", "equipo", "pc", "impresora", "cuenta", "usuario", "usuaria"]);

const strip = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** ¿`w` es (casi) alguna de estas palabras? Con erratas de una o dos letras según el largo. */
function isWord(w: string, list: string[]): boolean {
  const t = tolerance(w);
  return list.some((v) => v === w || (t > 0 && editDistance(w, v, t) <= t));
}

const HOST = /^[a-z0-9][a-z0-9._-]{1,62}$/i;
/** Palabras que, tras «vaciar cola», significan «todas las colas». */
const ALL_QUEUES = new Set(["impresion", "impresiones", "impresoras", "todas", "todo", "todos"]);
const EXPLORER = ["explorador", "explorer", "explorer.exe"];

export function parseAction(raw: string): ActionIntent | null {
  const words = raw.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0 || raw.length > 80) return null;
  const first = strip(words[0]);
  let action = (Object.keys(VERBS) as ActionKind[]).find((k) => isWord(first, VERBS[k]));
  if (!action) return null;
  let rest = words.slice(1);
  // «vaciar cola recepción» y «vaciar la cola de recepción»: «vaciar» solo vale si habla de la cola.
  if (first.startsWith("vaci")) {
    const i = rest.findIndex((w) => isWord(strip(w), ["cola", "colas"]));
    if (i < 0 || i > 1) return null;
    rest = rest.slice(i + 1);
    action = "queue";
  } else if (action === "queue") {
    // «cola» sin nada más no es una acción: es una búsqueda.
  }
  while (rest.length && LINK.has(strip(rest[0]))) rest = rest.slice(1);
  // «vaciar cola», «vaciar la cola de impresión», «vaciar todas las colas»: sin nombre, son todas.
  if (action === "queue" && first.startsWith("vaci") && rest.every((w) => ALL_QUEUES.has(strip(w)) || LINK.has(strip(w)))) return { action, arg: "" };
  // «reiniciar el explorador»: el Explorador de Windows, no un equipo.
  if (action === "restart" && rest.length === 1 && isWord(strip(rest[0]), EXPLORER)) return { action: "explorer", arg: "" };
  if (action === "space") {
    // «liberar espacio» (o «liberar» a secas); «liberar la cuenta de x» no es esto.
    const ok = rest.length === 0 || (rest.length === 1 && isWord(strip(rest[0]), ["espacio", "disco"]));
    return ok ? { action, arg: "" } : null;
  }
  const arg = rest.join(" ").trim();
  if (arg.length < 2) return null;
  if ((action === "restart" || action === "laps") && !HOST.test(arg)) return null;
  return { action, arg };
}

// ---------- Varias acciones en una frase ----------
//
// «liberar espacio y vaciar cola y reiniciar explorador». Solo se parte si cada
// trozo es una acción reconocida: «desbloquear maria y juan» o «maria y juan»
// siguen siendo una sola búsqueda.

/** « y », comas y puntos y coma. */
const SEPARATOR = /\s+y\s+|\s*[,;]\s*/gi;
const MAX_STEPS = 6;

export interface ActionStep {
  /** El trozo de frase, tal como se escribió. */
  text: string;
  intent: ActionIntent;
}

/** La frase partida por « y », «, » y «;», sin trozos vacíos. */
export function splitPhrase(raw: string): string[] {
  return raw
    .split(SEPARATOR)
    .map((s) => s.trim())
    .filter(Boolean);
}

/** Los pasos de la frase, o null si no son dos o más y todos acciones reconocidas. */
export function parseSteps(raw: string): ActionStep[] | null {
  const parts = splitPhrase(raw);
  if (parts.length < 2 || parts.length > MAX_STEPS) return null;
  const steps: ActionStep[] = [];
  for (const text of parts) {
    const intent = parseAction(text);
    if (!intent) return null;
    steps.push({ text, intent });
  }
  return steps;
}

/** Órdenes completas que Tab sabe terminar, de la más probable a la menos. */
const COMPLETIONS = ["liberar espacio", "vaciar cola de impresión", "reiniciar explorador", "desbloquear ", "reiniciar ", "laps ", "vaciar cola "];

/**
 * Tab: completa la orden a medias (la que va tras el último « y » o coma) con la
 * acción más probable. Devuelve la frase entera completada, o null si no hay nada que completar.
 */
export function completePhrase(raw: string): string | null {
  let cut = 0;
  for (const m of raw.matchAll(SEPARATOR)) cut = (m.index ?? 0) + m[0].length;
  const head = raw.slice(0, cut);
  const tail = raw.slice(cut).replace(/^\s+/, "");
  const t = strip(tail);
  if (t.length < 2) return null;
  const hit = COMPLETIONS.find((c) => strip(c).startsWith(t) && strip(c) !== t);
  return hit ? head + hit : null;
}
