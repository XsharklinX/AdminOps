// «¿Está bien mi equipo?»: lo primero que ve una persona en el modo usuario. Un sí o un no, tres
// tarjetas en lenguaje normal y un único botón de arreglo. Cada tarjeta se abre en un «por qué».
import { CheckCircle2, ChevronDown, ChevronRight, CircleAlert, Loader2, MessageSquareShare, Stethoscope, Wrench, XCircle } from "lucide-react";
import { useState } from "react";
import { Bar, Button, Card } from "./ui";
import { summarize, type Level, type UserInput } from "../lib/userHome";

const TONE: Record<Level, { ring: string; text: string; Icon: typeof CheckCircle2 }> = {
  ok: { ring: "border-ok/40 bg-ok/10", text: "text-ok", Icon: CheckCircle2 },
  warn: { ring: "border-warn/40 bg-warn/10", text: "text-warn", Icon: CircleAlert },
  bad: { ring: "border-bad/40 bg-bad/10", text: "text-bad", Icon: XCircle },
};
const BAR: Record<Level, string> = { ok: "var(--color-ok)", warn: "var(--color-warn)", bad: "var(--color-bad)" };

export function UserHome({ input, busy, onFix, onTellTechnician, onReview }: { input: UserInput; busy: "fix" | "review" | null; onFix: () => void; onTellTechnician: () => void; onReview: () => void }) {
  const s = summarize(input);
  const [open, setOpen] = useState<string | null>(null);
  const t = TONE[s.level];
  return (
    <Card className={`space-y-4 p-6 ${t.ring}`}>
      <div className="flex flex-wrap items-center gap-4">
        <t.Icon size={44} strokeWidth={1.4} className={t.text} />
        <div className="min-w-60 flex-1">
          <h2 className="text-2xl font-semibold tracking-tight">{s.headline}</h2>
          <p className="text-sm text-dim">{s.sub}</p>
        </div>
      </div>
      <div className="grid gap-3 sm:grid-cols-3">
        {s.cards.map((c) => {
          const tone = TONE[c.level];
          const isOpen = open === c.id;
          return (
            <div key={c.id} className="rounded-lg border border-line bg-panel p-3">
              <button type="button" className="flex w-full items-center justify-between gap-2 text-left" onClick={() => setOpen(isOpen ? null : c.id)} aria-expanded={isOpen}>
                <span className="text-xs text-dim">{c.title}</span>
                {isOpen ? <ChevronDown size={14} className="text-mute" /> : <ChevronRight size={14} className="text-mute" />}
              </button>
              <div className={`mt-1 text-lg font-semibold ${tone.text}`}>{c.label}</div>
              <div className="mt-2">
                <Bar value={c.value} color={BAR[c.level]} />
              </div>
              {isOpen && (
                <ul className="mt-3 space-y-1.5 text-xs leading-relaxed text-dim">
                  {c.why.map((w) => (
                    <li key={w}>{w}</li>
                  ))}
                </ul>
              )}
            </div>
          );
        })}
      </div>
      <div className="flex flex-wrap gap-2">
        {s.fixable && (
          <Button onClick={onFix} disabled={busy !== null}>
            {busy === "fix" ? <Loader2 size={14} className="animate-spin" /> : <Wrench size={14} />} Arreglar lo que se pueda
          </Button>
        )}
        <Button kind="secondary" onClick={onReview} disabled={busy !== null}>
          {busy === "review" ? <Loader2 size={14} className="animate-spin" /> : <Stethoscope size={14} />} Revisar
        </Button>
        <Button kind="secondary" onClick={onTellTechnician}>
          <MessageSquareShare size={14} /> Avisar a mi técnico
        </Button>
      </div>
      {s.fixable && <p className="text-[11px] text-mute">«Arreglar lo que se pueda» borra archivos temporales y no toca nada tuyo.</p>}
    </Card>
  );
}
