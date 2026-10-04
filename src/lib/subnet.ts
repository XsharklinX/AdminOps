// Calculadora de red: de una IP y su máscara (o /24), el rango de la red, el
// router típico, cuántos equipos caben y si otra IP está en la misma red.

export interface Subnet {
  ip: string;
  prefix: number;
  mask: string;
  network: string;
  broadcast: string;
  first: string;
  last: string;
  hosts: number;
  /** Privada (192.168, 10, 172.16-31), la de los equipos de casa y oficina. */
  private: boolean;
}

export function parseIp(s: string): number | null {
  const m = s.trim().match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (!m) return null;
  const parts = m.slice(1).map(Number);
  if (parts.some((p) => p > 255)) return null;
  return ((parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3]) >>> 0;
}

export const ipText = (n: number) => [n >>> 24, (n >>> 16) & 255, (n >>> 8) & 255, n & 255].join(".");

/** «/24», «24» o «255.255.255.0» → 24. */
export function parsePrefix(s: string): number | null {
  const t = s.trim().replace(/^\//, "");
  if (/^\d{1,2}$/.test(t)) {
    const p = Number(t);
    return p >= 0 && p <= 32 ? p : null;
  }
  const m = parseIp(t);
  if (m === null) return null;
  // Tiene que ser una máscara de verdad: unos seguidos y luego ceros.
  const inv = ~m >>> 0;
  if ((inv & (inv + 1)) !== 0) return null;
  return 32 - Math.log2(inv + 1);
}

/** «192.168.1.37/24» o («192.168.1.37», «255.255.255.0»). */
export function subnet(ipOrCidr: string, maskOrPrefix?: string): Subnet | null {
  let ipS = ipOrCidr.trim();
  let pre = maskOrPrefix?.trim() ?? "";
  if (ipS.includes("/")) [ipS, pre] = ipS.split("/");
  const ip = parseIp(ipS);
  const prefix = parsePrefix(pre || "24");
  if (ip === null || prefix === null) return null;
  const mask = prefix === 0 ? 0 : (0xffffffff << (32 - prefix)) >>> 0;
  const network = (ip & mask) >>> 0;
  const broadcast = (network | (~mask >>> 0)) >>> 0;
  const hosts = prefix >= 31 ? 2 ** (32 - prefix) : Math.max(0, 2 ** (32 - prefix) - 2);
  const first = prefix >= 31 ? network : network + 1;
  const last = prefix >= 31 ? broadcast : broadcast - 1;
  const a = ip >>> 24;
  const b = (ip >>> 16) & 255;
  return {
    ip: ipText(ip),
    prefix,
    mask: ipText(mask),
    network: ipText(network),
    broadcast: ipText(broadcast),
    first: ipText(first),
    last: ipText(last),
    hosts,
    private: a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168),
  };
}

/** ¿Está `other` en la misma red que `s`? */
export function sameNetwork(s: Subnet, other: string): boolean | null {
  const o = parseIp(other);
  if (o === null) return null;
  const mask = parseIp(s.mask)!;
  return ((o & mask) >>> 0) === parseIp(s.network);
}
