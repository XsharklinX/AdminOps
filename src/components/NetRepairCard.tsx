import { ArrowRight, CheckCircle2, Loader2, Wrench, XCircle } from "lucide-react";
import { useState } from "react";
import { useConfirm, useToast } from "./feedback";
import { Button, Card } from "./ui";
import { troubleshootApi, type NetCheck, type NetRepair } from "../lib/api";

const ROWS: { label: string; get: (c: NetCheck) => boolean | null }[] = [
  { label: "Conectado con IP válida", get: (c) => c.connected },
  { label: "El router responde", get: (c) => c.gateway },
  { label: "Salida a Internet", get: (c) => c.internet },
  { label: "Resuelve nombres (DNS)", get: (c) => c.dns },
];

function Mark({ v }: { v: boolean | null }) {
  if (v === null) return <span className="text-mute">—</span>;
  return v ? <CheckCircle2 size={14} className="text-ok" /> : <XCircle size={14} className="text-bad" />;
}

/** Reparación de red en un clic, con el estado antes y después. */
export function NetRepairCard({ isAdmin }: { isAdmin: boolean }) {
  const [busy, setBusy] = useState<"quick" | "deep" | null>(null);
  const [result, setResult] = useState<NetRepair | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const run = async (deep: boolean) => {
    const ok = await confirm({
      title: deep ? "Reparar la red a fondo" : "Reparar la red",
      body: deep
        ? "Vacía la caché de DNS, restablece Winsock y la pila TCP/IP, reinicia los adaptadores y renueva la IP. Se pierde la IP fija si la hay y hay que reiniciar el equipo. Si estás conectado en remoto, la sesión se caerá."
        : "Vacía la caché de DNS, reinicia los adaptadores de red y renueva la IP. La conexión se corta unos segundos: si estás conectado en remoto, la sesión puede caerse.",
      confirmLabel: "Reparar",
      danger: deep,
    });
    if (!ok) return;
    setBusy(deep ? "deep" : "quick");
    setResult(null);
    try {
      const r = await troubleshootApi.repairNetwork(deep);
      setResult(r);
      const fine = r.after.internet && r.after.dns;
      toast(fine ? "ok" : "info", fine ? (r.reboot ? "Red reparada. Reinicia para completar." : "Red reparada: hay Internet.") : "Hecho, pero sigue sin Internet: reinicia el router o prueba la reparación a fondo.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Card title="Reparar la red" icon={<Wrench size={14} />} className="col-span-12">
      <p className="mb-3 text-xs text-dim">
        Para «conectado sin Internet», webs que no cargan o una IP 169.254. Comprueba la conexión antes y después para ver si se arregló.
      </p>
      <div className="flex flex-wrap gap-2">
        <Button onClick={() => run(false)} disabled={!!busy || !isAdmin} title={isAdmin ? undefined : "Requiere ejecutar AdminOps como administrador"}>
          {busy === "quick" ? <Loader2 size={13} className="animate-spin" /> : <Wrench size={13} />} Reparación rápida
        </Button>
        <Button kind="ghost" onClick={() => run(true)} disabled={!!busy || !isAdmin} title={isAdmin ? "Además restablece Winsock y TCP/IP (requiere reiniciar)" : "Requiere ejecutar AdminOps como administrador"}>
          {busy === "deep" ? <Loader2 size={13} className="animate-spin" /> : <Wrench size={13} />} Reparación a fondo
        </Button>
      </div>
      {busy && <p className="mt-3 text-xs text-mute">Reparando y esperando a que vuelva la conexión (hasta medio minuto)…</p>}
      {result && (
        <div className="mt-4 grid gap-4 md:grid-cols-2">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-[11px] text-mute">
                <th className="pb-1.5 font-medium" />
                <th className="pb-1.5 text-center font-medium">Antes</th>
                <th className="pb-1.5 font-medium" />
                <th className="pb-1.5 text-center font-medium">Después</th>
              </tr>
            </thead>
            <tbody>
              {ROWS.map((r) => (
                <tr key={r.label} className="border-t border-line/60">
                  <td className="py-1.5 text-xs text-dim">{r.label}</td>
                  <td className="py-1.5">
                    <span className="flex justify-center">
                      <Mark v={r.get(result.before)} />
                    </span>
                  </td>
                  <td className="py-1.5 text-mute">
                    <ArrowRight size={11} />
                  </td>
                  <td className="py-1.5">
                    <span className="flex justify-center">
                      <Mark v={r.get(result.after)} />
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <ul className="space-y-1 text-xs">
            {result.steps.map((s) => (
              <li key={s.title} className="flex items-start gap-2">
                {s.ok ? <CheckCircle2 size={12} className="mt-0.5 shrink-0 text-ok" /> : <XCircle size={12} className="mt-0.5 shrink-0 text-bad" />}
                <span>
                  <span className="text-ink">{s.title}</span>
                  {s.detail && <span className="block text-mute">{s.detail}</span>}
                </span>
              </li>
            ))}
            {result.reboot && <li className="pt-1 text-warn">Reinicia el equipo para completar el restablecimiento.</li>}
          </ul>
        </div>
      )}
      {dialog}
    </Card>
  );
}
