import { Battery, Cpu, Gauge, HardDrive, MemoryStick, MonitorSmartphone, Network, ShieldCheck, type LucideIcon } from "lucide-react";
import { useState } from "react";
import type { Finding } from "../lib/api";
import type { Part } from "../lib/partsMap";
import type { PageId } from "./Sidebar";
import { Card } from "./ui";

const ICON: Record<string, LucideIcon> = { cpu: Cpu, ram: MemoryStick, gpu: MonitorSmartphone, board: Gauge, net: Network, battery: Battery, windows: MonitorSmartphone, security: ShieldCheck };
const RING = {
  ok: "border-line/80",
  warn: "border-warn/70 shadow-[0_0_18px_-4px_var(--color-warn)]",
  bad: "border-bad/80 shadow-[0_0_22px_-2px_var(--color-bad)]",
  unknown: "border-line/60 opacity-70",
} as const;
const DOT = { ok: "bg-ok", warn: "bg-warn", bad: "bg-bad", unknown: "bg-mute" } as const;
const WORD = { ok: "Bien", warn: "Conviene mirarlo", bad: "Hay que actuar", unknown: "Sin datos" } as const;

/**
 * El equipo dibujado como una placa: cada pieza en su sitio con el color de lo
 * peor que tiene. Un clic en una pieza dice qué le pasa y lleva a arreglarlo.
 */
export function PartsMap({ parts, onNavigate }: { parts: Part[]; onNavigate: (p: PageId, focus?: string | null) => void }) {
  const firstBad = parts.find((p) => p.status === "bad") ?? parts.find((p) => p.status === "warn") ?? null;
  const [picked, setPicked] = useState<string | null>(firstBad?.id ?? null);
  const sel = parts.find((p) => p.id === picked) ?? null;
  const hw = parts.filter((p) => !["windows", "security", "net"].includes(p.id));
  const sw = parts.filter((p) => ["windows", "security", "net"].includes(p.id));

  const tile = (p: Part) => {
    const Icon = ICON[p.id] ?? HardDrive;
    return (
      <button
        key={p.id}
        onClick={() => setPicked(p.id === picked ? null : p.id)}
        aria-pressed={p.id === picked}
        title={`${p.label}: ${WORD[p.status]}`}
        className={`flex min-w-0 flex-col gap-1 rounded-xl border bg-panel/90 px-3 py-2.5 text-left backdrop-blur transition-[transform,box-shadow] hover:-translate-y-0.5 ${RING[p.status]} ${p.id === picked ? "ring-2 ring-neon/70" : ""}`}
      >
        <span className="flex items-center gap-1.5 text-[11.5px] font-medium text-ink">
          <Icon size={13} className="shrink-0 text-mute" />
          <span className="truncate">{p.label}</span>
          <span className={`ml-auto size-2 shrink-0 rounded-full ${DOT[p.status]}`} />
        </span>
        {p.sub && <span className="truncate text-[11px] text-mute">{p.sub}</span>}
      </button>
    );
  };

  const go = (f: Finding) => {
    const a = f.actions.find((x) => x.kind === "page");
    if (a && a.kind === "page") onNavigate(a.page as PageId, a.focus);
    else onNavigate("diagnostics");
  };

  return (
    <Card title="El equipo, pieza a pieza">
      <div className="grid gap-4 @3xl:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)]">
        {/* La «placa»: un fondo con pistas y las piezas encima. */}
        <div
          className="relative overflow-hidden rounded-2xl border border-line/70 p-4"
          style={{
            background:
              "radial-gradient(circle at 30% 20%, color-mix(in srgb, var(--color-neon) 10%, transparent), transparent 55%), repeating-linear-gradient(90deg, transparent 0 38px, color-mix(in srgb, var(--color-line) 55%, transparent) 38px 39px), repeating-linear-gradient(0deg, transparent 0 38px, color-mix(in srgb, var(--color-line) 40%, transparent) 38px 39px), var(--color-panel-2)",
          }}
        >
          <div className="grid grid-cols-2 gap-2.5 @lg:grid-cols-3">{hw.map(tile)}</div>
          <div className="my-3 flex items-center gap-2 text-[10px] tracking-widest text-mute uppercase">
            <span className="h-px flex-1 bg-line-2" /> Windows y red <span className="h-px flex-1 bg-line-2" />
          </div>
          <div className="grid grid-cols-3 gap-2.5">{sw.map(tile)}</div>
        </div>

        <div className="min-w-0 rounded-2xl bg-panel-2/50 p-4">
          {sel ? (
            <>
              <div className="flex items-center gap-2">
                <span className={`size-2.5 rounded-full ${DOT[sel.status]}`} />
                <span className="text-[15px] font-semibold text-ink">{sel.label}</span>
                <span className={`ml-auto text-xs ${sel.status === "bad" ? "text-bad" : sel.status === "warn" ? "text-warn" : "text-ok"}`}>{WORD[sel.status]}</span>
              </div>
              {sel.sub && <div className="mt-0.5 truncate text-xs text-mute">{sel.sub}</div>}
              {sel.findings.length === 0 ? (
                <p className="mt-3 text-[13px] text-dim">El último diagnóstico no encontró nada en esta pieza.</p>
              ) : (
                <ul className="mt-3 space-y-2">
                  {sel.findings.map((f) => (
                    <li key={f.key || f.title} className="rounded-lg border border-line/70 bg-panel px-3 py-2">
                      <div className="flex items-start gap-2 text-[13px]">
                        <span className={`mt-1.5 size-1.5 shrink-0 rounded-full ${f.severity === "bad" ? "bg-bad" : "bg-warn"}`} />
                        <span className="min-w-0 flex-1 text-ink">{f.title}</span>
                      </div>
                      <button onClick={() => go(f)} className="mt-1 pl-3.5 text-xs text-neon hover:underline">
                        {f.actions.find((x) => x.kind === "page")?.label ?? "Ver en Diagnóstico"} →
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </>
          ) : (
            <p className="text-[13px] text-mute">Pulsa una pieza para ver qué le pasa y cómo se arregla.</p>
          )}
        </div>
      </div>
    </Card>
  );
}
