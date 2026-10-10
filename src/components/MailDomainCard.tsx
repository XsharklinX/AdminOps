// Correo que no sale o no llega: MX, SPF, DKIM, DMARC, puertos y listas negras
// del dominio de la empresa, con el texto listo para el proveedor.
import { ClipboardCopy, Mail, Search } from "lucide-react";
import { useState } from "react";
import { useToast } from "./feedback";
import { Button, Card, inputClass } from "./ui";
import { mailApi, type MailReport } from "../lib/api";
import { Term } from "./Term";

const TONE = { ok: "text-ok border-ok/30", info: "text-neon border-line", warn: "text-warn border-warn/40", bad: "text-bad border-bad/40" };

export function MailDomainCard() {
  const toast = useToast();
  const [domain, setDomain] = useState("");
  const [server, setServer] = useState("");
  const [r, setR] = useState<MailReport | null>(null);
  const [busy, setBusy] = useState(false);
  const run = async () => {
    setBusy(true);
    try {
      setR(await mailApi.check(domain, server.trim() || null));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Card title="Correo del dominio" icon={<Mail size={14} />}>
      <p className="mb-3 text-xs text-dim">
        Por qué los correos de la empresa acaban en spam o no llegan: registros del dominio (<Term k="SPF" />, <Term k="DKIM" />, <Term k="DMARC" />), servidores y puertos, y si la IP de la oficina está en una lista negra.
      </p>
      <form
        className="flex flex-wrap items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void run();
        }}
      >
        <input value={domain} onChange={(e) => setDomain(e.target.value)} placeholder="empresa.com (o un correo)" className={`${inputClass} w-56`} aria-label="Dominio del correo" />
        <input value={server} onChange={(e) => setServer(e.target.value)} placeholder="Servidor de correo (opcional)" className={`${inputClass} w-60`} aria-label="Servidor de correo" />
        <Button onClick={run} disabled={busy || domain.trim().length < 4}>
          <Search size={14} /> {busy ? "Revisando…" : "Revisar"}
        </Button>
      </form>
      {r && (
        <div className="mt-4 space-y-2">
          {r.lines.map((l, i) => (
            <div key={i} className={`rounded-lg border bg-void/30 px-3 py-2 ${TONE[l.level]}`}>
              <p className="text-xs font-semibold">{l.what}</p>
              <p className="text-xs text-dim">{l.text}</p>
            </div>
          ))}
          {r.ports.length > 0 && (
            <div className="overflow-x-auto rounded-lg border border-line">
              <table className="w-full text-xs">
                <tbody>
                  {r.ports.map((p) => (
                    <tr key={p.port} className="border-t border-line/60 first:border-t-0">
                      <td className="px-3 py-1.5 font-mono text-dim">
                        {p.host}:{p.port}
                      </td>
                      <td className="px-3 py-1.5 text-dim">{p.label}</td>
                      <td className={`px-3 py-1.5 text-right ${p.ok ? "text-ok" : "text-mute"}`}>{p.ok ? `responde (${p.ms} ms)` : "no responde"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <div className="flex justify-end">
            <Button kind="secondary" size="sm" onClick={() => navigator.clipboard.writeText(r.providerText).then(() => toast("ok", "Texto copiado: pégalo en el correo al proveedor."))}>
              <ClipboardCopy size={13} /> Copiar el texto para el proveedor
            </Button>
          </div>
        </div>
      )}
    </Card>
  );
}
