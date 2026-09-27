import {
  AlertTriangle,
  ArrowRight,
  Bluetooth,
  CheckCircle2,
  Gauge,
  Globe,
  Info,
  Loader2,
  Monitor,
  Printer,
  RefreshCw,
  RefreshCcwDot,
  Volume2,
  Wifi,
  Wrench,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { NAV, pageLabel, type PageId } from "../components/Sidebar";
import { Card } from "../components/ui";
import { troubleshootApi, type Symptom, type TroubleFinding, type TroubleFix } from "../lib/api";

export const SYMPTOMS: { id: Symptom; title: string; hint: string; icon: LucideIcon; keywords: string }[] = [
  { id: "internet", title: "No hay Internet", hint: "No cargan las webs, «sin Internet», cortes", icon: Globe, keywords: "red conexion internet navegar dns router proxy" },
  { id: "wifi", title: "La Wi-Fi no funciona", hint: "No aparece, no conecta o se corta", icon: Wifi, keywords: "wifi wireless inalambrica tarjeta" },
  { id: "audio", title: "No suena", hint: "Sin sonido, altavoz con una X, micrófono", icon: Volume2, keywords: "audio sonido altavoces auriculares microfono volumen" },
  { id: "bluetooth", title: "Bluetooth", hint: "No aparece, no empareja o no conecta", icon: Bluetooth, keywords: "bluetooth auriculares raton teclado emparejar" },
  { id: "display", title: "Pantalla o monitor", hint: "Monitor sin imagen, parpadeos, resolución", icon: Monitor, keywords: "pantalla monitor grafica video hdmi resolucion negra" },
  { id: "printer", title: "No imprime", hint: "Cola atascada, sin conexión, predeterminada", icon: Printer, keywords: "impresora imprimir cola spooler" },
  { id: "slow", title: "Va lento", hint: "Tarda en abrir, se congela, arranca lento", icon: Gauge, keywords: "lento lentitud rendimiento memoria cpu disco congelado" },
  { id: "winupdate", title: "Windows Update falla", hint: "No actualiza, errores al instalar", icon: RefreshCcwDot, keywords: "actualizaciones windows update error parches" },
];

const LEVEL = {
  ok: { icon: CheckCircle2, color: "text-ok", border: "border-ok/30" },
  info: { icon: Info, color: "text-neon", border: "border-line" },
  warn: { icon: AlertTriangle, color: "text-warn", border: "border-warn/40" },
  bad: { icon: XCircle, color: "text-bad", border: "border-bad/40" },
};

const isPage = (p: string | null): p is PageId => !!p && NAV.some((n) => n.id === p);

/** «Algo no funciona»: eliges el síntoma y AdminOps comprueba y ofrece las reparaciones. */
export function Troubleshoot({ isAdmin, focus, onNavigate }: { isAdmin: boolean; focus: string | null; onNavigate: (p: PageId) => void }) {
  const [symptom, setSymptom] = useState<Symptom | null>(null);
  const [findings, setFindings] = useState<TroubleFinding[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [fixing, setFixing] = useState<string | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const seq = useRef(0);

  const check = useCallback(async (s: Symptom) => {
    const n = ++seq.current;
    setSymptom(s);
    setChecking(true);
    setError(null);
    setFindings(null);
    try {
      const r = await troubleshootApi.check(s);
      if (n === seq.current) setFindings(r.findings);
    } catch (e) {
      if (n === seq.current) setError(String(e));
    } finally {
      if (n === seq.current) setChecking(false);
    }
  }, []);

  // Desde la búsqueda o un aviso: abre el síntoma y lo comprueba.
  useEffect(() => {
    const s = SYMPTOMS.find((x) => x.id === focus);
    if (s) check(s.id);
  }, [focus, check]);

  const runFix = async (f: TroubleFix) => {
    if (f.confirm && !(await confirm({ title: f.label, body: f.confirm, confirmLabel: "Continuar" }))) return;
    setFixing(f.id);
    try {
      const msg = await troubleshootApi.fix(f.id);
      if (msg) toast("ok", msg);
    } catch (e) {
      toast("error", `${f.label}: ${e}`);
    } finally {
      setFixing(null);
    }
    // Abrir Configuración no cambia nada que volver a comprobar.
    if (symptom && !f.id.startsWith("open:")) check(symptom);
  };

  const current = SYMPTOMS.find((s) => s.id === symptom);

  return (
    <div className="mx-auto grid max-w-5xl grid-cols-12 gap-4 p-6">
      <Card title="¿Qué le pasa al equipo?" icon={<Wrench size={14} />} className="col-span-12">
        <p className="mb-3 text-xs text-dim">Elige el síntoma: AdminOps revisa en orden las causas típicas y te ofrece la reparación de cada una.</p>
        <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
          {SYMPTOMS.map((s) => (
            <button
              key={s.id}
              onClick={() => check(s.id)}
              disabled={checking && symptom === s.id}
              className={`flex items-start gap-2.5 rounded-lg border px-3 py-2.5 text-left transition-colors ${
                symptom === s.id ? "border-neon/60 bg-neon/10" : "border-line hover:border-line-2 hover:bg-panel-2"
              }`}
            >
              <s.icon size={16} className={`mt-0.5 shrink-0 ${symptom === s.id ? "text-neon" : "text-dim"}`} />
              <span className="min-w-0">
                <span className="block text-sm text-ink">{s.title}</span>
                <span className="block text-[11px] leading-snug text-mute">{s.hint}</span>
              </span>
            </button>
          ))}
        </div>
      </Card>

      {current && (
        <Card
          title={current.title}
          icon={<current.icon size={14} />}
          className="col-span-12"
          right={
            <button onClick={() => check(current.id)} disabled={checking || !!fixing} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink disabled:opacity-40">
              <RefreshCw size={11} className={checking ? "animate-spin" : ""} /> Volver a comprobar
            </button>
          }
        >
          {checking ? (
            <p className="flex items-center gap-2 py-4 text-sm text-mute">
              <Loader2 size={14} className="animate-spin" /> Comprobando…
            </p>
          ) : error ? (
            <p className="text-sm text-bad">{error}</p>
          ) : (
            <ul className="space-y-2">
              {findings?.map((f, i) => {
                const L = LEVEL[f.level];
                return (
                  <li key={i} className={`rounded-lg border ${L.border} bg-void/30 px-3.5 py-3`}>
                    <div className="flex items-start gap-2.5">
                      <L.icon size={16} className={`mt-0.5 shrink-0 ${L.color}`} />
                      <div className="min-w-0 flex-1">
                        <p className="text-sm font-medium text-ink">{f.title}</p>
                        {f.detail && <p className="mt-0.5 text-xs leading-relaxed break-words text-dim select-text">{f.detail}</p>}
                        {(f.fixes.length > 0 || isPage(f.page)) && (
                          <div className="mt-2.5 flex flex-wrap items-center gap-2">
                            {f.fixes.map((x) => {
                              const blocked = x.admin && !isAdmin;
                              return (
                                <button
                                  key={x.id}
                                  onClick={() => runFix(x)}
                                  disabled={!!fixing || blocked}
                                  title={blocked ? "Requiere ejecutar AdminOps como administrador" : undefined}
                                  className={`flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-xs transition-colors disabled:opacity-40 ${
                                    f.level === "bad" && !x.id.startsWith("open:") ? "border-neon/50 text-neon hover:bg-neon/10" : "border-line-2 text-dim hover:border-neon/40 hover:text-ink"
                                  }`}
                                >
                                  {fixing === x.id && <Loader2 size={11} className="animate-spin" />}
                                  {x.label}
                                </button>
                              );
                            })}
                            {isPage(f.page) && (
                              <button onClick={() => onNavigate(f.page as PageId)} className="flex items-center gap-1 text-xs text-mute hover:text-neon">
                                Ver en {pageLabel(f.page as PageId)} <ArrowRight size={11} />
                              </button>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </Card>
      )}
      {dialog}
    </div>
  );
}
