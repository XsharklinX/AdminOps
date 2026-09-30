// De qué equipo estamos hablando.
//
// AdminOps se usa tanto en el portátil del técnico como en el equipo de un
// cliente que tiene ocho años y 4 GB de RAM. Lo que en el primero es cómodo
// (precargar el correo, refrescar a menudo) en el segundo lo deja inservible, y
// es justo en ese donde más falta hace que la herramienta funcione.
//
// WebView2 es Chromium, así que expone los dos datos que hacen falta sin
// preguntarle nada al sistema: núcleos y memoria aproximada.

interface ChromiumNavigator extends Navigator {
  /** GB de RAM redondeados a la baja a una potencia de 2, con tope de 8. */
  deviceMemory?: number;
}

/** Núcleos lógicos, o 4 si el navegador no lo dice. */
export const cores = (): number => navigator.hardwareConcurrency || 4;

/** GB de RAM aproximados, o 8 si el navegador no lo dice (no dar por malo lo que no se sabe). */
export const memoryGb = (): number => (navigator as ChromiumNavigator).deviceMemory ?? 8;

/**
 * ¿Es un equipo justo de recursos? Con 4 GB o menos, o 4 núcleos o menos,
 * AdminOps se aparta: nada de precargas ni de trabajo de fondo que no haya
 * pedido el técnico.
 *
 * El umbral es deliberadamente generoso. Equivocarse por exceso solo cuesta que
 * un portal tarde un segundo más en abrir; equivocarse por defecto cuesta que la
 * aplicación se congele en el equipo de un cliente, delante del cliente.
 */
export const isModestMachine = (): boolean => memoryGb() <= 4 || cores() <= 4;

/** Para el registro técnico y Ajustes → Rendimiento. */
export const machineSummary = (): string => `${cores()} núcleos · ~${memoryGb()} GB${isModestMachine() ? " · equipo justo de recursos" : ""}`;
