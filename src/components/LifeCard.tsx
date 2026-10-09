// ¿Reparar o cambiar este equipo? Junta la edad, el procesador, el disco, la memoria, Windows 11 y la
// batería, dice qué pieza lo limita y compara alargarle la vida con comprar uno nuevo.
import { ClipboardCopy, Loader2, Scale } from "lucide-react";
import { useState } from "react";
import { useToast } from "./feedback";
import { Button, Card, smallBtn } from "./ui";
import { DEFAULT_COSTS, lifeApi, type LifeCosts, type LifeReport } from "../lib/api";

const KEY = "adminops.lifeCosts";
const TONE = {
  keep: "border-ok/40 bg-ok/10 text-ok",
  upgrade: "border-warn/40 bg-warn/10 text-warn",
  replace: "border-bad/40 bg-bad/10 text-bad",
} as const;
const COST_LABELS: [keyof LifeCosts, string][] = [
  ["ssd", "SSD"],
  ["ram", "Memoria"],
  ["battery", "Batería"],
  ["labor", "Mano de obra por mejora"],
  ["newPc", "Equipo nuevo"],
];

function savedCosts(): LifeCosts {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? { ...DEFAULT_COSTS, ...(JSON.parse(raw) as Partial<LifeCosts>) } : DEFAULT_COSTS;
  } catch {
    return DEFAULT_COSTS;
  }
}

const eur = (n: number) => `${Math.round(n)} €`;

export function LifeCard() {
  const toast = useToast();
  const [costs, setCosts] = useState<LifeCosts>(savedCosts);
  const [r, setR] = useState<LifeReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [showCosts, setShowCosts] = useState(false);

  const run = async () => {
    setBusy(true);
    try {
      setR(await lifeApi.report(costs));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  const setCost = (k: keyof LifeCosts, v: number) => {
    const next = { ...costs, [k]: Number.isFinite(v) ? Math.max(0, v) : 0 };
    setCosts(next);
    try {
      localStorage.setItem(KEY, JSON.stringify(next));
    } catch {
      /* sin almacenamiento: los precios valen para esta sesión */
    }
  };

  return (
    <Card title="¿Reparar o cambiar este equipo?" icon={<Scale size={14} />}>
      <div className="space-y-3 p-4">
        <p className="text-xs leading-relaxed text-dim">
          Junta la edad del equipo y del procesador, el disco, la memoria, la compatibilidad con Windows 11 y la batería, dice qué pieza lo frena y compara alargarle la vida con comprar uno nuevo. Usa el último diagnóstico y el estado de los
          discos.
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <Button onClick={() => void run()} disabled={busy}>
            {busy ? <Loader2 size={14} className="animate-spin" /> : <Scale size={14} />} {r ? "Valorar otra vez" : "Valorar el equipo"}
          </Button>
          <button type="button" className="text-xs text-neon hover:underline" onClick={() => setShowCosts(!showCosts)}>
            {showCosts ? "Ocultar precios" : "Ajustar precios"}
          </button>
        </div>
        {showCosts && (
          <div className="grid gap-2 sm:grid-cols-3">
            {COST_LABELS.map(([k, label]) => (
              <label key={k} className="flex items-center justify-between gap-2 text-xs text-dim">
                {label}
                <span className="flex items-center gap-1">
                  <input type="number" min={0} value={costs[k]} onChange={(e) => setCost(k, Number(e.target.value))} className="h-8 w-20 rounded-md border border-line-2 bg-void px-2 text-right text-xs text-ink" />€
                </span>
              </label>
            ))}
          </div>
        )}
        {r && (
          <>
            <div className={`rounded-lg border p-3 ${TONE[r.decision]}`}>
              <div className="text-sm font-medium">{r.headline}</div>
              <p className="mt-1 text-xs text-dim">{r.detail}</p>
            </div>
            <ul className="grid gap-x-6 gap-y-2 sm:grid-cols-2">
              {r.parts.map((p) => (
                <li key={p.name} className="min-w-0">
                  <div className="flex items-baseline justify-between gap-2 text-xs">
                    <span className={p.limiter ? "font-medium text-ink" : "text-dim"}>
                      {p.name}
                      {p.limiter ? " · lo que más frena" : ""}
                    </span>
                    <span className="tabular font-mono text-mute">{p.score}</span>
                  </div>
                  <div className="mt-0.5 h-1 overflow-hidden rounded-full bg-line">
                    <div className="h-full rounded-full" style={{ width: `${p.score}%`, background: p.score >= 70 ? "var(--color-ok)" : p.score >= 40 ? "var(--color-warn)" : "var(--color-bad)" }} />
                  </div>
                  <div className="mt-0.5 truncate text-[10px] text-mute" title={p.text}>
                    {p.text}
                  </div>
                </li>
              ))}
            </ul>
            {r.upgrades.length > 0 && (
              <div className="rounded-lg border border-line bg-panel-2 p-3 text-xs">
                <div className="mb-1 font-medium text-ink">Alargarle la vida</div>
                {r.upgrades.map((u) => (
                  <div key={u.label} className="flex justify-between gap-3 py-0.5 text-dim">
                    <span>{u.label}</span>
                    <span className="tabular font-mono text-ink">{eur(u.cost)}</span>
                  </div>
                ))}
                <div className="mt-1 flex justify-between gap-3 border-t border-line pt-1">
                  <span className="text-ink">Total de las mejoras</span>
                  <span className="tabular font-mono text-ink">{eur(r.upgradeTotal)}</span>
                </div>
                <div className="flex justify-between gap-3 text-mute">
                  <span>Un equipo nuevo, aproximadamente</span>
                  <span className="tabular font-mono">{eur(r.newCost)}</span>
                </div>
              </div>
            )}
            <button
              type="button"
              className={smallBtn}
              onClick={() => void navigator.clipboard.writeText(r.clientText).then(() => toast("ok", "Texto para el cliente copiado."), () => toast("error", "No se pudo copiar."))}
            >
              <ClipboardCopy size={12} /> Copiar el texto para el cliente
            </button>
          </>
        )}
      </div>
    </Card>
  );
}
