// Respuestas que llegan tarde.
//
// Un efecto pide algo al sistema y guarda el resultado en el estado. Si mientras
// tanto cambia lo que se está mirando (otro usuario, otro cliente, otra pestaña)
// o la página se cierra, la respuesta de la consulta anterior llega después y
// pisa a la nueva: se ven los datos del equipo que ya no está seleccionado.
//
// No es raro: pasa cada vez que una consulta lenta (inventario, sensores, perfil
// de un usuario) se cruza con un clic rápido, y es de los fallos que no se
// reproducen cuando los buscas.
import { useEffect, type DependencyList } from "react";

/**
 * Igual que `useEffect`, pero el efecto recibe `vigente()`: devuelve `false` en
 * cuanto el efecto se limpia, ya sea porque cambiaron sus dependencias o porque
 * la página se cerró. Lo que llegue después se descarta.
 *
 * ```ts
 * useLiveEffect((vigente) => {
 *   usersApi.profileSize(user.sid).then((p) => vigente() && setProfile(p)).catch(() => {});
 * }, [user.sid]);
 * ```
 */
export function useLiveEffect(effect: (vigente: () => boolean) => void | (() => void), deps: DependencyList): void {
  useEffect(() => {
    let live = true;
    const cleanup = effect(() => live);
    return () => {
      live = false;
      cleanup?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- las dependencias las pone quien llama; ahí sí se revisan
  }, deps);
}
