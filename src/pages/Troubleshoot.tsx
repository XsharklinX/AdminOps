import { AlertTriangle, ArrowRight, CheckCircle2, Info, Loader2, RefreshCw, Wrench, XCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { isPageId, pageLabel, type PageId } from "../components/Sidebar";
import { Responsible } from "../components/Responsible";
import { Card } from "../components/ui";
import { TweaksPage } from "./TweaksPage";
import { troubleshootApi, type TroubleFinding, type TroubleFix } from "../lib/api";
import { SYMPTOMS } from "../lib/symptoms";
import type { Symptom } from "../lib/api";



const LEVEL = {
  ok: { icon: CheckCircle2, color: "text-ok", border: "border-ok/30" },
  info: { icon: Info, color: "text-neon", border: "border-line" },
  warn: { icon: AlertTriangle, color: "text-warn", border: "border-warn/40" },
  bad: { icon: XCircle, color: "text-bad", border: "border-bad/40" },
};

const isPage = isPageId;

/** «Algo no funciona»: eliges el síntoma y AdminOps comprueba y ofrece las reparaciones. */
export function Troubleshoot({ isAdmin, focus, onNavigate }: { isAdmin: boolean; focus: string | null; onNavigate: (p: PageId) => void }) {
  const [symptom, setSymptom] = useState<Symptom | null>(null);
  const [findings, setFindings] = useState<TroubleFinding[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [fixing, setFixing] = useState<string | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const seq = useRef(0);

  const check = useCallback(async (s: Symptom) => {
    const n = ++seq.current;
    setSymptom(s);
    setChecking(true);
    setError(null);
    setFindings(null);
    try {
      const r = await troubleshootApi.check(s);
      if (n === seq.current) setFindings(r.findings);
    } catch (e) {
      if (n === seq.current) setError(String(e));
    } finally {
      if (n === seq.current) setChecking(false);
    }
  }, []);

  // Enlaces a las reparaciones: «repairs» (la sección) o un ajuste concreto («repair.sfc»).
  const repairFocus = focus && focus.startsWith("repair.") ? focus : null;
  useEffect(() => {
    if (focus === "repairs" || focus?.startsWith("repair.")) {
      window.setTimeout(() => document.getElementById("repairs")?.scrollIntoView({ behavior: "smooth", block: "start" }), 80);
    }
  }, [focus]);

  // Desde la búsqueda o un aviso: abre el síntoma y lo comprueba.
  useEffect(() => {
    const s = SYMPTOMS.find((x) => x.id === focus);
    if (s) check(s.id);
  }, [focus, check]);

  const runFix = async (f: TroubleFix) => {
    if (f.confirm && !(await confirm({ title: f.label, body: f.confirm, confirmLabel: "Continuar" }))) return;
    setFixing(f.id);
    try {
      const msg = await troubleshootApi.fix(f.id);
      if (msg) toast("ok", msg);
    } catch (e) {
      toast("error", `${f.label}: ${e}`);
    } finally {
      setFixing(null);
    }
    // Abrir Configuración no cambia nada que volver a comprobar.
    if (symptom && !f.id.startsWith("open:")) check(symptom);
  };

  const current = SYMPTOMS.find((s) => s.id === symptom);

  return (
    <div className="mx-auto grid max-w-5xl grid-cols-12 gap-4 p-6">
      <Card title="¿Qué le pasa al equipo?" icon={<Wrench size={14} />} className="col-span-12">
        <p className="mb-3 text-xs text-dim">Elige el síntoma: AdminOps revisa en orden las causas típicas y te ofrece la reparación de cada una.</p>
        <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
          {SYMPTOMS.map((s) => (
            <button
              key={s.id}
              onClick={() => check(s.id)}
              disabled={checking && symptom === s.id}
              className={`flex items-start gap-2.5 rounded-lg border px-3 py-2.5 text-left transition-colors ${
                symptom === s.id ? "border-neon/60 bg-neon/10" : "border-line hover:border-line-2 hover:bg-panel-2"
              }`}
            >
              <s.icon size={16} className={`mt-0.5 shrink-0 ${symptom === s.id ? "text-neon" : "text-dim"}`} />
              <span className="min-w-0">
                <span className="block text-sm text-ink">{s.title}</span>
                <span className="block text-[11px] leading-snug text-mute">{s.hint}</span>
              </span>
            </button>
          ))}
        </div>
      </Card>

      {current && (
        <Card
          title={current.title}
          icon={<current.icon size={14} />}
          className="col-span-12"
          right={
            <button onClick={() => check(current.id)} disabled={checking || !!fixing} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink disabled:opacity-40">
              <RefreshCw size={11} className={checking ? "animate-spin" : ""} /> Volver a comprobar
            </button>
          }
        >
          {checking ? (
            <p className="flex items-center gap-2 py-4 text-sm text-mute">
              <Loader2 size={14} className="animate-spin" /> Comprobando…
            </p>
          ) : error ? (
            <p className="text-sm text-bad">{error}</p>
          ) : (
            <ul className="space-y-2">
              {findings?.map((f, i) => {
                const L = LEVEL[f.level];
                return (
                  <li key={i} className={`rounded-lg border ${L.border} bg-void/30 px-3.5 py-3`}>
                    <div className="flex items-start gap-2.5">
                      <L.icon size={16} className={`mt-0.5 shrink-0 ${L.color}`} />
                      <div className="min-w-0 flex-1">
                        <p className="text-sm font-medium text-ink">{f.title}</p>
                        {f.detail && <p className="mt-0.5 text-xs leading-relaxed break-words text-dim select-text">{f.detail}</p>}
                        {(f.fixes.length > 0 || isPage(f.page)) && (
                          <div className="mt-2.5 flex flex-wrap items-center gap-2">
                            {f.fixes.map((x) => {
                              const blocked = x.admin && !isAdmin;
                              return (
                                <button
                                  key={x.id}
                                  onClick={() => runFix(x)}
                                  disabled={!!fixing || blocked}
                                  title={blocked ? "Requiere ejecutar AdminOps como administrador" : undefined}
                                  className={`flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-xs transition-colors disabled:opacity-40 ${
                                    f.level === "bad" && !x.id.startsWith("open:") ? "border-neon/50 text-neon hover:bg-neon/10" : "border-line-2 text-dim hover:border-neon/40 hover:text-ink"
                                  }`}
                                >
                                  {fixing === x.id && <Loader2 size={11} className="animate-spin" />}
                                  {x.label}
                                </button>
                              );
                            })}
                            {isPage(f.page) && (
                              <button onClick={() => onNavigate(f.page as PageId)} className="flex items-center gap-1 text-xs text-mute hover:text-neon">
                                Ver en {pageLabel(f.page as PageId)} <ArrowRight size={11} />
                              </button>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
          {findings?.some((f) => f.level === "bad" || f.level === "warn") && <Responsible topic={current.id} className="mt-3 border-t border-line pt-3" />}
        </Card>
      )}
      {dialog}
      {/* Reparaciones de Windows (antes, su propia página) */}
      <section id="repairs" className="col-span-12 scroll-mt-4">
        <h2 className="mb-1 flex items-center gap-2 text-[15px] font-semibold">
          <Wrench size={15} className="text-neon" /> Reparaciones de Windows
        </h2>
        <p className="text-xs text-dim">Para cuando ya sabes qué falla: archivos del sistema (SFC, DISM), red, Windows Update, cola de impresión, Explorador, hora…</p>
        <div className="-mx-6">
          <TweaksPage category="repair" isAdmin={isAdmin} focus={repairFocus} />
        </div>
      </section>
    </div>
  );
}
