// Memorias USB: permitidas, solo lectura o bloqueadas (usbpolicy.rs).
import { invoke } from "./core";

export type UsbMode = "allowed" | "readonly" | "blocked";

export interface UsbState {
  mode: UsbMode;
  /** «Permitir N minutos» en marcha: a qué hora vuelve (HH:mm). */
  until: string | null;
  admin: boolean;
}

/** Una memoria o disco USB que Windows ha visto en este equipo. */
export interface UsbSeen {
  name: string;
  serial: string;
  /** Última conexión («2026-11-24 09:15»); no siempre se sabe. */
  last: string | null;
}

export const usbApi = {
  /** Tarda unos segundos (PowerShell): se pide al pulsar, no al abrir. */
  history: () => invoke<UsbSeen[]>("usb_history"),
  state: () => invoke<UsbState>("usb_state"),
  set: (mode: UsbMode) => invoke<UsbState>("usb_set", { mode }),
  allowFor: (minutes: number) => invoke<UsbState>("usb_allow_for", { minutes }),
};
