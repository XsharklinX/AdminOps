// «En tus casos, lo que funcionó»: las soluciones de un síntoma ordenadas por
// las veces que de verdad lo arreglaron.
import { TrendingUp } from "lucide-react";
import { useEffect, useState } from "react";
import { knowledgeApi, type LearnedStat } from "../lib/api";
import { Bar } from "./ui";

export function LearnedStats({ topic, className = "" }: { topic: string; className?: string }) {
  const [stats, setStats] = useState<LearnedStat[] | null>(null);
  useEffect(() => {
    let alive = true;
    knowledgeApi.stats(topic).then(
      (s) => alive && setStats(s),
      () => alive && setStats([]),
    );
    return () => {
      alive = false;
    };
  }, [topic]);
  if (!stats?.length) return null;
  const total = stats.reduce((t, s) => t + s.count, 0);
  return (
    <div className={`rounded-lg border border-line bg-void/30 px-3 py-2.5 ${className}`}>
      <p className="mb-2 flex items-center gap-1.5 text-xs font-medium text-ink">
        <TrendingUp size={13} className="text-neon" /> En tus casos, lo que funcionó
        <span className="font-normal text-mute">· {total} {total === 1 ? "caso" : "casos"}</span>
      </p>
      <ul className="space-y-1.5">
        {stats.slice(0, 5).map((s, i) => (
          <li key={s.fix} className="grid grid-cols-[1fr_8rem_3rem] items-center gap-3 text-xs">
            <span className="truncate text-dim">
              {i + 1}. {s.fix}
            </span>
            <Bar value={s.pct} color={i === 0 ? "var(--color-ok)" : "var(--color-neon-2)"} />
            <span className="text-right font-mono text-mute">{s.pct} %</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
