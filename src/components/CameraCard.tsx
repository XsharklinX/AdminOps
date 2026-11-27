import { Camera, RefreshCw, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { camApi, type CamView } from "../lib/api/camera";
import { useToast } from "./feedback";
import { Button, Card, Notice } from "./ui";

/** Una vista previa en vivo de una cámara (la pide la propia interfaz). */
function Preview({ deviceId, label }: { deviceId: string; label: string }) {
  const ref = useRef<HTMLVideoElement>(null);
  const [state, setState] = useState<"wait" | "ok" | "busy" | "denied">("wait");
  useEffect(() => {
    let stream: MediaStream | null = null;
    let alive = true;
    navigator.mediaDevices
      .getUserMedia({ video: { deviceId: { exact: deviceId }, width: 320, height: 180 }, audio: false })
      .then((s) => {
        if (!alive) return s.getTracks().forEach((t) => t.stop());
        stream = s;
        if (ref.current) ref.current.srcObject = s;
        setState("ok");
      })
      .catch((e: DOMException) => setState(e.name === "NotAllowedError" ? "denied" : "busy"));
    return () => {
      alive = false;
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, [deviceId]);
  return (
    <div className="min-w-0">
      <div className={`relative aspect-video overflow-hidden rounded-lg border ${state === "ok" ? "border-ok/50" : "border-line"} bg-void`}>
        <video ref={ref} autoPlay muted playsInline className={`h-full w-full object-cover ${state === "ok" ? "" : "hidden"}`} />
        {state !== "ok" && (
          <span className="absolute inset-0 grid place-items-center px-2 text-center text-[11px] text-mute">
            {state === "wait" ? "Abriendo…" : state === "denied" ? "Windows no deja usarla (permiso)" : "No da imagen: ocupada por otro programa o tapada"}
          </span>
        )}
      </div>
      <div className="mt-1 truncate text-sm text-ink">{label || "Cámara"}</div>
      {state === "ok" && <div className="text-[11px] text-ok">te ve</div>}
    </div>
  );
}

/**
 * «No me ven en Teams»: cada cámara con su vista previa, los permisos que falten, quién la tiene ocupada y,
 * como Windows no tiene cámara predeterminada, desactivar la que sobra para que Teams use la buena.
 */
export function CameraCard({ isAdmin }: { isAdmin: boolean }) {
  const [view, setView] = useState<CamView | null>(null);
  const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [round, setRound] = useState(0);
  const toast = useToast();

  const load = useCallback(() => {
    camApi
      .view()
      .then((v) => {
        setView(v);
        setError("");
      })
      .catch((e) => setError(String(e)));
    // Los nombres de las cámaras solo los da el navegador tras pedir permiso una vez.
    navigator.mediaDevices
      ?.getUserMedia({ video: true })
      .then((s) => s.getTracks().forEach((t) => t.stop()))
      .catch(() => {})
      .finally(() =>
        navigator.mediaDevices
          .enumerateDevices()
          .then((d) => setDevices(d.filter((x) => x.kind === "videoinput")))
          .catch(() => setDevices([])),
      );
  }, []);
  useEffect(load, [load, round]);

  if (error) return <Card title="No me ven (cámara)" icon={<Camera size={14} />} className="col-span-12"><Notice tone="error" where="Cámara">{error}</Notice></Card>;
  if (!view) return null;

  const p = view.privacy;
  const perms = [
    !p.device && "La cámara está apagada para todo el equipo.",
    p.device && !p.apps && "Windows no deja usar la cámara a las aplicaciones.",
    p.device && p.apps && !p.desktop && "Las aplicaciones de escritorio (Teams clásico, Zoom, el navegador) no tienen permiso de cámara.",
    p.device && p.apps && p.teamsNew === false && "Teams no tiene permiso de cámara.",
  ].filter(Boolean) as string[];
  const act = async (key: string, f: () => Promise<unknown>, ok: string) => {
    setBusy(key);
    try {
      await f();
      toast("ok", ok);
      setRound((r) => r + 1);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };
  return (
    <Card
      title="No me ven (cámara)"
      icon={<Camera size={14} />}
      className="col-span-12"
      right={
        <button type="button" onClick={() => setRound((r) => r + 1)} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
          <RefreshCw size={11} /> Volver a mirar
        </button>
      }
    >
      {(perms.length > 0 || view.inUse.length > 0) && (
        <div className="mb-3 rounded-lg border border-warn/40 bg-warn/5 px-3 py-2 text-sm text-ink">
          {perms.map((t) => (
            <p key={t}>{t}</p>
          ))}
          {view.inUse.length > 0 && <p>La está usando ahora: {view.inUse.join(", ")}. Mientras tanto, otro programa no puede.</p>}
          {perms.length > 0 && (
            <div className="mt-2">
              <Button size="sm" disabled={busy !== null} onClick={() => act("perm", camApi.allow, "Permisos de cámara dados.")}>
                <ShieldCheck size={13} /> Dar permiso de cámara
              </Button>
            </div>
          )}
        </div>
      )}
      {devices.length > 0 ? (
        <div className="grid grid-cols-2 gap-3 md:grid-cols-3" key={round}>
          {devices.map((d) => (
            <Preview key={d.deviceId} deviceId={d.deviceId} label={d.label} />
          ))}
        </div>
      ) : (
        <p className="text-sm text-mute">{view.cameras.length ? "No se pudo abrir ninguna cámara para verla." : "No hay ninguna cámara conectada."}</p>
      )}
      {view.cameras.length > 1 && (
        <div className="mt-4">
          <div className="mb-1 text-[11px] font-medium uppercase tracking-wide text-mute">Qué cámara usa Teams</div>
          <p className="mb-2 text-xs text-mute">Windows no tiene cámara predeterminada y Teams suele quedarse con la primera. Desactiva la que no se usa y Teams cogerá la otra; se vuelve a activar igual.</p>
          <div className="space-y-1.5">
            {view.cameras.map((c) => (
              <div key={c.id} className="flex items-center gap-3 rounded-lg border border-line bg-panel-2/40 px-3 py-2 text-sm">
                <Camera size={14} className={c.state === "ok" ? "text-ok" : "text-mute"} />
                <span className="min-w-0 flex-1 truncate text-ink">{c.name}</span>
                <span className={`text-[11px] ${c.state === "ok" ? "text-ok" : c.state === "error" ? "text-bad" : "text-mute"}`}>{c.state === "ok" ? "activa" : c.state === "error" ? "con error" : "desactivada"}</span>
                <Button size="sm" kind="ghost" disabled={!isAdmin || busy !== null} onClick={() => act(c.id, () => camApi.enable(c.id, c.state !== "ok"), c.state === "ok" ? `«${c.name}» desactivada.` : `«${c.name}» activada.`)}>
                  {busy === c.id ? "…" : c.state === "ok" ? "Desactivar esta" : "Activar"}
                </Button>
              </div>
            ))}
          </div>
          {!isAdmin && <Notice tone="warn" className="mt-1">Para activar o desactivar cámaras hay que abrir AdminOps como administrador.</Notice>}
        </div>
      )}
    </Card>
  );
}
