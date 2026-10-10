// Registro de errores de la propia AdminOps (1.2.9): lo que falla dentro de la
// aplicación, agrupado y sin datos personales. Local: nada se envía solo.
import { invoke } from "./core";

export interface ErrorGroup {
  source: "pantalla" | "orden";
  place: string;
  message: string;
  count: number;
  first: number;
  last: number;
  version: string;
}

export const errorLogApi = {
  list: () => invoke<ErrorGroup[]>("error_log_list"),
  /** El texto para pegar en un aviso de fallo. */
  report: () => invoke<string>("error_log_report"),
  clear: () => invoke<void>("error_log_clear"),
};
