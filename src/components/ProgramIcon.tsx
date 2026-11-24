import { useEffect, useState } from "react";
import { netUsageApi } from "../lib/api";

/** Los iconos que ya llegaron (por ruta del programa). Lo que no llegó (`null`) no se guarda: Rust puede no conocer aún la ruta. */
const cache = new Map<string, string>();
const pending = new Map<string, Promise<string | null>>();
/** Cuántas veces se vuelve a pedir un icono que no llegó, y cada cuánto. */
const RETRIES = 3;
const RETRY_MS = 3000;

function iconFor(path: string): Promise<string | null> {
  const hit = pending.get(path);
  if (hit) return hit;
  const p = netUsageApi
    .icon(path)
    .catch(() => null)
    .then((url) => {
      if (url) cache.set(path, url);
      pending.delete(path);
      return url;
    });
  pending.set(path, p);
  return p;
}

/** El icono del programa; sin él, su inicial en un cuadro. */
export function ProgramIcon({ path, name, size = 18 }: { path: string | null; name: string; size?: number }) {
  const [url, setUrl] = useState<string | null>(path ? (cache.get(path) ?? null) : null);
  useEffect(() => {
    if (!path) {
      setUrl(null);
      return;
    }
    const known = cache.get(path);
    if (known) {
      setUrl(known);
      return;
    }
    let alive = true;
    let timer = 0;
    const ask = (left: number) => {
      void iconFor(path).then((u) => {
        if (!alive) return;
        if (u) setUrl(u);
        else if (left > 0) timer = window.setTimeout(() => ask(left - 1), RETRY_MS);
      });
    };
    ask(RETRIES);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [path]);
  const box = { width: size, height: size };
  if (url) return <img src={url} alt="" aria-hidden width={size} height={size} style={box} className="shrink-0 rounded-sm" />;
  return (
    <span aria-hidden style={box} className="flex shrink-0 items-center justify-center rounded-sm border border-line-2 bg-panel-2 text-[10px] font-semibold text-dim">
      {name.trim().charAt(0).toUpperCase() || "?"}
    </span>
  );
}
