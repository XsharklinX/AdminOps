import { ArrowRight, CheckCircle2, CircleAlert, Gauge, Globe, Loader2, Router, ShieldOff, Wifi, Wrench, XCircle } from "lucide-react";
import { useState } from "react";
import { useConfirm, useToast } from "./feedback";
import { Button, Card } from "./ui";
import { goToPage } from "../lib/navigate";
import { troubleshootApi, type NetCheck, type NetRepair, type NetVerdict } from "../lib/api";

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

/** Lo que conviene hacer después, según dónde se haya cortado la cadena. */
const NEXT: Record<string, { label: string; icon: typeof Router; run: (deep: () => void) => void }> = {
  router: { label: "Abrir el panel del router", icon: Router, run: () => goToPage("router") },
  dns: { label: "Cambiar los DNS", icon: Globe, run: () => goToPage("nettools", "dns") },
  wifi: { label: "Estado de la Wi-Fi", icon: Wifi, run: () => goToPage("network") },
  proxy: { label: "Ver el proxy de Windows", icon: ShieldOff, run: () => goToPage("nettools", "dns") },
  speed: { label: "Probar la velocidad", icon: Gauge, run: () => goToPage("network") },
  deep: { label: "Reparación a fondo", icon: Wrench, run: (deep) => deep() },
};

const TONE = {
  ok: { box: "border-ok/40 bg-ok/10", text: "text-ok", Icon: CheckCircle2 },
  warn: { box: "border-warn/40 bg-warn/10", text: "text-warn", Icon: CircleAlert },
  bad: { box: "border-bad/40 bg-bad/10", text: "text-bad", Icon: XCircle },
} as const;

/**
 * El resultado en una frase: qué pasa y qué hacer. Marcar cuatro casillas sirve
 * para ver si mejoró; esto es lo que dice dónde está el problema, que es lo que
 * el técnico necesita cuando la reparación no lo arregla sola.
 */
function VerdictBox({ v, onDeep }: { v: NetVerdict; onDeep: () => void }) {
  const tone = TONE[v.level] ?? TONE.warn;
  const { Icon } = tone;
  return (
    <div className={`mt-4 rounded-lg border p-3 ${tone.box}`}>
      <div className={`flex items-center gap-2 text-sm font-medium ${tone.text}`}>
        <Icon size={15} className="shrink-0" /> {v.title}
      </div>
      <p className="mt-1.5 text-xs leading-relaxed text-dim">{v.text}</p>
      {v.next.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {v.next.map((id) => {
            const a = NEXT[id];
            if (!a) return null;
            const { icon: I } = a;
            return (
              <Button key={id} kind="ghost" onClick={() => a.run(onDeep)}>
                <I size={13} /> {a.label}
              </Button>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** Los datos de la conexión, para no tener que ir a buscarlos a otra página. */
function Details({ c }: { c: NetCheck }) {
  const rows: [string, string][] = [
    ["Adaptador", c.adapter ? `${c.adapter}${c.wifi ? " (Wi-Fi)" : ""}` : "—"],
    ["Dirección IP", c.ip ? `${c.ip}${c.dhcp ? " (la da el router)" : " (puesta a mano)"}` : "—"],
    ["Router", c.gatewayIp || "—"],
    ["DNS", c.dnsServers.length ? c.dnsServers.join(", ") : "—"],
  ];
  if (c.proxy) rows.push(["Proxy", c.proxy]);
  if (c.vpn.length) rows.push(["VPN activa", c.vpn.join(", ")]);
  return (
    <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
      {rows.map(([k, v]) => (
        <div key={k} className="contents">
          <dt className="text-mute">{k}</dt>
          <dd className="font-mono text-ink select-text">{v}</dd>
        </div>
      ))}
    </dl>
  );
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
      // El detalle va en el diagnóstico de abajo; el aviso solo dice cómo quedó.
      toast(r.verdict.level === "ok" ? "ok" : "info", r.verdict.title);
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
      {result && <VerdictBox v={result.verdict} onDeep={() => run(true)} />}
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
          <div className="md:col-span-2">
            <div className="text-[11px] text-mute">Cómo quedó la conexión</div>
            <Details c={result.after} />
          </div>
        </div>
      )}
      {dialog}
    </Card>
  );
}
