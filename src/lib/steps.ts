// Soluciones que se ejecutan paso a paso: el texto de una solución partido en
// pasos numerados y la comprobación que toca después de cada uno.
import type { Symptom } from "./api";

export interface Step {
  n: number;
  text: string;
}

/** «1. Haz esto\n   detalle\n2. Lo otro» → pasos. Lo de antes del primer número va como paso 0. */
export function parseSteps(text: string): Step[] {
  const out: Step[] = [];
  for (const line of text.split("\n")) {
    const m = /^\s*(\d+)[.)]\s*(.*)$/.exec(line);
    if (m) out.push({ n: Number(m[1]), text: m[2] });
    else if (out.length) out[out.length - 1].text += `\n${line}`;
    else if (line.trim()) out.push({ n: 0, text: line });
  }
  return out.map((s) => ({ ...s, text: s.text.replace(/\s+$/, "") }));
}

const CHECKS: [RegExp, Symptom][] = [
  [/impresora|imprim|cola de impresi/i, "printer"],
  [/wi-?fi|inal[aá]mbric/i, "wifi"],
  [/internet|dns|\bred\b|conexi[oó]n|navegar/i, "internet"],
  [/sonido|audio|altavoz|micr[oó]fono/i, "audio"],
  [/bluetooth/i, "bluetooth"],
  [/outlook|office/i, "outlook"],
  [/onedrive|teams|sincroniz/i, "onedrive"],
  [/windows update|actualizaci/i, "winupdate"],
  [/perfil temporal/i, "profile"],
  [/navegador|chrome|edge|firefox/i, "browser"],
  [/lento|lentitud/i, "slow"],
  [/pantalla|monitor/i, "display"],
];

/** La comprobación de «Solucionar problemas» que dice si una solución ha funcionado. */
export function checkFor(s: { title: string; tags: string[] }): Symptom | null {
  const text = `${s.title} ${s.tags.join(" ")}`;
  return CHECKS.find(([re]) => re.test(text))?.[1] ?? null;
}
