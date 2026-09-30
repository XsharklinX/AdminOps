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

export function recognize(raw: string): Recognized | null {
  const v = raw.trim();
  if (v.length < 2 || v.length > 64) return null;
  if (/^#\d{2,}$/.test(v)) return { kind: "ticket", value: v, sure: true };
  if (IP.test(v)) return { kind: "ip", value: v, sure: true };
  // Extensión: 3 a 5 cifras. Más largo es un teléfono o un número de serie.
  if (/^\d{3,5}$/.test(v)) return { kind: "extension", value: v, sure: true };
  if (PRINTER.test(v)) return { kind: "printer", value: v, sure: true };
  if (COMPUTER.test(v)) return { kind: "computer", value: v.toUpperCase(), sure: true };
  if (SAM.test(v)) return { kind: "person", value: v.split("\\").pop() ?? v, sure: true };
  if (FULL_NAME.test(v)) return { kind: "person", value: v, sure: false };
  return null;
}
