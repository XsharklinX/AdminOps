// Filtro de texto del inventario: «windows 10 sin tpm», un modelo, un cliente…
import type { Client, Machine } from "./api";

type Row = { client: Client; machine: Machine };

const fold = (t: string) => t.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();

/**
 * ¿Encaja el equipo con lo escrito? Cada palabra tiene que aparecer en su ficha;
 * «sin tpm» y «sin ssd» buscan lo que falta.
 */
export function matchesMachine(r: Row, query: string): boolean {
  const q = fold(query.trim());
  if (!q) return true;
  const i = r.machine.inventory;
  const text = fold([r.client.name, r.machine.host, i?.manufacturer, i?.model, i?.cpu, i?.os, i?.gpu, i?.disks, i?.ip, i?.serial, ...(i?.reasons ?? [])].filter(Boolean).join(" "));
  const parts = q.split(/\s+/);
  for (let k = 0; k < parts.length; k++) {
    const w = parts[k];
    if (w === "sin" && parts[k + 1]) {
      const missing = parts[++k];
      if (missing === "tpm" ? i?.tpm !== false : missing === "ssd" ? !!i && /ssd|nvme/i.test(i.disks) : text.includes(missing)) return false;
      continue;
    }
    if (!text.includes(w)) return false;
  }
  return true;
}
