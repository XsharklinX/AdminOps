// La biblioteca de drivers de impresoras y escáneres RICOH y EPSON: los paquetes
// que ya tienes (en una carpeta o un pendrive), añadir más y bajarlos de la
// página oficial de la marca, recogiendo el archivo cuando llega a Descargas.
import { Download, FilePlus2, FolderOpen, FolderPlus, Loader2, Package, RefreshCw, RotateCcw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { Button, EmptyLine, ErrorState, Loading, smallBtn, Notice } from "../components/ui";
import { ActionError, BrandBadge } from "../components/printers/Parts";
import { bestCandidate, bytesText, coveredBy, dayText, describeDriver, downloadTitle, VENDOR_NAME } from "../components/printers/logic";
import {
  disksApi,
  onVendorEvent,
  vendorApi,
  wipeApi,
  type DriverLibrary,
  type VendorDetected,
  type VendorDownloadEvent,
  type VendorKey,
  type VendorPrinter,
  type VendorWatchEvent,
} from "../lib/api";

const WATCH_MINUTES = 15;

export function Drivers({ isAdmin }: { isAdmin: boolean }) {
  const toast = useToast();
  const [lib, setLib] = useState<DriverLibrary | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [printers, setPrinters] = useState<VendorPrinter[]>([]);
  const [pending, setPending] = useState<VendorDetected[]>([]);
  const [watching, setWatching] = useState(false);
  const [arrived, setArrived] = useState<VendorDownloadEvent[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const loadLib = useCallback(
    async (rescan = false) => {
      setScanning(true);
      try {
        setLib(await vendorApi.library(rescan));
        setFailed(null);
      } catch (e) {
        setFailed(String(e));
      } finally {
        setScanning(false);
      }
    },
    [],
  );

  useEffect(() => {
    void loadLib(false);
    vendorApi
      .printers()
      .then((i) => {
        setPrinters(i.printers);
        setPending(i.notInstalled);
      })
      .catch(() => {});
  }, [loadLib]);

  useEffect(() => {
    const offDl = onVendorEvent<VendorDownloadEvent>("printvendor:download", (ev) => setArrived((a) => [ev, ...a.filter((x) => x.id !== ev.id)]));
    const offWatch = onVendorEvent<VendorWatchEvent>("printvendor:watch", (ev) => setWatching(ev.state === "started"));
    return () => {
      offDl();
      offWatch();
    };
  }, []);

  const guard = async (key: string, op: () => Promise<void>) => {
    setBusy(key);
    setActionError(null);
    try {
      await op();
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const changeDir = () =>
    guard("dir", async () => {
      const dir = await disksApi.pickFolder();
      if (!dir) return;
      await vendorApi.setLibraryDir(dir);
      await loadLib(true);
      toast("ok", "Carpeta de la biblioteca cambiada.");
    });

  const defaultDir = () =>
    guard("dir", async () => {
      await vendorApi.setLibraryDir(null);
      await loadLib(true);
      toast("ok", "Vuelve a usarse la carpeta de AdminOps.");
    });

  const addPaths = (key: string, pick: () => Promise<string[]>) =>
    guard(key, async () => {
      const paths = await pick();
      for (const p of paths) {
        const r = await vendorApi.libraryAdd(p);
        toast("ok", r.message || `«${r.name}» guardado en la biblioteca.`);
      }
      if (paths.length) await loadLib(true);
    });

  const fromWeb = (vendor: VendorKey, model: string) =>
    guard(`web-${vendor}`, async () => {
      const page = await vendorApi.openVendorPage(vendor, model);
      await vendorApi.watchDownloads(WATCH_MINUTES);
      toast("ok", page.copied && model ? `Página de ${VENDOR_NAME[vendor]} abierta. El modelo está copiado: pégalo en su buscador.` : `Página de ${VENDOR_NAME[vendor]} abierta.`);
    });

  const save = (ev: VendorDownloadEvent, install: boolean) =>
    guard(`dl-${ev.id}`, async () => {
      const r = await vendorApi.libraryAdd(ev.id);
      toast("ok", r.message || "Guardado en la biblioteca.");
      setArrived((a) => a.filter((x) => x.id !== ev.id));
      await loadLib(true);
      if (!install) return;
      const cover = coveredBy(ev, printers, pending);
      const target = cover.printers[0];
      const dev = cover.pending[0];
      const device = target?.device ?? dev?.device;
      if (!device) return;
      const c = await vendorApi.candidates(device, false);
      const best = bestCandidate(c.candidates.filter((x) => x.source === "biblioteca"));
      if (!best) throw new Error("El paquete se guardó, pero no encaja con esa impresora.");
      if (target) {
        const u = await vendorApi.update(target.name, best.id);
        toast("ok", u.message);
      } else {
        const i = await vendorApi.install(device, best.id);
        toast("ok", i.message);
      }
    });

  if (!lib) return failed ? <ErrorState page message={failed} onRetry={() => void loadLib(true)} /> : <Loading page text="Leyendo la biblioteca…" />;

  const needing = printers.filter((p) => p.vendor !== "otra" && ["falta", "generico", "viejo", "sinComparar"].includes(p.state.status));
  const packages = new Map<string, typeof lib.drivers>();
  for (const d of lib.drivers) packages.set(d.package, [...(packages.get(d.package) ?? []), d]);

  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      {/* La carpeta */}
      <section className="rounded-2xl border border-line/70 bg-panel p-4 shadow-elev-1">
        <div className="flex flex-wrap items-center gap-3">
          <FolderOpen size={18} className="shrink-0 text-neon" />
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium text-ink">Biblioteca de drivers</div>
            <div className="truncate text-[11px] text-mute" title={lib.customDir ?? undefined}>
              {lib.customDir ?? "Carpeta de AdminOps (viaja con la versión portable)"} · {packages.size} {packages.size === 1 ? "paquete" : "paquetes"} · leída {dayText(lib.scannedAt)}
            </div>
          </div>
          <Button kind="secondary" size="sm" onClick={changeDir} disabled={busy !== null}>
            <FolderOpen size={13} /> Cambiar carpeta
          </Button>
          {!lib.isDefault && (
            <Button kind="ghost" size="sm" onClick={defaultDir} disabled={busy !== null}>
              <RotateCcw size={13} /> La de AdminOps
            </Button>
          )}
          <button type="button" onClick={() => void loadLib(true)} disabled={scanning} className={smallBtn} title="Volver a leer la carpeta">
            <RefreshCw size={13} className={scanning ? "animate-spin" : ""} /> Volver a leer
          </button>
        </div>
        {!lib.exists && <Notice tone="warn" className="mt-3">La carpeta no está disponible. Si es un pendrive, conéctalo y pulsa «Volver a leer».</Notice>}
        {actionError && <ActionError className="mt-3" message={actionError} />}
      </section>

      {/* Añadir y bajar */}
      <section className="rounded-2xl border border-line/70 bg-panel p-4 shadow-elev-1">
        <div className="flex flex-wrap items-center gap-2">
          <Button
            kind="secondary"
            onClick={() =>
              addPaths("add-folder", async () => {
                const p = await disksApi.pickFolder();
                return p ? [p] : [];
              })
            }
            disabled={busy !== null}
          >
            <FolderPlus size={14} /> Añadir carpeta
          </Button>
          <Button kind="secondary" onClick={() => addPaths("add-file", () => wipeApi.pick(false))} disabled={busy !== null}>
            <FilePlus2 size={14} /> Añadir archivo (.zip o .exe)
          </Button>
          <span className="mx-1 h-5 w-px bg-line" />
          {(["ricoh", "epson"] as const).map((k) => (
            <Button key={k} kind="secondary" onClick={() => fromWeb(k, "")} disabled={busy !== null}>
              {busy === `web-${k}` ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />} Bajar de {VENDOR_NAME[k]}
            </Button>
          ))}
        </div>
        <p className="mt-2 text-[11px] text-mute">
          «Bajar de» abre la página oficial de la marca: AdminOps nunca baja drivers de webs de terceros. Cuando el archivo llega a Descargas, lo recoge y te ofrece guardarlo aquí.
        </p>
        {watching && (
          <p className="mt-2 flex items-center gap-2 text-xs text-dim">
            <Loader2 size={12} className="animate-spin text-neon" /> Esperando una descarga en Descargas ({WATCH_MINUTES} minutos como mucho).
            <button type="button" onClick={() => void vendorApi.watchDownloads(0).catch((e) => toast("error", String(e)))} className="text-neon hover:underline">
              Dejar de esperar
            </button>
          </p>
        )}
        {arrived.map((ev) => {
          const cover = coveredBy(ev, printers, pending);
          const names = [...cover.printers.map((p) => p.name), ...cover.pending.map((d) => d.model || d.name)];
          const key = `dl-${ev.id}`;
          return (
            <div key={ev.id} className="mt-3 rounded-lg border border-neon/40 bg-neon/5 p-3 text-xs">
              <p className="flex flex-wrap items-center gap-1.5 text-ink">
                <BrandBadge vendor={ev.vendor} /> Llegó a Descargas: <span className="font-medium select-text">{downloadTitle(ev)}</span>
                <span className="text-mute">({bytesText(ev.size)})</span>
              </p>
              {names.length > 0 && <p className="mt-1 text-dim">Cubre: {names.join(", ")}.</p>}
              <div className="mt-2 flex flex-wrap items-center gap-2">
                <Button size="sm" kind="secondary" onClick={() => save(ev, false)} disabled={busy !== null}>
                  {busy === key ? <Loader2 size={13} className="animate-spin" /> : null} Guardar en la biblioteca
                </Button>
                {names.length > 0 && (
                  <Button size="sm" onClick={() => save(ev, true)} disabled={busy !== null || !isAdmin} title={isAdmin ? undefined : "Requiere administrador"}>
                    Guardar e instalar
                  </Button>
                )}
                <button type="button" onClick={() => setArrived((a) => a.filter((x) => x.id !== ev.id))} className="text-mute hover:text-ink">
                  Ignorar
                </button>
              </div>
            </div>
          );
        })}
      </section>

      {/* Impresoras que necesitan driver */}
      {(needing.length > 0 || pending.length > 0) && (
        <section className="rounded-2xl border border-line/70 bg-panel p-4 shadow-elev-1">
          <h3 className="mb-2 text-[11px] font-semibold text-dim">Lo que le falta a este equipo</h3>
          <ul className="space-y-1.5">
            {[...pending.map((d) => ({ key: d.instanceId || d.name, vendor: d.vendor, model: d.model || d.name, text: "conectada, sin driver" })), ...needing.map((p) => ({ key: p.name, vendor: p.vendor, model: p.model || p.name, text: p.state.label.toLowerCase() }))].map((r) => (
              <li key={r.key} className="flex items-center gap-2 text-xs">
                <BrandBadge vendor={r.vendor} />
                <span className="min-w-0 flex-1 truncate text-ink">
                  {r.model} <span className="text-mute">· driver: {r.text}</span>
                </span>
                {r.vendor !== "otra" && (
                  <button type="button" onClick={() => void fromWeb(r.vendor, r.model)} disabled={busy !== null} className="shrink-0 text-neon hover:underline disabled:opacity-40">
                    Bajar de {VENDOR_NAME[r.vendor]}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* Paquetes */}
      <section className="rounded-2xl border border-line/70 bg-panel shadow-elev-1">
        <h3 className="border-b border-line/70 px-4 py-2.5 text-[11px] font-semibold text-dim">Paquetes de drivers</h3>
        {packages.size === 0 ? (
          <div className="p-4">
            <EmptyLine>La biblioteca está vacía. Añade una carpeta o un archivo, o bájalo de la página oficial.</EmptyLine>
          </div>
        ) : (
          <ul className="divide-y divide-line/60">
            {[...packages.entries()].map(([pkg, drivers]) => (
              <li key={pkg} className="px-4 py-2.5 text-xs">
                <div className="flex items-center gap-2">
                  <Package size={14} className="shrink-0 text-mute" />
                  <span className="font-medium text-ink">{pkg}</span>
                </div>
                <ul className="mt-1.5 space-y-1 pl-6">
                  {drivers.map((d) => (
                    <li key={d.id} className="flex flex-wrap items-center gap-x-2 text-mute">
                      <BrandBadge vendor={d.vendor} />
                      <span className="text-dim">{describeDriver(d.provider || d.class, d.version)}</span>
                      <span>{d.date}</span>
                      <span className="min-w-0 flex-1 truncate" title={d.models.join(", ")}>
                        · cubre {d.modelCount} {d.modelCount === 1 ? "modelo" : "modelos"}
                        {d.models.length > 0 ? `: ${d.models.slice(0, 3).join(", ")}${d.modelCount > 3 ? "…" : ""}` : ""}
                      </span>
                    </li>
                  ))}
                </ul>
              </li>
            ))}
          </ul>
        )}
        {lib.installers.length > 0 && (
          <>
            <h3 className="border-y border-line/70 px-4 py-2.5 text-[11px] font-semibold text-dim">Instaladores del fabricante</h3>
            <ul className="divide-y divide-line/60">
              {lib.installers.map((i) => (
                <li key={i.id} className="flex items-center gap-2 px-4 py-2 text-xs">
                  <Package size={14} className="shrink-0 text-mute" />
                  <span className="min-w-0 flex-1 truncate text-ink" title={i.name}>
                    {i.name}
                  </span>
                  <span className="shrink-0 text-mute">{bytesText(i.size)}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
    </div>
  );
}
