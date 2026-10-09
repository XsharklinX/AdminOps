// Ajustes → Datos de AdminOps: tamaño, limpieza, copias y paquete de soporte.
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../../components/feedback";
import { Button, Card, Toggle } from "../../components/ui";
import { appcareApi, type DataUsage, logQuietly, type UpdateInfo } from "../../lib/api";
import { bytes } from "../../lib/format";
import { TaskStatus } from "../../components/TaskStatus";
import { Row, selectClass, type SettingsProps } from "./shared";

/** `part`: solo los datos (Datos y copias) o solo las actualizaciones (Acerca de). */
export function DataCare({ s, set, part }: SettingsProps & { part: "data" | "updates" }) {
  const [usage, setUsage] = useState<DataUsage | null>(null);
  const [months, setMonths] = useState(6);
  const [parts, setParts] = useState({
    journal: true,
    snapshots: true,
    reports: false,
  });
  const [busy, setBusy] = useState(false);
  const [update, setUpdate] = useState<string | null>(null);
  /** La versión nueva que se puede instalar desde aquí (null: no hay, o no se ha buscado). */
  const [ready, setReady] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);
  const toast = useToast();
  const load = useCallback(() => {
    appcareApi
      .usage()
      .then(setUsage)
      .catch(logQuietly("SettingsPage"));
  }, []);
  useEffect(load, [load]);

  const clean = async () => {
    setBusy(true);
    try {
      const r = await appcareApi.cleanup(
        months,
        parts.journal,
        parts.snapshots,
        parts.reports,
      );
      toast(
        "ok",
        `Limpieza hecha: ${r.journal} entradas del historial, ${r.snapshots} análisis y ${r.reports} informes (${bytes(r.freed)}).`,
      );
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const checkNow = async () => {
    setUpdate("Buscando…");
    try {
      const u = await appcareApi.checkUpdate();
      setReady(u.newer ? u : null);
      setUpdate(
        !u.published
          ? "Todavía no hay ninguna versión publicada en GitHub. Cuando la haya, desde aquí se podrá descargar e instalar."
          : !u.newer
          ? `Tienes la última versión (${u.current}).`
          : u.installable
            ? `Hay una versión nueva: ${u.latest} (${bytes(u.size)}).`
            : `Hay una versión nueva: ${u.latest}, pero no trae un instalador que se pueda descargar desde aquí.`,
      );
    } catch (e) {
      setReady(null);
      setUpdate(String(e));
    }
  };

  // Descarga la versión nueva y abre su instalador: el instalador cierra
  // AdminOps, actualiza y conserva los datos. Nada se instala en silencio.
  const install = async () => {
    setInstalling(true);
    try {
      const how = await appcareApi.installUpdate();
      setReady(null);
      setUpdate(
        how === "installer"
          ? "Descargada. Acepta el aviso de Windows y pulsa «Actualizar» en el instalador: AdminOps se cierra sola en cuanto el instalador se abre, y tus datos se conservan."
          : "Descargada en tu carpeta de Descargas (se abrió). Cierra AdminOps y descomprime el .zip encima de la carpeta del portable: tus datos se conservan.",
      );
    } catch (e) {
      toast("error", String(e));
    } finally {
      setInstalling(false);
    }
  };

  const check = (key: keyof typeof parts, label: string) => (
    <label className="flex items-center gap-2 text-sm text-dim">
      <Toggle checked={parts[key]} onChange={(v) => setParts({ ...parts, [key]: v })} />
      {label}
    </label>
  );

  return (
    <>
      {part === "data" && (
      <Card title="Datos de AdminOps">
        {usage && (
          <div className="mb-4 grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-4">
            <span className="text-dim">
              Historial:{" "}
              <span className="text-ink">{usage.journalEntries} entradas</span>
            </span>
            <span className="text-dim">
              Análisis:{" "}
              <span className="text-ink">
                {usage.snapshots} · {bytes(usage.snapshotsBytes)}
              </span>
            </span>
            <span className="text-dim">
              Informes:{" "}
              <span className="text-ink">
                {usage.reports} · {bytes(usage.reportsBytes)}
              </span>
            </span>
            <span className="text-dim">
              Registros:{" "}
              <span className="text-ink">{bytes(usage.logsBytes)}</span>
            </span>
          </div>
        )}
        <div className="flex flex-wrap items-center gap-3">
          <span className="text-sm text-dim">Borrar lo que tenga más de</span>
          <select
            value={months}
            onChange={(e) => setMonths(Number(e.target.value))}
            className={selectClass}
          >
            {[3, 6, 12, 24].map((m) => (
              <option key={m} value={m}>
                {m} meses
              </option>
            ))}
          </select>
          {check("journal", "Historial")}
          {check("snapshots", "Análisis")}
          {check("reports", "Informes (a la papelera)")}
          <Button
            kind="ghost"
            onClick={clean}
            disabled={
              busy || (!parts.journal && !parts.snapshots && !parts.reports)
            }
          >
            Limpiar ahora
          </Button>
        </div>
        <p className="mt-2 text-[11px] text-mute">
          Nunca se borran los ajustes aplicados que aún se pueden deshacer, el
          último análisis ni el de una sesión en curso. Los informes van a la
          papelera.
        </p>
        <div className="mt-3 border-t border-line/60 pt-3">
          <Row
            title="Limpieza automática al abrir"
            sub="Una vez al día como mucho: historial, análisis e informes más antiguos que lo elegido."
          >
            <select
              value={s.autoCleanupMonths}
              onChange={(e) =>
                set({ autoCleanupMonths: Number(e.target.value) })
              }
              className={selectClass}
            >
              <option value={0}>Desactivada</option>
              {[6, 12, 24].map((m) => (
                <option key={m} value={m}>
                  Más de {m} meses
                </option>
              ))}
            </select>
          </Row>
        </div>
      </Card>
      )}

      {part === "updates" && (
      <Card title="Actualizaciones">
        <Row
          title="Avisar de versiones nuevas"
          sub="Al abrir, consulta en GitHub si hay una versión nueva de AdminOps y lo indica en la barra lateral. No descarga ni instala nada solo: eso lo decides tú con el botón de abajo."
        >
          <Toggle checked={s.checkUpdates} onChange={(v) => set({ checkUpdates: v })} />
        </Row>
        <div className="flex flex-wrap items-center gap-3 pt-1">
          <Button kind="ghost" onClick={checkNow} disabled={installing}>
            Buscar ahora
          </Button>
          {ready?.installable && (
            <Button onClick={() => void install()} disabled={installing}>
              Descargar e instalar la {ready.latest}
            </Button>
          )}
          {ready && !ready.installable && (
            <Button kind="ghost" onClick={() => appcareApi.openRelease(ready.url).catch((e) => toast("error", String(e)))}>
              Ver en GitHub
            </Button>
          )}
          {update && <span className="min-w-0 flex-1 text-xs text-dim">{update}</span>}
        </div>
        <TaskStatus task="update" active={installing} fallback="Descargando la versión nueva…" className="mt-2" />
      </Card>
      )}
    </>
  );
}
