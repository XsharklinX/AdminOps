// Emparejar programas instalados con la lista de winget (los nombres cambian
// de versión y de arquitectura: «7-Zip 25.01 (x64)» es «7-Zip»).

export const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
/** «7-Zip 25.01 (x64)» → «7-zip»: para emparejar con la lista de winget. */
export const programKey = (name: string) =>
  norm(name.replace(/\s*\(.*\)\s*$/, ""))
    .replace(/\s+v?\d[\d.]*\s*$/, "")
    .replace(/\s+(x64|x86|64-bit|32-bit)$/, "")
    .trim();
