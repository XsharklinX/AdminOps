// Consejos cortos para la espera (1.2.9): mientras una tarea tarda, uno distinto
// cada pocos segundos. Solo de cosas que AdminOps hace de verdad.

export const TIPS: string[] = [
  "Con Ctrl+K puedes pegar un código de error (por ejemplo 0x80070005) y AdminOps lo explica.",
  "Arrastra un archivo o una carpeta a la ventana y AdminOps te ofrece qué hacer con ello.",
  "Clic derecho sobre un dato (IP, nombre de equipo, serie) para copiarlo o abrir un caso con él.",
  "Ctrl+Z deshace lo último que AdminOps cambió en el equipo, fuera de los campos de texto.",
  "En Puestos puedes revisar a la vez todos los equipos de la oficina y ver sus discos, antivirus y pantallazos.",
  "El icono de la cámara de cada tarjeta la copia como imagen, lista para pegar en un ticket o en Teams.",
  "En las listas, J y K (o las flechas) recorren las filas y Alt+↑/↓ reordena lo que se puede reordenar.",
  "El modo privacidad (Ctrl+Alt+P) difumina IP, correos y claves antes de compartir pantalla.",
  "Ctrl+Alt+N abre una nota de llamada aunque AdminOps esté minimizado.",
  "Un clic sobre una IP, una MAC o un número de serie lo copia: verás un «Copiado» al instante.",
  "F11 abre la pantalla de taller: cifras enormes para dejarla en un monitor mientras trabajas.",
  "En Programas → Actualizar salen primero los que tienen fallos de seguridad que se están explotando.",
];

/** El consejo que toca a los `secs` segundos de espera, cambiando cada `every` segundos desde `start`. */
export function tipAt(secs: number, start = 0, every = 8): string {
  const n = TIPS.length;
  return TIPS[(((start + Math.floor(secs / every)) % n) + n) % n];
}
