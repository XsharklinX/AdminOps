import { ChevronDown, History } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { diagApi, logQuietly, tweaksApi, type JournalEntry, type Problem } from "../lib/api";
import { ago, shortDate } from "../lib/format";
import { problemColumns, problemStory } from "../lib/problems";

const DOT = { bad: "bg-bad", warn: "bg-warn" } as Record<string, string>;

/**
 * Problemas de este equipo con memoria: abiertos, en curso (se probó algo) y
 * resueltos, cada uno con su historia. Se pone al día con cada análisis completo.
 */
export function ProblemsBoard({ refresh }: { refresh: number }) {
  const [list, setList] = useState<Problem[] | null>(null);
  const [journal, setJournal] = useState<JournalEntry[]>([]);
  const [open, setOpen] = useState(false);
  const [picked, setPicked] = useState<string | null>(null);

  useEffect(() => {
    diagApi.problems().then(setList).catch(logQuietly("ProblemsBoard"));
    tweaksApi.journal().then(setJournal).catch(logQuietly("ProblemsBoard"));
  }, [refresh]);

  const cols = useMemo(() => problemColumns(list ?? [], journal, Date.now() / 1000), [list, journal]);
  if (!list || list.length === 0) return null;
  const sel = list.find((p) => p.key === picked) ?? null;

  return (
    <section className="mb-4 rounded-2xl border border-line/70 bg-panel shadow-elev-1">
      <button onClick={() => setOpen((v) => !v)} aria-expanded={open} className="flex w-full flex-wrap items-center gap-x-4 gap-y-1 px-5 py-3 text-left">
        <History size={15} className="text-mute" />
        <span className="text-sm font-semibold text-ink">Problemas de este equipo</span>
        <span className="text-xs text-dim">
          <b className="text-bad">{cols.open.length}</b> abiertos · <b className="text-neon">{cols.working.length}</b> en curso · <b className="text-ok">{cols.solved.length}</b> resueltos
        </span>
        <ChevronDown size={14} className={`ml-auto text-mute transition-transform ${open ? "rotate-180" : ""}`} />
      </button>
      {open && (
        <div className="grid gap-4 border-t border-line/60 p-4 lg:grid-cols-[repeat(3,minmax(0,1fr))_minmax(0,1.3fr)]">
          {(
            [
              ["Abiertos", cols.open, "Aún no se ha probado nada"],
              ["En curso", cols.working, "Se probó algo y sigue"],
              ["Resueltos", cols.solved, "Dejaron de salir"],
            ] as const
          ).map(([title, items, hint]) => (
            <div key={title} className="min-w-0">
              <div className="mb-1.5 text-[11px] tracking-wide text-mute uppercase">
                {title} <span className="normal-case">· {hint}</span>
              </div>
              <ul className="space-y-1.5">
                {items.length === 0 && <li className="text-xs text-mute">Ninguno.</li>}
                {items.map((p) => (
                  <li key={p.key}>
                    <button
                      onClick={() => setPicked(p.key === picked ? null : p.key)}
                      aria-pressed={p.key === picked}
                      className={`flex w-full items-start gap-2 rounded-lg border px-2.5 py-2 text-left text-[13px] transition-colors ${p.key === picked ? "border-neon/60 bg-neon/5" : "border-line/70 hover:bg-panel-2"}`}
                    >
                      <span className={`mt-1.5 size-2 shrink-0 rounded-full ${p.resolvedAt ? "bg-ok" : DOT[p.severity]}`} />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-ink">{p.title}</span>
                        <span className="block text-[11px] text-mute">{p.resolvedAt ? `resuelto ${ago(p.resolvedAt)}` : `desde ${shortDate(p.firstSeen)}`}</span>
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ))}
          <div className="min-w-0 rounded-xl bg-panel-2/50 p-3.5">
            {sel ? (
              <>
                <div className="mb-2 text-[13.5px] font-medium text-ink">{sel.title}</div>
                <ol className="space-y-1.5 border-l border-line-2 pl-3">
                  {problemStory(sel, journal).map((e, i) => (
                    <li key={`${e.at}-${i}`} className="relative text-xs">
                      <span className={`absolute top-1 -left-[17px] size-2 rounded-full ${e.tone === "ok" ? "bg-ok" : e.tone === "bad" ? "bg-bad" : e.tone === "warn" ? "bg-warn" : "bg-neon"}`} />
                      <span className="font-mono text-mute">{shortDate(e.at)}</span> <span className="text-ink">{e.text}</span>
                    </li>
                  ))}
                </ol>
              </>
            ) : (
              <p className="text-xs text-mute">Pulsa un problema para ver su historia: cuándo apareció, qué se probó y si se resolvió o volvió.</p>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
