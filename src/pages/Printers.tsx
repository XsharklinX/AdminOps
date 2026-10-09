import { CheckCircle2, CircleAlert, ExternalLink, FileCheck2, Loader2, Printer, RefreshCw, Search, Star, Stethoscope, Trash2, Wrench, X, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, ErrorState, IconButton, Loading, iconBtn } from "../components/ui";
import { DataTable, type Column } from "../components/DataTable";
import { printersApi, tweaksApi, type FoundPrinter, type PrinterCheck, type PrinterInfo, type PrinterSupply } from "../lib/api";

const STATUS: Record<number, string> = { 1: "Otro", 2: "Desconocido", 3: "Lista", 4: "Imprimiendo", 5: "Calentando", 6: "Detenida", 7: "Sin conexión" };

export function Printers({ isAdmin }: { isAdmin: boolean }) {
  const [list, setList] = useState<PrinterInfo[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  // Resultado de «Revisar» por impresora.
  const [checks, setChecks] = useState<Record<string, PrinterCheck>>({});
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const [failed, setFailed] = useState<string | null>(null);
  const load = useCallback(async () => {
    setFailed(null);
    setLoading(true);
    try {
      setList(await printersApi.list());
    } catch (e) {
      setFailed(String(e));
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  const run = async (key: string, ok: string, op: () => Promise<void>) => {
    setBusy(key);
    try {
      await op();
      toast("ok", ok);
      await load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const resetSpooler = async () => {
    const yes = await confirm({
      title: "Reiniciar la cola de impresión",
      body: "Detiene el servicio de impresión, borra TODOS los trabajos atascados de todas las impresoras y lo vuelve a arrancar. Úsalo cuando vaciar la cola de una impresora no basta.",
      confirmLabel: "Reiniciar cola",
      danger: true,
    });
    if (yes) void run("spooler", "Cola de impresión reiniciada.", () => tweaksApi.run("repair.print-queue").then(() => {}));
  };

  /** Revisa una impresora y deja el resultado bajo su fila. */
  const check = async (p: PrinterInfo) => {
    setBusy(p.name);
    try {
      const c = await printersApi.check(p.name);
      setChecks((m) => ({ ...m, [p.name]: c }));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const remove = async (p: PrinterInfo) => {
    const yes = await confirm({
      title: `Quitar ${p.name}`,
      body: "Se eliminará la impresora de Windows (no desinstala su driver). Si es de verdad, se puede volver a añadir desde Configuración → Impresoras.",
      confirmLabel: "Quitar",
      danger: true,
    });
    if (yes) void run(p.name, `${p.name} quitada.`, () => printersApi.remove(p.name));
  };

  if (!list) return failed ? <ErrorState page message={failed} onRetry={() => void load()} /> : <Loading page text="Leyendo impresoras…" />;
  const real = list.filter((p) => !p.virtual);
  const virtual = list.filter((p) => p.virtual);

  const columns: Column<PrinterInfo>[] = [
    {
      id: "name",
      header: "Impresora",
      sortBy: (p) => p.name,
      className: "max-w-0 w-full",
      cell: (p) => {
        const problem = p.error ?? (p.offline ? "Sin conexión" : null);
        return (
          <div className="flex items-center gap-3 py-1">
            <Printer size={18} className={`shrink-0 ${problem ? "text-warn" : "text-neon"}`} />
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="font-medium text-ink">{p.name}</span>
                {p.default && (
                  <span className="flex items-center gap-1 rounded border border-neon/40 px-1.5 py-px text-[11px] text-neon">
                    <Star size={9} fill="currentColor" /> Predeterminada
                  </span>
                )}
                {p.network && <span className="rounded border border-line px-1.5 py-px text-[11px] text-mute">Red</span>}
                {p.shared && <span className="rounded border border-line px-1.5 py-px text-[11px] text-mute">Compartida</span>}
                {problem && (
                  <span className="flex items-center gap-1 rounded border border-warn/40 px-1.5 py-px text-[11px] text-warn">
                    <CircleAlert size={9} /> {problem}
                  </span>
                )}
              </div>
              <div className="truncate text-[11px] text-mute">
                {STATUS[p.status] ?? "—"} · {p.driver ?? "sin driver"} · {p.port ?? "sin puerto"}
              </div>
            </div>
          </div>
        );
      },
    },
    {
      id: "jobs",
      header: "Cola",
      align: "right",
      sortBy: (p) => p.jobs,
      className: (p) => `whitespace-nowrap text-xs ${p.jobs ? "text-warn" : "text-mute"}`,
      cell: (p) => (p.jobs ? `${p.jobs} en cola` : "Vacía"),
    },
    {
      id: "actions",
      header: "",
      align: "right",
      stopClick: true,
      cell: (p) =>
        busy === p.name ? (
          <Loader2 size={16} className="ml-auto animate-spin text-neon" />
        ) : (
          <div className="flex items-center justify-end gap-0.5">
            <IconButton label="Vaciar la cola de esta impresora" disabled={!p.jobs || busy !== null} onClick={() => void run(p.name, `Cola de ${p.name} vaciada.`, () => printersApi.clearQueue(p.name))}>
              <XCircle size={15} />
            </IconButton>
            <IconButton label="Revisar: por qué no imprime y qué hacer" disabled={busy !== null} onClick={() => void check(p)}>
              <Stethoscope size={15} />
            </IconButton>
            <IconButton label="Imprimir página de prueba" disabled={busy !== null} onClick={() => void run(p.name, `Página de prueba enviada a ${p.name}.`, () => printersApi.testPage(p.name))}>
              <FileCheck2 size={15} />
            </IconButton>
            <IconButton
              label="Hacer predeterminada (desactiva «Permitir que Windows administre la impresora predeterminada»)"
              disabled={p.default || busy !== null}
              onClick={() => void run(p.name, `${p.name} es ahora la predeterminada.`, () => printersApi.setDefault(p.name))}
            >
              <Star size={15} />
            </IconButton>
            <IconButton label={isAdmin ? "Quitar impresora" : "Quitar impresora: requiere administrador"} danger disabled={!isAdmin || busy !== null} onClick={() => void remove(p)}>
              <Trash2 size={15} />
            </IconButton>
          </div>
        ),
    },
  ];
  const table = (rows: PrinterInfo[], empty?: string) => (
    <DataTable
      padded
      rows={rows}
      rowKey={(p) => p.name}
      columns={columns}
      empty={empty}
      expanded={(p) => (checks[p.name] ? <CheckBox c={checks[p.name]} onClose={() => setChecks(({ [p.name]: _quitada, ...resto }) => resto)} /> : null)}
    />
  );

  return (
    <div className="mx-auto max-w-(--page-max) p-6">
      <div className="mb-4 flex items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{real.length}</span> {real.length === 1 ? "impresora" : "impresoras"}
          {real.some((p) => p.jobs) && <span className="text-warn"> · hay trabajos en cola</span>}
        </p>
        <div className="ml-auto flex items-center gap-2">
          <Button kind="ghost" onClick={resetSpooler} disabled={!isAdmin || busy !== null} title={isAdmin ? undefined : "Requiere administrador"}>
            {busy === "spooler" ? <Loader2 size={14} className="animate-spin" /> : <Wrench size={14} />} Reiniciar cola de impresión
          </Button>
          <button onClick={() => void load()} disabled={loading} className={iconBtn} title="Volver a leer" aria-label="Volver a leer">
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          </button>
        </div>
      </div>

      <div className="overflow-x-auto rounded-xl border border-line bg-panel">{table(real, "No hay impresoras físicas instaladas.")}</div>

      <NetworkPrinters isAdmin={isAdmin} />

      {virtual.length > 0 && (
        <>
          <h2 className="mt-5 mb-2 text-[11px] font-semibold text-dim">Impresoras virtuales</h2>
          <div className="overflow-x-auto rounded-xl border border-line bg-panel opacity-80">{table(virtual)}</div>
        </>
      )}
      <p className="mt-3 text-xs text-mute">
        ¿Impresora que ya no existe en la lista? Quítala: las «impresoras fantasma» de equipos antiguos o drivers viejos confunden al usuario y a
        algunos programas.
      </p>
      {dialog}
    </div>
  );
}

/** El resultado de revisar una impresora: qué le pasa y qué hacer. */
function CheckBox({ c, onClose }: { c: PrinterCheck; onClose: () => void }) {
  const tono =
    c.level === "ok"
      ? { box: "border-ok/40 bg-ok/10", text: "text-ok", Icon: CheckCircle2 }
      : c.level === "warn"
        ? { box: "border-warn/40 bg-warn/10", text: "text-warn", Icon: CircleAlert }
        : { box: "border-bad/40 bg-bad/10", text: "text-bad", Icon: XCircle };
  const { Icon } = tono;
  return (
    <div className={`rounded-lg border p-3 ${tono.box}`}>
      <div className="flex items-start gap-2">
        <Icon size={15} className={`mt-0.5 shrink-0 ${tono.text}`} />
        <div className="min-w-0 flex-1">
          <div className={`text-sm font-medium ${tono.text}`}>{c.title}</div>
          <p className="mt-1 text-xs leading-relaxed text-dim">{c.text}</p>
          <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-mute">
            <span>Cola de Windows: {c.spooler ? "en marcha" : "parada"}</span>
            {c.host && (
              <span>
                Dirección: <span className="font-mono text-dim select-text">{c.host}</span>
                {c.reachable !== null && ` · ${c.reachable ? "responde" : "no responde"}`}
              </span>
            )}
            {c.jobs > 0 && <span>{c.jobs} en cola, el más viejo de hace {c.oldestJobMin} min</span>}
            {c.device?.model && <span>{c.device.model}</span>}
            {c.device?.pages != null && <span>{c.device.pages.toLocaleString("es")} páginas impresas</span>}
          </div>
          {c.device && c.device.supplies.length > 0 && <Supplies supplies={c.device.supplies} />}
        </div>
        <button onClick={onClose} className="shrink-0 text-mute hover:text-ink" title="Cerrar">
          <X size={14} />
        </button>
      </div>
    </div>
  );
}

/** Niveles de tóner, tinta y demás consumibles, como los cuenta la impresora. */
function Supplies({ supplies }: { supplies: PrinterSupply[] }) {
  // Primero lo que se cambia a menudo (tóner, tinta); luego fusor, tambor, etc.
  const orden = [...supplies].sort((a, b) => Number(b.consumable) - Number(a.consumable));
  return (
    <ul className="mt-2 grid gap-x-4 gap-y-1 sm:grid-cols-2">
      {orden.map((s, i) => {
        const pct = s.percent;
        const color = pct === null ? "bg-mute" : pct <= 10 ? "bg-bad" : pct <= 25 ? "bg-warn" : "bg-ok";
        return (
          <li key={`${s.name}-${i}`} className="flex items-center gap-2 text-[11px]">
            <span className="min-w-0 flex-1 truncate text-dim" title={s.name}>
              {s.name}
            </span>
            <span className="h-1.5 w-16 shrink-0 overflow-hidden rounded-full bg-line">
              {pct !== null && <span className={`block h-full ${color}`} style={{ width: `${pct}%` }} />}
            </span>
            <span className="w-9 shrink-0 text-right font-mono text-mute">{pct === null ? "—" : `${pct} %`}</span>
          </li>
        );
      })}
    </ul>
  );
}

/**
 * Impresoras que hay en la red y no están instaladas aquí.
 *
 * Hasta ahora, saber qué impresoras hay en una oficina era preguntar o ir
 * mirando aparato por aparato.
 */
function NetworkPrinters({ isAdmin }: { isAdmin: boolean }) {
  const [found, setFound] = useState<FoundPrinter[] | null>(null);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const buscar = async () => {
    setBusy(true);
    try {
      setFound(await printersApi.findOnNetwork());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const nuevas = (found ?? []).filter((f) => !f.installed);
  return (
    <section className="mt-4 rounded-xl border border-line bg-panel">
      <div className="flex flex-wrap items-center gap-3 px-4 py-3">
        <Search size={16} className="text-neon" />
        <div className="min-w-0 flex-1">
          <div className="text-sm text-ink">Buscar impresoras en la red</div>
          <div className="text-[11px] text-mute">Pregunta en la red quién es impresora (mDNS y WS-Discovery) y prueba los puertos de impresión de los equipos que ya responden. No instala nada.</div>
        </div>
        <Button kind="ghost" onClick={buscar} disabled={busy}>
          {busy ? <Loader2 size={13} className="animate-spin" /> : <Search size={13} />} {busy ? "Buscando…" : "Buscar"}
        </Button>
      </div>
      {found !== null && !busy && (
        <div className="border-t border-line/70 px-4 py-3">
          {found.length === 0 ? (
            <p className="text-xs text-mute">No se ha visto ninguna impresora en esta red. Enciéndelas y vuelve a buscar.</p>
          ) : (
            <>
              <p className="mb-2 text-xs text-dim">
                {nuevas.length > 0 ? `${nuevas.length} sin instalar en este equipo` : "Todas las que se ven ya están instaladas aquí"}
              </p>
              <ul className="space-y-1">
                {found.map((f) => (
                  <li key={f.ip} className="flex items-center gap-3 text-xs">
                    <span className="w-32 shrink-0 font-mono text-ink select-text">{f.ip}</span>
                    <span className="min-w-0 flex-1 truncate" title={[f.name, f.model].filter(Boolean).join(" · ")}>
                      {f.name || f.model ? <span className="text-dim">{[f.name, f.model !== f.name ? f.model : ""].filter(Boolean).join(" · ")}</span> : null}
                      <span className="ml-2 text-mute">{f.ports.map((p) => (p === 9100 ? "RAW" : p === 631 ? "IPP" : "LPD")).join(" · ") || (f.via === "wsd" ? "WS-Discovery" : "mDNS")}</span>
                    </span>
                    {f.installed ? (
                      <span className="text-mute">ya instalada</span>
                    ) : (
                      <button
                        onClick={() => navigator.clipboard.writeText(f.ip).then(() => toast("ok", "Dirección copiada: pégala en el asistente de Windows."))}
                        className="text-neon hover:underline"
                      >
                        Copiar dirección
                      </button>
                    )}
                  </li>
                ))}
              </ul>
              {nuevas.length > 0 && (
                <div className="mt-3">
                  <Button kind="ghost" onClick={() => tweaksApi.run("open:ms-settings:printers").catch((e) => toast("error", String(e)))} disabled={!isAdmin}>
                    <ExternalLink size={13} /> Abrir «Añadir impresora» de Windows
                  </Button>
                </div>
              )}
            </>
          )}
        </div>
      )}
    </section>
  );
}
