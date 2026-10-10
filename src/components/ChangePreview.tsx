// «Antes → después»: el valor actual de cada cosa que tocará un ajuste (o un
// perfil entero) y el que tendrá, con lo que se puede deshacer y lo que no.
// Es la diferencia entre «¿seguro?» y «ya lo he visto».
import { ArrowRight, RotateCcw, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { tweaksApi, type TweakPreview } from "../lib/api";
import { Loading } from "./ui";

export function ChangePreview({ ids, compact = false }: { ids: string[]; compact?: boolean }) {
  const [data, setData] = useState<TweakPreview[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const key = ids.join(",");
  useEffect(() => {
    let alive = true;
    setData(null);
    tweaksApi
      .preview(key ? key.split(",") : [])
      .then((d) => alive && setData(d))
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
    };
  }, [key]);
  if (error) return <p className="text-xs text-bad">{error}</p>;
  if (!data) return <Loading text="Leyendo los valores de ahora…" />;
  const lines = data.flatMap((t) => t.lines.map((l) => ({ ...l, tweak: t.name })));
  const unchanged = lines.filter((l) => l.before === l.after).length;
  const noUndo = lines.filter((l) => !l.undoable).length;
  return (
    <div className="space-y-2">
      <div className={`overflow-auto rounded-md border border-line bg-void/50 ${compact ? "" : "max-h-64"}`}>
        <table className="w-full text-[11px]">
          <thead className="sticky top-0 bg-panel-2 text-mute">
            <tr>
              {!compact && <th className="px-2 py-1 text-left font-medium">Ajuste</th>}
              <th className="px-2 py-1 text-left font-medium">Qué</th>
              <th className="px-2 py-1 text-left font-medium">Ahora</th>
              <th />
              <th className="px-2 py-1 text-left font-medium">Después</th>
              <th className="px-2 py-1" />
            </tr>
          </thead>
          <tbody className="font-mono">
            {lines.map((l, i) => (
              <tr key={i} className={`border-t border-line/50 ${l.before === l.after ? "opacity-50" : ""}`}>
                {!compact && <td className="px-2 py-1 font-sans text-ink">{l.tweak}</td>}
                <td className="px-2 py-1 break-all text-dim">{l.what}</td>
                <td className="px-2 py-1 break-all text-dim">{l.before}</td>
                <td className="text-mute">
                  <ArrowRight size={11} />
                </td>
                <td className="px-2 py-1 break-all text-ink">{l.after}</td>
                <td className="px-2 py-1" title={l.undoable ? "Se puede deshacer" : "No se puede deshacer"}>
                  {l.undoable ? <RotateCcw size={11} className="text-ok" /> : <TriangleAlert size={11} className="text-warn" />}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="text-[11px] text-mute">
        {lines.length - unchanged} {lines.length - unchanged === 1 ? "cambio" : "cambios"}
        {unchanged > 0 && ` · ${unchanged} ya estaban así`}
        {noUndo > 0 ? ` · ${noUndo} sin vuelta atrás (marcados en ámbar)` : " · todo se puede deshacer"}
        {data.some((t) => t.reboot) && " · pide reiniciar"}
      </p>
    </div>
  );
}
