// Memorias USB: permitidas, solo lectura o bloqueadas (usbpolicy.rs).
import { invoke } from "./core";

export type UsbMode = "allowed" | "readonly" | "blocked";

export interface UsbState {
  mode: UsbMode;
  /** «Permitir N minutos» en marcha: a qué hora vuelve (HH:mm). */
  until: string | null;
  admin: boolean;
  /** AdminOps portable se ejecuta desde esta memoria («E:»): no se pueden bloquear. */
  portableDrive: string | null;
}

/** Una memoria o disco USB que Windows ha visto en este equipo. */
export interface UsbSeen {
  name: string;
  serial: string;
  /** Última conexión («2026-11-24 09:15»); no siempre se sabe. */
  last: string | null;
}

export interface HiddenUsb {
  name: string;
  /** «sin-letra» o la letra que le tapa una unidad de red. */
  reason: string;
  disk: number;
  partition: number;
}

export const usbApi = {
  /** Tarda unos segundos (PowerShell): se pide al pulsar, no al abrir. */
  history: () => invoke<UsbSeen[]>("usb_history"),
  state: () => invoke<UsbState>("usb_state"),
  set: (mode: UsbMode) => invoke<UsbState>("usb_set", { mode }),
  allowFor: (minutes: number) => invoke<UsbState>("usb_allow_for", { minutes }),
  /** Memorias conectadas que el Explorador no enseña (sin letra o tapadas por una unidad de red). */
  hidden: () => invoke<HiddenUsb[]>("usb_hidden"),
  /** Les da una letra libre y abre la primera. Devuelve «Nombre → F:». */
  fixLetters: () => invoke<string[]>("usb_fix_letters"),
};
