import { Keyboard, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "../../components/feedback";
import { NAV, type PageId } from "../../components/Sidebar";
import { Button, Card } from "../../components/ui";
import { comboOf, RESERVED, setPrefs, usePrefs } from "../../lib/prefs";

const labelOf = (p: PageId) => NAV.find((n) => n.id === p)?.label ?? p;

/** Atajos de teclado propios para abrir cualquier página. */
export function ShortcutEditor() {
  const prefs = usePrefs();
  const [page, setPage] = useState<PageId>("diagnostics");
  const [recording, setRecording] = useState(false);
  const toast = useToast();
  const entries = Object.entries(prefs.shortcuts).sort((a, b) => labelOf(a[1]).localeCompare(labelOf(b[1])));

  useEffect(() => {
    if (!recording) return;
    // Mientras se graba, los atajos de la app no actúan.
    document.body.dataset.recordingShortcut = "1";
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      // Antes que los atajos de la app (Ctrl+K, Ctrl+L…), que escuchan en la misma ventana.
      e.stopImmediatePropagation();
      if (e.key === "Escape") {
        setRecording(false);
        return;
      }
      const combo = comboOf(e);
      if (!combo) return;
      if (RESERVED.has(combo)) {
        toast("info", `${combo} ya lo usa AdminOps o Windows. Prueba con Ctrl+Alt+letra o una tecla F.`);
        return;
      }
      setPrefs({ shortcuts: { ...prefs.shortcuts, [combo]: page } });
      toast("ok", `${combo} abre «${labelOf(page)}».`);
      setRecording(false);
    };
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      delete document.body.dataset.recordingShortcut;
    };
  }, [recording, page, prefs.shortcuts, toast]);

  const remove = (combo: string) => {
    const next = { ...prefs.shortcuts };
    delete next[combo];
    setPrefs({ shortcuts: next });
  };

  return (
    <Card title="Atajos para abrir páginas" icon={<Keyboard size={14} />}>
      <p className="mb-3 text-sm text-dim">
        Asigna una combinación de teclas a cualquier página (por ejemplo Ctrl+Alt+D para Diagnóstico o F2 para Dispositivos en la red). Funcionan desde
        cualquier parte de AdminOps.
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <select value={page} onChange={(e) => setPage(e.target.value as PageId)} className="rounded-md border border-line bg-void/60 px-3 py-1.5 text-sm text-ink outline-none">
          {NAV.map((n) => (
            <option key={n.id} value={n.id}>
              {n.label}
            </option>
          ))}
        </select>
        <Button onClick={() => setRecording(!recording)} kind={recording ? "ghost" : "primary"}>
          <Plus size={14} /> {recording ? "Pulsa la combinación… (Esc cancela)" : "Asignar atajo"}
        </Button>
      </div>
      {entries.length > 0 && (
        <ul className="mt-4 divide-y divide-line/60">
          {entries.map(([combo, p]) => (
            <li key={combo} className="flex items-center gap-3 py-2 text-sm">
              <kbd className="min-w-28 rounded border border-line-2 px-2 py-0.5 text-center font-sans text-xs text-ink">{combo}</kbd>
              <span className="flex-1 text-dim">{labelOf(p)}</span>
              <button onClick={() => remove(combo)} className="text-mute hover:text-bad" title="Quitar atajo">
                <Trash2 size={14} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}
