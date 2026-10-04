import {
  CircleCheck,
  CircleHelp,
  Info,
  CircleX,
  Copy,
  Eye,
  HardDriveDownload,
  KeyRound,
  Loader2,
  Puzzle,
  RefreshCw,
  ScanSearch,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import type { PageId } from "../components/Sidebar";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, Modal, Loading } from "../components/ui";
import { TweaksPage } from "./TweaksPage";
import { securityApi, toolboxApi, toolsApi, type BitlockerVolume, type SecurityAudit, type SecurityCheck, type SuspiciousItem } from "../lib/api";
import { useLiveEffect } from "../lib/useLiveEffect";

const STATUS = {
  ok: { icon: CircleCheck, color: "text-ok" },
  warn: { icon: TriangleAlert, color: "text-warn" },
  bad: { icon: CircleX, color: "text-bad" },
  unknown: { icon: CircleHelp, color: "text-mute" },
  // Un dato: no suma ni resta en la nota.
  info: { icon: Info, color: "text-dim" },
};

const scoreColor = (s: number) => (s >= 80 ? "var(--color-ok)" : s >= 60 ? "var(--color-warn)" : "var(--color-bad)");
const CONVERSION = ["Sin cifrar", "Cifrada", "Cifrando…", "Descifrando…", "Cifrado en pausa", "Descifrado en pausa"];
const KIND = { task: "Tarea programada", service: "Servicio", startup: "Inicio", hosts: "Hosts" };

export function Security({ isAdmin, focus, onNavigate }: { isAdmin: boolean; focus: string | null; onNavigate: (p: PageId, f?: string | null) => void }) {
  const [audit, setAudit] = useState<SecurityAudit | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [updating, setUpdating] = useState(false);
  const toast = useToast();

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setAudit(await securityApi.audit());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const fix = (c: SecurityCheck) => {
    const f = c.fix;
    if (!f) return;
    if (f.tool) toolboxApi.launch(f.tool).catch((e) => toast("error", String(e)));
    else if (f.page === "security" && !f.focus) document.getElementById("security-software")?.scrollIntoView({ behavior: "smooth" });
    else if (f.page) onNavigate(f.page as PageId, f.focus);
  };

  const updateAll = async () => {
    if (!audit) return;
    setUpdating(true);
    try {
      const r = await toolsApi.upgradeSoftware(audit.vulnerable.map((v) => v.id));
      const ok = r.filter((x) => x.ok).length;
      toast(ok === r.length ? "ok" : "info", `${ok} de ${r.length} programas actualizados.`);
      void load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setUpdating(false);
    }
  };

  return (
    <div className="mx-auto max-w-6xl space-y-4 p-6">
      <Card
        title="Nota de seguridad"
        icon={<ShieldCheck size={14} />}
        right={
          <button onClick={load} disabled={loading} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
            <RefreshCw size={11} className={loading ? "animate-spin" : ""} /> Volver a comprobar
          </button>
        }
      >
        {error ? (
          <p className="text-sm text-bad">{error}</p>
        ) : !audit ? (
          <p className="flex items-center gap-2 text-sm text-mute">
            <Loader2 size={14} className="animate-spin" /> Revisando antivirus, firewall, cifrado, cuentas y programas…
          </p>
        ) : (
          <div className="flex gap-6">
            <div className="flex w-36 shrink-0 flex-col items-center justify-center">
              <div className="font-mono text-5xl font-semibold tabular" style={{ color: scoreColor(audit.score) }}>
                {audit.score}
              </div>
              <div className="text-xs text-mute">de 100</div>
              <div className="mt-1 text-xs" style={{ color: scoreColor(audit.score) }}>
                {audit.score >= 80 ? "Bien protegido" : audit.score >= 60 ? "Mejorable" : "En riesgo"}
              </div>
            </div>
            <div className="min-w-0 flex-1 divide-y divide-line/60">
              {audit.checks.map((c) => {
                const S = STATUS[c.status];
                return (
                  <div key={c.id} className="flex items-center gap-3 py-1.5">
                    <S.icon size={15} className={`shrink-0 ${S.color}`} />
                    <div className="min-w-0 flex-1">
                      <div className="text-sm text-ink">{c.label}</div>
                      <div className="truncate text-[11px] text-mute" title={c.detail}>
                        {c.detail}
                      </div>
                    </div>
                    {c.fix && c.status !== "ok" && (
                      <button onClick={() => fix(c)} className="shrink-0 rounded-md border border-neon/40 px-2.5 py-1 text-xs text-neon hover:bg-neon/10">
                        {c.fix.label}
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {!isAdmin && audit && <p className="mt-3 text-[11px] text-warn">Sin administrador algunas comprobaciones (BitLocker) no se pueden hacer y no cuentan en la nota.</p>}
      </Card>

      {audit && audit.vulnerable.length > 0 && (
        <Card id="security-software" title={`Programas de riesgo desactualizados (${audit.vulnerable.length})`} icon={<TriangleAlert size={14} />}>
          <p className="mb-2 text-xs text-dim">Navegadores, lectores de PDF, compresores y programas de acceso remoto son la puerta de entrada más habitual.</p>
          <div className="mb-3 divide-y divide-line/60">
            {audit.vulnerable.map((v) => (
              <div key={v.id} className="flex items-center gap-3 py-1.5 text-sm">
                <span className="min-w-0 flex-1 truncate text-ink">{v.name}</span>
                <span className="font-mono text-xs text-mute">{v.version}</span>
                <span className="text-xs text-dim">→</span>
                <span className="font-mono text-xs text-neon">{v.available}</span>
              </div>
            ))}
          </div>
          {updating ? (
            <TaskStatus task="software" active fallback="Actualizando…" />
          ) : (
            <Button onClick={updateAll} disabled={!isAdmin}>
              Actualizar todos
            </Button>
          )}
        </Card>
      )}

      <BitlockerCard isAdmin={isAdmin} />
      <SuspiciousCard isAdmin={isAdmin} />
      <ExtensionsCard />

      <div>
        <h2 className="mb-1 px-1 text-[11px] font-semibold text-dim">Ajustes de seguridad</h2>
        <div className="-mx-6 -mt-4">
          <TweaksPage category="security" isAdmin={isAdmin} focus={focus} />
        </div>
      </div>
    </div>
  );
}

function BitlockerCard({ isAdmin }: { isAdmin: boolean }) {
  const [vols, setVols] = useState<BitlockerVolume[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [keys, setKeys] = useState<{ drive: string; id: string; key: string }[] | null>(null);
  const toast = useToast();

  useLiveEffect(
    (vigente) => {
      if (isAdmin) securityApi.bitlocker().then((v) => vigente() && setVols(v)).catch((e) => vigente() && setError(String(e)));
    },
    [isAdmin],
  );

  const showKeys = () => securityApi.keys().then(setKeys).catch((e) => toast("error", String(e)));
  const exportKeys = () =>
    securityApi
      .exportKeys()
      .then((p) => p && toast("ok", `Claves guardadas en ${p}`))
      .catch((e) => toast("error", String(e)));

  const anyKey = vols?.some((v) => v.hasRecoveryKey);

  return (
    <Card title="BitLocker" icon={<KeyRound size={14} />}>
      {!isAdmin ? (
        <p className="text-sm text-mute">Requiere administrador para ver el cifrado y las claves de recuperación.</p>
      ) : error ? (
        <p className="text-sm text-mute">{error}</p>
      ) : !vols ? (
        <Loading text="Leyendo unidades…" />
      ) : vols.length === 0 ? (
        <p className="text-sm text-mute">Esta edición de Windows no ofrece BitLocker ni cifrado de dispositivo.</p>
      ) : (
        <>
          <div className="mb-3 divide-y divide-line/60">
            {vols.map((v) => (
              <div key={v.drive} className="flex items-center gap-3 py-1.5 text-sm">
                <span className="w-10 font-mono text-ink">{v.drive}</span>
                <span className={`flex-1 ${v.protection === 1 ? "text-ok" : v.conversion === 0 ? "text-dim" : "text-warn"}`}>
                  {CONVERSION[v.conversion] ?? "—"}
                  {v.conversion === 2 && ` (${v.percent}%)`}
                  {v.protection !== 1 && v.conversion === 1 && " · protección suspendida"}
                </span>
                <span className="text-xs text-mute">{v.hasRecoveryKey ? "Con clave de recuperación" : "Sin clave de recuperación"}</span>
              </div>
            ))}
          </div>
          {anyKey && (
            <>
              <div className="flex gap-2">
                <Button onClick={exportKeys}>
                  <HardDriveDownload size={14} /> Guardar las claves en un USB…
                </Button>
                <Button kind="ghost" onClick={showKeys}>
                  <Eye size={14} /> Ver claves
                </Button>
              </div>
              <p className="mt-2 text-[11px] text-mute">
                Si Windows pide la clave de recuperación (tras cambiar la placa, la BIOS o el disco) y no la tienes, los datos se pierden. Guárdala
                fuera del equipo.
              </p>
            </>
          )}
        </>
      )}
      {keys && (
        <Modal title="Claves de recuperación" onClose={() => setKeys(null)} width="w-[600px]">
          <div className="space-y-3">
            {keys.map((k) => (
              <div key={k.id} className="rounded-lg border border-line bg-void/40 p-3">
                <div className="text-xs text-mute">
                  Unidad {k.drive} · <span className="font-mono">{k.id}</span>
                </div>
                <div className="mt-1 flex items-center gap-2">
                  <span className="flex-1 font-mono text-sm text-ink select-text">{k.key}</span>
                  <button onClick={() => navigator.clipboard.writeText(k.key).then(() => toast("ok", "Clave copiada."))} className="text-mute hover:text-ink" title="Copiar">
                    <Copy size={13} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        </Modal>
      )}
    </Card>
  );
}

function SuspiciousCard({ isAdmin }: { isAdmin: boolean }) {
  const [items, setItems] = useState<SuspiciousItem[] | null>(null);
  const [scanning, setScanning] = useState(false);
  const toast = useToast();

  const scan = async () => {
    setScanning(true);
    try {
      setItems(await securityApi.suspicious());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setScanning(false);
    }
  };

  const disable = async (i: SuspiciousItem) => {
    try {
      await securityApi.disableTask(i.id!);
      toast("ok", `Tarea «${i.name}» desactivada.`);
      setItems((l) => l?.filter((x) => x !== i) ?? null);
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <Card
      title="Elementos sospechosos"
      icon={<ScanSearch size={14} />}
      right={
        <Button kind="ghost" onClick={scan} disabled={scanning}>
          {scanning ? <Loader2 size={13} className="animate-spin" /> : <ScanSearch size={13} />} Analizar
        </Button>
      }
    >
      {scanning ? (
        <p className="text-sm text-mute">Revisando tareas programadas, servicios, inicio y el archivo hosts (comprueba firmas digitales, puede tardar)…</p>
      ) : items === null ? (
        <p className="text-sm text-mute">
          Busca lo que suele dejar el malware: programas sin firmar en carpetas temporales, PowerShell oculto, descargas por script y el archivo hosts
          redirigiendo antivirus o Windows Update. No sustituye a un antivirus.
        </p>
      ) : items.length === 0 ? (
        <p className="flex items-center gap-2 text-sm text-ok">
          <CircleCheck size={14} /> No se encontró nada sospechoso.
        </p>
      ) : (
        <div className="divide-y divide-line/60">
          {items.map((i, n) => (
            <div key={n} className="py-2">
              <div className="flex items-center gap-2">
                <span className="rounded border border-bad/40 px-1.5 py-px text-[11px] text-bad">{KIND[i.kind]}</span>
                <span className="min-w-0 flex-1 truncate text-sm text-ink">{i.name}</span>
                {i.kind === "task" && i.id && (
                  <Button kind="ghost" onClick={() => disable(i)} disabled={!isAdmin}>
                    Desactivar
                  </Button>
                )}
              </div>
              <div className="mt-0.5 truncate font-mono text-[11px] text-mute select-text" title={i.detail}>
                {i.detail}
              </div>
              <div className="text-xs text-warn">{i.reason}</div>
            </div>
          ))}
        </div>
      )}
      {items && items.some((i) => i.kind !== "task") && (
        <p className="mt-2 text-[11px] text-mute">Servicios e inicio se gestionan en sus páginas; el archivo hosts, en Red → Herramientas.</p>
      )}
    </Card>
  );
}

function ExtensionsCard() {
  const [list, setList] = useState<Awaited<ReturnType<typeof securityApi.extensions>> | null>(null);
  const [loading, setLoading] = useState(false);
  const toast = useToast();

  const load = async () => {
    setLoading(true);
    try {
      setList(await securityApi.extensions());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  };

  const browsers = [...new Set((list ?? []).map((e) => e.browser))];

  return (
    <Card
      title="Extensiones de los navegadores"
      icon={<Puzzle size={14} />}
      right={
        <Button kind="ghost" onClick={load} disabled={loading}>
          {loading ? <Loader2 size={13} className="animate-spin" /> : <Puzzle size={13} />} Ver extensiones
        </Button>
      }
    >
      {list === null ? (
        <p className="text-sm text-mute">Muchas infecciones actuales son extensiones que cambian el buscador o insertan publicidad. Revisa las que el cliente no reconozca.</p>
      ) : list.length === 0 ? (
        <p className="text-sm text-mute">No hay extensiones instaladas.</p>
      ) : (
        <div className="grid grid-cols-2 gap-4">
          {browsers.map((b) => (
            <div key={b}>
              <div className="mb-1 text-[11px] text-mute">{b}</div>
              {list
                .filter((e) => e.browser === b)
                .map((e) => (
                  <div key={`${e.profile}-${e.id}`} className={`flex items-center gap-2 text-sm ${e.enabled ? "text-ink" : "text-mute line-through"}`} title={`${e.id} · perfil ${e.profile}`}>
                    <Puzzle size={11} className="shrink-0 text-mute" />
                    <span className="truncate">{e.name}</span>
                  </div>
                ))}
            </div>
          ))}
        </div>
      )}
      {list && list.length > 0 && <p className="mt-2 text-[11px] text-mute">Para quitar una, ábrela en su navegador: menú → Extensiones.</p>}
    </Card>
  );
}
