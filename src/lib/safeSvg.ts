// Un SVG que se pinta con innerHTML (el código QR de la Wi-Fi) solo se acepta si
// es exactamente lo que genera AdminOps: un <svg> con rectángulos y trazos, sin
// scripts, sin eventos (onload…), sin enlaces y sin nada más. Si algo no cuadra,
// no se pinta.

// Atributo: nombre sin «on…» y valor sin comillas, <, > ni &.
const ATTR = String.raw`\s+(?!on)[a-zA-Z][a-zA-Z:-]*="[^"<>&]*"`;
const SHAPE = String.raw`<(?:rect|path)(?:${ATTR})*\s*/>`;
const SVG = new RegExp(String.raw`^(?:<\?xml[^>?]*\?>\s*)?<svg(?:${ATTR})*\s*>\s*(?:<title>[^<>&]*</title>\s*)?(?:${SHAPE}\s*)*</svg>\s*$`, "i");

export function safeSvg(svg: string): string | null {
  if (svg.length > 200_000) return null;
  const text = svg.trim();
  if (!SVG.test(text)) return null;
  // Aun con la forma buena: nada de enlaces ni de código en los valores.
  if (/javascript:|data:|xlink:href|href=|<script|<foreignObject|<style|<image|<a\s/i.test(text)) return null;
  return text;
}
