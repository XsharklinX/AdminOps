// La barra de arriba: el equipo en una línea.
import { invoke } from "./core";

export interface MachineContext {
  computerName: string;
  /** "domain", "workgroup" o "unknown". */
  join: string;
  /** El dominio (DNS si se sabe) o el grupo de trabajo. */
  joinName: string | null;
  /** "C:". */
  systemDrive: string;
  freeBytes: number;
  totalBytes: number;
}

export interface InternetProbe {
  online: boolean;
  ms: number | null;
}

export const contextApi = {
  /** Nombre, dominio y disco del sistema, leídos de Windows sin PowerShell (milisegundos). */
  machine: () => invoke<MachineContext>("machine_context"),
  /** ¿Hay Internet? Una conexión de prueba con tope de 2 s. */
  internet: () => invoke<InternetProbe>("internet_probe"),
};
