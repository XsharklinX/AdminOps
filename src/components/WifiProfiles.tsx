import { Copy, Eye, EyeOff, Lock, RefreshCw, Trash2, Wifi } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "./feedback";
import { Card } from "./ui";
import { wifiApi, type WifiProfile } from "../lib/api";

const SECURITY: Record<string, string> = {
  open: "Abierta",
  WPA2PSK: "WPA2",
  WPA3SAE: "WPA3",
  WPAPSK: "WPA",
  WPA2: "WPA2 empresa",
  WPA3ENT: "WPA3 empresa",
  WPA3ENT192: "WPA3 empresa",
  WPA: "WPA empresa",
  shared: "WEP",
};

/** Redes Wi-Fi guardadas en el equipo con su contraseña (oculta hasta pulsar). */
export function WifiProfiles({ isAdmin }: { isAdmin: boolean }) {
  const [list, setList] = useState<WifiProfile[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [shown, setShown] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setList(await wifiApi.list());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const toggle = (name: string) =>
    setShown((s) => {
      const n = new Set(s);
      if (n.has(name)) n.delete(name);
      else n.add(name);
      return n;
    });

  const forget = async (p: WifiProfile) => {
    const ok = await confirm({
      title: "Olvidar red",
      body: `El equipo dejará de conectarse solo a «${p.name}»${p.connected ? " y se desconectará ahora" : ""}. Para volver a usarla habrá que escribir la contraseña.`,
      confirmLabel: "Olvidar",
      danger: true,
    });
    if (!ok) return;
    try {
      await wifiApi.forget(p.name);
      toast("ok", `Red «${p.name}» olvidada.`);
      load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <Card
      title="Redes Wi-Fi guardadas"
      icon={<Wifi size={14} />}
      className="col-span-12"
      right={
        <button onClick={load} disabled={loading} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
          <RefreshCw size={11} className={loading ? "animate-spin" : ""} /> Volver a leer
        </button>
      }
    >
      {error ? (
        <p className="text-sm text-mute">{error}</p>
      ) : !list ? (
        <p className="text-sm text-mute">Leyendo redes guardadas…</p>
      ) : list.length === 0 ? (
        <p className="text-sm text-mute">No hay redes Wi-Fi guardadas en este equipo.</p>
      ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-[11px] text-mute">
              <th className="pb-2 font-medium">Red</th>
              <th className="pb-2 font-medium">Seguridad</th>
              <th className="pb-2 font-medium">Contraseña</th>
              <th className="pb-2 font-medium">Conexión</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {list.map((p) => (
              <tr key={p.name} className="border-t border-line/60">
                <td className="py-2">
                  <span className="text-ink">{p.name}</span>
                  {p.ssid !== p.name && <span className="ml-1.5 text-xs text-mute">({p.ssid})</span>}
                  {p.connected && <span className="ml-2 rounded border border-ok/40 px-1.5 py-px text-[11px] text-ok">Conectada</span>}
                </td>
                <td className="py-2 text-xs text-dim">{SECURITY[p.authentication] ?? p.authentication}</td>
                <td className="py-2">
                  {p.password ? (
                    <span className="inline-flex items-center gap-2 font-mono text-xs">
                      <span className="select-text">{shown.has(p.name) ? p.password : "••••••••••"}</span>
                      <button onClick={() => toggle(p.name)} className="text-mute hover:text-ink" title={shown.has(p.name) ? "Ocultar" : "Mostrar"}>
                        {shown.has(p.name) ? <EyeOff size={12} /> : <Eye size={12} />}
                      </button>
                      <button
                        onClick={() => navigator.clipboard.writeText(p.password!).then(() => toast("ok", "Contraseña copiada."))}
                        className="text-mute hover:text-ink"
                        title="Copiar"
                      >
                        <Copy size={12} />
                      </button>
                    </span>
                  ) : p.protected ? (
                    <span className="inline-flex items-center gap-1 text-xs text-mute" title="Windows solo muestra la clave a un administrador">
                      <Lock size={11} /> {isAdmin ? "Protegida" : "Requiere administrador"}
                    </span>
                  ) : (
                    <span className="text-xs text-mute">{p.authentication === "open" ? "Sin contraseña" : "Red de empresa (usuario y contraseña)"}</span>
                  )}
                </td>
                <td className="py-2 text-xs text-dim">{p.autoConnect ? "Automática" : "Manual"}</td>
                <td className="py-2 text-right">
                  <button
                    onClick={() => forget(p)}
                    disabled={!isAdmin}
                    title={isAdmin ? "Olvidar esta red" : "Requiere administrador"}
                    className="rounded p-1 text-mute transition-colors hover:text-bad disabled:opacity-30"
                  >
                    <Trash2 size={13} />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {dialog}
    </Card>
  );
}
