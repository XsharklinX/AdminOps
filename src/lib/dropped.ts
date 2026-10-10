// Archivos soltados sobre la ventana: se dejan esperando a la pantalla que sabe
// qué hacer con ellos, que los recoge al montarse o al momento si ya estaba abierta.
import { useEffect, useRef } from "react";

export type DropTarget = "image" | "vault" | "profile";

const EVENT = "adminops-dropped";
const pending = new Map<DropTarget, string>();

/** Entrega un archivo (ruta o, para un perfil, su texto) a la pantalla que lo usa. */
export function deliverDropped(target: DropTarget, value: string) {
  pending.set(target, value);
  window.dispatchEvent(new CustomEvent<DropTarget>(EVENT, { detail: target }));
}

/** La pantalla recibe lo que se soltó para ella. */
export function useDropped(target: DropTarget, onValue: (value: string) => void) {
  const cb = useRef(onValue);
  useEffect(() => {
    cb.current = onValue;
  });
  useEffect(() => {
    const take = () => {
      const v = pending.get(target);
      if (v === undefined) return;
      pending.delete(target);
      cb.current(v);
    };
    take();
    const on = (e: Event) => (e as CustomEvent<DropTarget>).detail === target && take();
    window.addEventListener(EVENT, on);
    return () => window.removeEventListener(EVENT, on);
  }, [target]);
}
