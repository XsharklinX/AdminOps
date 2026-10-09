// Cuentas de la tarjeta de la batería: la capacidad de cada semana sobre 100 y
// las marcas de los meses del eje.

/** Capacidad respecto a la de fábrica, en %. */
export const healthOf = (p: { full: number; design: number }) => (p.design ? (p.full / p.design) * 100 : 0);

/** Días desde 1970 de «AAAA-MM-DD» (sin zona horaria: solo se comparan entre sí). */
export function dayNumber(day: string): number {
  const [y, m, d] = day.split("-").map(Number);
  return Date.UTC(y, (m || 1) - 1, d || 1) / 86_400_000;
}

/** La gráfica empieza un poco por debajo del peor dato, sin subir del 50 % si se acerca a él. */
export function floorOf(values: number[]): number {
  if (!values.length) return 0;
  const min = Math.min(...values);
  return Math.max(0, Math.min(50, Math.floor((min - 10) / 10) * 10));
}

const MONTHS = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];

/** Marcas del eje: el primero de cada mes (o de cada trimestre, si son muchos), con el año en enero. */
export function monthTicks(first: string, last: string): { day: number; label: string }[] {
  const a = dayNumber(first);
  const b = dayNumber(last);
  const start = new Date(a * 86_400_000);
  const months = (new Date(b * 86_400_000).getUTCFullYear() - start.getUTCFullYear()) * 12 + new Date(b * 86_400_000).getUTCMonth() - start.getUTCMonth();
  const every = months > 18 ? 3 : months > 8 ? 2 : 1;
  const out: { day: number; label: string }[] = [];
  for (let y = start.getUTCFullYear(), m = start.getUTCMonth() + 1; ; m++) {
    if (m > 11) {
      m = 0;
      y++;
    }
    const day = Date.UTC(y, m, 1) / 86_400_000;
    if (day > b) break;
    if (m % every === 0) out.push({ day, label: m === 0 ? `ene ${String(y).slice(2)}` : MONTHS[m] });
  }
  return out;
}

/** «8 meses», «2 años y 3 meses», «más de 5 años». */
export function monthsText(months: number): string {
  const m = Math.round(months);
  if (m < 1) return "menos de un mes";
  if (m < 24) return `${m} ${m === 1 ? "mes" : "meses"}`;
  if (m > 60) return "más de 5 años";
  const y = Math.floor(m / 12);
  const r = m % 12;
  return r ? `${y} años y ${r} ${r === 1 ? "mes" : "meses"}` : `${y} años`;
}
