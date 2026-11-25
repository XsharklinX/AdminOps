// Ajustes → Datos de AdminOps: tamaño, limpieza, copias y paquete de soporte.
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../../components/feedback";
import { Button, Card, Toggle } from "../../components/ui";
import { appcareApi, type DataUsage, logQuietly, type PreviousVersion, type UpdateInfo } from "../../lib/api";
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
  /** La versión anterior guardada al actualizar, para poder volver (1.5). */
  const [previous, setPrevious] = useState<PreviousVersion | null>(null);
  /** Ya vio el aviso de que la anterior no lee algunos datos de esta. */
  const [dataWarned, setDataWarned] = useState(false);
  const [rollingBack, setRollingBack] = useState(false);
  const toast = useToast();
  useEffect(() => {
    if (part !== "updates") return;
    appcareApi.previous().then(setPrevious).catch(logQuietly("SettingsPage"));
  }, [part]);

  const goBack = async () => {
    if (!previous) return;
    if (!previous.compatible && !dataWarned) {
      setDataWarned(true);
      return;
    }
    setRollingBack(true);
    try {
      const v = await appcareApi.rollback(!previous.compatible);
      setUpdate(`Se abrió el instalador de la ${v}. Pulsa «Volver a la ${v}»: AdminOps se cierra sola y tus datos se conservan.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRollingBack(false);
    }
  };
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

  // Descarga la versión nueva y la instala sola: el instalador cierra AdminOps,
  // actualiza conservando los datos y la vuelve a abrir.
  const install = async () => {
    setInstalling(true);
    try {
      const r = await appcareApi.installUpdate();
      setReady(null);
      // Qué se comprobó: la huella SHA-256 contra lo publicado, una o dos veces.
      const check =
        r.verified === "none"
          ? " Esta versión no publica huella para comprobar el archivo: solo se comprobó el tamaño."
          : ` Huella SHA-256 comprobada${r.verified === "both" ? " con dos fuentes" : ""}: ${r.sha256.slice(0, 12)}…`;
      setUpdate(
        (r.how === "installer"
          ? "Descargada. Se está instalando: AdminOps se cierra en un momento y vuelve a abrirse sola al terminar. Tus datos se conservan."
          : r.how === "portable"
            ? "Descargada. AdminOps se cierra para cambiar de versión y vuelve a abrirse sola en unos segundos. Tus datos se conservan."
            : "Descargada en tu carpeta de Descargas (se abrió), pero esta carpeta del portable no se puede cambiar sola. Cierra AdminOps y descomprime el .zip encima de la carpeta del portable: tus datos se conservan.") + check,
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
        {previous && (
          <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-line pt-3">
            <div className="min-w-0 flex-1">
              <p className="text-sm text-ink">Versión anterior guardada</p>
              <p className="text-xs text-dim">
                <span className="font-mono text-ink">{previous.version}</span> · Setup comprobado con SHA256 · {bytes(previous.size)}
              </p>
              <p className={`text-xs ${previous.compatible ? "text-ok" : "text-warn"}`}>
                {previous.compatible
                  ? "Tus datos sirven en las dos versiones"
                  : `Esta versión cambió algunos datos que la ${previous.version} no sabe leer: lo que hayas hecho desde que actualizaste podría no verse al volver.`}
              </p>
            </div>
            <Button kind="ghost" onClick={() => void goBack()} disabled={rollingBack || installing}>
              {!previous.compatible && dataWarned ? `Volver igualmente a la ${previous.version}` : `Volver a la ${previous.version}`}
            </Button>
          </div>
        )}
      </Card>
      )}
    </>
  );
}
