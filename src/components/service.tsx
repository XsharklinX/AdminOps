import { fillTemplate, recipients, reportNumber } from "../lib/reportText";
import { quoteTemplates, saveQuoteTemplate, useQuoteQueue } from "../lib/quote";
import { goToPage } from "../lib/navigate";
import { Eraser, FileText, Mail, Plus, Trash2, Wrench, LayoutList } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { logQuietly, diagApi, portalsApi, workApi, type Billing, type ClientReport, type DocKind, type Line, type Settings, type Template } from "../lib/api";
import { money } from "../lib/format";
import { useToast } from "./feedback";
import { Button, inputClass, Modal } from "./ui";
import { useLiveEffect } from "../lib/useLiveEffect";

/** Importes igual que en el backend (se redondea a céntimos en cada paso). */
export function totals(b: Billing, taxRate: number) {
  const round = (v: number) => Math.round(v * 100) / 100;
  const subtotal = round(b.lines.filter((l) => l.description.trim()).reduce((a, l) => a + l.qty * l.price, 0));
  const discount = round(Math.min(Math.max(b.discount, 0), subtotal));
  const tax = round(((subtotal - discount) * taxRate) / 100);
  return { subtotal, discount, tax, total: round(subtotal - discount + tax) };
}

export const billingActive = (b: Billing) => b.kind !== "none" && b.lines.some((l) => l.description.trim());

function Segmented<T extends string>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className="inline-flex rounded-lg border border-line bg-void p-0.5">
      {options.map(([v, label]) => (
        <button
          key={v}
          type="button"
          onClick={() => onChange(v)}
          aria-pressed={value === v}
          className={`rounded-md px-3 py-1.5 text-[13px] transition-colors ${value === v ? "bg-panel-2 font-medium text-ink" : "text-dim hover:text-ink"}`}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

/** Plantilla del informe: resumen para el cliente o detalle técnico. */
export function TemplatePicker({ value, onChange, custom }: { value: Template; onChange: (t: Template) => void; /** Nombre de la plantilla propia (Ajustes → Informes). */ custom?: string }) {
  const option = (t: Template, icon: ReactNode, title: string, sub: string) => (
    <button
      type="button"
      onClick={() => onChange(t)}
      aria-pressed={value === t}
      className={`flex flex-1 items-start gap-3 rounded-lg border p-3 text-left transition-colors ${value === t ? "border-neon bg-neon/5" : "border-line hover:border-line-2"}`}
    >
      <span className={`mt-0.5 ${value === t ? "text-neon" : "text-mute"}`}>{icon}</span>
      <span>
        <span className="block text-sm font-medium text-ink">{title}</span>
        <span className="block text-xs text-mute">{sub}</span>
      </span>
    </button>
  );
  return (
    <div className="flex flex-wrap gap-3">
      {option("client", <FileText size={16} />, "Para el cliente", "Estado del equipo en lenguaje claro, trabajo hecho, pendientes y firmas.")}
      {option("technical", <Wrench size={16} />, "Técnico", "Lo mismo más hardware, discos, SMART, estabilidad, drivers y seguridad.")}
      {option("custom", <LayoutList size={16} />, custom?.trim() || "Mi plantilla", "Las secciones que elegiste, en tu orden (Ajustes → Informes y cobros).")}
    </div>
  );
}

const num = (v: string) => {
  const n = parseFloat(v.replace(",", "."));
  return Number.isFinite(n) && n >= 0 ? n : 0;
};

const cell = "w-full rounded-md border border-line bg-void/60 px-2 py-1.5 text-[13px] text-ink outline-none focus:border-neon/50 disabled:opacity-40";

/** Presupuesto o recibo: líneas de servicio y piezas, descuento, impuesto y total. */
export function BillingEditor({ billing, onChange, settings }: { billing: Billing; onChange: (b: Billing) => void; settings: Settings | null }) {
  const cur = settings?.currency ?? "";
  const taxRate = settings?.taxRate ?? 0;
  const setLine = (i: number, patch: Partial<Line>) => onChange({ ...billing, lines: billing.lines.map((l, j) => (j === i ? { ...l, ...patch } : l)) });
  const add = (l?: Partial<Line>) =>
    onChange({ ...billing, lines: [...billing.lines, { description: "", part: false, qty: 1, price: 0, warrantyDays: 0, ...l }] });
  const t = totals(billing, taxRate);
  const catalog = settings?.catalog.filter((c) => c.name.trim()) ?? [];
  // Lo que llega desde «¿Reparar o cambiar?» o los repuestos compatibles, sin volver a escribirlo.
  useQuoteQueue((lines) => onChange({ ...billing, kind: billing.kind === "none" ? "quote" : billing.kind, lines: [...billing.lines.filter((l) => l.description.trim() || l.price), ...lines] }));
  const [templates, setTemplates] = useState(quoteTemplates);

  return (
    <div>
      <Segmented<DocKind>
        value={billing.kind}
        onChange={(kind) => onChange({ ...billing, kind, lines: kind !== "none" && billing.lines.length === 0 ? [{ description: "", part: false, qty: 1, price: 0, warrantyDays: 0 }] : billing.lines })}
        options={[
          ["none", "Sin importes"],
          ["quote", "Presupuesto"],
          ["receipt", "Recibo"],
        ]}
      />
      {billing.kind === "none" ? (
        <p className="mt-3 text-sm text-mute">El informe no incluirá precios. Elige Presupuesto (antes de trabajar) o Recibo (trabajo hecho y cobrado).</p>
      ) : (
        <>
          <table className="mt-4 w-full text-[13px]">
            <thead>
              <tr className="text-left text-xs text-mute">
                <th className="pb-1.5 font-medium">Descripción</th>
                <th className="w-14 pb-1.5 text-center font-medium" title="Las piezas pueden llevar su propia garantía">
                  Pieza
                </th>
                <th className="w-24 pb-1.5 font-medium">Garantía (días)</th>
                <th className="w-16 pb-1.5 font-medium">Cant.</th>
                <th className="w-28 pb-1.5 font-medium">Precio</th>
                <th className="w-28 pb-1.5 text-right font-medium">Importe</th>
                <th className="w-8" />
              </tr>
            </thead>
            <tbody>
              {billing.lines.map((l, i) => (
                <tr key={i}>
                  <td className="py-1 pr-2">
                    <input value={l.description} onChange={(e) => setLine(i, { description: e.target.value })} placeholder="Servicio o pieza" className={cell} />
                  </td>
                  <td className="py-1 text-center">
                    <input type="checkbox" checked={l.part} onChange={(e) => setLine(i, { part: e.target.checked })} className="size-4 accent-[var(--color-neon)]" />
                  </td>
                  <td className="py-1 pr-2">
                    <input
                      type="number"
                      min={0}
                      disabled={!l.part}
                      value={l.part ? l.warrantyDays : ""}
                      onChange={(e) => setLine(i, { warrantyDays: Math.round(num(e.target.value)) })}
                      className={cell}
                    />
                  </td>
                  <td className="py-1 pr-2">
                    <input type="number" min={0} step="any" value={l.qty} onChange={(e) => setLine(i, { qty: num(e.target.value) })} className={cell} />
                  </td>
                  <td className="py-1 pr-2">
                    <input type="number" min={0} step="any" value={l.price} onChange={(e) => setLine(i, { price: num(e.target.value) })} className={cell} />
                  </td>
                  <td className="py-1 text-right font-mono text-xs text-ink">{money(l.qty * l.price, cur)}</td>
                  <td className="py-1 text-right">
                    <button onClick={() => onChange({ ...billing, lines: billing.lines.filter((_, j) => j !== i) })} className="text-mute hover:text-bad" title="Quitar línea">
                      <Trash2 size={13} />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <button type="button" onClick={() => add()} className="flex items-center gap-1 rounded-md border border-line-2 px-2.5 py-1.5 text-xs text-dim hover:text-ink">
              <Plus size={12} /> Línea
            </button>
            {catalog.length > 0 && (
              <select
                value=""
                onChange={(e) => {
                  const c = catalog[Number(e.target.value)];
                  if (c) add({ description: c.name, price: c.price, part: c.part, warrantyDays: c.warrantyDays });
                }}
                className="rounded-md border border-line-2 bg-void px-2 py-1.5 text-xs text-dim outline-none"
              >
                <option value="">Añadir del catálogo…</option>
                {catalog.map((c, i) => (
                  <option key={i} value={i}>
                    {c.name}
                    {c.price ? ` · ${money(c.price, cur)}` : ""}
                  </option>
                ))}
              </select>
            )}
            {templates.length > 0 && (
              <select
                value=""
                onChange={(e) => {
                  const tpl = templates.find((x) => x.name === e.target.value);
                  if (tpl) onChange({ ...billing, lines: [...billing.lines.filter((l) => l.description.trim() || l.price), ...tpl.lines] });
                }}
                className="rounded-md border border-line-2 bg-void px-2 py-1.5 text-xs text-dim outline-none"
                aria-label="Usar una plantilla de presupuesto"
              >
                <option value="">Usar una plantilla…</option>
                {templates.map((x) => (
                  <option key={x.name} value={x.name}>
                    {x.name} · {x.lines.length} líneas
                  </option>
                ))}
              </select>
            )}
            {billing.lines.some((l) => l.description.trim()) && (
              <button
                type="button"
                onClick={() => {
                  const name = window.prompt("Nombre de la plantilla (p. ej. «Cambio a SSD con clonado»)")?.trim();
                  if (name) setTemplates(saveQuoteTemplate(name, billing.lines.filter((l) => l.description.trim())));
                }}
                className="flex items-center gap-1 rounded-md border border-line-2 px-2.5 py-1.5 text-xs text-dim hover:text-ink"
                title="Guarda estas líneas para usarlas en otros presupuestos"
              >
                Guardar como plantilla
              </button>
            )}
          </div>

          <div className="mt-4 flex flex-wrap items-end justify-between gap-4">
            <div className="flex gap-3">
              <label className="block w-32">
                <span className="mb-1 block text-xs text-dim">Descuento</span>
                <input type="number" min={0} step="any" value={billing.discount} onChange={(e) => onChange({ ...billing, discount: num(e.target.value) })} className={cell} />
              </label>
              {billing.kind === "receipt" && (
                <label className="block w-44">
                  <span className="mb-1 block text-xs text-dim">Forma de pago</span>
                  <input list="adminops-payments" value={billing.payment} onChange={(e) => onChange({ ...billing, payment: e.target.value })} placeholder="Efectivo…" className={cell} />
                  <datalist id="adminops-payments">
                    <option value="Efectivo" />
                    <option value="Transferencia" />
                    <option value="Tarjeta" />
                    <option value="Pendiente de pago" />
                  </datalist>
                </label>
              )}
            </div>
            <dl className="min-w-56 space-y-0.5 text-[13px]">
              <div className="flex justify-between gap-6 text-dim">
                <dt>Subtotal</dt>
                <dd className="font-mono text-xs">{money(t.subtotal, cur)}</dd>
              </div>
              {t.discount > 0 && (
                <div className="flex justify-between gap-6 text-dim">
                  <dt>Descuento</dt>
                  <dd className="font-mono text-xs">-{money(t.discount, cur)}</dd>
                </div>
              )}
              {taxRate > 0 && (
                <div className="flex justify-between gap-6 text-dim">
                  <dt>
                    {settings?.taxName || "Impuesto"} ({taxRate}%)
                  </dt>
                  <dd className="font-mono text-xs">{money(t.tax, cur)}</dd>
                </div>
              )}
              <div className="flex justify-between gap-6 border-t border-line pt-1 font-semibold text-ink">
                <dt>Total</dt>
                <dd className="font-mono">{money(t.total, cur)}</dd>
              </div>
            </dl>
          </div>
          <p className="mt-2 text-[11px] text-mute">Moneda, impuesto y catálogo se configuran en Ajustes.</p>
        </>
      )}
    </div>
  );
}

/** Firma a mano alzada (ratón, lápiz o dedo). Devuelve un PNG en data URL. */
export function SignaturePad({ value, onChange, height = 150 }: { value: string | null; onChange: (v: string | null) => void; height?: number }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const drawing = useRef(false);
  const last = useRef<{ x: number; y: number } | null>(null);

  // Tamaño real según la pantalla y, si ya había firma, se vuelve a pintar.
  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = c.clientWidth * dpr;
    c.height = height * dpr;
    const ctx = c.getContext("2d")!;
    ctx.scale(dpr, dpr);
    if (value) {
      const img = new Image();
      img.onload = () => ctx.drawImage(img, 0, 0, c.clientWidth, height);
      img.src = value;
    }
    // Solo al montar: después, el lienzo es la fuente de verdad.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `value` y `height` solo se leen al preparar el lienzo
  }, []);

  const point = (e: React.PointerEvent) => {
    const r = canvas.current!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const down = (e: React.PointerEvent) => {
    canvas.current!.setPointerCapture(e.pointerId);
    drawing.current = true;
    last.current = point(e);
  };
  const move = (e: React.PointerEvent) => {
    if (!drawing.current || !last.current) return;
    const ctx = canvas.current!.getContext("2d")!;
    const p = point(e);
    ctx.strokeStyle = "#1b2330";
    ctx.lineWidth = e.pointerType === "pen" ? 1.5 + e.pressure * 2 : 2.2;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.beginPath();
    ctx.moveTo(last.current.x, last.current.y);
    ctx.lineTo(p.x, p.y);
    ctx.stroke();
    last.current = p;
  };
  const up = () => {
    if (!drawing.current) return;
    drawing.current = false;
    last.current = null;
    onChange(canvas.current!.toDataURL("image/png"));
  };
  const clear = () => {
    const c = canvas.current!;
    c.getContext("2d")!.clearRect(0, 0, c.width, c.height);
    onChange(null);
  };

  return (
    <div>
      <div className="relative overflow-hidden rounded-lg border border-line-2 bg-white">
        <canvas
          ref={canvas}
          style={{ height, touchAction: "none" }}
          className="block w-full cursor-crosshair"
          onPointerDown={down}
          onPointerMove={move}
          onPointerUp={up}
          onPointerCancel={up}
          aria-label="Zona de firma"
        />
        <div className="pointer-events-none absolute right-4 bottom-7 left-4 border-b border-dashed border-[#c5ccd6]" />
        {!value && <span className="pointer-events-none absolute bottom-2 left-4 text-[11px] text-[#8a95a5]">Firme aquí</span>}
      </div>
      <button type="button" onClick={clear} disabled={!value} className="mt-1.5 flex items-center gap-1 text-xs text-mute hover:text-ink disabled:opacity-40">
        <Eraser size={12} /> Borrar firma
      </button>
    </div>
  );
}

/** Correo al cliente con el informe adjunto. */
export function SendReportModal({
  path,
  client,
  onClose,
}: {
  path: string;
  client: { name: string; contact?: string; email?: string; report?: ClientReport } | null;
  onClose: () => void;
}) {
  const toast = useToast();
  const [to, setTo] = useState(recipients(client?.email, client?.report?.to));
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [hasMail, setHasMail] = useState(false);

  useLiveEffect((vigente) => {
    portalsApi
      .list()
      .then((l) => vigente() && setHasMail(l.some((p) => p.kind === "mail")))
      .catch(logQuietly("service"));
    workApi.settings().then((s) => {
      if (!vigente()) return;
      const brand = s.company.trim() || s.technician.trim();
      const number = reportNumber(path);
      const who = client?.contact?.trim() || client?.name?.trim();
      const sign = [s.technician, s.company, s.phone].map((x) => x.trim()).filter(Boolean).join("\n");
      const values = {
        cliente: client?.name,
        contacto: client?.contact?.trim() || client?.name,
        numero: number,
        fecha: new Date().toLocaleDateString("es", { dateStyle: "long" }),
        empresa: s.company,
        tecnico: s.technician,
      };
      // La plantilla del cliente, si tiene; si no, la de siempre.
      const t = client?.report;
      setSubject(t?.subject ? fillTemplate(t.subject, values) : `Informe de servicio técnico${number ? ` Nº ${number}` : ""}${brand ? ` · ${brand}` : ""}`);
      setBody(
        t?.body
          ? fillTemplate(t.body, values)
          : `${who ? `Hola, ${who}:` : "Hola:"}\n\nTe envío adjunto el informe del servicio técnico realizado el ${new Date().toLocaleDateString("es", { dateStyle: "long" })}. ` +
              `En él encontrarás el estado del equipo, el trabajo realizado, lo que queda pendiente y nuestras recomendaciones.\n\n` +
              `Quedo a tu disposición para cualquier consulta.\n\nUn saludo,${sign ? `\n${sign}` : ""}`,
      );
    }).catch(logQuietly("service"));
    // Solo al abrir: después el texto es del usuario.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- los datos del cliente solo se leen al preparar el mensaje
  }, [path]);

  // Con el Correo de AdminOps: el mensaje queda escrito y se abre la carpeta del
  // PDF para arrastrarlo (el correo web no deja adjuntar un archivo automáticamente).
  const sendInApp = async () => {
    setBusy(true);
    try {
      const ok = await portalsApi.compose(to.replace(/\s+/g, ""), subject, body);
      if (!ok) throw new Error("Configura antes el Correo (su icono, en la barra de arriba).");
      await diagApi.revealReport(path).catch(() => {});
      toast("ok", "Mensaje listo en el Correo: arrastra el PDF desde la carpeta que se abrió y envíalo.");
      onClose();
      goToPage("mail");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const send = async (manual: boolean) => {
    setBusy(true);
    try {
      await (manual ? diagApi.emailReportManual : diagApi.emailReport)(path, to.trim(), subject, body);
      toast("ok", manual ? "Correo abierto: arrastra el PDF desde la carpeta que se abrió." : "Correo preparado con el informe adjunto: revísalo y envíalo.");
      onClose();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="Enviar el informe por correo"
      onClose={onClose}
      width="w-[560px]"
      footer={
        <>
          <Button kind="ghost" onClick={() => send(true)} disabled={busy} title="Para Gmail u Outlook en el navegador: abre el correo y la carpeta del PDF">
            Sin adjunto (correo web)
          </Button>
          {hasMail && (
            <Button kind="ghost" onClick={sendInApp} disabled={busy} title="Escribe el mensaje en el Correo de AdminOps y abre la carpeta del PDF para arrastrarlo">
              <Mail size={14} /> Con el Correo de AdminOps
            </Button>
          )}
          <Button onClick={() => send(false)} disabled={busy}>
            <Mail size={14} /> Abrir en el correo
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Para</span>
          <input value={to} onChange={(e) => setTo(e.target.value)} placeholder="correo@cliente.com" className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Asunto</span>
          <input value={subject} onChange={(e) => setSubject(e.target.value)} className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Mensaje</span>
          <textarea value={body} onChange={(e) => setBody(e.target.value)} rows={9} className={`${inputClass} resize-y`} />
        </label>
        <p className="text-xs text-mute">
          Se abre tu programa de correo (Outlook, Thunderbird, Correo…) con el PDF ya adjunto, listo para revisar y enviar. Nada se envía sin que lo
          confirmes.
        </p>
      </div>
    </Modal>
  );
}
