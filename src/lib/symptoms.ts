import { Bluetooth, Gauge, Globe, Monitor, Printer, RefreshCcwDot, Volume2, Wifi, type LucideIcon } from "lucide-react";
import type { Symptom } from "./api";

// Síntomas de «Solucionar problemas» (también en la búsqueda global).
export const SYMPTOMS: { id: Symptom; title: string; hint: string; icon: LucideIcon; keywords: string }[] = [
  { id: "internet", title: "No hay Internet", hint: "No cargan las webs, «sin Internet», cortes", icon: Globe, keywords: "red conexion internet navegar dns router proxy" },
  { id: "wifi", title: "La Wi-Fi no funciona", hint: "No aparece, no conecta o se corta", icon: Wifi, keywords: "wifi wireless inalambrica tarjeta" },
  { id: "audio", title: "No suena", hint: "Sin sonido, altavoz con una X, micrófono", icon: Volume2, keywords: "audio sonido altavoces auriculares microfono volumen" },
  { id: "bluetooth", title: "Bluetooth", hint: "No aparece, no empareja o no conecta", icon: Bluetooth, keywords: "bluetooth auriculares raton teclado emparejar" },
  { id: "display", title: "Pantalla o monitor", hint: "Monitor sin imagen, parpadeos, resolución", icon: Monitor, keywords: "pantalla monitor grafica video hdmi resolucion negra" },
  { id: "printer", title: "No imprime", hint: "Cola atascada, sin conexión, predeterminada", icon: Printer, keywords: "impresora imprimir cola spooler" },
  { id: "slow", title: "Va lento", hint: "Tarda en abrir, se congela, arranca lento", icon: Gauge, keywords: "lento lentitud rendimiento memoria cpu disco congelado" },
  { id: "winupdate", title: "Windows Update falla", hint: "No actualiza, errores al instalar", icon: RefreshCcwDot, keywords: "actualizaciones windows update error parches" },
];
