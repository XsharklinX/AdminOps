// Apuestas grandes (1.2.8): programas con fallos de seguridad conocidos y el
// pendrive de rescate arrancable para los equipos que no arrancan.
import { CheckCircle2, Download, LifeBuoy, RefreshCw, ShieldAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "./feedback";
import { TypedConfirm } from "./DiskPartitions";
import { TaskStatus } from "./TaskStatus";
import { Button, Card, ErrorState, Loading, smallBtn } from "./ui";
import { appsApi, bigApi, toolsApi, type RescueStatus, type VulnReport } from "../lib/api";
import { bytes } from "../lib/format";

// ---------- Programas desactualizados con fallos conocidos ----------

export function VulnerableCard() {
  const toast = useToast();
  const [report, setReport] = useState<VulnReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const load = useCallback(() => {
    setError(null);
    bigApi.vulnerable().then(setReport, (e) => setError(String(e)));
  }, []);
  useEffect(load, [load]);

  const upgrade = async (ids: string[]) => {
    setBusy(ids.join(","));
    try {
      const r = await toolsApi.upgradeSoftware(ids);
      const ok = r.filter((x) => x.ok).length;
      toast(ok === r.length ? "ok" : "info", ok === r.length ? `${ok} programa(s) actualizados.` : `${ok} de ${r.length} actualizados. ${r.find((x) => !x.ok)?.message ?? ""}`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  const updatable = (report?.hits ?? []).filter((h) => h.winget).map((h) => h.winget);
  return (
    <Card
      title="Programas con fallos de seguridad conocidos"
      icon={<ShieldAlert size={14} />}
      right={
        <span className="flex items-center gap-2">
          {updatable.length > 1 && (
            <button className={smallBtn} disabled={!!busy} onClick={() => void upgrade(updatable)}>
              <Download size={12} /> Actualizar todos
            </button>
          )}
          <button className={smallBtn} onClick={load} title="Volver a mirar">
            <RefreshCw size={12} />
          </button>
        </span>
      }
    >
      {error ? (
        <ErrorState message={error} onRetry={load} where="Programas con fallos conocidos" />
      ) : !report ? (
        <Loading text="Cruzando los programas instalados con los fallos conocidos…" />
      ) : report.hits.length === 0 ? (
        <p className="flex items-center gap-2 text-sm text-dim">
          <CheckCircle2 size={14} className="text-ok" /> Ninguno de los {report.checked} programas instalados tiene fallos conocidos en la base de AdminOps.
        </p>
      ) : (
        <ul className="space-y-2">
          {report.hits.map((h) => (
            <li key={h.name + h.version} className={`rounded-lg border px-3 py-2 ${h.exploited ? "border-bad/40 bg-bad/5" : h.eol ? "border-warn/40 bg-warn/5" : "border-line"}`}>
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm text-ink">{h.name}</span>
                <span className="font-mono text-xs text-mute">{h.version}</span>
                {h.fixed && <span className="font-mono text-xs text-dim">→ {h.fixed} o posterior</span>}
                {h.exploited && <span className="rounded bg-bad/15 px-1.5 py-px text-[10px] font-medium text-bad uppercase">Se está explotando</span>}
                {h.eol && <span className="rounded bg-warn/15 px-1.5 py-px text-[10px] font-medium text-warn uppercase">Sin soporte</span>}
                <span className="ml-auto flex items-center gap-2">
                  {h.cve && <span className="font-mono text-[11px] text-mute">{h.cve}</span>}
                  {h.winget && (
                    <button className={smallBtn} disabled={!!busy} onClick={() => void upgrade([h.winget])}>
                      <Download size={12} /> {busy?.split(",").includes(h.winget) ? "Actualizando…" : "Actualizar"}
                    </button>
                  )}
                </span>
              </div>
              <p className="mt-1 text-xs text-dim">{h.eol ? `${h.what} Hay que cambiarlo por una versión actual o desinstalarlo.` : h.what}</p>
            </li>
          ))}
        </ul>
      )}
      {busy && <TaskStatus task="software" active fallback="Actualizando con winget…" />}
      {report && <p className="mt-3 text-[11px] text-mute">Base de fallos de AdminOps hasta {report.dbDate}: primero lo que se está usando en ataques reales. No sustituye a un antivirus ni a Windows Update; lo más nuevo sale en «Actualizar» de abajo.</p>}
    </Card>
  );
}

// ---------- Pendrive de rescate arrancable ----------

export function RescueUsbCard({ isAdmin }: { isAdmin: boolean }) {
  const toast = useToast();
  const [st, setSt] = useState<RescueStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [disk, setDisk] = useState<number | null>(null);
  const [asking, setAsking] = useState(false);
  const [running, setRunning] = useState(false);
  const [installing, setInstalling] = useState(false);
  const load = useCallback(() => {
    setError(null);
    bigApi.rescueStatus().then(
      (s) => {
        setSt(s);
        setDisk((d) => (s.usb.some((u) => u.number === d) ? d : (s.usb.find((u) => !u.system)?.number ?? null)));
      },
      (e) => setError(String(e)),
    );
  }, []);
  useEffect(load, [load]);

  const installAdk = async () => {
    setInstalling(true);
    try {
      const r = await appsApi.install([
        { id: "Microsoft.WindowsADK", name: "Windows ADK", category: "Herramientas", source: "winget" },
        { id: "Microsoft.ADKPEAddon", name: "Complemento WinPE del ADK", category: "Herramientas", source: "winget" },
      ]);
      const bad = r.filter((x) => !x.ok);
      toast(bad.length ? "error" : "ok", bad.length ? `No se pudo instalar: ${bad.map((x) => x.name).join(", ")}` : "Windows ADK y WinPE instalados.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setInstalling(false);
      load();
    }
  };

  const create = async () => {
    if (disk === null) return;
    setAsking(false);
    setRunning(true);
    try {
      toast("ok", await bigApi.rescueCreate(disk));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(false);
      load();
    }
  };

  const chosen = st?.usb.find((u) => u.number === disk);
  const ready = !!st && st.adk && st.winpe;
  return (
    <Card title="Pendrive de rescate arrancable" icon={<LifeBuoy size={14} />} right={<button className={smallBtn} onClick={load} title="Volver a mirar los pendrives"><RefreshCw size={12} /></button>}>
      <p className="mb-3 text-xs text-dim">
        Para el equipo que no arranca: un Windows mínimo (WinPE) en un pendrive con el menú de rescate de AdminOps. Copiar los datos de un usuario a otro disco, ver la salud de los discos, reparar el arranque, pasar chkdsk, desactivar Driver Verifier o quitar el driver que provoca el pantallazo, sin arrancar el Windows dañado.
      </p>
      {error ? (
        <ErrorState message={error} onRetry={load} where="Pendrive de rescate" />
      ) : !st ? (
        <Loading text="Buscando el Windows ADK y los pendrives…" />
      ) : (
        <div className="space-y-3">
          <ol className="space-y-1.5 text-sm">
            <li className="flex items-center gap-2">
              {ready ? <CheckCircle2 size={14} className="text-ok" /> : <span className="size-3.5 rounded-full border border-line-2" />}
              <span className={ready ? "text-dim" : "text-ink"}>Windows ADK y su complemento WinPE (herramientas gratuitas de Microsoft, unos 4 GB)</span>
              {!ready && (
                <Button onClick={installAdk} disabled={installing || running}>
                  <Download size={13} /> {installing ? "Instalando…" : "Instalar"}
                </Button>
              )}
            </li>
            <li className="flex flex-wrap items-center gap-2">
              {chosen ? <CheckCircle2 size={14} className="text-ok" /> : <span className="size-3.5 rounded-full border border-line-2" />}
              <span className="text-ink">Pendrive de 1 GB o más (se borra lo que tenga)</span>
              {st.usb.length === 0 ? (
                <span className="text-xs text-mute">No hay ninguno conectado.</span>
              ) : (
                <select value={disk ?? ""} onChange={(e) => setDisk(Number(e.target.value))} className="rounded-md border border-line bg-panel px-2 py-1 text-xs text-ink">
                  {st.usb.map((u) => (
                    <option key={u.number} value={u.number} disabled={u.system}>
                      {u.letters.map((l) => `${l}:`).join(" ")} {u.name} · {bytes(u.size)}
                      {u.system ? " (sistema)" : ""}
                    </option>
                  ))}
                </select>
              )}
            </li>
          </ol>
          {running ? (
            <TaskStatus task="rescue" active fallback="Creando el pendrive (10–20 minutos)…" />
          ) : (
            <Button onClick={() => setAsking(true)} disabled={!ready || !chosen || !isAdmin}>
              <LifeBuoy size={13} /> Crear el pendrive de rescate
            </Button>
          )}
          {!isAdmin && <p className="text-xs text-warn">Requiere ejecutar AdminOps como administrador.</p>}
          <p className="text-[11px] text-mute">
            Para usarlo: enciende el equipo con el pendrive puesto y pulsa la tecla del menú de arranque (F12, F8, F11 o Esc según la marca). La ventana completa de AdminOps no funciona dentro de WinPE porque le falta WebView2: el menú de rescate es de texto, pero hace lo importante.
          </p>
        </div>
      )}
      {asking && chosen && (
        <TypedConfirm
          title="Crear el pendrive de rescate"
          body={`Se borrará todo lo que hay en ${chosen.letters.map((l) => `${l}:`).join(" ")} ${chosen.name} (${bytes(chosen.size)}) y se convertirá en un pendrive arrancable.`}
          word="BORRAR"
          confirmLabel="Crear"
          touches={[`Todo el contenido de ${chosen.name}`]}
          keeps={["Los discos de este equipo"]}
          checks={["He copiado lo que había en el pendrive o no me hace falta.", "He comprobado que es el pendrive correcto (nombre y tamaño)."]}
          onClose={() => setAsking(false)}
          onConfirm={() => void create()}
        />
      )}
    </Card>
  );
}
