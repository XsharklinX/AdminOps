// «Pregúntale al equipo»: preguntas fijas, con la respuesta en una frase y las
// pruebas debajo. Si no hay pruebas, dice «No lo sé» y qué mirar. Solo lee.
import { CircleHelp, Loader2, MessageCircleQuestion } from "lucide-react";
import { useEffect, useState } from "react";
import { askPcApi, logQuietly, printersApi, type PcAnswer, type PcQuestion, type PrinterInfo } from "../lib/api";
import { goToPage } from "../lib/navigate";
import { addDays, isoDay, lastMonday, weekdayOf } from "../lib/whatChanged";
import { useToast } from "./feedback";
import { Button, Card, inputClass } from "./ui";

const when = (t: number) => new Date(t * 1000).toLocaleString("es", { day: "2-digit", month: "2-digit", year: "numeric", hour: "2-digit", minute: "2-digit" });

export function AskPc() {
  const [busy, setBusy] = useState<PcQuestion | null>(null);
  const [answer, setAnswer] = useState<PcAnswer | null>(null);
  const [since, setSince] = useState(() => isoDay(lastMonday(new Date())));
  const [program, setProgram] = useState("");
  const [printers, setPrinters] = useState<PrinterInfo[]>([]);
  const [printer, setPrinter] = useState("");
  const toast = useToast();

  useEffect(() => {
    printersApi
      .list()
      .then((l) => {
        setPrinters(l);
        setPrinter((p) => p || l[0]?.name || "");
      })
      .catch(logQuietly("AskPc"));
  }, []);

  const ask = async (q: PcQuestion, arg?: string, day?: string) => {
    setBusy(q);
    setAnswer(null);
    try {
      setAnswer(await askPcApi.ask(q, arg, day));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const spin = (q: PcQuestion) => (busy === q ? <Loader2 size={13} className="animate-spin" /> : <CircleHelp size={13} />);

  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <Card title="Pregúntale al equipo" icon={<MessageCircleQuestion size={14} />}>
        <p className="mb-3 text-sm text-dim">
          Preguntas fijas, con la respuesta en una frase y las pruebas debajo. Sale de lo que AdminOps ya lee de este equipo; si no hay pruebas, dice «No lo sé» y qué mirar. Solo lee.
        </p>
        <ul className="space-y-2">
          <li className="flex flex-wrap items-center gap-2">
            <Button kind="ghost" onClick={() => void ask("restart")} disabled={!!busy}>
              {spin("restart")} ¿Por qué se reinició anoche?
            </Button>
          </li>
          <li className="flex flex-wrap items-center gap-2">
            <Button kind="ghost" onClick={() => void ask("crash")} disabled={!!busy}>
              {spin("crash")} ¿Se apagó o se colgó?
            </Button>
          </li>
          <li className="flex flex-wrap items-center gap-2">
            <Button kind="ghost" onClick={() => void ask("slow", undefined, since)} disabled={!!busy || !since}>
              {spin("slow")} ¿Por qué va lento desde el {weekdayOf(since)}?
            </Button>
            <input
              type="date"
              value={since}
              max={isoDay(new Date())}
              min={isoDay(addDays(new Date(), -30))}
              onChange={(e) => e.target.value && setSince(e.target.value)}
              className="rounded-md border border-line bg-void px-2 py-1 text-xs text-ink"
              aria-label="Desde qué día va lento"
            />
          </li>
          <li className="flex flex-wrap items-center gap-2">
            <Button kind="ghost" onClick={() => void ask("installed", program)} disabled={!!busy || program.trim().length < 2}>
              {spin("installed")} ¿Quién instaló esto?
            </Button>
            <input value={program} onChange={(e) => setProgram(e.target.value)} placeholder="Nombre del programa" className={`${inputClass} max-w-64`} aria-label="Nombre del programa" />
          </li>
          <li className="flex flex-wrap items-center gap-2">
            <Button kind="ghost" onClick={() => void ask("printer", printer)} disabled={!!busy || !printer}>
              {spin("printer")} ¿Por qué no imprime?
            </Button>
            {printers.length > 0 ? (
              <select value={printer} onChange={(e) => setPrinter(e.target.value)} className={`${inputClass} max-w-72`} aria-label="Impresora">
                {printers.map((p) => (
                  <option key={p.name} value={p.name}>
                    {p.name}
                  </option>
                ))}
              </select>
            ) : (
              <span className="text-xs text-mute">No hay impresoras en este equipo.</span>
            )}
          </li>
        </ul>
      </Card>

      {busy && (
        <p className="flex items-center gap-2 px-1 text-xs text-mute" role="status">
          <Loader2 size={13} className="animate-spin" /> Mirando lo que ha pasado en el equipo…
        </p>
      )}
      {answer && (
        <Card title={answer.question}>
          <p className={`text-base ${answer.known ? "text-ink" : "text-warn"}`}>{answer.answer}</p>
          {!answer.known && answer.lookAt && <p className="mt-1 text-sm text-dim">Qué mirar: {answer.lookAt}</p>}
          {answer.evidence.length > 0 && (
            <div className="mt-3 border-t border-line pt-2">
              <h3 className="mb-1 text-[11px] font-medium tracking-wide text-mute uppercase">Las pruebas</h3>
              <ul className="space-y-1 text-xs">
                {answer.evidence.map((e, i) => (
                  <li key={i} className="flex gap-3">
                    <span className="w-32 shrink-0 font-mono text-mute">{e.t ? when(e.t) : ""}</span>
                    <span className="min-w-0 flex-1 text-dim [overflow-wrap:anywhere]">{e.text}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {answer.page && (
            <div className="mt-3">
              <Button kind="ghost" onClick={() => goToPage(answer.page ?? "machine", answer.focus)}>
                Ir a verlo
              </Button>
            </div>
          )}
        </Card>
      )}
    </div>
  );
}
