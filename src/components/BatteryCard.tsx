import { BatteryMedium, Loader2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { hwApi, type BatteryHistory } from "../lib/api";
import { dayNumber, floorOf, healthOf, monthsText, monthTicks } from "../lib/batteryChart";
import { Card } from "./ui";

const W = 1000;
const H = 200;
const PAD = { left: 40, right: 12, top: 10, bottom: 24 };

/**
 * La batería a lo largo del tiempo (Hardware). En un equipo sin batería no se
 * enseña nada. Los datos son los que Windows apunta cada semana.
 */
export function BatteryCard({ className = "" }: { className?: string }) {
  const [data, setData] = useState<BatteryHistory | null | "loading" | string>("loading");

  useEffect(() => {
    hwApi
      .batteryHistory()
      .then(setData)
      .catch((e) => setData(String(e)));
  }, []);

  const chart = useMemo(() => {
    if (!data || typeof data === "string" || data.points.length < 2) return null;
    const pts = data.points.map((p) => ({ x: dayNumber(p.day), y: healthOf(p), day: p.day }));
    const x0 = pts[0].x;
    const x1 = pts[pts.length - 1].x;
    const floor = floorOf(pts.map((p) => p.y));
    const sx = (x: number) => PAD.left + ((x - x0) / Math.max(1, x1 - x0)) * (W - PAD.left - PAD.right);
    const sy = (y: number) => PAD.top + ((100 - Math.min(100, y)) / (100 - floor)) * (H - PAD.top - PAD.bottom);
    const line = pts.map((p, i) => `${i ? "L" : "M"}${sx(p.x).toFixed(1)},${sy(p.y).toFixed(1)}`).join(" ");
    const area = `${line} L${sx(x1).toFixed(1)},${H - PAD.bottom} L${sx(x0).toFixed(1)},${H - PAD.bottom} Z`;
    const grid = [100, 80, 60, 50, 40, 20].filter((g) => g >= floor);
    return { line, area, grid, sx, sy, ticks: monthTicks(pts[0].day, pts[pts.length - 1].day), last: pts[pts.length - 1] };
  }, [data]);

  if (data === null) return null;

  return (
    <Card id="focus-battery" title="Batería" icon={<BatteryMedium size={14} />} className={className}>
      {data === "loading" ? (
        <p className="flex items-center gap-2 text-sm text-mute">
          <Loader2 size={13} className="animate-spin" /> Leyendo el informe de batería de Windows…
        </p>
      ) : typeof data === "string" ? (
        <p className="text-sm text-mute">No se pudo leer el historial de la batería: {data}</p>
      ) : (
        <>
          {(() => {
            const health = healthOf(data);
            const tone = health >= 80 ? "text-ok" : health >= 60 ? "text-warn" : "text-bad";
            return (
              <div className="mb-3 grid grid-cols-2 gap-3 md:grid-cols-4">
                <Stat label="Capacidad hoy" value={<span className={tone}>{Math.round(health)} %</span>} sub="de la de fábrica" />
                <Stat
                  label="Pierde al mes"
                  value={data.lossPerMonth == null ? "—" : `${data.lossPerMonth.toFixed(1)} puntos`}
                  sub={data.lossPerMonth == null ? "Hace falta más historial" : "según el último año"}
                />
                <Stat
                  label="A la mitad en"
                  value={data.monthsToHalf == null ? "—" : health <= 50 ? "ya está" : monthsText(data.monthsToHalf)}
                  sub={data.monthsToHalf == null ? (data.lossPerMonth != null ? "Apenas se gasta" : "Sin datos bastantes") : "a este ritmo"}
                />
                <Stat label="Ciclos de carga" value={data.cycles ?? "—"} sub={data.cycles == null ? "El fabricante no lo da" : "cargas completas"} />
              </div>
            );
          })()}
          {chart ? (
            <svg viewBox={`0 0 ${W} ${H}`} className="h-48 w-full" role="img" aria-label="Capacidad de la batería por semana, respecto a la de fábrica">
              {chart.grid.map((g) => (
                <g key={g}>
                  <line x1={PAD.left} x2={W - PAD.right} y1={chart.sy(g)} y2={chart.sy(g)} stroke="var(--color-line)" strokeDasharray={g === 50 ? "6 4" : undefined} />
                  <text x={PAD.left - 6} y={chart.sy(g) + 4} textAnchor="end" fontSize="12" fill="var(--color-mute)">
                    {g} %
                  </text>
                </g>
              ))}
              {chart.ticks.map((t) => (
                <text key={t.day} x={chart.sx(t.day)} y={H - 6} textAnchor="middle" fontSize="12" fill="var(--color-mute)">
                  {t.label}
                </text>
              ))}
              <path d={chart.area} fill="var(--color-neon)" fillOpacity="0.12" />
              <path d={chart.line} fill="none" stroke="var(--color-neon)" strokeWidth="2" vectorEffect="non-scaling-stroke" />
              <circle cx={chart.sx(chart.last.x)} cy={chart.sy(chart.last.y)} r="4" fill="var(--color-neon)" />
            </svg>
          ) : (
            <p className="text-sm text-mute">Windows aún no ha apuntado historial de esta batería: se ve al cabo de unas semanas de uso.</p>
          )}
          <p className="mt-2 text-[11px] text-mute">
            {data.name ? `${data.name} · ` : ""}De fábrica {Math.round(data.design / 1000)} Wh, hoy {Math.round(data.full / 1000)} Wh. Por debajo del 50 % suele
            convenir cambiarla: dura la mitad que nueva.
          </p>
        </>
      )}
    </Card>
  );
}

function Stat({ label, value, sub }: { label: string; value: React.ReactNode; sub: string }) {
  return (
    <div className="flex flex-col rounded-lg border border-line px-3 py-2">
      <span className="text-[11px] text-mute">{label}</span>
      <span className="text-lg font-semibold tabular text-ink">{value}</span>
      <span className="text-[11px] text-dim">{sub}</span>
    </div>
  );
}
