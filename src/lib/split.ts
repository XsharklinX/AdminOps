// Pantalla dividida: el portal a la izquierda y otra página de AdminOps a la derecha.
//
// Se lee el ticket mientras se resuelve, sin ir y volver. Una sola lista de qué
// puede ir en cada lado, para que App y el selector del portal no se desacompasen.
import type { PageId } from "../components/Sidebar";

/** Portales que se pueden dividir: el ticket, el correo o Teams a la izquierda. */
export const SPLIT_LEFT: PageId[] = ["tickets", "mail", "teams"];

/**
 * Lo que puede ir a la derecha: páginas que se leen bien en media pantalla. Ningún
 * portal, porque dos vistas web nativas a la vez no caben ni se tapan bien.
 */
export const SPLIT_RIGHT: PageId[] = ["people", "troubleshoot", "printers", "contacts", "knowledge"];
