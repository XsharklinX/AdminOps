// El último síntoma comprobado en «Solucionar problemas»: al cerrar el caso se
// propone como «qué problema era».
const KEY = "adminops.lastSymptom";

export function rememberSymptom(id: string) {
  try {
    sessionStorage.setItem(KEY, id);
  } catch {
    /* sin almacenamiento */
  }
}

export function lastSymptom(): string | null {
  try {
    return sessionStorage.getItem(KEY);
  } catch {
    return null;
  }
}
