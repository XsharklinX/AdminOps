// Textos del informe y de su correo con campos que se rellenan solos:
// {cliente}, {contacto}, {numero}, {fecha}, {equipo}, {empresa}, {tecnico}.

export const REPORT_FIELDS = ["cliente", "contacto", "numero", "fecha", "equipo", "empresa", "tecnico"] as const;
export type ReportField = (typeof REPORT_FIELDS)[number];

/** Sustituye los campos conocidos; los que no tienen valor quedan vacíos y los desconocidos, tal cual. */
export function fillTemplate(text: string, values: Partial<Record<ReportField, string>>): string {
  return text
    .replace(/\{(\w+)\}/g, (all, key: string) => ((REPORT_FIELDS as readonly string[]).includes(key) ? (values[key as ReportField] ?? "").trim() : all))
    .replace(/[ \t]+([,.;:])/g, "$1")
    .replace(/[ \t]{2,}/g, " ");
}

/** Número del informe a partir del nombre del archivo («Informe_2026-0012…»). */
export const reportNumber = (path: string) => /Informe_(\d{4}-\d{4})/.exec(path)?.[1] ?? "";

/** Destinatarios: el correo del cliente más los de su plantilla, sin repetir. */
export function recipients(main: string | undefined, extra: string | undefined): string {
  const all = [main ?? "", ...(extra ?? "").split(/[,;\s]+/)].map((x) => x.trim()).filter((x) => x.includes("@"));
  return [...new Set(all.map((x) => x.toLowerCase()))].map((l) => all.find((x) => x.toLowerCase() === l)!).join("; ");
}
