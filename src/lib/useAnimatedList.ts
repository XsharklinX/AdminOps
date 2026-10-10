// Filas que entran y salen con animación (1.2.9): al desaparecer algo de una
// lista, su fila se pliega un momento y las demás suben; lo que llega nuevo baja
// y se resalta. Devuelve la lista con los que están saliendo todavía incluidos.
import { useEffect, useMemo, useRef, useState } from "react";
import { motionOk } from "../components/motion";

export type RowState = "idle" | "enter" | "leave";

export interface AnimatedRow<T> {
  item: T;
  key: string;
  state: RowState;
}

/** Une los elementos actuales con los que se acaban de ir (cada uno en el sitio donde estaba). */
export function mergeLeaving<T>(rows: AnimatedRow<T>[], ghosts: { item: T; key: string; at: number }[]): AnimatedRow<T>[] {
  const out = [...rows];
  for (const g of [...ghosts].sort((a, b) => a.at - b.at)) {
    if (out.some((r) => r.key === g.key)) continue;
    out.splice(Math.min(g.at, out.length), 0, { item: g.item, key: g.key, state: "leave" });
  }
  return out;
}

export function useAnimatedList<T>(items: T[], keyOf: (t: T) => string, ms = 240): AnimatedRow<T>[] {
  const prev = useRef<T[] | null>(null);
  const [ghosts, setGhosts] = useState<{ item: T; key: string; at: number }[]>([]);
  const [entering, setEntering] = useState<Set<string>>(new Set());
  const signature = items.map(keyOf).join("\u0001");

  useEffect(() => {
    const before = prev.current;
    prev.current = items;
    if (!before || !motionOk()) return;
    const now = new Set(items.map(keyOf));
    const was = new Set(before.map(keyOf));
    const gone = before.map((item, at) => ({ item, at, key: keyOf(item) })).filter((g) => !now.has(g.key));
    const fresh = items.map(keyOf).filter((k) => !was.has(k));
    // Un cambio enorme (otra lista entera) no se anima: sería un parpadeo.
    if (gone.length + fresh.length > 6) return;
    let t1 = 0;
    let t2 = 0;
    if (gone.length) {
      setGhosts((g) => [...g, ...gone]);
      t1 = window.setTimeout(() => setGhosts((g) => g.filter((x) => !gone.some((y) => y.key === x.key))), ms);
    }
    if (fresh.length) {
      setEntering(new Set(fresh));
      t2 = window.setTimeout(() => setEntering(new Set()), 600);
    }
    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
    };
    // La lista se vuelve a crear en cada pintado: lo que importa son sus claves.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [signature]);

  return useMemo(
    () =>
      mergeLeaving(
        items.map((item) => {
          const key = keyOf(item);
          return { item, key, state: (entering.has(key) ? "enter" : "idle") as RowState };
        }),
        ghosts,
      ),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [signature, ghosts, entering],
  );
}
