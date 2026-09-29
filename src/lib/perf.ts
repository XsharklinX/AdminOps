// Medición de la interfaz: cuánto tarda en abrirse AdminOps y cada página la
// primera vez que se visita (descargar su código, pintarla y esperar a que
// terminen las llamadas al backend que hace al abrirse). Solo en memoria; se ve
// en Ajustes → Rendimiento de AdminOps y el resumen del arranque queda en el
// registro técnico.

export interface PageLoad {
  page: string;
  /** Momento de la visita (ms desde que se cargó la interfaz). */
  at: number;
  /** Descargar y preparar el código de la página (null si ya estaba cargado). */
  codeMs: number | null;
  /** Hasta que la página se pinta por primera vez. */
  paintMs: number | null;
  /** Hasta que terminan las llamadas al backend que lanzó al abrirse. */
  readyMs: number | null;
  /** Llamadas al backend durante la carga, la más lenta primero. */
  calls: { cmd: string; ms: number }[];
}

export interface CommandStat {
  cmd: string;
  count: number;
  totalMs: number;
  maxMs: number;
}

const MAX_LOADS = 60;
/** Sin llamadas pendientes durante este tiempo = la página ya cargó. */
const QUIET_MS = 200;
/** Más allá de esto no se espera (páginas que consultan sin parar). */
const GIVE_UP_MS = 30_000;

const loads: PageLoad[] = [];
const stats = new Map<string, CommandStat>();
let current: (PageLoad & { t0: number; lastEnd: number }) | null = null;
let pending = 0;
let quietTimer: ReturnType<typeof setTimeout> | undefined;
let listeners: (() => void)[] = [];

const now = () => performance.now();
const notify = () => listeners.forEach((l) => l());

export function onPerfChange(l: () => void) {
  listeners.push(l);
  return () => {
    listeners = listeners.filter((x) => x !== l);
  };
}

export const pageLoads = () => [...loads];
export const commandStats = () => [...stats.values()].sort((a, b) => b.maxMs - a.maxMs);

function finish() {
  if (!current) return;
  const { t0, lastEnd, ...load } = current;
  load.readyMs = Math.round(Math.max(lastEnd, t0 + (load.paintMs ?? 0)) - t0);
  load.calls.sort((a, b) => b.ms - a.ms);
  loads.unshift(load);
  loads.length = Math.min(loads.length, MAX_LOADS);
  current = null;
  notify();
}

function checkQuiet() {
  clearTimeout(quietTimer);
  if (!current || current.paintMs === null) return;
  if (now() - current.t0 > GIVE_UP_MS) return finish();
  if (pending === 0) quietTimer = setTimeout(() => pending === 0 && finish(), QUIET_MS);
}

/** Primera visita a una página: empieza a medir. `t0`: desde cuándo (0 = desde que se abrió la ventana). */
export function pageOpened(page: string, t0 = now()) {
  // StrictMode repite los inicializadores: la misma apertura no se duplica.
  if (current && current.page === page && current.t0 === t0) return;
  if (current) finish();
  current = { page, at: Math.round(t0), codeMs: null, paintMs: null, readyMs: null, calls: [], t0, lastEnd: t0 };
}

/** Terminó de descargarse código de una página (lo avisa `lazyPage`). */
export function codeLoaded(startedAt: number) {
  if (!current || startedAt < current.t0) return;
  // Varias piezas pueden cargarse a la vez: cuenta la que acaba más tarde.
  current.codeMs = Math.max(current.codeMs ?? 0, Math.round(now() - current.t0));
}

/** La página ya está pintada. */
export function pagePainted(page: string) {
  if (!current || current.page !== page || current.paintMs !== null) return;
  current.paintMs = Math.round(now() - current.t0);
  checkQuiet();
}

/** Arranque de la interfaz: cuándo empezó a ejecutarse React (ms desde que se abrió la ventana). */
let appStart: number | null = null;
export function appStarted() {
  appStart ??= Math.round(now());
}
export const appStartMs = () => appStart;

/** Envuelve cada llamada al backend para saber cuánto tarda y cuándo terminan todas. */
export function trackCall<T>(cmd: string, p: Promise<T>): Promise<T> {
  const t = now();
  pending++;
  const done = () => {
    pending--;
    const ms = Math.round(now() - t);
    const s = stats.get(cmd) ?? { cmd, count: 0, totalMs: 0, maxMs: 0 };
    s.count++;
    s.totalMs += ms;
    s.maxMs = Math.max(s.maxMs, ms);
    stats.set(cmd, s);
    if (current && t >= current.t0) {
      current.calls.push({ cmd, ms });
      current.lastEnd = now();
    }
    checkQuiet();
  };
  p.then(done, done);
  return p;
}
