// Partes de AdminOps apagadas de momento.
//
// WEB_TICKETS: la pantalla Tickets y la pestaña «Inventario web» (las webs de tickets e
// inventario de la empresa dentro de AdminOps). Apagadas en la 1.4.0 por decisión de David:
// no funcionan como él quiere. El código sigue (Correo y Teams usan los mismos portales);
// los portales de Tickets o inventario que ya hubiera guardados no se borran, solo no se ven.
// Para recuperarlas: ponerlo a true y devolver los textos que se reescribieron sin ellas
// (Onboarding, CaseBar «Pegar en Tickets», FirstSteps, settings/catalog.ts, settings/Portals.tsx,
// settings/General.tsx, settings/Outbound.tsx, la ayuda de «inventory» en Sidebar.tsx y guide.ts).
export const WEB_TICKETS = false;
