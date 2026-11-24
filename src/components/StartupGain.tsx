import { Loader2, TrendingDown } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { logQuietly, maintenanceApi, measureApi, systemApi, type BootAnalysis, type StartupItem } from "../lib/api";
import { average, forHost, gains, measured, readBaseline, secsText, writeBaseline, type Baseline } from "../lib/startupGain";
import { ViewCommand } from "./ActionCommand";
import { useToast } from "./feedback";
import { Card } from "./ui";
import { hostName } from "../lib/machineHost";

/**
 * Inicio de Windows → «Lo que se gana»: qué programas retrasan el arranque y
 * cuánto (según Windows), lo que se ganaría quitándolos y, tras reiniciar, lo
 * que se ganó de verdad.
 */
export function StartupGain({ items, isAdmin, onChanged }: { items: StartupItem[]; isAdmin: boolean; onChanged: () => void }) {
  const [boot, setBoot] = useState<BootAnalysis | null>(null);
  const [picked, setPicked] = useState<Set<string> | null>(null);
  const [busy, setBusy] = useState(false);
  const [baseline, setBaseline] = useState<Baseline | null>(readBaseline);
  const toast = useToast();

  useEffect(() => {
    maintenanceApi.boot().then(setBoot).catch(logQuietly("StartupGain"));
    // Un apunte hecho en otro equipo (viajó en el USB) no se enseña aquí.
    void hostName().then((h) => setBaseline((b) => forHost(b, h)));
  }, []);

  const list = useMemo(() => (boot ? gains(items, boot.culprits) : []), [boot, items]);
  const chosen = picked ?? new Set(list.slice(0, 3).map((g) => g.item.id));
  const avg = boot ? average(boot.boots) : 0;
  const saving = list.filter((g) => chosen.has(g.item.id)).reduce((a, g) => a + g.delayMs, 0);
  const result = boot ? measured(boot.boots, baseline) : null;

  if (!boot || (!list.length && !result)) return null;

  const apply = async () => {
    setBusy(true);
    const done: string[] = [];
    // Cuántos programas arrancaban antes de quitarlos, para decir cuántos quedan.
    const before = await measureApi.read("inicio").catch(() => null);
    for (const g of list.filter((x) => chosen.has(x.item.id))) {
      if (g.item.needsAdmin && !isAdmin) continue;
      try {
        await systemApi.setStartupEnabled(g.item.id, false);
        done.push(g.item.description ?? g.item.name);
      } catch (e) {
        toast("error", `${g.item.name}: ${e}`);
      }
    }
    setBusy(false);
    if (!done.length) return;
    const b: Baseline = { at: Date.now() / 1000, avgMs: avg, estimateMs: Math.max(0, avg - saving), names: done, host: await hostName() };
    writeBaseline(b);
    setBaseline(b);
    const after = before === null ? null : await measureApi.read("inicio").catch(() => null);
    const count = after === null || before === null ? null : await measureApi.text({ label: "Programas al iniciar", before, after, unit: "" }).catch(() => null);
    toast("ok", `${done.length} programas ya no arrancan con Windows.${count ? ` ${count}.` : ""} Tras el próximo reinicio, aquí verás lo que se ganó.`);
    onChanged();
  };

  return (
    <Card title="Lo que se gana" icon={<TrendingDown size={14} />} className="mb-4">
      {result && (
        <div className={`mb-4 rounded-xl border border-line/70 p-4 ${result.afterMs < result.beforeMs ? "tone-ok" : "tone-warn"}`}>
          <div className="text-[15px] font-semibold text-ink">
            Medido tras reiniciar: {secsText(result.beforeMs)} → {secsText(result.afterMs)}
            {result.afterMs < result.beforeMs && <span className="ml-2 text-ok">(−{secsText(result.beforeMs - result.afterMs)})</span>}
          </div>
          <p className="mt-0.5 text-xs text-dim">
            Se estimó {secsText(result.estimateMs)}. Quitados: {baseline?.names.join(", ")}.
          </p>
          <button
            onClick={() =>
              void navigator.clipboard
                .writeText(`El equipo arranca en ${secsText(result.afterMs)} (antes ${secsText(result.beforeMs)}) tras quitar del inicio: ${baseline?.names.join(", ")}.`)
                .then(() => toast("ok", "Copiado para el ticket o el informe."), () => toast("error", "No se pudo copiar."))
            }
            className="mt-2 text-xs text-neon hover:underline"
          >
            Copiar para el ticket o el informe
          </button>
        </div>
      )}
      {list.length > 0 && (
        <>
          <p className="mb-2 text-[13px] text-dim">
            Arranque de media: <b className="font-mono text-ink">{secsText(avg)}</b>. Windows apunta cuánto retrasa cada programa:
          </p>
          <ul className="mb-3 divide-y divide-line/60">
            {list.map((g) => (
              <li key={g.item.id} className="flex items-center gap-3 py-1.5 text-[13px]">
                <input
                  type="checkbox"
                  checked={chosen.has(g.item.id)}
                  onChange={() => {
                    const n = new Set(chosen);
                    if (n.has(g.item.id)) n.delete(g.item.id);
                    else n.add(g.item.id);
                    setPicked(n);
                  }}
                  aria-label={`Quitar ${g.item.description ?? g.item.name} del inicio`}
                  className="size-4 accent-[var(--color-neon)]"
                />
                <span className="min-w-0 flex-1 truncate text-ink">{g.item.description ?? g.item.name}</span>
                <span className="text-[11px] text-mute">en {g.times} arranques</span>
                <span className="w-16 text-right font-mono text-xs text-warn">+{secsText(g.delayMs)}</span>
              </li>
            ))}
          </ul>
          <div className="flex flex-wrap items-center gap-3">
            <span className="text-[13px] text-ink">
              Si quitas {chosen.size === 1 ? "este" : `estos ${chosen.size}`}: arranque estimado <b className="font-mono text-ok">{secsText(Math.max(0, avg - saving))}</b>
              <span className="text-dim"> (−{secsText(saving)})</span>
            </span>
            <button onClick={() => void apply()} disabled={busy || chosen.size === 0} className="ml-auto flex items-center gap-1.5 rounded-lg bg-neon px-3 py-1.5 text-xs font-medium text-on-neon disabled:opacity-50">
              {busy && <Loader2 size={12} className="animate-spin" />} Quitarlos y medir
            </button>
            {chosen.size === 1 && <ViewCommand id="startup.disable" context={[...chosen][0]} />}
          </div>
          <p className="mt-2 text-[11px] text-mute">Es una estimación: los retrasos se solapan entre sí. Lo de verdad se ve tras el próximo reinicio.</p>
        </>
      )}
    </Card>
  );
}
