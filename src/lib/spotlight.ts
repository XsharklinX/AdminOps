// El foco que sigue al cursor sobre lo que se puede pulsar (.glow, ver index.css).
// Un solo oyente para toda la ventana: pone --mx y --my en el elemento bajo el ratón.

let on = false;

export function watchSpotlight() {
  if (on || typeof window === "undefined") return;
  on = true;
  let frame = 0;
  let last: PointerEvent | null = null;
  const paint = () => {
    frame = 0;
    const e = last;
    if (!e || !(e.target instanceof Element)) return;
    const el = e.target.closest<HTMLElement>(".glow");
    if (!el) return;
    const r = el.getBoundingClientRect();
    el.style.setProperty("--mx", `${e.clientX - r.left}px`);
    el.style.setProperty("--my", `${e.clientY - r.top}px`);
  };
  window.addEventListener(
    "pointermove",
    (e) => {
      if (e.pointerType !== "mouse") return;
      last = e;
      if (!frame) frame = requestAnimationFrame(paint);
    },
    { passive: true },
  );
}
