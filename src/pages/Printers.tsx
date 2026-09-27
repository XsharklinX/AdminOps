import { CircleAlert, FileCheck2, Loader2, Printer, RefreshCw, Star, Trash2, Wrench, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button } from "../components/ui";
import { printersApi, tweaksApi, type PrinterInfo } from "../lib/api";

const STATUS: Record<number, string> = { 1: "Otro", 2: "Desconocido", 3: "Lista", 4: "Imprimiendo", 5: "Calentando", 6: "Detenida", 7: "Sin conexión" };

export function Printers({ isAdmin }: { isAdmin: boolean }) {
  const [list, setList] = useState<PrinterInfo[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setList(await printersApi.list());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    load();
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
    if (yes) run("spooler", "Cola de impresión reiniciada.", () => tweaksApi.run("repair.print-queue").then(() => {}));
  };

  const remove = async (p: PrinterInfo) => {
    const yes = await confirm({
      title: `Quitar ${p.name}`,
      body: "Se eliminará la impresora de Windows (no desinstala su driver). Si es de verdad, se puede volver a añadir desde Configuración → Impresoras.",
      confirmLabel: "Quitar",
      danger: true,
    });
    if (yes) run(p.name, `${p.name} quitada.`, () => printersApi.remove(p.name));
  };

  if (!list) return <p className="p-8 font-mono text-sm text-mute">Leyendo impresoras…</p>;
  const real = list.filter((p) => !p.virtual);
  const virtual = list.filter((p) => p.virtual);

  const row = (p: PrinterInfo, i: number) => {
    const working = busy === p.name;
    const problem = p.error ?? (p.offline ? "Sin conexión" : null);
    return (
      <div key={p.name} className={`flex items-center gap-4 px-4 py-3 ${i ? "border-t border-line/70" : ""}`}>
        <Printer size={18} className={problem ? "text-warn" : "text-neon"} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-sm font-medium text-ink">{p.name}</span>
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
        <div className={`w-24 text-right text-xs ${p.jobs ? "text-warn" : "text-mute"}`}>
          {p.jobs ? `${p.jobs} en cola` : "Cola vacía"}
        </div>
        {working ? (
          <Loader2 size={16} className="animate-spin text-neon" />
        ) : (
          <div className="flex shrink-0 items-center gap-0.5">
            <IconBtn
              title="Vaciar la cola de esta impresora"
              disabled={!p.jobs || busy !== null}
              onClick={() => run(p.name, `Cola de ${p.name} vaciada.`, () => printersApi.clearQueue(p.name))}
            >
              <XCircle size={15} />
            </IconBtn>
            <IconBtn title="Imprimir página de prueba" disabled={busy !== null} onClick={() => run(p.name, `Página de prueba enviada a ${p.name}.`, () => printersApi.testPage(p.name))}>
              <FileCheck2 size={15} />
            </IconBtn>
            <IconBtn
              title="Hacer predeterminada (desactiva «Permitir que Windows administre la impresora predeterminada»)"
              disabled={p.default || busy !== null}
              onClick={() => run(p.name, `${p.name} es ahora la predeterminada.`, () => printersApi.setDefault(p.name))}
            >
              <Star size={15} />
            </IconBtn>
            <IconBtn title={isAdmin ? "Quitar impresora" : "Quitar impresora: requiere administrador"} danger disabled={!isAdmin || busy !== null} onClick={() => remove(p)}>
              <Trash2 size={15} />
            </IconBtn>
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="mx-auto max-w-5xl p-6">
      <div className="mb-4 flex items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{real.length}</span> {real.length === 1 ? "impresora" : "impresoras"}
          {real.some((p) => p.jobs) && <span className="text-warn"> · hay trabajos en cola</span>}
        </p>
        <div className="ml-auto flex items-center gap-2">
          <Button kind="ghost" onClick={resetSpooler} disabled={!isAdmin || busy !== null} title={isAdmin ? undefined : "Requiere administrador"}>
            {busy === "spooler" ? <Loader2 size={14} className="animate-spin" /> : <Wrench size={14} />} Reiniciar cola de impresión
          </Button>
          <button onClick={load} disabled={loading} className="rounded-md p-1.5 text-dim hover:bg-panel-2 hover:text-ink" title="Volver a leer">
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          </button>
        </div>
      </div>

      <div className="overflow-hidden rounded-xl border border-line bg-panel">
        {real.length ? real.map(row) : <p className="px-4 py-8 text-center text-sm text-mute">No hay impresoras físicas instaladas.</p>}
      </div>

      {virtual.length > 0 && (
        <>
          <h2 className="mt-5 mb-2 text-[11px] font-semibold text-dim">Impresoras virtuales</h2>
          <div className="overflow-hidden rounded-xl border border-line bg-panel opacity-80">{virtual.map(row)}</div>
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

function IconBtn({ children, title, disabled, danger, onClick }: { children: React.ReactNode; title: string; disabled?: boolean; danger?: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`rounded-md p-2 text-dim transition-colors disabled:opacity-25 ${danger ? "hover:bg-bad/10 hover:text-bad" : "hover:bg-panel-2 hover:text-neon"}`}
    >
      {children}
    </button>
  );
}
