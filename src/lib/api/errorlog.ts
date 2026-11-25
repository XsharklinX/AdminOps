// Registro de errores de la propia AdminOps (1.2.9): lo que falla dentro de la
// aplicación, agrupado y sin datos personales. Local: nada se envía solo.
import { invoke } from "./core";

export interface ErrorGroup {
  /** «cierre»: AdminOps se cerró de golpe (abrir a salvo). */
  source: "pantalla" | "orden" | "cierre";
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

/** Abrir a salvo (1.5): AdminOps se cerró de golpe dos veces seguidas. */
export interface SafeStartInfo {
  safe: boolean;
  badCloses: number;
  /** Qué estaba haciendo cada vez (vacío si no se sabe). */
  doing: string[];
}

export const safeStartApi = {
  info: () => invoke<SafeStartInfo>("safe_start_info"),
  /** La pantalla en la que está, por si se cierra de golpe. */
  page: (label: string) => invoke<void>("session_page", { label }),
  /** «Abrir normal»: olvida los cierres y reinicia AdminOps. */
  leave: () => invoke<void>("safe_start_leave"),
  report: () => invoke<string>("safe_start_report"),
};
