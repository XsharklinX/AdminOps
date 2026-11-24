// Al abrir AdminOps, si había un plan de «Preparar equipos» esperando un reinicio y ya
// se reinició (lo lanza la tarea de un solo uso con --seguir-plan, o el equipo arrancó
// de nuevo desde que se guardó), lleva a «Preparar equipos», que lo retoma donde se quedó.
// Sin reinicio no se mete en medio: «Preparar equipos» enseña un aviso discreto.
// Monta también `PendingMeasures` (medidas que solo se pueden tomar tras reiniciar),
// para que App solo tenga que montar un componente. No dibuja nada.
import { useEffect } from "react";
import { logQuietly, planApi } from "../lib/api";
import { goToPage } from "../lib/navigate";
import { requestResume, shouldResume } from "../lib/planResume";
import { PendingMeasures } from "./PendingMeasures";

export function PlanResume() {
  useEffect(() => {
    let vivo = true;
    planApi
      .load()
      .then((l) => {
        if (!vivo || !l || !shouldResume(l)) return;
        requestResume();
        goToPage("recipes");
      })
      .catch(logQuietly("PlanResume"));
    return () => {
      vivo = false;
    };
  }, []);
  return <PendingMeasures />;
}
