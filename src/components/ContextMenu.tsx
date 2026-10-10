// Clic derecho sobre cualquier cosa: sus acciones habituales donde estás
// mirando, sin ir a buscar el botón. Se deduce de lo que hay bajo el ratón (un
// texto seleccionado, una IP, una ruta, un nombre de equipo, una tarjeta) y de
// lo que la pantalla marque con `data-ctx-*`:
//   data-ctx-copy="texto"     lo que se copia (si no, el texto visible)
//   data-ctx-path="C:\…"      se ofrece abrirlo en el Explorador
//   data-ctx-machine="PC-01"  abrir un caso con ese equipo
import { ClipboardCopy, ClipboardPaste, FolderOpen, Globe, Images, LifeBuoy, RotateCw, Scissors, Search, TextSelect } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { openCase } from "../lib/currentCase";
import { recognize } from "../lib/recognize";
import { lanApi, logQuietly, uxApi } from "../lib/api";
import { copyAsImage } from "../lib/copyImage";
import { openPalette } from "../lib/palette";

interface Item {
  label: string;
  icon: ReactNode;
  run: () => void;
  hint?: string;
}

/** Ruta de Windows dentro de un texto (C:\… o \\servidor\…). */
export function findPath(text: string): string | null {
  const m = text.match(/(?:[a-z]:\\|\\\\[\w.-]+\\)[^\n"<>|*?]*/i);
  return m ? m[0].replace(/[\s.,;:)]+$/, "") : null;
}

/** IP dentro de un texto. */
export function findIp(text: string): string | null {
  const m = text.match(/\b(?:25[0-5]|2[0-4]\d|1?\d?\d)(?:\.(?:25[0-5]|2[0-4]\d|1?\d?\d)){3}\b/);
  return m ? m[0] : null;
}

/** Texto corto que representa lo que hay bajo el ratón (una celda, una etiqueta…). */
export function shortText(raw: string, max = 80): string {
  const t = raw.replace(/\s+/g, " ").trim();
  return t.length > max ? `${t.slice(0, max - 1)}…` : t;
}

const LEAF = "td, th, li, dd, dt, code, kbd, a, button, label, h1, h2, h3, h4, p, span";

const copy = (text: string) => void navigator.clipboard?.writeText(text).catch(logQuietly("ContextMenu"));

function itemsFor(target: HTMLElement): Item[] {
  const items: Item[] = [];
  const field = target.closest<HTMLInputElement | HTMLTextAreaElement>("input:not([type=checkbox]):not([type=radio]):not([type=range]), textarea");
  const selected = window.getSelection()?.toString().trim() ?? "";
  if (field) {
    const sel = field.value.slice(field.selectionStart ?? 0, field.selectionEnd ?? 0);
    if (sel)
      items.push({
        label: "Cortar",
        icon: <Scissors size={13} />,
        hint: "Ctrl+X",
        run: () => {
          copy(sel);
          field.focus();
          document.execCommand("delete");
        },
      });
    if (sel) items.push({ label: "Copiar", icon: <ClipboardCopy size={13} />, hint: "Ctrl+C", run: () => copy(sel) });
    items.push({
      label: "Pegar",
      icon: <ClipboardPaste size={13} />,
      hint: "Ctrl+V",
      run: () =>
        void navigator.clipboard?.readText().then((t) => {
          field.focus();
          document.execCommand("insertText", false, t);
        }),
    });
    items.push({ label: "Seleccionar todo", icon: <TextSelect size={13} />, hint: "Ctrl+A", run: () => field.select() });
    return items;
  }

  const marked = target.closest<HTMLElement>("[data-ctx-copy],[data-ctx-path],[data-ctx-machine]");
  const leaf = target.closest<HTMLElement>(LEAF) ?? target;
  const text = selected || marked?.dataset.ctxCopy || shortText(leaf.innerText || leaf.textContent || "", 400);
  if (text) items.push({ label: `Copiar «${shortText(text, 36)}»`, icon: <ClipboardCopy size={13} />, run: () => copy(text) });

  const path = marked?.dataset.ctxPath ?? findPath(text);
  if (path) items.push({ label: "Abrir en el Explorador", icon: <FolderOpen size={13} />, run: () => void uxApi.revealPath(path).catch(logQuietly("ContextMenu")) });
  const ip = findIp(text);
  if (ip) items.push({ label: `Abrir la página de ${ip}`, icon: <Globe size={13} />, run: () => void lanApi.openDevice(ip).catch(logQuietly("ContextMenu")) });
  const r = text.length <= 64 ? recognize(text) : null;
  const machine = marked?.dataset.ctxMachine ?? (r?.kind === "computer" ? r.value : null);
  if (text && text.length <= 80) items.push({ label: `Buscar «${shortText(text, 28)}» en Ctrl+K`, icon: <Search size={13} />, run: () => openPalette(text) });
  items.push({
    label: machine ? `Abrir un caso con ${machine}` : "Abrir un caso",
    icon: <LifeBuoy size={13} />,
    run: () => openCase(machine ? { machine } : {}),
  });
  const card = target.closest<HTMLElement>("section[data-setting]");
  if (card) items.push({ label: "Copiar la tarjeta como imagen", icon: <Images size={13} />, run: () => void copyAsImage(card).catch(logQuietly("ContextMenu")) });
  items.push({ label: "Recargar esta pantalla", icon: <RotateCw size={13} />, hint: "F5", run: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "F5" })) });
  return items;
}

export function ContextMenu() {
  const [menu, setMenu] = useState<{ x: number; y: number; items: Item[] } | null>(null);
  const [index, setIndex] = useState(0);
  const box = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x: 0, y: 0 });

  useEffect(() => {
    const open = (e: MouseEvent) => {
      e.preventDefault();
      const t = e.target as HTMLElement | null;
      // Las vistas web y lo que ya tiene su propio menú no se tocan.
      if (!t || t.closest("[data-no-ctx]")) return;
      const items = itemsFor(t);
      if (!items.length) return;
      setMenu({ x: e.clientX, y: e.clientY, items });
      setIndex(0);
    };
    document.addEventListener("contextmenu", open);
    return () => document.removeEventListener("contextmenu", open);
  }, []);

  useLayoutEffect(() => {
    if (!menu || !box.current) return;
    const r = box.current.getBoundingClientRect();
    setPos({ x: Math.min(menu.x, window.innerWidth - r.width - 6), y: Math.min(menu.y, window.innerHeight - r.height - 6) });
    box.current.focus();
  }, [menu]);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
      else if (e.key === "ArrowDown") setIndex((i) => (i + 1) % menu.items.length);
      else if (e.key === "ArrowUp") setIndex((i) => (i - 1 + menu.items.length) % menu.items.length);
      else if (e.key === "Enter") {
        menu.items[index]?.run();
        close();
      } else return;
      e.preventDefault();
      e.stopPropagation();
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("blur", close);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("wheel", close, { passive: true });
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("wheel", close);
    };
  }, [menu, index]);

  if (!menu) return null;
  return createPortal(
    <div
      ref={box}
      role="menu"
      tabIndex={-1}
      onMouseDown={(e) => e.stopPropagation()}
      className="fixed z-[70] min-w-52 max-w-80 rounded-lg border border-line-2 bg-panel p-1 shadow-2xl outline-none"
      style={{ left: pos.x, top: pos.y }}
    >
      {menu.items.map((it, i) => (
        <button
          key={it.label}
          role="menuitem"
          onMouseEnter={() => setIndex(i)}
          onClick={() => {
            it.run();
            setMenu(null);
          }}
          className={`flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-[13px] ${i === index ? "bg-neon/10 text-ink" : "text-dim"}`}
        >
          <span className="text-mute">{it.icon}</span>
          <span className="min-w-0 flex-1 truncate">{it.label}</span>
          {it.hint && <span className="font-mono text-[10.5px] text-mute">{it.hint}</span>}
        </button>
      ))}
    </div>,
    document.body,
  );
}
