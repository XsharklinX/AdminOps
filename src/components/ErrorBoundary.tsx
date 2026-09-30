import { TriangleAlert } from "lucide-react";
import { Component, type ErrorInfo, type ReactNode } from "react";
import { appApi } from "../lib/api";

/**
 * Si una página falla al pintarse, muestra el error en lugar de dejar la
 * ventana en negro, y lo deja en el registro técnico (Historial → Registro).
 */
export class ErrorBoundary extends Component<{ children: ReactNode; onHome?: () => void }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    void appApi.logError(`${error.name}: ${error.message}\n${error.stack ?? ""}\n${info.componentStack ?? ""}`);
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="mx-auto max-w-2xl p-8">
        <div className="rounded-xl border border-bad/40 bg-bad/5 p-5">
          <h2 className="mb-2 flex items-center gap-2 font-semibold text-bad">
            <TriangleAlert size={18} /> Esta sección ha fallado
          </h2>
          <p className="text-sm text-dim">
            El error quedó guardado en el registro técnico (Historial → Registro técnico). El resto de AdminOps sigue funcionando.
          </p>
          <pre className="mt-3 pane-sm overflow-auto rounded-md bg-void/60 p-3 font-mono text-[11px] whitespace-pre-wrap text-mute select-text">
            {error.name}: {error.message}
          </pre>
          <div className="mt-4 flex gap-2">
            <button
              onClick={() => this.setState({ error: null })}
              className="rounded-md border border-neon/50 px-3.5 py-1.5 text-sm text-neon hover:bg-neon/10"
            >
              Reintentar
            </button>
            {this.props.onHome && (
              <button onClick={this.props.onHome} className="rounded-md px-3.5 py-1.5 text-sm text-dim hover:bg-panel-2 hover:text-ink">
                Ir al Panel
              </button>
            )}
            {!this.props.onHome && (
              <button onClick={() => location.reload()} className="rounded-md px-3.5 py-1.5 text-sm text-dim hover:bg-panel-2 hover:text-ink">
                Recargar AdminOps
              </button>
            )}
          </div>
        </div>
      </div>
    );
  }
}
