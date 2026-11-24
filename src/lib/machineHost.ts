// El nombre de este equipo, para ligar a él lo que se apunta y puede viajar en el USB (el portable).
import { contextApi } from "./api";

/** El nombre del equipo, o «» si no se pudo leer. */
export const hostName = (): Promise<string> =>
  contextApi
    .machine()
    .then((m) => m.computerName)
    .catch(() => "");
