// Franja del modo demostración y su interruptor (Ajustes → General).
import { Presentation } from "lucide-react";
import { setPrefs, usePrefs } from "../lib/prefs";

/** Enciende o apaga la demostración y vuelve a cargar las pantallas con los datos que tocan. */
export function setDemo(on: boolean) {
  setPrefs({ demo: on });
  window.setTimeout(() => window.location.reload(), 150);
}

export function DemoBanner() {
  const { demo } = usePrefs();
  if (!demo) return null;
  return (
    <div className="flex items-center gap-3 border-b border-neon/40 bg-neon/10 px-8 py-2 text-sm text-neon">
      <Presentation size={15} />
      <span className="flex-1">
        Modo demostración: clientes, contactos e inventario son de ejemplo y no se guarda nada. Lo que se ve del equipo (procesador, discos…) es el de verdad.
      </span>
      <button onClick={() => setDemo(false)} className="rounded border border-neon/50 px-2 py-0.5 text-xs hover:bg-neon/15">
        Salir
      </button>
    </div>
  );
}
