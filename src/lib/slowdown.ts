// «Qué frena el equipo ahora»: de los procesos con más carga, cuáles pesan de
// verdad, qué son en palabras normales y si tiene sentido cerrarlos.
import type { LiveMetrics } from "./api";

/** Programas de Windows o del propio equipo: se explican, no se ofrece cerrarlos. */
const KNOWN: Record<string, string> = {
  "msmpeng.exe": "El antivirus de Windows está analizando. Es normal y suele durar unos minutos.",
  "tiworker.exe": "Windows está instalando actualizaciones. Mejor dejar que termine.",
  "trustedinstaller.exe": "Windows está instalando actualizaciones. Mejor dejar que termine.",
  "searchindexer.exe": "Windows está indexando archivos para el buscador. Baja solo al rato.",
  "searchhost.exe": "El buscador de Windows.",
  "system": "El propio Windows: drivers y disco. Si no baja, mira Diagnóstico → Drivers.",
  "memory compression": "Windows comprimiendo memoria porque queda poca libre.",
  "registry": "Parte de Windows.",
  "dwm.exe": "Dibuja las ventanas. Si pesa mucho, suele ser el driver de la gráfica.",
  "explorer.exe": "El Explorador y la barra de tareas. Se reinicia desde Acciones rápidas.",
  "svchost.exe": "Un servicio de Windows. En Procesos se ve cuál.",
  "wmiprvse.exe": "Windows respondiendo consultas, a veces de AdminOps mismo.",
  "csrss.exe": "Parte de Windows.",
  "lsass.exe": "Parte de Windows (inicio de sesión).",
  "audiodg.exe": "El sonido de Windows.",
  "adminops.exe": "AdminOps. Sube mientras analiza y baja al terminar.",
  "msedgewebview2.exe": "El navegador interno de AdminOps o de otra app (Teams, Outlook).",
  "powershell.exe": "Una consola de PowerShell; puede ser AdminOps leyendo el equipo.",
};

export interface Hog {
  pid: number;
  name: string;
  cpu: number;
  memory: number;
  /** Qué es, si se sabe. */
  about: string | null;
  /** Tiene sentido ofrecer cerrarlo (un programa del usuario). */
  closable: boolean;
}

export interface Slowdown {
  level: "ok" | "warn" | "bad";
  headline: string;
  hogs: Hog[];
}

const GB = 1024 ** 3;

/** Lo que pesa de verdad: 10 % de procesador o 1 GB (o el 10 %) de memoria. */
function heavy(p: { cpu: number; memory: number }, memoryTotal: number) {
  return p.cpu >= 10 || p.memory >= Math.min(GB, memoryTotal * 0.1);
}

export function slowdown(m: Pick<LiveMetrics, "cpuTotal" | "memoryUsed" | "memoryTotal" | "topProcesses">): Slowdown {
  const cpu = Math.round(m.cpuTotal);
  const mem = m.memoryTotal ? Math.round((m.memoryUsed / m.memoryTotal) * 100) : 0;
  const hogs = m.topProcesses
    .filter((p) => heavy(p, m.memoryTotal))
    .slice(0, 4)
    .map((p) => {
      const about = KNOWN[p.name.toLowerCase()] ?? null;
      return { ...p, about, closable: about === null };
    });
  const parts = [cpu >= 80 && `procesador al ${cpu} %`, mem >= 85 && `memoria al ${mem} %`].filter(Boolean) as string[];
  if (!parts.length) {
    return { level: "ok", headline: hogs.length ? "Va holgado. Esto es lo que más usa ahora" : "Nada lo está frenando ahora", hogs };
  }
  const text = parts.join(" y ");
  return { level: cpu >= 95 || mem >= 95 ? "bad" : "warn", headline: text.charAt(0).toUpperCase() + text.slice(1), hogs };
}
