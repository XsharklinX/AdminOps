import {
  Check,
  ChevronDown,
  Copy,
  Cpu,
  Eye,
  EyeOff,
  Fan,
  HardDrive,
  Loader2,
  MemoryStick,
  Monitor,
  RefreshCw,
  Thermometer,
  TriangleAlert,
  Zap,
} from "lucide-react";
import { Fragment, useCallback, useEffect, useState, type ReactNode } from "react";
import { useToast } from "../components/feedback";
import type { PageId } from "../components/Sidebar";
import { TaskStatus } from "../components/TaskStatus";
import { Card } from "../components/ui";
import { tempColor, useSensors } from "../hooks/useSensors";
import { hwApi, type Inventory, type MemoryTest, type SmartDisk } from "../lib/api";
import { bytes } from "../lib/format";

const ageOf = (iso: string | null) => {
  if (!iso) return null;
  const d = new Date(iso);
  if (isNaN(+d)) return null;
  const months = Math.floor((Date.now() - +d) / (30 * 24 * 3600 * 1000));
  return { date: d.toLocaleDateString("es", { dateStyle: "medium" }), months };
};

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex justify-between gap-4 border-t border-line/60 py-1.5 text-sm first:border-t-0">
      <span className="shrink-0 text-dim">{label}</span>
      <span className="min-w-0 text-right text-ink">{children}</span>
    </div>
  );
}

function Temp({ value, label }: { value: number | null | undefined; label: string }) {
  return (
    <div className="rounded-lg border border-line bg-void/40 px-3 py-2">
      <div className="truncate text-[11px] text-mute" title={label}>
        {label}
      </div>
      <div className="font-mono text-2xl tabular" style={{ color: tempColor(value) }}>
        {value != null ? `${value.toFixed(0)} °C` : "—"}
      </div>
    </div>
  );
}

export function Hardware({ isAdmin, focus, onNavigate }: { isAdmin: boolean; focus?: string | null; onNavigate: (p: PageId, f?: string | null) => void }) {
  const [inv, setInv] = useState<Inventory | null>(null);
  const [invError, setInvError] = useState<string | null>(null);
  const [smart, setSmart] = useState<SmartDisk[] | null>(null);
  const [smartError, setSmartError] = useState<string | null>(null);
  const [memTest, setMemTest] = useState<MemoryTest | null | undefined>(undefined);
  const [showKey, setShowKey] = useState(false);
  const [showSerial, setShowSerial] = useState(false);
  const [openSmart, setOpenSmart] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [hl, setHl] = useState<string | null>(null);
  const { sensors, error: sensorsError } = useSensors(3000);
  const toast = useToast();

  const load = useCallback(() => {
    setInv(null);
    hwApi.inventory().then(setInv).catch((e) => setInvError(String(e)));
    hwApi.smart().then(setSmart).catch((e) => setSmartError(String(e)));
    hwApi.memoryTest().then(setMemTest).catch(() => setMemTest(null));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    if (!focus || !inv) return;
    const el = document.getElementById(`focus-${focus}`);
    if (!el) return;
    el.scrollIntoView({ behavior: "smooth", block: "start" });
    setHl(focus);
    const t = window.setTimeout(() => setHl(null), 2500);
    return () => window.clearTimeout(t);
  }, [focus, inv]);

  const ring = (k: string) => (hl === k ? "border-neon! glow-neon" : "");

  const installPawnio = async () => {
    setInstalling(true);
    try {
      toast("ok", await hwApi.installPawnio());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setInstalling(false);
    }
  };

  const copy = (text: string) => navigator.clipboard.writeText(text).then(() => toast("ok", "Copiado al portapapeles."));

  if (invError) return <p className="p-8 text-bad">{invError}</p>;
  if (!inv)
    return (
      <p className="flex items-center gap-2 p-8 text-sm text-dim">
        <Loader2 size={15} className="animate-spin" /> Leyendo el hardware del equipo…
      </p>
    );

  const bios = ageOf(inv.biosDate);
  const sizes = new Set(inv.modules.map((m) => m.capacity));
  const slots = Math.max(inv.ramSlots, inv.modules.length);
  const bySlot = (i: number) => inv.modules[i];

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      {/* Resumen */}
      <Card
        title={`${inv.manufacturer} ${inv.model}`.trim() || "Equipo"}
        icon={<Monitor size={14} />}
        className="col-span-12"
        right={
          <button onClick={load} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
            <RefreshCw size={11} /> Volver a leer
          </button>
        }
      >
        <div className="grid grid-cols-2 gap-x-8 md:grid-cols-2">
          <div>
            <Row label="Windows">
              {inv.os} {inv.osVersion} <span className="font-mono text-xs text-mute">({inv.osBuild})</span>
            </Row>
            <Row label="Arquitectura">{inv.architecture}</Row>
            <Row label="Número de serie">
              <span className="inline-flex items-center gap-2 font-mono text-xs">
                {showSerial ? inv.serial || "—" : "••••••••"}
                <button onClick={() => setShowSerial(!showSerial)} className="text-mute hover:text-ink">
                  {showSerial ? <EyeOff size={12} /> : <Eye size={12} />}
                </button>
              </span>
            </Row>
          </div>
          <div>
            <Row label="Clave de Windows (placa)">
              {inv.productKey ? (
                <span className="inline-flex items-center gap-2 font-mono text-xs">
                  {showKey ? inv.productKey : "•••••-•••••-•••••-•••••-•••••"}
                  <button onClick={() => setShowKey(!showKey)} className="text-mute hover:text-ink" title="Mostrar/ocultar">
                    {showKey ? <EyeOff size={12} /> : <Eye size={12} />}
                  </button>
                  <button onClick={() => copy(inv.productKey!)} className="text-mute hover:text-ink" title="Copiar">
                    <Copy size={12} />
                  </button>
                </span>
              ) : (
                <span className="text-xs text-mute">No hay clave OEM en el firmware (licencia digital o retail)</span>
              )}
            </Row>
            <Row label="Firmware">{inv.firmware}</Row>
            <Row label="Virtualización">
              {inv.virtualization === null ? "—" : inv.virtualization ? <span className="text-ok">Activada</span> : <span className="text-warn">Desactivada en la BIOS</span>}
            </Row>
          </div>
        </div>
      </Card>

      {/* Sensores */}
      <Card id="focus-sensors" title="Temperaturas en vivo" icon={<Thermometer size={14} />} className={`col-span-12 ${ring("sensors")}`}>
        {!sensors && !sensorsError && (
          <p className="flex items-center gap-2 text-sm text-dim">
            <Loader2 size={14} className="animate-spin" /> Cargando sensores…
          </p>
        )}
        {sensorsError && <p className="text-sm text-bad">{sensorsError}</p>}
        {sensors && (
          <>
            <div className="grid grid-cols-2 gap-2.5 md:grid-cols-4">
              <Temp value={sensors.cpuTemp} label={`CPU${sensors.cpuPower ? ` · ${sensors.cpuPower.toFixed(0)} W` : ""}`} />
              {sensors.gpus.map((g) => (
                <Temp key={g.name} value={g.temperature} label={`${g.name}${g.load != null ? ` · ${g.load.toFixed(0)} %` : ""}`} />
              ))}
              {sensors.gpus.map((g) => g.hotspot != null && <Temp key={`${g.name}-hs`} value={g.hotspot} label="GPU · punto caliente" />)}
              {sensors.otherTemps.map(([n, v]) => (
                <Temp key={n} value={v} label={n} />
              ))}
            </div>
            {sensors.fans.length > 0 && (
              <div className="mt-3 flex flex-wrap gap-4 text-xs text-dim">
                {sensors.fans.map(([n, v]) => (
                  <span key={n} className="flex items-center gap-1.5">
                    <Fan size={12} className="text-neon" /> {n}: <span className="font-mono text-ink">{v.toFixed(0)} rpm</span>
                  </span>
                ))}
              </div>
            )}
            {sensors.cpuNeedsDriver && (
              <div className="mt-3 flex items-center gap-3 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn">
                <TriangleAlert size={14} className="shrink-0" />
                <span className="flex-1">
                  {!isAdmin
                    ? "La temperatura de la CPU y la placa requieren ejecutar AdminOps como administrador."
                    : !sensors.pawnioInstalled
                      ? "Para leer CPU y placa hace falta el driver PawnIO (libre y firmado)."
                      : "Esta CPU no expone su temperatura o requiere reiniciar AdminOps tras instalar PawnIO."}
                </span>
                {isAdmin && !sensors.pawnioInstalled && (
                  <button
                    onClick={installPawnio}
                    disabled={installing}
                    className="shrink-0 rounded-md border border-warn/50 px-3 py-1 text-warn hover:bg-warn/10 disabled:opacity-50"
                  >
                    Instalar PawnIO
                  </button>
                )}
              </div>
            )}
            <TaskStatus task="pawnio" active={installing} fallback="Instalando…" cancellable={false} className="mt-2" />
          </>
        )}
      </Card>

      {/* Placa y CPU */}
      <Card id="focus-board" title="Placa base y procesador" icon={<Cpu size={14} />} className={`col-span-12 lg:col-span-6 ${ring("board")}`}>
        <Row label="Placa base">
          {inv.boardManufacturer} {inv.boardProduct}
        </Row>
        <Row label="BIOS">
          {inv.biosVendor} {inv.biosVersion}
          {bios && (
            <span className={`ml-1 text-xs ${bios.months >= 36 ? "text-warn" : "text-mute"}`}>
              · {bios.date}
              {bios.months >= 36 && ` (hace ${Math.floor(bios.months / 12)} años)`}
            </span>
          )}
        </Row>
        <Row label="Procesador">{inv.cpu}</Row>
        <Row label="Núcleos / hilos">
          {inv.cores} / {inv.threads} · {(inv.maxMhz / 1000).toFixed(2)} GHz
        </Row>
        <Row label="Zócalo">{inv.socket || "—"}</Row>
      </Card>

      {/* Gráfica y monitores */}
      <Card id="focus-gpu" title="Gráfica y pantallas" icon={<Zap size={14} />} className={`col-span-12 lg:col-span-6 ${ring("gpu")}`}>
        {inv.gpus.map((g) => {
          const age = ageOf(g.driverDate);
          return (
            <div key={g.name} className="mb-2 border-b border-line/60 pb-2 last:mb-0 last:border-0">
              <div className="text-sm font-medium text-ink">{g.name}</div>
              <div className="text-xs text-dim">
                {g.vram ? `${bytes(g.vram)} VRAM · ` : ""}driver {g.driverVersion}
                {age && <span className={age.months >= 12 ? "text-warn" : ""}> · {age.date}{age.months >= 12 && ` (hace ${age.months} meses)`}</span>}
              </div>
              {g.resolution && <div className="font-mono text-[11px] text-mute">{g.resolution}</div>}
            </div>
          );
        })}
        {inv.monitors.map((m, i) => (
          <Row key={i} label="Monitor">
            {m.manufacturer} {m.name}
            {m.year && <span className="text-xs text-mute"> · {m.year}</span>}
          </Row>
        ))}
      </Card>

      {/* Memoria */}
      <Card id="focus-memory" title="Memoria RAM" icon={<MemoryStick size={14} />} className={`col-span-12 ${ring("memory")}`}>
        <div className="mb-3 flex flex-wrap items-baseline gap-x-4 text-sm">
          <span>
            <span className="font-mono text-lg text-neon">{bytes(inv.ramTotal)}</span> instalados
          </span>
          <span className="text-dim">
            {inv.modules.length} de {slots} ranuras ocupadas
            {inv.ramMax && ` · máximo admitido ${bytes(inv.ramMax)}`}
          </span>
          {inv.modules.length === 1 && slots >= 2 && <span className="text-xs text-warn">Un solo canal: añadir un módulo igual duplica el ancho de banda.</span>}
          {sizes.size > 1 && <span className="text-xs text-warn">Módulos de distinto tamaño: parte de la memoria puede ir sin doble canal.</span>}
        </div>
        <div className="grid gap-2" style={{ gridTemplateColumns: `repeat(${Math.min(Math.max(slots, 1), 4)}, minmax(0, 1fr))` }}>
          {Array.from({ length: slots }, (_, i) => {
            const m = bySlot(i);
            return (
              <div key={i} className={`rounded-lg border px-3 py-2.5 ${m ? "border-neon/30 bg-neon/5" : "border-dashed border-line-2 bg-void/20"}`}>
                <div className="text-[11px] text-mute">{m ? m.slot || `Ranura ${i + 1}` : `Ranura ${i + 1}`}</div>
                {m ? (
                  <>
                    <div className="font-mono text-xl text-ink">{bytes(m.capacity)}</div>
                    <div className="text-xs text-dim">
                      {m.kind} {m.configuredSpeed ?? m.speed ? `· ${m.configuredSpeed ?? m.speed} MT/s` : ""}
                    </div>
                    <div className="truncate text-[11px] text-mute" title={`${m.manufacturer} ${m.partNumber}`}>
                      {m.manufacturer} {m.partNumber}
                    </div>
                  </>
                ) : (
                  <div className="text-sm text-mute">Libre</div>
                )}
              </div>
            );
          })}
        </div>
        <div className="mt-3 flex items-center gap-3 text-xs">
          <span className="text-dim">Prueba de memoria:</span>
          {memTest === undefined ? (
            <span className="text-mute">…</span>
          ) : memTest === null ? (
            <span className="text-mute">nunca se ha ejecutado</span>
          ) : memTest.passed ? (
            <span className="flex items-center gap-1 text-ok">
              <Check size={12} /> sin errores ({new Date(memTest.time).toLocaleDateString("es")})
            </span>
          ) : (
            <span className="text-bad">errores detectados ({new Date(memTest.time).toLocaleDateString("es")})</span>
          )}
          <button onClick={() => onNavigate("repair", "repair.memory-test")} className="text-neon hover:underline">
            Programar prueba →
          </button>
        </div>
      </Card>

      {/* SMART */}
      <Card id="focus-smart" title="Salud de discos (SMART)" icon={<HardDrive size={14} />} className={`col-span-12 ${ring("smart")}`}>
        {smartError ? (
          <p className="text-sm text-mute">{smartError}</p>
        ) : smart === null ? (
          <p className="text-sm text-mute">Leyendo…</p>
        ) : smart.length === 0 ? (
          <p className="text-sm text-mute">Ningún disco SATA expone SMART (los NVMe se revisan en Diagnóstico).</p>
        ) : (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-[11px] text-mute">
                <th className="pb-2 font-medium">Disco</th>
                <th className="pb-2 text-right font-medium">Reasignados</th>
                <th className="pb-2 text-right font-medium">Pendientes</th>
                <th className="pb-2 text-right font-medium">No corregibles</th>
                <th className="pb-2 text-right font-medium">CRC</th>
                <th className="pb-2 text-right font-medium">Horas</th>
                <th className="pb-2 text-right font-medium">Temp.</th>
              </tr>
            </thead>
            <tbody className="font-mono text-xs tabular">
              {smart.map((k) => {
                const bad = (v: number | null) => (v ? "text-bad" : "text-ok");
                const open = openSmart === k.model;
                return (
                  <Fragment key={k.model}>
                    <tr onClick={() => setOpenSmart(open ? null : k.model)} className="cursor-pointer border-t border-line/60 hover:bg-panel-2">
                      <td className="py-1.5 font-sans text-[13px] text-ink">
                        <span className="inline-flex items-center gap-1.5">
                          <ChevronDown size={12} className={`text-mute transition-transform ${open ? "rotate-180" : ""}`} />
                          {k.model}
                          {k.predictFailure && <span className="rounded bg-bad/15 px-1.5 text-[11px] text-bad">FALLO PREVISTO</span>}
                        </span>
                      </td>
                      <td className={`py-1.5 text-right ${bad(k.reallocated)}`}>{k.reallocated ?? "—"}</td>
                      <td className={`py-1.5 text-right ${bad(k.pending)}`}>{k.pending ?? "—"}</td>
                      <td className={`py-1.5 text-right ${bad(k.uncorrectable)}`}>{k.uncorrectable ?? "—"}</td>
                      <td className={`py-1.5 text-right ${k.crcErrors ? "text-warn" : "text-ok"}`}>{k.crcErrors ?? "—"}</td>
                      <td className="py-1.5 text-right">{k.powerOnHours?.toLocaleString("es") ?? "—"}</td>
                      <td className="py-1.5 text-right" style={{ color: tempColor(k.temperature) }}>
                        {k.temperature != null ? `${k.temperature} °C` : "—"}
                      </td>
                    </tr>
                    {open && (
                      <tr>
                        <td colSpan={7} className="bg-void/40 px-3 py-2">
                          <div className="grid grid-cols-2 gap-x-6 gap-y-0.5 text-[11px] md:grid-cols-3">
                            {k.attributes.map((a) => (
                              <span key={a.id} className="flex justify-between gap-2">
                                <span className="truncate text-dim">
                                  {a.id} · {a.name}
                                </span>
                                <span className="text-ink">{a.raw.toLocaleString("es")}</span>
                              </span>
                            ))}
                          </div>
                        </td>
                      </tr>
                    )}
                  </Fragment>
                );
              })}
            </tbody>
          </table>
        )}
      </Card>

      <p className="col-span-12 text-center text-[11px] text-mute">
        Sensores: LibreHardwareMonitor (MPL-2.0).{" "}
        <button onClick={() => hwApi.openNotices().catch((e) => toast("error", String(e)))} className="underline hover:text-ink">
          Componentes de terceros
        </button>
      </p>
    </div>
  );
}
