// Copia automática de los datos de AdminOps a OneDrive u otro disco.
//
// En un pendrive, perderlo o que se estropee es perderlo todo: clientes,
// contactos, agenda. La copia cifrada se hace sola cada N días en el equipo
// donde se configura (en otro PC, esa carpeta sería de otra persona).
import { CloudUpload, FolderOpen, Loader2, RefreshCw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../../components/feedback";
import { Button, Card, inputClass } from "../../components/ui";
import { logQuietly, autoBackupApi, disksApi, type AutoBackupInfo } from "../../lib/api";
import { withoutUserPaths } from "../../lib/errors";
import { fullDate } from "../../lib/format";

const when = (t: number | null) => (t ? fullDate(t) : "nunca");

export function AutoBackup() {
  const [info, setInfo] = useState<AutoBackupInfo | null>(null);
  const [folder, setFolder] = useState("");
  const [password, setPassword] = useState("");
  const [every, setEvery] = useState(7);
  const [keep, setKeep] = useState(5);
  const [busy, setBusy] = useState<"save" | "run" | null>(null);
  const toast = useToast();

  const load = useCallback(
    () =>
      autoBackupApi
        .info()
        .then((i) => {
          setInfo(i);
          setFolder(i.folder);
          setEvery(i.everyDays);
          setKeep(i.keep);
        })
        .catch(logQuietly("AutoBackup")),
    [],
  );
  useEffect(() => {
    void load();
  }, [load]);

  if (!info) return null;

  const save = async (off = false) => {
    setBusy("save");
    try {
      await autoBackupApi.set(off ? "" : folder, password, every, keep);
      setPassword("");
      toast("ok", off ? "Copia automática desactivada." : "Copia automática configurada.");
      await load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const runNow = async () => {
    setBusy("run");
    try {
      const r = await autoBackupApi.runNow();
      toast("ok", `Copia hecha: ${r.files} archivos.`);
      await load();
    } catch (e) {
      toast("error", String(e));
      await load();
    } finally {
      setBusy(null);
    }
  };

  const pick = async () => {
    const f = await disksApi.pickFolder().catch(() => null);
    if (f) setFolder(f);
  };

  return (
    <Card title="Copia automática" icon={<CloudUpload size={14} />}>
      <p className="text-xs text-dim">
        Una copia cifrada de tus datos (ajustes, clientes, contactos, agenda, portales) cada pocos días, en una carpeta de OneDrive o en otro disco. Si pierdes el
        pendrive, la recuperas en Seguridad de tus datos → Restaurar.
      </p>

      {info.enabled && (
        <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
          <span className="text-dim">
            Última: <b className="text-ink">{when(info.lastBackup)}</b>
          </span>
          {info.nextDue && info.here && (
            <span className="text-dim">
              Próxima: <b className="text-ink">{when(info.nextDue)}</b>
            </span>
          )}
          {!info.here && <span className="text-warn">Configurada en el equipo {info.configuredOn}: solo allí se hace sola.</span>}
          <Button kind="ghost" onClick={() => void runNow()} disabled={busy !== null}>
            {busy === "run" ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />} Hacer una ahora
          </Button>
        </div>
      )}
      {info.lastError && (
        <p className="mt-2 flex items-start gap-1.5 text-xs text-warn">
          <TriangleAlert size={13} className="mt-0.5 shrink-0" /> La última copia falló: {info.lastError}
        </p>
      )}

      <div className="mt-3 grid gap-3 sm:grid-cols-2">
        <div>
          <div className="mb-1 text-[11px] text-mute">Carpeta</div>
          <div className="flex gap-1.5">
            <button onClick={() => void pick()} className="flex min-w-0 flex-1 items-center gap-2 rounded-md border border-line bg-void/60 px-3 py-2 text-left text-sm text-ink hover:border-neon/50">
              <FolderOpen size={14} className="shrink-0 text-mute" />
              <span className="truncate">{folder ? withoutUserPaths(folder) : "Elegir carpeta…"}</span>
            </button>
          </div>
          {info.onedrive && folder !== info.onedrive && (
            <button onClick={() => setFolder(info.onedrive ?? "")} className="mt-1 text-[11px] text-neon hover:underline">
              Usar OneDrive
            </button>
          )}
        </div>
        <label className="text-[11px] text-mute">
          Contraseña de la copia {info.enabled && "(vacía: la de siempre)"}
          <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="new-password" placeholder="Al menos 8 caracteres" className={`${inputClass} mt-1`} />
        </label>
        <label className="text-[11px] text-mute">
          Cada
          <select value={every} onChange={(e) => setEvery(Number(e.target.value))} className={`${inputClass} mt-1`}>
            {[1, 3, 7, 14, 30].map((d) => (
              <option key={d} value={d}>
                {d === 1 ? "día" : `${d} días`}
              </option>
            ))}
          </select>
        </label>
        <label className="text-[11px] text-mute">
          Copias que se guardan
          <select value={keep} onChange={(e) => setKeep(Number(e.target.value))} className={`${inputClass} mt-1`}>
            {[3, 5, 10, 20].map((k) => (
              <option key={k} value={k}>
                Las {k} últimas
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <Button onClick={() => void save()} disabled={busy !== null || !folder || (!info.enabled && password.length < 8)}>
          {busy === "save" && <Loader2 size={14} className="animate-spin" />} {info.enabled ? "Guardar cambios" : "Activar"}
        </Button>
        {info.enabled && (
          <Button kind="ghost" onClick={() => void save(true)} disabled={busy !== null}>
            Desactivar
          </Button>
        )}
        <span className="text-[11px] text-mute">Apunta la contraseña en un sitio seguro: sin ella la copia no se puede abrir.</span>
      </div>
    </Card>
  );
}
