import { useLiveMetrics } from "../hooks/useLiveMetrics";
import { tempColor, useSensors } from "../hooks/useSensors";
import { loadColor, rate } from "../lib/format";
import { Sparkline } from "./ui";

/**
 * El mini monitor: una ventanita siempre encima (minimon.rs) con lo justo para
 * vigilar el equipo mientras se prueba otra cosa. Sin botones: se cierra con su X.
 */
export function MiniMonitor() {
  const { metrics: m, history } = useLiveMetrics(1500);
  const { sensors } = useSensors(5000);
  if (!m) return <p className="grid h-screen place-items-center bg-void text-xs text-mute">Leyendo el equipo…</p>;

  const ram = m.memoryTotal ? (m.memoryUsed / m.memoryTotal) * 100 : 0;
  const cpuTemp = sensors?.cpuTemp ?? null;
  const gpu = sensors?.gpus.find((g) => g.temperature != null) ?? null;
  const top = m.topProcesses[0];

  const row = (label: string, value: string, color: string, data: number[], max: number) => (
    <div className="flex items-center gap-2">
      <span className="w-9 shrink-0 text-[11px] text-mute">{label}</span>
      <div className="min-w-0 flex-1">
        <Sparkline data={data} max={max} color={color} height={18} />
      </div>
      <span className="w-12 shrink-0 text-right font-mono text-[13px] tabular" style={{ color }}>
        {value}
      </span>
    </div>
  );

  return (
    <div className="flex h-screen flex-col justify-between gap-1.5 overflow-hidden bg-void px-3 py-2.5 text-ink select-none">
      {row("CPU", `${Math.round(m.cpuTotal)} %`, loadColor(m.cpuTotal), history.cpu, 100)}
      {row("RAM", `${Math.round(ram)} %`, loadColor(ram), history.ram, 100)}
      <div className="flex items-center gap-2 text-[11px]">
        <span className="w-9 shrink-0 text-mute">Temp.</span>
        <span className="min-w-0 flex-1 truncate font-mono text-[12px]">
          {cpuTemp != null ? <span style={{ color: tempColor(cpuTemp) }}>CPU {cpuTemp.toFixed(0)} °C</span> : <span className="text-mute">sin lectura</span>}
          {gpu?.temperature != null && <span style={{ color: tempColor(gpu.temperature) }}> · GPU {gpu.temperature.toFixed(0)} °C</span>}
        </span>
      </div>
      <div className="flex items-center gap-2 text-[11px]">
        <span className="w-9 shrink-0 text-mute">Red</span>
        <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-dim">
          ↓ {rate(m.netRxPerSec)} · ↑ {rate(m.netTxPerSec)}
        </span>
      </div>
      {top && (
        <div className="truncate border-t border-line pt-1.5 text-[11px] text-mute">
          Lo que más usa: <span className="text-dim">{top.name}</span> · {Math.round(top.cpu)} %
        </div>
      )}
    </div>
  );
}
