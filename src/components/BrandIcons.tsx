// Iconos de color de la barra de arriba. Teams y Correo llevan el suyo de siempre (Microsoft Teams y
// Outlook), redibujado para que se lea a 17 px; la cámara de «Grabar el fallo» sigue la misma
// composición (un cuadrado delante y la pieza detrás) con el verde de AdminOps, para que los tres se
// vean como una familia.
import { useId } from "react";

type Props = { size?: number; className?: string };

export function TeamsIcon({ size = 18, className }: Props) {
  const g = useId();
  return (
    <svg width={size} height={(size * 24) / 26} viewBox="0 0 26 24" className={className} aria-hidden>
      <defs>
        <linearGradient id={g} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#5a62c3" />
          <stop offset="1" stopColor="#3940ab" />
        </linearGradient>
      </defs>
      <circle cx="15" cy="4.2" r="3.4" fill="#7b83eb" />
      <circle cx="22" cy="5" r="2.6" fill="#5059c9" />
      <path d="M19 9h6v6.5a3.5 3.5 0 0 1-6 2.4z" fill="#5059c9" />
      <path d="M9 9h11v8a5.5 5.5 0 0 1-11 0z" fill="#7b83eb" />
      <rect x="1" y="5.5" width="12" height="12" rx="1.6" fill={`url(#${g})`} />
      <path d="M4 8.6h6v1.6H7.8V15H6.2v-4.8H4z" fill="#fff" />
    </svg>
  );
}

export function OutlookIcon({ size = 18, className }: Props) {
  const g = useId();
  return (
    <svg width={size} height={(size * 24) / 26} viewBox="0 0 26 24" className={className} aria-hidden>
      <defs>
        <linearGradient id={g} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#1784d9" />
          <stop offset="1" stopColor="#0a63c9" />
        </linearGradient>
      </defs>
      <rect x="8" y="2" width="17" height="11" rx="1.2" fill="#0364b8" />
      <rect x="14" y="5" width="5.5" height="4" fill="#28a8ea" />
      <rect x="19.5" y="5" width="5.5" height="4" fill="#50d9ff" />
      <path d="M7 11h18v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 7 20.5z" fill="#1490df" />
      <path d="m7 11 9 6 9-6" fill="none" stroke="#0a2767" strokeWidth="1" />
      <rect x="1" y="6" width="12" height="12" rx="1.6" fill={`url(#${g})`} />
      <ellipse cx="7" cy="12" rx="2.9" ry="3.4" fill="none" stroke="#fff" strokeWidth="1.6" />
    </svg>
  );
}

/** «Grabar el fallo»: la cámara verde detrás y, delante, el cuadrado con el punto de grabar. */
export function RecordIcon({ size = 18, className }: Props) {
  const g = useId();
  return (
    <svg width={size} height={(size * 24) / 26} viewBox="0 0 26 24" className={className} aria-hidden>
      <defs>
        <linearGradient id={g} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#25a36d" />
          <stop offset="1" stopColor="#14784e" />
        </linearGradient>
      </defs>
      <rect x="7" y="4" width="13" height="15" rx="2.2" fill="#3ecf8e" />
      <path d="M19.5 9.2 25 6v11l-5.5-3.2z" fill="#2bb57a" />
      <rect x="1" y="6.5" width="12" height="12" rx="1.6" fill={`url(#${g})`} />
      <circle cx="7" cy="12.5" r="2.9" fill="#fff" />
    </svg>
  );
}

/** El botón de grabar: un punto rojo en su aro; verde y latiendo mientras graba (la barra del cohete y la cabecera). */
export function RecDot({ size = 18, recording = false, className }: Props & { recording?: boolean }) {
  const c = recording ? "#3ecf8e" : "#f2706b";
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} className={className} aria-hidden>
      <circle cx="12" cy="12" r="10" fill={c} fillOpacity="0.16" stroke={c} strokeWidth="1.6" />
      <circle cx="12" cy="12" r="5.2" fill={c} className={recording ? "animate-pulse" : undefined} />
    </svg>
  );
}
