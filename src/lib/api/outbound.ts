// Qué sale de este equipo (1.2.9): los servicios de Internet con los que AdminOps
// habla por su cuenta, con lo que se envía a cada uno.
import { invoke } from "./core";

export interface OutboundService {
  id: string;
  host: string;
  what: string;
  sends: string;
  when: string;
  /** Se puede apagar desde Ajustes. */
  switchable: boolean;
  enabled: boolean;
  /** Segundos desde 1970 de la última vez que se usó (0: nunca desde que se lleva la cuenta). */
  lastUsed: number;
}

export const outboundApi = {
  list: () => invoke<OutboundService[]>("outbound_list"),
};
