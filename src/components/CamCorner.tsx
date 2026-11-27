import { getCurrentWindow } from "@tauri-apps/api/window";
import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { faultRecApi } from "../lib/api";
import { logQuietly } from "../lib/api/core";

/**
 * «Cámara» de Grabar el fallo: la webcam en un círculo en la esquina de lo que se graba (faultrec.rs, ventana
 * `reccam`). Es una ventana más encima del monitor, así que sale en el vídeo. Se arrastra a otra esquina; la ✕
 * (al pasar el ratón) la quita. Se ve como en un espejo, que es como la gente espera verse.
 */
export function CamCorner() {
  const video = useRef<HTMLVideoElement>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    for (const el of [document.documentElement, document.body, document.getElementById("root")]) if (el) el.style.background = "transparent";
    let stream: MediaStream | null = null;
    let alive = true;
    navigator.mediaDevices
      ?.getUserMedia({ video: { width: { ideal: 640 }, height: { ideal: 480 } }, audio: false })
      .then((s) => {
        if (!alive) return s.getTracks().forEach((t) => t.stop());
        stream = s;
        if (video.current) video.current.srcObject = s;
      })
      .catch((e: unknown) => {
        const name = e instanceof DOMException ? e.name : "";
        setError(name === "NotAllowedError" ? "Windows no deja usar la cámara (Privacidad → Cámara)" : name === "NotFoundError" ? "No hay cámara" : name === "NotReadableError" ? "Otra aplicación está usando la cámara" : "No se pudo abrir la cámara");
      });
    return () => {
      alive = false;
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, []);

  // Con un fallo se avisa unos segundos y se quita sola: no debe quedarse un círculo vacío en el vídeo.
  useEffect(() => {
    if (!error) return;
    const id = window.setTimeout(() => void faultRecApi.camera(false).catch(logQuietly("CamCorner.close")), 4000);
    return () => window.clearTimeout(id);
  }, [error]);

  return (
    <div
      className="group relative h-screen w-screen cursor-grab select-none overflow-hidden rounded-full border-[3px] border-white/90 bg-[#0e1319] shadow-[0_6px_20px_rgba(0,0,0,0.45)]"
      onPointerDown={(e) => {
        if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
        void getCurrentWindow().startDragging().catch(logQuietly("CamCorner.drag"));
      }}
    >
      {error ? (
        <div className="flex h-full items-center justify-center p-4 text-center text-[12px] leading-[15px] text-[#e8eef4]">{error}</div>
      ) : (
        <video ref={video} autoPlay muted playsInline className="h-full w-full scale-x-[-1] object-cover" />
      )}
      <button
        type="button"
        aria-label="Quitar la cámara"
        title="Quitar la cámara"
        className="absolute left-1/2 top-[8%] grid h-7 w-7 -translate-x-1/2 place-items-center rounded-full bg-black/60 text-white opacity-0 transition-opacity group-hover:opacity-100"
        onClick={() => void faultRecApi.camera(false).catch(logQuietly("CamCorner.close"))}
      >
        <X size={15} />
      </button>
    </div>
  );
}
