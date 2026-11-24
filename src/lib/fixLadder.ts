// «Si un arreglo falla, el siguiente»: para cada arreglo, qué probar después,
// de menos a más fuerte. Y qué partes del análisis rápido sirven para comprobar
// si un arreglo funcionó (lo demás necesita el análisis completo).

export type Rung = { kind: "fix"; id: string; label: string; minutes?: number } | { kind: "manual"; label: string; detail: string };

/** Escalones después de cada arreglo, en orden. */
export const LADDERS: Record<string, Rung[]> = {
  "repair.sfc": [
    { kind: "fix", id: "repair.dism", label: "Reparar la imagen de Windows (DISM)", minutes: 20 },
    { kind: "fix", id: "repair.sfc", label: "Volver a comprobar los archivos (SFC)", minutes: 10 },
    { kind: "manual", label: "Reparar Windows sin perder nada", detail: "Con una ISO de la misma versión: montarla y ejecutar setup.exe → «Conservar archivos y aplicaciones». Unos 40 minutos." },
  ],
  "repair.dism": [
    { kind: "fix", id: "repair.sfc", label: "Comprobar los archivos con la imagen ya sana (SFC)", minutes: 10 },
    { kind: "manual", label: "Reparar Windows sin perder nada", detail: "Con una ISO de la misma versión: setup.exe → «Conservar archivos y aplicaciones»." },
  ],
  "repair.windows-update": [
    { kind: "fix", id: "repair.dism", label: "Reparar la imagen de Windows (DISM)", minutes: 20 },
    { kind: "manual", label: "Instalar la actualización a mano", detail: "Buscar su número (KB…) en el Catálogo de Microsoft Update, descargarla e instalarla con doble clic." },
  ],
  // Windows Update que falla: de lo más concreto (la causa medida) a lo más fuerte.
  "repair.esp-fonts": [
    { kind: "fix", id: "repair.wu-services", label: "Reiniciar los servicios de Windows Update" },
    { kind: "fix", id: "repair.windows-update", label: "Restablecer Windows Update", minutes: 2 },
    { kind: "fix", id: "repair.dism", label: "Reparar la imagen de Windows (DISM)", minutes: 20 },
    { kind: "manual", label: "Instalar la actualización a mano", detail: "Buscar su número (KB…) en el Catálogo de Microsoft Update, descargarla e instalarla con doble clic." },
  ],
  "repair.wu-services": [
    { kind: "fix", id: "repair.windows-update", label: "Restablecer Windows Update", minutes: 2 },
    { kind: "fix", id: "repair.dism", label: "Reparar la imagen de Windows (DISM)", minutes: 20 },
    { kind: "manual", label: "Instalar la actualización a mano", detail: "Buscar su número (KB…) en el Catálogo de Microsoft Update, descargarla e instalarla con doble clic." },
  ],
  "repair.time-sync": [
    { kind: "fix", id: "repair.windows-update", label: "Restablecer Windows Update", minutes: 2 },
    { kind: "fix", id: "repair.dism", label: "Reparar la imagen de Windows (DISM)", minutes: 20 },
  ],
  "cleanup.windows-update-cache": [
    { kind: "fix", id: "repair.wu-services", label: "Reiniciar los servicios de Windows Update" },
    { kind: "fix", id: "repair.windows-update", label: "Restablecer Windows Update", minutes: 2 },
    { kind: "fix", id: "repair.dism", label: "Reparar la imagen de Windows (DISM)", minutes: 20 },
  ],
  "cleanup.user-temp": [
    { kind: "fix", id: "cleanup.windows-temp", label: "Limpiar también los temporales de Windows" },
    { kind: "fix", id: "cleanup.windows-update-cache", label: "Vaciar las descargas de Windows Update" },
    { kind: "manual", label: "Ver qué ocupa el disco", detail: "Discos → Espacio: carpetas grandes, duplicados y lo que nadie usa." },
  ],
  "repair.defender-signatures": [{ kind: "manual", label: "Comprobar Windows Update e Internet", detail: "Las firmas llegan por Windows Update: si falla, se arregla primero eso (Programas → Windows Update)." }],
  "repair.defender-realtime": [
    { kind: "manual", label: "Encenderla en Seguridad de Windows", detail: "Si una directiva o la protección contra alteraciones lo impide, solo se puede desde allí (o lo decide la empresa)." },
  ],
  "repair.defender-scan": [{ kind: "manual", label: "Análisis completo", detail: "Seguridad de Windows → Protección antivirus → Opciones de examen → Examen completo. Tarda una hora o más." }],
};

/** El siguiente escalón que aún no se ha probado. */
export function nextRung(fixId: string, tried: string[]): Rung | null {
  for (const r of LADDERS[fixId] ?? []) {
    if (r.kind === "manual") return r;
    if (!tried.includes(r.id) || (r.id === fixId && tried.filter((t) => t === r.id).length < 2)) return r;
  }
  return null;
}

/** Áreas que el análisis rápido vuelve a mirar en segundos. */
export const QUICK_AREAS = new Set(["Discos", "Estabilidad", "Drivers", "Batería", "Sistema", "Almacenamiento"]);

/** Cómo se comprueba un hallazgo tras arreglarlo: rápido o con el análisis completo. */
export const checkKind = (area: string): "quick" | "full" => (QUICK_AREAS.has(area) ? "quick" : "full");
