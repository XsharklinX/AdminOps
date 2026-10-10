import type { ReactNode } from "react";

/** Un dato que se copia al pulsarlo (IP, MAC, serie, ruta, código): destella y dice «Copiado». */
export function Copyable({ value, children, className = "" }: { value?: string; children: ReactNode; className?: string }) {
  return (
    <span data-copy={value ?? ""} title="Clic para copiar" className={className}>
      {children}
    </span>
  );
}
