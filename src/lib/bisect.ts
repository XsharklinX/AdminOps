// Encontrar el cambio que lo rompió: búsqueda por mitades sobre una lista de
// cambios. Se supone que el culpable es uno solo y que está entre ellos.
//
// En cada paso hay que tener deshecha una mitad de los sospechosos (y aplicado
// todo lo demás). Si con eso «ya funciona», el culpable está en esa mitad; si
// no, en la otra. Con n cambios nunca hacen falta más de ceil(log2 n) preguntas.

export interface BisectState {
  /** Cuántos cambios hay (se identifican por su posición 0..count-1). */
  count: number;
  /** Los que aún pueden ser el culpable. */
  suspects: number[];
  /** Preguntas ya contestadas. */
  asked: number;
}

export function startBisect(count: number): BisectState {
  return { count, suspects: Array.from({ length: count }, (_, i) => i), asked: 0 };
}

/** ¿Ya se sabe cuál es? */
export function culprit(s: BisectState): number | null {
  return s.suspects.length === 1 ? s.suspects[0] : null;
}

/** Los que hay que tener deshechos ahora para hacer la siguiente pregunta (el culpable, al terminar). */
export function toUndo(s: BisectState): number[] {
  if (s.suspects.length <= 1) return [...s.suspects];
  return s.suspects.slice(0, Math.ceil(s.suspects.length / 2));
}

/** Contesta «¿ya funciona?»: sí → el culpable está entre los deshechos; no → entre los que siguen aplicados. */
export function answer(s: BisectState, works: boolean): BisectState {
  if (s.suspects.length <= 1) return s;
  const half = toUndo(s);
  const suspects = works ? half : s.suspects.filter((i) => !half.includes(i));
  return { ...s, suspects, asked: s.asked + 1 };
}

/** Lo que hay que hacer para pasar del estado de ahora al que se quiere: deshacer unos y volver a aplicar otros. */
export function plan(undoneNow: number[], wanted: number[]): { undo: number[]; redo: number[] } {
  const now = new Set(undoneNow);
  const want = new Set(wanted);
  return {
    undo: wanted.filter((i) => !now.has(i)),
    redo: undoneNow.filter((i) => !want.has(i)),
  };
}

/** Preguntas que hacen falta como máximo con `count` cambios. */
export function maxQuestions(count: number): number {
  return count <= 1 ? 0 : Math.ceil(Math.log2(count));
}

export interface Candidate {
  id: number;
  timestamp: number;
  op: string;
  tweakId: string | null;
  title: string;
  ok: boolean;
  reverted: boolean;
  undoable: boolean;
}

/**
 * Los cambios que entran: aplicados con éxito, que se pueden deshacer y volver a
 * aplicar (tienen ajuste del catálogo) y siguen aplicados, desde `since` (segundos).
 * Si un ajuste se aplicó varias veces, cuenta la última. Del más antiguo al más reciente.
 */
export function candidates(entries: Candidate[], since: number): Candidate[] {
  const seen = new Set<string>();
  const out: Candidate[] = [];
  for (const e of [...entries].sort((a, b) => b.timestamp - a.timestamp || b.id - a.id)) {
    if (e.timestamp < since || e.op !== "apply" || !e.ok || !e.undoable || e.reverted || !e.tweakId) continue;
    if (seen.has(e.tweakId)) continue;
    seen.add(e.tweakId);
    out.push(e);
  }
  return out.reverse();
}
