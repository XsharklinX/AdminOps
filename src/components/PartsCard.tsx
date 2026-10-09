// Repuestos compatibles: con lo que el equipo cuenta de sí mismo (tipo y velocidad de la memoria, ranuras,
// discos), dice qué se puede comprar para ampliarlo y qué no encaja. Lo que Windows no sabe, lo dice.
import { CircleAlert, CircleCheck, ClipboardCopy, Info, Loader2, MemoryStick } from "lucide-react";
import { useState } from "react";
import { useToast } from "./feedback";
import { Button, Card, smallBtn } from "./ui";
import { partsApi, type PartsAdvice } from "../lib/api";

const ICON = {
  ok: <CircleCheck size={14} className="text-ok" />,
  warn: <CircleAlert size={14} className="text-warn" />,
  info: <Info size={14} className="text-mute" />,
} as const;

export function PartsCard() {
  const toast = useToast();
  const [a, setA] = useState<PartsAdvice | null>(null);
  const [busy, setBusy] = useState(false);
  const copy = (text: string, ok: string) => void navigator.clipboard.writeText(text).then(() => toast("ok", ok), () => toast("error", "No se pudo copiar."));

  const run = async () => {
    setBusy(true);
    try {
      setA(await partsApi.advice());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card title="Repuestos compatibles" icon={<MemoryStick size={14} />}>
      <div className="space-y-3 p-4">
        <p className="text-xs leading-relaxed text-dim">Con el tipo y la velocidad de la memoria, las ranuras y los discos de este equipo, dice qué comprar para ampliarlo y qué no encaja, para no equivocarse con la compra.</p>
        <Button onClick={() => void run()} disabled={busy}>
          {busy ? <Loader2 size={14} className="animate-spin" /> : <MemoryStick size={14} />} {a ? "Volver a mirar" : "Ver qué se puede comprar"}
        </Button>
        {a && (
          <>
            {a.model && <p className="text-xs text-mute">{a.model}</p>}
            <ul className="divide-y divide-line/60 rounded-lg border border-line">
              {a.items.map((i) => (
                <li key={i.title} className="flex gap-3 px-3 py-2">
                  <span className="mt-0.5 shrink-0">{ICON[i.level]}</span>
                  <div className="min-w-0 text-xs">
                    <div className="font-medium text-ink">{i.title}</div>
                    <p className="mt-0.5 leading-relaxed text-dim">{i.text}</p>
                  </div>
                </li>
              ))}
            </ul>
            <div className="flex flex-wrap gap-2">
              {a.shopping.length > 0 && (
                <button type="button" className={smallBtn} onClick={() => copy(a.shopping.map((s) => `• ${s}`).join("\n"), "Lista de compra copiada.")}>
                  <ClipboardCopy size={12} /> Copiar la lista de compra
                </button>
              )}
              <button type="button" className={smallBtn} onClick={() => copy(a.manualQuery, "Búsqueda copiada: pégala en el navegador.")} title="Para buscar el manual de servicio de este modelo">
                <ClipboardCopy size={12} /> Copiar búsqueda del manual
              </button>
            </div>
          </>
        )}
      </div>
    </Card>
  );
}
