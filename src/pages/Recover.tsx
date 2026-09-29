import { Download, FileSearch, Lightbulb } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, inputClass, Loading } from "../components/ui";
import { recoverApi, type RecoverDrive } from "../lib/api";
import { bytes, friendlyPath } from "../lib/format";

const KINDS: [string, string][] = [
  ["documents", "Documentos (Word, Excel, PDF…)"],
  ["photos", "Fotos"],
  ["videos", "Vídeos"],
  ["music", "Música"],
  ["archives", "Comprimidos (zip, rar, 7z)"],
];

const driveLabel = (d: RecoverDrive) => `${d.letter}: ${d.label ? `· ${d.label} ` : ""}· ${bytes(d.size, 0)} · ${d.fs}${d.kind === "Removable" ? " · extraíble" : ""}`;

export function Recover() {
  const [installed, setInstalled] = useState<boolean | null>(null);
  const [drives, setDrives] = useState<RecoverDrive[]>([]);
  const [source, setSource] = useState("");
  const [dest, setDest] = useState("");
  const [kinds, setKinds] = useState<string[]>(["documents", "photos"]);
  const [folder, setFolder] = useState("");
  const [extensive, setExtensive] = useState(false);
  const [busy, setBusy] = useState<"install" | "run" | null>(null);
  const [result, setResult] = useState<{ folder: string; files: number } | null>(null);
  const toast = useToast();

  const load = useCallback(async () => {
    try {
      const s = await recoverApi.status();
      setInstalled(s.installed);
      setDrives(s.drives);
      setSource((x) => x || s.drives[0]?.letter || "");
      setDest((x) => x || s.drives.find((d) => d.letter !== (s.drives[0]?.letter ?? ""))?.letter || "");
    } catch (e) {
      toast("error", String(e));
    }
  }, [toast]);

  useEffect(() => {
    load();
  }, [load]);

  const install = async () => {
    setBusy("install");
    try {
      await recoverApi.install();
      toast("ok", "Windows File Recovery instalado.");
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const fsOf = drives.find((d) => d.letter === source)?.fs ?? "";

  const run = async () => {
    setBusy("run");
    setResult(null);
    try {
      // En FAT/exFAT (tarjetas, USB) winfr solo admite la búsqueda a fondo.
      const r = await recoverApi.run(source, dest, extensive || (fsOf !== "" && fsOf !== "NTFS"), kinds, folder);
      setResult(r);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <Card title="Antes de empezar" icon={<Lightbulb size={14} />} className="col-span-12 lg:col-span-5">
        <ul className="space-y-2 text-sm text-dim">
          <li>
            <b className="font-medium text-ink">Mira la Papelera</b> y, si se usa, OneDrive (tiene su propia papelera web de 30 días).
          </li>
          <li>
            <b className="font-medium text-ink">Deja de usar esa unidad</b>: cada archivo nuevo puede sobrescribir lo que se busca.
          </li>
          <li>Lo recuperado se guarda en otra unidad (un USB sirve).</li>
          <li>
            En discos SSD con TRIM, lo borrado hace tiempo suele no poder recuperarse. En discos duros, tarjetas y USB las probabilidades son mucho
            mejores.
          </li>
        </ul>
      </Card>

      <Card title="Recuperar archivos borrados" icon={<FileSearch size={14} />} className="col-span-12 lg:col-span-7">
        {installed === null ? (
          <Loading />
        ) : !installed ? (
          <div>
            <p className="mb-3 text-sm text-dim">
              Usa <b className="font-medium text-ink">Windows File Recovery</b>, la herramienta gratuita de Microsoft. AdminOps la instala desde la
              Microsoft Store y la maneja por ti.
            </p>
            <Button onClick={install} disabled={busy !== null}>
              <Download size={14} /> Instalar Windows File Recovery
            </Button>
            <TaskStatus task="recover-install" active={busy === "install"} fallback="Instalando…" className="mt-3" />
          </div>
        ) : (
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-3">
              <label className="block">
                <span className="mb-1 block text-xs text-dim">Unidad donde estaban</span>
                <select value={source} onChange={(e) => setSource(e.target.value)} className={inputClass}>
                  {drives.map((d) => (
                    <option key={d.letter} value={d.letter}>
                      {driveLabel(d)}
                    </option>
                  ))}
                </select>
              </label>
              <label className="block">
                <span className="mb-1 block text-xs text-dim">Guardar lo recuperado en</span>
                <select value={dest} onChange={(e) => setDest(e.target.value)} className={inputClass}>
                  {drives.map((d) => (
                    <option key={d.letter} value={d.letter} disabled={d.letter === source}>
                      {driveLabel(d)}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            {source && source === dest && <p className="text-xs text-bad">Elige otra unidad para guardar lo recuperado.</p>}

            <div>
              <span className="mb-1.5 block text-xs text-dim">Qué buscar</span>
              <div className="grid grid-cols-2 gap-1.5">
                {KINDS.map(([k, label]) => (
                  <label key={k} className="flex items-center gap-2 text-sm text-ink">
                    <input
                      type="checkbox"
                      checked={kinds.includes(k)}
                      onChange={() => setKinds(kinds.includes(k) ? kinds.filter((x) => x !== k) : [...kinds, k])}
                      className="size-4 accent-[var(--color-neon)]"
                    />
                    {label}
                  </label>
                ))}
              </div>
              <input value={folder} onChange={(e) => setFolder(e.target.value)} placeholder="Opcional: una carpeta concreta, p. ej. \Users\Ana\Desktop\" className={`mt-2 ${inputClass}`} />
            </div>

            <div>
              <span className="mb-1.5 block text-xs text-dim">Búsqueda</span>
              <div className="flex flex-col gap-1.5 text-sm">
                <label className="flex items-start gap-2">
                  <input type="radio" checked={!extensive} onChange={() => setExtensive(false)} className="mt-1 accent-[var(--color-neon)]" />
                  <span>
                    <span className="text-ink">Rápida</span>
                    <span className="block text-xs text-mute">Borrados hace poco en un disco NTFS en buen estado.</span>
                  </span>
                </label>
                <label className="flex items-start gap-2">
                  <input type="radio" checked={extensive} onChange={() => setExtensive(true)} className="mt-1 accent-[var(--color-neon)]" />
                  <span>
                    <span className="text-ink">A fondo</span>
                    <span className="block text-xs text-mute">Hace tiempo, tras formatear, disco dañado, tarjetas y USB (FAT/exFAT). Puede tardar horas.</span>
                  </span>
                </label>
              </div>
              {fsOf && fsOf !== "NTFS" && !extensive && <p className="mt-1 text-xs text-mute">La unidad es {fsOf}: se usará la búsqueda a fondo, la única que funciona con ella.</p>}
            </div>

            <div className="flex justify-end">
              <Button onClick={run} disabled={busy !== null || !source || !dest || source === dest || (kinds.length === 0 && !folder.trim())}>
                <FileSearch size={14} /> Buscar y recuperar
              </Button>
            </div>
            <TaskStatus task="recover" active={busy === "run"} fallback="Buscando…" />
            {result && (
              <p className="rounded-md bg-ok/10 px-3 py-2 text-sm text-ink">
                {result.files > 0 ? `${result.files} archivos recuperados` : "No se encontró nada con esos criterios"} en {friendlyPath(result.folder)}.
                {result.files === 0 && !extensive && " Prueba la búsqueda a fondo."}
              </p>
            )}
          </div>
        )}
      </Card>
    </div>
  );
}
