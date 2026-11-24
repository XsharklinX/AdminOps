import { Keyboard as KeyboardIcon, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { ViewCommand } from "../components/ActionCommand";
import { DataTable, type Column } from "../components/DataTable";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, ErrorState, Loading, iconBtn, inputClass } from "../components/ui";
import { kbdApi, tweaksApi, type KbdState } from "../lib/api";
import { analyzeKeyTest, canKeepOnlySpanish, keyTestSentence, KIND_LABEL, markRows, OTHER_USER_SENTENCE, shortcutLine, type MarkedRow } from "../lib/kbdlang";
import { usePageActive } from "../lib/pageActive";
import { AUDIT_TITLE, useAuditOn } from "../lib/useAudit";

/** Estado del equipo → Teclado e idioma: qué distribuciones hay, el atajo que cambia solo y la prueba de «ñ @ ´». */
export function Keyboard() {
  const [state, setState] = useState<KbdState | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Lo último que se hizo, con la entrada del diario para deshacerlo.
  const [done, setDone] = useState<{ message: string; entryId: number } | null>(null);
  const [typed, setTyped] = useState("");
  const [focused, setFocused] = useState(false);
  const [busy, setBusy] = useState(false);
  const { confirm, dialog } = useConfirm();
  const toast = useToast();
  const active = usePageActive();
  const audit = useAuditOn();

  const load = useCallback(async () => {
    try {
      setState(await kbdApi.state());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Al volver a la pestaña los idiomas pueden haber cambiado (Configuración de Windows, otro programa).
  useEffect(() => {
    if (active) void load();
  }, [active, load]);

  const keepSpanish = async () => {
    if (busy) return;
    // Si quitar un idioma puede cambiar el idioma de Windows, se pregunta antes.
    if (state?.warning) {
      const ok = await confirm({ title: "¿Dejar solo Español?", body: state.warning, confirmLabel: "Dejar solo Español" });
      if (!ok) return;
    }
    setBusy(true);
    try {
      setDone(await kbdApi.onlySpanish());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
      await load();
    }
  };

  const undo = async () => {
    if (!done || busy) return;
    setBusy(true);
    try {
      await tweaksApi.revertEntry(done.entryId);
      setDone(null);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
      await load();
    }
  };

  if (error && !state) return <ErrorState message={error} onRetry={() => void load()} where="Estado del equipo · Teclado e idioma" />;
  if (!state) return <Loading text="Leyendo los idiomas de teclado…" page />;

  const rows = markRows(state.rows, state.region);
  const line = shortcutLine(state);
  const test = analyzeKeyTest(typed);
  const lockTitle = audit ? AUDIT_TITLE : undefined;

  const columns: Column<MarkedRow>[] = [
    {
      id: "language",
      header: "Idioma y distribución",
      cell: (r) => (
        <div className="min-w-0">
          <div className="truncate text-ink">{r.language}</div>
          <div className="text-xs text-dim">{r.layout}</div>
        </div>
      ),
    },
    {
      id: "kind",
      header: "",
      className: (r) => `whitespace-nowrap ${r.kind === "mine" ? "text-ok" : "text-warn"}`,
      cell: (r) => KIND_LABEL[r.kind],
    },
    { id: "active", header: "", className: "whitespace-nowrap text-dim", cell: (r) => (r.active ? "En uso ahora" : "") },
  ];

  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="text-sm text-dim">
          {state.languageCount} {state.languageCount === 1 ? "idioma" : "idiomas"} de teclado
        </p>
        <div className="ml-auto">
          <button onClick={() => void load()} className={iconBtn} title="Volver a leer" aria-label="Volver a leer">
            <RefreshCw size={14} />
          </button>
        </div>
      </div>

      {done && (
        <div className="flex flex-wrap items-center gap-3 rounded-lg border border-neon/40 bg-neon/5 px-3 py-2 text-sm" role="status">
          <span className="text-ink">{done.message}</span>
          <div className="ml-auto">
            <Button kt="D" kind="secondary" size="sm" disabled={audit || busy} title={lockTitle} onClick={undo}>
              Deshacer
            </Button>
          </div>
        </div>
      )}

      <Card title="Idiomas de teclado" icon={<KeyboardIcon size={14} />}>
        <DataTable padded rows={rows} rowKey={(r, i) => `${r.tag}-${r.klid ?? i}`} columns={columns} empty="Windows no tiene ningún idioma de teclado para este usuario." />
        <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-line pt-3 text-sm">
          <span className="text-ink">{line.text}</span>
          <span className={line.risky ? "text-warn" : "text-dim"}>{line.note}</span>
          <span className={line.on ? "text-warn" : "text-ok"}>{line.on ? "Activo" : "Quitado"}</span>
        </div>
      </Card>

      <Card title="Probar el teclado">
        <p className="mb-2 text-sm text-dim">{keyTestSentence({ state: "empty" })}</p>
        <div className="flex items-center gap-2">
          <input
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            onFocus={() => setFocused(true)}
            onBlur={() => setFocused(false)}
            // Mientras se prueba, las letras con Alt son del teclado, no de los botones.
            data-keytest={focused ? "true" : undefined}
            autoComplete="off"
            spellCheck={false}
            aria-label="Prueba del teclado"
            className={`${inputClass} h-14 text-2xl`}
          />
          <Button kt="B" kind="ghost" size="sm" disabled={!typed} onClick={() => setTyped("")}>
            Borrar
          </Button>
        </div>
        {test.state !== "empty" && <p className={`mt-2 text-sm ${test.state === "ok" ? "text-ok" : "text-warn"}`}>{keyTestSentence(test)}</p>}
      </Card>

      <Card title="Arreglo">
        {state.otherUser ? (
          <p className="text-sm text-dim">{OTHER_USER_SENTENCE}</p>
        ) : canKeepOnlySpanish(state) ? (
          <div className="space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <Button kt="E" disabled={audit || busy} title={lockTitle} onClick={keepSpanish}>
                Dejar solo Español y quitar el atajo
              </Button>
              <ViewCommand id="kbd.only-spanish" icon />
            </div>
            {state.note && <p className="text-xs text-dim">{state.note}</p>}
          </div>
        ) : state.hasSpanish ? (
          <p className="text-sm text-dim">Solo hay Español y no hay atajo: no hay nada que quitar.</p>
        ) : (
          <p className="text-sm text-dim">No hay ningún idioma español instalado: no se puede dejar solo Español.</p>
        )}
      </Card>
      {dialog}
    </div>
  );
}
