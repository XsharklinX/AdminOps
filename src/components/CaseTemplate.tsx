// «Guardar como plantilla»: lo que se hizo en el caso, sacado del diario, se
// convierte en una plantilla de «Preparar equipos». Lo que falló no sale, y lo que
// solo valía para ese equipo sale desactivado.
import { Loader2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { casesApi, libraryApi, type Case, type TemplateDraft } from "../lib/api";
import { caseSource, checkedByDefault, stepsFromDraft } from "../lib/caseTemplate";
import { RISK } from "../lib/risk";
import { useToast } from "./feedback";
import { Button, inputClass, Modal } from "./ui";

export function CaseTemplate({ current, onClose }: { current: Case; onClose: () => void }) {
  const [draft, setDraft] = useState<TemplateDraft | null>(null);
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const toast = useToast();

  useEffect(() => {
    let vivo = true;
    casesApi
      .templateDraft()
      .then((d) => {
        if (!vivo) return;
        setDraft(d);
        // Lo que se puede repetir sale marcado, salvo lo de riesgo medio o alto: eso se elige a propósito.
        setPicked(new Set(d.items.flatMap((x, i) => (checkedByDefault(x) ? [i] : []))));
      })
      .catch((e) => vivo && setError(String(e)));
    return () => {
      vivo = false;
    };
  }, []);

  const source = useMemo(() => caseSource(current), [current]);

  const toggle = (i: number) =>
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(i)) n.delete(i);
      else n.add(i);
      return n;
    });

  const save = async () => {
    if (!draft) return;
    if (!name.trim()) return toast("error", "Pon un nombre a la plantilla.");
    const steps = stepsFromDraft(draft, picked);
    if (!steps.length) return toast("error", "Marca al menos una acción.");
    setBusy(true);
    try {
      await libraryApi.save("recipes", { id: "", name: name.trim(), description: source, steps, isNew: true });
      toast("ok", "Plantilla guardada");
      onClose();
    } catch (e) {
      toast("error", String(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      title={
        <span>
          Nueva plantilla
          <span className="block text-xs font-normal text-mute">{source}</span>
        </span>
      }
      onClose={onClose}
      width="w-[560px]"
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={() => void save()} disabled={!draft || busy}>
            {busy && <Loader2 size={14} className="animate-spin" />} Guardar plantilla
          </Button>
        </>
      }
    >
      {error && <p className="text-sm text-bad">{error}</p>}
      {!draft && !error && (
        <p className="flex items-center gap-2 text-sm text-mute">
          <Loader2 size={14} className="animate-spin" /> Leyendo lo que se hizo…
        </p>
      )}
      {draft && (
        <div className="space-y-3">
          {draft.items.length === 0 ? (
            <p className="text-sm text-dim">En este caso no se hizo nada que se pueda repetir.</p>
          ) : (
            <ul className="divide-y divide-line/60 rounded-lg border border-line">
              {draft.items.map((item, i) => (
                <li key={i}>
                  <label className={`flex items-center gap-3 px-3 py-2 text-[13px] ${item.repeatable ? "cursor-pointer text-ink" : "text-mute"}`}>
                    <input
                      type="checkbox"
                      checked={item.repeatable && picked.has(i)}
                      disabled={!item.repeatable}
                      onChange={() => toggle(i)}
                      className="size-4 accent-[var(--color-neon)]"
                    />
                    <span className="min-w-0 flex-1 break-words">
                      {item.title}
                      {item.repeatable && item.note &&<span className="mt-0.5 block text-[11px] font-normal text-mute">{item.note}</span>}
                    </span>
                    {item.repeatable && item.risk && (
                      <span className={`shrink-0 rounded border px-1.5 py-px text-[11px] font-medium ${RISK[item.risk].cls}`}>{RISK[item.risk].label}</span>
                    )}
                    {item.reason && <span className="shrink-0 text-[11px]">{item.reason}</span>}
                  </label>
                </li>
              ))}
            </ul>
          )}
          {draft.removed.map((t) => (
            <p key={t} className="text-xs text-mute">
              Quitado: {t} (falló)
            </p>
          ))}
          <label className="block text-xs text-dim">
            Nombre
            <input autoFocus value={name} onChange={(e) => setName(e.target.value)} className={`${inputClass} mt-1`} aria-required="true" />
          </label>
        </div>
      )}
    </Modal>
  );
}
