import { X } from "lucide-react";
import { useMemo, useRef, useState, type CSSProperties } from "react";
import { norm, TAG_COLORS } from "../../lib/contacts";

export type TagColors = Record<string, string>;

/** Estilo de una etiqueta con color (o neutro). */
export function tagStyle(color?: string): CSSProperties | undefined {
  const hex = color ? (TAG_COLORS[color] ?? color) : undefined;
  return hex ? { color: hex, borderColor: `${hex}66`, background: `${hex}1a` } : undefined;
}

export const colorOf = (colors: TagColors, tag: string) => colors[norm(tag)];

export function TagChip({
  name,
  color,
  onRemove,
  onClick,
  active,
  count,
}: {
  name: string;
  color?: string;
  onRemove?: () => void;
  onClick?: () => void;
  active?: boolean;
  count?: number;
}) {
  const cls = `inline-flex items-center gap-1 rounded-full border px-2 py-px text-[11px] leading-5 transition-colors ${
    active ? "ring-1 ring-neon/70" : ""
  } ${color ? "" : "border-line text-dim"} ${onClick ? "cursor-pointer hover:brightness-125" : ""}`;
  const body = (
    <>
      {name}
      {count !== undefined && <span className="opacity-60">{count}</span>}
      {onRemove && (
        <span
          role="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          className="-mr-0.5 rounded-full opacity-60 hover:opacity-100"
          title="Quitar"
        >
          <X size={10} />
        </span>
      )}
    </>
  );
  return onClick ? (
    <button type="button" onClick={onClick} className={cls} style={tagStyle(color)}>
      {body}
    </button>
  ) : (
    <span className={cls} style={tagStyle(color)}>
      {body}
    </span>
  );
}

/**
 * Varias etiquetas: se escribe y con Enter, coma o Tab queda puesta; Retroceso
 * con el campo vacío quita la última. Sugiere las que ya existen.
 */
export function TagInput({
  value,
  onChange,
  suggestions,
  colors,
  placeholder = "Escribe y pulsa Enter…",
  autoFocus,
}: {
  value: string[];
  onChange: (tags: string[]) => void;
  suggestions: string[];
  colors: TagColors;
  placeholder?: string;
  autoFocus?: boolean;
}) {
  const [text, setText] = useState("");
  const [open, setOpen] = useState(false);
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  const matches = useMemo(() => {
    const q = norm(text);
    const taken = new Set(value.map(norm));
    return suggestions.filter((s) => !taken.has(norm(s)) && (!q || norm(s).includes(q))).slice(0, 8);
  }, [text, suggestions, value]);

  const add = (raw: string) => {
    const t = raw.trim().replace(/^#/, "");
    if (!t) return;
    // Si ya existe con otras mayúsculas, se usa la existente (evita «TI» y «ti»).
    const existing = suggestions.find((s) => norm(s) === norm(t));
    const tag = existing ?? t;
    if (!value.some((v) => norm(v) === norm(tag))) onChange([...value, tag]);
    setText("");
    setIndex(0);
  };

  return (
    <div className="relative">
      <div
        onClick={() => input.current?.focus()}
        className="flex min-h-[38px] w-full cursor-text flex-wrap items-center gap-1 rounded-md border border-line bg-void/60 px-2 py-1.5 focus-within:border-neon/50"
      >
        {value.map((t) => (
          <TagChip key={t} name={t} color={colorOf(colors, t)} onRemove={() => onChange(value.filter((x) => x !== t))} />
        ))}
        <input
          ref={input}
          autoFocus={autoFocus}
          value={text}
          onChange={(e) => {
            const v = e.target.value;
            // Pegar «a, b, c» crea las tres.
            if (v.includes(",")) {
              const parts = v.split(",");
              parts.slice(0, -1).forEach(add);
              setText(parts[parts.length - 1]);
            } else setText(v);
            setOpen(true);
            setIndex(0);
          }}
          onFocus={() => setOpen(true)}
          onBlur={() => window.setTimeout(() => setOpen(false), 120)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || (e.key === "Tab" && text.trim())) {
              e.preventDefault();
              add(open && matches[index] && text ? matches[index] : text);
            } else if (e.key === "Backspace" && !text && value.length) {
              onChange(value.slice(0, -1));
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              setIndex((i) => Math.min(i + 1, matches.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setIndex((i) => Math.max(i - 1, 0));
            } else if (e.key === "Escape") {
              setOpen(false);
            }
          }}
          placeholder={value.length ? "" : placeholder}
          className="min-w-24 flex-1 bg-transparent px-1 text-sm text-ink outline-none placeholder:text-mute"
        />
      </div>
      {open && matches.length > 0 && (
        <div className="absolute top-full right-0 left-0 z-20 mt-1 max-h-48 overflow-y-auto rounded-md border border-line-2 bg-panel py-1 shadow-xl">
          {matches.map((m, i) => (
            <button
              key={m}
              type="button"
              onMouseDown={(e) => {
                e.preventDefault();
                add(m);
              }}
              onMouseMove={() => setIndex(i)}
              className={`flex w-full items-center px-3 py-1 text-left ${i === index ? "bg-neon/10" : ""}`}
            >
              <TagChip name={m} color={colorOf(colors, m)} />
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** Selector de color de etiqueta. */
export function ColorPicker({ value, onChange }: { value: string; onChange: (c: string) => void }) {
  return (
    <div className="flex flex-wrap gap-1">
      <button type="button" onClick={() => onChange("")} title="Sin color" className={`size-5 rounded-full border border-line ${!value ? "ring-2 ring-neon" : ""}`} />
      {Object.entries(TAG_COLORS).map(([name, hex]) => (
        <button
          key={name}
          type="button"
          onClick={() => onChange(name)}
          title={name}
          className={`size-5 rounded-full ${value === name ? "ring-2 ring-neon ring-offset-1 ring-offset-panel" : ""}`}
          style={{ background: hex }}
        />
      ))}
    </div>
  );
}
