// Ajustes → Acerca de.
import { BadgeCheck, BookOpen, Bug, Scale, Sparkles, UserRound, Wrench } from "lucide-react";
import { useEffect, useState } from "react";
import logo from "../../assets/logo.svg";
import { useToast } from "../../components/feedback";
import { Button, Card, inputClass, Modal } from "../../components/ui";
import { appApi, type AppInfo, lockApi, type LockStatus, logQuietly } from "../../lib/api";
import { type AppMode, setPrefs, usePrefs } from "../../lib/prefs";
import { openHelp } from "../../lib/help";

export function About({ appInfo }: { appInfo: AppInfo | null }) {
  const toast = useToast();
  return (
    <Card title="Acerca de" icon={<BadgeCheck size={14} />}>
      <div className="flex items-center gap-3">
        <img src={logo} alt="" className="size-12" />
        <div>
          <div className="font-semibold">AdminOps v{appInfo?.version}</div>
          <div className="text-sm text-dim">
            by <span className="font-semibold text-neon">David Bonilla</span>
          </div>
        </div>
      </div>
      <div className="mt-4 flex flex-wrap gap-2 text-xs">
        {(
          [
            ["data", "Carpeta de datos"],
            ["reports", "Carpeta de informes"],
            ["logs", "Registros"],
          ] as const
        ).map(([kind, label]) => (
          <button
            key={kind}
            onClick={() =>
              appApi.openFolder(kind).catch((e) => toast("error", String(e)))
            }
            className="rounded-md border border-line-2 px-2.5 py-1 text-dim hover:border-neon/40 hover:text-neon"
          >
            {label} →
          </button>
        ))}
      </div>
      <div className="mt-4 grid grid-cols-2 gap-2 border-t border-line/60 pt-4 lg:grid-cols-4">
        {(
          [
            ["guide", "Guía y glosario", BookOpen],
            ["news", "Novedades", Sparkles],
            ["terms", "Términos de uso", Scale],
            ["report", "Reportar un problema", Bug],
          ] as const
        ).map(([tab, label, Icon]) => (
          <button
            key={tab}
            onClick={() => openHelp(tab)}
            className="flex items-center gap-2 rounded-lg border border-line px-3 py-2.5 text-left text-[13px] text-dim transition-colors hover:border-neon/40 hover:text-ink"
          >
            <Icon size={15} strokeWidth={1.7} className="shrink-0 text-neon" /> {label}
          </button>
        ))}
      </div>
      <div className="mt-4 border-t border-line/60 pt-3">
        <Button
          kind="ghost"
          onClick={() =>
            appApi
              .supportPackage()
              .then(() =>
                toast("ok", "Paquete de soporte creado: se abrió su carpeta."),
              )
              .catch((e) => toast("error", String(e)))
          }
        >
          Crear paquete de soporte
        </Button>
        <p className="mt-1.5 text-[11px] text-mute">
          Un .zip con el registro de actividad, el último diagnóstico y la
          versión. «Reportar un problema» lo prepara y lo adjunta al correo
          por ti. Puede contener el nombre del equipo y del usuario: revísalo
          antes de compartirlo.
        </p>
      </div>
      <div className="mt-4 border-t border-line/60 pt-3 text-xs text-dim">
        <div className="mb-1 font-medium text-ink">Atajos de AdminOps</div>
        <div className="grid grid-cols-2 gap-1">
          <span>Ctrl + K · buscar o ejecutar</span>
          <span>Ctrl + , · Ajustes</span>
          <span>Ctrl + L · bloquear (si hay PIN)</span>
          <span>Alt + ← / → · página anterior / siguiente</span>
        </div>
      </div>
    </Card>
  );
}

/**
 * Para quién es esta instalación. Volver al modo técnico pide el PIN si lo hay:
 * así el técnico puede dejar la app a un usuario y que no se lo cambie.
 */
export function ModeCard() {
  const prefs = usePrefs();
  const [lock, setLock] = useState<LockStatus | null>(null);
  const [asking, setAsking] = useState(false);
  const [secret, setSecret] = useState("");
  const [error, setError] = useState<string | null>(null);
  const toast = useToast();

  useEffect(() => {
    lockApi
      .status()
      .then(setLock)
      .catch(logQuietly("SettingsPage"));
  }, []);

  const apply = (mode: AppMode) => {
    setPrefs({ mode });
    toast(
      "ok",
      mode === "admin"
        ? "Modo técnico: AdminOps completo."
        : "Modo usuario: solo lo necesario.",
    );
  };

  const choose = (mode: AppMode) => {
    if (mode === prefs.mode) return;
    // Pasar a técnico con PIN puesto hay que autorizarlo; a usuario, no.
    if (mode === "admin" && lock?.enabled) {
      setSecret("");
      setError(null);
      setAsking(true);
      return;
    }
    apply(mode);
  };

  const unlock = async () => {
    try {
      if (await lockApi.verify(secret)) {
        setAsking(false);
        apply("admin");
      } else setError("No coincide.");
    } catch (e) {
      setError(String(e));
    }
  };

  const card = (id: AppMode, title: string, sub: string) => (
    <button
      key={id}
      onClick={() => choose(id)}
      className={`flex-1 rounded-lg border px-3 py-2.5 text-left transition-colors ${
        prefs.mode === id
          ? "border-neon/60 bg-neon/10"
          : "border-line hover:border-line-2"
      }`}
    >
      <div
        className={`flex items-center gap-2 text-sm font-medium ${prefs.mode === id ? "text-neon" : "text-ink"}`}
      >
        {id === "admin" ? <Wrench size={14} /> : <UserRound size={14} />}
        {title}
      </div>
      <div className="text-[11px] text-mute">{sub}</div>
    </button>
  );

  return (
    <Card title="Para quién es AdminOps en este equipo">
      <div className="flex flex-wrap gap-2">
        {card(
          "admin",
          "Modo técnico",
          "AdminOps completo: diagnóstico, ajustes, usuarios, clientes e informes.",
        )}
        {card(
          "user",
          "Modo usuario",
          "Solo ver cómo está el equipo, resolver lo típico y dar acceso remoto.",
        )}
      </div>
      <p className="mt-3 text-[11px] text-mute">
        {prefs.mode === "user"
          ? "En modo usuario no se ven tus clientes, contactos, tickets ni correo, ni lo que puede romper el equipo."
          : "Si dejas AdminOps a la persona que usa el equipo, pásalo a modo usuario."}{" "}
        Es un modo de la interfaz, no una barrera de seguridad
        {lock?.enabled
          ? ": con el PIN puesto, volver a modo técnico lo pide."
          : "; pon un PIN en Seguridad para que no se pueda volver a técnico sin ti."}
      </p>

      {asking && (
        <Modal
          title="Volver al modo técnico"
          onClose={() => setAsking(false)}
          footer={
            <>
              {error && <p className="mr-auto text-xs text-bad">{error}</p>}
              <Button kind="ghost" onClick={() => setAsking(false)}>
                Cancelar
              </Button>
              <Button onClick={unlock} disabled={!secret}>
                Continuar
              </Button>
            </>
          }
        >
          <label className="block">
            <span className="mb-1 block text-xs text-dim">
              {lock?.kind === "pin"
                ? "PIN de AdminOps"
                : "Contraseña de AdminOps"}
            </span>
            <input
              autoFocus
              type="password"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && secret && unlock()}
              className={inputClass}
            />
          </label>
        </Modal>
      )}
    </Card>
  );
}
