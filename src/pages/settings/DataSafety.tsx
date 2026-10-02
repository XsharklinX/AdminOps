import { AlertTriangle, CheckCircle2, HardDrive, KeyRound, Loader2, RefreshCw, ShieldCheck, Upload } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../../components/feedback";
import { Button, Card, inputClass, Modal } from "../../components/ui";
import { appBackupApi, type StorageHealth } from "../../lib/api";
import { bytes } from "../../lib/format";
import { exportPrefs, importPrefs } from "../../lib/prefs";

const ago = (t: number | null) => {
  if (!t) return "Nunca";
  const d = Math.floor((Date.now() / 1000 - t) / 86400);
  return d === 0 ? "Hoy" : d === 1 ? "Ayer" : `Hace ${d} días`;
};

/** Salud de la unidad de datos y copia de seguridad cifrada (a otra unidad o a la nube). */
export function DataSafety() {
  const [h, setH] = useState<StorageHealth | null>(null);
  const [mode, setMode] = useState<"backup" | "restore" | null>(null);
  const [password, setPassword] = useState("");
  const [repeat, setRepeat] = useState("");
  const [machines, setMachines] = useState(false);
  const [reports, setReports] = useState(false);
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => appBackupApi.health().then(setH).catch(() => {}), []);
  useEffect(() => {
    void load();
  }, [load]);

  const close = () => {
    setMode(null);
    setPassword("");
    setRepeat("");
  };

  const backup = async () => {
    if (password.length < 8) return toast("error", "La contraseña debe tener al menos 8 caracteres.");
    if (password !== repeat) return toast("error", "Las contraseñas no coinciden.");
    setBusy(true);
    try {
      const r = await appBackupApi.backup(password, exportPrefs(), machines, reports);
      if (r) {
        toast("ok", `Copia cifrada guardada: ${r.files} archivos (${bytes(r.bytes)}). Guarda la contraseña: sin ella no se puede abrir.`);
        close();
        void load();
      }
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const restore = async () => {
    const ok = await confirm({
      title: "Restaurar una copia",
      body: "Los datos de la copia sustituyen a los actuales (contactos, clientes, ajustes…). Lo que se sobrescribe se guarda antes en una carpeta «antes-de-restaurar». Después se recargará AdminOps.",
      confirmLabel: "Elegir copia y restaurar",
      danger: true,
    });
    if (!ok) return;
    setBusy(true);
    try {
      const r = await appBackupApi.restore(password);
      if (r) {
        if (r.prefs) importPrefs(r.prefs);
        toast("ok", `Restaurados ${r.files} archivos de la copia del ${new Date(r.created * 1000).toLocaleDateString("es")}. Recargando…`);
        close();
        window.setTimeout(() => window.location.reload(), 1500);
      }
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const used = h && h.total ? Math.round(((h.total - h.free) / h.total) * 100) : 0;

  return (
    <Card
      title="Seguridad de tus datos"
      icon={<ShieldCheck size={14} />}
      right={
        <button onClick={load} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
          <RefreshCw size={11} /> Revisar
        </button>
      }
    >
      {!h ? (
        <p className="text-sm text-mute">Revisando…</p>
      ) : (
        <>
          <div className="grid gap-3 text-sm md:grid-cols-4">
            <div>
              <p className="text-[11px] text-mute">{h.portable ? "Unidad del USB" : "Unidad de datos"}</p>
              <p className="flex items-center gap-1.5 text-ink">
                <HardDrive size={13} className="text-mute" /> {h.drive} {h.label && `· ${h.label}`}
              </p>
              <p className="text-[11px] text-dim">
                {h.fileSystem} · {h.health === "Healthy" ? "sana" : h.health || "estado desconocido"}
              </p>
            </div>
            <div>
              <p className="text-[11px] text-mute">Espacio</p>
              <p className="text-ink">{h.total ? `${bytes(h.free)} libres` : "—"}</p>
              {h.total > 0 && (
                <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-line">
                  <div className={`h-full ${used > 90 ? "bg-bad" : "bg-neon"}`} style={{ width: `${used}%` }} />
                </div>
              )}
            </div>
            <div>
              <p className="text-[11px] text-mute">Datos de AdminOps</p>
              <p className="text-ink">{bytes(h.dataBytes)}</p>
              {h.keyPresent !== null && (
                <p className={`flex items-center gap-1 text-[11px] ${h.keyPresent ? "text-dim" : "text-warn"}`}>
                  <KeyRound size={10} /> Clave de contraseñas {h.keyPresent ? "presente" : "aún no creada"}
                </p>
              )}
              {h.keyPresent && h.keyProtected !== null && (
                <p className={`text-[11px] ${h.keyProtected ? "text-ok" : "text-warn"}`}>
                  {h.keyProtected ? "Protegida con tu PIN" : "Sin PIN: activa el bloqueo en Seguridad para protegerla"}
                </p>
              )}
            </div>
            <div>
              <p className="text-[11px] text-mute">Copias</p>
              <p className="text-ink">Cifrada: {ago(h.lastBackup)}</p>
              <p className="text-[11px] text-dim">Contactos: {ago(h.lastContactsBackup)}</p>
            </div>
          </div>
          {h.warnings.length > 0 ? (
            <ul className="mt-3 space-y-1">
              {h.warnings.map((w) => (
                <li key={w} className="flex items-start gap-2 text-xs text-warn">
                  <AlertTriangle size={12} className="mt-0.5 shrink-0" /> {w}
                </li>
              ))}
            </ul>
          ) : (
            <p className="mt-3 flex items-center gap-2 text-xs text-ok">
              <CheckCircle2 size={12} /> Todo en orden.
            </p>
          )}
          <div className="mt-4 flex flex-wrap gap-2 border-t border-line pt-3">
            <Button onClick={() => setMode("backup")}>
              <ShieldCheck size={13} /> Hacer copia cifrada
            </Button>
            <Button kind="ghost" onClick={() => setMode("restore")}>
              <Upload size={13} /> Restaurar una copia
            </Button>
            <p className="basis-full text-[11px] text-mute">
              Guárdala en otra unidad o en una carpeta de OneDrive/Google Drive: si pierdes {h.portable ? "el USB" : "el equipo"}, recuperas contactos, clientes, routers,
              soluciones, plantillas de preparación y ajustes en minutos.
            </p>
          </div>
        </>
      )}

      {mode && (
        <Modal
          title={mode === "backup" ? "Copia de seguridad cifrada" : "Restaurar una copia"}
          onClose={busy ? () => {} : close}
          footer={
            <>
              <Button kind="ghost" onClick={close} disabled={busy}>
                Cancelar
              </Button>
              <Button onClick={mode === "backup" ? backup : restore} disabled={busy || !password}>
                {busy && <Loader2 size={13} className="animate-spin" />}
                {mode === "backup" ? "Elegir dónde guardar" : "Elegir la copia"}
              </Button>
            </>
          }
        >
          <div className="space-y-3">
            <p className="text-sm text-dim">
              {mode === "backup"
                ? "Todo va cifrado con AES-256 y esta contraseña. Sin ella la copia no se puede abrir: apúntala en un sitio seguro."
                : "Escribe la contraseña con la que se hizo la copia."}
            </p>
            <input type="password" autoFocus value={password} onChange={(e) => setPassword(e.target.value)} placeholder="Contraseña de la copia" className={inputClass} />
            {mode === "backup" && (
              <>
                <input type="password" value={repeat} onChange={(e) => setRepeat(e.target.value)} placeholder="Repite la contraseña" className={inputClass} />
                <label className="flex items-center gap-2 text-xs text-dim">
                  <input type="checkbox" checked={machines} onChange={(e) => setMachines(e.target.checked)} className="accent-[var(--color-neon)]" />
                  Incluir lo de cada equipo (diagnósticos, historial): ocupa más
                </label>
                <label className="flex items-center gap-2 text-xs text-dim">
                  <input type="checkbox" checked={reports} onChange={(e) => setReports(e.target.checked)} className="accent-[var(--color-neon)]" />
                  Incluir los informes PDF
                </label>
              </>
            )}
          </div>
        </Modal>
      )}
      {dialog}
    </Card>
  );
}
