// Medidas que solo se pueden tomar después: el arranque tras quitar programas del
// inicio. Al abrir AdminOps, si ya hubo un arranque posterior, se mide (con lo que
// ya hace «Lo que se gana») y se apunta en el diario, de donde pasa al caso y al informe.
// No dibuja nada: se monta una vez en la raíz de la aplicación.
import { useEffect } from "react";
import { logQuietly, maintenanceApi, measureApi } from "../lib/api";
import { hostName } from "../lib/machineHost";
import { forHost, measured, readBaseline, writeBaseline } from "../lib/startupGain";

export function PendingMeasures() {
  useEffect(() => {
    const saved = readBaseline();
    if (!saved || saved.logged) return;
    let vivo = true;
    // Solo si el apunte es de este equipo: en el USB viaja a otros y el arranque medido no sería suyo.
    hostName()
      .then(async (host) => {
        const b = forHost(saved, host);
        if (!b) return;
        const boot = await maintenanceApi.boot();
        const r = measured(boot.boots, b);
        if (!vivo || !r) return;
        const n = b.names.length;
        await measureApi.record(`Quitar ${n} ${n === 1 ? "programa" : "programas"} del inicio: arranque medido`, {
          label: "Arranque",
          before: r.beforeMs / 1000,
          after: r.afterMs / 1000,
          unit: "s",
        });
        // Una sola vez: StartupGain sigue enseñando el resultado.
        writeBaseline({ ...b, logged: true });
      })
      .catch(logQuietly("PendingMeasures"));
    return () => {
      vivo = false;
    };
  }, []);
  return null;
}
