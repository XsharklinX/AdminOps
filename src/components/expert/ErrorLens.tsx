// Lector de errores: pega o suelta el recorte (o la foto) de un mensaje de
// error, o pega el texto, y dice qué significa cada código y qué arreglo de
// AdminOps lo resuelve. El texto se lee con el OCR de Windows, sin nube.
import { ClipboardPaste, Loader2, ScanText } from "lucide-react";
import { useCallback, useState } from "react";
import { Card, EmptyLine, smallBtn, softBtn } from "../ui";
import { isPageId, pageLabel, type PageId } from "../Sidebar";
import { diagnoseApi, type ErrorFound, type ErrorLens as Lens } from "../../lib/api";
import { useDropped } from "../../lib/dropped";
import { BUILTIN_SOLUTIONS } from "../../lib/solutionsCatalog";

/** Soluciones de AdminOps que nombran este código o este error. */
export function solutionsFor(f: ErrorFound) {
  const needles = [f.raw.replace(/^0x0*/i, "").toLowerCase(), f.info?.name.toLowerCase() ?? ""].filter((n) => n.length >= 3);
  return BUILTIN_SOLUTIONS.filter((s) => {
    const hay = `${s.title} ${s.problem} ${s.solution} ${s.tags.join(" ")}`.toLowerCase();
    return needles.some((n) => hay.includes(n));
  }).slice(0, 2);
}

/** Una imagen pegada o soltada en el navegador, como base64 sin la cabecera. */
function toBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result).split(",")[1] ?? "");
    r.onerror = () => reject(new Error("No se pudo leer la imagen."));
    r.readAsDataURL(file);
  });
}

export function ErrorLens({ onNavigate }: { onNavigate: (page: PageId, focus?: string | null) => void }) {
  const [lens, setLens] = useState<Lens | null>(null);
  const [reading, setReading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [text, setText] = useState("");

  const run = useCallback(async (job: () => Promise<Lens>) => {
    setReading(true);
    setError(null);
    try {
      setLens(await job());
    } catch (e) {
      setLens(null);
      setError(String(e));
    } finally {
      setReading(false);
    }
  }, []);

  // Una imagen soltada sobre la ventana (la ruta llega desde DropLayer).
  useDropped("errorimage", (path) => void run(() => diagnoseApi.lensFile(path)));

  const onPaste = (e: React.ClipboardEvent) => {
    const img = Array.from(e.clipboardData.items).find((i) => i.type.startsWith("image/"));
    if (!img) return;
    const file = img.getAsFile();
    if (!file) return;
    e.preventDefault();
    void run(async () => diagnoseApi.lensImage(await toBase64(file)));
  };

  return (
    <Card id="errorlens" title="Lector de errores" icon={<ScanText size={14} />} className="col-span-12">
      <div onPaste={onPaste}>
        <p className="mb-3 text-xs text-dim">
          Pega aquí (Ctrl+V) el recorte o la foto del mensaje de error, o arrástrala sobre la ventana. Lee el texto de la imagen y te dice qué significa el código y cómo se arregla. También vale un texto pegado.
        </p>
        <div className="flex flex-wrap items-start gap-2">
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            rows={2}
            placeholder="Pega aquí la imagen (Ctrl+V) o escribe el texto del error…"
            className="min-w-60 flex-1 resize-y rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
            aria-label="Texto del error"
          />
          <div className="flex flex-col gap-2">
            <button onClick={() => void run(() => diagnoseApi.lensText(text))} disabled={reading || !text.trim()} className={softBtn}>
              {reading ? <Loader2 size={12} className="animate-spin" /> : <ScanText size={12} />} Leer el texto
            </button>
            <button onClick={() => void run(() => diagnoseApi.lensClipboard())} disabled={reading} className={smallBtn} title="Usa la imagen que hay ahora en el portapapeles (por ejemplo, tras Win + Mayús + S)">
              <ClipboardPaste size={12} /> Leer el recorte copiado
            </button>
          </div>
        </div>

        {reading && (
          <p className="mt-3 flex items-center gap-2 text-sm text-mute">
            <Loader2 size={14} className="animate-spin" /> Leyendo…
          </p>
        )}
        {error && <p className="mt-3 text-sm text-bad">{error}</p>}

        {lens && !reading && (
          <div className="mt-4 space-y-3">
            {lens.text && (
              <div>
                <h4 className="mb-1 text-xs font-medium text-dim">Leído en la imagen</h4>
                <p className="max-h-28 overflow-auto rounded-md border border-line bg-void/40 px-3 py-2 text-xs whitespace-pre-wrap text-dim select-text">{lens.text}</p>
              </div>
            )}
            {lens.codes.length === 0 ? (
              <EmptyLine>{lens.text ? "No encuentro ningún código de error en ese texto. Si hay uno, escríbelo a mano arriba." : "No se leyó ningún texto en la imagen. Prueba con un recorte más grande o más recto."}</EmptyLine>
            ) : (
              lens.codes.map((f) => {
                const sols = solutionsFor(f);
                const page = f.info?.page && isPageId(f.info.page) ? f.info.page : null;
                return (
                  <div key={f.raw} className="rounded-lg border border-line bg-void/40 px-3 py-2.5">
                    <div className="flex flex-wrap items-baseline gap-2">
                      <span className="font-mono text-sm text-neon select-text">{f.raw}</span>
                      <span className="text-sm text-ink">{f.info?.name ?? "Código que AdminOps no conoce"}</span>
                    </div>
                    {f.info ? (
                      <dl className="mt-1.5 space-y-1 text-xs text-dim">
                        <div>
                          <dt className="inline font-medium text-ink">Qué es: </dt>
                          <dd className="inline">{f.info.what}</dd>
                        </div>
                        {f.info.why && (
                          <div>
                            <dt className="inline font-medium text-ink">Por qué suele pasar: </dt>
                            <dd className="inline">{f.info.why}</dd>
                          </div>
                        )}
                        <div>
                          <dt className="inline font-medium text-ink">Qué hacer: </dt>
                          <dd className="inline">{f.info.todo}</dd>
                        </div>
                      </dl>
                    ) : (
                      <p className="mt-1.5 text-xs text-dim">No está en el diccionario de AdminOps. Búscalo en Ctrl+K o en la documentación de quien fabrica el programa.</p>
                    )}
                    {(page || sols.length > 0) && (
                      <div className="mt-2 flex flex-wrap gap-2">
                        {page && (
                          <button onClick={() => onNavigate(page)} className={softBtn}>
                            Ir a {pageLabel(page)}
                          </button>
                        )}
                        {sols.map((s) => (
                          <button key={s.id} onClick={() => onNavigate("knowledge", `solution:${s.id}`)} className={smallBtn}>
                            Solución: {s.title}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                );
              })
            )}
          </div>
        )}
      </div>
    </Card>
  );
}
