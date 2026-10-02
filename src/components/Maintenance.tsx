import { CalendarClock, HardDriveUpload, Loader2, Timer, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "./feedback";
import { TaskStatus } from "./TaskStatus";
import { Button, Card, Modal, Loading } from "./ui";
import { bytes } from "../lib/format";
import { maintenanceApi, type BootAnalysis, type MaintenanceSchedule } from "../lib/api";

const secs = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
const KIND = { app: "Programa", driver: "Driver", service: "Servicio", device: "Dispositivo", other: "Otro" };

/** Qué retrasa el arranque (registro de rendimiento de Windows). Va en la página Inicio. */
export function BootCard() {
  const [data, setData] = useState<BootAnalysis | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    maintenanceApi.boot().then(setData).catch((e) => setError(String(e)));
  }, []);

  const last = data?.boots[0];
  const avg = data && data.boots.length ? data.boots.reduce((a, b) => a + b.totalMs, 0) / data.boots.length : 0;

  return (
    <Card title="Arranque" icon={<Timer size={14} />} className="mb-4">
      {error ? (
        <p className="text-sm text-mute">{error}</p>
      ) : !data ? (
        <Loading text="Analizando los últimos arranques…" />
      ) : (
        <div className="grid grid-cols-12 gap-4">
          <div className="col-span-12 md:col-span-4">
            <div className="text-[11px] text-mute">Último arranque</div>
            <div className={`font-mono text-2xl ${last && last.totalMs > 60000 ? "text-warn" : "text-neon"}`}>{last ? secs(last.totalMs) : "—"}</div>
            <div className="text-xs text-dim">Media de los últimos {data.boots.length}: {secs(avg)}</div>
            {last && (
              <div className="mt-1 text-[11px] text-mute">
                Hasta el inicio de sesión {secs(last.mainMs)} · después {secs(last.postMs)}
              </div>
            )}
          </div>
          <div className="col-span-12 md:col-span-8">
            <div className="mb-1 text-[11px] text-mute">Lo que más lo retrasa (últimos 60 días)</div>
            {data.culprits.length === 0 ? (
              <p className="text-sm text-ok">Windows no ha detectado nada que ralentice el arranque.</p>
            ) : (
              <div className="pane-sm space-y-0.5 overflow-y-auto">
                {data.culprits.map((c) => (
                  <div key={`${c.kind}-${c.name}`} className="flex items-center gap-3 text-sm">
                    <span className="w-20 shrink-0 text-[11px] text-mute">{KIND[c.kind]}</span>
                    <span className="min-w-0 flex-1 truncate text-ink" title={c.name}>
                      {c.name}
                    </span>
                    <span className="shrink-0 text-[11px] text-mute">×{c.times}</span>
                    <span className="w-16 shrink-0 text-right font-mono text-xs text-warn">+{secs(c.avgDelayMs)}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </Card>
  );
}

/** Espacio que ocupan los puntos de restauración y borrar los antiguos. Va en Historial. */
export function RestoreStorageCard({ isAdmin }: { isAdmin: boolean }) {
  const [s, setS] = useState<Awaited<ReturnType<typeof maintenanceApi.restoreStorage>> | null>(null);
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => {
    if (isAdmin) maintenanceApi.restoreStorage().then(setS).catch(() => {});
  }, [isAdmin]);
  useEffect(load, [load]);

  if (!isAdmin || !s) return null;

  const clean = async () => {
    const ok = await confirm({
      title: "Borrar puntos de restauración antiguos",
      body: `Se borrarán ${s.count - 1} puntos de restauración del disco del sistema y se conservará el más reciente. No se podrá volver a esos puntos.`,
      confirmLabel: "Borrar antiguos",
      danger: true,
    });
    if (!ok) return;
    setBusy(true);
    try {
      const n = await maintenanceApi.deleteOldRestorePoints();
      toast("ok", `${n} puntos de restauración borrados.`);
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mb-3 flex items-center gap-3 rounded-lg border border-line bg-void/40 px-3 py-2 text-xs text-dim">
      <span>
        Ocupan <span className="font-mono text-ink">{bytes(s.used)}</span>
        {s.max > 0 && <> de un máximo de {bytes(s.max)}</>} · {s.count} {s.count === 1 ? "punto" : "puntos"}
      </span>
      {s.count > 1 && (
        <button onClick={clean} disabled={busy} className="ml-auto flex items-center gap-1 text-warn hover:underline disabled:opacity-40">
          {busy ? <Loader2 size={11} className="animate-spin" /> : <Trash2 size={11} />} Borrar antiguos (conservar el último)
        </button>
      )}
      {dialog}
    </div>
  );
}

/** Restaurar drivers desde una copia hecha con AdminOps. Botón + diálogo, va en Diagnóstico. */
export function DriverRestoreButton() {
  const [open, setOpen] = useState(false);
  const [list, setList] = useState<{ name: string; drivers: number; size: number }[] | null>(null);
  const [running, setRunning] = useState<string | null>(null);
  const toast = useToast();

  const show = () => {
    setOpen(true);
    void maintenanceApi
      .driverBackups()
      .then(setList)
      .catch(() => setList([]));
  };

  const restore = async (name: string) => {
    setRunning(name);
    try {
      toast("ok", await maintenanceApi.restoreDrivers(name));
      setOpen(false);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(null);
    }
  };

  return (
    <>
      <button onClick={show} className="flex items-center gap-1 text-[11px] text-mute hover:text-neon" title="Reinstalar drivers desde una copia hecha con AdminOps">
        Restaurar <HardDriveUpload size={10} />
      </button>
      {open && (
        <Modal title="Restaurar drivers" onClose={running ? () => {} : () => setOpen(false)} width="w-[560px]">
          {running ? (
            <TaskStatus task="drivers-restore" active fallback="Instalando drivers…" cancellable={false} />
          ) : !list ? (
            <Loading text="Buscando copias…" />
          ) : list.length === 0 ? (
            <p className="text-sm text-mute">No hay copias de drivers. Haz una con «Copia de drivers» antes de formatear.</p>
          ) : (
            <div className="space-y-1">
              <p className="mb-2 text-xs text-dim">Útil tras formatear el mismo equipo: instala todos los drivers de la copia elegida.</p>
              {list.map((b) => (
                <div key={b.name} className="flex items-center gap-3 rounded-md px-2 py-1.5 hover:bg-panel-2">
                  <span className="min-w-0 flex-1 truncate font-mono text-xs text-ink">{b.name}</span>
                  <span className="text-xs text-mute">
                    {b.drivers} drivers · {bytes(b.size)}
                  </span>
                  <Button kind="ghost" onClick={() => restore(b.name)}>
                    Restaurar
                  </Button>
                </div>
              ))}
            </div>
          )}
        </Modal>
      )}
    </>
  );
}

const DAYS: [string, string][] = [
  ["Monday", "lunes"],
  ["Tuesday", "martes"],
  ["Wednesday", "miércoles"],
  ["Thursday", "jueves"],
  ["Friday", "viernes"],
  ["Saturday", "sábado"],
  ["Sunday", "domingo"],
];

/** Limpieza programada como tarea del sistema. Va en la página Limpieza. */
export function ScheduleCard({ isAdmin }: { isAdmin: boolean }) {
  const [view, setView] = useState<Awaited<ReturnType<typeof maintenanceApi.schedule>> | null>(null);
  const [s, setS] = useState<MaintenanceSchedule | null>(null);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const load = useCallback(() => {
    maintenanceApi
      .schedule()
      .then((v) => {
        setView(v);
        setS(v.schedule);
      })
      .catch(() => {});
  }, []);
  useEffect(load, [load]);

  if (!view || !s) return null;

  const save = async (enable: boolean) => {
    setBusy(true);
    try {
      await maintenanceApi.setSchedule(enable ? s : null);
      toast("ok", enable ? "Mantenimiento programado guardado." : "Mantenimiento programado desactivado.");
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const toggleTask = (id: string) => setS({ ...s, tasks: s.tasks.includes(id) ? s.tasks.filter((t) => t !== id) : [...s.tasks, id] });
  const disabled = !isAdmin || view.portable || busy;
  const select = "rounded border border-line bg-void/60 px-1.5 py-1 text-xs text-ink outline-none";

  return (
    <Card
      title="Mantenimiento programado"
      icon={<CalendarClock size={14} />}
      className="mb-4"
      right={
        view.enabled ? (
          <span className="text-[11px] text-ok">Activo{view.nextRun && ` · próximo: ${new Date(view.nextRun).toLocaleString("es", { dateStyle: "short", timeStyle: "short" })}`}</span>
        ) : (
          <span className="text-[11px] text-mute">Desactivado</span>
        )
      }
    >
      {view.portable ? (
        <p className="text-sm text-mute">En modo portable no se puede programar: instala AdminOps en este equipo para usarlo.</p>
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-2 text-sm text-dim">
            Limpiar
            <select value={s.weeks} onChange={(e) => setS({ ...s, weeks: Number(e.target.value) })} disabled={disabled} className={select}>
              <option value={1}>cada semana</option>
              <option value={2}>cada 2 semanas</option>
              <option value={4}>cada 4 semanas</option>
            </select>
            el
            <select value={s.day} onChange={(e) => setS({ ...s, day: e.target.value })} disabled={disabled} className={select}>
              {DAYS.map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
            a las
            <input type="time" value={s.time} onChange={(e) => setS({ ...s, time: e.target.value })} disabled={disabled} className={select} />
          </div>
          <div className="mt-3 grid grid-cols-2 gap-1">
            {view.available.map(([id, name]) => (
              <label key={id} className="flex items-center gap-2 text-xs text-dim">
                <input type="checkbox" checked={s.tasks.includes(id)} onChange={() => toggleTask(id)} disabled={disabled} className="accent-[var(--color-neon)]" />
                {name}
              </label>
            ))}
          </div>
          <div className="mt-3 flex items-center gap-2">
            <Button onClick={() => save(true)} disabled={disabled || !s.tasks.length}>
              {busy && <Loader2 size={13} className="animate-spin" />} {view.enabled ? "Guardar cambios" : "Activar"}
            </Button>
            {view.enabled && (
              <Button kind="ghost" onClick={() => save(false)} disabled={disabled}>
                Desactivar
              </Button>
            )}
            {view.lastRun && (
              <span className="ml-auto text-[11px] text-mute">
                Última vez: {new Date(view.lastRun).toLocaleString("es", { dateStyle: "short", timeStyle: "short" })}
                {view.lastResult !== null && (view.lastResult === 0 ? " · bien" : " · con errores (ver Historial)")}
              </span>
            )}
          </div>
          <p className="mt-2 text-[11px] text-mute">
            Se ejecuta en segundo plano aunque AdminOps esté cerrado; si el equipo estaba apagado, al encenderlo. Cada limpieza queda en el Historial.
            {!isAdmin && " Requiere administrador."}
          </p>
        </>
      )}
    </Card>
  );
}
