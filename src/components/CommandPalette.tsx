import { AppWindow, BookOpen, Bug, CornerDownLeft, FileText, Lightbulb, ListTree, Mail, MessagesSquare, Pin, Search, Settings as SettingsIcon, SlidersHorizontal, Sparkles, UserRound, Wrench, X, Zap } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { NAV, allowedInMode, isPageId, navLabel, pageLabel, visibleAreas, type Area, type PageId } from "./Sidebar";
import type { Badge } from "../lib/machineState";
import { usePrefs } from "../lib/prefs";
import { navKey, PAGE_KEYWORDS, parseNavKey, sectionsOf } from "../lib/sections";
import { useToast } from "./feedback";
import { logQuietly, contactsApi, errorsApi, lanApi, libraryApi, officeApi, toolboxApi, tweaksApi, type Contact, type ErrorInfo, type Solution, type TextTemplate, type ToolboxView } from "../lib/api";
import { openCase } from "../lib/currentCase";
import { recognize, type Recognized } from "../lib/recognize";
import { BUILTIN_SOLUTIONS } from "../lib/solutionsCatalog";
import { useLiveEffect } from "../lib/useLiveEffect";
import { EmptyLine } from "./ui";
import { GUIDE } from "../lib/guide";
import { openHelp, openHelpTopic } from "../lib/help";
import { GLOSSARY as TERMS } from "../lib/glossary";
import { openComm } from "../lib/comms";
import { didYouMean, fuzzyHas, tokens } from "../lib/palette";

/** Páginas del catálogo de ajustes, por categoría. */
const CATEGORY_PAGE: Record<string, PageId> = {
  privacy: "privacy",
  performance: "performance",
  repair: "repair",
  services: "services",
  cleanup: "cleanup",
  security: "security",
};

const KIND_LABEL = { error: "Código de error", smart: "Sugerido", page: "Pantalla", section: "Sección", tool: "Herramienta", tweak: "Ajuste", repair: "Reparación", action: "Acción", contact: "Contacto", solution: "Solución", template: "Plantilla", help: "Guía" };

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
let errorsCache: ErrorInfo[] | null = null;

/** ¿Lo escrito parece un código de error? (0x80070005, 80070005, -2147024891, stop 0x133) */
export function looksLikeCode(q: string): boolean {
  const t = q.trim().toLowerCase().replace(/^stop\s*:?\s*/, "");
  return /^0x[0-9a-f]{1,8}$/.test(t) || /^[0-9a-f]{8}$/.test(t) || /^-\d{9,10}$/.test(t);
}

export interface PaletteAction {
  id: string;
  title: string;
  subtitle?: string;
  /** Palabras extra para encontrarla (no se muestran). */
  keywords?: string;
  /** Si devuelve una promesa, su texto se muestra como aviso (y su error también). */
  run: () => void | Promise<unknown>;
}

/** Palabras de relleno: «la impresora no imprime» busca «impresora imprime». */
const STOP = new Set(["el", "la", "los", "las", "de", "del", "se", "me", "mi", "que", "y", "en", "un", "una", "no", "al", "lo", "por", "con", "para", "es", "esta", "esto"]);

/**
 * La búsqueda de siempre (todas las palabras; el título que empieza igual,
 * antes) y, si encuentra poco, otra que perdona erratas y palabras de relleno.
 */
export function searchEntries(entries: Entry[], q: string): Entry[] {
  const words = q.split(/\s+/).filter(Boolean);
  const meaningful = words.filter((w) => !STOP.has(w));
  const strict = (e: Entry) => words.every((w) => e.search.includes(w));
  const loose = (e: Entry) => {
    if (!meaningful.length) return false;
    let toks: string[] | null = null;
    return meaningful.every((w) => e.search.includes(w) || fuzzyHas((toks ??= tokens(e.search)), w));
  };
  const scored = (match: (e: Entry) => boolean, bonus: number) =>
    entries
      .map((e) => {
        if (!match(e)) return null;
        const title = norm(e.title);
        const score = bonus + (title.startsWith(q) ? 4 : title.includes(q) ? 2 : 0) + (e.kind === "page" || e.kind === "section" ? 1 : 0);
        return { e, score };
      })
      .filter((x): x is { e: Entry; score: number } => x !== null);
  let found = scored(strict, 10);
  if (found.length < 5) {
    const seen = new Set(found.map((x) => x.e.key));
    found = [...found, ...scored(loose, 0).filter((x) => !seen.has(x.e.key))];
  }
  return found
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
  onSection,
  badges = {},
  actions,
  initialQuery = "",
}: {
  open: boolean;
  onClose: () => void;
  onNavigate: (page: PageId, focus?: string | null) => void;
  /** Ir a una pantalla y, si se dice, a una de sus secciones. */
  onSection: (page: PageId, section?: string | null) => void;
  badges?: Record<string, Badge>;
  actions: PaletteAction[];
  /** Texto con el que se abre (clic derecho → «Buscar en Ctrl+K»). */
  initialQuery?: string;
}) {
  const prefs = usePrefs();
  const areas = visibleAreas("dashboard");
  const areaOfPage = (p: PageId) => areas.find((a) => a.pages.includes(p)) ?? null;
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const [tools, setTools] = useState(toolsCache);
  const [tweaks, setTweaks] = useState(tweaksCache);
  const [errors, setErrors] = useState(errorsCache);
  // Código pegado que no está en el diccionario: se pregunta al programa (sabe leer los de Windows).
  const [codeHit, setCodeHit] = useState<ErrorInfo[]>([]);
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [solutions, setSolutions] = useState<Solution[]>([]);
  const [templates, setTemplates] = useState<TextTemplate[]>([]);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const toast = useToast();

  useLiveEffect(
    (vigente) => {
      if (!open) return;
      setQuery(initialQuery);
      setIndex(0);
      window.setTimeout(() => input.current?.focus(), 0);
      if (!toolsCache) toolboxApi.list().then((t) => vigente() && setTools((toolsCache = t))).catch(logQuietly("CommandPalette"));
      if (!tweaksCache) tweaksApi.index().then((t) => vigente() && setTweaks((tweaksCache = t))).catch(logQuietly("CommandPalette"));
      if (!errorsCache) errorsApi.all().then((t) => vigente() && setErrors((errorsCache = t))).catch(logQuietly("CommandPalette"));
      // Sin caché: la agenda cambia a menudo.
      contactsApi.list().then((l) => vigente() && setContacts(l.filter((c) => !c.deleted))).catch(logQuietly("CommandPalette"));
      // Las del técnico y las que trae AdminOps: Ctrl+K encuentra ambas.
      libraryApi
        .list("solutions")
        .then((l) => vigente() && setSolutions([...l, ...BUILTIN_SOLUTIONS]))
        .catch(() => vigente() && setSolutions(BUILTIN_SOLUTIONS));
      libraryApi.list("templates").then((t) => vigente() && setTemplates(t)).catch(logQuietly("CommandPalette"));
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- el texto inicial solo cuenta al abrir
    [open],
  );

  const entries = useMemo<Entry[]>(() => {
    const out: Entry[] = NAV.filter((n) => allowedInMode(n.id, prefs.mode)).map((n) => ({
      key: `page:${n.id}`,
      kind: "page",
      title: pageLabel(n.id),
      subtitle: areaOfPage(n.id)?.label,
      search: norm(`${pageLabel(n.id)} ${n.label} ${PAGE_KEYWORDS[n.id] ?? ""}`),
      run: () => onNavigate(n.id),
    }));
    // Cada sección, con su ruta y sus otros nombres: «AD» lleva a Dominio.
    for (const n of NAV) {
      if (!allowedInMode(n.id, prefs.mode)) continue;
      const where = [areaOfPage(n.id)?.label, pageLabel(n.id)].filter(Boolean).join(" › ");
      for (const sec of sectionsOf(n.id))
        out.push({
          key: `section:${n.id}:${sec.id}`,
          kind: "section",
          title: sec.label,
          subtitle: where,
          search: norm(`${sec.label} ${pageLabel(n.id)} ${sec.keywords ?? ""}`),
          run: () => onSection(n.id, sec.id),
        });
    }
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
    // La guía: «cómo se hace…» lleva al apartado que lo explica.
    for (const ch of GUIDE)
      for (const t of ch.topics)
        out.push({ key: `help:${t.id}`, kind: "help", title: t.title, subtitle: ch.title, search: norm(`${t.title} ${ch.title} ${t.what}`), run: () => openHelpTopic(ch.id, t.id) });
    // El diccionario de códigos de error: «0x80070005», «dpc watchdog».
    for (const er of errors ?? [])
      out.push({
        key: `error:${er.code}`,
        kind: "error",
        title: `${er.code} · ${er.name}`,
        subtitle: `${er.what} Qué hacer: ${er.todo}`,
        search: norm(`${er.code} ${er.code.replace(/^0x/i, "")} ${er.name} ${er.name.replace(/_/g, " ")}`),
        run: () => (er.page && isPageId(er.page) ? onNavigate(er.page) : navigator.clipboard?.writeText(`${er.code} ${er.name}\n${er.what}\nPor qué: ${er.why}\nQué hacer: ${er.todo}`).then(() => `${er.code}: explicación copiada.`)),
      });
    // «qué es tpm», «bcd»: el término explicado, con qué hacer.
    for (const g of TERMS)
      out.push({ key: `term:${g.term}`, kind: "help", title: `¿Qué es ${g.term}?`, subtitle: `${g.what} ${g.todo}`, search: norm(`que es ${g.term} ${(g.aliases ?? []).join(" ")} ${g.what}`), run: () => openHelp("glossary") });
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
    // eslint-disable-next-line react-hooks/exhaustive-deps -- las áreas salen de las preferencias (prefs.layout), que ya están en la lista
  }, [tools, tweaks, errors, contacts, solutions, templates, actions, onNavigate, onSection, toast, prefs.mode, prefs.layout, prefs.pageLabels]);

  useEffect(() => {
    if (!looksLikeCode(query)) return setCodeHit([]);
    let alive = true;
    errorsApi
      .lookup(query)
      .then((r) => alive && setCodeHit(r))
      .catch(() => alive && setCodeHit([]));
    return () => {
      alive = false;
    };
  }, [query]);

  // Lo que se ha escrito es un equipo, una persona, una IP…: sus acciones.
  const smart = useMemo(() => {
    const r = recognize(query);
    return r ? { sure: r.sure, entries: smartEntries(r, onNavigate) } : null;
  }, [query, onNavigate]);

  const results = useMemo(() => {
    const q = norm(query.trim());
    // Sin texto se ve el mapa, no una lista.
    if (!q) return [];
    let found = searchEntries(entries, q);
    // Un código conocido por su número va primero, aunque se haya escrito distinto (decimal, sin 0x).
    if (codeHit.length) {
      const top = codeHit.map((er) => entries.find((e) => e.key === `error:${er.code}`) ?? { key: `error:${er.code}`, kind: "error" as const, title: `${er.code} · ${er.name}`, subtitle: `${er.what} Qué hacer: ${er.todo}`, search: "", run: () => undefined });
      found = [...top, ...found.filter((e) => !top.some((t) => t.key === e.key))];
    }
    if (!smart) return found;
    return (smart.sure ? [...smart.entries, ...found] : [...found, ...smart.entries]).slice(0, 40);
  }, [entries, query, smart, codeHit]);

  // Palabras que conoce Ctrl+K, para «¿Quisiste decir…?» cuando no encuentra nada.
  const vocabulary = useMemo(() => {
    const v = new Set<string>();
    for (const e of entries) for (const w of tokens(norm(e.title))) if (w.length > 3) v.add(w);
    return v;
  }, [entries]);
  const suggestion = useMemo(() => (query.trim() && results.length === 0 ? didYouMean(norm(query.trim()), vocabulary) : null), [query, results.length, vocabulary]);

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
    const I = { error: Bug, smart: Sparkles, page: CornerDownLeft, section: ListTree, tool: AppWindow, tweak: SlidersHorizontal, repair: Wrench, action: Zap, contact: UserRound, solution: Lightbulb, template: FileText, help: BookOpen }[k];
    return <I size={14} className="shrink-0 text-neon" />;
  };

  const go = (page: PageId, section: string | null = null) => {
    onClose();
    onSection(page, section);
  };
  const desc = (p: PageId) => (NAV.find((n) => n.id === p)?.help ?? "").split(/(?<=\.)\s/)[0];
  const pins = prefs.sidebar.favorites.filter((k) => isPageId(parseNavKey(k).page) && allowedInMode(parseNavKey(k).page, prefs.mode));
  const tone = (b: Badge | undefined) => (!b ? "" : b.tone === "bad" ? "text-bad" : b.tone === "warn" ? "text-warn" : b.tone === "ok" ? "text-ok" : "text-dim");
  const fixed: { page: PageId; Icon: typeof Mail }[] = (
    [
      { page: "tools", Icon: Wrench },
      { page: "teams", Icon: MessagesSquare },
      { page: "mail", Icon: Mail },
      { page: "settings", Icon: SettingsIcon },
    ] as { page: PageId; Icon: typeof Mail }[]
  ).filter((f) => allowedInMode(f.page, prefs.mode));

  const areaCard = (a: Area) => {
    const Icon = a.icon;
    const nSections = a.pages.reduce((t, p) => t + sectionsOf(p).length, 0);
    return (
      <section key={a.id} className="mb-3 break-inside-avoid overflow-hidden rounded-xl border border-line bg-panel">
        <header className="flex items-center gap-2.5 border-b border-line px-3.5 py-2.5">
          <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-panel-2 text-ink">
            <Icon size={16} strokeWidth={1.7} />
          </span>
          <span className="min-w-0 flex-1 leading-tight">
            <b className="block text-sm font-semibold text-ink">{a.label}</b>
            <span className="text-[11px] text-mute">
              {a.pages.length} {a.pages.length === 1 ? "pantalla" : "pantallas"}
              {nSections > 0 && ` · ${nSections} secciones`}
            </span>
          </span>
        </header>
        {a.pages.map((p) => {
          const list = sectionsOf(p);
          const b = badges[navKey(p)];
          return (
            <div key={p} className="border-b border-line px-3.5 py-2.5 last:border-b-0">
              <button onClick={() => go(p)} className="group flex w-full items-center gap-2 text-left">
                <b className="text-[13px] font-semibold text-ink group-hover:text-neon">{pageLabel(p)}</b>
                {b && <span className={`ml-auto font-mono text-[10.5px] ${tone(b)}`} title={b.title}>{b.text}</span>}
              </button>
              <p className="mt-0.5 line-clamp-2 text-[11.5px] leading-snug text-mute">{desc(p)}</p>
              {list.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-1">
                  {list.map((sec) => {
                    const sb = badges[navKey(p, sec.id)];
                    return (
                      <button
                        key={sec.id}
                        onClick={() => go(p, sec.id)}
                        className="flex items-center gap-1 rounded-md border border-line-2 bg-void px-2 py-0.5 text-[11.5px] text-dim transition-colors hover:border-neon/60 hover:text-ink"
                        title={sb?.title}
                      >
                        {sec.label}
                        {sb && <span className={`font-mono text-[10px] ${tone(sb)}`}>{sb.text}</span>}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </section>
    );
  };

  return (
    <div className="fixed inset-0 z-50 flex justify-center bg-black/55 pt-[5vh] backdrop-blur-[2px]" onClick={onClose}>
      <div
        className="flex h-[88vh] w-[1120px] max-w-[95vw] flex-col overflow-hidden rounded-2xl border border-line-2 bg-void shadow-2xl"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Todo AdminOps"
      >
        <div className="border-b border-line bg-panel px-5 pt-4 pb-3">
          <div className="mb-2.5 flex items-baseline justify-between gap-3">
            <h2 className="text-base font-semibold tracking-tight text-ink">Todo AdminOps</h2>
            <span className="text-xs text-mute">
              {areas.reduce((t, a) => t + a.pages.length, 0)} pantallas · {areas.reduce((t, a) => t + a.pages.reduce((n, p) => n + sectionsOf(p).length, 0), 0)} secciones
            </span>
            <button onClick={onClose} className="ml-auto rounded-md p-1 text-mute hover:bg-panel-2 hover:text-ink" title="Cerrar (Esc)" aria-label="Cerrar">
              <X size={16} />
            </button>
          </div>
          <label className="flex h-11 items-center gap-2.5 rounded-xl border border-line-2 bg-void px-3 focus-within:border-neon focus-within:ring-3 focus-within:ring-neon/15">
            <Search size={17} className="shrink-0 text-mute" />
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
              placeholder="Qué quieres hacer: dominio, no imprime, liberar espacio, un equipo, una persona, una IP…"
              className="min-w-0 flex-1 bg-transparent text-[15px] text-ink outline-none placeholder:text-mute"
              aria-label="Buscar en todo AdminOps"
            />
            <span className="hidden shrink-0 items-center gap-1 text-[11px] text-mute md:flex">
              <kbd className="rounded border border-line-2 px-1 font-mono">↑↓</kbd> elegir <kbd className="rounded border border-line-2 px-1 font-mono">Intro</kbd> abrir{" "}
              <kbd className="rounded border border-line-2 px-1 font-mono">Esc</kbd> cerrar
            </span>
          </label>
        </div>

        {query.trim() ? (
          <div ref={list} className="min-h-0 flex-1 overflow-y-auto px-3 py-2">
            {results.map((e, i) => (
              <button
                key={e.key}
                data-i={i}
                onMouseMove={() => setIndex(i)}
                onClick={() => pick(e)}
                className={`flex w-full items-center gap-3 rounded-lg border px-3 py-2 text-left ${i === index ? "border-neon/60 bg-neon/10" : "border-transparent"}`}
              >
                <span className={`grid size-7 shrink-0 place-items-center rounded-md ${i === index ? "bg-neon/15" : "bg-panel-2"}`}>{icon(e.kind)}</span>
                <span className="min-w-0 flex-1">
                  <span className={`block truncate text-sm ${i === index ? "text-ink" : "text-ink"} font-medium`}>{e.title}</span>
                  {e.subtitle && <span className="block truncate text-[11.5px] text-mute">{e.subtitle}</span>}
                </span>
                <span className="shrink-0 text-[11px] tracking-wide text-mute">{KIND_LABEL[e.kind]}</span>
                {i === index && <span className="shrink-0 font-mono text-[10.5px] text-neon">Intro ↵</span>}
              </button>
            ))}
            {results.length === 0 && (
              <EmptyLine>
                Nada coincide con «{query}».{" "}
                {suggestion ? (
                  <button onClick={() => setQuery(suggestion)} className="text-neon underline-offset-2 hover:underline">
                    ¿Quisiste decir «{suggestion}»?
                  </button>
                ) : (
                  "Prueba con otra palabra: «impresora», «contraseña», «espacio», o pega un código de error."
                )}
              </EmptyLine>
            )}
          </div>
        ) : (
          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
            {pins.length > 0 && (
              <>
                <div className="mb-2 font-mono text-[10.5px] tracking-[0.08em] text-mute uppercase">Fijados</div>
                <div className="mb-5 grid grid-cols-[repeat(auto-fill,minmax(190px,1fr))] gap-2">
                  {pins.map((k) => {
                    const { page, section } = parseNavKey(k);
                    const a = areaOfPage(page);
                    const Icon = a?.icon ?? Pin;
                    return (
                      <button key={k} onClick={() => go(page, section)} className="flex min-w-0 items-center gap-2.5 rounded-xl border border-line bg-panel px-3 py-2 text-left transition-colors hover:border-neon/60">
                        <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-neon/10 text-neon">
                          <Icon size={15} strokeWidth={1.8} />
                        </span>
                        <span className="min-w-0">
                          <b className="block truncate text-[13px] font-semibold text-ink">{navLabel(k)}</b>
                          <span className="block truncate text-[11px] text-mute">{[a?.label, section ? pageLabel(page) : null].filter(Boolean).join(" › ")}</span>
                        </span>
                      </button>
                    );
                  })}
                </div>
              </>
            )}
            <div className="mb-2 font-mono text-[10.5px] tracking-[0.08em] text-mute uppercase">Por áreas</div>
            <div className="columns-1 gap-3 md:columns-2 xl:columns-3">{areas.map(areaCard)}</div>
            {fixed.length > 0 && (
              <>
                <div className="mt-2 mb-2 font-mono text-[10.5px] tracking-[0.08em] text-mute uppercase">Siempre a mano</div>
                <div className="grid grid-cols-[repeat(auto-fill,minmax(190px,1fr))] gap-2">
                  {fixed.map(({ page, Icon }) => (
                    <button
                      key={page}
                      onClick={() => {
                        if (page === "teams" || page === "mail") {
                          onClose();
                          openComm(page);
                        } else go(page);
                      }}
                      className="flex items-center gap-2.5 rounded-xl border border-line bg-panel px-3 py-2 text-left transition-colors hover:border-neon/60"
                    >
                      <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-panel-2 text-ink">
                        <Icon size={15} strokeWidth={1.7} />
                      </span>
                      <span className="min-w-0">
                        <b className="block truncate text-[13px] font-semibold text-ink">{pageLabel(page)}</b>
                        <span className="block truncate text-[11px] text-mute">{page === "teams" || page === "mail" ? "También en la barra de arriba" : "Abajo en la columna de áreas"}</span>
                      </span>
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
