// Miniatura de AdminOps para Ajustes → Apariencia: se pinta con los mismos
// colores de la app, así que enseña al momento el tema y el color elegidos, y
// además el tamaño, la densidad de la barra lateral y el ancho de las pantallas.
import { usePrefs, ZOOMS } from "../../lib/prefs";

const ROWS = { compact: 5, normal: 4, comfortable: 3 } as const;

export function AppPreview() {
  const prefs = usePrefs();
  const scale = prefs.zoom;
  const rows = ROWS[prefs.sidebar.density as keyof typeof ROWS] ?? 4;
  const centered = prefs.pageWidth === "limited";
  const bar = (w: string, accent = false) => <div className={`h-1.5 rounded-full ${accent ? "bg-neon" : "bg-line-2"}`} style={{ width: w }} />;

  return (
    <div className="rounded-xl border border-line bg-panel p-3">
      <div className="mb-2 flex items-baseline justify-between text-xs">
        <span className="font-medium text-ink">Así se verá</span>
        <span className="text-mute">{ZOOMS.find((z) => z.value === prefs.zoom)?.label ?? "Normal"}</span>
      </div>
      <div className="flex h-44 overflow-hidden rounded-lg border border-line bg-void" aria-hidden>
        {/* Barra lateral: columna de áreas y su lista */}
        <div className="flex w-5 flex-col items-center gap-1.5 border-r border-line bg-panel pt-2">
          {Array.from({ length: 5 }, (_, i) => (
            <span key={i} className={`size-2.5 rounded ${i === 1 ? "bg-neon/70" : "bg-line-2"}`} />
          ))}
        </div>
        <div className="flex w-14 flex-col gap-1 border-r border-line bg-panel px-1.5 pt-2">
          {Array.from({ length: rows }, (_, i) => (
            <span key={i} className={`h-2 rounded ${i === 0 ? "bg-neon/25" : "bg-line"}`} style={{ marginBottom: rows === 3 ? 4 : rows === 5 ? 0 : 2 }} />
          ))}
        </div>
        {/* Contenido */}
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-5 items-center gap-1 border-b border-line px-2">
            <span className="h-2 w-10 rounded bg-line-2" />
            <span className="ml-auto size-2 rounded-full bg-ok" />
          </div>
          <div className={`flex flex-1 flex-col gap-1.5 p-2 ${centered ? "mx-auto w-3/4" : "w-full"}`} style={{ fontSize: 9 * scale }}>
            <div className="rounded border border-line bg-panel px-2 py-1.5">
              <span className="font-semibold text-ink">Todo en orden</span>
              <div className="mt-1 flex gap-1">
                <span className="rounded bg-neon px-1.5 text-[0.85em] text-on-neon">Revisar</span>
                <span className="rounded border border-line-2 px-1.5 text-[0.85em] text-dim">Informe</span>
              </div>
            </div>
            <div className="grid grid-cols-3 gap-1">
              {["CPU", "RAM", "Disco"].map((k, i) => (
                <div key={k} className="rounded border border-line bg-panel p-1">
                  <div className="text-mute">{k}</div>
                  {bar(`${[60, 80, 45][i]}%`, i === 0)}
                </div>
              ))}
            </div>
            <div className="flex flex-col gap-1 rounded border border-line bg-panel p-1.5">
              {bar("90%")}
              {bar("70%")}
            </div>
          </div>
        </div>
      </div>
      <p className="mt-2 text-[11px] text-mute">Los cambios se aplican al momento en toda la app; esto es solo para verlos de un vistazo.</p>
    </div>
  );
}
