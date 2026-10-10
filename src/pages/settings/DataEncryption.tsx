// Ajustes → Seguridad → Cifrar mis datos.
import { Check, ClipboardCopy, Download, ShieldCheck, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../../components/feedback";
import { Button, Card, Modal, inputClass, smallBtn } from "../../components/ui";
import { dataCryptApi, knowledgeApi, lockApi, logQuietly, type DataCryptStatus, type LockStatus } from "../../lib/api";

export function DataEncryption() {
  const toast = useToast();
  const [st, setSt] = useState<DataCryptStatus | null>(null);
  const [lock, setLock] = useState<LockStatus | null>(null);
  const [asking, setAsking] = useState<"enable" | "disable" | null>(null);
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [recovery, setRecovery] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [rescueKey, setRescueKey] = useState("");

  const load = useCallback(() => {
    dataCryptApi.status().then(setSt, logQuietly("DataEncryption"));
    lockApi.status().then(setLock, logQuietly("DataEncryption"));
  }, []);
  useEffect(() => {
    load();
    // El bloqueo puede cambiar en la tarjeta de al lado.
    window.addEventListener("adminops-lock", load);
    return () => window.removeEventListener("adminops-lock", load);
  }, [load]);

  const close = () => {
    setAsking(null);
    setSecret("");
  };
  const run = async () => {
    setBusy(true);
    try {
      if (asking === "enable") {
        setRecovery(await dataCryptApi.enable(secret));
        setSaved(false);
      } else {
        const n = await dataCryptApi.disable(secret);
        toast("ok", `Cifrado quitado: ${n} ${n === 1 ? "archivo descifrado" : "archivos descifrados"}.`);
      }
      close();
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  const unlock = async () => {
    try {
      if (await dataCryptApi.unlockRecovery(rescueKey)) {
        toast("ok", "Datos abiertos con la clave de rescate.");
        setRescueKey("");
        load();
      } else toast("error", "Esa clave de rescate no es correcta.");
    } catch (e) {
      toast("error", String(e));
    }
  };
  const copyKey = () => void navigator.clipboard.writeText(recovery ?? "").then(() => toast("ok", "Clave copiada."), () => toast("error", "No se pudo copiar."));
  const saveKey = () =>
    void knowledgeApi
      .saveText("AdminOps-clave-de-rescate", `Clave de rescate de los datos de AdminOps\n\n${recovery}\n\nGuárdala fuera de este equipo. Sin el PIN o la contraseña y sin esta clave, los datos cifrados no se pueden recuperar.\n`, "txt")
      .then((p) => p && toast("ok", "Guardada. Llévala a un sitio seguro, fuera de este equipo."), (e) => toast("error", String(e)));

  if (!st || !lock) return null;
  return (
    <Card title="Cifrar mis datos" icon={<ShieldCheck size={14} />}>
      <p className="mb-3 text-xs text-dim">
        Clientes, contactos, notas, casos, seguimientos, agenda y lo que sabes de cada equipo se guardan cifrados (AES-256). Si se pierde el pendrive o roban el equipo, no se puede leer nada sin tu PIN o contraseña. Los ajustes y la pantalla de bloqueo quedan en claro; los informes PDF no se cifran.
      </p>
      {!st.available ? (
        <p className="flex items-center gap-2 rounded-lg border border-line px-3 py-2 text-sm text-dim">
          <TriangleAlert size={14} className="shrink-0 text-warn" /> Primero activa el bloqueo con PIN o contraseña, arriba: con él se abre la clave de los datos.
        </p>
      ) : !st.enabled ? (
        <>
          {lock.kind === "pin" && (
            <p className="mb-3 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn">
              <TriangleAlert size={14} className="mt-0.5 shrink-0" /> Un PIN de 4 a 8 números se puede probar entero con tiempo si alguien se lleva el pendrive. Para una protección de verdad, cambia el bloqueo a una contraseña larga antes de cifrar.
            </p>
          )}
          <Button onClick={() => setAsking("enable")}>
            <ShieldCheck size={14} /> Cifrar mis datos…
          </Button>
        </>
      ) : (
        <div className="space-y-3">
          <p className="flex items-center gap-2 text-sm text-ok">
            <Check size={14} /> Cifrados: {st.sealed} de {st.files} {st.files === 1 ? "archivo" : "archivos"} de datos.
          </p>
          {!st.unlocked && (
            <div className="rounded-lg border border-warn/30 bg-warn/5 p-3">
              <p className="mb-2 text-xs text-warn">Los datos están cerrados en esta sesión (entraste con la contraseña de Windows). Escribe la clave de rescate para abrirlos.</p>
              <div className="flex flex-wrap items-center gap-2">
                <input value={rescueKey} onChange={(e) => setRescueKey(e.target.value)} placeholder="XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" className={`${inputClass} w-72 font-mono`} aria-label="Clave de rescate" />
                <Button onClick={unlock} disabled={rescueKey.trim().length < 10}>
                  Abrir con la clave
                </Button>
              </div>
            </div>
          )}
          <Button kind="ghost" onClick={() => setAsking("disable")}>
            Quitar el cifrado…
          </Button>
        </div>
      )}

      {asking && (
        <Modal
          title={asking === "enable" ? "Cifrar mis datos" : "Quitar el cifrado"}
          onClose={close}
          footer={
            <>
              <Button kind="ghost" onClick={close}>
                Cancelar
              </Button>
              <Button onClick={run} disabled={busy || !secret}>
                {asking === "enable" ? "Cifrar" : "Quitar el cifrado"}
              </Button>
            </>
          }
        >
          <p className="mb-3 text-sm text-dim">
            {asking === "enable"
              ? "Escribe tu PIN o contraseña de AdminOps. Después se te dará una clave de rescate, que se enseña una sola vez: si olvidas el PIN y la pierdes, los datos no se pueden recuperar."
              : "Escribe tu PIN o contraseña de AdminOps para descifrar todo y quitar las claves."}
          </p>
          <input autoFocus type="password" value={secret} onChange={(e) => setSecret(e.target.value)} className={inputClass} aria-label="PIN o contraseña" onKeyDown={(e) => e.key === "Enter" && secret && !busy && void run()} />
        </Modal>
      )}

      {recovery && (
        <Modal
          title="Tu clave de rescate"
          onClose={() => saved && setRecovery(null)}
          footer={
            <Button onClick={() => setRecovery(null)} disabled={!saved}>
              Ya la he guardado
            </Button>
          }
        >
          <p className="mb-3 text-sm text-dim">Los datos ya están cifrados. Esta es la única forma de abrirlos si olvidas el PIN o entras con la contraseña de Windows. No se vuelve a enseñar.</p>
          <div className="rounded-lg border border-line-2 bg-void px-4 py-3 text-center font-mono text-lg tracking-wider text-ink select-text">{recovery}</div>
          <div className="mt-3 flex flex-wrap gap-2">
            <button className={smallBtn} onClick={copyKey}>
              <ClipboardCopy size={12} /> Copiar
            </button>
            <button className={smallBtn} onClick={saveKey}>
              <Download size={12} /> Guardar en un archivo
            </button>
          </div>
          <label className="mt-4 flex items-start gap-2 text-sm text-dim">
            <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} className="mt-1 accent-[var(--color-neon)]" />
            He guardado la clave de rescate fuera de este equipo (un gestor de contraseñas o un papel).
          </label>
        </Modal>
      )}
    </Card>
  );
}
