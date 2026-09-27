import { Building2, CircleCheck, CircleX, Eye, EyeOff, Loader2, Pencil, Power, RefreshCw, ShieldCheck, TriangleAlert, Unlink, Wrench } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, Modal, inputClass } from "../components/ui";
import { domainApi, toolboxApi, tweaksApi, workApi, type DomainCheck, type DomainStatus } from "../lib/api";

const LAST_DOMAIN = "adminops.lastDomain";

const remembered = () => {
  try {
    return localStorage.getItem(LAST_DOMAIN) ?? "";
  } catch {
    return "";
  }
};

export function Domain({ isAdmin }: { isAdmin: boolean }) {
  const [s, setS] = useState<DomainStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dialogKind, setDialogKind] = useState<"repair" | "leave" | "rename" | null>(null);
  const [needsRestart, setNeedsRestart] = useState<string | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setS(await domainApi.status());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const restart = async () => {
    const ok = await confirm({
      title: "Reiniciar ahora",
      body: "El equipo se reiniciará inmediatamente para aplicar el cambio. Guarda antes todo lo abierto.",
      confirmLabel: "Reiniciar",
      danger: true,
    });
    if (ok) toolboxApi.launch("boot-restart").catch((e) => toast("error", String(e)));
  };

  const syncTime = async () => {
    try {
      await tweaksApi.run("repair.time-sync");
      toast("ok", "Hora sincronizada.");
      load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (error) return <p className="p-8 text-sm text-bad">{error}</p>;
  if (!s)
    return (
      <p className="flex items-center gap-2 p-8 text-sm text-mute">
        <Loader2 size={14} className="animate-spin" /> Comprobando el dominio (puede tardar si el controlador no responde)…
      </p>
    );

  const skewBad = s.timeOffset !== null && Math.abs(s.timeOffset) > 300;

  return (
    <div className="mx-auto max-w-5xl space-y-4 p-6">
      {!isAdmin && (
        <p className="flex items-center gap-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn">
          <TriangleAlert size={13} /> Sin administrador solo puedes ver el estado. Unir, salir, reparar o renombrar requiere administrador.
        </p>
      )}
      {needsRestart && (
        <div className="flex items-center gap-3 rounded-lg border border-neon/40 bg-neon/5 px-4 py-3">
          <CircleCheck size={16} className="text-ok" />
          <p className="flex-1 text-sm text-ink">{needsRestart} Reinicia el equipo para completarlo.</p>
          <Button onClick={restart}>
            <Power size={14} /> Reiniciar ahora
          </Button>
        </div>
      )}

      <Card
        title="Estado"
        icon={<Building2 size={14} />}
        right={
          <button onClick={load} disabled={loading} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
            <RefreshCw size={11} className={loading ? "animate-spin" : ""} /> Volver a comprobar
          </button>
        }
      >
        <div className="mb-4 flex items-center gap-3">
          <span
            className={`rounded-md border px-2.5 py-1 text-sm font-medium ${
              s.partOfDomain ? "border-neon/50 bg-neon/10 text-neon" : s.azureAdJoined ? "border-neon-2/50 text-neon-2" : "border-line text-dim"
            }`}
          >
            {s.partOfDomain ? `Dominio: ${s.domain}` : s.azureAdJoined ? "Entra ID (Azure AD)" : `Grupo de trabajo: ${s.workgroup ?? "—"}`}
          </span>
          <span className="text-sm text-dim">
            Equipo <span className="font-mono text-ink">{s.computerName}</span>
          </span>
        </div>
        <div className="grid grid-cols-2 gap-x-8 gap-y-2 text-sm lg:grid-cols-3">
          <Field label="Edición">
            {s.caption.replace(/^Microsoft /, "")}
            {!s.canJoin && <span className="block text-xs text-warn">Home: no puede unirse a un dominio</span>}
          </Field>
          <Field label="Usuario de la sesión">{s.userIsDomain === null ? "—" : s.userIsDomain ? "Cuenta del dominio" : "Cuenta local"}</Field>
          {s.azureAdJoined && <Field label="Inquilino de Entra ID">{s.tenant ?? "—"}</Field>}
          {s.partOfDomain && (
            <>
              <Field label="Controlador de dominio">
                {s.dc ?? <span className="text-warn">No encontrado (¿fuera de la red de la empresa?)</span>}
              </Field>
              <Field label="Relación de confianza">
                {s.secureChannel === null ? (
                  <span className="text-mute">{isAdmin ? "No se pudo comprobar" : "Requiere administrador"}</span>
                ) : s.secureChannel ? (
                  <span className="flex items-center gap-1 text-ok">
                    <CircleCheck size={13} /> Correcta
                  </span>
                ) : (
                  <span className="flex items-center gap-1 text-bad">
                    <CircleX size={13} /> Rota: repárala
                  </span>
                )}
              </Field>
              <Field label="Diferencia de hora">
                {s.timeOffset === null ? (
                  "—"
                ) : (
                  <span className={skewBad ? "text-bad" : Math.abs(s.timeOffset) > 60 ? "text-warn" : "text-ok"}>
                    {Math.abs(s.timeOffset).toFixed(1)} s{skewBad && " (más de 5 min: impide iniciar sesión)"}
                  </span>
                )}
              </Field>
            </>
          )}
        </div>
        <div className="mt-4 flex flex-wrap gap-2 border-t border-line/60 pt-3">
          {s.partOfDomain && (
            <>
              <Button kind={s.secureChannel === false ? "primary" : "ghost"} onClick={() => setDialogKind("repair")} disabled={!isAdmin}>
                <Wrench size={14} /> Reparar relación de confianza
              </Button>
              <Button kind="ghost" onClick={syncTime} disabled={!isAdmin}>
                <RefreshCw size={14} /> Sincronizar hora
              </Button>
              <Button kind="ghost" onClick={() => setDialogKind("leave")} disabled={!isAdmin}>
                <Unlink size={14} /> Salir del dominio
              </Button>
            </>
          )}
          <Button kind="ghost" onClick={() => setDialogKind("rename")} disabled={!isAdmin}>
            <Pencil size={14} /> Cambiar nombre del equipo
          </Button>
        </div>
      </Card>

      {!s.partOfDomain && (
        <JoinCard
          status={s}
          isAdmin={isAdmin}
          onJoined={(msg) => {
            setNeedsRestart(msg);
            load();
          }}
        />
      )}

      {dialogKind && (
        <CredentialsDialog
          kind={dialogKind}
          status={s}
          onClose={() => setDialogKind(null)}
          onDone={(msg, restartNeeded) => {
            setDialogKind(null);
            toast("ok", msg);
            if (restartNeeded) setNeedsRestart(msg);
            load();
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="min-w-0">
      <div className="text-[11px] text-mute">{label}</div>
      <div className="text-ink">{children}</div>
    </div>
  );
}

function Password({ value, onChange, placeholder = "Contraseña" }: { value: string; onChange: (v: string) => void; placeholder?: string }) {
  const [show, setShow] = useState(false);
  return (
    <div className="relative">
      <input
        type={show ? "text" : "password"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoComplete="off"
        className={`${inputClass} pr-8`}
      />
      <button type="button" onClick={() => setShow(!show)} className="absolute top-1/2 right-2 -translate-y-1/2 text-mute hover:text-ink">
        {show ? <EyeOff size={13} /> : <Eye size={13} />}
      </button>
    </div>
  );
}

const ICON = { ok: CircleCheck, warn: TriangleAlert, fail: CircleX };
const COLOR = { ok: "text-ok", warn: "text-warn", fail: "text-bad" };

function JoinCard({ status, isAdmin, onJoined }: { status: DomainStatus; isAdmin: boolean; onJoined: (msg: string) => void }) {
  const [domain, setDomain] = useState(remembered);
  const [user, setUser] = useState("");
  const [password, setPassword] = useState("");
  const [newName, setNewName] = useState("");
  const [ou, setOu] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const [checks, setChecks] = useState<DomainCheck[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [joining, setJoining] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Sin dominio recordado, el habitual de Ajustes.
  useEffect(() => {
    workApi
      .settings()
      .then((s) => setDomain((d) => d || s.defaultDomain))
      .catch(() => {});
  }, []);

  const check = async () => {
    setChecking(true);
    setError(null);
    setChecks(null);
    try {
      setChecks(await domainApi.check(domain));
    } catch (e) {
      setError(String(e));
    } finally {
      setChecking(false);
    }
  };

  const join = async () => {
    setJoining(true);
    setError(null);
    try {
      await domainApi.join({ domain, user, password, ou, newName });
      try {
        localStorage.setItem(LAST_DOMAIN, domain.trim());
      } catch {
        /* sin almacenamiento */
      }
      setPassword("");
      onJoined(`El equipo se unió al dominio ${domain.trim()}${newName.trim() ? ` como ${newName.trim().toUpperCase()}` : ""}.`);
    } catch (e) {
      setError(String(e));
    } finally {
      setJoining(false);
    }
  };

  const failed = checks?.some((c) => c.status === "fail") ?? false;

  return (
    <Card title="Unir a un dominio" icon={<ShieldCheck size={14} />}>
      {!status.canJoin ? (
        <p className="text-sm text-warn">Esta edición de Windows (Home) no puede unirse a un dominio. Hay que actualizarla a Windows Pro.</p>
      ) : (
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <label>
              <span className="mb-1 block text-xs text-dim">Dominio</span>
              <input
                value={domain}
                onChange={(e) => {
                  setDomain(e.target.value);
                  setChecks(null);
                }}
                placeholder="pgr.gob.do"
                className={`${inputClass} font-mono`}
              />
            </label>
            <label>
              <span className="mb-1 block text-xs text-dim">Nuevo nombre del equipo (opcional)</span>
              <input
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                maxLength={15}
                placeholder={status.computerName}
                className={`${inputClass} font-mono`}
              />
            </label>
            <label>
              <span className="mb-1 block text-xs text-dim">Usuario con permiso para unir equipos</span>
              <input value={user} onChange={(e) => setUser(e.target.value)} placeholder="usuario (o DOMINIO\usuario)" autoComplete="off" className={inputClass} />
            </label>
            <div>
              <span className="mb-1 block text-xs text-dim">Contraseña</span>
              <Password value={password} onChange={setPassword} />
            </div>
          </div>
          {advanced ? (
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Unidad organizativa (opcional)</span>
              <input value={ou} onChange={(e) => setOu(e.target.value)} placeholder="OU=Equipos,OU=Soporte,DC=pgr,DC=gob,DC=do" className={`${inputClass} font-mono text-xs`} />
            </label>
          ) : (
            <button onClick={() => setAdvanced(true)} className="text-xs text-mute hover:text-ink">
              + Elegir unidad organizativa (OU)
            </button>
          )}

          {checks && (
            <div className="space-y-1.5 rounded-lg border border-line bg-void/40 p-3">
              {checks.map((c) => {
                const I = ICON[c.status];
                return (
                  <div key={c.label} className="flex items-start gap-2 text-sm">
                    <I size={14} className={`mt-0.5 shrink-0 ${COLOR[c.status]}`} />
                    <span className="w-44 shrink-0 text-ink">{c.label}</span>
                    <span className="min-w-0 flex-1 text-xs text-dim">{c.detail}</span>
                  </div>
                );
              })}
            </div>
          )}
          {error && <p className="text-sm text-bad">{error}</p>}

          <div className="flex items-center gap-2">
            <Button kind="ghost" onClick={check} disabled={!domain.trim() || checking || joining}>
              {checking ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />} Comprobar antes de unir
            </Button>
            <Button onClick={join} disabled={!isAdmin || !domain.trim() || !user.trim() || !password || joining || failed}>
              {joining ? <Loader2 size={14} className="animate-spin" /> : <Building2 size={14} />} Unir al dominio
            </Button>
            {failed && <span className="text-xs text-bad">Corrige lo marcado en rojo antes de unir.</span>}
          </div>
          <p className="text-[11px] text-mute">
            La contraseña solo se usa para esta operación: no se guarda ni queda en el historial. Después hay que reiniciar el equipo e iniciar
            sesión con el usuario del dominio.
          </p>
        </div>
      )}
    </Card>
  );
}

function CredentialsDialog({
  kind,
  status,
  onClose,
  onDone,
}: {
  kind: "repair" | "leave" | "rename";
  status: DomainStatus;
  onClose: () => void;
  onDone: (msg: string, restart: boolean) => void;
}) {
  const [user, setUser] = useState("");
  const [password, setPassword] = useState("");
  const [workgroup, setWorkgroup] = useState("WORKGROUP");
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const needsCreds = kind !== "rename" || status.partOfDomain;

  const title = { repair: "Reparar la relación de confianza", leave: "Salir del dominio", rename: "Cambiar el nombre del equipo" }[kind];

  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      if (kind === "repair") {
        await domainApi.repair({ user, password });
        onDone("Relación de confianza reparada: ya se puede iniciar sesión con usuarios del dominio.", false);
      } else if (kind === "leave") {
        await domainApi.leave({ user, password, workgroup });
        onDone(`El equipo salió del dominio y ahora está en el grupo ${workgroup.toUpperCase()}.`, true);
      } else {
        await domainApi.rename({ newName, user, password });
        onDone(`El equipo se llamará ${newName.trim().toUpperCase()}.`, true);
      }
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  const ready = (!needsCreds || (user.trim() && password)) && (kind !== "rename" || newName.trim());

  return (
    <Modal
      title={title}
      onClose={busy ? () => {} : onClose}
      footer={
        <>
          {error && <p className="mr-auto max-w-64 text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose} disabled={busy}>
            Cancelar
          </Button>
          <Button kind={kind === "leave" ? "danger" : "primary"} onClick={run} disabled={!ready || busy}>
            {busy && <Loader2 size={14} className="animate-spin" />} {kind === "leave" ? "Salir del dominio" : kind === "repair" ? "Reparar" : "Cambiar nombre"}
          </Button>
        </>
      }
    >
      <div className="space-y-3 text-sm text-dim">
        {kind === "repair" && (
          <p>
            Arregla el error «la relación de confianza entre esta estación de trabajo y el dominio principal ha fallado» sin sacar el equipo del
            dominio. Necesita una cuenta del dominio con permiso sobre este equipo.
          </p>
        )}
        {kind === "leave" && (
          <p className="text-warn">
            Los usuarios del dominio dejarán de poder iniciar sesión en este equipo. Asegúrate de conocer la contraseña de un administrador local
            (AdminOps no deja continuar si no hay ninguno activo).
          </p>
        )}
        {kind === "rename" && (
          <input
            autoFocus
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            maxLength={15}
            placeholder={`Nuevo nombre (ahora: ${status.computerName})`}
            className={`${inputClass} font-mono`}
          />
        )}
        {kind === "leave" && <input value={workgroup} onChange={(e) => setWorkgroup(e.target.value)} placeholder="Grupo de trabajo" className={inputClass} />}
        {needsCreds && (
          <>
            <input
              autoFocus={kind !== "rename"}
              value={user}
              onChange={(e) => setUser(e.target.value)}
              placeholder={`Usuario del dominio ${status.domain ?? ""}`}
              autoComplete="off"
              className={inputClass}
            />
            <Password value={password} onChange={setPassword} />
          </>
        )}
        <p className="text-[11px] text-mute">La contraseña no se guarda ni queda en el historial.</p>
      </div>
    </Modal>
  );
}
