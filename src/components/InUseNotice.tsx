import { useEffect, useState } from "react";
import { invoke } from "../lib/api/core";
import { Notice } from "./ui";

/**
 * «La cámara la está usando otra aplicación»: cuál. Lo dice Windows (ConsentStore, ver inuse.rs). Se vuelve a mirar cada
 * 5 s mientras la tarjeta está a la vista; si nadie la usa, no enseña nada.
 */
export function InUseNotice({ kind }: { kind: "webcam" | "microphone" }) {
  const [apps, setApps] = useState<string[]>([]);
  useEffect(() => {
    let alive = true;
    const look = () =>
      invoke<{ apps: string[] }>("device_in_use", { kind })
        .then((r) => alive && setApps(r.apps))
        .catch(() => {});
    void look();
    const t = window.setInterval(() => void look(), 5000);
    return () => {
      alive = false;
      window.clearInterval(t);
    };
  }, [kind]);
  if (!apps.length) return null;
  const cam = kind === "webcam";
  return (
    <Notice tone="warn" className="mb-3">
      Ahora mismo {cam ? "la cámara la" : "el micrófono lo"} está usando <b>{apps.join(", ")}</b>. Si otro programa (Teams, Zoom) dice que está {cam ? "ocupada" : "ocupado"}, cierra ese primero (Procesos lo cierra si no responde).
    </Notice>
  );
}
