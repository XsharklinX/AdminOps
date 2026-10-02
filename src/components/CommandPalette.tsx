import { AppWindow, CornerDownLeft, FileText, Lightbulb, Search, SlidersHorizontal, Sparkles, UserRound, Wrench, Zap } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { NAV, pageLabel, type PageId } from "./Sidebar";
import { useToast } from "./feedback";
import { contactsApi, lanApi, libraryApi, officeApi, toolboxApi, tweaksApi, type Contact, type Solution, type TextTemplate, type ToolboxView } from "../lib/api";
import { openCase } from "../lib/currentCase";
import { recognize, type Recognized } from "../lib/recognize";
import { BUILTIN_SOLUTIONS } from "../lib/solutionsCatalog";
import { useLiveEffect } from "../lib/useLiveEffect";
import { EmptyLine } from "./ui";

/** Páginas del catálogo de ajustes, por categoría. */
const CATEGORY_PAGE: Record<string, PageId> = {
  privacy: "privacy",
  performance: "performance",
  repair: "repair",
  services: "services",
  cleanup: "cleanup",
  security: "security",
};

const KIND_LABEL = { smart: "Sugerido", page: "Página", tool: "Herramienta", tweak: "Ajuste", repair: "Reparación", action: "Acción", contact: "Contacto", solution: "Solución", template: "Plantilla" };

type Kind = keyof typeof KIND_LABEL;

interface Entry {
  key: string;
  kind: Kind;
  title: string;
  subtitle?: string;
  search: string;
  run: () => void | Promise<unknown>;
}

const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

// Se cargan una vez y se reutilizan entre aperturas.
let toolsCache: ToolboxView | null = null;
let tweaksCache: { id: string; name: string; description: string; category: string }[] | null = null;

export interface PaletteAction {
  id: string;
  title: string;
  subtitle?: string;
  /** Palabras extra para encontrarla (no se muestran). */
  keywords?: string;
  /** Si devuelve una promesa, su texto se muestra como aviso (y su error también). */
  run: () => void | Promise<unknown>;
}

/** La búsqueda de siempre: todas las palabras, y el título que empieza igual, antes. */
function searchEntries(entries: Entry[], q: string): Entry[] {
  const words = q.split(/\s+/);
  return entries
    .map((e) => {
      const title = norm(e.title);
      if (!words.every((w) => e.search.includes(w))) return null;
      const score = (title.startsWith(q) ? 4 : title.includes(q) ? 2 : 0) + (e.kind === "page" ? 1 : 0);
      return { e, score };
    })
    .filter((x): x is { e: Entry; score: number } => x !== null)
    .sort((a, b) => b.score - a.score || a.e.title.localeCompare(b.e.title))
    .slice(0, 40)
    .map((x) => x.e);
}

/**
 * Qué se puede hacer con lo que se ha escrito. Solo acciones que funcionan de
 * verdad desde aquí: si una página no sabe recibir un equipo, no se promete.
 */
function smartEntries(r: Recognized, go: (page: PageId, focus?: string | null) => void): Entry[] {
  const e = (key: string, title: string, subtitle: string, run: () => void | Promise<unknown>): Entry => ({ key: `smart:${key}`, kind: "smart", title, subtitle, search: "", run });
  const v = r.value;
  switch (r.kind) {
    case "computer":
      return [
        e("pc-secrets", `Contraseñas de ${v}`, "LAPS y recuperación de BitLocker del dominio", () => go("people", `pc:${v}`)),
        e("pc-rdp", `Conectar a ${v}`, "Escritorio remoto", () => officeApi.rdp(v)),
        e("pc-case", `Abrir un caso con ${v}`, "Lo que hagas queda apuntado", () => openCase({ machine: v })),
      ];
    case "person":
      return [
        e("person", `Buscar a «${v}» en el dominio`, "Su cuenta, si está bloqueada, su equipo", () => go("people", v)),
        e("person-case", `Abrir un caso con ${v}`, "Lo que hagas queda apuntado", () => openCase(r.sure ? { sam: v, person: v } : { person: v })),
      ];
    case "extension":
      return [e("ext", `Buscar la extensión ${v} en el dominio`, "De quién es y su ficha", () => go("people", v))];
    case "ip":
      return [
        e("ip-web", `Abrir la página de ${v}`, "Router, impresora o lo que sea que responda ahí", () => lanApi.openDevice(v)),
        e("ip-tools", "Herramientas de red", "Red → Herramientas: ping, traceroute y puertos", () => go("nettools")),
      ];
    case "printer":
      return [e("printer", "Revisar las impresoras", "Por qué no imprime y qué hacer", () => go("printers"))];
    case "ticket":
      return [e("ticket", `Abrir un caso para el ticket ${v}`, "Lo que hagas queda apuntado", () => openCase({ ticket: v }))];
  }
}

export function CommandPalette({
  open,
  onClose,
  onNavigate,
  actions,
}: {
  open: boolean;
  onClose: () => void;
  onNavigate: (page: PageId, focus?: string | null) => void;
  actions: PaletteAction[];
}) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const [tools, setTools] = useState(toolsCache);
  const [tweaks, setTweaks] = useState(tweaksCache);
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [solutions, setSolutions] = useState<Solution[]>([]);
  const [templates, setTemplates] = useState<TextTemplate[]>([]);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const toast = useToast();

  useLiveEffect(
    (vigente) => {
      if (!open) return;
      setQuery("");
      setIndex(0);
      window.setTimeout(() => input.current?.focus(), 0);
      if (!toolsCache) toolboxApi.list().then((t) => vigente() && setTools((toolsCache = t))).catch(() => {});
      if (!tweaksCache) tweaksApi.index().then((t) => vigente() && setTweaks((tweaksCache = t))).catch(() => {});
      // Sin caché: la agenda cambia a menudo.
      contactsApi.list().then((l) => vigente() && setContacts(l.filter((c) => !c.deleted))).catch(() => {});
      // Las del técnico y las que trae AdminOps: Ctrl+K encuentra ambas.
      libraryApi
        .list("solutions")
        .then((l) => vigente() && setSolutions([...l, ...BUILTIN_SOLUTIONS]))
        .catch(() => vigente() && setSolutions(BUILTIN_SOLUTIONS));
      libraryApi.list("templates").then((t) => vigente() && setTemplates(t)).catch(() => {});
    },
    [open],
  );

  const entries = useMemo<Entry[]>(() => {
    const out: Entry[] = NAV.map((n) => ({ key: `page:${n.id}`, kind: "page", title: pageLabel(n.id), search: norm(`${pageLabel(n.id)} ${n.label}`), run: () => onNavigate(n.id) }));
    for (const a of actions)
      out.push({ key: `action:${a.id}`, kind: "action", title: a.title, subtitle: a.subtitle, search: norm(`${a.title} ${a.subtitle ?? ""} ${a.keywords ?? ""}`), run: a.run });
    for (const t of tweaks ?? []) {
      const page = CATEGORY_PAGE[t.category];
      if (!page) continue;
      out.push({
        key: `tweak:${t.id}`,
        kind: t.category === "repair" ? "repair" : "tweak",
        title: t.name,
        subtitle: pageLabel(page),
        search: norm(`${t.name} ${t.description}`),
        run: () => onNavigate(page, t.id),
      });
    }
    for (const t of tools?.tools ?? []) {
      out.push({
        key: `tool:${t.id}`,
        kind: "tool",
        title: t.name,
        subtitle: t.unavailable ?? t.description,
        search: norm(`${t.name} ${t.description} ${t.keywords}`),
        // Lo que pide confirmación (reinicios) o no está disponible se abre en su página.
        run: () =>
          t.confirm || t.unavailable || (t.needsAdmin && !tools?.elevated)
            ? onNavigate("tools")
            : toolboxApi.launch(t.id).catch((e) => toast("error", `${t.name}: ${e}`)),
      });
    }
    for (const c of contacts) {
      const reach = [c.extension && `Ext. ${c.extension}`, c.phone, c.mobile, c.email].filter(Boolean).join(" · ");
      out.push({
        key: `contact:${c.id}`,
        kind: "contact",
        title: c.name,
        subtitle: [reach, c.reason].filter(Boolean).join(" — "),
        search: norm([c.name, c.role, c.company, c.extension, c.phone, c.mobile, c.email, c.reason, ...c.tags].join(" ")),
        run: () => onNavigate("contacts", c.id),
      });
    }
    for (const s of solutions)
      out.push({ key: `solution:${s.id}`, kind: "solution", title: s.title, subtitle: s.problem, search: norm(`${s.title} ${s.problem} ${s.solution} ${s.tags.join(" ")}`), run: () => onNavigate("knowledge", `solution:${s.id}`) });
    for (const t of templates)
      out.push({ key: `template:${t.id}`, kind: "template", title: t.name, subtitle: t.category || "Plantilla de texto", search: norm(`${t.name} ${t.category} ${t.body}`), run: () => onNavigate("knowledge", `template:${t.id}`) });
    for (const c of tools?.custom ?? []) {
      out.push({
        key: `tool:${c.id}`,
        kind: "tool",
        title: c.name,
        subtitle: c.target,
        search: norm(`${c.name} ${c.target}`),
        run: () => toolboxApi.launch(c.id).catch((e) => toast("error", `${c.name}: ${e}`)),
      });
    }
    return out;
  }, [tools, tweaks, contacts, solutions, templates, actions, onNavigate, toast]);

  // Lo que se ha escrito es un equipo, una persona, una IP…: sus acciones.
  const smart = useMemo(() => {
    const r = recognize(query);
    return r ? { sure: r.sure, entries: smartEntries(r, onNavigate) } : null;
  }, [query, onNavigate]);

  const results = useMemo(() => {
    const q = norm(query.trim());
    if (!q) return entries.filter((e) => e.kind === "page" || e.kind === "action").slice(0, 40);
    const found = searchEntries(entries, q);
    if (!smart) return found;
    return (smart.sure ? [...smart.entries, ...found] : [...found, ...smart.entries]).slice(0, 40);
  }, [entries, query, smart]);

  useEffect(() => setIndex(0), [query]);
  useEffect(() => {
    list.current?.querySelector(`[data-i="${index}"]`)?.scrollIntoView({ block: "nearest" });
  }, [index]);

  if (!open) return null;

  const pick = (e: Entry | undefined) => {
    if (!e) return;
    onClose();
    const r = e.run();
    if (r instanceof Promise) r.then((m) => typeof m === "string" && m && toast("ok", m)).catch((err) => toast("error", `${e.title}: ${err}`));
  };

  const icon = (k: Kind) => {
    const I = { smart: Sparkles, page: CornerDownLeft, tool: AppWindow, tweak: SlidersHorizontal, repair: Wrench, action: Zap, contact: UserRound, solution: Lightbulb, template: FileText }[k];
    return <I size={14} className="shrink-0 text-neon" />;
  };

  return (
    <div className="fixed inset-0 z-50 flex justify-center bg-black/50 pt-[12vh]" onClick={onClose}>
      <div className="flex h-fit max-h-[70vh] w-[620px] flex-col overflow-hidden rounded-xl border border-line-2 bg-panel shadow-2xl" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center gap-2 border-b border-line px-4">
          <Search size={16} className="text-mute" />
          <input
            ref={input}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setIndex((i) => Math.min(i + 1, results.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setIndex((i) => Math.max(i - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                pick(results[index]);
              } else if (e.key === "Escape") {
                onClose();
              }
            }}
            placeholder="Busca una página, acción, problema, herramienta o ajuste…"
            className="flex-1 bg-transparent py-3.5 text-sm text-ink outline-none placeholder:text-mute"
          />
          <kbd className="rounded border border-line px-1.5 font-mono text-[11px] text-mute">Esc</kbd>
        </div>
        <div ref={list} className="overflow-y-auto py-1">
          {results.map((e, i) => (
            <button
              key={e.key}
              data-i={i}
              onMouseMove={() => setIndex(i)}
              onClick={() => pick(e)}
              className={`flex w-full items-center gap-3 px-4 py-2 text-left ${i === index ? "bg-neon/10" : ""}`}
            >
              {icon(e.kind)}
              <span className="min-w-0 flex-1">
                <span className={`block truncate text-sm ${i === index ? "text-neon" : "text-ink"}`}>{e.title}</span>
                {e.subtitle && <span className="block truncate text-[11px] text-mute">{e.subtitle}</span>}
              </span>
              <span className="shrink-0 text-[11px] tracking-wide text-mute">{KIND_LABEL[e.kind]}</span>
            </button>
          ))}
          {results.length === 0 && <EmptyLine>Nada coincide con «{query}».</EmptyLine>}
        </div>
        <div className="flex gap-4 border-t border-line px-4 py-2 text-[11px] text-mute">
          <span>↑↓ moverse</span>
          <span>Enter abrir</span>
          <span>Alt+← / Alt+→ página anterior / siguiente</span>
          <span>Ctrl+, ajustes</span>
        </div>
      </div>
    </div>
  );
}
