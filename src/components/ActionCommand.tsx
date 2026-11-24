// «Ver el comando»: el PowerShell que hace una acción, comentado en español, para
// copiarlo o guardarlo como .ps1 y llevarlo a un equipo donde AdminOps no abre.
// El texto sale de Rust (el mismo script que se ejecuta), no está copiado aquí.
import { Copy, Download, Terminal, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { actionsApi, type ActionCommand } from "../lib/api";
import { Drawer } from "./Drawer";
import { useToast } from "./feedback";
import { Button, iconBtn, Loading } from "./ui";

/** Las líneas de comentario (#) van en el color suave; el resto, como código. */
function Code({ script }: { script: string }) {
  const lines = script.replace(/\s+$/, "").split("\n");
  return (
    <pre className="overflow-x-auto rounded-lg border border-line bg-void/60 p-3 font-mono text-[12px] leading-relaxed select-text" aria-label="Comando de PowerShell">
      {lines.map((l, i) => (
        <div key={i} className={l.trimStart().startsWith("#") ? "text-mute" : "text-ink"}>
          {l || " "}
        </div>
      ))}
    </pre>
  );
}

/**
 * El enlace pequeño «Ver el comando» de al lado del botón de una acción, y el cajón
 * con el comando. `id`: la acción (un id del catálogo o una de las de AdminOps);
 * `context`: lo que rellena el comando (la impresora, el programa del inicio).
 */
export function ViewCommand({ id, context, className = "", icon = false }: { id: string; context?: string; className?: string; /** Solo el icono, para filas donde no cabe el texto. */ icon?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      {icon ? (
        <button type="button" onClick={() => setOpen(true)} className={`${iconBtn} ${className}`} title="Ver el comando" aria-label="Ver el comando">
          <Terminal size={15} />
        </button>
      ) : (
        <button type="button" onClick={() => setOpen(true)} className={`text-xs text-mute hover:text-ink hover:underline ${className}`}>
          Ver el comando
        </button>
      )}
      {open && <CommandDrawer id={id} context={context} onClose={() => setOpen(false)} />}
    </>
  );
}

function CommandDrawer({ id, context, onClose }: { id: string; context?: string; onClose: () => void }) {
  const [cmd, setCmd] = useState<ActionCommand | null>(null);
  const [error, setError] = useState<string | null>(null);
  const toast = useToast();

  useEffect(() => {
    let vivo = true;
    actionsApi
      .command(id, context)
      .then((c) => vivo && setCmd(c))
      .catch((e) => vivo && setError(String(e)));
    return () => {
      vivo = false;
    };
  }, [id, context]);

  const copy = async () => {
    if (!cmd) return;
    try {
      await navigator.clipboard.writeText(cmd.script);
      toast("ok", "Comando copiado");
    } catch {
      toast("error", "No se pudo copiar.");
    }
  };

  const save = async () => {
    try {
      const name = await actionsApi.saveScript(id, context);
      if (name) toast("ok", "Guardado");
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <Drawer title={cmd?.name ?? "Ver el comando"} subtitle="Lo que hace AdminOps, paso a paso" onClose={onClose} width={560}>
      <div className="space-y-3">
        {error && <p className="text-sm text-bad">{error}</p>}
        {!cmd && !error && <Loading />}
        {cmd && (
          <>
            {cmd.approximate && (
              <p className="flex gap-1.5 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn">
                <TriangleAlert size={13} className="mt-px shrink-0" />
                AdminOps lo hace con funciones de Windows; esto es lo equivalente en PowerShell.
              </p>
            )}
            <Code script={cmd.script} />
            <div className="flex flex-wrap gap-2">
              <Button onClick={() => void copy()}>
                <Copy size={14} /> Copiar
              </Button>
              <Button kind="secondary" onClick={() => void save()}>
                <Download size={14} /> Guardar como .ps1
              </Button>
            </div>
          </>
        )}
      </div>
    </Drawer>
  );
}
