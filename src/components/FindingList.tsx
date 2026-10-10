// Lista de hallazgos con sus arreglos: la de «Solucionar problemas», también
// en Seguridad a fondo y donde haga falta.
import { AlertTriangle, ArrowRight, CheckCircle2, Info, Loader2, XCircle } from "lucide-react";
import { isPageId, pageLabel, type PageId } from "./Sidebar";
import { CodeLinks } from "./CodeLinks";
import type { TroubleFinding, TroubleFix } from "../lib/api";

const LEVEL = {
  ok: { icon: CheckCircle2, color: "text-ok", border: "border-ok/30" },
  info: { icon: Info, color: "text-neon", border: "border-line" },
  warn: { icon: AlertTriangle, color: "text-warn", border: "border-warn/40" },
  bad: { icon: XCircle, color: "text-bad", border: "border-bad/40" },
};

export function FindingList({
  findings,
  isAdmin,
  fixing,
  onFix,
  onNavigate,
}: {
  findings: TroubleFinding[];
  isAdmin: boolean;
  /** Id del arreglo en curso. */
  fixing: string | null;
  onFix: (f: TroubleFix) => void;
  onNavigate: (p: PageId) => void;
}) {
  return (
    <ul className="space-y-2">
      {findings.map((f, i) => {
        const L = LEVEL[f.level];
        return (
          <li key={i} className={`rounded-lg border ${L.border} bg-void/30 px-3.5 py-3`}>
            <div className="flex items-start gap-2.5">
              <L.icon size={16} className={`mt-0.5 shrink-0 ${L.color}`} />
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium text-ink">
                  <CodeLinks text={f.title} />
                </p>
                {f.detail && (
                  <p className="mt-0.5 text-xs leading-relaxed break-words whitespace-pre-line text-dim select-text">
                    <CodeLinks text={f.detail} />
                  </p>
                )}
                {(f.fixes.length > 0 || isPageId(f.page)) && (
                  <div className="mt-2.5 flex flex-wrap items-center gap-2">
                    {f.fixes.map((x) => {
                      const blocked = x.admin && !isAdmin;
                      return (
                        <button
                          key={x.id}
                          onClick={() => onFix(x)}
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
                    {isPageId(f.page) && (
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
  );
}
