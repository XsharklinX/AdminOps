// Encontrar el cambio que lo rompió. De los cambios de hoy que se pueden deshacer
// y volver a aplicar, deshace la mitad y pregunta «¿ya funciona?»; según la
// respuesta sigue con esa mitad o con la otra. Al final deja deshecho solo el
// culpable, vuelve a aplicar el resto y lo apunta en el diario (y en el caso).
import { Loader2, Search, Undo2 } from "lucide-react";
import { useMemo, useState } from "react";
import { useConfirm, useToast } from "../feedback";
import { useKeepAlive } from "../../lib/alive";
import { Card, EmptyLine, Notice, smallBtn, softBtn } from "../ui";
import { diagnoseApi, tweaksApi, type JournalEntry } from "../../lib/api";
import { answer, candidates, culprit, maxQuestions, plan, startBisect, toUndo, type BisectState, type Candidate } from "../../lib/bisect";
import { timeOfDay } from "../../lib/format";

type Phase = "idle" | "moving" | "asking" | "done";

const startOfToday = () => {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime() / 1000;
};

export function BisectCard({ entries, onChanged }: { entries: JournalEntry[] | null; onChanged: () => void }) {
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [range, setRange] = useState<"today" | "week">("today");
  const [items, setItems] = useState<Candidate[]>([]);
  const [state, setState] = useState<BisectState | null>(null);
  const [undone, setUndone] = useState<number[]>([]);
  const [phase, setPhase] = useState<Phase>("idle");
  const [failed, setFailed] = useState<string | null>(null);
  // A medias hay cambios deshechos que solo esta tarjeta sabe volver a aplicar.
  useKeepAlive("machine", state !== null || phase === "moving");
  const [found, setFound] = useState<number | null>(null);

  const list = useMemo(() => candidates(entries ?? [], range === "today" ? startOfToday() : Date.now() / 1000 - 7 * 86400), [entries, range]);
  const shown = phase === "idle" ? list : items;

  /** Deja deshechos justo los cambios `wanted` (por posición) y aplicados todos los demás. */
  const moveTo = async (wanted: number[], from = undone): Promise<boolean> => {
    const p = plan(from, wanted);
    let now = [...from];
    setPhase("moving");
    try {
      for (const i of p.undo) {
        await tweaksApi.revert(items[i].tweakId!);
        now = [...now, i];
        setUndone(now);
      }
      for (const i of p.redo) {
        await tweaksApi.apply(items[i].tweakId!, true);
        now = now.filter((x) => x !== i);
        setUndone(now);
      }
      setFailed(null);
      return true;
    } catch (e) {
      setFailed(`${e}`);
      toast("error", String(e));
      return false;
    } finally {
      onChanged();
    }
  };

  const start = async () => {
    if (list.length < 2) return;
    const ok = await confirm({
      title: "¿Buscar el cambio que lo rompió?",
      confirmLabel: "Empezar",
      body: (
        <p>
          Se van a deshacer y volver a aplicar varios de los {list.length} cambios, por mitades, mientras pruebas. Al final solo queda deshecho el culpable. Cada paso se puede deshacer; si cancelas, todo vuelve como estaba.
        </p>
      ),
    });
    if (!ok) return;
    setItems(list);
    const s = startBisect(list.length);
    setState(s);
    setUndone([]);
    setFound(null);
    setFailed(null);
    // `items` aún no se ha actualizado dentro de moveTo: se pasa la lista por su cuenta.
    await runFirst(list, s);
  };

  const runFirst = async (l: Candidate[], s: BisectState) => {
    setPhase("moving");
    let now: number[] = [];
    try {
      for (const i of toUndo(s)) {
        await tweaksApi.revert(l[i].tweakId!);
        now = [...now, i];
        setUndone(now);
      }
      setPhase("asking");
    } catch (e) {
      setFailed(String(e));
      toast("error", String(e));
    } finally {
      onChanged();
    }
  };

  const reply = async (works: boolean) => {
    if (!state) return;
    const next = answer(state, works);
    setState(next);
    const c = culprit(next);
    const ok = await moveTo(toUndo(next));
    if (!ok) return;
    if (c === null) {
      setPhase("asking");
      return;
    }
    setFound(c);
    setPhase("done");
    const title = items[c].title;
    try {
      await diagnoseApi.bisectNote(title, items.length, next.asked);
    } catch (e) {
      toast("error", `No se pudo apuntar en el diario: ${e}`);
    }
    onChanged();
  };

  const cancel = async () => {
    if (await moveTo([])) {
      toast("ok", "Todo vuelve a estar aplicado como antes.");
      setPhase("idle");
      setState(null);
      setItems([]);
    }
  };

  const done = () => {
    setPhase("idle");
    setState(null);
    setItems([]);
    setFound(null);
  };

  const suspects = new Set(state?.suspects ?? []);
  const wanted = new Set(state ? toUndo(state) : []);

  return (
    <Card
      id="bisect"
      title="Encontrar el cambio que lo rompió"
      icon={<Search size={14} />}
      className="col-span-12"
      right={
        phase === "idle" && (
          <div className="flex gap-1.5 text-[11px]">
            {(
              [
                ["today", "Hoy"],
                ["week", "7 días"],
              ] as const
            ).map(([id, label]) => (
              <button key={id} onClick={() => setRange(id)} className={`rounded-full border px-2.5 py-0.5 ${range === id ? "border-neon/60 bg-neon/10 text-ink" : "border-line text-mute hover:text-ink"}`}>
                {label}
              </button>
            ))}
          </div>
        )
      }
    >
      <p className="mb-3 text-xs text-dim">
        ¿Hiciste varios cambios y ahora algo no va? AdminOps deshace la mitad y te pregunta «¿ya funciona?»; según lo que contestes sigue con esa mitad o con la otra. Con 8 cambios nunca hacen falta más de 3 preguntas. Solo entran los cambios que se pueden deshacer y volver a aplicar.
      </p>

      {shown.length === 0 ? (
        <EmptyLine>{range === "today" ? "Hoy no hay cambios que se puedan deshacer y volver a aplicar. Prueba con «7 días»." : "En los últimos 7 días no hay cambios que se puedan deshacer y volver a aplicar."}</EmptyLine>
      ) : (
        <>
          <ol className="space-y-1">
            {shown.map((c, i) => {
              const isUndone = undone.includes(i);
              const culpritNow = found === i;
              const out = phase !== "idle" && state && !suspects.has(i);
              return (
                <li key={c.id} className={`flex items-center gap-3 rounded-lg border px-3 py-1.5 text-sm ${culpritNow ? "border-bad/60 bg-bad/10" : isUndone ? "border-warn/40 bg-warn/5" : "border-line"}`}>
                  <span className="w-5 font-mono text-[11px] text-mute">{i + 1}</span>
                  <span className={`min-w-0 flex-1 truncate ${out && !culpritNow ? "text-dim" : "text-ink"}`}>{c.title}</span>
                  <span className="font-mono text-[11px] text-mute">{timeOfDay(c.timestamp)}</span>
                  {phase !== "idle" && (
                    <span className="w-28 text-right text-[11px] text-mute">
                      {culpritNow ? <span className="text-bad">el culpable</span> : isUndone ? (wanted.has(i) ? "deshecho ahora" : "deshaciendo…") : out ? "descartado" : "aplicado"}
                    </span>
                  )}
                </li>
              );
            })}
          </ol>

          {phase === "idle" && (
            <div className="mt-3 flex flex-wrap items-center gap-3">
              <button onClick={() => void start()} disabled={list.length < 2} className={softBtn}>
                <Search size={12} /> Buscar el culpable
              </button>
              <span className="text-[11px] text-mute">
                {list.length < 2 ? "Con un solo cambio no hace falta buscar: pruébalo con «Deshacer» en el diario." : `${list.length} cambios · como mucho ${maxQuestions(list.length)} preguntas`}
              </span>
            </div>
          )}

          {phase === "moving" && (
            <p className="mt-3 flex items-center gap-2 text-sm text-mute">
              <Loader2 size={14} className="animate-spin" /> Deshaciendo y aplicando cambios…
            </p>
          )}

          {phase === "asking" && state && (
            <div className="mt-3 rounded-lg border border-neon/40 bg-neon/5 px-4 py-3">
              <p className="text-xs text-mute">
                Pregunta {state.asked + 1} de {maxQuestions(items.length)} como máximo
              </p>
              <p className="mt-1 text-sm text-ink">Con los cambios marcados como «deshecho ahora» quitados, ¿ya funciona lo que fallaba?</p>
              <div className="mt-2 flex flex-wrap gap-2">
                <button onClick={() => void reply(true)} className={softBtn}>
                  Sí, ya funciona
                </button>
                <button onClick={() => void reply(false)} className={smallBtn}>
                  No, sigue igual
                </button>
                <button onClick={() => void cancel()} className={`${smallBtn} ml-auto`}>
                  <Undo2 size={12} /> Cancelar y dejarlo como estaba
                </button>
              </div>
            </div>
          )}

          {phase === "done" && found !== null && (
            <div className="mt-3 rounded-lg border border-line-2 bg-panel-2 px-4 py-3">
              <p className="text-sm text-ink">
                Era «{items[found].title}». Queda deshecho; los otros {items.length - 1} siguen aplicados. Queda apuntado en el diario y en el caso.
              </p>
              <p className="mt-1 text-[11px] text-mute">Comprueba una vez más que sigue funcionando con los demás aplicados. Si vuelve a fallar, puede que haya más de un culpable.</p>
              <button onClick={done} className={`${smallBtn} mt-2`}>
                Hecho
              </button>
            </div>
          )}

          {failed && (
            <div className="mt-3">
              <Notice tone="error" where="Encontrar el cambio que lo rompió">Se interrumpió: {failed}</Notice>
              <button onClick={() => void cancel()} className={`${smallBtn} mt-2`}>
                <Undo2 size={12} /> Dejarlo todo como estaba
              </button>
            </div>
          )}
        </>
      )}
      {dialog}
    </Card>
  );
}
