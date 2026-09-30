import type { Contact } from "../../lib/api";

/** Tono estable para cada nombre: la misma persona tiene siempre el mismo color. */
export function hueOf(name: string): number {
  let h = 0;
  for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) % 360;
  return h;
}

export function initials(name: string): string {
  return (
    name
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w.charAt(0).toUpperCase())
      .join("") || "?"
  );
}

/** Círculo con las iniciales, del color de la persona. */
export function Avatar({ c, size = 36 }: { c: Pick<Contact, "name" | "favorite">; size?: number }) {
  const h = hueOf(c.name);
  return (
    <span
      className="relative grid shrink-0 place-items-center rounded-full font-semibold"
      style={{
        width: size,
        height: size,
        fontSize: Math.round(size * 0.36),
        background: `hsl(${h} 70% 50% / 0.16)`,
        color: `hsl(${h} 65% 55%)`,
        boxShadow: `inset 0 0 0 1px hsl(${h} 70% 50% / 0.35)`,
      }}
      aria-hidden
    >
      {initials(c.name)}
    </span>
  );
}
