// «¿Está bien mi equipo?»: la pantalla del modo usuario. Un sí o un no, tres tarjetas en lenguaje
// normal (velocidad, seguridad, espacio) y, en cada una, un «por qué» corto. Aquí solo se decide
// el estado de cada tarjeta a partir de lo que ya se sabe del equipo.

export type Level = "ok" | "warn" | "bad";

export interface UserCard {
  id: "speed" | "security" | "space";
  title: string;
  level: Level;
  /** Una palabra: «Rápido», «Protegido», «Casi lleno». */
  label: string;
  /** Por qué, en una o dos frases. */
  why: string[];
  /** 0-100, para la barra. */
  value: number;
}

export interface UserInput {
  /** Memoria en uso, %. */
  ramPct: number;
  cpuPct: number;
  /** Nota de seguridad del último análisis (None: no hay). */
  securityScore: number | null;
  /** Hallazgos de seguridad del último análisis (títulos). */
  securityIssues: string[];
  /** Espacio libre de la unidad de Windows, %. */
  freePct: number | null;
  /** Discos que el diagnóstico da por mal o con avisos: [nombre, estado]. */
  disks: [string, Level][];
  /** Programas que arrancan con Windows. */
  startupCount: number | null;
  /** Hay un análisis hecho. */
  analyzed: boolean;
}

export interface UserSummary {
  level: Level;
  headline: string;
  sub: string;
  cards: UserCard[];
  /** Cosas que se pueden arreglar con un clic. */
  fixable: boolean;
}

const worst = (ls: Level[]): Level => (ls.includes("bad") ? "bad" : ls.includes("warn") ? "warn" : "ok");

export function summarize(i: UserInput): UserSummary {
  // Velocidad
  const speedWhy: string[] = [];
  let speed: Level = "ok";
  if (i.ramPct >= 90) {
    speed = "bad";
    speedWhy.push(`La memoria está casi llena (${Math.round(i.ramPct)} %): hay demasiados programas abiertos para este equipo.`);
  } else if (i.ramPct >= 80) {
    speed = "warn";
    speedWhy.push(`La memoria está bastante ocupada (${Math.round(i.ramPct)} %). Cerrar programas que no uses ayuda.`);
  }
  if (i.cpuPct >= 90) {
    speed = worst([speed, "warn"]);
    speedWhy.push("Algún programa está trabajando a tope y por eso todo va más lento.");
  }
  if (i.startupCount !== null && i.startupCount > 15) {
    speed = worst([speed, "warn"]);
    speedWhy.push(`Se abren ${i.startupCount} programas al encender, lo que alarga el arranque.`);
  }
  const badDisk = i.disks.find(([, l]) => l === "bad");
  if (badDisk) {
    speed = worst([speed, "warn"]);
  }
  if (speed === "ok") speedWhy.push("El equipo responde con fluidez.");

  // Seguridad
  const secWhy: string[] = [];
  let sec: Level = "ok";
  if (i.securityScore === null) {
    secWhy.push(i.analyzed ? "No se pudo comprobar la protección." : "Todavía no se ha revisado la protección del equipo.");
  } else if (i.securityScore < 60) {
    sec = "bad";
    secWhy.push("Faltan protecciones importantes (antivirus, cortafuegos o actualizaciones).");
  } else if (i.securityScore < 80) {
    sec = "warn";
    secWhy.push("La protección es mejorable.");
  } else {
    secWhy.push("El equipo está bien protegido.");
  }
  secWhy.push(...i.securityIssues.slice(0, 2));

  // Espacio
  const spWhy: string[] = [];
  let space: Level = "ok";
  if (i.freePct !== null) {
    if (i.freePct < 10) {
      space = "bad";
      spWhy.push(`Queda muy poco espacio libre (${Math.round(i.freePct)} %): Windows no puede actualizarse bien y todo va más lento.`);
    } else if (i.freePct < 20) {
      space = "warn";
      spWhy.push(`Queda poco espacio libre (${Math.round(i.freePct)} %).`);
    } else {
      spWhy.push(`Hay espacio de sobra (${Math.round(i.freePct)} % libre).`);
    }
  } else {
    spWhy.push("No se pudo medir el espacio.");
  }
  for (const [name, level] of i.disks) {
    if (level === "bad") {
      space = "bad";
      spWhy.push(`El disco ${name} está fallando: conviene hacer una copia de tus archivos y avisar a tu técnico.`);
    } else if (level === "warn") {
      space = worst([space, "warn"]);
      spWhy.push(`El disco ${name} tiene señales de desgaste.`);
    }
  }

  const label = {
    speed: { ok: "Rápido", warn: "Algo lento", bad: "Muy lento" },
    security: { ok: i.securityScore === null ? "Sin revisar" : "Protegido", warn: "Mejorable", bad: "En riesgo" },
    space: { ok: "De sobra", warn: "Justo", bad: "Casi lleno" },
  };
  const cards: UserCard[] = [
    { id: "speed", title: "Velocidad", level: speed, label: label.speed[speed], why: speedWhy, value: Math.max(0, 100 - Math.round(i.ramPct)) },
    { id: "security", title: "Seguridad", level: sec, label: label.security[sec], why: secWhy, value: i.securityScore ?? 0 },
    { id: "space", title: "Espacio", level: space, label: label.space[space], why: spWhy, value: i.freePct === null ? 0 : Math.round(i.freePct) },
  ];
  const level = worst(cards.map((c) => c.level));
  const issues = cards.filter((c) => c.level !== "ok").length;
  return {
    level,
    headline: level === "ok" ? "Tu equipo está bien" : level === "warn" ? "Tu equipo funciona, pero se puede mejorar" : "Tu equipo necesita atención",
    sub: !i.analyzed ? "Pulsa «Revisar» para hacer una revisión completa." : issues === 0 ? "No hay nada que hacer ahora." : `Hay ${issues === 1 ? "una cosa" : `${issues} cosas`} que mejorar.`,
    cards,
    // Limpiar temporales ayuda con el espacio y, un poco, con la velocidad.
    fixable: space !== "ok" || speed !== "ok",
  };
}

/** El resumen en texto, para mandarlo al técnico. */
export function summaryText(s: UserSummary, host: string): string {
  return [`Mi equipo (${host}): ${s.headline.toLowerCase()}.`, ...s.cards.map((c) => `• ${c.title}: ${c.label}. ${c.why.join(" ")}`), "", "¿Puedes echarle un vistazo?"].join("\n");
}
