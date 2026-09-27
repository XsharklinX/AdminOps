import { useEffect } from "react";

/** Aviso interno: el diario cambió fuera de la página (p. ej. «Deshacer» en un aviso). */
const JOURNAL_EVENT = "adminops:journal";

export const notifyJournalChange = () => window.dispatchEvent(new Event(JOURNAL_EVENT));

/** Vuelve a cargar la página cuando se deshace un cambio desde otro sitio. */
export function useOnJournalChange(reload: () => unknown) {
  useEffect(() => {
    const f = () => void reload();
    window.addEventListener(JOURNAL_EVENT, f);
    return () => window.removeEventListener(JOURNAL_EVENT, f);
  }, [reload]);
}
