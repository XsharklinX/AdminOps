// «Deshacer» siempre a mano: lo último que cambió AdminOps en el equipo
// («Desactivar telemetría»), con Ctrl+Z fuera de los campos de texto. Se apoya
// en el diario, que ya guarda el valor anterior de cada cambio.
import { listen } from "@tauri-apps/api/event";
import { Loader2, Undo2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { logQuietly, tweaksApi, type JournalEntry } from "../lib/api";
import { notifyJournalChange, useOnJournalChange } from "../lib/journalEvents";
import { useToast } from "./feedback";

/** Lo último que se puede deshacer (no deshecho, no fallido, de las últimas 24 h). */
export function lastUndoable(entries: JournalEntry[], now = Date.now()): JournalEntry | null {
  const day = 24 * 3600 * 1000;
  const list = entries.filter((e) => e.undoable && e.ok && !e.reverted && e.op !== "revert" && now - e.timestamp * (e.timestamp < 1e12 ? 1000 : 1) < day);
  return list.sort((a, b) => b.timestamp - a.timestamp)[0] ?? null;
}

/** ¿La tecla se pulsó escribiendo en un campo? (ahí Ctrl+Z es deshacer el texto). */
const typing = (t: EventTarget | null) => {
  const el = t as HTMLElement | null;
  return !!el && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName));
};

export function UndoLast() {
  const toast = useToast();
  const [last, setLast] = useState<JournalEntry | null>(null);
  const [busy, setBusy] = useState(false);
  const load = useCallback(() => tweaksApi.journal().then((j) => setLast(lastUndoable(j))).catch(logQuietly("UndoLast")), []);
  useEffect(() => {
    void load();
    const off = listen("undoable-change", () => window.setTimeout(() => void load(), 300));
    return () => void off.then((f) => f());
  }, [load]);
  useOnJournalChange(load);

  const undo = useCallback(async () => {
    if (!last || busy) return;
    setBusy(true);
    try {
      const r = await tweaksApi.revertEntry(last.id);
      toast("ok", `Deshecho: ${last.title}. ${r.message}`);
      notifyJournalChange();
    } catch (e) {
      toast("error", `No se pudo deshacer «${last.title}»: ${e}`);
    } finally {
      setBusy(false);
      void load();
    }
  }, [last, busy, toast, load]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "z") || typing(e.target)) return;
      if (document.querySelector('[role="dialog"]')) return;
      e.preventDefault();
      if (last) void undo();
      else toast("info", "No hay ningún cambio reciente que deshacer.");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [last, undo, toast]);

  if (!last) return null;
  return (
    <button
      onClick={() => void undo()}
      disabled={busy}
      data-tour="undo"
      className="flex max-w-56 items-center gap-1.5 rounded-md px-2 py-1 text-xs text-mute transition-colors hover:bg-panel-2 hover:text-ink"
      title={`Deshacer «${last.title}» (Ctrl+Z)`}
    >
      {busy ? <Loader2 size={13} className="animate-spin" /> : <Undo2 size={13} />}
      <span className="truncate">Deshacer</span>
    </button>
  );
}
