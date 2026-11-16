import { Check, ChevronRight, ClipboardCopy, Loader2, Moon, Stethoscope, Undo2, Wrench } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { disksApi, logQuietly, tweaksApi, type DiskReport, type JournalEntry } from "../lib/api";
import { assess, DISK_TWEAKS, effects, KIND_LABEL, type DiskErrors, type ErrorKind, type Step } from "../lib/diskAssist";
import { goToPage } from "../lib/navigate";
import { notifyJournalChange } from "../lib/journalEvents";
import { useToast } from "./feedback";
import { Modal } from "./ui";
import { scheduleNight } from "../lib/nightJobs";

const TONE = { ok: "text-ok", warn: "text-warn", bad: "text-bad" } as const;
const DONE_KEY = (d: DiskReport) => `adminops.diskAssist.${d.number}.${d.model}`;

function readDone(d: DiskReport): string[] {
  try {
    return JSON.parse(localStorage.getItem(DONE_KEY(d)) ?? "[]") as string[];
  } catch {
    return [];
  }
}

/**
 * Asistente para un disco con errores: qué falla de verdad (con las pruebas),
 * el plan en orden con su botón en cada paso, y el veredicto para el cliente.
 */
export function DiskAssistant({ d, errs, controllerResets, onClose }: { d: DiskReport; errs: DiskErrors | undefined; controllerResets: number; onClose: () => void }) {
  const a = useMemo(() => assess(d, errs, controllerResets), [d, errs, controllerResets]);
  const [tab, setTab] = useState<"what" | "plan" | "verdict">("what");
  const [done, setDone] = useState<string[]>(() => readDone(d));
  const [busy, setBusy] = useState<string | null>(null);
  const [journal, setJournal] = useState<JournalEntry[]>([]);
  const toast = useToast();

  useEffect(() => {
    tweaksApi.journal().then(setJournal).catch(logQuietly("DiskAssistant"));
  }, []);
  const eff = effects(errs, journal, Date.now() / 1000, DISK_TWEAKS);

  const mark = (id: string) =>
    setDone((cur) => {
      const next = cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id];
      try {
        localStorage.setItem(DONE_KEY(d), JSON.stringify(next));
      } catch {
        /* sin almacenamiento: dura esta sesión */
      }
      return next;
    });

  const run = async (s: Step) => {
    const act = s.action;
    if (act.kind === "goto") {
      onClose();
      if (act.target === "drivers") goToPage("diagnostics");
      else window.setTimeout(() => document.getElementById(act.target === "rescue" ? "disk-rescue" : "disk-clone")?.scrollIntoView({ behavior: "smooth", block: "start" }), 50);
      return;
    }
    if (act.kind === "manual") return mark(s.id);
    setBusy(s.id);
    try {
      if (act.kind === "tweak") {
        const r = await tweaksApi.apply(act.tweak, true);
        notifyJournalChange();
        toast("ok", `${s.title}: ${r.message || "hecho"}. Aquí se verá si los errores paran.`);
      } else if (act.kind === "check") {
        const r = await disksApi.check(act.letter);
        toast(r.level === "ok" ? "ok" : "info", r.text);
      } else if (act.kind === "repair") {
        const r = await disksApi.repair(act.letter);
        toast(r.level === "ok" ? "ok" : "info", r.text);
      } else if (act.kind === "scan") {
        await scheduleNight({ kind: "surface", disk: d.number, model: d.model, hdd: d.media === "HDD", mode: act.mode });
        toast("ok", "Prueba de superficie apuntada para esta noche. Por la mañana verás el resultado.");
      }
      if (!done.includes(s.id)) mark(s.id);
    } catch (e) {
      toast("error", `${s.title}: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const ticket = `${d.model} (disco ${d.number}): ${a.headline}. ${a.verdict.title}: ${a.verdict.text}`;

  return (
    <Modal title={`Asistente · ${d.model || `Disco ${d.number}`}`} onClose={onClose} width="w-[760px]">
      <div className="mb-4 flex gap-1 rounded-lg bg-panel-2 p-1" role="tablist">
        {(
          [
            ["what", "1 · Qué falla"],
            ["plan", `2 · Plan${a.plan.length ? ` (${done.filter((x) => a.plan.some((s) => s.id === x)).length}/${a.plan.length})` : ""}`],
            ["verdict", "3 · Veredicto"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            role="tab"
            aria-selected={tab === id}
            onClick={() => setTab(id)}
            className={`flex-1 rounded-md px-3 py-1.5 text-[13px] transition-colors ${tab === id ? "bg-panel font-medium text-ink shadow-elev-1" : "text-mute hover:text-ink"}`}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === "what" && (
        <div className="space-y-4">
          <div className={`rounded-xl border border-line/70 p-4 ${a.cause === "none" ? "tone-ok" : a.cause === "surface" ? "tone-bad" : "tone-warn"}`}>
            <div className="text-[17px] font-semibold text-ink">{a.headline}</div>
            <p className="mt-1 text-[13px] text-dim">{a.why}</p>
          </div>
          {a.evidence.length > 0 && (
            <div>
              <div className="mb-1.5 text-[11px] tracking-wide text-mute uppercase">Las pruebas</div>
              <ul className="divide-y divide-line/60 rounded-xl border border-line/70">
                {a.evidence.map((e) => (
                  <li key={e.label} className="flex items-center gap-3 px-3.5 py-2 text-[13px]">
                    <span className={`size-2 shrink-0 rounded-full ${e.tone === "ok" ? "bg-ok" : e.tone === "warn" ? "bg-warn" : "bg-bad"}`} />
                    <span className="min-w-0 flex-1 text-dim">{e.label}</span>
                    <span className={`font-mono tabular ${TONE[e.tone]}`}>{e.value}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {a.plan.length > 0 && (
            <button onClick={() => setTab("plan")} className="flex items-center gap-1 text-sm text-neon hover:underline">
              Ver el plan <ChevronRight size={14} />
            </button>
          )}
        </div>
      )}

      {tab === "plan" &&
        (a.plan.length === 0 ? (
          <p className="text-sm text-dim">No hace falta hacer nada con este disco.</p>
        ) : (
          <div className="space-y-4">
            <ol className="space-y-2">
              {a.plan.map((s, i) => {
                const isDone = done.includes(s.id);
                const next = !isDone && a.plan.slice(0, i).every((p) => done.includes(p.id));
                return (
                  <li key={s.id} className={`flex items-start gap-3 rounded-xl border p-3.5 ${next ? "border-neon/50 bg-neon/5" : "border-line/70"}`}>
                    <button
                      onClick={() => mark(s.id)}
                      title={isDone ? "Marcar como pendiente" : "Marcar como hecho"}
                      aria-label={isDone ? `«${s.title}» hecho; marcar como pendiente` : `Marcar «${s.title}» como hecho`}
                      className={`mt-0.5 grid size-5 shrink-0 place-items-center rounded-full border ${isDone ? "border-ok bg-ok text-void" : "border-line-2 text-mute"}`}
                    >
                      {isDone ? <Check size={12} /> : <span className="text-[10px]">{i + 1}</span>}
                    </button>
                    <div className="min-w-0 flex-1">
                      <div className={`text-[14px] font-medium ${isDone ? "text-dim line-through" : "text-ink"}`}>{s.title}</div>
                      <p className="mt-0.5 text-xs text-dim">{s.detail}</p>
                    </div>
                    {s.action.kind !== "manual" && !isDone && (
                      <button
                        onClick={() => void run(s)}
                        disabled={busy !== null}
                        className={`flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50 ${next ? "bg-neon text-on-neon" : "border border-line-2 text-dim hover:text-ink"}`}
                      >
                        {busy === s.id ? <Loader2 size={12} className="animate-spin" /> : s.action.kind === "scan" ? <Moon size={12} /> : s.action.kind === "goto" ? <ChevronRight size={12} /> : <Wrench size={12} />}
                        {s.action.kind === "scan" ? "Esta noche" : s.action.kind === "goto" ? "Ir" : s.action.kind === "tweak" ? "Aplicar" : s.action.kind === "check" ? "Comprobar" : "Reparar"}
                      </button>
                    )}
                  </li>
                );
              })}
            </ol>
            {eff.length > 0 && (
              <div className="rounded-xl border border-line/70 p-3.5">
                <div className="mb-1.5 text-[11px] tracking-wide text-mute uppercase">¿Funcionó?</div>
                {eff.map((e) => (
                  <div key={`${e.tweak}-${e.at}`} className="flex items-center gap-3 text-[13px]">
                    <span className={`size-2 shrink-0 rounded-full ${e.verdict === "works" ? "bg-ok" : e.verdict === "too-soon" ? "bg-mute" : "bg-warn"}`} />
                    <span className="min-w-0 flex-1 text-ink">{e.title}</span>
                    <span className="font-mono text-xs text-dim tabular">
                      {e.verdict === "too-soon"
                        ? "aplicado hace menos de un día: deja el equipo funcionando"
                        : `${e.beforePerDay.toFixed(1)} → ${e.afterPerDay.toFixed(1)} errores al día`}
                    </span>
                    {e.verdict !== "too-soon" && <span className={e.verdict === "works" ? "text-ok" : "text-warn"}>{e.verdict === "works" ? "Funcionó" : "No cambió"}</span>}
                  </div>
                ))}
                <p className="mt-2 flex items-center gap-1.5 text-[11px] text-mute">
                  <Undo2 size={11} /> Cada arreglo se deshace desde el Historial del equipo.
                </p>
              </div>
            )}
          </div>
        ))}

      {tab === "verdict" && (
        <div className="space-y-4">
          <div className={`rounded-xl border border-line/70 p-5 ${a.verdict.level === "ok" ? "tone-ok" : a.verdict.level === "warn" ? "tone-warn" : "tone-bad"}`}>
            <div className={`text-[22px] font-semibold ${TONE[a.verdict.level]}`}>{a.verdict.title}</div>
            <p className="mt-1 text-[13.5px] text-ink">{a.verdict.text}</p>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => void navigator.clipboard.writeText(ticket).then(() => toast("ok", "Copiado para el ticket."), () => toast("error", "No se pudo copiar."))}
              className="flex items-center gap-1.5 rounded-lg border border-line-2 px-3 py-1.5 text-xs text-dim hover:text-ink"
            >
              <ClipboardCopy size={12} /> Copiar para el ticket
            </button>
            <button
              onClick={() => void navigator.clipboard.writeText(d.texts.client).then(() => toast("ok", "Copiado para el cliente."), () => toast("error", "No se pudo copiar."))}
              className="flex items-center gap-1.5 rounded-lg border border-line-2 px-3 py-1.5 text-xs text-dim hover:text-ink"
            >
              <ClipboardCopy size={12} /> Copiar para el cliente
            </button>
          </div>
          <p className="text-[11px] text-mute">El estado de los discos ya va en el informe del cliente (sección «Estado de los discos»).</p>
        </div>
      )}
    </Modal>
  );
}

/** Los errores de un disco en los últimos 30 días, por día y por tipo, con un botón al asistente. */
export function DiskErrorStrip({ errs, onAssist }: { errs: DiskErrors | undefined; onAssist: () => void }) {
  const today = new Date();
  const days = Array.from({ length: 30 }, (_, i) => {
    const t = new Date(today.getTime() - (29 - i) * 86_400_000);
    return t.toISOString().slice(0, 10);
  });
  const map = new Map(errs?.days ?? []);
  const max = Math.max(1, ...days.map((k) => map.get(k) ?? 0));
  const kinds = Object.entries(errs?.byKind ?? {}).filter(([, v]) => (v ?? 0) > 0) as [ErrorKind, number][];
  return (
    <div className="mx-4 mb-3 rounded-xl border border-line/70 bg-panel-2/40 p-3">
      <div className="mb-2 flex flex-wrap items-center gap-2">
        <span className="text-[11px] tracking-wide text-mute uppercase">Errores de Windows · 30 días</span>
        <span className={`font-mono text-xs ${errs?.total ? "text-warn" : "text-ok"}`}>{errs?.total ?? 0}</span>
        <button onClick={onAssist} className="ml-auto flex items-center gap-1.5 rounded-lg bg-neon px-2.5 py-1 text-xs font-medium text-on-neon">
          <Stethoscope size={12} /> Asistente
        </button>
      </div>
      <div className="flex h-10 items-end gap-[3px]" role="img" aria-label={`Errores por día en los últimos 30 días: ${errs?.total ?? 0} en total`}>
        {days.map((k) => {
          const v = map.get(k) ?? 0;
          return <span key={k} title={`${k}: ${v}`} className={`flex-1 rounded-sm ${v ? "bg-bad/80" : "bg-line"}`} style={{ height: `${v ? Math.max(12, (v / max) * 100) : 6}%` }} />;
        })}
      </div>
      {kinds.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-dim">
          {kinds.map(([k, v]) => (
            <span key={k}>
              {KIND_LABEL[k]}: <b className="font-mono text-ink">{v}</b>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
