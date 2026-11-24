// Escáneres RICOH (fi, ScanSnap, antes Fujitsu) y EPSON: qué driver tienen, si
// está el programa con el que se escanea, instalarlo desde la biblioteca y
// «Probar escaneo», que pide una página por WIA y la enseña.
import { CheckCircle2, CircleAlert, Download, Loader2, RefreshCw, ScanLine } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { ScanToFolder } from "../components/ScanToFolder";
import { ActionError, BrandBadge, DriverChip } from "../components/printers/Parts";
import { describeDriver, installerFor, missingSoftware, VENDOR_NAME } from "../components/printers/logic";
import { Button, EmptyState, ErrorState, iconBtn } from "../components/ui";
import { goToPage } from "../lib/navigate";
import { vendorApi, type DriverLibrary, type VendorScanner, type VendorScannerInventory } from "../lib/api";

export function Scanners({ isAdmin }: { isAdmin: boolean }) {
  const toast = useToast();
  const [inv, setInv] = useState<VendorScannerInventory | null>(null);
  const [lib, setLib] = useState<DriverLibrary | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [shots, setShots] = useState<Record<string, { dataUrl: string; bytes: number }>>({});
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [lastOkAt, setLastOkAt] = useState<number | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setInv(await vendorApi.scanners());
      setFailed(null);
      setLastOkAt(Date.now());
      // La biblioteca solo hace falta para los instaladores; si falla, no estorba.
      setLib(await vendorApi.library(false).catch(() => null));
    } catch (e) {
      setFailed(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const act = async (key: string, op: () => Promise<void>) => {
    setBusy(key);
    setErrors((m) => {
      const { [key]: _quitado, ...resto } = m;
      return resto;
    });
    try {
      await op();
    } catch (e) {
      setErrors((m) => ({ ...m, [key]: String(e) }));
    } finally {
      setBusy(null);
    }
  };

  const test = (s: VendorScanner) =>
    act(`test-${s.name}`, async () => {
      const r = await vendorApi.scanTest(s.name);
      setShots((m) => ({ ...m, [s.name]: r }));
    });

  if (!inv)
    return (
      <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
        <div className="flex items-center gap-3">
          <p className="text-sm text-dim">{failed ? "No se pudo leer la lista de escáneres" : "Buscando escáneres…"}</p>
        </div>
        {/* El asistente no depende de la lista de escáneres: se ve aunque falle o tarde (y en la misma posición que abajo, para no perder su estado). */}
        <ScanToFolder isAdmin={isAdmin} />
        {failed && <ErrorState message={failed} onRetry={() => void load()} />}
      </div>
    );

  const vendors = [...inv.scanners.map((s) => s.vendor), ...inv.notInstalled.map((d) => d.vendor)];
  const missing = missingSoftware(inv.software, vendors);
  const installers = lib?.installers ?? [];

  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{inv.scanners.length}</span> {inv.scanners.length === 1 ? "escáner" : "escáneres"} RICOH, PFU o EPSON
        </p>
        <button onClick={() => void load()} disabled={loading} className={`${iconBtn} ml-auto`} title="Volver a leer" aria-label="Volver a leer">
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      <ScanToFolder isAdmin={isAdmin} />

      {inv.scanners.length === 0 && inv.notInstalled.length === 0 && (
        <EmptyState icon={<ScanLine size={28} />} title="No hay escáneres RICOH, PFU ni EPSON conectados">
          Enciende el escáner, conéctalo por USB y pulsa «Volver a leer». Los que Windows ve sin driver también salen aquí.
        </EmptyState>
      )}

      {inv.notInstalled.length > 0 && (
        <ul className="divide-y divide-warn/20 overflow-hidden rounded-2xl border border-warn/40 bg-warn/5 shadow-elev-1">
          {inv.notInstalled.map((d) => (
            <li key={d.instanceId || d.name} className="flex flex-wrap items-center gap-2 px-4 py-3 text-sm text-ink">
              <CircleAlert size={16} className="shrink-0 text-warn" />
              Escáner conectado sin driver: <BrandBadge vendor={d.vendor} /> <span className="font-medium">{d.model || d.name}</span>
              <button type="button" onClick={() => goToPage("drivers")} className="ml-auto text-xs text-neon hover:underline">
                Buscar el driver en Drivers
              </button>
            </li>
          ))}
        </ul>
      )}

      {failed && inv && lastOkAt && (
        <p className="mb-3 flex flex-wrap items-center gap-2 text-xs text-warn">
          <CircleAlert size={13} /> No se pudo actualizar: {failed}, se enseña la lectura de las{" "}
          {new Date(lastOkAt).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" })}
          <button type="button" onClick={() => void load()} className="text-neon hover:underline">
            Reintentar
          </button>
        </p>
      )}
      {inv.scanners.length > 0 && (
        <ul className="divide-y divide-line/60 overflow-hidden rounded-2xl border border-line/70 bg-panel shadow-elev-1">
          {inv.scanners.map((s) => {
            const key = `test-${s.name}`;
            const works = s.health === "funciona";
            return (
              <li key={s.instanceId || s.name} className="px-4 py-3">
                <div className="flex flex-wrap items-center gap-3">
                  <ScanLine size={18} className={`shrink-0 ${works ? "text-neon" : "text-warn"}`} />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-1.5">
                      <BrandBadge vendor={s.vendor} />
                      <span className="font-medium text-ink">{s.model || s.name}</span>
                    </div>
                    <div className="text-[11px] text-mute">
                      {[s.driver ? describeDriver(s.driver.name, s.driver.version) : "sin driver", s.interfaces.join(" · "), works ? "responde" : s.problem ? `Windows da el error ${s.problem}` : "no responde"].filter(Boolean).join(" · ")}
                    </div>
                  </div>
                  <DriverChip {...s.state} />
                  <Button kind="secondary" size="sm" onClick={() => test(s)} disabled={!works || busy !== null} title={works ? "Pide una página de prueba al escáner" : "El escáner no responde"}>
                    {busy === key ? <Loader2 size={13} className="animate-spin" /> : <ScanLine size={13} />} {busy === key ? "Escaneando…" : "Probar escaneo"}
                  </Button>
                </div>
                {busy === key && <p className="mt-2 ml-8 text-[11px] text-mute">Pon una hoja en el escáner. Si no responde en un minuto, se avisa.</p>}
                {errors[key] && <ActionError className="mt-2 ml-8" message={errors[key]} />}
                {shots[s.name] && (
                  <div className="mt-3 ml-8">
                    <p className="mb-1.5 flex items-center gap-1.5 text-xs text-ok">
                      <CheckCircle2 size={13} /> Página recibida: el escáner funciona. Si no escanea desde un programa, el fallo está en el programa.
                    </p>
                    <img src={shots[s.name].dataUrl} alt={`Escaneo de prueba de ${s.model || s.name}`} className="max-h-72 rounded-lg border border-line bg-white" />
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}

      {/* Programas con los que se escanea */}
      <section className="rounded-2xl border border-line/70 bg-panel shadow-elev-1">
        <h3 className="border-b border-line/70 px-4 py-2.5 text-[11px] font-semibold text-dim">Programas de escaneo</h3>
        <ul className="divide-y divide-line/60">
          {inv.software.map((sw) => {
            const key = `soft-${sw.id}`;
            const inst = !sw.installed ? installerFor(sw, installers) : null;
            const needed = missing.some((m) => m.id === sw.id);
            return (
              <li key={sw.id} className="px-4 py-2.5">
                <div className="flex flex-wrap items-center gap-2 text-xs">
                  {sw.installed ? <CheckCircle2 size={14} className="shrink-0 text-ok" /> : <CircleAlert size={14} className={`shrink-0 ${needed ? "text-warn" : "text-mute"}`} />}
                  <span className="min-w-0 flex-1 text-ink">
                    {sw.name} <span className="text-mute">· {VENDOR_NAME[sw.vendor as keyof typeof VENDOR_NAME] || sw.vendor}</span>
                    <span className="ml-1 text-mute">{sw.installed ? `instalado${sw.version ? ` ${sw.version}` : ""}` : needed ? "falta" : "no instalado"}</span>
                  </span>
                  {!sw.installed &&
                    (inst ? (
                      <Button
                        size="sm"
                        kind={needed ? "primary" : "secondary"}
                        disabled={busy !== null}
                        title="Abre el instalador de tu biblioteca"
                        onClick={() =>
                          act(key, async () => {
                            const r = await vendorApi.runInstaller(inst.id);
                            setNotes((m) => ({ ...m, [key]: r.message }));
                            toast(r.finished ? "ok" : "info", r.message);
                            await load();
                          })
                        }
                      >
                        {busy === key ? <Loader2 size={13} className="animate-spin" /> : <Download size={13} />} Instalar {sw.name}
                      </Button>
                    ) : needed ? (
                      <button type="button" onClick={() => goToPage("drivers")} className="text-neon hover:underline">
                        No está en la biblioteca: añadirlo en Drivers
                      </button>
                    ) : null)}
                </div>
                {!sw.installed && inst && <p className="mt-1 ml-6 text-[11px] text-mute">Windows pedirá permiso.</p>}
                {notes[key] && <p className="mt-1 ml-6 text-[11px] text-mute">{notes[key]}</p>}
                {errors[key] && <ActionError className="mt-2 ml-6" message={errors[key]} needs="Instalar un programa necesita administrador." />}
              </li>
            );
          })}
        </ul>
      </section>
    </div>
  );
}
