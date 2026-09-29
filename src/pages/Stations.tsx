import { Ban, CheckCircle2, Download, Loader2, MessageSquare, Monitor, MonitorPlay, Play, RefreshCcw, RotateCw, Save, ScanSearch, Trash2, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, inputClass, Modal } from "../components/ui";
import { libraryApi, officeApi, remoteApi, stationsApi, type Station, type StationAction, type StationActionResult, type StationList } from "../lib/api";

const ACTION_LABEL: Record<StationAction, string> = {
  restart: "Reiniciar",
  cancelRestart: "Cancelar reinicio",
  gpupdate: "Actualizar directivas",
  message: "Enviar mensaje",
};

/** Más ventanas de Escritorio remoto que esto a la vez es inmanejable. */
const MAX_RDP = 6;

const PORTS: Record<number, string> = { 3389: "Escritorio remoto", 445: "Carpetas", 5985: "WinRM" };

const parseHosts = (text: string) => [...new Set(text.split(/[\s,;]+/).map((h) => h.trim()).filter(Boolean))];
const days = (d: number | null | undefined) => (d == null ? "—" : `${Math.floor(d)} d`);

/** Comprobar puestos: qué equipos de una lista responden y cuáles necesitan atención. */
export function Stations() {
  const [lists, setLists] = useState<StationList[]>([]);
  const [current, setCurrent] = useState<StationList | null>(null);
  const [text, setText] = useState("");
  const [name, setName] = useState("");
  const [results, setResults] = useState<Station[] | null>(null);
  const [busy, setBusy] = useState<"quick" | "deep" | null>(null);
  const [onlyIssues, setOnlyIssues] = useState(false);
  // Puestos marcados para actuar sobre ellos a la vez.
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [acting, setActing] = useState<StationAction | null>(null);
  const [outcome, setOutcome] = useState<{ action: StationAction; results: StationActionResult[] } | null>(null);
  const [asking, setAsking] = useState<StationAction | null>(null);
  const [msg, setMsg] = useState("");
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => libraryApi.list("stations").then(setLists).catch(() => {}), []);
  useEffect(() => {
    load();
  }, [load]);

  const hosts = useMemo(() => parseHosts(text), [text]);

  const open = (l: StationList) => {
    setCurrent(l);
    setName(l.name);
    setText(l.hosts.join("\n"));
    setResults(null);
  };

  const save = async () => {
    if (!name.trim() || !hosts.length) return toast("error", "Pon un nombre y al menos un equipo.");
    try {
      const saved = await libraryApi.save("stations", { id: current?.id ?? "", name: name.trim(), hosts });
      setCurrent(saved);
      toast("ok", "Lista guardada.");
      load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  const remove = async (l: StationList) => {
    if (!(await confirm({ title: "Borrar lista", body: `Se borrará la lista «${l.name}».`, confirmLabel: "Borrar", danger: true }))) return;
    await libraryApi.remove("stations", l.id).catch(() => {});
    if (current?.id === l.id) setCurrent(null);
    load();
  };

  const check = async (deep: boolean) => {
    if (!hosts.length) return toast("error", "Escribe los equipos: nombres o IP, uno por línea.");
    setBusy(deep ? "deep" : "quick");
    try {
      setResults(await stationsApi.check(hosts, deep));
      setPicked(new Set());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const shown = (results ?? []).filter((r) => !onlyIssues || !r.online || r.warnings.length > 0);
  const online = (results ?? []).filter((r) => r.online).length;
  const issues = (results ?? []).filter((r) => r.warnings.length > 0).length;

  const exportCsv = () => {
    const rows = [
      ["Equipo", "IP", "Responde", "ms", "Puertos", "Windows", "Usuario", "Libre GB", "Días sin reiniciar", "Días sin actualizar", "Avisos"],
      ...(results ?? []).map((r) => [
        r.host,
        r.ip,
        r.online ? "Sí" : "No",
        r.ms?.toString() ?? "",
        r.ports.map((p) => PORTS[p] ?? p).join(", "),
        r.remote?.os ?? "",
        r.remote?.user ?? "",
        r.remote?.freeGb != null ? r.remote.freeGb.toFixed(1) : "",
        r.remote?.bootDays != null ? Math.floor(r.remote.bootDays).toString() : "",
        r.remote?.updateDays != null ? Math.floor(r.remote.updateDays).toString() : "",
        [...r.warnings, r.remoteError].filter(Boolean).join(" · "),
      ]),
    ];
    const csv = rows.map((r) => r.map((c) => (/[;"\n]/.test(c) ? `"${c.replace(/"/g, '""')}"` : c)).join(";")).join("\r\n");
    officeApi.exportCsv(`Puestos ${name || ""}`.trim(), csv).catch((e) => toast("error", String(e)));
  };

  const rdp = (host: string) =>
    remoteApi.connectRdp(host, "", { fullscreen: false, multimon: false, clipboard: true, drives: false, printers: false, audio: true }).catch((e) => toast("error", String(e)));

  const togglePick = (host: string) =>
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(host)) n.delete(host);
      else n.add(host);
      return n;
    });
  const allPicked = shown.length > 0 && shown.every((r) => picked.has(r.host));
  const pickAll = () => setPicked(allPicked ? new Set() : new Set(shown.map((r) => r.host)));
  const pickedList = [...picked];

  const act = async (action: StationAction, text?: string) => {
    setAsking(null);
    setActing(action);
    try {
      const results = await stationsApi.act(pickedList, action, text);
      setOutcome({ action, results });
      const failed = results.filter((r) => !r.ok).length;
      toast(failed ? "info" : "ok", `${ACTION_LABEL[action]}: ${results.length - failed} de ${results.length} correctos.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setActing(null);
    }
  };

  const askRestart = async () => {
    const go = await confirm({
      title: `¿Reiniciar ${pickedList.length} ${pickedList.length === 1 ? "equipo" : "equipos"}?`,
      body: (
        <p>
          Quien esté usando {pickedList.length === 1 ? "el equipo" : "los equipos"} verá un aviso y tendrá 2 minutos para guardar su trabajo. Mientras tanto se puede
          cancelar con «Cancelar reinicio».
        </p>
      ),
      confirmLabel: "Reiniciar",
      danger: true,
    });
    if (go) act("restart");
  };

  const rdpPicked = () => {
    const withRdp = (results ?? []).filter((r) => picked.has(r.host) && r.ports.includes(3389));
    if (!withRdp.length) return toast("info", "Ninguno de los elegidos tiene el Escritorio remoto abierto.");
    withRdp.slice(0, MAX_RDP).forEach((r) => rdp(r.host));
    if (withRdp.length > MAX_RDP) toast("info", `Se abrieron ${MAX_RDP}; elige menos equipos para abrir el resto.`);
  };

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <Card title="Listas de equipos" icon={<Monitor size={14} />} className="col-span-12 lg:col-span-3 lg:self-start">
        <button
          onClick={() => {
            setCurrent(null);
            setName("");
            setText("");
            setResults(null);
          }}
          className="mb-2 w-full rounded-md border border-dashed border-line py-1.5 text-xs text-neon hover:border-neon/50"
        >
          + Lista nueva
        </button>
        {lists.length === 0 && <p className="text-xs text-mute">Guarda aquí los equipos de cada oficina para comprobarlos de un vistazo.</p>}
        {lists.map((l) => (
          <div key={l.id} className={`group flex items-center gap-2 rounded-md px-2 py-1.5 ${current?.id === l.id ? "bg-neon/10" : "hover:bg-panel-2"}`}>
            <button onClick={() => open(l)} className="min-w-0 flex-1 text-left">
              <span className="block truncate text-sm text-ink">{l.name}</span>
              <span className="text-[11px] text-mute">{l.hosts.length} equipo(s)</span>
            </button>
            <button onClick={() => remove(l)} className="p-1 text-mute opacity-0 group-hover:opacity-100 hover:text-bad" title="Borrar">
              <Trash2 size={12} />
            </button>
          </div>
        ))}
      </Card>

      <div className="col-span-12 space-y-4 lg:col-span-9">
        <Card title="Equipos a comprobar" icon={<ScanSearch size={14} />}>
          <div className="grid gap-3 md:grid-cols-3">
            <div className="md:col-span-2">
              <textarea
                value={text}
                onChange={(e) => setText(e.target.value)}
                rows={6}
                placeholder={"PC-CONTA-01\nPC-RECEPCION\n192.168.1.20"}
                className={`${inputClass} font-mono text-xs`}
              />
              <p className="mt-1 text-[11px] text-mute">{hosts.length} equipo(s). Nombres o IP, uno por línea (también separados por comas).</p>
            </div>
            <div className="space-y-2">
              <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Nombre de la lista (Oficina central)" className={inputClass} />
              <Button kind="ghost" onClick={save}>
                <Save size={13} /> Guardar lista
              </Button>
              <div className="flex flex-col gap-2 border-t border-line pt-2">
                <Button onClick={() => check(false)} disabled={!!busy}>
                  {busy === "quick" ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} />} Comprobar (rápido)
                </Button>
                <Button kind="ghost" onClick={() => check(true)} disabled={!!busy}>
                  {busy === "deep" ? <Loader2 size={13} className="animate-spin" /> : <ScanSearch size={13} />} A fondo (disco, reinicios, actualizaciones)
                </Button>
              </div>
            </div>
          </div>
          <p className="mt-2 text-[11px] text-mute">
            «A fondo» lee cada equipo por administración remota (WinRM o DCOM): funciona en un dominio con un usuario administrador; si no, se indica por qué.
          </p>
        </Card>

        {results && (
          <Card
            title={`Resultado · ${online} de ${results.length} responden${issues ? ` · ${issues} con avisos` : ""}`}
            icon={<CheckCircle2 size={14} />}
            right={
              <div className="flex items-center gap-3 text-[11px]">
                <label className="flex items-center gap-1.5 text-dim">
                  <input type="checkbox" checked={onlyIssues} onChange={(e) => setOnlyIssues(e.target.checked)} className="accent-[var(--color-neon)]" /> Solo con problemas
                </label>
                <button onClick={exportCsv} className="flex items-center gap-1 text-mute hover:text-ink">
                  <Download size={11} /> CSV
                </button>
              </div>
            }
          >
            {picked.size > 0 && (
              <div className="mb-3 flex flex-wrap items-center gap-2 rounded-lg border border-neon/30 bg-neon/5 px-3 py-2 text-xs">
                <span className="mr-1 text-ink">
                  {picked.size} {picked.size === 1 ? "elegido" : "elegidos"}
                </span>
                <Button kind="ghost" onClick={askRestart} disabled={!!acting}>
                  {acting === "restart" ? <Loader2 size={12} className="animate-spin" /> : <RotateCw size={12} />} Reiniciar
                </Button>
                <Button kind="ghost" onClick={() => act("cancelRestart")} disabled={!!acting}>
                  {acting === "cancelRestart" ? <Loader2 size={12} className="animate-spin" /> : <Ban size={12} />} Cancelar reinicio
                </Button>
                <Button kind="ghost" onClick={() => act("gpupdate")} disabled={!!acting}>
                  {acting === "gpupdate" ? <Loader2 size={12} className="animate-spin" /> : <RefreshCcw size={12} />} Actualizar directivas
                </Button>
                <Button kind="ghost" onClick={() => setAsking("message")} disabled={!!acting}>
                  {acting === "message" ? <Loader2 size={12} className="animate-spin" /> : <MessageSquare size={12} />} Mensaje
                </Button>
                <Button kind="ghost" onClick={rdpPicked} disabled={!!acting}>
                  <MonitorPlay size={12} /> Escritorio remoto
                </Button>
                <button onClick={() => setPicked(new Set())} className="ml-auto text-mute hover:text-ink">
                  Quitar selección
                </button>
              </div>
            )}
            {outcome && (
              <div className="mb-3 rounded-lg border border-line bg-void/40 px-3 py-2 text-xs">
                <div className="mb-1 flex items-center">
                  <span className="text-dim">{ACTION_LABEL[outcome.action]}</span>
                  <button onClick={() => setOutcome(null)} className="ml-auto text-mute hover:text-ink">
                    Cerrar
                  </button>
                </div>
                <ul className="space-y-0.5">
                  {outcome.results.map((r) => (
                    <li key={r.host} className="flex gap-2">
                      {r.ok ? <CheckCircle2 size={12} className="mt-0.5 shrink-0 text-ok" /> : <XCircle size={12} className="mt-0.5 shrink-0 text-bad" />}
                      <span className="font-medium text-ink">{r.host}</span>
                      <span className="text-dim">{r.detail}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="text-left text-[11px] text-mute">
                    <th className="w-6 pb-2">
                      <input type="checkbox" checked={allPicked} onChange={pickAll} title="Elegir todos" className="accent-[var(--color-neon)]" />
                    </th>
                    <th className="pb-2 font-medium">Equipo</th>
                    <th className="pb-2 font-medium">Estado</th>
                    <th className="pb-2 font-medium">Windows · usuario</th>
                    <th className="pb-2 text-right font-medium">Libre</th>
                    <th className="pb-2 text-right font-medium">Sin reiniciar</th>
                    <th className="pb-2 text-right font-medium">Sin actualizar</th>
                    <th className="pb-2 font-medium">Avisos</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {shown.map((r) => (
                    <tr key={r.host} className={`border-t border-line/60 align-top ${picked.has(r.host) ? "bg-neon/5" : ""}`}>
                      <td className="py-2.5">
                        <input type="checkbox" checked={picked.has(r.host)} onChange={() => togglePick(r.host)} className="accent-[var(--color-neon)]" />
                      </td>
                      <td className="py-2">
                        <span className="block text-sm text-ink">{r.host}</span>
                        <span className="font-mono text-[11px] text-mute">{r.ip}</span>
                      </td>
                      <td className="py-2">
                        {r.online ? (
                          <span className="flex items-center gap-1 text-ok">
                            <CheckCircle2 size={12} /> {r.ms != null ? `${r.ms} ms` : "Responde"}
                          </span>
                        ) : (
                          <span className="flex items-center gap-1 text-bad">
                            <XCircle size={12} /> No responde
                          </span>
                        )}
                        <span className="mt-0.5 flex flex-wrap gap-1">
                          {r.ports.map((p) => (
                            <span key={p} className="rounded border border-line px-1 text-[10px] text-mute">
                              {PORTS[p] ?? p}
                            </span>
                          ))}
                        </span>
                      </td>
                      <td className="py-2 text-dim">
                        {r.remote ? (
                          <>
                            <span className="block">{r.remote.os}</span>
                            <span className="text-mute">{r.remote.user || "Nadie conectado"}</span>
                          </>
                        ) : (
                          <span className="text-mute">{r.remoteError || "—"}</span>
                        )}
                      </td>
                      <td className="py-2 text-right font-mono text-dim">{r.remote?.freeGb != null ? `${r.remote.freeGb.toFixed(0)} GB` : "—"}</td>
                      <td className="py-2 text-right font-mono text-dim">{days(r.remote?.bootDays)}</td>
                      <td className="py-2 text-right font-mono text-dim">{days(r.remote?.updateDays)}</td>
                      <td className="py-2 text-warn">{r.warnings.join(" · ")}</td>
                      <td className="py-2 text-right">
                        {r.ports.includes(3389) && (
                          <button onClick={() => rdp(r.host)} className="rounded px-1.5 py-0.5 text-[11px] text-neon hover:bg-neon/10" title="Conectar por Escritorio remoto">
                            Conectar
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Card>
        )}
      </div>
      {asking === "message" && (
        <Modal title={`Mensaje a ${pickedList.length} ${pickedList.length === 1 ? "equipo" : "equipos"}`} onClose={() => setAsking(null)}>
          <textarea
            value={msg}
            onChange={(e) => setMsg(e.target.value.slice(0, 250))}
            rows={4}
            autoFocus
            placeholder="Por ejemplo: A las 13:00 se reiniciarán los equipos para instalar actualizaciones."
            className={inputClass}
          />
          <p className="mt-1 text-[11px] text-mute">Aparece en la pantalla de quien tenga la sesión abierta durante 10 minutos. {msg.length}/250</p>
          <div className="mt-3 flex justify-end gap-2">
            <Button kind="ghost" onClick={() => setAsking(null)}>
              Cancelar
            </Button>
            <Button onClick={() => act("message", msg)} disabled={!msg.trim()}>
              <MessageSquare size={13} /> Enviar
            </Button>
          </div>
        </Modal>
      )}
      {dialog}
    </div>
  );
}
