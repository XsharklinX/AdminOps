import { BadgeCheck, Bluetooth, Cloud, Compass, Gauge, Globe, Mail, MailWarning, Monitor, Printer, RefreshCcwDot, Usb, UserX, Volume2, Wifi, type LucideIcon } from "lucide-react";
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
  { id: "winupdate", title: "Windows Update falla", hint: "Qué actualización falla y por qué, con el arreglo en orden", icon: RefreshCcwDot, keywords: "actualizaciones windows update error parches kb dism sfc cbs 0x800f081f 0x80073712" },
  { id: "outlook", title: "Outlook y Office", hint: "Lento, pide la contraseña, archivo de datos lleno, complementos", icon: Mail, keywords: "outlook office correo ost pst complementos lento contraseña perfil reparar licencia" },
  { id: "onedrive", title: "OneDrive o Teams no sincronizan", hint: "Archivos que no suben, conflictos, Teams en blanco", icon: Cloud, keywords: "onedrive sincronizar subir nube teams cache conflicto ruta larga sharepoint" },
  { id: "profile", title: "Perfil temporal", hint: "«Iniciaste sesión con un perfil temporal», escritorio vacío", icon: UserX, keywords: "perfil temporal usuario escritorio vacio bak profilelist sesion" },
  { id: "browser", title: "Navegador raro o lento", hint: "Buscador cambiado, extensiones, «Administrado por tu organización»", icon: Compass, keywords: "navegador chrome edge firefox secuestrado buscador extensiones administrado organizacion lento cache" },
  { id: "license", title: "Licencias de Windows y Office", hint: "Tipo de licencia, clave de la placa, activación", icon: BadgeCheck, keywords: "licencia activacion windows office oem retail kms clave placa bios" },
  { id: "usb", title: "USB que se desconecta", hint: "Ratón, teclado o lector que se va y vuelve", icon: Usb, keywords: "usb desconecta raton teclado receptor ahorro energia puerto" },
  { id: "mail", title: "Correo que no sale o no llega", hint: "Spam, rechazos: MX, SPF, DKIM, DMARC y listas negras", icon: MailWarning, keywords: "correo spam rechazado dominio mx spf dkim dmarc smtp lista negra" },
];
