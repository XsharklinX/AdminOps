/** Las páginas visitadas se mantienen montadas (conservan sus datos al volver).
 * Este contexto dice si la página es la visible, para que las ocultas pausen
 * sus temporizadores, escuchas de teclado y vistas web. */
import { createContext, useContext } from "react";

export const PageActiveContext = createContext(true);

export const usePageActive = () => useContext(PageActiveContext);
