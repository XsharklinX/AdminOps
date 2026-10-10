// Ajustes → Apariencia.
import { useState } from "react";
import { Volume2 } from "lucide-react";
import { Card, Toggle, inputClass, smallBtn } from "../../components/ui";
import { type Accent, ACCENTS, previewAccent, setPrefs, usePrefs, ZOOMS } from "../../lib/prefs";
import { getSchedule, getTheme, getThemeMode, setSchedule, setThemeMode, type ThemeMode } from "../../lib/theme";
import { playSound } from "../../lib/sounds";
import { celebrate } from "../../components/Celebrate";
import { Row } from "./shared";

export function Appearance() {
  const prefs = usePrefs();
  const [theme, setThemeState] = useState<ThemeMode>(getThemeMode);
  const [schedule, setScheduleState] = useState(getSchedule);
  const pick = (t: ThemeMode) => {
    setThemeMode(t);
    setThemeState(t);
  };
  const hours = Array.from({ length: 24 }, (_, h) => h);
  const changeSchedule = (patch: Partial<typeof schedule>) => {
    const next = { ...schedule, ...patch };
    setScheduleState(next);
    setSchedule(next);
  };
  const option = (
    t: ThemeMode,
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
        <div className="grid gap-3 sm:grid-cols-2">
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
          {option("auto", "Como Windows", "Cambia solo cuando cambia Windows", "linear-gradient(135deg,#111315 50%,#f6f6f4 50%)", "#7e8691")}
          {option("schedule", "Por horario", "Claro de día y oscuro por la tarde", "linear-gradient(180deg,#f6f6f4 50%,#111315 50%)", "#7e8691")}
        </div>
        {theme === "schedule" && (
          <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-dim">
            Claro desde las
            <select className={`${inputClass} w-20 py-1`} value={schedule.lightFrom} onChange={(e) => changeSchedule({ lightFrom: Number(e.target.value) })} aria-label="Hora a la que empieza el tema claro">
              {hours.map((h) => (
                <option key={h} value={h}>{`${h}:00`}</option>
              ))}
            </select>
            y oscuro desde las
            <select className={`${inputClass} w-20 py-1`} value={schedule.darkFrom} onChange={(e) => changeSchedule({ darkFrom: Number(e.target.value) })} aria-label="Hora a la que empieza el tema oscuro">
              {hours.map((h) => (
                <option key={h} value={h}>{`${h}:00`}</option>
              ))}
            </select>
            <span className="text-mute">· ahora toca el {getTheme() === "light" ? "claro" : "oscuro"}</span>
          </div>
        )}
      </Card>
      <Card title="Color de acento">
        <div className="flex flex-wrap gap-2">
          {(Object.keys(ACCENTS) as Accent[]).map((a) => (
            <button
              key={a}
              onClick={() => setPrefs({ accent: a })}
              onPointerEnter={() => previewAccent(a)}
              onPointerLeave={() => previewAccent(null)}
              onFocus={() => previewAccent(a)}
              onBlur={() => previewAccent(null)}
              aria-pressed={prefs.accent === a}
              className={`flex items-center gap-2 rounded-lg border px-3 py-2 text-sm ${prefs.accent === a ? "border-neon text-ink" : "border-line text-dim hover:border-line-2"}`}
            >
              <span
                className="size-4 rounded-full"
                style={{
                  background:
                    getTheme() === "light" ? ACCENTS[a].light : ACCENTS[a].dark,
                }}
              />
              {ACCENTS[a].label}
            </button>
          ))}
        </div>
        <p className="mt-2 text-[11px] text-mute">
          Se usa en la selección, los botones principales y los enlaces. Pasa el
          ratón por uno para verlo en toda la app sin guardarlo. Los estados
          (bien, aviso, error) no cambian. «Alto contraste» además marca más los
          bordes y los textos secundarios.
        </p>
      </Card>
      <Card title="Detalles del aspecto">
        <Row title="Consejos mientras esperas" sub="Debajo de las tareas largas, un consejo corto sobre algo que AdminOps sabe hacer.">
          <Toggle checked={prefs.tips} onChange={(v) => setPrefs({ tips: v })} />
        </Row>
        <Row title="Estados con forma" sub="Bien es un círculo, aviso un triángulo y problema un cuadrado, además del color: se distinguen con daltonismo o con poca luz.">
          <Toggle checked={prefs.stateShapes} onChange={(v) => setPrefs({ stateShapes: v })} />
        </Row>
        <Row title="Resplandor de estado en el Panel" sub="Una luz suave detrás del veredicto: verde si todo va bien, ámbar si hay avisos y roja si hay problemas.">
          <Toggle checked={prefs.aura} onChange={(v) => setPrefs({ aura: v })} />
        </Row>
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
        <Row title="Celebrar cuando todo queda en orden" sub="Un ✓ grande y breve al resolver lo último pendiente de una revisión.">
          <span className="flex items-center gap-2">
            <button className={smallBtn} onClick={() => celebrate("Así se ve", true)}>
              Ver
            </button>
            <Toggle checked={prefs.celebrate} onChange={(v) => setPrefs({ celebrate: v })} />
          </span>
        </Row>
      </Card>
      <Card title="Sonidos">
        <Row title="Sonidos suaves" sub="Un «tic» al terminar una tarea larga y otro tono si algo falla. Se callan solos mientras el micrófono o la cámara están en uso (llamadas y reuniones).">
          <Toggle checked={prefs.sounds} onChange={(v) => setPrefs({ sounds: v })} />
        </Row>
        <Row title="Volumen" sub="Independiente del volumen del resto del equipo.">
          <span className="flex items-center gap-2">
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={prefs.soundVolume}
              onChange={(e) => setPrefs({ soundVolume: Number(e.target.value) })}
              aria-label="Volumen de los sonidos"
              className="w-32 accent-[var(--color-neon)]"
            />
            <button className={smallBtn} onClick={() => void playSound("done", true)} title="Probar el sonido de «terminado»">
              <Volume2 size={12} /> Probar
            </button>
            <button className={smallBtn} onClick={() => void playSound("error", true)} title="Probar el sonido de «falló»">
              Error
            </button>
          </span>
        </Row>
      </Card>
    </div>
  );
}
