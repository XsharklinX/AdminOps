// Presupuestos: líneas que llegan desde el diagnóstico («¿Reparar o cambiar?»,
// repuestos compatibles) al presupuesto del informe sin volver a escribirlas,
// el precio del catálogo cuando encaja y las plantillas de presupuestos frecuentes.
import { useEffect, useRef } from "react";
import type { CatalogItem, Line } from "./api";

const EVENT = "adminops-quote";
let pending: Line[] = [];

const line = (l: Partial<Line>): Line => ({ description: "", part: false, qty: 1, price: 0, warrantyDays: 0, ...l });

/** Manda líneas al presupuesto (las recoge el editor de cobros del informe). */
export function addToQuote(lines: Partial<Line>[]) {
  pending = [...pending, ...lines.map(line)];
  window.dispatchEvent(new Event(EVENT));
}

/** El editor de cobros recoge lo que haya pendiente (al montarse o al llegar). */
export function useQuoteQueue(onLines: (lines: Line[]) => void) {
  const cb = useRef(onLines);
  useEffect(() => {
    cb.current = onLines;
  });
  useEffect(() => {
    const take = () => {
      if (!pending.length) return;
      const l = pending;
      pending = [];
      cb.current(l);
    };
    take();
    window.addEventListener(EVENT, take);
    return () => window.removeEventListener(EVENT, take);
  }, []);
}

const words = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((w) => w.length > 1);

/** La entrada del catálogo que mejor encaja con un texto («Memoria DDR4 8 GB» → «Memoria 8 GB DDR4 SO-DIMM»). */
export function bestCatalogMatch(text: string, catalog: CatalogItem[]): CatalogItem | null {
  const t = new Set(words(text));
  let best: CatalogItem | null = null;
  let score = 0;
  for (const c of catalog) {
    const w = words(c.name);
    if (!w.length) continue;
    const hits = w.filter((x) => t.has(x)).length;
    const s = hits / w.length;
    if (hits >= 2 && s > score) {
      score = s;
      best = c;
    }
  }
  return score >= 0.5 ? best : null;
}

/** Línea de presupuesto para un texto, con el precio del catálogo si encaja. */
export function lineFor(text: string, catalog: CatalogItem[], fallbackPrice = 0): Partial<Line> {
  const c = bestCatalogMatch(text, catalog);
  return c ? { description: c.name, price: c.price, part: c.part, warrantyDays: c.warrantyDays } : { description: text, price: fallbackPrice, part: true };
}

/** Margen de una entrada del catálogo (%), si se sabe su coste. */
export function margin(c: CatalogItem): number | null {
  if (!c.cost || c.cost <= 0 || c.price <= 0) return null;
  return Math.round(((c.price - c.cost) / c.price) * 100);
}

// ---------- Plantillas de presupuestos frecuentes ----------

const KEY = "adminops.quoteTemplates";

export interface QuoteTemplate {
  name: string;
  lines: Line[];
}

export function quoteTemplates(): QuoteTemplate[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}

export function saveQuoteTemplate(name: string, lines: Line[]): QuoteTemplate[] {
  const list = [...quoteTemplates().filter((t) => t.name !== name), { name, lines }];
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    /* sin almacenamiento */
  }
  return list;
}

export function deleteQuoteTemplate(name: string): QuoteTemplate[] {
  const list = quoteTemplates().filter((t) => t.name !== name);
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    /* sin almacenamiento */
  }
  return list;
}
