// Ajustes → Apariencia.
import { useState } from "react";
import { Card, Toggle } from "../../components/ui";
import { type Accent, ACCENTS, applyAppearance, setPrefs, usePrefs, ZOOMS } from "../../lib/prefs";
import { getTheme, setTheme, type Theme } from "../../lib/theme";
import { Row } from "./shared";

export function Appearance() {
  const prefs = usePrefs();
  const [theme, setThemeState] = useState<Theme>(getTheme);
  const pick = (t: Theme) => {
    setTheme(t);
    setThemeState(t);
    applyAppearance();
  };
  const option = (
    t: Theme,
    title: string,
    sub: string,
    bg: string,
    bar: string,
  ) => (
    <button
      onClick={() => pick(t)}
      aria-pressed={theme === t}
      className={`flex flex-1 items-center gap-3 rounded-lg border p-3 text-left transition-colors ${theme === t ? "border-neon" : "border-line hover:border-line-2"}`}
    >
      <span
        className="flex h-10 w-14 shrink-0 flex-col justify-end gap-1 rounded-md border border-line-2 p-1.5"
        style={{ background: bg }}
      >
        <span className="h-1 w-8 rounded-full" style={{ background: bar }} />
        <span
          className="h-1 w-5 rounded-full"
          style={{ background: bar, opacity: 0.5 }}
        />
      </span>
      <span>
        <span className="block text-sm font-medium text-ink">{title}</span>
        <span className="block text-xs text-mute">{sub}</span>
      </span>
    </button>
  );
  return (
    <div className="space-y-4">
      <Card title="Tema">
        <div className="flex gap-3">
          {option(
            "dark",
            "Oscuro",
            "Menos brillo en talleres y de noche",
            "#111315",
            "#a5acb5",
          )}
          {option(
            "light",
            "Claro",
            "Más legible con mucha luz y en oficinas",
            "#f6f6f4",
            "#4b5058",
          )}
        </div>
      </Card>
      <Card title="Color de acento">
        <div className="flex flex-wrap gap-2">
          {(Object.keys(ACCENTS) as Accent[]).map((a) => (
            <button
              key={a}
              onClick={() => setPrefs({ accent: a })}
              aria-pressed={prefs.accent === a}
              className={`flex items-center gap-2 rounded-lg border px-3 py-2 text-sm ${prefs.accent === a ? "border-neon text-ink" : "border-line text-dim hover:border-line-2"}`}
            >
              <span
                className="size-4 rounded-full"
                style={{
                  background:
                    theme === "light" ? ACCENTS[a].light : ACCENTS[a].dark,
                }}
              />
              {ACCENTS[a].label}
            </button>
          ))}
        </div>
        <p className="mt-2 text-[11px] text-mute">
          Se usa en la selección, los botones principales y los enlaces. Los
          estados (bien, aviso, error) no cambian.
        </p>
      </Card>
      <Card title="Tamaño y movimiento">
        <Row
          title="Tamaño de la interfaz"
          sub="Textos, botones y espacios, todo a la vez."
        >
          <div className="inline-flex rounded-lg border border-line bg-void p-0.5">
            {ZOOMS.map((z) => (
              <button
                key={z.value}
                onClick={() => setPrefs({ zoom: z.value })}
                className={`rounded-md px-3 py-1.5 text-[13px] ${prefs.zoom === z.value ? "bg-panel-2 font-medium text-ink" : "text-dim hover:text-ink"}`}
              >
                {z.label}
              </button>
            ))}
          </div>
        </Row>
        <Row
          title="Ancho de las pantallas"
          sub="«Toda la ventana» aprovecha un monitor grande para ver más a la vez. «Centrado» deja el ancho de antes, más cómodo para leer en pantallas muy anchas."
        >
          <div className="inline-flex rounded-lg border border-line bg-void p-0.5">
            {(
              [
                ["full", "Toda la ventana"],
                ["limited", "Centrado"],
              ] as const
            ).map(([value, label]) => (
              <button
                key={value}
                onClick={() => setPrefs({ pageWidth: value })}
                aria-pressed={prefs.pageWidth === value}
                className={`rounded-md px-3 py-1.5 text-[13px] ${prefs.pageWidth === value ? "bg-panel-2 font-medium text-ink" : "text-dim hover:text-ink"}`}
              >
                {label}
              </button>
            ))}
          </div>
        </Row>
        <Row
          title="Reducir animaciones"
          sub="Quita transiciones y giros (más cómodo si marean o en equipos lentos)."
        >
          <Toggle checked={prefs.reduceMotion} onChange={(v) => setPrefs({ reduceMotion: v })} />
        </Row>
      </Card>
    </div>
  );
}
