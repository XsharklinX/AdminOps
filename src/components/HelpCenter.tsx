// La ayuda de AdminOps en una ventana: la guía de cada pantalla, el glosario,
// las preguntas frecuentes, las novedades de cada versión, los términos de uso
// y reportar un problema.
import { ArrowUpRight, BookOpen, Bug, CircleHelp, FileText, Library, Loader2, Mail, Minus, Plus, Scale, Search, Sparkles, TriangleAlert, Wrench, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { appApi } from "../lib/api";
import { RELEASES } from "../lib/changelog";
import { FAQ, GLOSSARY, GUIDE, type GuideTopic } from "../lib/guide";
import { closeHelp, openHelp, parseTerms, searchHelp, SUPPORT_EMAIL, useHelp, useHelpFocus, type HelpTab } from "../lib/help";
import { goToPage } from "../lib/navigate";
import { useLiveEffect } from "../lib/useLiveEffect";
import { useToast } from "./feedback";
import { Button, EmptyLine, ErrorState, inputClass, Loading } from "./ui";

const TABS: { id: HelpTab; label: string; icon: typeof BookOpen }[] = [
  { id: "guide", label: "Guía", icon: BookOpen },
  { id: "glossary", label: "Glosario", icon: Library },
  { id: "faq", label: "Preguntas frecuentes", icon: CircleHelp },
  { id: "news", label: "Novedades", icon: Sparkles },
  { id: "terms", label: "Términos de uso", icon: Scale },
  { id: "report", label: "Reportar un problema", icon: Bug },
];

export function HelpCenter({ version }: { version: string }) {
  const tab = useHelp();
  const [query, setQuery] = useState("");
  const [chapter, setChapter] = useState(GUIDE[0].id);
  const body = useRef<HTMLDivElement>(null);

  // Escape cierra; al cambiar de pestaña o de capítulo se vuelve arriba.
  useEffect(() => {
    if (!tab) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && closeHelp();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tab]);
  useEffect(() => {
    body.current?.scrollTo({ top: 0 });
  }, [tab, chapter, query]);

  // Desde Ctrl+K: el capítulo de ese apartado, y la vista en él.
  const focus = useHelpFocus();
  useEffect(() => {
    if (!focus) return;
    setQuery("");
    setChapter(focus.chapter);
    const t = window.setTimeout(() => {
      const el = document.getElementById(`help-${focus.topic}`);
      el?.scrollIntoView({ block: "start", behavior: "smooth" });
      el?.classList.add("setting-flash");
    }, 80);
    return () => window.clearTimeout(t);
  }, [focus]);

  const results = useMemo(() => searchHelp(query), [query]);
  if (!tab) return null;
  const searching = query.trim().length > 0 && (tab === "guide" || tab === "glossary" || tab === "faq");

  return (
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/55" onClick={closeHelp}>
      <div className="flex h-[88vh] w-[1080px] max-w-[95vw] overflow-hidden rounded-2xl border border-line-2 bg-panel shadow-2xl" onClick={(e) => e.stopPropagation()} role="dialog" aria-label="Ayuda de AdminOps">
        <nav className="flex w-60 shrink-0 flex-col border-r border-line bg-void/40" aria-label="Secciones de la ayuda">
          <div className="px-5 pt-5 pb-3">
            <div className="text-sm font-semibold text-ink">Ayuda de AdminOps</div>
            <div className="font-mono text-[11px] text-mute">v{version}</div>
          </div>
          <ul className="space-y-0.5 px-2">
            {TABS.map((t) => (
              <li key={t.id}>
                <button
                  onClick={() => openHelp(t.id)}
                  aria-current={tab === t.id ? "page" : undefined}
                  className={`flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm transition-colors ${tab === t.id ? "bg-neon/10 font-medium text-neon" : "text-dim hover:bg-panel-2 hover:text-ink"}`}
                >
                  <t.icon size={15} strokeWidth={1.7} className="shrink-0" /> {t.label}
                </button>
              </li>
            ))}
          </ul>
          {tab === "guide" && !searching && (
            <>
              <div className="mt-4 px-5 pb-1 text-[10px] font-medium tracking-wide text-mute uppercase">Capítulos</div>
              <ul className="min-h-0 flex-1 space-y-0.5 overflow-y-auto px-2 pb-3">
                {GUIDE.map((c, i) => (
                  <li key={c.id}>
                    <button
                      onClick={() => setChapter(c.id)}
                      className={`flex w-full items-baseline gap-2 rounded-md px-3 py-1.5 text-left text-[13px] transition-colors ${chapter === c.id ? "bg-panel-2 text-ink" : "text-dim hover:text-ink"}`}
                    >
                      <span className="w-4 shrink-0 font-mono text-[10px] text-mute">{i + 1}</span>
                      {c.title}
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </nav>

        <div className="flex min-w-0 flex-1 flex-col">
          <header className="flex items-center gap-3 border-b border-line px-6 py-3">
            {tab === "guide" || tab === "glossary" || tab === "faq" ? (
              <div className="relative min-w-0 flex-1">
                <Search size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
                <input
                  autoFocus
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder="Buscar en la ayuda: deshacer, pendrive, permisos, contraseña…"
                  className={`${inputClass} pl-9`}
                  aria-label="Buscar en la ayuda"
                />
              </div>
            ) : (
              <h2 className="min-w-0 flex-1 text-base font-semibold text-ink">{TABS.find((t) => t.id === tab)?.label}</h2>
            )}
            <button onClick={closeHelp} className="rounded-md p-1.5 text-mute hover:bg-panel-2 hover:text-ink" title="Cerrar (Esc)">
              <X size={16} />
            </button>
          </header>

          <div ref={body} className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
            {searching ? (
              <SearchResults query={query} results={results} />
            ) : tab === "guide" ? (
              <Chapter id={chapter} onNext={(id) => setChapter(id)} />
            ) : tab === "glossary" ? (
              <Glossary entries={GLOSSARY} />
            ) : tab === "faq" ? (
              <Faq entries={FAQ} />
            ) : tab === "news" ? (
              <News />
            ) : tab === "terms" ? (
              <Terms />
            ) : (
              <ReportProblem />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function Chapter({ id, onNext }: { id: string; onNext: (id: string) => void }) {
  const index = Math.max(
    0,
    GUIDE.findIndex((c) => c.id === id),
  );
  const c = GUIDE[index];
  const next = GUIDE[index + 1];
  return (
    <div className="mx-auto max-w-3xl">
      <h2 className="text-xl font-semibold tracking-tight text-ink">{c.title}</h2>
      <p className="mt-1 text-sm text-dim">{c.intro}</p>
      <div className="mt-5 space-y-4">
        {c.topics.map((t) => (
          <Topic key={t.id} t={t} />
        ))}
      </div>
      {next && (
        <button onClick={() => onNext(next.id)} className="mt-6 flex items-center gap-1.5 text-sm text-neon hover:underline">
          Siguiente: {next.title} <ArrowUpRight size={14} />
        </button>
      )}
    </div>
  );
}

function Topic({ t, chapter }: { t: GuideTopic; chapter?: string }) {
  return (
    <article id={chapter ? undefined : `help-${t.id}`} className="scroll-mt-4 rounded-xl border border-line bg-panel-2/30 p-4">
      <div className="flex flex-wrap items-start gap-3">
        <div className="min-w-0 flex-1">
          {chapter && <div className="text-[10px] tracking-wide text-mute uppercase">{chapter}</div>}
          <h3 className="text-[15px] font-semibold text-ink">{t.title}</h3>
        </div>
        {t.go && (
          <button
            onClick={() => {
              closeHelp();
              goToPage(t.go![0], t.go![1] ?? null);
            }}
            className="flex shrink-0 items-center gap-1 rounded-md border border-line-2 px-2.5 py-1 text-xs text-dim transition-colors hover:border-neon/40 hover:text-neon"
          >
            Abrir esta pantalla <ArrowUpRight size={12} />
          </button>
        )}
      </div>
      <p className="mt-1.5 text-sm leading-relaxed text-dim">{t.what}</p>
      {t.how && (
        <Block icon={<Wrench size={12} />} title="Cómo se usa">
          <ol className="list-decimal space-y-1 pl-5">
            {t.how.map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ol>
        </Block>
      )}
      {t.notes && (
        <Block icon={<TriangleAlert size={12} />} title="Ten en cuenta">
          <ul className="list-disc space-y-1 pl-5">
            {t.notes.map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ul>
        </Block>
      )}
    </article>
  );
}

function Block({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <div className="mt-3">
      <div className="mb-1 flex items-center gap-1.5 text-[11px] font-medium tracking-wide text-mute uppercase">
        {icon} {title}
      </div>
      <div className="text-[13px] leading-relaxed text-dim">{children}</div>
    </div>
  );
}

function Glossary({ entries }: { entries: typeof GLOSSARY }) {
  return (
    <div className="mx-auto max-w-3xl">
      <p className="mb-4 text-sm text-dim">Las palabras que aparecen en AdminOps y en el trabajo de soporte, explicadas sin rodeos.</p>
      <dl className="divide-y divide-line/60 rounded-xl border border-line">
        {entries.map((g) => (
          <div key={g.term} className="grid grid-cols-[11rem_1fr] gap-4 px-4 py-2.5">
            <dt className="text-sm font-medium text-ink">{g.term}</dt>
            <dd className="text-[13px] leading-relaxed text-dim">{g.def}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

function Faq({ entries }: { entries: typeof FAQ }) {
  const [open, setOpen] = useState<string | null>(entries[0]?.q ?? null);
  return (
    <ul className="mx-auto max-w-3xl divide-y divide-line/60 rounded-xl border border-line">
      {entries.map((f) => (
        <li key={f.q}>
          <button onClick={() => setOpen(open === f.q ? null : f.q)} className="flex w-full items-center gap-3 px-4 py-3 text-left text-sm text-ink hover:bg-panel-2/50" aria-expanded={open === f.q}>
            <span className="min-w-0 flex-1">{f.q}</span>
            {open === f.q ? <Minus size={14} className="shrink-0 text-mute" /> : <Plus size={14} className="shrink-0 text-mute" />}
          </button>
          {open === f.q && <p className="px-4 pb-3 text-[13px] leading-relaxed text-dim">{f.a}</p>}
        </li>
      ))}
    </ul>
  );
}

function SearchResults({ query, results }: { query: string; results: ReturnType<typeof searchHelp> }) {
  const total = results.topics.length + results.glossary.length + results.faq.length;
  if (total === 0) return <EmptyLine>Nada en la ayuda coincide con «{query}». Prueba con otra palabra, o repórtalo si es algo que falta.</EmptyLine>;
  return (
    <div className="mx-auto max-w-3xl space-y-6">
      {results.topics.length > 0 && (
        <section className="space-y-4">
          <h2 className="text-xs font-medium tracking-wide text-mute uppercase">En la guía · {results.topics.length}</h2>
          {results.topics.slice(0, 20).map((h) => (
            <Topic key={`${h.chapter.id}-${h.topic.id}`} t={h.topic} chapter={h.chapter.title} />
          ))}
        </section>
      )}
      {results.faq.length > 0 && (
        <section>
          <h2 className="mb-2 text-xs font-medium tracking-wide text-mute uppercase">Preguntas frecuentes · {results.faq.length}</h2>
          <Faq entries={results.faq} />
        </section>
      )}
      {results.glossary.length > 0 && (
        <section>
          <h2 className="mb-2 text-xs font-medium tracking-wide text-mute uppercase">Glosario · {results.glossary.length}</h2>
          <dl className="divide-y divide-line/60 rounded-xl border border-line">
            {results.glossary.map((g) => (
              <div key={g.term} className="grid grid-cols-[11rem_1fr] gap-4 px-4 py-2.5">
                <dt className="text-sm font-medium text-ink">{g.term}</dt>
                <dd className="text-[13px] leading-relaxed text-dim">{g.def}</dd>
              </div>
            ))}
          </dl>
        </section>
      )}
    </div>
  );
}

function News() {
  const list = (title: string, items: string[] | undefined, tone: string) =>
    items?.length ? (
      <div className="mt-2">
        <div className={`text-[11px] font-medium tracking-wide uppercase ${tone}`}>{title}</div>
        <ul className="mt-1 list-disc space-y-1 pl-5 text-[13px] leading-relaxed text-dim">
          {items.map((i) => (
            <li key={i}>{i}</li>
          ))}
        </ul>
      </div>
    ) : null;
  return (
    <div className="mx-auto max-w-3xl">
      <p className="mb-4 text-sm text-dim">Lo que trae cada versión, de la más nueva a la primera.</p>
      <ol className="relative ml-2 space-y-5 border-l border-line pl-6">
        {RELEASES.map((r, i) => (
          <li key={r.version} className="relative">
            <span className={`absolute top-1.5 -left-[29px] size-2.5 rounded-full ring-4 ring-panel ${i === 0 ? "bg-neon" : "bg-line-2"}`} />
            <div className="flex flex-wrap items-baseline gap-x-3">
              <h3 className="font-mono text-base font-semibold text-ink">{r.version}</h3>
              <span className="text-sm text-dim">{r.title}</span>
              {i === 0 && <span className="rounded border border-neon/40 px-1.5 text-[10px] text-neon">la que tienes</span>}
            </div>
            {list("Nuevo", r.added, "text-ok")}
            {list("Arreglado", r.fixed, "text-mute")}
            {list("Quitado", r.removed, "text-warn")}
          </li>
        ))}
      </ol>
    </div>
  );
}

function Terms() {
  const [text, setText] = useState<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  useLiveEffect((vigente) => {
    appApi
      .terms()
      .then((t) => vigente() && setText(t))
      .catch((e) => vigente() && setFailed(String(e)));
  }, []);
  if (failed) return <ErrorState message={failed} />;
  if (text === null) return <Loading />;
  const [title, ...blocks] = parseTerms(text);
  return (
    <div className="mx-auto max-w-3xl">
      <div className="mb-4 flex items-center gap-2 rounded-lg border border-line bg-panel-2/40 px-3 py-2 text-xs text-dim">
        <FileText size={13} className="shrink-0 text-mute" /> Son los mismos términos que se aceptan al instalar AdminOps.
      </div>
      <h2 className="sr-only">{title?.text}</h2>
      {blocks.map((b) =>
        b.heading ? (
          <h3 key={b.text} className="mt-5 mb-1.5 text-sm font-semibold text-ink first:mt-0">
            {b.text}
          </h3>
        ) : (
          <p key={b.text} className="mb-2 text-[13px] leading-relaxed text-dim">
            {b.text}
          </p>
        ),
      )}
    </div>
  );
}

function ReportProblem() {
  const [what, setWhat] = useState("");
  const [steps, setSteps] = useState("");
  const [contact, setContact] = useState("");
  const [busy, setBusy] = useState<"mail" | "manual" | null>(null);
  const [sent, setSent] = useState<"mail" | "manual" | null>(null);
  const toast = useToast();

  const send = async (manual: boolean) => {
    setBusy(manual ? "manual" : "mail");
    try {
      await appApi.reportProblem(what, steps, contact, manual);
      setSent(manual ? "manual" : "mail");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const ready = what.trim().length >= 10;
  return (
    <div className="mx-auto max-w-2xl space-y-4">
      <p className="text-sm text-dim">
        Cuenta qué pasó y AdminOps prepara el correo para el autor (<span className="font-mono text-ink select-text">{SUPPORT_EMAIL}</span>) con un archivo de diagnóstico adjunto. Se abre en tu
        programa de correo: lo revisas y lo envías tú.
      </p>

      {sent && (
        <div className="rounded-xl border border-ok/30 bg-ok/5 px-4 py-3 text-sm text-ink">
          {sent === "mail"
            ? "Correo preparado con el archivo adjunto. Revísalo en tu programa de correo y pulsa Enviar."
            : "Correo abierto con el texto, y la carpeta del archivo de diagnóstico: arrástralo al correo y envíalo."}
          <span className="mt-1 block text-xs text-dim">Si no se abrió nada, este equipo no tiene programa de correo: usa el otro botón, o escribe a la dirección de arriba.</span>
        </div>
      )}

      <label className="block">
        <span className="mb-1 block text-xs text-dim">Qué pasó (lo que esperabas y lo que ocurrió)</span>
        <textarea value={what} onChange={(e) => setWhat(e.target.value)} rows={5} maxLength={4000} placeholder="Al abrir Tickets y poner mis credenciales, la página se queda en blanco." className={`${inputClass} resize-y`} />
      </label>
      <label className="block">
        <span className="mb-1 block text-xs text-dim">Qué estabas haciendo y cómo se repite (opcional)</span>
        <textarea value={steps} onChange={(e) => setSteps(e.target.value)} rows={3} maxLength={4000} placeholder="Pasa siempre, desde ayer. Antes actualicé a la versión nueva." className={`${inputClass} resize-y`} />
      </label>
      <label className="block">
        <span className="mb-1 block text-xs text-dim">Cómo contestarte (opcional)</span>
        <input value={contact} onChange={(e) => setContact(e.target.value)} maxLength={200} placeholder="Tu nombre y un correo o teléfono" className={inputClass} />
      </label>

      <div className="flex flex-wrap gap-2">
        <Button onClick={() => void send(false)} disabled={!ready || busy !== null}>
          {busy === "mail" ? <Loader2 size={14} className="animate-spin" /> : <Mail size={14} />} Preparar el correo
        </Button>
        <Button kind="ghost" onClick={() => void send(true)} disabled={!ready || busy !== null} title="Para Gmail u Outlook en el navegador: abre el correo con el texto y la carpeta del archivo para arrastrarlo">
          {busy === "manual" && <Loader2 size={14} className="animate-spin" />} Sin adjunto (correo web)
        </Button>
      </div>

      <div className="rounded-lg border border-line bg-panel-2/40 px-3 py-2.5 text-[11px] leading-relaxed text-mute">
        <b className="text-dim">Qué lleva el archivo de diagnóstico:</b> tu descripción, la versión de AdminOps y de Windows, el registro de actividad de AdminOps y el último diagnóstico guardado de
        este equipo. Puede contener el nombre del equipo y del usuario. No incluye contraseñas ni los archivos de tus clientes y contactos. AdminOps no envía nada por su cuenta: si no envías el correo, no
        sale nada de este equipo.
      </div>
    </div>
  );
}
