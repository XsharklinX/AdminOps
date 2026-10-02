// Discos: salud, reparación y rescate de archivos.
//
// Primero el veredicto de cada disco (qué está roto: la superficie, la
// conexión o el índice de archivos), porque de eso depende qué hacer. Lo que
// pierde datos va antes que lo que se arregla: con un disco que falla, lo
// primero es copiar lo que aún se lee, y eso está aquí mismo.
import { Activity, CheckCircle2, CircleAlert, FolderInput, FolderOutput, HardDrive, Loader2, RefreshCw, ScanSearch, ShieldAlert, Usb, Wrench, XCircle } from "lucide-react";
import { useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { BitLockerChip, DiskTrend, RemovableActions, SpeedTest } from "../components/DiskExtras";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, Loading } from "../components/ui";
import { disksApi, type DiskCheck, type DiskReport, type DiskVolume, type RescueResult } from "../lib/api";
import { withoutUserPaths } from "../lib/errors";
import { bytes } from "../lib/format";
import { useLiveEffect } from "../lib/useLiveEffect";

const TONE = {
  ok: { box: "border-ok/40 bg-ok/10", text: "text-ok", Icon: CheckCircle2 },
  warn: { box: "border-warn/40 bg-warn/10", text: "text-warn", Icon: CircleAlert },
  bad: { box: "border-bad/40 bg-bad/10", text: "text-bad", Icon: XCircle },
};

const known = (n: number | null | undefined) => n !== null && n !== undefined && n >= 0;

export function Disks({ isAdmin }: { isAdmin: boolean }) {
  const [disks, setDisks] = useState<DiskReport[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = (vigente: () => boolean = () => true) => {
    setLoading(true);
    disksApi
      .status()
      .then((d) => vigente() && (setDisks(d), setError(null)))
      .catch((e) => vigente() && setError(String(e)))
      .finally(() => vigente() && setLoading(false));
  };
  useLiveEffect((vigente) => load(vigente), []);

  return (
    <div className="mx-auto max-w-6xl space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="min-w-0 flex-1 text-xs text-dim">
          Qué le pasa a cada disco y qué hacer. Si Windows te dice «Reparar disco» o hay archivos que no se copian, empieza por el veredicto: no es lo mismo un
          índice de archivos dañado (se repara) que un disco que se estropea (se copia lo que se pueda y se cambia).
        </p>
        <Button kind="ghost" onClick={() => load()} disabled={loading}>
          {loading ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />} Actualizar
        </Button>
      </div>

      {!isAdmin && (
        <p className="flex items-start gap-2 rounded-lg border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
          <ShieldAlert size={14} className="mt-0.5 shrink-0" />
          Sin administrador no se ven los datos SMART, las temperaturas ni si Windows marcó un disco como dañado, y no se puede reparar. Abre AdminOps como
          administrador para verlo todo.
        </p>
      )}

      {error ? (
        <p className="text-sm text-bad">{error}</p>
      ) : disks === null ? (
        <Loading text="Leyendo los discos… (con discos grandes o externos tarda unos segundos)" />
      ) : (
        disks.map((d) => <DiskCard key={d.number} d={d} isAdmin={isAdmin} onChanged={() => load()} />)
      )}

      <Rescue isAdmin={isAdmin} />

      <Card title="¿Qué significa cada cosa?" icon={<Activity size={14} />}>
        <dl className="grid gap-3 text-xs text-dim md:grid-cols-2">
          <div>
            <dt className="font-medium text-ink">«Reparar disco» de Windows</dt>
            <dd>
              Windows marcó el sistema de archivos como dañado: el índice que dice dónde está cada archivo. Casi siempre por quitar un disco externo sin expulsarlo o
              por un corte de luz. «Reparar sistema de archivos» lo arregla sin tocar lo que está bien.
            </dd>
          </div>
          <div>
            <dt className="font-medium text-ink">Sectores dañados</dt>
            <dd>
              Zonas del disco que ya no se leen. No se «reparan»: el disco las aparta y, si siguen apareciendo, el disco está fallando. «Buscar sectores dañados»
              (chkdsk /r) las marca para que Windows no las use, pero lee todo el disco: con uno que falla, copia antes lo importante.
            </dd>
          </div>
          <div>
            <dt className="font-medium text-ink">Errores de conexión (CRC)</dt>
            <dd>Los datos se estropean por el camino: cable, puerto o la caja USB del disco externo. El disco puede estar perfecto.</dd>
          </div>
          <div>
            <dt className="font-medium text-ink">Rescatar archivos</dt>
            <dd>
              Copia a otro disco todo lo que aún se lee, sin quedarse atascada en lo ilegible (un reintento y sigue), y te dice qué archivos se quedaron. Es lo
              primero que hay que hacer con un disco que falla.
            </dd>
          </div>
        </dl>
      </Card>
    </div>
  );
}

function DiskCard({ d, isAdmin, onChanged }: { d: DiskReport; isAdmin: boolean; onChanged: () => void }) {
  const t = TONE[d.verdict.level];
  const external = d.bus.toUpperCase() === "USB";
  const Icon = external ? Usb : HardDrive;
  const facts: string[] = [];
  if (known(d.temperature) && d.temperature > 0) facts.push(`${d.temperature} °C`);
  if (known(d.hours) && d.hours > 0) facts.push(`${d.hours.toLocaleString("es")} h encendido`);
  if (known(d.wear)) facts.push(`${d.wear} % de desgaste`);
  if (d.reallocated !== null) facts.push(`${d.reallocated} sectores apartados`);
  if (d.pending !== null) facts.push(`${d.pending} pendientes`);
  if (d.uncorrectable !== null) facts.push(`${d.uncorrectable} no corregibles`);
  if (d.crcErrors !== null) facts.push(`${d.crcErrors} errores de conexión`);
  if (known(d.readErrors)) facts.push(`${d.readErrors} errores de lectura`);

  return (
    <section className="overflow-hidden rounded-xl border border-line bg-panel">
      <header className="flex flex-wrap items-center gap-3 px-4 py-3">
        <Icon size={18} className="shrink-0 text-mute" />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium text-ink">{d.model || `Disco ${d.number}`}</div>
          <div className="text-[11px] text-mute">
            {[external ? "Externo (USB)" : d.bus, d.media !== "Unspecified" ? d.media : "", bytes(d.size), d.system ? "disco de Windows" : ""].filter(Boolean).join(" · ")}
          </div>
        </div>
        <span className={`flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs ${t.box} ${t.text}`}>
          <t.Icon size={12} /> {d.verdict.level === "ok" ? "Sano" : d.verdict.level === "warn" ? "Con avisos" : "Fallando"}
        </span>
      </header>

      {d.verdict.level !== "ok" && (
        <div className={`mx-4 mb-3 rounded-lg border p-3 ${t.box}`}>
          <div className={`text-sm font-medium ${t.text}`}>{d.verdict.title}</div>
          <p className="mt-1 text-xs leading-relaxed text-dim">{d.verdict.text}</p>
          {d.verdict.advice.length > 0 && (
            <ol className="mt-2 list-decimal space-y-0.5 pl-5 text-xs text-ink">
              {d.verdict.advice.map((a) => (
                <li key={a}>{a}</li>
              ))}
            </ol>
          )}
        </div>
      )}
      {d.verdict.level === "ok" && <p className="-mt-1 px-4 pb-2 text-xs text-dim">{d.verdict.text}</p>}
      {facts.length > 0 && <p className="px-4 pb-3 text-[11px] text-mute">{facts.join(" · ")}</p>}
      <DiskTrend d={d} />
      <SpeedTest d={d} />

      {d.volumes.length > 0 && (
        <ul className="divide-y divide-line/60 border-t border-line/60">
          {d.volumes.map((v) => (
            <VolumeRow key={v.letter} v={v} failing={d.verdict.level === "bad"} removable={external || d.bus.toUpperCase() === "SD"} diskSize={d.size} isAdmin={isAdmin} onChanged={onChanged} />
          ))}
        </ul>
      )}
    </section>
  );
}

function VolumeRow({
  v,
  failing,
  removable,
  diskSize,
  isAdmin,
  onChanged,
}: {
  v: DiskVolume;
  failing: boolean;
  removable: boolean;
  diskSize: number;
  isAdmin: boolean;
  onChanged: () => void;
}) {
  const [busy, setBusy] = useState<"check" | "repair" | "surface" | null>(null);
  const [result, setResult] = useState<DiskCheck | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const used = v.size > 0 ? Math.round(((v.size - v.free) / v.size) * 100) : 0;

  const run = async (kind: "check" | "repair" | "surface", p: () => Promise<DiskCheck>) => {
    setBusy(kind);
    setResult(null);
    try {
      setResult(await p());
      if (kind !== "check") onChanged();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const repair = async () => {
    const ok = await confirm({
      title: `Reparar el sistema de archivos de ${v.letter}:`,
      body: v.system
        ? "Es el disco de Windows: se revisará y reparará al reiniciar el equipo, antes de que arranque Windows (unos minutos)."
        : "Cierra antes los archivos abiertos de esa unidad: se desconecta un momento de Windows mientras se repara. Arregla el índice de archivos sin tocar los archivos buenos.",
      confirmLabel: v.system ? "Programar al reiniciar" : "Reparar",
    });
    if (ok) await run("repair", () => disksApi.repair(v.letter));
  };

  const surface = async () => {
    const ok = await confirm({
      title: `Buscar sectores dañados en ${v.letter}:`,
      body: failing
        ? "Este disco está fallando. Leer toda la superficie puede empeorarlo: copia antes lo importante con «Rescatar archivos». ¿Seguir de todos modos?"
        : "Lee todo el disco y aparta los sectores que no se leen (chkdsk /r). Puede tardar horas en discos grandes y la unidad no se puede usar mientras tanto.",
      confirmLabel: "Empezar",
      danger: failing,
    });
    if (ok) await run("surface", () => disksApi.surfaceScan(v.letter));
  };

  const t = result ? TONE[result.level] : null;
  return (
    <li className="px-4 py-3">
      <div className="flex flex-wrap items-center gap-3">
        <span className="w-8 font-mono text-sm font-semibold text-ink">{v.letter}:</span>
        <div className="min-w-40 flex-1">
          <div className="flex items-center gap-2 text-xs">
            <span className="truncate text-dim">{v.label || "Sin nombre"}</span>
            <span className="text-mute">{v.fs}</span>
            {v.dirty === true && <span className="rounded border border-warn/50 px-1 text-[10px] text-warn">marcado como dañado</span>}
            <BitLockerChip v={v} isAdmin={isAdmin} />
          </div>
          <div className="mt-1 flex items-center gap-2">
            <span className="h-1.5 flex-1 overflow-hidden rounded-full bg-line">
              <span className={`block h-full ${used >= 90 ? "bg-bad" : used >= 75 ? "bg-warn" : "bg-neon"}`} style={{ width: `${used}%` }} />
            </span>
            <span className="shrink-0 text-[11px] text-mute">
              {bytes(v.free)} libres de {bytes(v.size)}
            </span>
          </div>
        </div>
        <div className="flex flex-wrap gap-1.5">
          <Button kind="ghost" onClick={() => void run("check", () => disksApi.check(v.letter))} disabled={!isAdmin || busy !== null} title="Revisa el sistema de archivos sin cambiar nada">
            {busy === "check" ? <Loader2 size={13} className="animate-spin" /> : <ScanSearch size={13} />} Comprobar
          </Button>
          <Button kind={v.dirty ? "primary" : "ghost"} onClick={() => void repair()} disabled={!isAdmin || busy !== null} title="Arregla el índice de archivos (chkdsk /f)">
            {busy === "repair" ? <Loader2 size={13} className="animate-spin" /> : <Wrench size={13} />} Reparar sistema de archivos
          </Button>
          {!v.system && (
            <Button kind="ghost" onClick={() => void surface()} disabled={!isAdmin || busy !== null} title="Lee toda la superficie y aparta los sectores dañados (chkdsk /r)">
              <HardDrive size={13} /> Buscar sectores dañados
            </Button>
          )}
        </div>
      </div>
      {busy === "surface" && <TaskStatus task={`disk-surface:${v.letter}`} active className="mt-2" fallback="Revisando la superficie…" />}
      {removable && !v.system && <RemovableActions v={v} size={Math.min(v.size || diskSize, diskSize || v.size)} onChanged={onChanged} />}
      {busy === "check" && <p className="mt-2 text-xs text-mute">Comprobando sin cambiar nada… (en discos grandes, unos minutos)</p>}
      {result && t && (
        <p className={`mt-2 flex items-start gap-1.5 text-xs ${t.text}`}>
          <t.Icon size={13} className="mt-0.5 shrink-0" /> {result.text}
        </p>
      )}
      {dialog}
    </li>
  );
}

/** Copiar lo que aún se lee de un disco que falla. */
function Rescue({ isAdmin }: { isAdmin: boolean }) {
  const [source, setSource] = useState("");
  const [dest, setDest] = useState("");
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<RescueResult | null>(null);
  const toast = useToast();

  const pick = async (set: (s: string) => void) => {
    const f = await disksApi.pickFolder().catch(() => null);
    if (f) set(f);
  };

  const start = async () => {
    setRunning(true);
    setResult(null);
    try {
      const r = await disksApi.rescue(source, dest);
      setResult(r);
      toast(r.failed.length ? "info" : "ok", r.failed.length ? `Copiado lo que se podía leer. ${r.failed.length} archivos se quedaron en el disco.` : "Copiado todo.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(false);
    }
  };

  const folder = (label: string, value: string, set: (s: string) => void, icon: React.ReactNode) => (
    <div className="min-w-0 flex-1">
      <div className="mb-1 text-[11px] text-mute">{label}</div>
      <button
        onClick={() => void pick(set)}
        disabled={running}
        className="flex w-full items-center gap-2 rounded-md border border-line bg-void/60 px-3 py-2 text-left text-sm text-ink hover:border-neon/50 disabled:opacity-50"
      >
        <span className="shrink-0 text-mute">{icon}</span>
        <span className="truncate">{value ? withoutUserPaths(value) : "Elegir carpeta…"}</span>
      </button>
    </div>
  );

  return (
    <Card title="Rescatar archivos" icon={<FolderOutput size={14} />}>
      <p className="mb-3 text-xs text-dim">
        Para un disco que da errores o del que «hay archivos que no se pasan»: copia a otro disco todo lo que aún se puede leer, salta enseguida lo ilegible (en vez
        de quedarse colgado) y al final dice qué archivos se quedaron. Lo que ya estaba copiado en el destino no se vuelve a copiar.
      </p>
      <div className="flex flex-wrap items-end gap-3">
        {folder("Copiar desde (el disco con problemas)", source, setSource, <FolderOutput size={14} />)}
        {folder("Hacia (otro disco, con espacio)", dest, setDest, <FolderInput size={14} />)}
        <Button onClick={() => void start()} disabled={running || !source || !dest}>
          {running ? <Loader2 size={14} className="animate-spin" /> : <FolderOutput size={14} />} Rescatar
        </Button>
      </div>
      {running && <TaskStatus task="disk-rescue" active className="mt-3" fallback="Copiando lo que se puede leer…" />}
      {result && (
        <div className="mt-3 rounded-lg border border-line bg-panel-2 p-3 text-xs">
          <p className="text-ink">
            En el destino: {result.copied.toLocaleString("es")} archivos ({bytes(result.bytes)}).
          </p>
          {result.failed.length === 0 ? (
            <p className="mt-1 text-ok">No se quedó nada: todo se pudo leer.</p>
          ) : (
            <>
              <p className="mt-1 text-warn">
                {result.failed.length} {result.failed.length === 1 ? "archivo no se pudo leer" : "archivos no se pudieron leer"} (están en zonas dañadas del disco):
              </p>
              <ul className="mt-1 max-h-48 overflow-y-auto font-mono text-[11px] text-dim">
                {result.failed.map((f) => (
                  <li key={f} className="truncate" title={withoutUserPaths(f)}>
                    {withoutUserPaths(f)}
                  </li>
                ))}
              </ul>
              <button
                onClick={() => void navigator.clipboard.writeText(result.failed.map(withoutUserPaths).join("\n")).then(() => toast("ok", "Lista copiada."), () => {})}
                className="mt-2 text-neon hover:underline"
              >
                Copiar la lista
              </button>
            </>
          )}
        </div>
      )}
      {!isAdmin && <p className="mt-2 text-[11px] text-mute">Sin administrador se copia igual; solo se saltan las carpetas protegidas de Windows.</p>}
    </Card>
  );
}
