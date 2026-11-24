import { ChevronDown, ChevronRight, Info, Loader2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { ViewCommand } from "../components/ActionCommand";
import { NeedsAdmin } from "../components/AdminBanner";
import { DataTable, type Column } from "../components/DataTable";
import { useToast } from "../components/feedback";
import { ProgramIcon } from "../components/ProgramIcon";
import { Button, ErrorState, Loading } from "../components/ui";
import { logQuietly, netUsageApi, toolsApi, tweaksApi, type UsageSnapshot } from "../lib/api";
import { goToPage } from "../lib/navigate";
import { buildView, destinationsText, freeUpText, lastSpeed, ONEDRIVE_KEY, pausedText, rateText, todayText, totalText, uploadedText, type Speed, type UsageRow } from "../lib/netusage";
import { usePageActive } from "../lib/pageActive";
import { AUDIT_TITLE, useAuditOn } from "../lib/useAudit";

const TONE_CLASS = { bad: "text-bad", ok: "text-ok", mute: "text-mute" } as const;

/** Red → Uso de la red: qué programa sube y baja cuánto, en vivo, y «Pausar 2 h» para OneDrive. */
export function NetUsage() {
  const [snap, setSnap] = useState<UsageSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  // `undefined`: aún leyendo el historial; `null`: no hay ninguna prueba de velocidad guardada.
  const [speed, setSpeed] = useState<Speed | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [showQuiet, setShowQuiet] = useState(false);
  // Reintentar tras un fallo al empezar a medir.
  const [tries, setTries] = useState(0);
  const toast = useToast();
  const active = usePageActive();
  const audit = useAuditOn();
  // La ruta de cada programa que se ha visto: el de una fila sin tráfico (OneDrive en pausa) conserva su icono.
  const paths = useRef(new Map<string, string>());

  // Mide solo mientras la pantalla se ve: al cambiar de pestaña o salir se para (y en Rust, si deja de preguntar, se suelta sola).
  useEffect(() => {
    if (!active) return;
    let alive = true;
    let inflight = false;
    const apply = (s: UsageSnapshot) => {
      if (!alive) return;
      for (const p of s.programs) if (p.path) paths.current.set(p.key, p.path);
      setSnap(s);
      setError(null);
    };
    const fail = (e: unknown) => alive && setError(String(e));
    netUsageApi.start().then(apply).catch(fail);
    const timer = window.setInterval(() => {
      if (inflight) return;
      inflight = true;
      netUsageApi
        .snapshot()
        // Si Rust soltó la medición (el equipo estuvo dormido, por ejemplo) y no hay un fallo de permisos, se vuelve a pedir.
        .then((s) => (alive && !s.running && !s.error ? netUsageApi.start() : s))
        .then(apply)
        .catch(fail)
        .finally(() => {
          inflight = false;
        });
    }, 1000);
    return () => {
      alive = false;
      window.clearInterval(timer);
      netUsageApi.stop().catch(logQuietly("NetUsage"));
    };
  }, [active, tries]);

  // La última prueba de velocidad guardada (se hizo en otra pestaña): se vuelve a leer al volver aquí.
  useEffect(() => {
    if (!active) return;
    let alive = true;
    toolsApi
      .speedHistory()
      .then((h) => alive && setSpeed(lastSpeed(h)))
      .catch((e) => {
        logQuietly("NetUsage")(e);
        if (alive) setSpeed(null);
      });
    return () => {
      alive = false;
    };
  }, [active]);

  const view = useMemo(() => (snap ? buildView(snap, speed ?? null) : null), [snap, speed]);

  if (error && !snap) return <ErrorState message={error} onRetry={() => { setError(null); setTries((t) => t + 1); }} where="Red · Uso de la red" />;
  if (!snap || !view) return <Loading text="Empezando a medir la red…" page />;

  const pause = async () => {
    if (busy) return;
    setBusy(true);
    try {
      toast("ok", (await netUsageApi.pauseOneDrive()).message);
      setSnap(await netUsageApi.snapshot());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  const resume = async (entryId: number) => {
    if (busy) return;
    setBusy(true);
    try {
      toast("ok", (await tweaksApi.revertEntry(entryId)).message);
      setSnap(await netUsageApi.snapshot());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const columns: Column<UsageRow>[] = [
    {
      id: "program",
      header: "Programa",
      sortBy: (r) => r.p.name,
      cell: (r) => {
        const sub = r.paused ? pausedText(r.paused.untilMs) : (uploadedText(r.p) ?? destinationsText(r.p));
        return (
          <div className="flex min-w-0 items-center gap-2.5">
            <ProgramIcon path={r.p.path ?? paths.current.get(r.p.key) ?? null} name={r.p.name} />
            <div className="min-w-0">
              <div className="truncate text-ink">{r.p.name}</div>
              {sub && (
                <div className="truncate text-xs text-dim" title={r.paused ? undefined : destinationsText(r.p)}>
                  {sub}
                </div>
              )}
            </div>
          </div>
        );
      },
    },
    {
      id: "now",
      header: "Ahora",
      sortBy: (r) => r.p.upMbps + r.p.downMbps,
      className: "whitespace-nowrap font-mono text-sm text-ink",
      cell: (r) => (r.paused ? "—" : rateText(r.p)),
    },
    {
      id: "today",
      header: <span title="Lo que AdminOps midió hoy mientras esta pantalla o el vigilante de la conexión estaban en marcha">Hoy</span>,
      sortBy: (r) => r.p.todayUp + r.p.todayDown,
      className: "whitespace-nowrap font-mono text-xs text-dim",
      cell: (r) => todayText(r.p),
    },
    {
      id: "state",
      header: "Estado",
      sortBy: (r) => r.state?.label ?? "",
      className: (r) => `whitespace-nowrap ${r.state ? TONE_CLASS[r.state.tone] : "text-mute"}`,
      cell: (r) => r.state?.label ?? "",
    },
    {
      id: "actions",
      header: "",
      align: "right",
      stopClick: true,
      cell: (r) => {
        if (r.p.key !== ONEDRIVE_KEY) return null;
        const lockTitle = audit ? AUDIT_TITLE : undefined;
        return (
          <div className="flex items-center justify-end gap-1">
            {r.paused ? (
              <Button kt="N" kind="secondary" size="sm" disabled={audit || busy} title={lockTitle} onClick={() => resume(r.paused!.entryId)}>
                Reanudar ahora
              </Button>
            ) : (
              <Button kt="P" kind={r.state?.tone === "bad" ? "primary" : "secondary"} size="sm" disabled={audit || busy} title={lockTitle ?? "Cierra OneDrive y lo vuelve a abrir solo a las 2 horas"} onClick={pause}>
                Pausar 2 h
              </Button>
            )}
            <ViewCommand id="onedrive.pause" icon />
          </div>
        );
      },
    },
  ];

  const anyPaused = view.rows.some((r) => r.paused);
  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-medium text-ink">Uso de la red</span> · En vivo · por programa
        </p>
        <span className="ml-auto font-mono text-xs text-dim">{snap.running ? totalText(snap) : ""}</span>
      </div>

      {snap.error?.kind === "needsAdmin" && <NeedsAdmin>{snap.error.message} Windows solo deja ver el tráfico de cada programa al administrador.</NeedsAdmin>}
      {snap.error?.kind === "failed" && (
        <p className="flex items-center gap-2 rounded-lg border border-bad/40 bg-bad/5 px-3 py-2 text-xs text-bad" role="alert">
          <Info size={14} className="shrink-0" /> {snap.error.message}
        </p>
      )}
      {snap.tcpOnly && (
        <p className="flex items-center gap-2 rounded-lg border border-warn/40 bg-warn/5 px-3 py-2 text-xs text-warn" role="note">
          <Info size={14} className="shrink-0" /> Solo se ve el tráfico TCP (el método de reserva de Windows): lo que va por UDP, como QUIC o las llamadas de Teams, no sale aquí.
        </p>
      )}
      {snap.lostEvents > 0 && (
        <p className="flex items-center gap-2 rounded-lg border border-warn/40 bg-warn/5 px-3 py-2 text-xs text-warn" role="note">
          <Info size={14} className="shrink-0" /> Windows no pudo entregar {snap.lostEvents.toLocaleString("es")} eventos de red: las cifras de esta pantalla se quedan algo cortas.
        </p>
      )}
      {speed === null && (
        <p className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg border border-line bg-panel px-3 py-2 text-xs text-dim" role="note">
          <Info size={14} className="shrink-0" />
          <span className="min-w-48 flex-1">Aún no hay una prueba de velocidad guardada: sin ella no se sabe cuándo un programa satura la línea.</span>
          <Button kt="T" kind="secondary" size="sm" onClick={() => goToPage("router", "speed")}>
            Hacer la prueba
          </Button>
        </p>
      )}
      {anyPaused && view.freeUpMbps !== null && speed && (
        <p className="rounded-lg border border-ok/40 bg-ok/5 px-3 py-2 text-sm text-ok" role="status">
          {freeUpText(view.freeUpMbps, speed)}
        </p>
      )}

      {snap.error ? null : snap.running && view.rows.length === 0 ? (
        <p className="flex items-center gap-2 rounded-lg border border-line bg-panel px-4 py-6 text-sm text-dim">
          <Loader2 size={14} className="animate-spin" /> Mirando la red… los programas aparecen aquí cuando mueven datos.
        </p>
      ) : (
        <DataTable
          padded
          live
          resizable="netusage"
          rows={view.rows}
          rowKey={(r) => r.p.key}
          columns={columns}
          rowClass={(r) => (r.state?.tone === "bad" ? "bg-bad/5" : "")}
          expanded={(r) =>
            r.bar && speed ? (
              <div>
                <div className="h-1.5 w-full overflow-hidden rounded-full bg-line" role="img" aria-label={`Usa el ${r.bar.pct} % de la ${r.bar.dir === "up" ? "subida" : "bajada"}`}>
                  <div className="h-full rounded-full bg-bad" style={{ width: `${r.bar.pct}%` }} />
                </div>
                <p className="mt-1 text-[11px] text-mute">
                  {r.bar.pct} % de la {r.bar.dir === "up" ? "subida" : "bajada"} de la línea (la última prueba midió {Math.round(r.bar.dir === "up" ? speed.upMbps : speed.downMbps)} Mbit/s).
                </p>
              </div>
            ) : null
          }
          empty="Ningún programa mueve datos ahora mismo."
        />
      )}

      {view.quiet.length > 0 && (
        <div>
          <button type="button" onClick={() => setShowQuiet((v) => !v)} className="flex items-center gap-1 text-xs text-dim hover:text-ink" aria-expanded={showQuiet}>
            {showQuiet ? <ChevronDown size={13} /> : <ChevronRight size={13} />} Sin tráfico ahora ({view.quiet.length})
          </button>
          {showQuiet && (
            <ul className="mt-2 divide-y divide-line/60 overflow-hidden rounded-lg border border-line/70 bg-panel text-sm">
              {view.quiet.map((p) => (
                <li key={p.key} className="flex items-center gap-2.5 px-3 py-1.5">
                  <ProgramIcon path={p.path ?? paths.current.get(p.key) ?? null} name={p.name} />
                  <span className="min-w-0 flex-1 truncate text-ink">{p.name}</span>
                  <span className="font-mono text-xs text-dim">{todayText(p)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      <p className="text-[11px] text-mute">«Hoy» es lo que AdminOps midió mientras esta pantalla o el vigilante de la conexión estaban en marcha, no todo lo que el equipo usó.</p>
    </div>
  );
}
