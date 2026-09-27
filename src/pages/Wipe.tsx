import { Eraser, ExternalLink, FilePlus, FolderPlus, HardDrive, PackageCheck, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import type { PageId } from "../components/Sidebar";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card } from "../components/ui";
import { recoverApi, wipeApi, type RecoverDrive } from "../lib/api";
import { bytes, friendlyPath } from "../lib/format";

type Step = { title: string; detail: string; action?: { label: string; page?: PageId; settings?: string } };

const SELL: Step[] = [
  { title: "Copia de los datos del cliente", detail: "Documentos, fotos, marcadores y redes Wi-Fi a un USB.", action: { label: "Copia de datos", page: "migrate" } },
  { title: "Guardar las claves de BitLocker", detail: "Si el disco está cifrado y se lleva a otro equipo, hará falta.", action: { label: "Seguridad", page: "security" } },
  { title: "Cerrar sesión en navegadores y apps", detail: "Chrome, Edge, WhatsApp, correo, nube (OneDrive, Google Drive, iCloud).", action: undefined },
  { title: "Liberar licencias de programas", detail: "Office, Adobe, antivirus de pago: desactivarlas en su cuenta para usarlas en el equipo nuevo.", action: undefined },
  { title: "Desactivar «Buscar mi dispositivo»", detail: "Si no, el equipo sigue asociado a la cuenta Microsoft del dueño.", action: { label: "Abrir", settings: "ms-settings:findmydevice" } },
  { title: "Quitar la cuenta Microsoft del equipo", detail: "Y retirarlo de account.microsoft.com → Dispositivos.", action: { label: "Abrir", settings: "ms-settings:emailandaccounts" } },
  {
    title: "Restablecer este PC → Quitar todo → Limpiar la unidad",
    detail: "Reinstala Windows y sobrescribe el disco. Es la opción segura también para SSD. Tarda varias horas.",
    action: { label: "Abrir", settings: "ms-settings:recovery" },
  },
];

export function Wipe({ onNavigate }: { onNavigate: (p: PageId) => void }) {
  const [items, setItems] = useState<string[]>([]);
  const [busy, setBusy] = useState<"items" | "free" | null>(null);
  const [drives, setDrives] = useState<RecoverDrive[]>([]);
  const [drive, setDrive] = useState("");
  const [done, setDone] = useState<boolean[]>(SELL.map(() => false));
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  useEffect(() => {
    recoverApi
      .status()
      .then((s) => {
        setDrives(s.drives);
        setDrive((d) => d || s.drives[0]?.letter || "");
      })
      .catch(() => {});
  }, []);

  const add = async (folders: boolean) => {
    const picked = await wipeApi.pick(folders);
    setItems((cur) => [...cur, ...picked.filter((p) => !cur.includes(p))]);
  };

  const wipe = async () => {
    const ok = await confirm({
      title: `¿Borrar ${items.length} elemento(s) de forma segura?`,
      danger: true,
      confirmLabel: "Borrar sin posibilidad de recuperar",
      body: <p>Se sobrescriben y se borran. No van a la papelera y ningún programa de recuperación podrá devolverlos.</p>,
    });
    if (!ok) return;
    setBusy("items");
    try {
      const r = await wipeApi.items(items);
      if (r.failed.length) toast("info", `${r.files} archivos borrados; ${r.failed.length} no se pudieron (en uso o sin permiso).`);
      else toast("ok", `${r.files} archivos borrados de forma segura (${bytes(r.bytes)}).`);
      setItems([]);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const freeSpace = async () => {
    const d = drives.find((x) => x.letter === drive);
    const ok = await confirm({
      title: `¿Vaciar el espacio libre de ${drive}:?`,
      confirmLabel: "Empezar",
      body: (
        <p>
          Se sobrescribe el espacio libre ({d ? bytes(d.free) : "…"}) para que lo borrado antes no se pueda recuperar. Los archivos actuales no se tocan. Puede
          tardar horas; se puede cancelar.
        </p>
      ),
    });
    if (!ok) return;
    setBusy("free");
    try {
      await wipeApi.freeSpace(drive);
      toast("ok", `Espacio libre de ${drive}: sobrescrito.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <Card title="Borrar archivos y carpetas" icon={<Eraser size={14} />} className="col-span-12 lg:col-span-7">
        <p className="mb-3 text-sm text-dim">Sobrescribe el contenido antes de borrarlo: ni la papelera ni los programas de recuperación podrán devolverlo.</p>
        <div className="flex gap-2">
          <Button kind="ghost" onClick={() => add(false)} disabled={busy !== null}>
            <FilePlus size={14} /> Añadir archivos
          </Button>
          <Button kind="ghost" onClick={() => add(true)} disabled={busy !== null}>
            <FolderPlus size={14} /> Añadir carpetas
          </Button>
        </div>
        {items.length > 0 && (
          <ul className="mt-3 max-h-56 space-y-1 overflow-y-auto">
            {items.map((p) => (
              <li key={p} className="flex items-center gap-2 rounded-md px-2 py-1 text-sm hover:bg-panel-2">
                <span className="min-w-0 flex-1 truncate text-ink">{friendlyPath(p)}</span>
                <button onClick={() => setItems(items.filter((x) => x !== p))} className="text-mute hover:text-ink" title="Quitar de la lista">
                  <X size={13} />
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="mt-3 flex items-center justify-between gap-3">
          <p className="text-[11px] text-mute">En discos SSD el borrado es muy fiable pero no absoluto; para vender un equipo con SSD usa «Limpiar la unidad» (abajo).</p>
          <Button onClick={wipe} disabled={busy !== null || items.length === 0}>
            <Eraser size={14} /> Borrar
          </Button>
        </div>
        <TaskStatus task="wipe" active={busy === "items"} fallback="Borrando…" className="mt-3" />
      </Card>

      <Card title="Vaciar el espacio libre" icon={<HardDrive size={14} />} className="col-span-12 lg:col-span-5">
        <p className="mb-3 text-sm text-dim">Lo que se borró antes con la papelera sigue en el disco hasta que algo lo sobrescribe. Esto lo sobrescribe todo de una vez.</p>
        <div className="flex gap-2">
          <select value={drive} onChange={(e) => setDrive(e.target.value)} className="flex-1 rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none">
            {drives.map((d) => (
              <option key={d.letter} value={d.letter}>
                {d.letter}: {d.label ? `· ${d.label} ` : ""}· {bytes(d.free)} libres
              </option>
            ))}
          </select>
          <Button onClick={freeSpace} disabled={busy !== null || !drive}>
            Empezar
          </Button>
        </div>
        <TaskStatus task="wipe-free" active={busy === "free"} fallback="Sobrescribiendo…" className="mt-3" />
      </Card>

      <Card title="Antes de vender o donar el equipo" icon={<PackageCheck size={14} />} className="col-span-12">
        <ol className="space-y-1">
          {SELL.map((s, i) => (
            <li key={s.title} className="flex items-center gap-3 rounded-md px-2 py-2 hover:bg-panel-2">
              <input
                type="checkbox"
                checked={done[i]}
                onChange={() => setDone(done.map((d, j) => (j === i ? !d : d)))}
                className="size-4 shrink-0 accent-[var(--color-neon)]"
                aria-label={s.title}
              />
              <div className="min-w-0 flex-1">
                <div className={`text-sm ${done[i] ? "text-dim line-through" : "text-ink"}`}>{s.title}</div>
                <div className="text-xs text-mute">{s.detail}</div>
              </div>
              {s.action && (
                <button
                  onClick={() => (s.action!.page ? onNavigate(s.action!.page) : wipeApi.openSettings(s.action!.settings!).catch((e) => toast("error", String(e))))}
                  className="flex shrink-0 items-center gap-1 text-xs text-neon hover:underline"
                >
                  {s.action.label} <ExternalLink size={11} />
                </button>
              )}
            </li>
          ))}
        </ol>
        <p className="mt-2 text-[11px] text-mute">
          {done.filter(Boolean).length} de {SELL.length} hechos
        </p>
      </Card>
      {dialog}
    </div>
  );
}
