// Lógica de la agenda: búsqueda con prefijos, filtros, orden, grupos,
// duplicados, vCard y la vista guardada (viaja con AdminOps en el portable).
import type { Contact } from "./api";

export type ViewMode = "cards" | "table" | "directory";
export type GroupBy = "none" | "company" | "role" | "tag" | "letter";
export type SortBy = "name" | "company" | "extension" | "used" | "recent" | "created";
export type Quick = "favorites" | "frequent" | "withExt" | "withEmail" | "incomplete" | "recent";
export type Column = "role" | "company" | "extension" | "phone" | "mobile" | "email" | "reason" | "availability" | "tags";

export interface Filters {
  query: string;
  quick: Quick[];
  tags: string[];
  tagMode: "all" | "any";
  company: string;
}

export interface SavedSearch extends Filters {
  name: string;
}

export interface ContactsView {
  view: ViewMode;
  group: GroupBy;
  sort: SortBy;
  columns: Column[];
  filters: Filters;
  saved: SavedSearch[];
}

export const EMPTY_FILTERS: Filters = { query: "", quick: [], tags: [], tagMode: "all", company: "" };

export const COLUMNS: { id: Column; label: string }[] = [
  { id: "role", label: "Cargo o área" },
  { id: "company", label: "Empresa" },
  { id: "extension", label: "Extensión" },
  { id: "phone", label: "Teléfono" },
  { id: "mobile", label: "Móvil" },
  { id: "email", label: "Correo" },
  { id: "reason", label: "Para qué" },
  { id: "availability", label: "Disponibilidad" },
  { id: "tags", label: "Etiquetas" },
];

export const QUICK: { id: Quick; label: string; hint: string }[] = [
  { id: "favorites", label: "Favoritos", hint: "Marcados con estrella" },
  { id: "frequent", label: "Más usados", hint: "Los que más copias, llamas o escribes" },
  { id: "withExt", label: "Con extensión", hint: "Tienen extensión" },
  { id: "withEmail", label: "Con correo", hint: "Tienen correo" },
  { id: "incomplete", label: "Sin completar", hint: "Les falta un teléfono o el correo" },
  { id: "recent", label: "Añadidos hace poco", hint: "En los últimos 14 días" },
];

export const SORTS: { id: SortBy; label: string }[] = [
  { id: "name", label: "Nombre" },
  { id: "company", label: "Empresa" },
  { id: "extension", label: "Extensión" },
  { id: "used", label: "Más usados" },
  { id: "recent", label: "Usados hace poco" },
  { id: "created", label: "Añadidos hace poco" },
];

export const GROUPS: { id: GroupBy; label: string }[] = [
  { id: "none", label: "Sin agrupar" },
  { id: "company", label: "Empresa" },
  { id: "role", label: "Cargo o área" },
  { id: "tag", label: "Etiqueta" },
  { id: "letter", label: "Letra" },
];

/** Colores de etiqueta (nombre → hex). */
export const TAG_COLORS: Record<string, string> = {
  gris: "#94a3b8",
  rojo: "#f87171",
  naranja: "#fb923c",
  ambar: "#fbbf24",
  verde: "#4ade80",
  turquesa: "#2dd4bf",
  cian: "#22d3ee",
  azul: "#60a5fa",
  violeta: "#a78bfa",
  rosa: "#f472b6",
};

const KEY = "adminops.contacts.view";

export function loadView(): ContactsView {
  const def: ContactsView = { view: "cards", group: "none", sort: "name", columns: ["role", "company", "extension", "phone", "email", "reason", "tags"], filters: EMPTY_FILTERS, saved: [] };
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (!v || typeof v !== "object") return def;
    return { ...def, ...v, filters: { ...EMPTY_FILTERS, ...(v.filters ?? {}) }, saved: Array.isArray(v.saved) ? v.saved : [] };
  } catch {
    return def;
  }
}

export function saveView(v: ContactsView) {
  try {
    localStorage.setItem(KEY, JSON.stringify(v));
  } catch {
    /* sin almacenamiento: la vista no se recuerda */
  }
}

export const norm = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .trim();

const digits = (s: string) => s.replace(/\D/g, "");

export const allPhones = (c: Contact) => [c.extension, c.phone, c.mobile, ...c.channels.filter((x) => x.kind === "phone").map((x) => x.value)].filter(Boolean);
export const allEmails = (c: Contact) => [c.email, ...c.channels.filter((x) => x.kind === "email").map((x) => x.value)].filter(Boolean);

const haystack = (c: Contact) => norm([c.name, c.role, c.company, c.reason, c.availability, c.notes, ...allPhones(c), ...allEmails(c), ...c.tags].join(" "));

// ---------- Búsqueda con prefijos ----------

/** Prefijos: `ext:21`, `empresa:x`, `cargo:x`, `correo:x`, `motivo:x`, `#etiqueta`, `es:favorito`. */
export const PREFIX_HELP = [
  ["ext:21", "extensión que empieza por 21"],
  ["empresa:telefónica", "empresa"],
  ["cargo:sistemas", "cargo o área"],
  ["correo:@proveedor", "correo"],
  ["motivo:impresoras", "para qué llamarle"],
  ["#urgencias", "etiqueta"],
  ["es:favorito", "solo favoritos"],
  ["-palabra", "que NO contenga"],
];

interface Term {
  field: "any" | "ext" | "company" | "role" | "email" | "reason" | "tag" | "fav";
  value: string;
  not: boolean;
}

export function parseQuery(q: string): Term[] {
  const out: Term[] = [];
  // Respeta "frases entre comillas".
  const parts = q.match(/-?(?:[a-zA-Záéíóúñ]+:)?"[^"]*"|\S+/g) ?? [];
  for (let raw of parts) {
    const not = raw.startsWith("-") && raw.length > 1;
    if (not) raw = raw.slice(1);
    let field: Term["field"] = "any";
    let value = raw;
    if (raw.startsWith("#")) {
      field = "tag";
      value = raw.slice(1);
    } else {
      const m = raw.match(/^([a-zA-Záéíóúñ]+):(.*)$/);
      if (m) {
        const k = norm(m[1]);
        const map: Record<string, Term["field"]> = {
          ext: "ext",
          extension: "ext",
          empresa: "company",
          company: "company",
          cargo: "role",
          area: "role",
          correo: "email",
          email: "email",
          motivo: "reason",
          para: "reason",
          tag: "tag",
          etiqueta: "tag",
          es: "fav",
        };
        if (map[k]) {
          field = map[k];
          value = m[2];
        }
      }
    }
    value = norm(value.replace(/"/g, ""));
    if (value || field === "fav") out.push({ field, value, not });
  }
  return out;
}

function termMatches(c: Contact, t: Term): boolean {
  switch (t.field) {
    case "ext":
      return norm(c.extension).startsWith(t.value);
    case "company":
      return norm(c.company).includes(t.value);
    case "role":
      return norm(c.role).includes(t.value);
    case "email":
      return allEmails(c).some((e) => norm(e).includes(t.value));
    case "reason":
      return norm(c.reason).includes(t.value);
    case "tag":
      return c.tags.some((x) => norm(x).includes(t.value));
    case "fav":
      return c.favorite;
    default:
      return haystack(c).includes(t.value) || (digits(t.value).length >= 3 && allPhones(c).some((p) => digits(p).includes(digits(t.value))));
  }
}

const DAY = 86_400;
const nowSecs = () => Date.now() / 1000;

export function isIncomplete(c: Contact) {
  return allPhones(c).length === 0 || allEmails(c).length === 0;
}

/** Umbral de «más usados»: el 20 % con más usos (mínimo 2 usos). */
export function frequentIds(list: Contact[]): Set<string> {
  const used = list.filter((c) => c.uses >= 2).sort((a, b) => b.uses - a.uses);
  return new Set(used.slice(0, Math.max(6, Math.ceil(list.length * 0.2))).map((c) => c.id));
}

export function applyFilters(list: Contact[], f: Filters): Contact[] {
  const terms = parseQuery(f.query);
  const frequent = f.quick.includes("frequent") ? frequentIds(list) : null;
  const tags = f.tags.map(norm);
  return list.filter((c) => {
    for (const q of f.quick) {
      if (q === "favorites" && !c.favorite) return false;
      if (q === "frequent" && !frequent?.has(c.id)) return false;
      if (q === "withExt" && !c.extension) return false;
      if (q === "withEmail" && allEmails(c).length === 0) return false;
      if (q === "incomplete" && !isIncomplete(c)) return false;
      if (q === "recent" && nowSecs() - c.created > 14 * DAY) return false;
    }
    if (tags.length) {
      const mine = c.tags.map(norm);
      const ok = f.tagMode === "all" ? tags.every((t) => mine.includes(t)) : tags.some((t) => mine.includes(t));
      if (!ok) return false;
    }
    if (f.company && norm(c.company) !== norm(f.company)) return false;
    return terms.every((t) => termMatches(c, t) !== t.not);
  });
}

export const hasFilters = (f: Filters) => !!(f.query.trim() || f.quick.length || f.tags.length || f.company);

// ---------- Orden y grupos ----------

const collator = new Intl.Collator("es", { numeric: true, sensitivity: "base" });

export function sortContacts(list: Contact[], by: SortBy): Contact[] {
  const byName = (a: Contact, b: Contact) => collator.compare(a.name, b.name);
  const empty = (s: string) => (s ? 0 : 1);
  const cmp: Record<SortBy, (a: Contact, b: Contact) => number> = {
    name: byName,
    company: (a, b) => empty(a.company) - empty(b.company) || collator.compare(a.company, b.company) || byName(a, b),
    extension: (a, b) => empty(a.extension) - empty(b.extension) || collator.compare(a.extension, b.extension) || byName(a, b),
    used: (a, b) => b.uses - a.uses || byName(a, b),
    recent: (a, b) => b.lastUsed - a.lastUsed || byName(a, b),
    created: (a, b) => b.created - a.created || byName(a, b),
  };
  return [...list].sort((a, b) => Number(b.favorite) - Number(a.favorite) || cmp[by](a, b));
}

export interface Group {
  key: string;
  label: string;
  items: Contact[];
}

export function groupContacts(list: Contact[], by: GroupBy): Group[] {
  if (by === "none") return [{ key: "all", label: "", items: list }];
  const map = new Map<string, Group>();
  const add = (label: string, c: Contact) => {
    const key = norm(label) || "~";
    if (!map.has(key)) map.set(key, { key, label, items: [] });
    map.get(key)!.items.push(c);
  };
  for (const c of list) {
    if (by === "company") add(c.company || "Sin empresa", c);
    else if (by === "role") add(c.role || "Sin cargo", c);
    else if (by === "letter") {
      const l = norm(c.name).charAt(0).toUpperCase();
      add(/[A-Z]/.test(l) ? l : "#", c);
    } else if (c.tags.length) c.tags.forEach((t) => add(t, c));
    else add("Sin etiqueta", c);
  }
  // Los grupos «Sin…» al final.
  return [...map.values()].sort((a, b) => Number(a.label.startsWith("Sin ")) - Number(b.label.startsWith("Sin ")) || collator.compare(a.label, b.label));
}

// ---------- Duplicados ----------

/** Grupos de posibles duplicados: mismo nombre, mismo correo o mismo teléfono. */
export function findDuplicates(list: Contact[]): Contact[][] {
  const parent = new Map<string, string>();
  const find = (x: string): string => {
    const p = parent.get(x) ?? x;
    if (p === x) return x;
    const r = find(p);
    parent.set(x, r);
    return r;
  };
  const union = (a: string, b: string) => parent.set(find(a), find(b));
  const seen = new Map<string, string>();
  const link = (key: string, id: string) => {
    const other = seen.get(key);
    if (other) union(id, other);
    else seen.set(key, id);
  };
  for (const c of list) {
    link(`n:${norm(c.name).replace(/\s+/g, " ")}`, c.id);
    allEmails(c).forEach((e) => link(`e:${norm(e)}`, c.id));
    [c.phone, c.mobile, ...c.channels.filter((x) => x.kind === "phone").map((x) => x.value)]
      .map(digits)
      .filter((d) => d.length >= 6)
      .forEach((d) => link(`p:${d.slice(-9)}`, c.id));
  }
  const groups = new Map<string, Contact[]>();
  for (const c of list) {
    const r = find(c.id);
    if (!groups.has(r)) groups.set(r, []);
    groups.get(r)!.push(c);
  }
  return [...groups.values()].filter((g) => g.length > 1);
}

// ---------- Texto para copiar y vCard ----------

/** Tarjeta completa para pegar en un ticket o en Teams. */
export function cardText(c: Contact, substitute?: Contact | null): string {
  const lines = [
    [c.name, [c.role, c.company].filter(Boolean).join(", ")].filter(Boolean).join(" — "),
    c.extension && `Ext. ${c.extension}`,
    c.phone && `Tel. ${c.phone}`,
    c.mobile && `Móvil ${c.mobile}`,
    ...c.channels.map((x) => `${x.label || (x.kind === "email" ? "Correo" : "Tel.")} ${x.value}`),
    c.email,
    c.reason && `Para: ${c.reason}`,
    c.availability && `Disponible: ${c.availability}`,
    substitute && `Si no está: ${substitute.name}${substitute.extension ? ` (ext. ${substitute.extension})` : ""}`,
  ];
  return lines.filter(Boolean).join("\n");
}

const vEsc = (s: string) => s.replace(/\\/g, "\\\\").replace(/\n/g, "\\n").replace(/,/g, "\\,").replace(/;/g, "\\;");

export function toVcard(list: Contact[]): string {
  return list
    .map((c) => {
      const parts = c.name.split(" ");
      const last = parts.length > 1 ? parts.slice(1).join(" ") : "";
      const lines = [
        "BEGIN:VCARD",
        "VERSION:3.0",
        `N:${vEsc(last)};${vEsc(parts[0])};;;`,
        `FN:${vEsc(c.name)}`,
        c.company && `ORG:${vEsc(c.company)}`,
        c.role && `TITLE:${vEsc(c.role)}`,
        c.phone && `TEL;TYPE=WORK,VOICE:${c.phone}`,
        c.extension && !c.phone && `TEL;TYPE=WORK,VOICE:${c.extension}`,
        c.mobile && `TEL;TYPE=CELL:${c.mobile}`,
        c.email && `EMAIL;TYPE=INTERNET,WORK:${c.email}`,
        ...c.channels.map((x) => (x.kind === "email" ? `EMAIL;TYPE=INTERNET:${x.value}` : `TEL;TYPE=VOICE:${x.value}`)),
        c.tags.length > 0 && `CATEGORIES:${c.tags.map(vEsc).join(",")}`,
        (c.notes || c.reason || c.extension) &&
          `NOTE:${vEsc([c.extension && `Ext. ${c.extension}`, c.reason && `Para: ${c.reason}`, c.notes].filter(Boolean).join("\n"))}`,
        "END:VCARD",
      ];
      return lines.filter(Boolean).join("\r\n");
    })
    .join("\r\n");
}

const csvCell = (s: string) => (/[;"\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);

export function toCsv(list: Contact[]): string {
  const head = ["Nombre", "Cargo", "Empresa", "Extensión", "Teléfono", "Móvil", "Correo", "Motivo", "Disponibilidad", "Etiquetas", "Notas"];
  const rows = list.map((c) =>
    [
      c.name,
      c.role,
      c.company,
      c.extension,
      c.phone,
      c.mobile,
      c.email,
      c.reason,
      c.availability,
      c.tags.join(", "),
      [c.notes, ...c.channels.map((x) => `${x.label}: ${x.value}`)].filter(Boolean).join("\n"),
    ]
      .map(csvCell)
      .join(";"),
  );
  return [head.join(";"), ...rows].join("\r\n");
}

export const EMPTY_CONTACT: Contact = {
  id: "",
  name: "",
  role: "",
  company: "",
  extension: "",
  phone: "",
  mobile: "",
  email: "",
  channels: [],
  reason: "",
  availability: "",
  substituteId: "",
  clientId: "",
  tags: [],
  notes: "",
  favorite: false,
  uses: 0,
  lastUsed: 0,
  created: 0,
  updated: 0,
  deleted: null,
};

// ---------- Responsable según el tema ----------

/** Palabras que identifican a quien se encarga de cada tema (en etiquetas, «para qué» o cargo). */
const TOPIC_WORDS: Record<string, string[]> = {
  internet: ["internet", "red", "redes", "fibra", "proveedor", "router", "isp", "conexion", "wifi", "telecom"],
  printer: ["impresora", "impresoras", "impresion", "toner", "fotocopiadora", "escaner", "copiadora"],
  hardware: ["hardware", "soporte", "sistemas", "informatica", "tecnico", "ti", "it", "helpdesk"],
  updates: ["sistemas", "informatica", "ti", "it", "windows", "actualizaciones"],
  security: ["seguridad", "antivirus", "sistemas", "informatica", "ti", "it", "ciberseguridad"],
  users: ["usuarios", "dominio", "active directory", "cuentas", "sistemas", "ti", "it", "rrhh"],
  email: ["correo", "email", "outlook", "office", "microsoft 365", "sistemas", "ti"],
};

/** Tema de cada síntoma o página de AdminOps. */
export const TOPIC_OF: Record<string, string> = {
  internet: "internet",
  wifi: "internet",
  router: "internet",
  devices: "internet",
  network: "internet",
  nettools: "internet",
  printer: "printer",
  printers: "printer",
  audio: "hardware",
  bluetooth: "hardware",
  display: "hardware",
  slow: "hardware",
  hardware: "hardware",
  diagnostics: "hardware",
  space: "hardware",
  processes: "hardware",
  troubleshoot: "hardware",
  winupdate: "updates",
  security: "security",
  users: "users",
  domain: "users",
};

/** Quién suele encargarse de un tema (como mucho 3; favoritos y más usados primero). */
export function responsibleFor(topic: string | undefined, contacts: Contact[]): Contact[] {
  const words = topic ? TOPIC_WORDS[topic] : undefined;
  if (!words) return [];
  const has = (text: string) => {
    const t = ` ${norm(text).replace(/[^a-z0-9 ]/g, " ")} `;
    return words.some((w) => t.includes(` ${w} `));
  };
  return contacts
    .filter((c) => !c.deleted)
    .map((c) => ({ c, score: (c.tags.some(has) ? 3 : 0) + (has(c.reason) ? 2 : 0) + (has(c.role) ? 1 : 0) }))
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score || Number(b.c.favorite) - Number(a.c.favorite) || b.c.uses - a.c.uses)
    .slice(0, 3)
    .map((x) => x.c);
}
