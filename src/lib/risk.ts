// Cómo se enseña el riesgo de un ajuste (etiqueta y colores): el mismo en el catálogo de ajustes
// y en las plantillas.
import type { Risk } from "./api";

export const RISK: Record<Risk, { label: string; cls: string }> = {
  low: { label: "Riesgo bajo", cls: "text-ok border-ok/30 bg-ok/5" },
  medium: { label: "Riesgo medio", cls: "text-warn border-warn/30 bg-warn/5" },
  high: { label: "Riesgo alto", cls: "text-bad border-bad/30 bg-bad/5" },
};
