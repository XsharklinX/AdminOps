// Copia una tarjeta al portapapeles como imagen, para pegarla en Teams o en un
// correo sin hacer un recorte a mano. La biblioteca se carga al usarla.

/** Lo que no debe salir en la imagen (los propios botones de la tarjeta). */
const SKIP = "data-no-capture";

export async function copyAsImage(el: HTMLElement): Promise<void> {
  if (typeof ClipboardItem === "undefined" || !navigator.clipboard?.write) {
    throw new Error("Este equipo no deja copiar imágenes desde AdminOps. Usa «Recorte de pantalla» (Ctrl+K).");
  }
  const { toBlob } = await import("html-to-image");
  const background = getComputedStyle(document.body).backgroundColor;
  // La promesa va dentro del ClipboardItem: así el permiso del clic sigue
  // valiendo aunque dibujar la imagen tarde un momento.
  const image = toBlob(el, {
    pixelRatio: 2,
    backgroundColor: background,
    filter: (node) => !(node instanceof HTMLElement && node.hasAttribute(SKIP)),
    // Un margen alrededor, del color del fondo, para que no quede a sangre.
    style: { margin: "0" },
  }).then((b) => {
    if (!b) throw new Error("No se pudo dibujar la tarjeta.");
    return b;
  });
  await navigator.clipboard.write([new ClipboardItem({ "image/png": image })]);
}

export const NO_CAPTURE = { [SKIP]: "" } as const;
