// Los códigos de error que aparecen en un texto (0x80070005, 0x800F081F…) se
// convierten en enlaces: un clic y Ctrl+K explica qué son y qué hacer.
import { Fragment } from "react";
import { openPalette } from "../lib/palette";

const CODE = /(0x[0-9a-f]{3,8})/gi;

/** Trozos de un texto, separando los códigos de error. */
export function splitCodes(text: string): { text: string; code: boolean }[] {
  return text
    .split(CODE)
    .filter((p) => p !== "")
    .map((p) => ({ text: p, code: /^0x[0-9a-f]{3,8}$/i.test(p) }));
}

export function CodeLinks({ text }: { text: string }) {
  return (
    <>
      {splitCodes(text).map((p, i) =>
        p.code ? (
          <button key={i} onClick={() => openPalette(p.text)} className="font-mono text-neon underline decoration-dotted underline-offset-2 hover:decoration-solid" title="Qué significa este código">
            {p.text}
          </button>
        ) : (
          <Fragment key={i}>{p.text}</Fragment>
        ),
      )}
    </>
  );
}
