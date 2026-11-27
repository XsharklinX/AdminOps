import { BellRing, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { webNotifApi, type WebNotif } from "../lib/api";
import { useToast } from "./feedback";
import { Button, Card } from "./ui";

/** Lo que identifica un aviso de un navegador y perfil concretos. */
export const notifKey = (n: WebNotif) => `${n.browser}\u0000${n.profile}\u0000${n.origin}`;

/**
 * Webs con permiso para mandar avisos a Windows. Los «avisos de virus» o de premios que salen solos
 * suelen venir de una de ellas. Quitar = bloquear el aviso (no vuelve a pedirlo); con el navegador cerrado.
 */
export function WebNotifCard() {
  const [list, setList] = useState<WebNotif[] | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [error, setError] = useState("");
  const toast = useToast();

  const load = useCallback(async () => {
    try {
      const l = await webNotifApi.list();
      setList(l);
      setPicked(new Set(l.filter((n) => n.suspicious).map(notifKey)));
      setError("");
    } catch (e) {
      setError(String(e));
      setList((l) => l ?? []);
    }
  }, []);
  useEffect(() => void load(), [load]);

  const toggle = (k: string) =>
    setPicked((p) => {
      const n = new Set(p);
      if (!n.delete(k)) n.add(k);
      return n;
    });

  const remove = async () => {
    const items = (list ?? []).filter((n) => picked.has(notifKey(n))).map((n): [string, string, string] => [n.browser, n.profile, n.origin]);
    if (items.length === 0) return;
    try {
      const n = await webNotifApi.remove(items);
      setError("");
      toast("ok", n === 1 ? "Aviso bloqueado." : `${n} avisos bloqueados.`);
      await load();
    } catch (e) {
      // Con el navegador abierto no se puede: el mensaje dice cuál cerrar.
      setError(String(e));
    }
  };

  return (
    <Card
      title="Avisos de webs"
      icon={<BellRing size={14} />}
      right={
        <Button kind="ghost" onClick={load}>
          <RefreshCw size={13} /> Volver a mirar
        </Button>
      }
    >
      <p className="mb-2 text-sm text-mute">Webs con permiso para mandar avisos a Windows. Las que fingen avisos de virus o premios suelen estar aquí.</p>
      {list !== null && list.length === 0 && <p className="text-sm text-mute">Ninguna web puede mandar avisos.</p>}
      {list !== null && list.length > 0 && (
        <ul className="divide-y divide-line/60 rounded-lg border border-line px-3">
          {list.map((n) => {
            const k = notifKey(n);
            return (
              <li key={k}>
                <label className="flex cursor-pointer items-center gap-3 py-1.5 text-sm">
                  <input type="checkbox" checked={picked.has(k)} onChange={() => toggle(k)} />
                  <span className="min-w-0 flex-1 truncate text-ink" title={n.host}>
                    {n.host}
                  </span>
                  <span className="shrink-0 text-xs text-mute">{n.browser}</span>
                  {n.suspicious && <span className="shrink-0 text-xs font-medium text-bad">sospechosa</span>}
                  {n.trusted && <span className="shrink-0 text-xs font-medium text-ok">de confianza</span>}
                </label>
              </li>
            );
          })}
        </ul>
      )}
      {error && <p className="mt-2 text-sm text-bad">{error}</p>}
      {list !== null && list.length > 0 && (
        <div className="mt-3 flex flex-wrap items-center gap-3">
          <Button disabled={picked.size === 0} onClick={remove}>
            Quitar {picked.size === 1 ? "la marcada" : `las ${picked.size} marcadas`}
          </Button>
          <span className="text-xs text-mute">Se bloquea el aviso: la web no podrá volver a pedirlo. Se guarda una copia de los ajustes.</span>
        </div>
      )}
    </Card>
  );
}
