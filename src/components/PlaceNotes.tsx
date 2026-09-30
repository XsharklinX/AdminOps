import { BookOpen, MapPin, Network, Pin, Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "./feedback";
import { Button, inputClass } from "./ui";
import { libraryApi, type PlaceNote } from "../lib/api";

/**
 * Notas de ESTE equipo y de ESTA red, con alta rápida. Se muestra en el Panel,
 * así lo que apuntaste la última vez aparece solo al volver.
 */
export function PlaceNotes({ onChanged, compact }: { onChanged?: () => void; compact?: boolean }) {
  const [place, setPlace] = useState<Awaited<ReturnType<typeof libraryApi.place>> | null>(null);
  const [notes, setNotes] = useState<PlaceNote[]>([]);
  const [adding, setAdding] = useState<PlaceNote["scope"] | null>(null);
  const [text, setText] = useState("");
  const toast = useToast();

  const load = useCallback(async () => {
    const [p, all] = await Promise.all([libraryApi.place().catch(() => null), libraryApi.list("notes").catch(() => [])]);
    setPlace(p);
    setNotes(p ? all.filter((n) => n.key === p.machine || (p.network && n.key === p.network)) : []);
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  const add = async () => {
    if (!place || !adding || !text.trim()) return;
    const network = adding === "network";
    try {
      await libraryApi.save("notes", {
        scope: adding,
        key: network ? place.network : place.machine,
        label: network ? place.networkLabel : place.machineLabel,
        text: text.trim(),
      });
      setText("");
      setAdding(null);
      void load();
      onChanged?.();
    } catch (e) {
      toast("error", String(e));
    }
  };
  const remove = async (n: PlaceNote) => {
    await libraryApi.remove("notes", n.id).catch(() => {});
    void load();
    onChanged?.();
  };

  if (!place) return null;
  if (compact && !notes.length) return null;
  const machineNotes = notes.filter((n) => n.scope === "machine");
  const networkNotes = notes.filter((n) => n.scope === "network");
  const block = (scope: PlaceNote["scope"], label: string, items: PlaceNote[], Icon: typeof MapPin) =>
    (items.length > 0 || !compact) && (
      <div className="min-w-0 flex-1">
        <p className="mb-1 flex items-center gap-1.5 text-[11px] font-medium tracking-wide text-mute uppercase">
          <Icon size={12} /> {scope === "machine" ? "Este equipo" : "Esta red"} · <span className="normal-case">{label || "—"}</span>
        </p>
        {items.map((n) => (
          <div key={n.id} className="group flex items-start gap-2 py-0.5 text-sm">
            <Pin size={11} className="mt-1 shrink-0 text-warn" />
            <p className="flex-1 whitespace-pre-wrap text-ink select-text">{n.text}</p>
            <button onClick={() => remove(n)} className="p-0.5 text-mute opacity-0 group-hover:opacity-100 hover:text-bad" title="Borrar">
              <Trash2 size={11} />
            </button>
          </div>
        ))}
        {!compact && (scope === "machine" || place.network) && (
          <button onClick={() => setAdding(scope)} className="mt-1 flex items-center gap-1 text-xs text-neon hover:underline">
            <Plus size={11} /> Nota para {scope === "machine" ? "este equipo" : "esta red"}
          </button>
        )}
      </div>
    );
  return (
    <div className="rounded-xl border border-warn/30 bg-warn/5 px-4 py-3">
      <div className="flex flex-col gap-4 md:flex-row">
        {block("machine", place.machineLabel, machineNotes, MapPin)}
        {block("network", place.networkLabel, networkNotes, Network)}
      </div>
      {adding && (
        <div className="mt-3 flex gap-2">
          <textarea
            autoFocus
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void add();
              }
              if (e.key === "Escape") setAdding(null);
            }}
            rows={2}
            placeholder={adding === "machine" ? "La impresora de contabilidad está compartida desde aquí…" : "El router se reinicia a mano; la clave está en la pegatina…"}
            className={inputClass}
          />
          <div className="flex flex-col gap-1">
            <Button onClick={add}>Guardar</Button>
            <Button kind="ghost" onClick={() => setAdding(null)}>
              Cancelar
            </Button>
          </div>
        </div>
      )}
      {compact && (
        <p className="mt-2 text-[11px] text-mute">
          <BookOpen size={10} className="mr-1 inline" />
          Notas guardadas la última vez · Soporte → Conocimiento
        </p>
      )}
    </div>
  );
}
