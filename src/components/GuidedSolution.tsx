// Una solución hecha paso a paso: cada paso con sus botones de AdminOps, una
// comprobación automática cuando se puede («¿hay Internet ya?») y la pregunta
// de si ya funciona. Si un paso lo arregla, se para ahí y se apunta (las
// soluciones aprenden de lo que funcionó).
import { ArrowRight, Check, ExternalLink, Loader2, RefreshCw, X, Zap } from "lucide-react";
import { useMemo, useState } from "react";
import { celebrate } from "./Celebrate";
import { Button, Modal } from "./ui";
import { knowledgeApi, troubleshootApi, type Solution, type Symptom, type TroubleFinding } from "../lib/api";
import type { SolutionAction } from "../lib/solutionsCatalog";
import { checkFor, parseSteps } from "../lib/steps";
import { openCase } from "../lib/currentCase";

export function GuidedSolution({ solution, actions, onRun, running, onClose }: { solution: Solution; actions: SolutionAction[]; onRun: (a: SolutionAction) => void; running: string | null; onClose: () => void }) {
  const steps = useMemo(() => parseSteps(solution.solution).filter((s) => s.n > 0), [solution.solution]);
  const check: Symptom | null = useMemo(() => checkFor(solution), [solution]);
  const [i, setI] = useState(0);
  const [result, setResult] = useState<TroubleFinding[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [done, setDone] = useState<"fixed" | "notfixed" | null>(null);
  const step = steps[i];
  const mine = step ? actions.filter((a) => a.step === step.n) : [];

  const runCheck = async () => {
    if (!check) return;
    setChecking(true);
    try {
      setResult((await troubleshootApi.check(check)).findings);
    } catch {
      setResult(null);
    } finally {
      setChecking(false);
    }
  };
  const fixed = () => {
    setDone("fixed");
    void knowledgeApi.record(`solution:${solution.id}`, `Paso ${step.n}: ${step.text.split("\n")[0].slice(0, 120)}`).catch(() => undefined);
    if (check) void knowledgeApi.record(check, solution.title).catch(() => undefined);
    celebrate(`Arreglado en el paso ${step.n}`);
  };
  const next = () => {
    setResult(null);
    if (i + 1 < steps.length) setI(i + 1);
    else setDone("notfixed");
  };
  const bad = result?.filter((f) => f.level === "bad" || f.level === "warn") ?? [];

  return (
    <Modal title={solution.title} onClose={onClose} width="w-[640px]">
      {steps.length === 0 ? (
        <p className="text-sm text-dim">Esta solución no tiene pasos numerados («1.», «2.»…). Edítala para poder seguirla paso a paso.</p>
      ) : done === "fixed" ? (
        <div className="space-y-3 text-sm">
          <p className="flex items-center gap-2 text-ok">
            <Check size={16} /> Arreglado en el paso {step.n}. Queda apuntado: la próxima vez esta solución sabrá qué suele funcionar.
          </p>
          <Button onClick={onClose}>Cerrar</Button>
        </div>
      ) : done === "notfixed" ? (
        <div className="space-y-3 text-sm text-dim">
          <p>Ningún paso lo ha arreglado. Siguientes opciones:</p>
          <div className="flex flex-wrap gap-2">
            <Button kind="secondary" onClick={() => (onClose(), openCase({ notes: `Probado sin éxito: ${solution.title}` }))}>
              Abrir un caso
            </Button>
            <Button kind="secondary" onClick={() => void knowledgeApi.escalate(`SÍNTOMA\n${solution.problem}\n\nPROBADO SIN ÉXITO\n${solution.title}\n${solution.solution}`).then(onClose)}>
              Paquete para escalar
            </Button>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          <ol className="flex flex-wrap gap-1" aria-label="Pasos">
            {steps.map((s, k) => (
              <li key={s.n} className={`grid size-6 place-items-center rounded-full border text-[11px] ${k < i ? "border-line text-mute line-through" : k === i ? "border-neon bg-neon/15 text-neon" : "border-line-2 text-mute"}`}>
                {s.n}
              </li>
            ))}
          </ol>
          <pre className="rounded-md border border-line bg-void/50 p-3 font-mono text-[12.5px] whitespace-pre-wrap text-ink select-text">{step.text}</pre>
          {mine.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {mine.map((a) => (
                <button key={`${a.kind}-${a.id}`} onClick={() => onRun(a)} disabled={running !== null} className="flex items-center gap-1 rounded-md border border-neon/40 px-2.5 py-1 text-xs text-neon hover:bg-neon/10 disabled:opacity-40">
                  {running === `${a.kind}-${a.id}` ? <Loader2 size={11} className="animate-spin" /> : a.kind === "fix" ? <Zap size={11} /> : a.kind === "tool" ? <ExternalLink size={11} /> : <ArrowRight size={11} />}
                  {a.kind === "fix" ? `${a.label} ahora` : a.label}
                </button>
              ))}
            </div>
          )}
          {check && (
            <div className="rounded-md border border-line px-3 py-2 text-xs">
              <button onClick={() => void runCheck()} disabled={checking} className="flex items-center gap-1.5 text-neon hover:underline disabled:opacity-50">
                {checking ? <Loader2 size={12} className="animate-spin" /> : <RefreshCw size={12} />} Comprobar si ya funciona
              </button>
              {result && (
                <p className={`mt-1 ${bad.length ? "text-warn" : "text-ok"}`}>
                  {bad.length ? `Sigue habiendo ${bad.length} problema(s): ${bad[0].title}.` : "La comprobación no encuentra problemas: parece arreglado."}
                </p>
              )}
            </div>
          )}
          <div className="flex flex-wrap items-center justify-between gap-2 border-t border-line pt-3">
            <span className="text-xs text-mute">
              Paso {i + 1} de {steps.length}. ¿Ya funciona?
            </span>
            <span className="flex gap-2">
              <Button kind="ghost" onClick={next}>
                <X size={14} /> No, {i + 1 < steps.length ? "siguiente paso" : "terminar"}
              </Button>
              <Button onClick={fixed}>
                <Check size={14} /> Sí, ya funciona
              </Button>
            </span>
          </div>
        </div>
      )}
    </Modal>
  );
}
