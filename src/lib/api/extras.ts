// Ideas del banco de mejoras: etiquetas, valoración del equipo, copias de seguridad,
// reglas de alerta, repuestos y traductor de hallazgos.
import { invoke } from "./core";

// ---------- Etiquetas con QR ----------

export interface LabelInfo {
  host: string;
  owner: string;
  place: string;
  lastVisit: string;
  phone: string;
  extension: string;
}

export const labelsApi = {
  /** El QR (SVG) de la ficha. */
  qr: (info: LabelInfo) => invoke<string>("label_qr", { info }),
  /** Crea el PDF de etiquetas (cols × rows por hoja A4) y lo abre. */
  sheet: (labels: LabelInfo[], cols: number, rows: number) => invoke<string>("label_sheet", { labels, cols, rows }),
};
