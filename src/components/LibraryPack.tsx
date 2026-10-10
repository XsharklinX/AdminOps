// Biblioteca compartida entre técnicos: exportar e importar soluciones,
// plantillas, recetas, reglas de alerta y la plantilla de informe en un archivo
// (.aolib). Sin servidor: viaja por correo o en el pendrive.
import { Download, Library, Upload } from "lucide-react";
import { useState } from "react";
import { useToast } from "./feedback";
import { Button, Card, inputClass, Modal } from "./ui";
import { alertRulesApi, knowledgeApi, libraryApi, workApi } from "../lib/api";
import { bump, diff, PACK_FORMAT, PACK_KINDS, parsePack, type Pack, type PackKind } from "../lib/pack";

const META = "adminops.packMeta";

function readMeta(): { name: string; author: string; version: string } {
  try {
    return { name: "Mi biblioteca", author: "", version: "0", ...JSON.parse(localStorage.getItem(META) ?? "{}") };
  } catch {
    return { name: "Mi biblioteca", author: "", version: "0" };
  }
}

async function current() {
  const [solutions, templates, recipes, alertRules, settings] = await Promise.all([libraryApi.list("solutions"), libraryApi.list("templates"), libraryApi.list("recipes"), alertRulesApi.get(), workApi.settings()]);
  return { solutions, templates, recipes, alertRules, settings };
}

export function LibraryPackCard() {
  const toast = useToast();
  const [exporting, setExporting] = useState(false);
  const [meta, setMeta] = useState(readMeta);
  const [kinds, setKinds] = useState<Record<PackKind, boolean>>({ solutions: true, templates: true, recipes: true, alertRules: false });
  const [incoming, setIncoming] = useState<{ pack: Pack; diffs: Record<PackKind, { added: { id: string }[]; changed: { id: string }[]; same: { id: string }[] }> } | null>(null);
  const [replace, setReplace] = useState(false);
  const [useLayout, setUseLayout] = useState(false);
  const [busy, setBusy] = useState(false);

  const doExport = async () => {
    setBusy(true);
    try {
      const c = await current();
      const version = bump(meta.version);
      const pack: Pack = {
        format: PACK_FORMAT,
        name: meta.name.trim() || "Mi biblioteca",
        author: meta.author.trim(),
        version,
        created: Math.floor(Date.now() / 1000),
        solutions: kinds.solutions ? c.solutions : [],
        templates: kinds.templates ? c.templates : [],
        recipes: kinds.recipes ? c.recipes : [],
        alertRules: kinds.alertRules ? c.alertRules : [],
        reportLayout: c.settings.reportLayout ?? null,
      };
      const path = await knowledgeApi.saveText(`${pack.name} v${version}.aolib`, JSON.stringify(pack, null, 1), "aolib");
      if (path) {
        const m = { ...meta, version };
        setMeta(m);
        try {
          localStorage.setItem(META, JSON.stringify(m));
        } catch {
          /* sin almacenamiento */
        }
        toast("ok", `Biblioteca guardada (versión ${version}). Pásala por correo o en el pendrive.`);
        setExporting(false);
      }
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const pick = async () => {
    try {
      const text = await knowledgeApi.openText("aolib");
      if (text === null) return;
      const pack = parsePack(text);
      if (!pack) return toast("error", "Ese archivo no es una biblioteca de AdminOps.");
      const c = await current();
      setIncoming({ pack, diffs: { solutions: diff(c.solutions, pack.solutions), templates: diff(c.templates, pack.templates), recipes: diff(c.recipes, pack.recipes), alertRules: diff(c.alertRules, pack.alertRules) } });
      setReplace(false);
      setUseLayout(false);
    } catch (e) {
      toast("error", String(e));
    }
  };

  const doImport = async () => {
    if (!incoming) return;
    setBusy(true);
    try {
      const { pack, diffs } = incoming;
      let n = 0;
      for (const kind of ["solutions", "templates", "recipes"] as const) {
        const list = [...diffs[kind].added, ...(replace ? diffs[kind].changed : [])] as Parameters<typeof libraryApi.save>[1][];
        for (const item of list) {
          await libraryApi.save(kind, item);
          n++;
        }
      }
      const rulesToAdd = [...diffs.alertRules.added, ...(replace ? diffs.alertRules.changed : [])].map((r) => pack.alertRules.find((x) => x.id === r.id)!);
      if (rulesToAdd.length) {
        const mine = await alertRulesApi.get();
        await alertRulesApi.save([...mine.filter((r) => !rulesToAdd.some((x) => x.id === r.id)), ...rulesToAdd]);
        n += rulesToAdd.length;
      }
      if (useLayout && pack.reportLayout) {
        const s = await workApi.settings();
        await workApi.saveSettings({ ...s, reportLayout: pack.reportLayout });
      }
      toast("ok", `Importado: ${n} elemento(s) de «${pack.name}» v${pack.version}.`);
      setIncoming(null);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card title="Biblioteca compartida" icon={<Library size={14} />}>
      <p className="mb-3 text-xs text-dim">Pasa a un compañero (o recibe) soluciones, plantillas, recetas y reglas de alerta en un solo archivo. Al importar se ve qué es nuevo y qué cambia; lo tuyo no se pisa salvo que lo digas.</p>
      <div className="flex flex-wrap gap-2">
        <Button kind="secondary" onClick={() => setExporting(true)}>
          <Download size={14} /> Exportar
        </Button>
        <Button kind="secondary" onClick={() => void pick()}>
          <Upload size={14} /> Importar…
        </Button>
      </div>
      {exporting && (
        <Modal
          title="Exportar la biblioteca"
          onClose={() => setExporting(false)}
          footer={
            <>
              <Button kind="ghost" onClick={() => setExporting(false)}>
                Cancelar
              </Button>
              <Button onClick={() => void doExport()} disabled={busy}>
                Guardar archivo
              </Button>
            </>
          }
        >
          <div className="space-y-3 text-sm">
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Nombre</span>
              <input value={meta.name} onChange={(e) => setMeta({ ...meta, name: e.target.value })} className={inputClass} />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Autor</span>
              <input value={meta.author} onChange={(e) => setMeta({ ...meta, author: e.target.value })} placeholder="Tu nombre o el del equipo" className={inputClass} />
            </label>
            <p className="text-xs text-mute">Versión {bump(meta.version)} (sube sola en cada exportación).</p>
            <div className="space-y-1">
              {PACK_KINDS.map((k) => (
                <label key={k.kind} className="flex items-center gap-2 text-xs text-dim">
                  <input type="checkbox" checked={kinds[k.kind]} onChange={(e) => setKinds({ ...kinds, [k.kind]: e.target.checked })} className="accent-[var(--color-neon)]" /> {k.label}
                </label>
              ))}
            </div>
          </div>
        </Modal>
      )}
      {incoming && (
        <Modal
          title={`Importar «${incoming.pack.name}» v${incoming.pack.version}`}
          onClose={() => setIncoming(null)}
          width="w-[560px]"
          footer={
            <>
              <Button kind="ghost" onClick={() => setIncoming(null)}>
                Cancelar
              </Button>
              <Button onClick={() => void doImport()} disabled={busy}>
                Importar
              </Button>
            </>
          }
        >
          <p className="mb-3 text-xs text-mute">
            {incoming.pack.author && `De ${incoming.pack.author} · `}
            {incoming.pack.created ? new Date(incoming.pack.created * 1000).toLocaleDateString("es-ES") : ""}
          </p>
          <table className="w-full text-xs">
            <thead className="text-mute">
              <tr>
                <th className="pb-1 text-left font-medium">Qué</th>
                <th className="pb-1 text-right font-medium">Nuevas</th>
                <th className="pb-1 text-right font-medium">Cambian</th>
                <th className="pb-1 text-right font-medium">Iguales</th>
              </tr>
            </thead>
            <tbody>
              {PACK_KINDS.map((k) => {
                const d = incoming.diffs[k.kind];
                return (
                  <tr key={k.kind} className="border-t border-line/60">
                    <td className="py-1 text-ink">{k.label}</td>
                    <td className="py-1 text-right text-ok">{d.added.length}</td>
                    <td className="py-1 text-right text-warn">{d.changed.length}</td>
                    <td className="py-1 text-right text-mute">{d.same.length}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <label className="mt-3 flex items-center gap-2 text-xs text-dim">
            <input type="checkbox" checked={replace} onChange={(e) => setReplace(e.target.checked)} className="accent-[var(--color-neon)]" /> Sustituir también las que cambian (si no, se conservan tus versiones)
          </label>
          {incoming.pack.reportLayout && (
            <label className="mt-1 flex items-center gap-2 text-xs text-dim">
              <input type="checkbox" checked={useLayout} onChange={(e) => setUseLayout(e.target.checked)} className="accent-[var(--color-neon)]" /> Usar también su plantilla de informe («{incoming.pack.reportLayout.name || "sin nombre"}»)
            </label>
          )}
        </Modal>
      )}
    </Card>
  );
}
