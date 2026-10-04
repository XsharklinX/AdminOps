// Carpetas compartidas, el otro lado: las unidades de red de este equipo (las
// letras que apuntan a carpetas de otros) y qué comparte otro equipo de la red.
import { Copy, ExternalLink, Folder, HardDrive, Loader2, Monitor, Plug, Printer, RotateCw, Search, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { officeApi, type NetDrives, type RemoteShare } from "../lib/api";
import { useConfirm, useToast } from "./feedback";
import { Button, Card, inputClass, Loading, iconBtn } from "./ui";

/** Unidades de red que Windows recuerda: reconectar las rotas, conectar otra, quitar. */
export function NetworkDrives({ refresh }: { refresh: number }) {
  const [data, setData] = useState<NetDrives | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [letter, setLetter] = useState("");
  const [path, setPath] = useState("");
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    try {
      const d = await officeApi.drives();
      setData(d);
      setLetter((l) => (l && d.free.includes(l) ? l : (d.free[0] ?? "")));
    } catch (e) {
      toast("error", String(e));
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load, refresh]);

  const open = (target: string) => void officeApi.openNetworkPath(target).catch((e) => toast("error", String(e)));

  const reconnect = async (l: string) => {
    setBusy(l);
    try {
      toast("ok", await officeApi.reconnectDrive(l));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
    void load();
  };

  const remove = async (l: string, target: string) => {
    const ok = await confirm({
      title: `¿Quitar la unidad ${l}:?`,
      confirmLabel: "Quitar",
      body: <p>Deja de aparecer en «Este equipo». La carpeta {target} y sus archivos no se tocan: siguen en el otro equipo.</p>,
    });
    if (!ok) return;
    setBusy(l);
    try {
      await officeApi.unmapDrive(l);
      toast("ok", `Unidad ${l}: quitada.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
    void load();
  };

  const map = async () => {
    setBusy("map");
    try {
      await officeApi.mapDrive(letter, path.trim());
      toast("ok", `Unidad ${letter}: conectada.`);
      setPath("");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
    void load();
  };

  return (
    <Card title="Unidades de red de este equipo" icon={<HardDrive size={14} />}>
      <div className="mb-3 flex items-center gap-3">
        <p className="min-w-0 flex-1 text-sm text-dim">Las letras que apuntan a carpetas de otros equipos. Si una sale con la X roja, aquí se ve si es que ese equipo no contesta.</p>
        <Button kind="ghost" onClick={() => void load()} title="Volver a mirar">
          <RotateCw size={14} />
        </Button>
      </div>
      {data === null ? (
        <Loading />
      ) : data.drives.length === 0 ? (
        <p className="text-sm text-mute">Este equipo no tiene ninguna unidad de red guardada.</p>
      ) : (
        <ul className="divide-y divide-line/60">
          {data.drives.map((d) => (
            <li key={d.letter} className="flex items-center gap-3 py-2.5">
              <span className="grid h-8 w-8 shrink-0 place-items-center rounded-md bg-panel-2 font-mono text-sm text-ink">{d.letter}:</span>
              <div className="min-w-0 flex-1">
                <div className="truncate font-mono text-xs text-ink" title={d.path}>
                  {d.path}
                </div>
                <div className={`text-[11px] ${d.reachable ? "text-ok" : "text-bad"}`}>
                  {d.reachable ? "El equipo contesta" : `«${d.host}» no contesta: apagado, fuera de la red o con otro nombre`}
                </div>
              </div>
              <Button kind="ghost" onClick={() => void reconnect(d.letter)} disabled={busy !== null} title="Volver a conectarla (cuando sale con la X roja)">
                {busy === d.letter ? <Loader2 size={14} className="animate-spin" /> : <Plug size={14} />} Reconectar
              </Button>
              <button onClick={() => open(`${d.letter}:`)} className={iconBtn} title="Abrir en el Explorador (si pide usuario y contraseña, se escriben ahí)">
                <ExternalLink size={14} />
              </button>
              <button onClick={() => void remove(d.letter, d.path)} disabled={busy !== null} className={`${iconBtn} hover:text-bad`} title="Quitar esta unidad">
                <Trash2 size={14} />
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="mt-4 border-t border-line/60 pt-3">
        <div className="mb-1.5 text-xs text-dim">Conectar una unidad nueva</div>
        <div className="flex flex-wrap items-center gap-2">
          <select value={letter} onChange={(e) => setLetter(e.target.value)} className={`${inputClass} w-20`} aria-label="Letra">
            {(data?.free ?? []).map((l) => (
              <option key={l} value={l}>
                {l}:
              </option>
            ))}
          </select>
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && path.trim() && letter && void map()}
            placeholder="\\EQUIPO\Carpeta"
            className={`${inputClass} min-w-56 flex-1 font-mono`}
            aria-label="Ruta de red"
          />
          <Button onClick={() => void map()} disabled={busy !== null || !letter || !path.trim()}>
            {busy === "map" ? <Loader2 size={14} className="animate-spin" /> : <Plug size={14} />} Conectar
          </Button>
        </div>
        <p className="mt-1.5 text-[11px] text-mute">Windows la recuerda y la reconecta al iniciar sesión. AdminOps no guarda contraseñas: si el otro equipo las pide, las pide y las guarda Windows.</p>
      </div>
      {dialog}
    </Card>
  );
}

const LAST_HOST = "adminops.shares.lastHost";

/** Qué comparte otro equipo de la red: escribir su nombre, ver sus carpetas y conectarlas. */
export function OtherPcShares({ onMapped }: { onMapped: () => void }) {
  const [host, setHost] = useState(() => {
    try {
      return localStorage.getItem(LAST_HOST) ?? "";
    } catch {
      return "";
    }
  });
  const [shown, setShown] = useState("");
  const [list, setList] = useState<RemoteShare[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const toast = useToast();

  const clean = host.trim().replace(/^\\+|\\+$/g, "");

  const look = async () => {
    if (!clean) return;
    setBusy("look");
    setError(null);
    try {
      setList(await officeApi.remoteShares(clean));
      setShown(clean);
      try {
        localStorage.setItem(LAST_HOST, clean);
      } catch {
        /* sin almacenamiento */
      }
    } catch (e) {
      setList(null);
      setShown(clean);
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const map = async (name: string) => {
    setBusy(name);
    try {
      const { free } = await officeApi.drives();
      if (free.length === 0) throw new Error("No queda ninguna letra libre.");
      await officeApi.mapDrive(free[0], `\\\\${shown}\\${name}`);
      toast("ok", `«${name}» conectada como ${free[0]}:.`);
      onMapped();
    } catch (e) {
      toast("error", String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(null);
    }
  };

  const open = (target: string) => void officeApi.openNetworkPath(target).catch((e) => toast("error", String(e)));

  return (
    <Card title="Qué comparte otro equipo" icon={<Monitor size={14} />}>
      <p className="mb-3 text-sm text-dim">Escribe el nombre o la IP de un equipo de la red para ver sus carpetas e impresoras compartidas, con tu usuario de Windows.</p>
      <div className="flex flex-wrap items-center gap-2">
        <input
          value={host}
          onChange={(e) => setHost(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void look()}
          placeholder="PC-RECEPCION o 192.168.1.20"
          className={`${inputClass} min-w-56 flex-1`}
          aria-label="Nombre o IP del equipo"
        />
        <Button onClick={() => void look()} disabled={busy !== null || !clean}>
          {busy === "look" ? <Loader2 size={14} className="animate-spin" /> : <Search size={14} />} Ver
        </Button>
      </div>

      {error && (
        <div className="mt-3 flex flex-wrap items-center gap-3 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2.5">
          <p className="min-w-0 flex-1 text-sm text-ink">{error}</p>
          <Button kind="ghost" onClick={() => open(`\\\\${shown}`)}>
            <ExternalLink size={14} /> Abrir en el Explorador
          </Button>
        </div>
      )}

      {list && list.length === 0 && <p className="mt-3 text-sm text-mute">«{shown}» no comparte nada (o nada que tu usuario pueda ver).</p>}
      {list && list.length > 0 && (
        <ul className="mt-3 divide-y divide-line/60">
          {list.map((s) => {
            const unc = `\\\\${shown}\\${s.name}`;
            return (
              <li key={s.name} className="flex items-center gap-3 py-2">
                {s.kind === "printer" ? <Printer size={15} className="shrink-0 text-mute" /> : <Folder size={15} className="shrink-0 text-mute" />}
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm text-ink">{s.name}</div>
                  <div className="truncate text-[11px] text-mute">{s.remark || (s.kind === "printer" ? "Impresora" : unc)}</div>
                </div>
                {s.kind === "folder" && (
                  <Button kind="ghost" onClick={() => void map(s.name)} disabled={busy !== null} title="Conectarla como unidad de red en la primera letra libre">
                    {busy === s.name ? <Loader2 size={14} className="animate-spin" /> : <Plug size={14} />} Conectar como unidad
                  </Button>
                )}
                <button onClick={() => void navigator.clipboard.writeText(unc).then(() => toast("ok", "Ruta copiada."), () => {})} className={iconBtn} title="Copiar la ruta">
                  <Copy size={14} />
                </button>
                <button onClick={() => open(unc)} className={iconBtn} title="Abrir en el Explorador">
                  <ExternalLink size={14} />
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </Card>
  );
}
