// Ventana estrecha (1.2.9): por debajo de 1000 px la barra lateral se pliega a
// iconos y las pantallas aprietan los márgenes, para que AdminOps quepa en media
// pantalla, al lado del programa que se está arreglando.
import { useEffect, useState } from "react";

export const NARROW_PX = 1000;

export function useNarrow(): boolean {
  const [narrow, setNarrow] = useState(() => window.innerWidth < NARROW_PX);
  useEffect(() => {
    const on = () => setNarrow(window.innerWidth < NARROW_PX);
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);
  return narrow;
}
