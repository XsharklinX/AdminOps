// Portada de Ajustes: lo importante de un vistazo, cada cosa con un enlace a
// donde se cambia. Solo lee; no cambia nada.
import { ChevronRight, Loader2 } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { appcareApi, autoBackupApi, lockApi, logQuietly, storageApi, type AppInfo, type AutoBackupInfo, type LockStatus, type StorageInfo, type UpdateInfo } from "../../lib/api";
import { ago } from "../../lib/format";
import { ACCENTS, usePrefs, ZOOMS } from "../../lib/prefs";
import { getTheme } from "../../lib/theme";
import type { SettingsSection } from "./catalog";
import type { SettingsProps } from "./shared";

type Tone = "ok" | "warn" | "neutral";
const DOT: Record<Tone, string> = { ok: "bg-ok", warn: "bg-warn", neutral: "bg-mute" };

function Tile({ label, tone, value, note, action, onGo }: { label: string; tone: Tone; value: ReactNode; note?: ReactNode; action: string; onGo: () => void }) {
  return (
    <div className="flex min-w-0 flex-col gap-1 rounded-xl border border-line bg-panel p-4">
      <span className="text-xs text-mute">{label}</span>
      <span className="flex items-center gap-2 text-[15px] font-medium text-ink">
        <span className={`size-2 shrink-0 rounded-full ${DOT[tone]}`} />
        <span className="min-w-0 truncate">{value}</span>
      </span>
      {note && <span className="text-xs text-dim">{note}</span>}
      <button onClick={onGo} className="mt-auto flex items-center gap-1 self-start pt-1 text-xs text-neon hover:underline">
        {action} <ChevronRight size={12} />
      </button>
    </div>
  );
}

export function Summary({ s, appInfo, onGo }: { s: SettingsProps["s"]; appInfo: AppInfo | null; onGo: (section: SettingsSection, title?: string) => void }) {
  const prefs = usePrefs();
  const [lock, setLock] = useState<LockStatus | null>(null);
  const [backup, setBackup] = useState<AutoBackupInfo | null>(null);
  const [storage, setStorage] = useState<StorageInfo | null>(null);
  // La versión nueva solo se busca si se pide: no se sale a Internet al abrir Ajustes.
  const [update, setUpdate] = useState<UpdateInfo | "busy" | "error" | null>(null);

  useEffect(() => {
    lockApi.status().then(setLock).catch(logQuietly("Summary"));
    autoBackupApi.info().then(setBackup).catch(logQuietly("Summary"));
    storageApi.info().then(setStorage).catch(logQuietly("Summary"));
  }, []);

  const checkUpdate = () => {
    setUpdate("busy");
    appcareApi.checkUpdate().then(setUpdate, () => setUpdate("error"));
  };

  const zoom = ZOOMS.find((z) => z.value === prefs.zoom)?.label ?? "Normal";
  const backupOn = !!backup?.enabled && !!backup.here;

  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      <Tile
        label="Bloqueo de AdminOps"
        tone={lock === null ? "neutral" : lock.enabled ? "ok" : "warn"}
        value={lock === null ? "…" : lock.enabled ? `Con ${lock.kind === "pin" ? "PIN" : "contraseña"}` : "Sin bloqueo"}
        note={lock?.enabled ? (lock.idleMinutes ? `Se bloquea solo a los ${lock.idleMinutes} min sin usarla.` : "Solo se bloquea a mano (Ctrl+L).") : "Cualquiera que se siente delante puede usarla."}
        action={lock?.enabled ? "Cambiarlo" : "Ponerlo"}
        onGo={() => onGo("security")}
      />
      <Tile
        label="Copia de tus datos"
        tone={backup === null ? "neutral" : backup.lastError ? "warn" : backupOn ? "ok" : "warn"}
        value={backup === null ? "…" : !backupOn ? "Sin copia automática" : backup.lastBackup ? `Última ${ago(backup.lastBackup)}` : "Aún no se ha hecho ninguna"}
        note={backup?.lastError ? `La última falló: ${backup.lastError}` : backupOn ? `Cada ${backup?.everyDays} días.` : "Contactos, notas, soluciones… si se pierde el pendrive, se pierden."}
        action={backupOn ? "Ver la copia" : "Activarla"}
        onGo={() => onGo("data", "Copia automática")}
      />
      <Tile
        label="Versión"
        tone={update && typeof update === "object" ? (update.newer ? "warn" : "ok") : "neutral"}
        value={
          <>
            {appInfo ? `AdminOps ${appInfo.version}` : "…"}
            {update && typeof update === "object" && <span className={`ml-2 text-xs ${update.newer ? "text-warn" : "text-ok"}`}>{update.newer ? `hay ${update.latest}` : "al día"}</span>}
          </>
        }
        note={
          update === "busy" ? (
            <span className="flex items-center gap-1.5">
              <Loader2 size={11} className="animate-spin" /> Mirando en GitHub…
            </span>
          ) : update === "error" ? (
            "No se pudo comprobar (¿sin Internet?)."
          ) : update === null ? (
            <button onClick={checkUpdate} className="text-dim underline-offset-2 hover:text-ink hover:underline">
              Buscar versión nueva
            </button>
          ) : undefined
        }
        action="Novedades y licencias"
        onGo={() => onGo("about")}
      />
      <Tile
        label="Dónde se guardan tus datos"
        tone="neutral"
        value={storage === null ? "…" : storage.portable ? `En el pendrive${storage.drive ? ` (${storage.drive})` : ""}` : "En este equipo"}
        note={storage?.portable ? (storage.browserOnUsb ? "Las sesiones de Correo y Teams también van en el pendrive." : "Las sesiones de Correo y Teams se quedan en cada equipo.") : undefined}
        action="Ver los detalles"
        onGo={() => onGo("data", storage?.portable ? "Tus datos viajan con AdminOps" : "Dónde se guardan tus datos")}
      />
      <Tile
        label="Avisos de Windows"
        tone={s.watchWindows ? "ok" : "warn"}
        value={s.watchWindows ? "Vigilando" : "Desactivados"}
        note={s.watchWindows ? { all: "Notificación de todos.", bad: "Notificación solo de los graves.", none: "Sin notificaciones: solo la campana." }[s.notifyAlerts] : "No llegarán avisos de pantallazos, discos ni programas que se cierran."}
        action="Cambiarlo"
        onGo={() => onGo("alerts", "Vigilar errores de Windows")}
      />
      <Tile
        label="Para quién es y cómo se ve"
        tone="neutral"
        value={prefs.mode === "user" ? "Modo usuario sencillo" : "Modo técnico"}
        note={`Tema ${getTheme() === "light" ? "claro" : "oscuro"} · ${ACCENTS[prefs.accent]?.label.toLowerCase() ?? "azul"} · tamaño ${zoom.toLowerCase()} · ${prefs.pageWidth === "limited" ? "centrado" : "toda la ventana"}.`}
        action="Apariencia"
        onGo={() => onGo("appearance")}
      />
      <Tile
        label="Tus informes"
        tone={s.company.trim() ? "ok" : "warn"}
        value={s.company.trim() || "Sin tu marca"}
        note={s.company.trim() ? [s.logo ? "Con logo" : "Sin logo", s.techSignature ? "con firma" : "sin firma"].join(", ") + "." : "Los informes salen sin nombre de empresa, logo ni firma."}
        action="Informes y cobros"
        onGo={() => onGo("reports")}
      />
    </div>
  );
}
