// Glosario: qué es cada término técnico, si debe preocupar y qué hacer. Lo usa
// el subrayado de puntos de <Term>, Ctrl+K («qué es TPM») y la Guía.

export interface GlossaryEntry {
  term: string;
  /** Qué es, en una frase sin jerga. */
  what: string;
  /** Si debe preocupar y cuándo. */
  worry: string;
  /** Qué hacer (o dónde mirarlo en AdminOps). */
  todo: string;
  /** Otras formas de escribirlo. */
  aliases?: string[];
}

export const GLOSSARY: GlossaryEntry[] = [
  { term: "SMART", what: "El informe de salud que lleva dentro cada disco: errores, horas encendido, sectores dañados.", worry: "Si hay sectores reasignados o pendientes, el disco se está estropeando.", todo: "Discos → Salud lo lee y lo traduce. Haz copia si sale en ámbar o rojo." },
  { term: "CRC", what: "Errores al pasar los datos por el cable entre el disco y la placa.", worry: "Si sube, el problema suele ser el cable o el conector, no el disco.", todo: "Cambia el cable SATA o el puerto y vuelve a mirar si sigue subiendo." },
  { term: "Sectores reasignados", what: "Trozos del disco que fallaron y el propio disco ha sustituido por otros de reserva.", worry: "Unos pocos se toleran; si crecen semana a semana, el disco va a fallar.", todo: "Copia de seguridad ya y vigila la evolución en Discos.", aliases: ["reasignados", "reallocated"] },
  { term: "Sectores pendientes", what: "Trozos del disco que no se pudieron leer y esperan a reasignarse.", worry: "Preocupan más que los reasignados: hay datos que no se leen.", todo: "No sigas usando el disco más de lo necesario: clónalo con Rescate.", aliases: ["pendientes", "pending"] },
  { term: "TBW", what: "Terabytes escritos: cuánto se ha escrito en un SSD a lo largo de su vida.", worry: "Cuando se acerca a lo que garantiza el fabricante, el SSD se gasta.", todo: "Discos → Vida útil estima cuánto le queda.", aliases: ["terabytes escritos"] },
  { term: "NVMe", what: "Un tipo de SSD muy rápido que va directo a la placa, sin cable.", worry: "No tiene la tabla SMART de los SATA, sino su propio registro de salud.", todo: "Mira «Desgaste» y «Repuesto disponible» en Discos." },
  { term: "TPM", what: "Un chip de seguridad de la placa que guarda claves (BitLocker, Windows Hello).", worry: "Windows 11 lo pide (versión 2.0). Sin él no se puede actualizar oficialmente.", todo: "Se activa en la BIOS (a veces se llama PTT o fTPM)." },
  { term: "Secure Boot", what: "Arranque seguro: la placa solo arranca sistemas firmados.", worry: "Apagado no es grave, pero Windows 11 lo pide y protege contra virus de arranque.", todo: "Se activa en la BIOS, con el disco en GPT.", aliases: ["arranque seguro"] },
  { term: "BitLocker", what: "El cifrado de disco de Windows: si roban el equipo, no pueden leer los datos.", worry: "Sin la clave de recuperación, un cambio de placa o de BIOS puede dejar el disco ilegible.", todo: "Guarda la clave de recuperación antes de tocar la BIOS o cambiar piezas." },
  { term: "GPT", what: "La forma moderna de organizar las particiones de un disco.", worry: "Necesaria para arrancar en UEFI y para discos de más de 2 TB.", todo: "Discos → Particiones enseña cuál tiene el disco." },
  { term: "MBR", what: "La forma antigua de organizar las particiones (hasta 4 principales y 2 TB).", worry: "No sirve para Windows 11 con Secure Boot.", todo: "Se puede convertir a GPT con mbr2gpt, con copia previa." },
  { term: "BCD", what: "La lista de arranque de Windows: qué sistema arranca y desde dónde.", worry: "Si se daña, el equipo dice que no encuentra el sistema.", todo: "Discos → Particiones → Reparar el arranque lo rehace." },
  { term: "EFI", what: "La pequeña partición donde están los archivos de arranque en los equipos UEFI.", worry: "Si se borra o se daña, Windows no arranca aunque esté entero.", todo: "Reparar el arranque la vuelve a llenar.", aliases: ["particion efi", "esp"] },
  { term: "UEFI", what: "El sustituto moderno de la BIOS clásica.", worry: "Windows 11 lo necesita.", todo: "Se ve en Estado del equipo → Hardware." },
  { term: "DISM", what: "Herramienta de Windows que repara el almacén de componentes del sistema.", worry: "Hace falta cuando SFC no puede reparar o Windows Update falla.", todo: "Solucionar problemas → Reparaciones la ejecuta con su progreso." },
  { term: "SFC", what: "Comprobador de archivos de sistema: busca y repone archivos de Windows dañados.", worry: "Si encuentra daños que no puede arreglar, toca DISM.", todo: "Solucionar problemas → Reparaciones." },
  { term: "DNS", what: "El listín que traduce nombres (google.com) a direcciones.", worry: "Si falla, hay Internet pero las páginas no cargan.", todo: "Red → Reparar la red, o cambiar a un DNS público." },
  { term: "DHCP", what: "El servicio (normalmente el router) que reparte las direcciones IP.", worry: "Si se llena o falla, los equipos nuevos no consiguen IP.", todo: "Reinicia el router o amplía el rango de direcciones." },
  { term: "MTU", what: "El tamaño máximo de cada paquete de red.", worry: "Si es demasiado grande para la conexión (VPN, fibra), algunas webs no cargan.", todo: "Red → Diagnóstico avanzado busca el tamaño que pasa." },
  { term: "SMBv1", what: "Versión antigua del protocolo para compartir carpetas.", worry: "Es inseguro (lo usó WannaCry). Solo lo necesitan aparatos muy viejos.", todo: "Desactívalo salvo que una impresora o NAS antiguo lo pida." },
  { term: "RDP", what: "Escritorio remoto de Windows.", worry: "Abierto a Internet es una de las entradas favoritas de los atacantes.", todo: "Úsalo solo por VPN o dentro de la red.", aliases: ["escritorio remoto"] },
  { term: "LAPS", what: "Contraseñas de administrador local distintas en cada equipo, guardadas en el dominio.", worry: "Sin LAPS suele haber la misma contraseña en todos: si cae uno, caen todos.", todo: "Personas → buscar el equipo enseña su contraseña LAPS." },
  { term: "KMS", what: "Activación por volumen de Windows u Office contra un servidor de la empresa.", worry: "Fuera de una empresa, un KMS suele ser una activación pirata.", todo: "Estado del equipo → Licencias explica el tipo de cada una." },
  { term: "OEM", what: "Licencia que viene con el equipo, grabada en la placa.", worry: "Va ligada a esa placa: si se cambia, puede dejar de valer.", todo: "Estado del equipo → Licencias." },
  { term: "OST", what: "El archivo donde Outlook guarda una copia del correo en el equipo.", worry: "Si pasa de 50 GB, Outlook se vuelve lento o deja de sincronizar.", todo: "Solucionar problemas → Outlook y Office lo mide.", aliases: ["pst"] },
  { term: "WinPE", what: "Un Windows mínimo que arranca desde un pendrive.", worry: "—", todo: "Sirve para rescatar equipos que no arrancan: Herramientas → Pendrive de rescate." },
  { term: "Driver Verifier", what: "Herramienta de Windows que somete a los drivers a pruebas duras para cazar el que falla.", worry: "Puede provocar pantallazos a propósito: úsalo con vuelta atrás.", todo: "Arranques y cuelgues → Driver Verifier guiado." },
  { term: "Pantallazo azul", what: "Windows se para para no dañar nada cuando algo grave falla (casi siempre un driver).", worry: "Uno suelto pasa; varios con el mismo culpable, no.", todo: "Estado del equipo → Arranques y cuelgues agrupa los pantallazos por causa.", aliases: ["bsod", "pantalla azul"] },
  { term: "Point and Print", what: "Directiva que controla quién puede instalar drivers de impresora desde un servidor.", worry: "Desde 2021 Windows pide administrador por defecto (PrintNightmare).", todo: "Impresoras → explica la directiva y cómo instalar con permisos." },
  { term: "SPF", what: "Registro del dominio que dice qué servidores pueden enviar correo en su nombre.", worry: "Si falta o está incompleto, los correos acaban en spam.", todo: "Red → Correo del dominio lo comprueba." },
  { term: "DKIM", what: "Firma de los correos del dominio para demostrar que no se han cambiado.", worry: "Sin DKIM, Gmail y Outlook desconfían de los correos.", todo: "Lo configura el proveedor del correo." },
  { term: "DMARC", what: "Regla del dominio que dice qué hacer con los correos que no pasan SPF o DKIM.", worry: "Sin DMARC cualquiera puede suplantar el dominio con más facilidad.", todo: "Red → Correo del dominio da el texto para crearlo." },
];

const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().trim();

/** Busca un término (o uno de sus otros nombres). */
export function lookupTerm(word: string): GlossaryEntry | null {
  const w = fold(word);
  return GLOSSARY.find((g) => fold(g.term) === w || (g.aliases ?? []).some((a) => fold(a) === w)) ?? null;
}

/** El primer término del glosario que aparece dentro de un texto («Errores CRC del cable» → CRC). */
export function termIn(text: string): GlossaryEntry | null {
  const t = ` ${fold(text).replace(/[^a-z0-9ñ]+/g, " ")} `;
  for (const g of GLOSSARY)
    for (const name of [g.term, ...(g.aliases ?? [])]) {
      const n = fold(name).replace(/[^a-z0-9ñ]+/g, " ").trim();
      if (n.length >= 3 && t.includes(` ${n} `)) return g;
    }
  return null;
}
