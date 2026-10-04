// Usuarios de este equipo: qué conviene revisar de cada cuenta. Solo con lo que
// Windows ya dice de ella; nada de probar contraseñas.
import type { LocalUser } from "./api";

export interface UserIssue {
  /** bad: no puede entrar o es un riesgo claro · warn: conviene mirarlo. */
  level: "bad" | "warn";
  text: string;
  /** Qué botón de la ficha lo arregla. */
  fix?: "password" | "enable" | "disable";
}

const DAY = 86_400_000;
/** Una cuenta activa que nadie usa desde hace este tiempo es una puerta que sobra. */
export const STALE_DAYS = 180;

export function userIssues(u: LocalUser, now = Date.now()): UserIssue[] {
  const out: UserIssue[] = [];
  if (u.builtin === "default" || u.builtin === "wdag") return out;

  if (u.builtin === "administrator" && u.enabled) {
    out.push({ level: "warn", text: "La cuenta «Administrador» integrada está activa. Windows la trae desactivada: es la primera que prueba cualquier ataque.", fix: "disable" });
  }
  if (u.builtin === "guest" && u.enabled) {
    out.push({ level: "warn", text: "La cuenta de invitado está activa: cualquiera puede usar el equipo sin contraseña.", fix: "disable" });
  }
  if (!u.enabled) return out;

  const expires = u.passwordExpires ? new Date(u.passwordExpires).getTime() : null;
  if (expires !== null && expires <= now) {
    out.push({ level: "bad", text: "La contraseña ha caducado: no puede entrar hasta cambiarla.", fix: "password" });
  } else if (expires !== null && expires - now <= 7 * DAY) {
    const days = Math.max(1, Math.ceil((expires - now) / DAY));
    out.push({ level: "warn", text: `La contraseña caduca en ${days} ${days === 1 ? "día" : "días"}.`, fix: "password" });
  }

  // Sin fecha de cambio: nunca se le puso contraseña (Windows no dice más).
  if (!u.builtin && !u.microsoft && !u.passwordLastSet) {
    out.push({
      level: u.admin ? "bad" : "warn",
      text: u.admin ? "Es administrador y parece no tener contraseña: cualquiera que se siente delante puede cambiar lo que quiera." : "Parece no tener contraseña.",
      fix: "password",
    });
  }

  if (!u.builtin && !u.signedIn && !u.isSelf && !u.isTarget) {
    const last = u.lastLogon ? new Date(u.lastLogon).getTime() : null;
    if (last !== null && now - last > STALE_DAYS * DAY) {
      const months = Math.floor((now - last) / (30 * DAY));
      out.push({ level: "warn", text: `Nadie la usa desde hace ${months} meses. Si ya no hace falta, desactívala.`, fix: "disable" });
    }
  }
  return out;
}

/** La más grave, para el punto de color de la lista. */
export function worstIssue(issues: UserIssue[]): "bad" | "warn" | null {
  return issues.some((i) => i.level === "bad") ? "bad" : issues.length ? "warn" : null;
}
