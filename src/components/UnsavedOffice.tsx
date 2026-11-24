import { FileSpreadsheet, FileText, File as FileIcon, History, Presentation } from "lucide-react";
import type { ReactNode } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { unsavedApi, type RecentDoc, type UnsavedFile, type UnsavedVersion } from "../lib/api";
import { bytes, friendlyPath } from "../lib/format";
import { usePageActive } from "../lib/pageActive";
import { canOpenCopy, EMPTY_UNSAVED, rowLine, sortUnsaved, versionsSentence, whenText } from "../lib/unsaved";
import { AUDIT_TITLE, useAuditOn } from "../lib/useAudit";
import { ViewCommand } from "./ActionCommand";
import { NeedsAdmin } from "./AdminBanner";
import { DataTable, type Column } from "./DataTable";
import { useToast } from "./feedback";
import { Button, Card, ErrorState, Loading } from "./ui";

const ICON: Record<string, ReactNode> = {
  Word: <FileText size={16} className="shrink-0 text-dim" />,
  Excel: <FileSpreadsheet size={16} className="shrink-0 text-dim" />,
  PowerPoint: <Presentation size={16} className="shrink-0 text-dim" />,
};
const appIcon = (app: string) => ICON[app] ?? <FileIcon size={16} className="shrink-0 text-dim" />;

/** La vista previa de una fila: los datos del archivo y, si es de Office, las primeras líneas de texto. */
function Preview({ file }: { file: UnsavedFile }) {
  const [lines, setLines] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let vivo = true;
    setLines(null);
    setError(null);
    unsavedApi
      .preview(file.path)
      .then((p) => vivo && setLines(p.lines))
      .catch((e) => vivo && setError(String(e)));
    return () => {
      vivo = false;
    };
  }, [file.path]);
  return (
    <div className="space-y-2 px-4 py-3 text-sm">
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
        <dt className="text-mute">Nombre</dt>
        <dd className="text-ink">{file.name}</dd>
        <dt className="text-mute">Tamaño</dt>
        <dd className="text-ink">{bytes(file.size)}</dd>
        <dt className="text-mute">Fecha</dt>
        <dd className="text-ink">{whenText(file.modified)}</dd>
        <dt className="text-mute">Origen</dt>
        <dd className="text-ink">{file.origin}</dd>
        <dt className="text-mute">Ruta</dt>
        <dd className="min-w-0 truncate font-mono text-xs text-dim" title={friendlyPath(file.path)}>
          {friendlyPath(file.path)}
        </dd>
      </dl>
      {error ? (
        <p className="text-xs text-bad">{error}</p>
      ) : lines === null ? (
        <p className="text-xs text-mute">Leyendo…</p>
      ) : lines.length ? (
        <ul className="rounded-md bg-void/60 px-3 py-2 text-xs text-dim select-text">
          {lines.map((l, i) => (
            <li key={i} className="truncate">
              {l}
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-xs text-mute">Sin vista previa del contenido: solo los datos.</p>
      )}
    </div>
  );
}

/** Recuperar archivos → Sin guardar de Office. Solo lee; «Abrir una copia» no toca el original. */
export function UnsavedOffice({ isAdmin }: { isAdmin: boolean }) {
  const [files, setFiles] = useState<UnsavedFile[] | null>(null);
  const [recent, setRecent] = useState<RecentDoc[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  // Las versiones anteriores del archivo elegido.
  const [chosen, setChosen] = useState<{ path: string; name: string } | null>(null);
  const [versions, setVersions] = useState<{ versions: UnsavedVersion[]; snapshots: number } | null>(null);
  const [versionsError, setVersionsError] = useState<string | null>(null);
  // La ruta que se pidió la última vez: una respuesta de otra anterior se descarta.
  const asked = useRef<string | null>(null);
  const toast = useToast();
  const active = usePageActive();
  const audit = useAuditOn();

  const load = useCallback(async () => {
    try {
      const l = await unsavedApi.list();
      setFiles(sortUnsaved(l.files));
      setRecent(l.recent);
      setError(null);
      setNow(Date.now());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Al volver a esta pantalla puede haber algo nuevo; el «hace n min» corre mientras se ve.
  useEffect(() => {
    if (!active) return;
    void load();
    const t = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(t);
  }, [active, load]);

  const copy = async (path: string, version?: string) => {
    try {
      const r = await unsavedApi.copy(path, version);
      toast("ok", r.message);
    } catch (e) {
      toast("error", String(e));
    }
  };

  const choose = async (path: string, name: string) => {
    asked.current = path;
    setChosen({ path, name });
    setVersions(null);
    setVersionsError(null);
    try {
      const found = await unsavedApi.versions(path);
      if (asked.current === path) setVersions(found);
    } catch (e) {
      if (asked.current === path) setVersionsError(String(e));
    }
  };

  const pick = async () => {
    try {
      const path = await unsavedApi.pick();
      if (path) await choose(path, path.split("\\").pop() ?? path);
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (error && !files) return <ErrorState message={error} onRetry={() => void load()} where="Recuperar archivos · Sin guardar de Office" />;
  if (!files) return <Loading text="Buscando lo que Office dejó sin guardar…" />;

  const columns: Column<UnsavedFile>[] = [
    {
      id: "name",
      header: "Archivo",
      sortBy: (f) => f.name,
      cell: (f) => (
        <div className="flex min-w-0 items-center gap-2">
          {appIcon(f.app)}
          <span className="truncate text-ink" title={friendlyPath(f.path)}>
            {rowLine(f, now)}
          </span>
        </div>
      ),
    },
    { id: "app", header: "Programa", sortBy: (f) => f.app, className: "whitespace-nowrap text-dim", cell: (f) => f.app },
    {
      id: "actions",
      header: "",
      align: "right",
      stopClick: true,
      cell: (f) => (
        <div className="flex items-center justify-end gap-1">
          <Button size="sm" kind="secondary" disabled={audit} title={audit ? AUDIT_TITLE : canOpenCopy(f) ? undefined : "No se sabe con qué programa abrirlo: la copia se deja en el Escritorio"} onClick={() => copy(f.path)}>
            Abrir una copia
          </Button>
          <ViewCommand id="unsaved.copy" context={f.path} icon />
        </div>
      ),
    },
  ];

  const sentence = versions ? versionsSentence(versions.snapshots, versions.versions.length) : null;
  return (
    <div className="space-y-4">
      <Card title="Sin guardar de Office">
        <p className="mb-3 text-sm text-dim">Lo que Word, Excel y PowerPoint dejaron por si se cerraban mal. Solo se lee: abrir una copia no toca el original.</p>
        <DataTable
          padded
          rows={files}
          rowKey={(f) => f.path}
          columns={columns}
          onRowClick={(f) => setOpen(open === f.path ? null : f.path)}
          selected={(f) => f.path === open}
          expanded={(f) => (f.path === open ? <Preview file={f} /> : null)}
          empty={EMPTY_UNSAVED}
        />
      </Card>

      <Card title="Versiones anteriores de Windows" icon={<History size={14} />}>
        {!isAdmin && <NeedsAdmin className="mb-3">Buscar en las copias de «Versiones anteriores» necesita administrador.</NeedsAdmin>}
        <div className="flex flex-wrap items-center gap-2">
          {recent.map((d) => (
            <Button key={d.path} kind={chosen?.path === d.path ? "primary" : "secondary"} size="sm" disabled={!isAdmin} title={friendlyPath(d.path)} onClick={() => choose(d.path, d.name)}>
              {d.name}
            </Button>
          ))}
          <Button kind="ghost" size="sm" disabled={!isAdmin} onClick={pick}>
            Elegir otro archivo…
          </Button>
        </div>
        {chosen && (
          <div className="mt-3 border-t border-line pt-3 text-sm">
            <div className="mb-2 text-ink">{chosen.name}</div>
            {versionsError ? (
              <p className="text-xs text-bad">{versionsError}</p>
            ) : !versions ? (
              <p className="text-xs text-mute">Buscando en las copias de Windows…</p>
            ) : sentence ? (
              <p className="text-dim">{sentence}</p>
            ) : (
              <ul className="divide-y divide-line/60">
                {versions.versions.map((v) => (
                  <li key={v.id} className="flex items-center gap-3 py-1.5">
                    <span className="text-ink">{whenText(v.when)}</span>
                    <span className="text-dim">{bytes(v.size)}</span>
                    <span className="ml-auto">
                      <Button size="sm" kind="secondary" disabled={audit} title={audit ? AUDIT_TITLE : undefined} onClick={() => copy(chosen.path, v.id)}>
                        Abrir una copia
                      </Button>
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </Card>
    </div>
  );
}
