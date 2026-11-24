// ¿Está activo el modo auditoría (AdminOps solo mira)? Para desactivar los botones que cambian el equipo.
// Se entera también cuando se activa o se quita desde la barra de arriba.
import { useEffect, useState } from "react";
import { auditApi, logQuietly } from "./api";

const EVENT = "adminops:audit";

export function useAuditOn(): boolean {
  const [on, setOn] = useState(false);
  useEffect(() => {
    auditApi.get().then(setOn).catch(logQuietly("useAuditOn"));
    const f = (e: Event) => setOn((e as CustomEvent<boolean>).detail);
    window.addEventListener(EVENT, f);
    return () => window.removeEventListener(EVENT, f);
  }, []);
  return on;
}

/** El texto de ayuda de un botón desactivado por la auditoría. */
export const AUDIT_TITLE = "Modo auditoría: AdminOps solo mira";
