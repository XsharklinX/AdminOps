// Windows Update: lo que se sabe de un fallo, en la propia fila. El código en una frase,
// la causa comprobada en este equipo y el botón del escalón que toca; «Siguiente paso»
// pasa al siguiente de la escalera.
import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { tweaksApi, updateApi, type UpdateCause } from "../lib/api";
import { advance, markTried, nextStep, startLadder, tryLabel, type Ladder } from "../lib/wuRow";
import { openPalette } from "../lib/palette";
import { ViewCommand } from "./ActionCommand";
import { useToast } from "./feedback";
import { Button } from "./ui";

// Leer el equipo para cada código tarda: se hace de uno en uno y una sola vez por código.
const causes = new Map<string, Promise<UpdateCause | null>>();
let chain: Promise<unknown> = Promise.resolve();

function loadCause(code: string, fresh = false): Promise<UpdateCause | null> {
  const key = code.toUpperCase();
  if (fresh) causes.delete(key);
  let p = causes.get(key);
  if (!p) {
    p = new Promise<UpdateCause | null>((resolve) => {
      chain = chain.then(() =>
        updateApi
          .cause(code)
          .catch(() => null)
          .then(resolve),
      );
    });
    causes.set(key, p);
  }
  return p;
}

export function UpdateFailure({ code, explanation, isAdmin }: { code: string; explanation: string | null; isAdmin: boolean }) {
  // undefined: se está comprobando
  const [cause, setCause] = useState<UpdateCause | null | undefined>(undefined);
  const [ladder, setLadder] = useState<Ladder>({ step: null, tried: [] });
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const toast = useToast();

  useEffect(() => {
    let alive = true;
    setCause(undefined);
    void loadCause(code).then((c) => {
      if (!alive) return;
      setCause(c);
      setLadder((l) => (l.tried.length === 0 ? startLadder(c) : l));
    });
    return () => {
      alive = false;
    };
  }, [code]);

  const step = ladder.step;
  const fixId = step?.kind === "fix" ? step.id : null;
  const more = nextStep(ladder);

  const tryIt = async () => {
    if (!fixId) return;
    setBusy(true);
    try {
      const r = await tweaksApi.run(fixId);
      toast("ok", r.message || "Hecho");
      setLadder((l) => markTried(l, fixId));
      setNote("Vuelve a buscar actualizaciones para comprobarlo.");
      // La causa se vuelve a medir: si ya no está, la frase de antes no puede seguir ahí.
      void loadCause(code, true).then(setCause);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mt-0.5 ml-6 space-y-1.5">
      <p className="text-xs text-dim">
        <span className="font-mono text-mute">{code}</span> {explanation ?? "Error sin explicación conocida: busca el código en la web de Microsoft."}
      </p>
      {cause === undefined && (
        <p className="flex items-center gap-1.5 text-xs text-mute">
          <Loader2 size={11} className="animate-spin" /> Comprobando en este equipo…
        </p>
      )}
      {cause && <p className="text-xs text-ink">{cause.sentence}</p>}
      {cause === null && ladder.tried.length > 0 && <p className="text-xs text-ok">Ya no se detecta esa causa en este equipo.</p>}
      {cause === null && ladder.tried.length === 0 && (
        <button type="button" onClick={() => openPalette("Windows Update da error")} className="text-xs text-neon hover:underline">
          Ver en Soluciones
        </button>
      )}
      {cause && !cause.fix && ladder.tried.length === 0 && cause.kind === "reboot" && <p className="text-xs text-dim">Reinicia el equipo y vuelve a buscar actualizaciones.</p>}
      {step?.kind === "fix" && (
        <div className="flex flex-wrap items-center gap-2 pt-0.5">
          <Button size="sm" onClick={tryIt} disabled={!isAdmin || busy} title={isAdmin ? undefined : "Requiere administrador"}>
            {tryLabel(step.id)}
          </Button>
          <ViewCommand id={step.id} />
          {more && (
            <Button size="sm" kind="ghost" onClick={() => setLadder((l) => advance(l))} disabled={busy}>
              Siguiente paso
            </Button>
          )}
        </div>
      )}
      {step?.kind === "manual" && (
        <p className="text-xs text-dim">
          <span className="text-ink">{step.label}.</span> {step.detail}
        </p>
      )}
      {note && <p className="text-xs text-ok">{note}</p>}
    </div>
  );
}
