// «Revisar su equipo»: desde la ficha de una persona, lee a distancia el equipo
// donde tiene la sesión abierta y ofrece los arreglos típicos a un clic.
// Solo lee hasta que se pulsa un arreglo; cada arreglo queda en el Historial.
import { CheckCircle2, Loader2, ScanSearch, TriangleAlert, XCircle } from "lucide-react";
import { useState } from "react";
import { stationsApi, type ReviewIssue, type StationAction, type StationReview } from "../lib/api";
import { useConfirm, useToast } from "./feedback";
import { Button } from "./ui";

const FIX_LABEL: Partial<Record<StationAction, string>> = {
  freeSpace: "Liberar espacio",
  clearQueue: "Vaciar la cola",
  restart: "Reiniciar con aviso",
};

const TONE = { bad: "bg-bad", warn: "bg-warn", ok: "bg-ok" };

const days = (d: number | null | undefined) => (d == null ? "—" : `${Math.floor(d)} d`);

export function ReviewMachine({ host }: { host: string }) {
  const [busy, setBusy] = useState(false);
  const [fixing, setFixing] = useState<StationAction | null>(null);
  const [review, setReview] = useState<StationReview | null>(null);
  const [done, setDone] = useState<{ ok: boolean; text: string }[]>([]);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const look = async () => {
    setBusy(true);
    setDone([]);
    try {
      setReview(await stationsApi.review(host));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const fix = async (action: StationAction) => {
    if (action === "restart") {
      const go = await confirm({
        title: `¿Reiniciar ${host}?`,
        body: <p>Quien esté usando el equipo verá un aviso y tendrá 2 minutos para guardar su trabajo. Se puede cancelar desde Puestos con «Cancelar reinicio».</p>,
        confirmLabel: "Reiniciar con aviso",
        danger: true,
      });
      if (!go) return;
    }
    setFixing(action);
    try {
      const r = await stationsApi.act([host], action);
      setDone((d) => [...d, ...r.map((x) => ({ ok: x.ok, text: `${FIX_LABEL[action] ?? action}: ${x.detail}` }))]);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setFixing(null);
    }
  };

  const row = (i: ReviewIssue, k: number) => (
    <li key={k} className="flex items-center gap-2 py-1">
      <span className={`size-2 shrink-0 rounded-full ${TONE[i.level]}`} />
      <span className="min-w-0 flex-1 text-ink">{i.text}</span>
      {i.fix && FIX_LABEL[i.fix] && (
        <Button kind="ghost" onClick={() => void fix(i.fix as StationAction)} disabled={fixing !== null}>
          {fixing === i.fix && <Loader2 size={12} className="animate-spin" />} {FIX_LABEL[i.fix]}
        </Button>
      )}
    </li>
  );

  const r = review?.station.remote;
  return (
    <div className="mt-2 border-t border-line/70 pt-2 text-xs">
      <Button kind="ghost" onClick={() => void look()} disabled={busy}>
        {busy ? <Loader2 size={13} className="animate-spin" /> : <ScanSearch size={13} />} Revisar su equipo
      </Button>
      {busy && <span className="ml-2 text-mute">Leyendo {host} a distancia…</span>}
      {review && !busy && (
        <div className="mt-2 space-y-2">
          {r && (
            <dl className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-[11px] sm:grid-cols-5">
              <Reading k="Libre" v={r.freeGb != null && r.totalGb != null ? `${Math.round(r.freeGb)} de ${Math.round(r.totalGb)} GB` : "—"} />
              <Reading k="Memoria" v={review.memUsedPct != null ? `${review.memUsedPct} % en uso` : "—"} />
              <Reading k="Encendido" v={days(r.bootDays)} />
              <Reading k="Errores (2 d)" v={review.errors != null ? String(review.errors) : "—"} />
              <Reading k="Cola de impresión" v={review.printJobs != null ? `${review.printJobs} trabajo(s)` : "—"} />
            </dl>
          )}
          <ul>{review.issues.map(row)}</ul>
          {done.length > 0 && (
            <ul className="space-y-0.5 rounded-md border border-line bg-void/40 px-2 py-1.5">
              {done.map((d, i) => (
                <li key={i} className="flex gap-1.5">
                  {d.ok ? <CheckCircle2 size={12} className="mt-0.5 shrink-0 text-ok" /> : <XCircle size={12} className="mt-0.5 shrink-0 text-bad" />}
                  <span className="text-dim">{d.text}</span>
                </li>
              ))}
            </ul>
          )}
          <p className="flex items-center gap-1 text-[11px] text-mute">
            <TriangleAlert size={11} /> Lee con tus permisos; cada arreglo queda en el Historial y el modo auditoría lo bloquea.
          </p>
        </div>
      )}
      {dialog}
    </div>
  );
}

function Reading({ k, v }: { k: string; v: string }) {
  return (
    <div>
      <dt className="text-mute">{k}</dt>
      <dd className="font-mono text-ink">{v}</dd>
    </div>
  );
}
